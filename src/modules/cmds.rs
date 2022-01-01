use std::{
    collections::hash_map::HashMap,
    error::Error,
    ffi::{OsStr, OsString},
    io::Write,
    path::{Path, PathBuf},
};

use clap::Parser;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use crate::modules::utils;

type BytesIO = Vec<u8>;

#[derive(Parser, Debug)]
struct Opt {
    /// input image paths
    #[clap(required = false, default_value = "./*", display_order = 0)]
    input: Vec<PathBuf>,
    #[clap(short, takes_value = true, default_value = "./out")]
    out_dir: PathBuf,
    /// avaible presets:    {n}
    /// "cjxl:{args}", "avif:{args}", "jpeg:{args}", "cwebp:{args}", png:{} {n}
    /// custom cmd format:  {n}
    /// "{encoder}>:{decoder}>:{extension}>:{output_from_stdout [0;1]}:>{args}"
    #[clap(short, required = true)]
    cmds: Vec<String>,
    /// percentage tolerance of commands to the following ones{n}
    /// (cmd res. filesize to orig. filesize precentage){n} (when not saving all results)
    #[clap(short, long, default_value = "10")]
    tolerance: u32,
    /// save all encoded images (Not only the best compressed one)
    #[clap(long = "save")]
    save_all: bool,
    /// save information to csv table
    #[clap(long = "csv")]
    save_csv: bool,
    /// path for csv table
    #[clap(long = "csv_path", default_value = "./res.csv")]
    csv_path: PathBuf,
    #[clap(long, default_value = "0")]
    nproc: usize,
}

pub fn main(args: Vec<OsString>) -> Result<(), Box<dyn Error>> {
    // if args.is_empty() {
    //     args = std::env::args_os().collect();
    // }
    let opt = Opt::parse_from(args);

    let csv_path = &opt.csv_path;

    let mut images = opt.input.to_owned();
    utils::ims_init(&mut images, &opt.out_dir, Some(opt.nproc))?;

    if opt.save_csv {
        let csv_file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .append(true)
            .open(csv_path)
            .unwrap();

        let mut csv_writer = csv::WriterBuilder::new()
            .delimiter(b'\t')
            .from_writer(csv_file);

        // csv header row
        let mut csv_row = Vec::from(["", ""]);
        for cmd in &opt.cmds {
            csv_row.push(cmd);
        }
        csv_writer.write_record(csv_row)?;
        csv_writer.flush()?;
    }

    let mut resultmap: HashMap<String, usize> = HashMap::new();
    for img in images {
        match process_image(&img, csv_path, &opt) {
            Ok(c) => *resultmap.entry(c).or_default() += 1,
            Err(e) => println!("Can't process image {}: {}", &img.display(), &e),
        }
    }
    println!("stats: \ncount\t cmd");
    resultmap
        .iter()
        .for_each(|(k, v)| println!("{}\t {}", v, k));

    Ok(())
}

/// Generate results from cmds and compare/save/output them
fn process_image(
    img: &Path,
    csv_path: &Path,
    opt: &Opt,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    println!("{}", &img.display());
    let img_filesize = img.metadata()?.len() as u32;
    let img_dimensions = image::image_dimensions(&img)?;
    let px_count = img_dimensions.0 * img_dimensions.1;

    let out_dir = &opt.out_dir;

    // generate results in ImageBuffers for each cmd
    let enc_img_buffers: Vec<ImageBuffer> = opt
        .cmds
        .par_iter()
        .map(|cmd| {
            let mut buff = ImageBuffer::new();
            buff.image_generate(img, cmd).map(|_| buff)
        })
        .collect::<Result<_, _>>()?;

    let mut res_filesize: u32 = 0;
    let mut res_buff = &ImageBuffer::new();

    // csv | open writer, push orig image filename&size
    let save_csv = opt.save_csv;
    let mut csv_row = Vec::<String>::new();

    let mut csv_writer = if save_csv {
        let csv_file = std::fs::OpenOptions::new()
            .write(true)
            .append(true)
            .open(csv_path)?;
        csv_row.push(img.to_string_lossy().to_string());
        csv_row.push(img_filesize.to_string());
        Some(
            csv::WriterBuilder::new()
                .delimiter(b'\t')
                .from_writer(csv_file),
        )
    } else {
        None
    };

    // Caclculate & print info for each ImageBuffer
    for (i, buff) in enc_img_buffers.iter().enumerate() {
        let buff_filesize = buff.get_image_size() as u32;
        let buff_bpp = (buff_filesize * 8) as f64 / px_count as f64;
        let percentage_of_original = format!("{:.2}", (100 * buff_filesize / img_filesize));
        let printing_status = format!(
            "{}\n{} --> {}\t{:6.2}bpp\t{}%\t{:>6.2}s",
            &buff.get_cmd(),
            byte2size(img_filesize as u64),
            byte2size(buff_filesize as u64),
            &buff_bpp,
            percentage_of_original,
            &buff.ex_time.as_secs_f32(),
        );

        println!("{}", printing_status);

        if save_csv {
            csv_row.push(buff_filesize.to_string());
        }

        if opt.save_all {
            // save each buffer to save_path
            if buff_filesize == 0 {
                continue;
            }
            let save_path = out_dir.join(format!(
                "{}_{}.{}",
                img.file_stem()
                    .and_then(OsStr::to_str)
                    .ok_or_else(|| format!("No filestem: {}", img.display()))?,
                i.to_string(),
                &buff.ext
            ));
            let mut f = std::fs::File::create(save_path)?;
            f.write_all(&buff.image)?;
            continue;
        }

        // difference between the res_buf and current buf
        // to orig file percentages must be greater than the tolerance
        let tolerance = opt.tolerance as f64; // %

        if res_filesize == 0
            || buff_filesize != 0
                && (res_filesize as f64 - buff_filesize as f64)
                    > img_filesize as f64 * tolerance * 0.01
        {
            res_buff = buff;
            res_filesize = buff_filesize;
        }
    }

    if save_csv {
        let w = csv_writer.as_mut().unwrap();
        w.write_record(&csv_row)?;
        w.flush()?;
    }

    if opt.save_all {
        return Ok("".into());
    }
    // save res_buf
    let save_path = out_dir.join(format!(
        "{}.{}",
        img.file_stem()
            .and_then(OsStr::to_str)
            .ok_or_else(|| format!("No filestem: {}", img.display()))?,
        &res_buff.ext
    ));
    let mut f = std::fs::File::create(save_path)?;
    f.write_all(&res_buff.image)?;
    println!("save {}\n", &res_buff.cmd);

    Ok(res_buff.cmd.to_string())
}

#[derive(Debug, Clone)]
struct ImageBuffer<'a> {
    image: BytesIO,
    cmd: &'a str,
    cmd_enc: String,
    cmd_enc_args: Vec<String>,
    cmd_enc_output_from_stdout: bool,
    cmd_dec: String,
    cmd_dec_args: Vec<String>,
    ext: String,
    ex_time: core::time::Duration,
}

impl<'a> ImageBuffer<'a> {
    fn new() -> ImageBuffer<'static> {
        ImageBuffer {
            image: Vec::new(),
            cmd: "",
            cmd_enc: String::new(),
            cmd_enc_args: Vec::new(),
            cmd_enc_output_from_stdout: false,
            cmd_dec: String::new(),
            cmd_dec_args: Vec::new(),
            ext: String::new(),
            ex_time: core::time::Duration::new(0, 0),
        }
    }

    fn get_image_size(&self) -> usize {
        core::mem::size_of_val(&self.image[..])
    }

    fn get_cmd(&self) -> String {
        self.cmd_enc.to_string() + " " + &self.cmd_enc_args.join(" ")
    }

    fn image_generate(
        &mut self,
        img_path: &Path,
        cmd: &'a str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.cmd = cmd;
        let cmd_args: Vec<String> = cmd.split(">:").map(|s| s.to_owned()).collect();
        let time_start = std::time::Instant::now();

        if cmd_args.len().eq(&1) {
            self.image_generate_preset(img_path, cmd)?;
        } else {
            self.image_generate_custom(img_path, &cmd_args)?;
        }
        self.ex_time = time_start.elapsed();
        Ok(())
    }

    fn image_generate_preset(
        &mut self,
        img_path: &Path,
        cmd: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let cmd_preset = cmd.split_once(":").ok_or("Cmd argument error")?.0;
        self.cmd_enc_args = cmd
            .split_once(":")
            .ok_or("Cmd subargument error")?
            .1
            .split(' ')
            .map(|s| s.to_owned())
            .collect();
        match cmd_preset {
            "jpeg" => {
                self.ext = "jpg".into();
                self.cmd_enc = "cjpeg".into();
                self.cmd_dec = "djpeg".into();
                self.cmd_enc_output_from_stdout = true;
            }
            // TODO use image crate to convert into png
            "png" => {
                self.ext = "png".into();
                self.cmd_enc = "convert".into();
                self.cmd_dec = "convert".into();
            }
            "cjxl" => {
                self.ext = "jxl".into();
                self.cmd_enc = "cjxl".into();
                self.cmd_dec = "djxl".into();
            }
            "avif" => {
                self.ext = "avif".into();
                self.cmd_enc = "avifenc".into();
                self.cmd_dec = "avifdec".into();
            }
            "cwebp" => {
                self.ext = "webp".into();
                self.cmd_enc = "cwebp".into();
                self.cmd_dec = "dwebp".into();
                self.cmd_dec_args = vec!["-o".into()];
            }
            _ => panic!("match error, cmd '{}' not supported", &cmd_preset),
        }
        self.gen_from_cmd(img_path)?;
        Ok(())
    }

    fn image_generate_custom(
        &mut self,
        img_path: &Path,
        cmd_args: &[String],
    ) -> std::io::Result<()> {
        self.cmd_enc_args = cmd_args[4].split(' ').map(|s| s.to_owned()).collect();
        self.ext = cmd_args[0].to_string();
        self.cmd_enc = cmd_args[1].to_string();
        self.cmd_dec = cmd_args[2].to_string();
        self.cmd_enc_output_from_stdout = cmd_args[3]
            .parse::<u8>()
            .expect("wrong 'output_from_stdout' flag")
            .ne(&0);
        self.gen_from_cmd(img_path)?;
        Ok(())
    }

    // fn image_decode(&self) -> Result<tempfile::NamedTempFile, Box<dyn Error>> {
    //     let mut tf = tempfile::NamedTempFile::new()?;
    //     let tf_out = tempfile::Builder::new().suffix(".png").tempfile()?;
    //     tf.write_all(&self.image)?;
    //     let outp = std::process::Command::new(&self.cmd_dec)
    //         .arg(tf.path())
    //         .args(&self.cmd_dec_args)
    //         .arg(tf_out.path())
    //         .output()?;
    //     command_print_if_error(&outp)?;
    //     Ok(tf_out)
    // }

    fn gen_from_cmd(&mut self, img_path: &Path) -> std::io::Result<()> {
        // no arguments -> return None
        if self.cmd_enc_args.contains(&"".into()) {
            self.cmd_enc_args.pop();
        }

        if self.cmd_enc_output_from_stdout {
            let output = std::process::Command::new(&self.cmd_enc)
                .args(&self.cmd_enc_args)
                .arg(img_path)
                .output()?;
            self.image = output.stdout;
        } else {
            let buffer = tempfile::Builder::new()
                .suffix(&format!(".{}", self.ext))
                .tempfile()?;
            let outp = std::process::Command::new(&self.cmd_enc)
                .arg(img_path)
                .args(&self.cmd_enc_args)
                .arg(buffer.path())
                .output()?;
            command_print_if_error(&outp)?;
            self.image = std::fs::read(buffer.path())?;
            buffer.close()?;
            // println!("{}", std::str::from_utf8(&output.stderr).unwrap());
        }
        Ok(())
    }
}

fn command_print_if_error(output: &std::process::Output) -> std::io::Result<()> {
    if output.status.success() {
        Ok(())
    } else {
        let o = [&output.stderr, "\n".as_bytes(), &output.stdout].concat();
        println!("{}", std::str::from_utf8(&o).unwrap());
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Command returned error status",
        ))
    }
}

fn byte2size(num: u64) -> String {
    let mut num_f = num as f64;
    for unit in ["", "K", "M", "G"].iter() {
        if num_f < 1024.0 {
            return format!("{:3.1}{}iB", num_f, unit);
        }
        num_f /= 1024.0;
    }
    return format!("{:3.1}TiB", num_f);
}

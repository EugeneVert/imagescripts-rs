// TODO: piping input file list, dry run;

use std::{
    error::Error,
    ffi::OsString,
    io::{BufRead, Write},
    path::Path,
};

use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use structopt::StructOpt;

use crate::modules::utils;

type BytesIO = Vec<u8>;

#[derive(StructOpt, Debug)]
#[structopt(name = "imagescripts-rs", about = " ")]
struct Opt {
    #[structopt(required = false, default_value = "./*", display_order = 0)]
    input: Vec<String>,
    #[structopt(short, takes_value = true, default_value = "./out")]
    out_dir: std::path::PathBuf,
    /// avaible presets:    {n}
    /// "cjxl:{args}", "avif:{args}"   {n}
    /// custom cmd format:  {n}
    /// "{encoder}>:{decoder}>:{extension}>:{output_from_stdout [0;1]}:>{args}"
    #[structopt(short, required = true)]
    cmds: Vec<String>,
    #[structopt(short, long, default_value = "10")]
    tolerance: u32,
    #[structopt(long = "save")]
    save_all: bool,
    #[structopt(long = "csv")]
    save_csv: bool,
    #[structopt(long = "csv_path", default_value = "./res.csv")]
    csv_path: String,
    #[structopt(long = "metrics")]
    do_metrics: bool,
    #[structopt(long, default_value = "0")]
    nproc: usize,
}

pub fn main(args: Vec<OsString>) -> Result<(), Box<dyn Error>> {
    // if args.is_empty() {
    //     args = std::env::args_os().collect();
    // }
    let opt = Opt::from_iter(args);

    let csv_path = &opt.csv_path;

    let mut opt_metrics = ImageMetricsOptions::new();
    if opt.do_metrics && opt_metrics.check_availability() {
        opt_metrics.do_metrics = true;
        opt_metrics.check_availability();
        let metrics = opt_metrics.list_avaible();
        println!("Metrics: {:?}", &metrics);
    }

    let mut images = opt.input.to_owned();
    utils::ims_init(&mut images, &opt.out_dir, Some(opt.nproc));

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
    
    for img in images {
        process_image(&img, &csv_path, &opt, &opt_metrics)?;
    }

    Ok(())
}

/// Generate results from cmds and compare/save/output them
fn process_image(
    img: &str,
    csv_path: &str,
    opt: &Opt,
    opt_metrics: &ImageMetricsOptions,
) -> Result<(), Box<dyn Error>> {
    println!("{}", &img);
    let img_filesize = Path::new(img).metadata().unwrap().len() as u32;
    let img_dimensions = image::image_dimensions(&img)?;
    let px_count = img_dimensions.0 * img_dimensions.1;

    let out_dir = &opt.out_dir;

    // generate results in ImageBuffers for each cmd
    let enc_img_buffers: Vec<ImageBuffer> = opt
        .cmds
        .par_iter()
        .map(|cmd| {
            let mut buff = ImageBuffer::new();
            buff.image_generate(&img, &cmd);
            buff
        })
        .collect();

    let mut res_filesize: u32 = 0;
    let mut res_buff = ImageBuffer::new();

    // csv | open writer, push orig image filename&size
    let save_csv = opt.save_csv;
    let mut csv_row = Vec::<String>::new();
    let mut csv_writer = if save_csv {
        let csv_file = std::fs::OpenOptions::new()
            .write(true)
            .append(true)
            .open(csv_path)?;
        csv_row.push(img.to_string());
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
        let mut printing_status = format!(
            "{}\n{} --> {}\t{:6.2}bpp\t{:>6.2}s \t{}%\n",
            &buff.get_cmd(),
            byte2size(img_filesize as u64),
            byte2size(buff_filesize as u64),
            &buff_bpp,
            &buff.ex_time.as_secs_f32(),
            percentage_of_original
        );
        if opt_metrics.do_metrics {
            let img_wa = image_remove_alpha(Path::new(&img))?;
            let img_wa_path = img_wa.path().to_str().unwrap().to_string();
            let img_distorted = buff.image_decode()?;
            let img_distorted_wa = image_remove_alpha(img_distorted.path())?;
            let img_distorted_path = img_distorted_wa.path().to_str().unwrap().to_string();
            img_distorted.close()?;
            if opt_metrics.butteraugli {
                let m = opt_metrics.butteraugli_run(&img_wa_path, &img_distorted_path)?;
                let butteraugli_max_norm = &m[0];
                let butteraugli_pnorm = m[1].split_once(":").unwrap().1;
                printing_status = printing_status
                    + "butteraugli_max_norm: "
                    + butteraugli_max_norm
                    + "\t butteraugli_pnorm: "
                    + butteraugli_pnorm
                    + "\t "
            }
            if opt_metrics.ssimulacra {
                let m = opt_metrics.ssimulacra_run(&img_wa_path, &img_distorted_path)?;
                let ssimulacra = &m;
                printing_status = printing_status
                    + "ssimulacra: "
                    + ssimulacra
            }
            println!("{}", printing_status);
        }

        if save_csv {
            csv_row.push(buff_filesize.to_string());
        }

        if opt.save_all {
            if buff_filesize == 0 {
                continue;
            }
            let save_path = out_dir.join(format!(
                "{}_{}.{}",
                Path::new(img).file_stem().unwrap().to_str().unwrap(),
                i.to_string(),
                &buff.ext
            ));
            let mut f = std::fs::File::create(save_path)?;
            f.write_all(&buff.image[..])?;
            continue;
        }

        let tolerance = opt.tolerance as f64; // %
                                              // Commands has value tolerance over next ones
        if (res_filesize == 0
            || (buff_filesize as f64) < (res_filesize as f64) * (1.0 - tolerance * 0.01))
            && buff_filesize != 0
        {
            res_buff = buff.clone();
            res_filesize = buff_filesize;
        }
    }

    if save_csv {
        let w = csv_writer.as_mut().unwrap();
        w.write_record(&csv_row)?;
        w.flush()?;
    }

    if opt.save_all {
        return Ok(());
    }
    // save res
    let save_path = out_dir.join(format!(
        "{}.{}",
        Path::new(img).file_stem().unwrap().to_str().unwrap(),
        &res_buff.ext
    ));
    let mut f = std::fs::File::create(save_path)?;
    f.write_all(&res_buff.image[..]).unwrap();

    Ok(())
}

#[derive(Debug, Clone)]
struct ImageMetricsOptions {
    do_metrics: bool,
    butteraugli: bool,
    ssimulacra: bool,
}

impl ImageMetricsOptions {
    fn new() -> ImageMetricsOptions {
        ImageMetricsOptions {
            do_metrics: false,
            butteraugli: false,
            ssimulacra: false,
        }
    }
    /// Checks the metrics avaibility in path and sets the corresponding struct field to `true`
    /// # Returns
    /// `1` if any metric is avaible
    fn check_availability(&mut self) -> bool {
        if is_program_in_path("butteraugli_main") {
            self.butteraugli = true;
        }
        if is_program_in_path("ssimulacra") {
            self.ssimulacra = true;
        }
        self.list_avaible().len().ne(&0)
    }
    /// returns a vec of avaible metrics
    fn list_avaible(&self) -> Vec<String> {
        let mut m: Vec<String> = Vec::new();
        if self.butteraugli {
            m.push("butteraugli max norm".into());
            m.push("butteraugli pnorm".into());
        }
        if self.ssimulacra {
            m.push("ssimulacra".into());
        }
        m
    }
    fn butteraugli_run(
        &self,
        original: &str,
        distorted: &str,
    ) -> Result<Vec<String>, Box<dyn Error>> {
        let outp = std::process::Command::new("butteraugli_main")
            .arg(original)
            .arg(distorted)
            .output()?;
        command_print_if_error(&outp)?;
        Ok(outp
            .stdout
            .lines()
            .map(|l| l.unwrap_or_else(|_| "-".into()))
            .collect())
    }
    fn ssimulacra_run(&self, original: &str, distorted: &str) -> Result<String, Box<dyn Error>> {
        let outp = std::process::Command::new("ssimulacra")
            .arg(original)
            .arg(distorted)
            .output()?;
        command_print_if_error(&outp)?;
        Ok(outp
            .stdout
            .lines()
            .next()
            .unwrap_or_else(|| Result::Ok("-".into()))
            .unwrap())
    }
}

#[derive(Debug, Clone)]
struct ImageBuffer {
    image: BytesIO,
    cmd_enc: String,
    cmd_enc_args: Vec<String>,
    cmd_enc_output_from_stdout: bool,
    cmd_dec: String,
    cmd_dec_args: Vec<String>,
    ext: String,
    ex_time: core::time::Duration,
}

impl ImageBuffer {
    fn new() -> ImageBuffer {
        ImageBuffer {
            image: Vec::new(),
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

    fn image_generate(&mut self, img_path: &str, cmd: &str) {
        let cmd_args: Vec<String> = cmd.split(">:").map(|s| s.to_owned()).collect();
        let time_start = std::time::Instant::now();

        if cmd_args.len().eq(&1) {
            self.image_generate_preset(img_path, cmd);
        } else {
            self.image_generate_custom(img_path, cmd_args);
        }
        self.ex_time = time_start.elapsed();
    }

    fn image_generate_preset(&mut self, img_path: &str, cmd: &str) {
        let cmd_preset = cmd.split_once(":").expect("Cmd argument error").0;
        self.cmd_enc_args = cmd
            .split_once(":")
            .unwrap()
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
        self.gen_from_cmd(img_path).unwrap();
    }

    fn image_generate_custom(&mut self, img_path: &str, cmd_args: Vec<String>) {
        self.cmd_enc_args = cmd_args
            .get(4)
            .unwrap()
            .split(' ')
            .map(|s| s.to_owned())
            .collect();
        self.ext = cmd_args.get(0).unwrap().into();
        self.cmd_enc = cmd_args.get(1).unwrap().into();
        self.cmd_dec = cmd_args.get(2).unwrap().into();
        self.cmd_enc_output_from_stdout = cmd_args.get(3).unwrap().parse::<u8>().unwrap().ne(&0);
        self.gen_from_cmd(img_path).unwrap();
    }

    fn image_decode(&self) -> Result<tempfile::NamedTempFile, Box<dyn Error>> {
        let mut tf = tempfile::NamedTempFile::new()?;
        let tf_out = tempfile::Builder::new().suffix(".png").tempfile()?;
        tf.write_all(&self.image)?;
        let outp = std::process::Command::new(&self.cmd_dec)
            .arg(tf.path())
            .args(&self.cmd_dec_args)
            .arg(tf_out.path())
            .output()?;
        command_print_if_error(&outp)?;
        Ok(tf_out)
    }

    fn gen_from_cmd(&mut self, img_path: &str) -> Result<(), Box<dyn Error>> {
        // no arguments -> return None
        if self.cmd_enc_args.contains(&"".into()) {
            self.cmd_enc_args.pop();
        }
        if self.cmd_enc_output_from_stdout {
            let output = std::process::Command::new(&self.cmd_enc)
                .args(&self.cmd_enc_args)
                .arg(img_path)
                .output()
                .unwrap();
            self.image = output.stdout;
        } else {
            let buffer = tempfile::Builder::new()
                .suffix(&format!(".{}", self.ext))
                .tempfile()
                .unwrap();
            let outp = std::process::Command::new(&self.cmd_enc)
                .arg(img_path)
                .args(&self.cmd_enc_args)
                .arg(buffer.path())
                .output()
                .unwrap();
            command_print_if_error(&outp)?;
            self.image = std::fs::read(buffer.path()).unwrap();
            buffer.close().unwrap();
            // println!("{}", std::str::from_utf8(&output.stderr).unwrap());
        }
        Ok(())
    }
}

fn image_remove_alpha(img: &Path) -> Result<tempfile::NamedTempFile, Box<dyn Error>> {
    let tf = tempfile::Builder::new().suffix(".png").tempfile()?;
    // TODO find a way to do alpha-removal w/o ImageMagick
    // let img = image::open(img)?.to_rgb8();
    // img.save_with_format(tf.path(), image::ImageFormat::Png)?;

    // ImageMagick gives back accurate image
    let outp = std::process::Command::new("convert")
        .arg(img)
        .args(["-alpha", "off"])
        .arg("PNG24:".to_string() + tf.path().to_str().unwrap())
        .output()?;
    command_print_if_error(&outp)?;
    Ok(tf)
}

fn command_print_if_error(output: &std::process::Output) -> Result<(), Box<dyn Error>> {
    if output.status.success() {
        Ok(())
    } else {
        println!("{}", std::str::from_utf8(&output.stderr).unwrap());
        println!("{}", std::str::from_utf8(&output.stdout).unwrap());
        Err("Command returned error status".into())
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

fn is_program_in_path(program: &str) -> bool {
    std::process::Command::new(program)
        .spawn()
        .and_then(|mut x| x.kill())
        .is_ok()
}

// TODO: piping input file list, dry run;

use std::{env::temp_dir, error::Error, ffi::OsString, io::{BufRead, Write}, path::Path, str::FromStr};

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
    #[structopt(short, required = true)]
    cmds: Vec<String>,
    #[structopt(short, long, default_value = "10")]
    tolerance: u32,
    #[structopt(long = "save")]
    save_all: bool,
    #[structopt(long = "csv")]
    save_csv: bool,
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

    let csv_path = "./res.csv";

    let mut opt_metrics = ImageMetricsOptions::new();
    if opt.do_metrics {
        opt_metrics.do_metrics = true;
        opt_metrics.check_availability();
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
        let metrics = opt_metrics.list_avaible();
        let mut csv_row = Vec::from(["", ""]);
        for cmd in &opt.cmds {
            csv_row.push(cmd);
            for metric in &metrics {
                csv_row.push(metric);
            }
        }
        csv_writer.write_record(csv_row)?;
        csv_writer.flush()?;
    }

    for img in images {
        process_image(&img, csv_path, &opt, &opt_metrics)?;
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
            let mut buff = ImageBuffer::new(&cmd);
            buff.image_generate(&img);
            buff
        })
        .collect();

    let mut res_filesize: u32 = 0;
    let mut res_buff = ImageBuffer::new("");

    // csv | open writer, push orig image filename&size
    let save_csv = opt.save_csv;
    let mut csv_row = Vec::<String>::new();
    let csv_writer = if save_csv {
        let csv_file = std::fs::OpenOptions::new()
            .write(true)
            .append(true)
            .open(csv_path)
            .unwrap();
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
        println!(
            "{} --> {}\t{:6.2}bpp\t{:>6.2}s \t{}%",
            byte2size(img_filesize as u64),
            byte2size(buff_filesize as u64),
            buff_bpp,
            buff.ex_time.as_secs_f32(),
            percentage_of_original
        );

        if save_csv {
            csv_row.push(buff_filesize.to_string());
            if opt_metrics.do_metrics {
                let buff_img_decoded = buff.image_decode();
                if opt_metrics.butteraugli {
                    let m = opt_metrics
                        .butteraugli_run(img, buff_img_decoded.path().to_str().unwrap());
                    csv_row.push(m[0].to_owned());
                    csv_row.push(m[1].split_once(":").unwrap().1.to_owned());
                    buff_img_decoded.close().unwrap();
                }
            }
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
            f.write_all(&buff.image[..]).unwrap();
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
        let mut w = csv_writer.unwrap();
        w.write_record(csv_row)?;
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
}

impl ImageMetricsOptions {
    fn new() -> ImageMetricsOptions {
        ImageMetricsOptions {
            do_metrics: false,
            butteraugli: false,
        }
    }
    fn check_availability(&mut self) {
        if is_program_in_path("butteraugli") {
            self.butteraugli = true;
        }
    }
    fn list_avaible(&self) -> Vec<String> {
        let mut m: Vec<String> = Vec::new();
        if self.butteraugli {
            m.push("butteraugli max norm".into());
            m.push("butteraugli pnorm".into());
        }
        m
    }
    fn butteraugli_run(&self, img_orig: &str, img_distort: &str) -> Vec<String> {
        let out = std::process::Command::new("butteraugli")
            .arg(img_orig)
            .arg(img_distort)
            .output()
            .unwrap()
            .stdout;
        // println!("{:?}", String::from_utf8(out.to_owned()));
        out.lines().map(|l| l.unwrap()).collect()
    }
}

#[derive(Debug, Clone)]
struct ImageBuffer {
    image: BytesIO,
    cmd: String,
    ext: String,
    ex_time: core::time::Duration,
}

impl ImageBuffer {
    fn new(cmd_in: &str) -> ImageBuffer {
        ImageBuffer {
            image: Vec::new(),
            cmd: String::from(cmd_in),
            ext: String::new(),
            ex_time: core::time::Duration::new(0, 0),
        }
    }

    fn get_image_size(&self) -> usize {
        core::mem::size_of_val(&self.image[..])
    }

    fn set_ext(&mut self, i: &str) {
        self.ext = String::from_str(i).unwrap();
    }

    fn image_generate(&mut self, img_path: &str) {
        let cmd_cmd = self.cmd.split_once(":").expect("Cmd argument error").0;
        // for i in cmd_args {
        //     match i {
        //         "alpha" =>
        //     }
        // }
        let time_start = std::time::Instant::now();
        match cmd_cmd {
            "image" => {}
            "jpeg" => self.gen_from_cmd(img_path, "cmozjpeg", "jpg", true),
            "cjxl" => self.gen_from_cmd(img_path, "cjxl", "jxl", false),
            "avif" => self.gen_from_cmd(img_path, "avifenc", "avif", false),
            "cwebp" => self.gen_from_cmd(img_path, "cwebp", "webp", false),
            _ => {
                panic!("match error, cmd '{}' not supported", &cmd_cmd)
            }
        }
        self.ex_time = time_start.elapsed();
    }

    fn image_decode(&self) -> tempfile::NamedTempFile {
        let mut tf = tempfile::NamedTempFile::new().unwrap();
        let tf_out = tempfile::Builder::new().suffix(".png").tempfile().unwrap();
        tf.write_all(&self.image).unwrap();
        let decoder = match self.ext.as_str() {
            "jxl" => "djxl",
            "avif" => "avifdec",
            _ => todo!()
        };
        std::process::Command::new(decoder)
            .arg(tf.path())
            .arg(tf_out.path()).output().unwrap();
        tf_out
    }

    fn gen_from_cmd(&mut self, img_path: &str, cmd: &str, ext: &str, img_from_stdout: bool) {
        let mut cmd_args: Vec<&str> = self.cmd.split_once(":").unwrap().1.split(' ').collect();
        // no arguments -> return None
        if cmd_args.contains(&"") {
            cmd_args.pop();
        }

        if img_from_stdout {
            let output = std::process::Command::new(cmd)
                .args(cmd_args)
                .arg(img_path)
                .output()
                .unwrap();
            self.image = output.stdout;
        } else {
            let buffer = tempfile::Builder::new()
                .suffix(&format!(".{}", ext))
                .tempfile()
                .unwrap();
            std::process::Command::new(cmd)
                .args(cmd_args)
                .arg(img_path)
                .arg(buffer.path())
                .output()
                .unwrap();
            self.image = std::fs::read(buffer.path()).unwrap();
            buffer.close().unwrap();
        }
        self.set_ext(ext);

        // println!("{}", std::str::from_utf8(&output.stderr).unwrap());
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
    match std::process::Command::new(program)
        .spawn()
        .and_then(|mut x| x.kill())
    {
        Ok(_) => true,
        Err(_) => false,
    }
}

// TODO: piping input file list, for image in input, csv output, dry run;

use clap::{App, Arg};
use core::{mem::size_of_val, time::Duration};
use image::GenericImageView;
use std::{
    error::Error, ffi::OsString, fs::read, io::Write, path::Path, process, str::FromStr, time::Instant,
};

type BytesIO = Vec<u8>;

pub fn main(args: Vec<OsString>) -> Result<(), Box<dyn Error>> {
    // if args.is_empty() {
    //     args = std::env::args_os().collect();
    // }
    #[rustfmt::skip]
    let matches = App::new("imagescripts-rs")
        .about(" ")
        .arg(
            Arg::with_name("path")
                .takes_value(true)
                .required(false)
                .default_value(".")
                .display_order(0),
        )
        .arg(
            Arg::with_name("out_dir")
                .short("o")
                .takes_value(true)
                .required(false)
                .default_value("./out")
                .display_order(0),
        )
        .arg(
            Arg::with_name("cmds")
                .short("c")
                .multiple(true)
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("save_all")
                .long("save")
                .takes_value(false)
            )
        .arg(
            Arg::with_name("tolerance")
                .long("tolerance")
                .short("t")
                .takes_value(true)
                .default_value("10")  // %
        )
        .get_matches_from(args);

    let img = "./test.png";
    let img_image = image::open(img)?;

    let mut enc_img_buffers = Vec::<ImageBuffer>::new();
    for cmd in matches.values_of("cmds").unwrap() {
        let mut buff = ImageBuffer::new(cmd);
        buff.image_generate(&img);
        enc_img_buffers.push(buff);
    }

    let img_filesize = Path::new(img).metadata().unwrap().len() as u32;
    let img_dimensions = img_image.dimensions();
    let px_count = img_dimensions.0 * img_dimensions.1;

    let mut res_filesize: u32 = 0;
    let mut res_buff = ImageBuffer::new("");

    for (i, buff) in enc_img_buffers.iter().enumerate() {
        let buff_filesize = buff.get_image_size() as u32;
        let buff_bpp = (buff_filesize * 8) as f64 / px_count as f64;
        let percentage_of_original = format!("{:.2}", (100 * buff_filesize / img_filesize));
        println!(
            "{} --> {}\t{:.2}bpp\t{:.2}s\t{}%",
            byte2size(img_filesize as u64),
            byte2size(buff_filesize as u64),
            buff_bpp,
            buff.ex_time.as_secs_f32(),
            percentage_of_original
        );

        if matches.is_present("save_all") {
            if buff_filesize == 0 {
                continue;
            }
            let save_path = format!(
                "{}/{}_{}.{}",
                matches.value_of("out_dir").unwrap(),
                Path::new(img).file_stem().unwrap().to_str().unwrap(),
                i.to_string(),
                buff.ext
            );
            let mut f = std::fs::File::create(save_path)?;
            f.write_all(&buff.image[..]).unwrap();
            continue;
        }

        let tolerance = matches
            .value_of("tolerance")
            .unwrap()
            .parse::<f64>()
            .unwrap(); // %
                       // Commands has value tolerance over next ones
        if (res_filesize == 0
            || (buff_filesize as f64) < (res_filesize as f64) * (1.0 - tolerance * 0.01))
            && buff_filesize != 0
        {
            res_buff = buff.clone();
            res_filesize = buff_filesize;
        }
    }

    if matches.is_present("save_all") {
        return Ok(());
    }

    let save_path = format!(
        "{}/{}.{}",
        matches.value_of("out_dir").unwrap(),
        Path::new(img).file_stem().unwrap().to_str().unwrap(),
        res_buff.ext
    );
    let mut f = std::fs::File::create(save_path)?;
    f.write_all(&res_buff.image[..]).unwrap();

    Ok(())
}

#[derive(Debug, Clone)]
struct ImageBuffer {
    image: BytesIO,
    cmd: String,
    ext: String,
    ex_time: Duration,
}

impl ImageBuffer {
    fn new(cmd_in: &str) -> ImageBuffer {
        ImageBuffer {
            image: Vec::new(),
            cmd: String::from(cmd_in),
            ext: String::new(),
            ex_time: Duration::new(0, 0),
        }
    }

    fn get_image_size(&self) -> usize {
        size_of_val(&self.image[..])
    }

    fn set_ext(&mut self, i: &str) {
        self.ext = String::from_str(i).unwrap();
    }

    fn image_generate(&mut self, img_path: &str) {
        let cmd_arg = self.cmd.split_once(":").expect("Cmd argument error").0;
        let time_start = Instant::now();
        match cmd_arg {
            "image" => {}
            "cjxl" => self.gen_jxl(img_path),
            _ => {
                panic!("match error")
            }
        }
        self.ex_time = time_start.elapsed();
    }

    fn gen_jxl(&mut self, img_path: &str) {
        let buffer = tempfile::Builder::new().suffix(".jxl").tempfile().unwrap();
        let cmd_args = self.cmd.split_once(":").unwrap().1;

        process::Command::new("cjxl")
            .arg(img_path)
            .args(cmd_args.split(' '))
            .arg(buffer.path())
            .output()
            .unwrap();

        self.image = read(buffer.path()).unwrap();
        self.set_ext("jxl");
        buffer.close().unwrap();
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

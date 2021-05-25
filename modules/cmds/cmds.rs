use clap::{App, Arg};
use core::{mem::size_of_val, time::Duration};
use image::GenericImageView;
use std::{
    error::Error, ffi::OsString, fs::read, iter::Enumerate, path::Path, process, time::Instant,
};

type BytesIO = Vec<u8>;

pub fn main(args: Vec<OsString>) -> Result<(), Box<dyn Error>> {
    // if args.is_empty() {
    //     args = std::env::args_os().collect();
    // }
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
        .arg(Arg::with_name("save_all").long("save").takes_value(false))
        .get_matches_from(args);

    println!("{:?}", matches);

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
    let m = 0;

    for (i, buff) in enc_img_buffers.iter().enumerate() {
        let buff_filesize = buff.get_size() as u32;
        let buff_bpp = (buff_filesize * 8) / px_count;
        let percentage_of_original = format!("{:.2}", (100 * buff_filesize / img_filesize));
        println!("{:?}", percentage_of_original);
    }

    Ok(())
}

#[derive(Debug)]
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

    fn get_size(&self) -> usize {
        size_of_val(&self.image)
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
        buffer.close().unwrap();
    }
}

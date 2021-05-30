// TODO: piping input file list, dry run;

use core::{mem::size_of_val, time::Duration};
use std::{
    error::Error, ffi::OsString, fs::read, io::Write, path::Path, process, str::FromStr,
    time::Instant,
};

use image::GenericImageView;
use rayon::iter::{ParallelBridge, ParallelIterator};
use structopt::StructOpt;

type BytesIO = Vec<u8>;

pub fn main(args: Vec<OsString>) -> Result<(), Box<dyn Error>> {
    // if args.is_empty() {
    //     args = std::env::args_os().collect();
    // }

    let opt = Opt::from_iter(args);

    let csv_path = "./res.csv";
    let images = &opt.input;

    if !Path::new(&opt.out_dir).exists() {
        std::fs::create_dir_all(&opt.out_dir)
            .unwrap_or_else(|_| panic!("Error creating dir {}", &opt.out_dir));
    }

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
        let mut csv_row = Vec::from(["", ""]);
        for cmd in &opt.cmds {
            csv_row.push(cmd);
        }
        csv_writer.write_record(csv_row)?;
        csv_writer.flush()?;
    }

    rayon::ThreadPoolBuilder::new()
        .num_threads(opt.nproc)
        .build_global()?;
    images
        .iter()
        .par_bridge()
        .for_each(|img| process_image(&img, csv_path, &opt).unwrap());

    Ok(())
}

#[derive(StructOpt, Debug)]
#[structopt(name = "imagescripts-rs", about = " ")]
struct Opt {
    #[structopt(required = false, default_value = "./*", display_order = 0)]
    input: Vec<String>,
    #[structopt(short, takes_value = true, default_value = "./out")]
    out_dir: String,
    #[structopt(short)]
    cmds: Vec<String>,
    #[structopt(short, long, default_value = "10")]
    tolerance: u32,
    #[structopt(long = "save")]
    save_all: bool,
    #[structopt(long = "csv")]
    save_csv: bool,
    #[structopt(long, default_value = "0")]
    nproc: usize,
}

fn process_image(img: &str, csv_path: &str, opt: &Opt) -> Result<(), Box<dyn Error>> {
    let img_image = image::open(img)?;
    let img_filesize = Path::new(img).metadata().unwrap().len() as u32;
    let img_dimensions = img_image.dimensions();
    let px_count = img_dimensions.0 * img_dimensions.1;

    let out_dir = &opt.out_dir;

    let mut enc_img_buffers = Vec::<ImageBuffer>::new();
    for cmd in &opt.cmds {
        let mut buff = ImageBuffer::new(&cmd);
        buff.image_generate(&img);
        enc_img_buffers.push(buff);
    }

    let mut res_filesize: u32 = 0;
    let mut res_buff = ImageBuffer::new("");

    // if save_csv
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
    // //

    println!("{}", &img);
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
        }

        if opt.save_all {
            if buff_filesize == 0 {
                continue;
            }
            let save_path = format!(
                "{}/{}_{}.{}",
                out_dir,
                Path::new(img).file_stem().unwrap().to_str().unwrap(),
                i.to_string(),
                buff.ext
            );
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
    let save_path = format!(
        "{}/{}.{}",
        out_dir,
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

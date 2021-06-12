use std::{error::Error, ffi::OsString, path::Path};

use rayon::iter::{ParallelBridge, ParallelIterator};
use structopt::StructOpt;

use crate::modules::utils;

#[derive(StructOpt, Debug)]
#[structopt(name = "imagescripts-rs find", about = " ")]
struct Opt {
    #[structopt(required = false, default_value = "./*", display_order = 0)]
    input: Vec<String>,
    #[structopt(short, conflicts_with = "bigger")]
    lesser: Option<f32>,
    #[structopt(short, conflicts_with = "lesser")]
    bigger: Option<f32>,
    #[structopt(short = "mv")]
    mv: bool,
    #[structopt(long, default_value = "0")]
    nproc: usize,
}

pub fn main(args: Vec<OsString>) -> Result<(), Box<dyn Error>> {
    // if args.is_empty() {
    //     args = std::env::args_os().collect();
    // }
    let opt = Opt::from_iter(args);
    let out_dir = std::path::PathBuf::from(unwrap_two(opt.lesser, opt.lesser).to_string());
    let mut images = opt.input.to_owned();
    utils::ims_init(&mut images, &out_dir, Some(opt.nproc));

    images
        .iter()
        .par_bridge()
        .for_each(|img| process_image(&img, &out_dir, &opt).unwrap());

    Ok(())
}

fn unwrap_two<T>(l: Option<T>, b: Option<T>) -> T {
    match l {
        Some(val) => val,
        None => b.expect("Both options is None"),
    }
}

fn process_image(img: &str, out_dir: &std::path::Path, opt: &Opt) -> Result<(), Box<dyn Error>> {
    let img_filesize = Path::new(img).metadata().unwrap().len();
    let img_dimensions = image::image_dimensions(&img)?;
    let px_count = img_dimensions.0 * img_dimensions.1;
    let img_bpp = (img_filesize * 8) as f32 / px_count as f32;

    println!("File: {}\n bpp: {:.3}", img, img_bpp);
    let mut save_flag: bool = false;
    match opt.lesser {
        Some(val) => {
            if img_bpp < val {
                save_flag = true;
            }
        }
        None => {
            let val = opt.bigger.unwrap();
            if img_bpp > val {
                save_flag = true;
            }
        }
    }
    if save_flag {
        let save_path = out_dir.join(Path::new(img).file_name().unwrap());
        std::fs::rename(img, save_path)?;
    }

    Ok(())
}

use std::{error::Error, ffi::OsString, path::Path};

use rayon::iter::{ParallelBridge, ParallelIterator};
use structopt::StructOpt;

use crate::modules::utils;

#[rustfmt::skip]
#[derive(StructOpt, Debug)]
#[structopt(name = "imagescripts-rs find", about = " ")]
struct Opt {
    #[structopt(required = false, default_value = "./*", display_order = 0)]
    input: Vec<String>,
    #[structopt(short, required = false, default_value = "./grayscale", display_order = 0)]
    out_dir: std::path::PathBuf,
    #[structopt(short, default_value = "5")]
    threshold: u32,
    #[structopt(long, default_value = "0")]
    nproc: usize,
}

pub fn main(args: Vec<OsString>) -> Result<(), Box<dyn Error>> {
    let opt = Opt::from_iter(args);

    let mut images = opt.input.to_owned();
    utils::ims_init(&mut images, opt.out_dir.as_path(), Some(opt.nproc));

    images
        .iter()
        .par_bridge()
        .for_each(|img| process_image(&img, opt.out_dir.as_path(), &opt).unwrap());

    Ok(())
}

fn process_image(img: &str, out_dir: &std::path::Path, opt: &Opt) -> Result<(), Box<dyn Error>> {
    println!("File: {}", img);
    let img_image = image::open(&img)?;

    if !image_is_colorfull(img_image, opt.threshold) {
        let save_path = Path::new(out_dir).join(Path::new(img).file_name().unwrap());
        std::fs::rename(img, save_path)?;
    }

    Ok(())
}

fn image_is_colorfull(img: image::DynamicImage, threshold: u32) -> bool {
    let thumb_size = 32;
    if img.color().has_color() {
        let thumb = image::imageops::resize(
            &img.into_rgb8(),
            thumb_size,
            thumb_size,
            image::imageops::Nearest,
        );

        let mut is_colorfull = false;
        let res = thumb.pixels().par_bridge().find_map_any(|pix| {
            let pix = pix.0;
            let pix_r = *pix.get(0).unwrap() as i32;
            let pix_g = *pix.get(1).unwrap() as i32;
            let pix_b = *pix.get(2).unwrap() as i32;
            if std::cmp::max(
                std::cmp::max((pix_r - pix_g).abs(), (pix_r - pix_b).abs()),
                (pix_g - pix_b).abs(),
            ) > threshold as i32
            {
                return Some(());
            }
            None
        });
        if res.is_some() {
            is_colorfull = true;
        }
        return is_colorfull;
    }
    //else
    true
}

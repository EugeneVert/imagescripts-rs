use std::{error::Error, ffi::OsString, path::Path};

use clap::AppSettings;
use image::GenericImageView;
use rayon::iter::{ParallelBridge, ParallelIterator};
use structopt::StructOpt;

use crate::modules::utils;

#[rustfmt::skip]
#[derive(StructOpt, Debug)]
#[structopt(name = "imagescripts-rs find", about = " ")]
#[structopt(setting = AppSettings::ColoredHelp)]
struct Opt {
    #[structopt(required = false, default_value = "./*", display_order = 0)]
    input: Vec<String>,
    #[structopt(short, required = false, default_value = "./grayscale", display_order = 0)]
    out_dir: std::path::PathBuf,
    #[structopt(short, default_value = "5")]
    threshold: u8,
    #[structopt(long, default_value = "0")]
    nproc: usize,
}

pub fn main(args: Vec<OsString>) -> Result<(), Box<dyn Error>> {
    let opt = Opt::from_iter(args);

    let mut images = opt.input.to_owned();
    utils::ims_init(&mut images, opt.out_dir.as_path(), Some(opt.nproc));

    images.iter().par_bridge().for_each(|img| {
        process_image(&img, opt.out_dir.as_path(), &opt)
            .expect(&(img.to_string() + "  Error processing image"))
    });

    Ok(())
}

fn process_image(img: &str, out_dir: &std::path::Path, opt: &Opt) -> Result<(), Box<dyn Error>> {
    println!("File: {}", img);
    let img_image = image::open(&img)?;

    if !image_is_colorful(img_image, opt.threshold) {
        let save_path = Path::new(out_dir).join(Path::new(img).file_name().unwrap());
        std::fs::rename(img, save_path)?;
    }

    Ok(())
}

/// Returns 'true' if any pixel of image has chroma over theshold
fn pixels_chroma_threshold<T>(pixels: T, threshold: &u8) -> bool
where
    T: Iterator<Item = [u8; 3]>,
{
    for p in pixels {
        let r = p[0];
        let g = p[1];
        let b = p[2];
        let max = core::cmp::max(core::cmp::max(r, g), b);
        let min = core::cmp::min(core::cmp::min(r, g), b);
        let chroma: u8 = max - min;
        if &chroma > threshold {
            return true;
        }
    }
    false
}

/// Checks if any pixel of a resized image has chroma over the threshold
fn image_is_colorful(img: image::DynamicImage, threshold: u8) -> bool {
    let dim = img.dimensions();
    let dim = core::cmp::max(dim.0, dim.1);
    let thumb_size;
    if dim.le(&2048) {
        thumb_size = 16;
    } else {
        thumb_size = 32;
    }
    if img.color().has_color() {
        // resize image
        let thumb = image::imageops::resize(
            &img.into_rgb8(),
            thumb_size,
            thumb_size,
            image::imageops::Nearest,
        );
        // is chroma over theshold?
        return pixels_chroma_threshold(thumb.pixels().map(|p| p.0), &threshold);
    }
    false
}

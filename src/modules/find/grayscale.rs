use core::panic;
use std::{error::Error, ffi::OsString, path::Path};

use clap::AppSettings;
use image::GenericImageView;
use rayon::iter::{ParallelBridge, ParallelIterator};
use structopt::StructOpt;

use crate::modules::utils;

#[rustfmt::skip]
#[derive(StructOpt, Debug)]
#[structopt(name = "imagescripts-rs find", about = " ")]
#[structopt(setting = AppSettings::ColoredHelp, setting = AppSettings::AllowNegativeNumbers)]
struct Opt {
    #[structopt(required = false, default_value = "./*", display_order = 0)]
    input: Vec<String>,
    #[structopt(short, required = false, default_value = "./grayscale", display_order = 0)]
    out_dir: std::path::PathBuf,
    #[structopt(short, default_value = "0.1")]
    threshold: f32,
    #[structopt(long, default_value = "0")]
    nproc: usize,
}

pub fn main(args: Vec<OsString>) -> Result<(), Box<dyn Error>> {
    let opt = Opt::from_iter(args);

    let mut images = opt.input.to_owned();
    utils::ims_init(&mut images, opt.out_dir.as_path(), Some(opt.nproc))?;

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

/// Checks if any pixel of a resized image has chroma over the threshold
/// https://en.wikipedia.org/wiki/HSL_and_HSV#From_RGB
fn image_is_colorful(img: image::DynamicImage, threshold: f32) -> bool {
    let dim = img.dimensions();
    let dim = core::cmp::max(dim.0, dim.1);
    let thumb_size;
    if dim.le(&2048) {
        thumb_size = 32;
    } else {
        thumb_size = 64;
    }
    if img.color().has_color() {
        // resize image
        let thumb = image::imageops::resize(
            &img.into_rgb8(),
            thumb_size,
            thumb_size,
            image::imageops::Nearest,
        );
        return !image_is_monochrome_by_MSE(&thumb, threshold, true);
    }
    false
}

#[allow(clippy::float_cmp)] // '==' was not used on any calculated value
fn rgb2hsv(rgb: &image::Rgb<u8>) -> [f32; 3] {
    let rgb = rgb.0;
    let r = rgb[0] as f32 / 255.0;
    let g = rgb[1] as f32 / 255.0;
    let b = rgb[2] as f32 / 255.0;
    let value = f32::max(r, f32::max(g, b));
    let min = f32::min(r, f32::min(g, b));

    let (hue, saturation, chroma);
    if value == min {
        hue = 0_f32;
        saturation = 0_f32;
    } else {
        chroma = value - min;
        hue = match value {
            v if v == r => 60.0 * (0.0 + (g - b) / chroma),
            v if v == g => 60.0 * (2.0 + (b - r) / chroma),
            v if v == b => 60.0 * (4.0 + (r - g) / chroma),
            _ => panic!(),
        };
        saturation = chroma / value;
    }
    [hue, saturation, value]
}

fn image_mean_color(image: &image::ImageBuffer<image::Rgb<u8>, Vec<u8>>) -> image::Rgb<u8> {
    let mut mean: [i32; 3] = [0, 0, 0];
    for pixel in image.pixels() {
        for (i, v) in mean.iter_mut().enumerate() {
            *v = (*v + pixel.0[i] as i32) / 2;
        }
    }
    image::Rgb::from([mean[0] as u8, mean[1] as u8, mean[2] as u8])
}

#[allow(non_snake_case)]
/// Computes Mean Squared Error (x100) from mean hue bias by converting each pixel of the image to hsv
/// returns true if MSE is less than mse_cutoff
fn image_is_monochrome_by_MSE(
    image: &image::ImageBuffer<image::Rgb<u8>, Vec<u8>>,
    mse_cutoff: f32,
    adjust_color_bias: bool,
) -> bool {
    let mut sse = 0.0;
    let mut hue_bias = 0.0;
    if adjust_color_bias {
        hue_bias = rgb2hsv(&image_mean_color(&image))[0];
    }

    for pixel in image.pixels() {
        let pixel_hsv = rgb2hsv(&pixel);
        if (pixel_hsv[0] - hue_bias).abs() < f32::EPSILON || pixel_hsv[0] < f32::EPSILON {
            continue;
        }
        sse += (pixel_hsv[1] - hue_bias).powi(2);
    }

    let image_dimensions = image.dimensions();
    let mse = sse / (image_dimensions.0 * image_dimensions.1) as f32 * 100.0;

    mse <= mse_cutoff
}

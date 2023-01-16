use core::panic;
use std::{
    error::Error,
    path::{Path, PathBuf},
};

use clap::Args;
use image::{GenericImageView, Rgb};
use rayon::iter::{ParallelBridge, ParallelIterator};

use crate::utils;

#[rustfmt::skip]
#[derive(Args, Debug, Clone)]
pub struct Opt {
    /// input image paths
    #[arg(required = false, default_value = "./*", display_order = 0)]
    input: Vec<PathBuf>,
    /// output directory path
    #[arg(short, required = false, default_value = "./monochrome", display_order = 0)]
    out_dir: PathBuf,
    /// MSE cutoff
    #[arg(short, default_value = "0.8")]
    threshold: f32,
    #[arg(long, default_value = "0")]
    nproc: usize,
    #[arg(short)]
    grayscale: bool,
    /// Don't move images
    #[arg(short = 's')]
    test: bool,
}

pub fn main(opt: Opt) -> Result<(), Box<dyn Error>> {
    let mut images = opt.input.to_owned();
    utils::ims_init(&mut images, opt.out_dir.as_path(), Some(opt.nproc))?;

    images.iter().par_bridge().for_each(|img| {
        process_image(img, opt.out_dir.as_path(), &opt)
            .unwrap_or_else(|_| panic!("Error processing image: {}", &img.display()))
    });

    Ok(())
}

fn process_image(img: &Path, out_dir: &std::path::Path, opt: &Opt) -> Result<(), Box<dyn Error>> {
    // println!("File: {}", img.display());
    let img_image = image::open(img)?;

    if image_is_monochrome(&img_image, opt.grayscale) <= opt.threshold {
        if opt.test {
            return Ok(());
        }
        let save_path = out_dir.join(img.file_name().unwrap());
        std::fs::rename(img, save_path)?;
    }

    Ok(())
}

pub fn image_is_monochrome(img: &image::DynamicImage, grayscale: bool) -> f32 {
    if img.color().has_color() {
        // calculate thumbnail size
        let dim = img.dimensions();
        let dim = core::cmp::max(dim.0, dim.1);
        let thumb_size = if dim < 2048 { 128 } else { 256 };
        // resize image
        let thumb = image::imageops::resize(
            &img.to_rgb8(),
            thumb_size,
            thumb_size,
            image::imageops::Nearest,
        );
        return image_monochrome_MSE(&thumb, true, grayscale);
    }
    -1.0
}

#[allow(clippy::float_cmp)] // '==' was not used on any calculated value
/// https://en.wikipedia.org/wiki/HSL_and_HSV#From_RGB
fn rgb2hsv(rgb: &Rgb<u8>) -> [f32; 3] {
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

    if hue >= 0.0 {
        [hue, saturation, value]
    } else {
        [hue + 360.0, saturation, value]
    }
}

fn image_mean_color(image: &image::ImageBuffer<image::Rgb<u8>, Vec<u8>>) -> image::Rgb<u8> {
    let mut mean: [u64; 3] = [0, 0, 0];
    let mut count = 0;

    for pixel in image.pixels() {
        // skip too light/dark pixels
        let pixel_hsv = rgb2hsv(pixel);
        if pixel_hsv[1] < 0.1 || pixel_hsv[2] < 0.1 {
            continue;
        }

        for (i, v) in mean.iter_mut().enumerate() {
            *v += pixel.0[i] as u64;
        }
        count += 1
    }

    for v in mean.iter_mut() {
        if count != 0 {
            *v /= count;
        } else {
            *v = 0;
        }
    }

    image::Rgb::from([mean[0] as u8, mean[1] as u8, mean[2] as u8])
}

#[allow(non_snake_case)]
/// Computes Mean Squared Error (x100) from mean hue bias by converting each pixel of the image to hsv
/// returns true if MSE is less than mse_cutoff
fn image_monochrome_MSE(
    image: &image::ImageBuffer<image::Rgb<u8>, Vec<u8>>,
    adjust_color_bias: bool,
    grayscale: bool,
) -> f32 {
    let mut sse = 0.0;
    let mut sse_step: f32;
    let mut hue_bias = 0.0;
    if adjust_color_bias {
        hue_bias = rgb2hsv(&image_mean_color(image))[0];
    }

    for pixel in image.pixels() {
        let pixel_hsv = rgb2hsv(pixel);
        if pixel_hsv[1] < 0.05 || pixel_hsv[1] > 0.99 || pixel_hsv[2] < 0.02 {
            continue;
        }
        if grayscale {
            sse_step = pixel_hsv[1] * 25.0;
        } else {
            sse_step = (pixel_hsv[0] - hue_bias).abs();

            if sse_step > 180.0 {
                sse_step -= 360.0;
            }
        }
        sse += sse_step.powi(2);
    }

    let image_dimensions = image.dimensions();

    // MSE
    sse / (image_dimensions.0 * image_dimensions.1) as f32 * 100.0
}

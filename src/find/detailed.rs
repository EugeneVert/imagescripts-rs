use std::{error::Error, path::PathBuf};

use clap::Args;
use image::{
    DynamicImage,
    ImageBuffer,
    // ImageResult,
    Luma,
};
// use imageproc::definitions::Image;

#[derive(Args, Debug, Clone)]
#[command(about = "Program for finding images without clean lines")]
pub struct Opt {
    /// input file
    #[arg(short)]
    input: PathBuf,
    /// mv dir
    #[arg(short)]
    output: Option<PathBuf>,
    /// threshold to mv
    #[arg(short, default_value = "3.25")]
    threshold: f32,
}

pub fn main(opt: Opt) -> Result<(), Box<dyn Error>> {
    let img = image::open(&opt.input)?;
    if image_is_detailed(&img, opt.threshold) {
        if let (Some(o), Some(n)) = (opt.output, &opt.input.file_name()) {
            if !o.exists() {
                std::fs::create_dir(&o)?;
            }
            std::fs::rename(&opt.input, &o.join(n))?;
        }
    }
    Ok(())
}

pub fn image_is_detailed(img: &DynamicImage, threshold: f32) -> bool {
    let mut img = img.to_luma8();
    imageproc::contrast::equalize_histogram_mut(&mut img);
    // let wh = img.dimensions();
    // #[rustfmt::skip]
    // const KERNEL_VEC: &[f32] = &[
    // 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5,
    // 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.5,
    // 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.5,
    // 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.5,
    // 0.5, 0.0, 0.0, 0.0, -18.0, 0.0, 0.0, 0.0, 0.5,
    // 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.5,
    // 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.5,
    // 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.5,
    // 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5];

    // let kernel = imageproc::filter::Kernel::new(KERNEL_VEC, 9, 9);

    // println!(
    //     "Please enter canny filter arguments \
    //     ( low_threshold: f32 )+( high_threshold: f32 )"
    // );
    // let mut input = String::new();
    // loop {
    //     std::io::stdin()
    //         .read_line(&mut input)
    //         .expect("error: unable to read user input");
    //     if input.trim_end().is_empty() {
    //         break;
    //     }
    //     let canny_args = input.clone();
    //     let canny_args = canny_args
    //         .trim_end()
    //         .split_once('+')
    //         .expect("error: unable to read user input");
    //     input.clear();
    //     let canny_args = (
    //         canny_args.0.parse::<f32>().unwrap(),
    //         canny_args.1.parse::<f32>().unwrap(),
    //     );
    let out = imageproc::edges::canny(&img, 128.0, 256.0);
    let out1 = imageproc::edges::canny(&img, 32.0, 64.0);
    let sum_div = pixels_sum(&out1) as f32 / pixels_sum(&out) as f32;
    println!("sum_div: {}", sum_div);
    return sum_div > threshold;
}

fn pixels_sum(img: &ImageBuffer<Luma<u8>, Vec<u8>>) -> i64 {
    let mut sum = 0;
    for i in img.pixels() {
        sum += i.0[0] as i64;
    }
    sum
}

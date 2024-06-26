use std::path::{Path, PathBuf};

use clap::Args;
use rayon::iter::{ParallelBridge, ParallelIterator};

use crate::{utils, BResult};

#[derive(Args, Debug, Clone)]
pub struct Opt {
    /// input image paths
    #[arg(required = false, default_value = "./*", display_order = 0)]
    input: Vec<PathBuf>,
    /// sort images w/ bpp greater than the target
    #[arg(short, conflicts_with = "greater")]
    lesser: Option<f32>,
    /// sort images w/ bpp less than the target
    #[arg(short, conflicts_with = "lesser")]
    greater: Option<f32>,
    /// Custom metric: bpp + px_count / 2048^2
    #[arg(short = 'm')]
    custom_metric: bool,
    #[arg(long, default_value = "0")]
    nproc: usize,
}

pub fn main(opt: Opt) -> BResult<()> {
    let out_dir = std::path::PathBuf::from(unwrap_two(opt.lesser, opt.greater).to_string());
    let images = utils::ims_init(&opt.input, &out_dir, Some(opt.nproc))?;

    images
        .iter()
        .par_bridge()
        .for_each(|img| process_image(img, &out_dir, &opt).unwrap());

    Ok(())
}

fn unwrap_two<T>(l: Option<T>, b: Option<T>) -> T {
    match l {
        Some(val) => val,
        None => b.expect("Both options is None"),
    }
}

fn process_image(img: &Path, out_dir: &std::path::Path, opt: &Opt) -> BResult<()> {
    let img_filesize = img.metadata()?.len();
    let img_dimensions = image::image_dimensions(img)?;
    let px_count = img_dimensions.0 * img_dimensions.1;
    let img_bpp = (img_filesize * 8) as f32 / px_count as f32;
    let img_metric = if opt.custom_metric {
        img_bpp + px_count as f32 / 4194304_f32
    } else {
        img_bpp
    };

    println!("File: {}\n bpp: {:.3}", img.display(), img_metric);
    let mut save_flag: bool = false;
    match opt.lesser {
        Some(val) => {
            if img_metric < val {
                save_flag = true;
            }
        }
        None => {
            let val = opt.greater.unwrap();
            if img_metric > val {
                save_flag = true;
            }
        }
    }
    if save_flag {
        let save_path = out_dir.join(img.file_name().unwrap());
        std::fs::rename(img, save_path)?;
    }

    Ok(())
}

use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

use clap::Args;
use rayon::iter::{ParallelBridge, ParallelIterator};

use crate::{utils, BResult};

#[rustfmt::skip]
#[derive(Args, Debug, Clone)]
pub struct Opt {
    /// input image paths
    #[arg(required = false, default_value = "./*", display_order = 0)]
    input: Vec<PathBuf>,
    /// search target
    #[arg(short = 's', long = "size", required = false, default_value = "3508", display_order = 0)]
    px_size: u32,
    /// keep empty folder after sorting
    #[arg(long)]
    keep_empty: bool,
    #[arg(long, default_value = "0")]
    nproc: usize,
}

pub fn main(opt: Opt) -> BResult<()> {
    let out_dir = Path::new("./").join(opt.px_size.to_string());
    let images = utils::ims_init(&opt.input, &out_dir, Some(opt.nproc))?;
    let images_to_mv = get_images_to_mv(&images, &opt);

    if images_to_mv.is_empty() {
        return Ok(());
    }

    for image in images_to_mv {
        let filename = image
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or_else(|| format!("Can't get image filename: {}", &image.display()))?;
        std::fs::rename(&image, out_dir.join(filename))?;
    }

    Ok(())
}

fn get_images_to_mv(images: &[PathBuf], opt: &Opt) -> Vec<PathBuf> {
    images
        .iter()
        .par_bridge()
        .filter_map(|image| {
            if is_image_to_move(image, opt).unwrap_or_else(|_| {
                panic!(
                    "Error, cannot check image dimmensions\n{}",
                    &image.display()
                )
            }) {
                Some(image.to_owned())
            } else {
                None
            }
        })
        .collect()
}

fn is_image_to_move(image: &Path, opt: &Opt) -> image::ImageResult<bool> {
    let img_dimmensions = image::image_dimensions(image)?;
    if img_dimmensions.0 > opt.px_size || img_dimmensions.1 > opt.px_size {
        Ok(true)
    } else {
        Ok(false)
    }
}

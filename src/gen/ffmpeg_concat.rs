use std::{
    error::Error,
    path::{Path, PathBuf},
};

use clap::Args;

use crate::utils;

use super::ffmpeg_demuxer_create_from_files;

#[derive(Args, Debug, Clone)]
pub struct Opt {
    /// input image paths
    #[arg(required = false, default_value = "./*", display_order = 0)]
    input: Vec<PathBuf>,
}

pub fn main(opt: Opt) -> Result<(), Box<dyn Error>> {
    let mut images = opt.input;
    if images[0].to_string_lossy() == "./*" {
        utils::input_get_from_cwd(&mut images)?;
        utils::input_filter_images(&mut images);
    }
    let demuxerf_path = Path::new("./concat_demuxer");
    ffmpeg_demuxer_create_from_files(demuxerf_path, &images)?;
    Ok(())
}

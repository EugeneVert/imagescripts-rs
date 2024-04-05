use std::path::{Path, PathBuf};

use clap::Args;

use crate::{utils, BResult};

use super::ffmpeg_demuxer_create_from_files;

#[derive(Args, Debug, Clone)]
pub struct Opt {
    /// input image paths
    #[arg(required = false, default_value = "./*", display_order = 0)]
    input: Vec<PathBuf>,
}

pub fn main(opt: Opt) -> BResult<()> {
    let mut images = opt.input;
    if images[0].to_string_lossy() == "./*" {
        images = utils::read_cwd()?;
        utils::filter_images(&mut images);
        images.sort_unstable();
    }
    let demuxerf_path = Path::new("./concat_demuxer");
    ffmpeg_demuxer_create_from_files(demuxerf_path, &images)?;
    Ok(())
}

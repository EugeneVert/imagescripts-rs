use std::{
    error::Error,
    ffi::OsString,
    path::{Path, PathBuf},
};

use clap::AppSettings;
use structopt::StructOpt;

use crate::modules::utils;

#[derive(StructOpt, Debug)]
#[structopt(setting = AppSettings::ColoredHelp)]
struct Opt {
    /// input directory
    #[structopt(required = false, default_value = "./*", display_order = 0)]
    input: Vec<PathBuf>,
}

pub fn main(args: Vec<OsString>) -> Result<(), Box<dyn Error>> {
    let opt = Opt::from_iter(args);

    let mut images = opt.input;
    if images[0].to_string_lossy() == "./*" {
        utils::input_get_from_cwd(&mut images)?;
        utils::input_filter_images(&mut images);
    }
    let demuxerf_path = Path::new("./concat_demuxer");
    utils::ffmpeg_demuxer_create_from_files(demuxerf_path, &images)?;
    Ok(())
}

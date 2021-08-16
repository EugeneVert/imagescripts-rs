use std::{
    cmp::Eq,
    collections::HashMap,
    error::Error,
    ffi::OsString,
    hash::Hash,
    path::{Path, PathBuf},
};

use clap::AppSettings;
use structopt::StructOpt;

use crate::modules::utils;

#[derive(StructOpt, Debug)]
#[structopt(setting = AppSettings::ColoredHelp)]
struct Opt {
    /// input files
    #[structopt(required = false, default_value = "./*", display_order = 0)]
    input: Vec<PathBuf>,

    /// video dimensions
    #[structopt(short = "d")]
    dimensions: Option<String>,
    /// video background for resized images
    #[structopt(long = "bg", default_value = "Black")]
    background: String,

    /// ffmpeg arguments (or preset name {n} ["x264", "x265", "apng", "vp9", "aom-av1", "aom-av1-simple"] )
    #[structopt(short, long = "ffmpeg", default_value = "x264")]
    ffmpeg_args: String,
    #[structopt(long = "p:crf", default_value = "17")]
    preset_crf: f32,
    #[structopt(short, long = "container")]
    container: Option<String>,
    #[structopt(short = "r", default_value = "2")]
    fps: f32,
    #[structopt(long)]
    two_pass: Option<bool>,
    // /// don't create archive w/ non-resized images if there any
    // #[structopt(long)]
    // noarchive: bool,
}

pub fn main(args: Vec<OsString>) -> Result<(), Box<dyn Error>> {
    let opt = Opt::from_iter(args);

    let mut images = opt.input.to_owned();
    if images[0].to_string_lossy() == "./*" {
        utils::input_get_from_cwd(&mut images)?;
        utils::input_filter_images(&mut images);
        images.sort_unstable()
    }

    let dimm = get_video_dimm_from_images(&images).expect("Can't calculate frequent image dimms");

    let mut videoopts = utils::VideoOpts::new(&opt.ffmpeg_args, &opt.container, &opt.two_pass);
    videoopts.args_match();
    if videoopts.args_ispreset() {
        videoopts.ffmpeg_args += format!(" -crf {}", &opt.preset_crf).as_str();
    }

    let demuxerf_path = Path::new("./concat_demuxer");
    utils::ffmpeg_demuxer_create_from_files(demuxerf_path, &images)?;

    let ffmpeg_cmd = format!(
        "-r {fps} -safe 0 -f concat -i {demuxer_path} {0} \
        -vf scale={1}:{2}:force_original_aspect_ratio=decrease\
        ,pad={1}:{2}:(ow-iw)/2:(oh-ih)/2:'{background}' ",
        &videoopts.ffmpeg_args,
        &dimm.0,
        &dimm.1,
        demuxer_path = &demuxerf_path.display(),
        fps = &opt.fps,
        background = &opt.background,
    );

    let container = videoopts.container.expect("No video container");
    let two_pass = videoopts.two_pass.expect("No encoder passes count");
    let output_filestem = images[0]
        .file_stem()
        .and_then(|x| x.to_str())
        .ok_or_else(|| format!("No filestem: {}", images[0].display()))?;
    utils::ffmpeg_run(&ffmpeg_cmd, output_filestem, two_pass, &container);

    Ok(())
}

/// Get most frequent width & hight from images (DirEntry) array
fn get_video_dimm_from_images(images: &[PathBuf]) -> Option<(u32, u32)> {
    let mut images_w = Vec::<u32>::new();
    let mut images_h = Vec::<u32>::new();
    images
        .iter()
        .map(|i| image::image_dimensions(i).unwrap())
        .for_each(|d| {
            images_w.push(d.0);
            images_h.push(d.1);
        });
    let freq_w = most_frequent(&images_w);
    let freq_h = most_frequent(&images_h);
    println!("{:?}", (freq_w, freq_h));
    match (freq_w, freq_h) {
        (Some(w), Some(h)) => Some((w, h)),
        _ => None,
    }
}

fn most_frequent<T>(iter: &[T]) -> Option<T>
where
    T: Eq + Hash + Copy,
{
    let mut m: HashMap<T, usize> = HashMap::new();
    for x in iter {
        *m.entry(*x).or_default() += 1;
    }
    m.into_iter().max_by_key(|(_, v)| *v).map(|(k, _)| k)
}

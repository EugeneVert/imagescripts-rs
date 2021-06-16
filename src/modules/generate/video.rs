use std::cmp::Eq;
use std::collections::HashMap;
use std::hash::Hash;
use std::{error::Error, ffi::OsString, fs::DirEntry};

use clap::AppSettings;
use structopt::StructOpt;

use crate::modules::utils;

#[derive(StructOpt, Debug)]
#[structopt(setting = AppSettings::ColoredHelp)]
struct Opt {
    /// input directory
    #[structopt(required = false, default_value = "./.", display_order = 0)]
    input: String,
    /// input images extension
    #[structopt(short = "e")]
    extension: String,

    /// video dimensions
    #[structopt(short = "d")]
    dimensions: Option<String>,
    /// video background for resized images
    #[structopt(long = "bg", default_value = "Black")]
    background: String,

    /// ffmpeg arguments (or preset name)
    #[structopt(short, long = "ffmpeg", default_value = "aom-av1")]
    ffmpeg_args: String,
    #[structopt(long = "p:crf", default_value = "18")]
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
    println!("{:?}", &opt);

    std::env::set_current_dir(&opt.input)?;
    println!("Chdir to: {:?}", std::env::current_dir().unwrap());

    let images: Result<Vec<DirEntry>, _> = std::fs::read_dir("./.")?.collect();
    let images: Vec<DirEntry> = images?
        .into_iter()
        .filter(|x| -> bool {
            let x_path = x.path();
            let x_ext = x_path.extension();
            x_path.is_file() && x_ext.is_some() && x_ext.unwrap().to_str().unwrap() == opt.extension
        })
        .collect();
    let dimm = get_video_dimm_from_images(&images).unwrap();

    let mut videoopts = utils::VideoOpts::new(&opt.ffmpeg_args, opt.container, opt.two_pass);
    videoopts.args_match();
    if videoopts.args_ispreset() {
        videoopts.ffmpeg_args += format!(" -crf {}", &opt.preset_crf).as_str();
    }

    let ffmpeg_cmd = format!(
        "-r {fps} -pattern_type glob -i ./*.{ext} {0} \
        -vf scale={1}:{2}:force_original_aspect_ratio=decrease\
        ,pad={1}:{2}:(ow-iw)/2:(oh-ih)/2:'{background}' ",
        &videoopts.ffmpeg_args,
        &dimm.0,
        &dimm.1,
        ext = &opt.extension,
        fps = &opt.fps,
        background = &opt.background,
    );

    let container = videoopts.container.expect("No video container specified");
    let two_pass = videoopts.two_pass.unwrap();
    utils::ffmpeg_run(&ffmpeg_cmd, "out", two_pass, &container);

    Ok(())
}

/// Get most frequent width & hight from images (DirEntry) array
fn get_video_dimm_from_images(images: &[DirEntry]) -> Option<(u32, u32)> {
    let mut images_w = Vec::<u32>::new();
    let mut images_h = Vec::<u32>::new();
    images
        .iter()
        .map(|i| image::image_dimensions(&i.path()).unwrap())
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

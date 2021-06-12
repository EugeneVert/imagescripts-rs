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
    #[structopt(short = "e", default_value = "png")]
    extension: String,

    /// video dimmensions
    #[structopt(short = "d")]
    dimmensions: Option<String>,
    /// video background for resized images
    #[structopt(long = "bg", default_value = "Black")]
    background: String,

    /// ffmpeg arguments (or preset name)
    #[structopt(short, long = "ffmpeg", default_value = "x264")]
    ffmpeg_args: String,
    #[structopt(long = "p:crf", default_value = "18")]
    preset_crf: u8,
    #[structopt(long = "p:r", default_value = "2")]
    preset_fps: u8,
    /// don't create archive w/ non-resized images if there any
    #[structopt(long)]
    noarchive: bool,
}

pub fn main(args: Vec<OsString>) -> Result<(), Box<dyn Error>> {
    let opt = Opt::from_iter(args);
    println!("{:?}", &opt);

    std::env::set_current_dir(&opt.input)?;
    println!("Chdir to: {:?}", std::env::current_dir());

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

    let mut container = "";
    let mut ffmpegargs = utils::match_ffmpegargs(&opt.ffmpeg_args, &mut container);
    ffmpegargs += format!(" -crf {} -r {}", &opt.preset_crf, &opt.preset_fps).as_str();

    // let demuxerf = std::fs::File::create("./concat_demuxer")?;
    // let mut demuxerf = std::io::BufWriter::new(demuxerf);
    // images.sort_unstable_by_key(|x| x.path().as_os_str().to_os_string());
    // demuxerf.write_all(b"ffconcat version 1.0\n")?;
    // for i in images {
    //     demuxerf.write_all(
    //         format!(
    //             "file \'{}\'\nduration 1\n",
    //             &i.path().file_name().unwrap().to_str().unwrap(),
    //         )
    //         .as_bytes(),
    //     )?
    // }
    // demuxerf.flush()?;
    // let ffmpeg_cmd = format!(
    //     "-f concat -i ./concat_demuxer {0} \
    //     -vf scale={1}:{2}:force_original_aspect_ratio=decrease\
    //     ,pad={1}:{2}:(ow-iw)/2:(oh-ih)/2:'{3}' out.{4}",
    //     ffmpegargs, dimm.0, dimm.1, opt.background, container
    // );
    // std::fs::remove_file("./concat_demuxer")?;

    let ffmpeg_cmd = format!(
        "-r 1 -pattern_type glob -i ./*.{ext} {0} \
        -vf scale={1}:{2}:force_original_aspect_ratio=decrease\
        ,pad={1}:{2}:(ow-iw)/2:(oh-ih)/2:'{3}' out.{4}",
        ffmpegargs,
        dimm.0,
        dimm.1,
        opt.background,
        container,
        ext = opt.extension
    );

    println!("{:?}", &ffmpeg_cmd);
    let p = std::process::Command::new("ffmpeg")
        .args(ffmpeg_cmd.split(' '))
        .output()
        .unwrap();
    println!("{}", std::str::from_utf8(&p.stderr).unwrap());

    Ok(())
}

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

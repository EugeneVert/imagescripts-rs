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
#[structopt(setting = AppSettings::ColoredHelp, setting = AppSettings::AllowLeadingHyphen)]
struct Opt {
    /// input image paths
    #[structopt(required = false, default_value = "./*", display_order = 0)]
    input: Vec<PathBuf>,

    /// video dimensions (e.g: '128x128')
    #[structopt(short = "d")]
    dimensions: Option<String>,
    /// video background for resized images
    #[structopt(long = "bg", default_value = "Black")]
    background: String,

    /// ffmpeg arguments (or preset name {n} ["x264", "x265", "apng", "vp9", "aom-av1", "aom-av1-simple"] ) {n}
    #[structopt(short, long = "ffmpeg", default_value = "x264")]
    ffmpeg_args: String,
    /// crf / qscale for preset
    #[structopt(short = "q", default_value = "17")]
    preset_quality: f32,
    /// video container
    #[structopt(short, long = "container")]
    container: Option<String>,
    /// video fps
    #[structopt(short = "r", default_value = "4")]
    fps: f32,
    /// two-pass video encoding
    #[structopt(long)]
    two_pass: Option<bool>,
    /// generate video thumbnail
    #[structopt(short = "t", long = "thumb")]
    create_thumbnail: bool,
    /// amount of 2x2 thumbnail sheets
    #[structopt(short = "s", long = "thumb_sheets", default_value = "2")]
    thumbnail_sheets: usize,
}

pub fn main(args: Vec<OsString>) -> Result<(), Box<dyn Error>> {
    let opt = Opt::from_iter(args);

    let mut images = opt.input.to_owned();
    if images[0].to_string_lossy() == "./*" {
        utils::input_get_from_cwd(&mut images)?;
        utils::input_filter_images(&mut images);
        images.sort_unstable()
    }

    let dimm = opt.dimensions.and_then(|s| {
        s.split_once('x')
            .map(|s| (s.0.parse().unwrap(), s.1.parse().unwrap()))
    });
    let dimm = match dimm {
        Some(x) => x,
        None => get_video_dimm_from_images(&images).expect("Can't calculate frequent image dimms"),
    };

    let mut videoopts = utils::VideoOpts::new(&opt.ffmpeg_args, &opt.container, &opt.two_pass);
    videoopts.args_match();
    videoopts.args_preset_add_quality(opt.preset_quality);

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

    if opt.create_thumbnail {
        let video_filename = format!("{}.{}", &output_filestem, &container);
        generate_thumbnail(&video_filename, images.len(), opt.thumbnail_sheets)?;
    }

    Ok(())
}

fn generate_thumbnail(
    video_filename: &str,
    n_frames: usize,
    thumbnail_sheets: usize,
) -> Result<(), std::io::Error> {
    let tmpdir = tempfile::tempdir()?;

    // Select the most representative frames for each `m = (n / (opt.thumbnail_sheets * 4))` frames from the video
    // using ffmpeg, and save them to the temporary directory
    std::process::Command::new("ffmpeg")
        .arg("-i")
        .arg(&video_filename)
        .arg("-vf")
        .arg(format!(
            "                                                             \
            scale=254:254,                                                 \
            drawtext=fontfile=/usr/share/fonts/noto/NotoSans-Regular.ttf: \
            fontsize=14:start_number=1:text='%{n}':x=(w-tw)/2:y=h-(2*lh): \
            fontcolor=white:                                              \
            bordercolor=black:borderw=1,thumbnail=n={}",
            {
                let m = (n_frames as f32 / (thumbnail_sheets as f32 * 4.0)).ceil() as usize;
                match m {
                    m if m < 2 => 2,
                    _ => m,
                }
            },
            n = "{n}"
        ))
        .arg("-vsync")
        .arg("0")
        .arg(format!("{}/capture%002d.png", &tmpdir.path().display()))
        .status()?;

    const MONTAGE_ARGS_2X2: [&str; 6] = [
        "-geometry",
        "+1+1", // add 1px border
        "-tile",
        "2x2", // 2x2 images grid
        "-background",
        "black",
    ];
    std::process::Command::new("montage")
        .args(MONTAGE_ARGS_2X2)
        .arg(&format!(
            "{}/capture%02d.png[1-{}]",
            &tmpdir.path().display(),
            &tmpdir.path().read_dir().unwrap().count()
        ))
        .arg(format!("{}.jxl", &video_filename))
        .status()?;
    tmpdir.close()?;

    Ok(())
}

/// Get most frequent width & hight from images
fn get_video_dimm_from_images(images: &[PathBuf]) -> Option<(u32, u32)> {
    let mut images_w = Vec::<u32>::new();
    let mut images_h = Vec::<u32>::new();
    images
        .iter()
        .map(|i| {
            image::image_dimensions(i)
                .unwrap_or_else(|e| panic!("Can't read image dimensions: {}; {}", &i.display(), &e))
        })
        .for_each(|d| {
            images_w.push(d.0);
            images_h.push(d.1);
        });
    let w = most_frequent(&images_w)?;
    let h = most_frequent(&images_h)?;
    println!("{:?}x{:?}", &w, &h);

    // find and print image paths whose sizes differs from the most frequent ones
    let be_resized = images_w
        .iter()
        .zip(&images_h)
        // .for_each(|x| println!("{:?}", &x));
        .enumerate()
        .filter(|x| x.1 != (&w, &h))
        .map(|x| &images[x.0])
        .map(|f| {
            println!("Image {} will be resized", f.display());
            f
        })
        .count() != 0;
    if be_resized {
        println!("Please press 'Enter'");
        std::io::stdin().read_line(&mut String::new()).unwrap();
    }

    Some((w, h))
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

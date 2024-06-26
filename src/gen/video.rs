use std::{
    cmp::Eq,
    collections::HashMap,
    hash::Hash,
    io::BufRead,
    path::{Path, PathBuf},
};

use clap::Args;

use crate::{utils, BResult};

use super::{ffmpeg_demuxer_create_from_files, ffmpeg_run, VideoOpts};

#[derive(Args, Debug, Clone)]
pub struct Opt {
    /// input image paths
    #[arg(required = false, default_value = "./*", display_order = 0)]
    input: Vec<PathBuf>,

    /// video dimensions (e.g: '128x128')
    #[arg(short = 'd')]
    dimensions: Option<String>,
    /// video background for resized images
    #[arg(long = "bg", default_value = "Black")]
    background: String,

    /// ffmpeg arguments (or preset name {n} ["x264", "x265", "apng", "vp9", "aom-av1", "aom-av1-simple"] ) {n}
    #[arg(
        short,
        long = "ffmpeg",
        default_value = "x264",
        allow_hyphen_values = true
    )]
    ffmpeg_args: String,
    /// crf / qscale for preset
    #[arg(short = 'q', default_value = "17")]
    preset_quality: f32,
    /// video container
    #[arg(short, long = "container")]
    container: Option<String>,
    /// video fps
    #[arg(short = 'r', default_value = "4")]
    fps: f32,
    /// two-pass video encoding
    #[arg(long)]
    two_pass: Option<bool>,
    /// generate video thumbnail
    #[arg(short = 't', long = "thumb")]
    create_thumbnail: bool,
    /// amount of thumbnail sheets
    #[arg(short = 's', long = "sheet_count", default_value = "2")]
    thumbnail_sheets: usize,
    /// thumbnail sheet size
    #[arg(long = "sheet_size", default_value = "2x2")]
    thumbnail_size: String,
    /// Don't ask for resize confirmation
    #[arg(short = 'n', long = "no_confirm")]
    no_confirm: bool,
    #[arg(long = "round")]
    round: bool,
}

pub fn main(opt: Opt) -> BResult<()> {
    let mut images = opt.input.to_owned();
    if images[0].to_string_lossy() == "./*" {
        images = utils::read_cwd()?;
        utils::filter_images(&mut images);
        images.sort_unstable()
    }

    let dimm = opt.dimensions.and_then(|s| {
        s.split_once('x')
            .map(|s| (s.0.parse().unwrap(), s.1.parse().unwrap()))
    });
    let dimm = match dimm {
        Some(x) => x,
        None => get_video_dimm_from_images(&images, opt.no_confirm, opt.round)
            .expect("Can't calculate frequent image dimms"),
    };

    let mut videoopts =
        VideoOpts::new(&dirs::config_dir().unwrap().join("vert/video_presets.json"))?;
    videoopts.args_match(
        &opt.ffmpeg_args,
        &opt.container,
        &opt.two_pass,
        opt.preset_quality,
    );

    let demuxerf_path = Path::new("./concat_demuxer");
    ffmpeg_demuxer_create_from_files(demuxerf_path, &images)?;

    // TODO png RGB->YUV BT709 colorspace matrix conversion
    // -vf "zscale=m=709:min=709:r=limited,format=yuv420p10le" -colorspace bt709 -color_primaries bt709 -color_trc bt709 -color_range tv
    let ffmpeg_cmd = format!(
        "-r {fps} -safe 0 -f concat -i {demuxer_path} {0} \
        -sws_flags spline+accurate_rnd+full_chroma_int \
        -vf scale={1}:{2}:force_original_aspect_ratio=decrease\
        ,pad={1}:{2}:(ow-iw)/2:(oh-ih)/2:'{background}'",
        &videoopts.args,
        &dimm.0,
        &dimm.1,
        demuxer_path = &demuxerf_path.display(),
        fps = &opt.fps,
        background = &opt.background,
    );

    let output_filestem = images[0]
        .file_stem()
        .and_then(|x| x.to_str())
        .ok_or_else(|| format!("No filestem: {}", images[0].display()))?;
    ffmpeg_run(
        &ffmpeg_cmd,
        output_filestem,
        videoopts.two_pass,
        &videoopts.container,
    );

    if opt.create_thumbnail {
        let video_filename = format!("{}.{}", &output_filestem, &videoopts.container);
        generate_thumbnail(
            &video_filename,
            images.len(),
            opt.thumbnail_sheets,
            get_thumbnail_size(&opt.thumbnail_size)?,
        )?;
    }

    Ok(())
}

fn get_thumbnail_size(a: &str) -> BResult<(usize, usize)> {
    if let Some(sizes) = a.split_once('x') {
        return Ok((sizes.0.parse()?, sizes.1.parse()?));
    }
    Err("Can't parse thumbnail_size".into())
}

fn generate_thumbnail(
    video_filename: &str,
    n_frames: usize,
    thumbnail_sheets: usize,
    thumbnail_size: (usize, usize),
) -> Result<(), std::io::Error> {
    let tmpdir = tempfile::tempdir()?;

    // Select the most representative frames for each `m = (n / (opt.thumbnail_sheets * 4))` frames from the video
    // using ffmpeg, and save them to the temporary directory
    std::process::Command::new("ffmpeg")
        .arg("-i")
        .arg(video_filename)
        .arg("-vf")
        .arg(format!(
            "                                                             \
            scale=254:254,                                                \
            drawtext=fontfile=/usr/share/fonts/noto/NotoSans-Regular.ttf: \
            fontsize=14:start_number=1:text='%{n}':x=(w-tw)/2:y=h-(2*lh): \
            fontcolor=white:                                              \
            bordercolor=black:borderw=1,thumbnail=n={}",
            {
                // thumbnail=n={}
                let images_in_thumbnail = thumbnail_size.0 * thumbnail_size.1;
                let m = (n_frames as f32 / ((thumbnail_sheets * images_in_thumbnail) as f32)).ceil()
                    as usize;
                match m {
                    m if m < 2 => 2, // Don't make thumbnail from every frame
                    _ => m,
                }
            },
            n = "{n}"
        ))
        .arg("-vsync")
        .arg("0")
        .arg(format!("{}/capture%002d.png", &tmpdir.path().display()))
        .status()?;

    let montage_args: [&str; 6] = [
        "-geometry",
        "+1+1", // add 1px border
        "-tile",
        &format!("{}x{}", thumbnail_size.0, thumbnail_size.1), // thumbnail grid
        "-background",
        "black",
    ];
    std::process::Command::new("montage")
        .args(montage_args)
        .arg(&format!(
            "{}/capture%02d.png[1-{}]",
            &tmpdir.path().display(),
            &tmpdir.path().read_dir().unwrap().count()
        ))
        .arg(format!("{}.png", &video_filename))
        .status()?;
    tmpdir.close()?;

    Ok(())
}

/// Get most frequent width & hight from images
fn get_video_dimm_from_images(
    images: &[PathBuf],
    no_confirm: bool,
    round: bool,
) -> Option<(u32, u32)> {
    let mut images_w = Vec::<u32>::new();
    let mut images_h = Vec::<u32>::new();
    images
        .iter()
        .map(|i| {
            image::image_dimensions(i)
                .or_else(|_| image_dimmesions_exiftool(i))
                .unwrap_or_else(|e| panic!("Can't read image dimensions: {}; {}", &i.display(), &e))
        })
        .for_each(|d| {
            images_w.push(d.0);
            images_h.push(d.1);
        });
    let mut w = most_frequent(&images_w)?;
    let mut h = most_frequent(&images_h)?;

    if round {
        if w % 2 == 1 {
            w = (w / 2) * 2 + 2;
        }
        if h % 2 == 1 {
            h = (h / 2) * 2 + 2;
        }
    }

    println!("{:?}x{:?}", &w, &h);

    // find and print image paths whose sizes differs from the most frequent ones
    let be_resized = images_w
        .iter()
        .zip(&images_h)
        // .for_each(|x| println!("{:?}", &x));
        .enumerate()
        .filter(|x| x.1 != (&w, &h))
        .map(|x| &images[x.0])
        .inspect(|f| {
            println!("Image {} will be resized", f.display());
        })
        .count()
        != 0;
    if be_resized && !no_confirm {
        println!("Please press 'Enter'");
        std::io::stdin().read_line(&mut String::new()).unwrap();
    }

    Some((w, h))
}

fn image_dimmesions_exiftool(image: &Path) -> BResult<(u32, u32)> {
    let output = std::process::Command::new("exiftool")
        .args(["-s3", "-ImageWidth", "-ImageHeight"])
        .arg(image)
        .output()?;
    let mut lines = output.stdout.lines().map(|l| l.unwrap());
    let width = lines.next().unwrap_or_default().parse()?;
    let height = lines.next().unwrap_or_default().parse()?;
    Ok((width, height))
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

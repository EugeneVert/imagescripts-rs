// TODO parse {}.zip.js file near {}.zip

use std::{
    collections::HashMap,
    error::Error,
    ffi::{OsStr, OsString},
    path::Path,
};

use clap::AppSettings;
use structopt::StructOpt;

use crate::modules::utils;

#[derive(StructOpt, Debug)]
#[structopt(setting = AppSettings::ColoredHelp)]
struct Opt {
    /// input zip
    #[structopt(display_order = 0)]
    input: String,

    /// ffmpeg arguments (or preset name {n} ["x264", "x265", "apng", "vp9", "aom-av1", "aom-av1-simple"] )
    #[structopt(short, long = "ffmpeg", default_value = "x264")]
    ffmpeg_args: String,
    #[structopt(long = "p:crf", default_value = "17")]
    preset_crf: f32,
    #[structopt(short, long = "container")]
    container: Option<String>,
    #[structopt(long)]
    two_pass: Option<bool>,
}

pub fn main(args: Vec<OsString>) -> Result<(), Box<dyn Error>> {
    let opt = Opt::from_iter(args);
    let filestem = Path::new(&opt.input)
        .file_stem()
        .and_then(OsStr::to_str)
        .ok_or_else(|| String::from("No filestem") + &opt.input)?;
    println!("{:?}", &opt);

    // extract zip to tempdir
    let zip_file = std::fs::File::open(&opt.input)?;
    // let zip_file_stem = std::path::Path::new(&opt.input).file_stem().unwrap();
    let mut zip_archive = zip::ZipArchive::new(zip_file)?;
    let tempdir = tempfile::tempdir()?;
    zip_archive.extract(&tempdir)?;
    // open animation.json from tempdir
    let animdata_path = std::fs::read_dir(&tempdir)?
        .find(|x| {
            let x = x.as_ref().unwrap();
            ["js", "json"].contains(
                &x.path()
                    .extension()
                    .and_then(OsStr::to_str)
                    .unwrap_or_default(),
            )
        })
        .expect("No 'js' or 'json' file in zip")?
        .path();
    let animdata_file = std::fs::File::open(&animdata_path)?;

    let animdata_type: u8 = match animdata_path.extension().and_then(OsStr::to_str).unwrap() {
        "json" => 1,
        "js" => 2,
        _ => return Err("Can't find animation data".into()), // TODO better animdata_type matching
    };

    let json: HashMap<String, serde_json::Value> = serde_json::from_reader(animdata_file)?;
    let json_frames = match animdata_type {
        1 => json.values().next().unwrap()["frames"].as_array(),
        2 => json["frames"].as_array(),
        _ => panic!(),
    }
    .expect("Cant't find frames data in js/json file");
    let json_mux: Vec<(String, f64)> = json_frames
        .iter()
        .map(|x| {
            (
                x["file"].as_str().unwrap().to_string(),
                x["delay"].as_i64().unwrap() as f64 / 1000.0,
            )
        })
        .collect();

    let mut videoopts = utils::VideoOpts::new(&opt.ffmpeg_args, opt.container, opt.two_pass);
    videoopts.args_match();
    if videoopts.args_ispreset() {
        videoopts.ffmpeg_args =
            videoopts.ffmpeg_args + " -crf " + opt.preset_crf.to_string().as_str();
    }

    let demuxerf_path = tempdir.path().join("concat_demuxer");
    utils::ffmpeg_demuxer_create_from_json(&demuxerf_path, &json_mux)?;
    let ffmpeg_cmd = format!(
        "-f concat -i {} {} \
        -vf pad=ceil(iw/2)*2:ceil(ih/2)*2' ",
        &demuxerf_path.display(),
        &videoopts.ffmpeg_args,
    );

    let container = videoopts.container.expect("No video container");
    let two_pass = videoopts.two_pass.expect("No encoder passes count");
    utils::ffmpeg_run(&ffmpeg_cmd, &filestem, two_pass, &container);

    Ok(())
}

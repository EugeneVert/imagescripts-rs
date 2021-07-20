use std::{collections::HashMap, error::Error, ffi::OsString, path::Path};

use clap::AppSettings;
use structopt::StructOpt;

use crate::modules::utils;

#[derive(StructOpt, Debug)]
#[structopt(setting = AppSettings::ColoredHelp)]
struct Opt {
    /// input zip
    #[structopt(display_order = 0)]
    input: String,

    /// ffmpeg arguments (or preset name)
    #[structopt(short, long = "ffmpeg", default_value = "aom-av1")]
    ffmpeg_args: String,
    #[structopt(long = "p:crf", default_value = "18")]
    preset_crf: f32,
    #[structopt(short, long = "container")]
    container: Option<String>,
    #[structopt(long)]
    two_pass: Option<bool>,
}

pub fn main(args: Vec<OsString>) -> Result<(), Box<dyn Error>> {
    let opt = Opt::from_iter(args);
    let filestem = Path::new(&opt.input).file_stem().unwrap().to_str().unwrap();
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
            ["js", "json"].contains(
                &x.as_ref()
                    .unwrap()
                    .path()
                    .extension()
                    .unwrap_or_default()
                    .to_str()
                    .unwrap(),
            )
        })
        .expect("No 'js' or 'json' file in zip")
        .unwrap()
        .path();
    let animdata_file = std::fs::File::open(animdata_path)?;

    // let animdata_type_1 = "animations.json".to_string();
    // let animdata_type_2 = format!(
    //     "{}.js",
    //     std::path::Path::new(&opt.input)
    //         .file_stem()
    //         .unwrap()
    //         .to_string_lossy()
    // );
    // let animdata_type: u8 = match animdata_path.file_name().unwrap().to_str().unwrap() {
    //     animdata_type_1 => 1,
    //     animdata_type_2 => 2,
    //     _ => return Err("Can't find animation data".into()),
    // };

    let json: HashMap<String, serde_json::Value> = serde_json::from_reader(animdata_file)?;
    let json_frames = &json.into_iter().next().unwrap().1["frames"]
        .as_array()
        .unwrap()
        .clone();
    let json_mux = json_frames.iter().map(|x| {
        (
            x["file"].as_str().unwrap().to_string(),
            x["delay"].as_i64().unwrap() as f64 / 1000.0,
        )
    });

    let mut videoopts = utils::VideoOpts::new(&opt.ffmpeg_args, opt.container, opt.two_pass);
    videoopts.args_match();
    if videoopts.args_ispreset() {
        videoopts.ffmpeg_args += format!(" -crf {}", &opt.preset_crf).as_str();
    }

    let demuxerf_path = tempdir.path().join("concat_demuxer");
    utils::ffmpeg_demuxer_create_from_json(&demuxerf_path, json_mux)?;
    let ffmpeg_cmd = format!(
        "-f concat -i {} {} \
        -vf pad=ceil(iw/2)*2:ceil(ih/2)*2' ",
        &demuxerf_path.to_str().unwrap(),
        &videoopts.ffmpeg_args,
    );

    let container = videoopts.container.expect("No video container specified");
    let two_pass = videoopts.two_pass.unwrap();
    utils::ffmpeg_run(&ffmpeg_cmd, &filestem, two_pass, &container);

    Ok(())
}

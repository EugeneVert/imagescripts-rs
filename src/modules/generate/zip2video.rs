use std::{
    collections::HashMap,
    error::Error,
    ffi::{OsStr, OsString},
    path::PathBuf,
};

use clap::AppSettings;
use structopt::StructOpt;

use crate::modules::utils;

#[derive(StructOpt, Debug)]
#[structopt(setting = AppSettings::ColoredHelp, setting = AppSettings::AllowLeadingHyphen)]
struct Opt {
    /// input zip archive
    #[structopt(display_order = 0)]
    input: PathBuf,

    /// ffmpeg arguments (or preset name {n} ["x264", "x265", "apng", "vp9", "aom-av1", "aom-av1-simple"] ) {n}
    #[structopt(short, long = "ffmpeg", default_value = "x264")]
    ffmpeg_args: String,
    /// crf / qscale for preset
    #[structopt(short = "q", default_value = "17")]
    preset_quality: f32,
    /// video container
    #[structopt(short, long = "container")]
    container: Option<String>,
    ///
    #[structopt(long)]
    two_pass: Option<bool>,
}

pub fn main(args: Vec<OsString>) -> Result<(), Box<dyn Error>> {
    let opt = Opt::from_iter(args);

    // extract zip to tempdir
    let zip_file = std::fs::File::open(&opt.input)?;
    let mut zip_archive = zip::ZipArchive::new(zip_file)?;
    let tempdir = tempfile::tempdir()?;
    zip_archive.extract(&tempdir)?;

    // try gather demuxer from json file in zip/folder
    let json_mux = match animdata2demux(&opt, &tempdir) {
        Ok(x) => x,
        Err(e) => {
            tempdir.close()?;
            return Err(e);
        }
    };
    // create options for encding video
    let mut videoopts = utils::VideoOpts::new(&opt.ffmpeg_args, &opt.container, &opt.two_pass);
    videoopts.args_match();
    videoopts.args_preset_add_quality(opt.preset_quality);

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
    let filestem = opt
        .input
        .file_stem()
        .and_then(OsStr::to_str)
        .ok_or_else(|| format!("No filestem; {}", &opt.input.display()))?;
    utils::ffmpeg_run(&ffmpeg_cmd, filestem, two_pass, &container);

    Ok(())
}

/// Creates ffmpeg demuxer from json file in extracted zip or json from file near zip
fn animdata2demux(
    opt: &Opt,
    tempdir: &tempfile::TempDir,
) -> Result<Vec<(String, f64)>, Box<dyn Error>> {
    let mut animdata_path = Some(std::path::PathBuf::new());
    animdata_search_in_zip(tempdir, &mut animdata_path)?;
    if animdata_path.is_none() {
        animdata_search_in_folder(opt, &mut animdata_path);
    }
    let animdata_path =
        animdata_path.ok_or("No 'js' or 'json' file in zip or ''.zip + js/json file in folder")?;
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
        _ => return Err("Wrong animdata_type".into()),
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
    Ok(json_mux)
}

/// search animdata in zip archive extracted to tempdir
fn animdata_search_in_zip(
    tempdir: &tempfile::TempDir,
    animdata_path: &mut Option<std::path::PathBuf>,
) -> Result<(), std::io::Error> {
    let path = std::fs::read_dir(tempdir)?
        .find(|x| {
            let x = x.as_ref().unwrap();
            ["js", "json"].contains(
                &x.path()
                    .extension()
                    .and_then(OsStr::to_str)
                    .unwrap_or_default(),
            )
        })
        .map(|x| x.unwrap().path());
    *animdata_path = path;
    Ok(())
}

/// search animdata file in folder, next to input zip archive, which is named {}.zip.js or {}.zip.json
fn animdata_search_in_folder(opt: &Opt, animdata_path: &mut Option<std::path::PathBuf>) {
    let animdata_path_pathbuf_json = PathBuf::from(format!("{}.json", opt.input.display()));
    if animdata_path_pathbuf_json.exists() {
        *animdata_path = Some(animdata_path_pathbuf_json);
        return;
    }
    let animdata_path_pathbuf_js = PathBuf::from(format!("{}.js", opt.input.display()));
    if animdata_path_pathbuf_js.exists() {
        *animdata_path = Some(animdata_path_pathbuf_js);
    }
}

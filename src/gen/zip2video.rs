use std::{collections::HashMap, ffi::OsStr, path::PathBuf};

use clap::Args;

use crate::BResult;

use super::{ffmpeg_demuxer_create_from_json, ffmpeg_run, VideoOpts};

#[derive(Args, Debug, Clone)]
pub struct Opt {
    /// input zip archive
    #[arg(display_order = 0)]
    input: PathBuf,

    /// ffmpeg arguments (or preset name {n} ["x264", "x265", "apng", "vp9", "aom-av1", "aom-av1-simple"] ) {n}
    #[arg(short, long = "ffmpeg", default_value = "x264")]
    ffmpeg_args: String,
    /// crf / qscale for preset
    #[arg(short = 'q', default_value = "17")]
    preset_quality: f32,
    /// video container
    #[arg(short, long = "container")]
    container: Option<String>,
    /// force overwrite existing file
    #[arg(short = 'y')]
    overwrite: bool,
    ///
    #[arg(long)]
    two_pass: Option<bool>,
}

pub fn main(opt: Opt) -> BResult<()> {
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
    let mut videoopts =
        VideoOpts::new(&dirs::config_dir().unwrap().join("vert/video_presets.json"))?;
    videoopts.args_match(
        &opt.ffmpeg_args,
        &opt.container,
        &opt.two_pass,
        opt.preset_quality,
    );

    let demuxerf_path = tempdir.path().join("concat_demuxer");
    ffmpeg_demuxer_create_from_json(&demuxerf_path, &json_mux)?;
    let mut ffmpeg_cmd = format!(
        "-f concat -i {} {} \
        -vf pad=ceil(iw/2)*2:ceil(ih/2)*2'",
        &demuxerf_path.display(),
        &videoopts.args,
    );
    if opt.overwrite {
        ffmpeg_cmd += " -y"
    }
    let filestem = opt
        .input
        .file_stem()
        .and_then(OsStr::to_str)
        .ok_or_else(|| format!("No filestem; {}", &opt.input.display()))?;
    ffmpeg_run(
        &ffmpeg_cmd,
        filestem,
        videoopts.two_pass,
        &videoopts.container,
    );

    Ok(())
}

/// Creates ffmpeg demuxer from json file in extracted zip or json from file near zip
fn animdata2demux(opt: &Opt, tempdir: &tempfile::TempDir) -> BResult<Vec<(String, f64)>> {
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

use std::{collections::HashMap, error::Error, ffi::OsString, io::Write, path::Path};

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
    #[structopt(short, long = "ffmpeg", default_value = "x264")]
    ffmpeg_args: String,
    #[structopt(long = "p:crf", default_value = "18")]
    preset_crf: u8,
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

    let mut container_type = ""; // TODO
    let mut ffmpegargs = utils::match_ffmpegargs(opt.ffmpeg_args.as_str(), &mut container_type);
    if utils::is_ffmpeg_preset(opt.ffmpeg_args.as_str()) {
        ffmpegargs += format!(" -crf {}", &opt.preset_crf).as_str();
    }

    let demuxerf_path = tempdir.path().join("concat_demuxer");
    demuxer_fill_from_json(&demuxerf_path, json_mux)?;
    let ffmpeg_cmd = format!(
        "-f concat -i {} {} \
        -vf pad=ceil(iw/2)*2:ceil(ih/2)*2' {}.{}",
        &demuxerf_path.to_str().unwrap(),
        &ffmpegargs,
        &filestem,
        &container_type
    );

    println!("{:?}", &ffmpeg_cmd);
    let p = std::process::Command::new("ffmpeg")
        .args(ffmpeg_cmd.split(' '))
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::inherit())
        .output()
        .unwrap();
    println!("{}", std::str::from_utf8(&p.stderr).unwrap());

    Ok(())
}

fn demuxer_fill_from_json<T>(demuxerf_path: &Path, json_mux: T) -> Result<(), Box<dyn Error>>
where
    T: Iterator<Item = (String, f64)>,
{
    let demuxerf = std::fs::File::create(demuxerf_path)?;
    let mut demuxerf = std::io::BufWriter::new(demuxerf);
    demuxerf.write_all(b"ffconcat version 1.0\n")?;
    for i in json_mux {
        demuxerf.write_all(format!("file \'{}\'\nduration {}\n", i.0, i.1,).as_bytes())?
    }
    demuxerf.flush()?;
    Ok(())
}

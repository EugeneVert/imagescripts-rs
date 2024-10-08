use std::{
    collections::HashMap,
    fs::File,
    io::Write,
    ops::Deref,
    path::{Path, PathBuf},
};

use serde::Deserialize;

use crate::BResult;

pub mod ffmpeg_concat;
pub mod video;
pub mod zip2video;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct VideoEncodeSettings {
    container: Option<String>,
    two_pass: Option<bool>,
    quality_slider: Option<String>,
    args: String,
}

#[derive(Clone, Debug)]
pub struct VideoOpts {
    pub container: String,
    pub two_pass: bool,
    pub args: String,
    quality_slider: String,
    json: HashMap<String, VideoEncodeSettings>,
}

impl VideoOpts {
    pub fn new(config_file: &Path) -> BResult<VideoOpts> {
        if !config_file.exists() {
            let mut writer = File::create(config_file)?;
            writer.write_all(
r#"{
  "default": {
    "container": "mp4",
    "two-pass": false,
    "quality-slider": "crf",
    "args": ""
  },
  "x264": {
    "container": "mp4",
    "args": "-c:v libx264 -pix_fmt yuv444p -preset veryslow -tune animation -deblock -1:-1"
  },
  "x265": {
    "container": "mp4",
    "two-pass": true,
    "args": "-c:v libx265 -pix_fmt yuv444p -preset veryslow -tune animation -x265-params bframes=8:psy-rd=1:aq-mode=3:aq-strength=0.8:deblock=-3,-3"
  },
  "apng": {
    "container": "apng",
    "args": "-c:v apng"
  },
  "webp": {
    "container": "webp",
    "quality-slider": "qscale",
    "args": "-c:v libwebp_anim"
  },
  "vp9": {
    "container": "webm",
    "two-pass": true,
    "args": "-c:v libvpx-vp9 -pix_fmt yuv444p -b:v 0"
  },
  "aom-av1": {
    "container": "webm",
    "args": "-c:v libaom-av1 -pix_fmt yuv444p10le -cpu-used 4 -tile-rows 2 -strict -2 -aq-mode 1 -aom-params enable-chroma-deltaq=1:deltaq-mode=3:qm-min=0:sharpness=2"
  },
  "aom-av1-simple": {
    "container": "webm",
    "args": "-c:v libaom-av1 -pix_fmt yuv444p10le -b:v 0 -cpu-used 4 -tile-rows 2 -strict -2"
  }
}
"#.as_bytes()
                )?;
        }
        let reader = File::open(config_file)?;
        let json: HashMap<String, VideoEncodeSettings> = serde_json::from_reader(reader)?;
        let default = json
            .get("default")
            .expect("No 'default' filed in config file");

        Ok(VideoOpts {
            args: default.args.to_string(),
            container: default
                .container
                .as_ref()
                .expect("Default 'container'(str) is not set")
                .to_string(),
            two_pass: default
                .two_pass
                .expect("Default use of 'two-pass'(bool) is not set"),
            quality_slider: default
                .quality_slider
                .as_ref()
                .expect("Default 'quality-slider'(str) if not set")
                .to_string(),
            json,
        })
    }

    /// Match preset for ffmpeg if 'args' is preset name.
    pub fn args_match(
        &mut self,
        args: &str,
        container: &Option<String>,
        two_pass: &Option<bool>,
        quality: f32,
    ) {
        if let Some(preset) = self.json.get(args) {
            if let Some(p_c) = &preset.container {
                self.container = p_c.to_string();
            }
            if let Some(p_tp) = preset.two_pass {
                self.two_pass = p_tp;
            }
            self.args = format!(
                "{}{} -{} {} -fps_mode passthrough",
                self.args, preset.args, self.quality_slider, quality
            );
        } else {
            self.args = self.args.to_string() + args;
        }

        if let Some(c) = container {
            self.container = c.to_string();
        }
        if let Some(tp) = two_pass {
            self.two_pass = *tp;
        }
    }

    pub fn presets_list(&self) -> Vec<&str> {
        self.json.keys().map(|s| s.deref()).collect()
    }

    pub fn args_ispreset(&self) -> bool {
        let presets = self.presets_list();
        presets.contains(&self.args.as_str())
    }
}

pub fn ffmpeg_run(ffmpeg_cmd: &str, filestem: &str, two_pass: bool, container: &str) {
    println!("{}", &ffmpeg_cmd);
    if two_pass {
        std::process::Command::new("ffmpeg")
            .args(ffmpeg_cmd.split(' '))
            .args("-pass 1 -an -f null /dev/null".split(' '))
            .status()
            .unwrap();
        std::process::Command::new("ffmpeg")
            .args(ffmpeg_cmd.split(' '))
            .args("-pass 2 -hide_banner".split(' '))
            .arg(format!("{}.{}", &filestem, &container))
            .status()
            .unwrap();
    } else {
        std::process::Command::new("ffmpeg")
            .args(ffmpeg_cmd.split(' '))
            .arg(format!("{}.{}", &filestem, &container))
            .status()
            .unwrap();
    }
}

pub fn ffmpeg_demuxer_create_from_json<T>(
    demuxerf_path: &Path,
    json_mux: &[(String, T)],
) -> BResult<()>
where
    T: std::fmt::Display,
{
    let demuxerf = std::fs::File::create(demuxerf_path)?;
    let mut demuxerf = std::io::BufWriter::new(demuxerf);
    demuxerf.write_all(b"ffconcat version 1.0\n")?;
    for i in json_mux {
        demuxerf.write_all(format!("file \'{}\'\nduration {}\n", i.0, i.1,).as_bytes())?;
    }
    demuxerf.write_all(("file ".to_string() + &json_mux.last().unwrap().0 + "\n").as_bytes())?;
    demuxerf.flush()?;
    Ok(())
}

pub fn ffmpeg_demuxer_create_from_files(demuxerf_path: &Path, input: &[PathBuf]) -> BResult<()> {
    let demuxerf = std::fs::File::create(demuxerf_path)?;
    let mut demuxerf = std::io::BufWriter::new(demuxerf);
    demuxerf.write_all(b"ffconcat version 1.0\n")?;
    for i in input {
        demuxerf.write_all((format!("file \'{}\'\n", i.display())).as_bytes())?;
    }
    demuxerf.write_all((format!("file \'{}\'", &input.last().unwrap().display())).as_bytes())?;
    demuxerf.flush()?;
    Ok(())
}

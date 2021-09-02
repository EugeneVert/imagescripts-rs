use std::{
    error::Error,
    io::Write,
    path::{Path, PathBuf},
};

pub fn ims_init(
    input: &mut Vec<PathBuf>,
    output_dir: &std::path::Path,
    nproc: Option<usize>,
) -> Result<(), Box<dyn Error>> {
    if input[0].to_string_lossy() == "./*" {
        input_get_from_cwd(input)?;
        input_filter_images(input);
    }
    mkdir(output_dir)?;
    if let Some(n) = nproc {
        rayon::ThreadPoolBuilder::new()
            .num_threads(n)
            .build_global()?
    }
    Ok(())
}

pub fn mkdir(dir: &std::path::Path) -> Result<(), std::string::String> {
    if !dir.exists() {
        match std::fs::create_dir_all(dir) {
            Ok(_) => return Ok(()),
            Err(_) => {
                return Err(String::from("Error creating dir: ") + dir.as_os_str().to_str().unwrap())
            }
        };
    };
    Ok(())
}

/// Gather image files from cwd, remove 1'st element from input Vec
///
/// # Examples
/// ```
/// struct Opt {
///     #[structopt(required = false, default_value = "./*", display_order = 0)]
///     input: Vec<PathBuf>,
/// }
/// let opt = Opt::from_iter(args);
/// let mut images = opt.input.to_owned();
/// if images[0].to_string_lossy() == "./*" {
///     utils::input_get_from_cwd(&mut images)?;
///     utils::input_filter_images(&mut images);
///     images.sort_unstable()
/// }
/// ```
pub fn input_get_from_cwd(input: &mut Vec<PathBuf>) -> Result<(), std::io::Error> {
    input.append(
        &mut std::path::Path::new(".")
            .read_dir()?
            .map(|r| r.map(|d| d.path()))
            .collect::<Result<Vec<PathBuf>, _>>()?,
    );
    input.remove(0);
    Ok(())
}

pub fn input_filter_images(input: &mut Vec<PathBuf>) {
    let image_formats = ["png", "jpg", "webp"];
    input.retain(|i| {
        image_formats
            .iter()
            .any(|&format| i.extension().unwrap_or_default() == format)
    });
}

#[derive(Clone, Debug)]
pub struct VideoOpts {
    args: String,
    pub container: Option<String>,
    pub ffmpeg_args: String,
    pub two_pass: Option<bool>,
}

impl VideoOpts {
    pub fn new(args: &str, container: &Option<String>, two_pass: &Option<bool>) -> VideoOpts {
        VideoOpts {
            args: String::from(args),
            container: container.to_owned(),
            ffmpeg_args: String::new(),
            two_pass: two_pass.to_owned(),
        }
    }

    /// Match preset for ffmpeg if 'args' is preset name.
    /// If container is "", assigns preset_container to container
    // TODO toml config
    pub fn args_match(&mut self) {
        let preset_container: &str;
        let preset_two_pass: bool;
        let ffmpegargs = match self.args.as_str() {
            "x264" => {
                preset_container = "mp4";
                preset_two_pass = false;
                "-c:v libx264 -pix_fmt yuv444p -preset veryslow -tune animation -deblock -1:-1"
            }
            "x265" => {
                preset_container = "mp4";
                preset_two_pass = true;
                "-c:v libx265 -pix_fmt yuv444p -preset veryslow -tune animation -x265-params bframes=8:psy-rd=1:aq-mode=3:aq-strength=0.8:deblock=-3,-3"
            }
            "apng" => {
                preset_container = "apng";
                preset_two_pass = false;
                "-c:v apng"
            }
            "webp" => {
                preset_container = "webp";
                preset_two_pass = false;
                "-c:v libwebp_anim"
            }
            "vp9" => {
                preset_container = "webm";
                preset_two_pass = true;
                "-c:v libvpx-vp9 -pix_fmt yuv444p -b:v 0"
            }
            "aom-av1" => {
                preset_container = "mkv";
                preset_two_pass = true;
                "-c:v libaom-av1 -pix_fmt yuv444p10le -b:v 0 -cpu-used 4 -tile-rows 2 -strict -2 -aom-params enable-chroma-deltaq=1"
            }
            "aom-av1-simple" => {
                preset_container = "mkv";
                preset_two_pass = true;
                "-c:v libaom-av1 -pix_fmt yuv444p10le -b:v 0 -cpu-used 4 -tile-rows 2 -strict -2"
            }
            _ => {
                preset_container = "mkv";
                preset_two_pass = false;
                &self.args
            }
        };
        if self.container.is_none() {
            self.container = Some(preset_container.into());
        }
        if self.two_pass.is_none() {
            self.two_pass = Some(preset_two_pass);
        }
        self.ffmpeg_args = ffmpegargs.to_string();
    }

    pub fn presets_list() -> Vec<&'static str> {
        vec![
            "x264",
            "x265",
            "apng",
            "webp",
            "vp9",
            "aom-av1",
            "aom-av1-simple",
        ]
    }

    pub fn args_ispreset(&self) -> bool {
        let presets = Self::presets_list();
        presets.contains(&self.args.as_str())
    }

    pub fn args_preset_add_quality(&mut self, q: f32) {
        if !self.args_ispreset() {
            return;
        }
        println!("{:?}", &self.args);
        match self.args.as_ref() {
            "x264" | "x265" | "vp9" | "aom-av1" | "aom-av1-simple" => {
                self.ffmpeg_args += &format!(" -crf {}", &q)
            }
            "webp" => self.ffmpeg_args += &format!(" -qscale {}", &q),
            _ => panic!(),
        }
        self.args += &format!("-crf {}", &q);
    }
}

pub fn ffmpeg_run(ffmpeg_cmd: &str, filestem: &str, two_pass: bool, container: &str) {
    if two_pass {
        let ffmpeg_cmd_pass1 = ffmpeg_cmd.to_owned() + "-pass 1 -an -f null /dev/null";
        let ffmpeg_cmd_pass2 =
            ffmpeg_cmd.to_owned() + "-pass 2 -hide_banner " + filestem + "." + container;
        ffmpeg_cmd_run(&ffmpeg_cmd_pass1);
        ffmpeg_cmd_run(&ffmpeg_cmd_pass2);
    } else {
        let ffmpeg_cmd_once = ffmpeg_cmd.to_owned() + filestem + "." + container;
        ffmpeg_cmd_run(&ffmpeg_cmd_once);
    }
}

fn ffmpeg_cmd_run(ffmpeg_cmd: &str) {
    println!("{:?}", ffmpeg_cmd);
    std::process::Command::new("ffmpeg")
        .args(ffmpeg_cmd.split(' '))
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .output()
        .unwrap();
}

pub fn ffmpeg_demuxer_create_from_json<T>(
    demuxerf_path: &Path,
    json_mux: &[(String, T)],
) -> Result<(), Box<dyn Error>>
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

pub fn ffmpeg_demuxer_create_from_files(
    demuxerf_path: &Path,
    input: &[PathBuf],
) -> Result<(), Box<dyn Error>> {
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

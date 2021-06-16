use std::path::Path;

pub fn ims_init(input: &mut Vec<String>, output_dir: &std::path::Path, nproc: Option<usize>) {
    if input.get(0).unwrap() == "./*" {
        input_get_from_cwd(input);
    }
    mkdir(output_dir);
    if let Some(n) = nproc {
        rayon::ThreadPoolBuilder::new()
            .num_threads(n)
            .build_global()
            .unwrap()
    }
}

pub fn mkdir(dir: &std::path::Path) {
    if !Path::new(dir).exists() {
        std::fs::create_dir_all(dir)
            .unwrap_or_else(|_| panic!("Error creating dir {}", dir.as_os_str().to_str().unwrap()));
    }
}

/// Gather image-files from cwd, remove 1'st element from input Vec
///
/// # Examples
/// ```
/// struct Opt {
///     #[structopt(required = false, default_value = "./*", display_order = 0)]
///     input: Vec<String>,
/// }
/// let opt = Opt::from_iter(args);
/// let mut images = opt.input.to_owned();
/// if images.get(0).unwrap() == "./*" {
///     images_get_from_cwd(&mut images);
/// }
/// ```
pub fn input_get_from_cwd(input: &mut Vec<String>) {
    let image_formats = ["png", "jpg", "webp"];
    input.append(
        &mut std::path::Path::new(".")
            .read_dir()
            .unwrap()
            .map(|x| x.unwrap().path().into_os_string().into_string().unwrap())
            .collect::<Vec<String>>(),
    );
    input.remove(0);
    input.retain(|i| image_formats.iter().any(|&format| i.ends_with(format)));
}

#[derive(Clone)]
pub struct VideoOpts {
    args: String,
    pub container: Option<String>,
    pub ffmpeg_args: String,
    pub two_pass: Option<bool>,
}

impl VideoOpts {
    pub fn new(args: &str, container: Option<String>, two_pass: Option<bool>) -> VideoOpts {
        VideoOpts {
            args: String::from(args),
            container,
            ffmpeg_args: String::new(),
            two_pass,
        }
    }

    /// Returns preset args for ffmpeg if 'args' is preset name. Else returns 'args'
    /// If container is "", assigns preset_container to container
    pub fn args_match(&mut self) {
        let preset_container: &str;
        let preset_two_pass: bool;
        let ffmpegargs = match self.args.as_str() {
            "x264" => {
                preset_container = "mkv";
                preset_two_pass = true;
                "-c:v libx264 -pix_fmt yuv444p -preset veryslow -tune animation -deblock -3:-3"
            }
            "x265" => {
                preset_container = "mkv";
                preset_two_pass = true;
                "-c:v libx265 -pix_fmt yuv444p -preset veryslow -tune animation -x265-params bframes=8:psy-rd=1:aq-mode=3:aq-strength=0.8:deblock=-3,-3"
            }
            "apng" => {
                preset_container = "apng";
                preset_two_pass = false;
                "-c:v apng"
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
        self.ffmpeg_args = ffmpegargs.into();
    }

    pub fn args_ispreset(&self) -> bool {
        let presets = ["x264", "x265", "apng", "vp9", "aom-av1"];
        presets.contains(&self.args.as_str())
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

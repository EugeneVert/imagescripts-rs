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

pub fn is_ffmpeg_preset(arg: &str) -> bool {
    let presets = ["x264", "x265", "apng"];
    presets.contains(&arg)
}

pub fn match_ffmpegargs(
    args: &str,
    container: &mut &str,
) -> String {
    let ffmpegargs = match args {
        "x264" => {
            *container = "mkv";
            "-c:v libx264 -pix_fmt yuv444p -preset veryslow -tune animation -deblock -3:-3"
        }
        "x265" => {
            *container = "mkv";
            "-c:v libx265 -pix_fmt yuv444p -preset veryslow -tune animation -x265-params bframes=8:psy-rd=1:aq-mode=3:aq-strength=0.8:deblock=-3,-3"
        }
        "apng" => {
            *container = "apng";
            "-c:v apng"
        }
        // "vp9" => {
        //     container = "webm";
        //     format!("")
        // }
        // "libaom-av1" => {
        //     container = "mkv";
        //     format!("")
        // }
        _ => {
            *container = "mkv";
            args
        }
    };
    ffmpegargs.to_string()
}

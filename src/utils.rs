use std::{error::Error, path::PathBuf};

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
    let image_formats = ["png", "jpg", "jxl", "avif", "webp"];
    input.retain(|i| {
        image_formats
            .iter()
            .any(|&format| i.extension().unwrap_or_default() == format)
    });
}

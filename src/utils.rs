use std::{error::Error, path::PathBuf};

pub fn ims_init(
    input: &[PathBuf],
    output_dir: &std::path::Path,
    nproc: Option<usize>,
) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut images = input.to_owned();
    if images[0].to_string_lossy() == "./*" {
        images = read_cwd()?;
        filter_images(&mut images);
    }
    mkdir(output_dir)?;
    if let Some(n) = nproc {
        rayon::ThreadPoolBuilder::new()
            .num_threads(n)
            .build_global()?
    }
    Ok(images)
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

pub fn read_cwd() -> Result<Vec<PathBuf>, std::io::Error> {
    std::path::Path::new(".")
        .read_dir()?
        .map(|r| r.map(|d| d.path()))
        .collect::<Result<Vec<PathBuf>, _>>()
}

pub fn filter_images(input: &mut Vec<PathBuf>) {
    let image_formats = ["png", "jpg", "jxl", "avif", "webp"];
    input.retain(|i| {
        image_formats
            .iter()
            .any(|&format| i.extension().unwrap_or_default() == format)
    });
}

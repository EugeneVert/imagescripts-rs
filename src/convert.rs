// NOTE Q: what about digikam output filename extension? A: Rename images in digikam using dk-album-manage
// DONE replace get_temp_path with Temp crate or delete them mannualy
// DONE resize
// TODO what to do with webp? cjxl/avifenc does't support it
// TODO implement manga mode

use std::path::Path;
use std::{error::Error, path::PathBuf};

use clap::Args;
use image::{self, DynamicImage, GenericImageView};
use rayon::iter::ParallelIterator;
use rayon::prelude::IntoParallelRefIterator;
use tempfile::{self, NamedTempFile};

use crate::cmds::ImageBuffer;
use crate::find::monochrome::image_is_monochrome;

#[derive(Args, Debug, Clone)]
pub struct Opt {
    input: PathBuf,
    output: PathBuf,
    #[arg(short, long)]
    avif: bool,
    #[arg(short, long)]
    manga: Option<u8>,
    #[arg(short, long)]
    renaame_original: bool,
}

#[derive(Debug, Clone, Copy)]
enum Format {
    Png,
    Jpeg,
    Webp,
}

impl Format {
    fn from_file_format(filepath: &Path) -> Option<Self> {
        match filepath
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase()
            .as_str()
        {
            "png" => Some(Self::Png),
            "jpg" | "jpeg" => Some(Self::Jpeg),
            "webp" => Some(Self::Webp),
            _ => None,
        }
    }
    fn as_ext(&self) -> &str {
        match self {
            Format::Png => ".png",
            Format::Jpeg => ".jpg",
            Format::Webp => ".webp",
        }
    }
}

pub fn main(opt: Opt) -> Result<(), Box<dyn Error>> {
    process_image(
        opt.input,
        opt.output,
        opt.avif,
        opt.manga,
        opt.renaame_original,
    )?;
    Ok(())
}

pub fn process_image(
    input_path: PathBuf,
    output_path: PathBuf,
    avif: bool,
    manga: Option<u8>,
    rename_original: bool,
) -> Result<(), Box<dyn Error>> {
    // LOAD
    let mut format = Format::from_file_format(&input_path).ok_or("Can't parse image format")?;
    let mut img = image::open(&input_path).map_err(|e| {
        format!(
            "Can't open input image file from input_path {}: {}",
            &input_path.display(),
            e
        )
    })?;

    // RESIZE
    let size = img.dimensions();
    let filepath;
    let mut tmp1 = None;
    if size.0 > 3508 || size.1 > 3508 {
        tmp1 = Some(tempfile::Builder::new().suffix(".png").tempfile()?);
        let tmp_path1 = tmp1.as_ref().unwrap().path().to_path_buf();
        img = img.resize(3508, 3508, image::imageops::FilterType::Lanczos3);
        img.save(&tmp_path1)?;
        format = Format::Png;
        filepath = tmp_path1;
        println!("resized to 3508");
    } else {
        filepath = input_path.clone();
    }

    // PROCESS MANGA
    if manga.is_some() {
        return process_manga_image(img, filepath, format);
    }

    let monochrome_mse = image_is_monochrome(&img, false);
    let (filepath, _, _is_grayscale, tmp) =
        image_to_grayscale_if_monochrome(img, input_path, format, monochrome_mse)?;
    let img_filesize = std::fs::metadata(&filepath)?.len() as usize;

    println!(
        "N: {:?}, F: {:?}, M_MSE: {:?}",
        filepath.display(),
        format,
        monochrome_mse,
    );

    // ENCODE SETTINGS
    let cmds: Vec<_> = match format {
        Format::Png => match avif {
            true => vec![
                ("cjxl -d 0 -j 0 -m 1 -e 4", "jxl", false, 0),
                ("cavif -Q 90 -f -o", "avif", false, 40),
            ],
            false => vec![
                ("cjxl -d 0 -j 0 -m 1 -e 4", "jxl", false, 0),
                ("cjxl -d 0 -j 0 -m 1 -e 7", "jxl", false, 0),
                ("cjxl -d 1 -j 0 -m 0 -e 7", "jxl", false, 40),
            ],
        },
        Format::Jpeg => match avif {
            true => vec![
                ("cjxl -d 0 -j 1 -m 0 -e 9", "jxl", false, 0),
                ("cavif -Q 90 -f -o", "avif", false, 32),
            ],
            false => vec![
                ("cjxl -d 0 -j 1 -m 0 -e 9", "jxl", false, 0),
                ("cjxl -d 0 -j 0 -m 1 -e 4", "jxl", false, 10),
                ("cjxl -d 1.5 -j 0 -m 0 -e 7", "jxl", false, 32),
            ],
        },
        Format::Webp => todo!(),
    };

    // ENCODE
    let (best, ext) = encode_and_get_best(&filepath, cmds)?;

    // BACKUP
    if rename_original {
        std::fs::rename(
            &input_path,
            input_path.with_file_name(
                input_path
                    .file_stem()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string()
                    + "-bak."
                    + input_path.extension().unwrap().to_str().unwrap(),
            ),
        )?;
    }

    // WRITE RESULT
    if ext == "copy" {
        std::fs::write(
            output_path.with_extension(filepath.extension().unwrap()),
            best,
        )?;
    } else {
        std::fs::write(output_path.with_extension(ext), best)?;
    }

    // CLEANUP
    if let Some(tmp) = tmp1 {
        tmp.close()?;
    }
    if let Some(tmp) = tmp2 {
        tmp.close()?;
    }
    Ok(())
}

fn encode_and_get_best(
    input_path: &Path,
    cmds: Vec<(&str, &str, bool, i32)>,
) -> Result<(Vec<u8>, String), Box<dyn Error>> {
    let img_filesize = std::fs::metadata(input_path)?.len() as usize;
    let mut best = &ImageBuffer::default();
    let mut best_filesize: usize = 0;
    let mut best_percentage_of_original = 100;

    let enc_img_buffers: Vec<ImageBuffer> = cmds
        .par_iter()
        .map(|cmd| {
            let mut buff = ImageBuffer::new(cmd.0, cmd.1, cmd.2);
            buff.image_generate(input_path).map(|_| buff)
        })
        .collect::<Result<_, _>>()
        .unwrap();

    for (i, buff) in enc_img_buffers.iter().enumerate() {
        let buff_filesize = buff.get_size();
        let buff_percentage_of_original = (100 * buff_filesize / img_filesize) as i32;
        let better = buff_filesize != 0
            && buff_filesize < img_filesize
            && (best_percentage_of_original - buff_percentage_of_original) > cmds[i].3;

        let printing_status = format!(
            "{}\n{} --> {}\t{:6.2}% {is_better}\t{:>6.2}s",
            &buff.get_cmd(),
            crate::cmds::byte2size(img_filesize as u64),
            crate::cmds::byte2size(buff_filesize as u64),
            buff_percentage_of_original,
            &buff.duration.as_secs_f32(),
            is_better = if better { "* " } else { "" },
        );
        println!("{}", printing_status);

        if better {
            best = buff;
            best_filesize = buff_filesize;
            best_percentage_of_original = buff_percentage_of_original;
        }
    }

    if best_filesize == 0 {
        return Ok((std::fs::read(input_path)?, "copy".to_string()));
    }

    Ok((best.image.to_owned(), best.extension.to_string()))
}

fn process_manga_image(
    _img: DynamicImage,
    _input_path: PathBuf,
    _format: Format,
) -> Result<(), Box<dyn Error>> {
    todo!()
    // if let Some(ncolors) = opt.manga {
    //     let temppath = get_tmp_path(&Format::Png);
    //     img = match format {
    //         Format::Png => image_pngquant(&filepath, &temppath, ncolors)?,
    //         _ => {
    //             img.save(&temppath)?;
    //             image_pngquant(&temppath, &temppath, ncolors)?
    //         }
    //     };
    //     filepath = temppath;
    // }
}

// fn image_pngquant(
//     filepath: &Path,
//     save_path: &Path,
//     ncolors: i8,
// ) -> Result<DynamicImage, Box<dyn Error>> {
//     let p = std::process::Command::new("pngquant")
//         .args(["-o", "-"])
//         .arg("--nofs")
//         .arg(ncolors.to_string())
//         .arg(filepath)
//         .output()?;
//     // if p.stdout.len() < std::fs::metadata(filepath)?.len() as usize {
//     std::fs::write(save_path, &p.stdout)?;
//     // }
//     image::load_from_memory_with_format(&p.stdout, image::ImageFormat::Png).map_err(|e| e.into())
// }

/// Image path, loaded image, monochrome flag and possible handle to temporary file
type PossibleMonochromeImageBundle = (PathBuf, DynamicImage, bool, Option<NamedTempFile>);

/// Convert image to grayscale if image is monochrome.
/// Ask user is image monochrome if unsure.
/// Return original filepath and image otherwise
fn image_to_grayscale_if_monochrome(
    img: DynamicImage,
    filepath: PathBuf,
    format: Format,
    monochrome_mse: f32,
) -> Result<PossibleMonochromeImageBundle, Box<dyn Error>> {
    if monochrome_mse == -1.0 {
        return Ok((filepath, img, true, None));
    }
    if monochrome_mse >= 896.0 {
        return Ok((filepath, img, false, None));
    }
    if monochrome_mse > 0.0 && !ask_is_monochrome(&filepath) {
        return Ok((filepath, img, false, None));
    }

    let tmp = tempfile::Builder::new()
        .suffix(&format.as_ext())
        .tempfile()?;
    let tmp_path = tmp.path().to_path_buf();

    match format {
        Format::Jpeg => {
            let _img = jpegtran_grayscale(&filepath, &tmp_path)?;
            Ok((tmp_path, _img, true, Some(tmp)))
        }
        _ => {
            let _img = if img.color().has_alpha() {
                DynamicImage::ImageLumaA8(img.into_luma_alpha8())
            } else {
                DynamicImage::ImageLuma8(img.into_luma8())
            };
            _img.save(&tmp_path)?;
            Ok((tmp_path, _img, true, Some(tmp)))
        }
    }
}

fn ask_is_monochrome(filepath: &Path) -> bool {
    open::that(filepath).unwrap();
    match tinyfiledialogs::message_box_yes_no(
        "convert",
        "Is image monochrome?",
        tinyfiledialogs::MessageBoxIcon::Question,
        tinyfiledialogs::YesNo::No,
    ) {
        tinyfiledialogs::YesNo::No => false,
        tinyfiledialogs::YesNo::Yes => true,
    }
}

fn jpegtran_grayscale(filepath: &Path, save_path: &Path) -> Result<DynamicImage, Box<dyn Error>> {
    let p = std::process::Command::new("jpegtran")
        .arg("-grayscale")
        .arg("-optimize")
        .args(["-copy", "all"])
        .arg(filepath)
        .output()?;
    // if p.stdout.len() < std::fs::metadata(filepath)?.len() as usize {
    std::fs::write(save_path, &p.stdout)?;
    // }
    image::load_from_memory_with_format(&p.stdout, image::ImageFormat::Jpeg).map_err(|e| e.into())
}

// fn get_tmp_path(format: &Format) -> PathBuf {
//     let tmpdir = std::env::temp_dir().join("ims-rs-convert");
//     mkdir(&tmpdir).unwrap();
//     let nanos_rng = (std::time::SystemTime::now()
//         .duration_since(std::time::UNIX_EPOCH)
//         .unwrap()
//         .subsec_nanos())
//     .to_string();
//     let tmp_filename = nanos_rng + format.as_ext();
//     let tmp_filepath = tmpdir.join(tmp_filename);
//     println!("{}", tmp_filepath.display());
//     tmp_filepath
// }

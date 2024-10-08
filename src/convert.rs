use std::path::Path;
use std::path::PathBuf;

use clap::Args;
use image::{self, DynamicImage, GenericImageView};
use rayon::iter::ParallelIterator;
use rayon::prelude::IntoParallelRefIterator;
use tempfile::{self, NamedTempFile};

use crate::{
    cmds::ImageBuffer, find::monochrome::image_is_monochrome, jpegquality::jpeg_quality, BResult,
};

#[derive(Args, Debug, Clone)]
pub struct Opt {
    input: PathBuf,
    output: PathBuf,
    #[arg(short = 'a', long)]
    avif: bool,
    #[arg(short, long)]
    manga: Option<u8>,
    #[arg(short = 'r', long)]
    rename_original: bool,
    #[arg(short = 'm', long)]
    no_monochrome_check: bool,
    #[arg(short = 's', long, default_value = "3508")]
    resize: u32,
    #[arg(short = 'q', long, default_value = "1.0")]
    quality_multiplier: f32,
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

pub fn main(opt: Opt) -> BResult<()> {
    process_images(
        opt.input,
        opt.output,
        ConvertOptions {
            use_avif: opt.avif,
            manga_mode: opt.manga,
            rename_original: opt.rename_original,
            monochrome_check: !opt.no_monochrome_check,
            resize: opt.resize,
            quality_multiplier: opt.quality_multiplier,
        },
    )?;
    Ok(())
}

#[derive(Debug)]
pub struct ConvertOptions {
    pub use_avif: bool,
    pub manga_mode: Option<u8>,
    pub rename_original: bool,
    pub monochrome_check: bool,
    pub resize: u32,
    pub quality_multiplier: f32,
}

pub fn process_images(
    input_path: PathBuf,
    output_path: PathBuf,
    options: ConvertOptions,
) -> BResult<()> {
    // LOAD
    let mut format = Format::from_file_format(&input_path).ok_or("Can't parse image format")?;
    let mut img = image::open(&input_path).map_err(|e| {
        format!(
            "Can't open input image file from input_path {}: {}",
            &input_path.display(),
            e
        )
    })?;

    // ESTIMATE JPEG QUALITY
    let quality = match format {
        Format::Jpeg => Some(jpeg_quality(&input_path)?),
        _ => None,
    };

    // RESIZE
    let size = img.dimensions();
    let filepath;
    let mut tmp1 = None;
    let resize_starting_from = options.resize + options.resize / 100;
    if options.resize != 0
        && (size.0 > resize_starting_from || size.1 > resize_starting_from)
        && quality.unwrap_or(100.0) > 90.0
    {
        tmp1 = Some(tempfile::Builder::new().suffix(".png").tempfile()?);
        let tmp_path1 = tmp1.as_ref().unwrap().path().to_path_buf();
        img = img.resize(
            options.resize,
            options.resize,
            image::imageops::FilterType::Lanczos3,
        );
        img.save(&tmp_path1)?;
        format = Format::Png;
        filepath = tmp_path1;
        println!("resized to {}", options.resize);
    } else {
        filepath = input_path.clone();
    }

    // PROCESS MANGA
    if options.manga_mode.is_some() {
        return process_manga_image(img, filepath, format);
    }

    // MONOCHROME
    let monochrome_mse = if options.monochrome_check {
        image_is_monochrome(&img, false)
    } else {
        f32::INFINITY
    };
    let (filepath, _, _is_grayscale, tmp2) =
        image_to_grayscale_if_monochrome(img, filepath, format, monochrome_mse)?;

    println!(
        "N: {:?}, F: {:?}, M_MSE: {:?}, Q: {}",
        input_path.display(),
        format,
        monochrome_mse,
        quality.unwrap_or_default(),
    );

    // ENCODE SETTINGS
    let cmds: Vec<_> = get_encode_settings(
        format,
        options.use_avif,
        quality,
        options.quality_multiplier,
    );

    // ENCODE
    let (best, ext) = encode_and_get_best(&filepath, cmds)?;

    // BACKUP
    if options.rename_original {
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

// TODO size-dependent quality?
fn get_encode_settings<'a>(
    format: Format,
    use_avif: bool,
    jpg_quality: Option<f32>,
    quality_multiplier: f32,
) -> Vec<(String, &'a str, bool, i32)> {
    let avif_normal_quality = (14.0 * quality_multiplier + 0.5) as i8;
    let avif_low_quality = (21.0 * quality_multiplier + 0.5) as i8;
    let cjxl_hi_quality = 1.0 * quality_multiplier;
    let cjxl_normal_quality = 1.125 * quality_multiplier;
    let cjxl_low_quality = 2.0 * quality_multiplier;
    match format {
        Format::Png => match use_avif {
            true => vec![
                (cjxl_l(9), "jxl", false, 100),
                (avifenc_q(avif_normal_quality), "avif", false, 35),
            ],
            false => vec![
                (cjxl_l(9), "jxl", false, 100),
                (cjxl_d(cjxl_hi_quality), "jxl", false, 45),
            ],
        },
        Format::Jpeg => match (jpg_quality, use_avif) {
            (Some(q), true) if q > 98.0 => {
                vec![
                    (cjxl_tr(7), "jxl", false, 100),
                    (avifenc_q(avif_normal_quality), "avif", false, 42),
                ]
            }
            (Some(q), false) if q > 98.0 => {
                vec![
                    (cjxl_tr(7), "jxl", false, 100),
                    (cjxl_l(9), "jxl", false, 95),
                    (cjxl_d(cjxl_hi_quality), "jxl", false, 50),
                ]
            }
            (Some(q), _) if q < 90.0 => vec![
                (cjxl_tr(9), "jxl", false, 100),
                (cjxl_d(cjxl_low_quality), "jxl", false, 30),
            ],
            (_, true) => {
                vec![
                    (cjxl_tr(7), "jxl", false, 100),
                    (avifenc_q(avif_low_quality), "avif", false, 42),
                ]
            }
            (_, false) => {
                vec![
                    (cjxl_tr(7), "jxl", false, 100),
                    (cjxl_l(9), "jxl", false, 95),
                    (cjxl_d(cjxl_normal_quality), "jxl", false, 50),
                ]
            }
        },
        Format::Webp => todo!(),
    }
}

#[inline]
pub fn cjxl_l(effort: i8) -> String {
    format!("cjxl -d 0 -j 0 -e {effort} --patches=0")
}

#[inline]
pub fn cjxl_d(distance: f32) -> String {
    const CJXL_EFFORT: i8 = 7;
    format!("cjxl -d {distance} -j 0 -e {CJXL_EFFORT} --patches=0")
}

#[inline]
pub fn cjxl_tr(effort: i8) -> String {
    format!("cjxl -d 0 -j 1 -m 0 -e {effort}")
}

// #[inline]
// pub fn cjxl_le(effort: i8) -> String {
//     format!("cjxl -d 0 -j 0 -e {} -m 1 -I 1 -E 3 --patches=0", effort)
// }

#[inline]
pub fn avifenc_q(quality: i8) -> String {
    const AVIFENC_SPEED: i8 = 4;
    format!("avifenc --min 0 --max 63 -d 10 -s {} -j 8 -a end-usage=q -a cq-level={} -a color:enable-chroma-deltaq=1 -a color:deltaq-mode=3 -a tune=ssim", AVIFENC_SPEED, quality)
}

fn encode_and_get_best(
    input_path: &Path,
    cmds: Vec<(String, &str, bool, i32)>,
) -> BResult<(Vec<u8>, String)> {
    let img_filesize = std::fs::metadata(input_path)?.len() as usize;
    let mut best = &ImageBuffer::default();
    let mut best_filesize: usize = img_filesize;

    let enc_img_buffers: Vec<ImageBuffer> = cmds
        .par_iter()
        .map(|cmd| {
            let mut buff = ImageBuffer::new(&cmd.0, cmd.1, cmd.2);
            buff.image_generate(input_path).map(|_| buff)
        })
        .collect::<std::result::Result<_, _>>()
        .unwrap();

    for (i, buff) in enc_img_buffers.iter().enumerate() {
        let buff_filesize = buff.get_size();
        let buff_percentage_of_best = (100 * buff_filesize / best_filesize) as i32;
        let better = buff_filesize != 0
            && buff_filesize < img_filesize
            && buff_percentage_of_best < cmds[i].3;

        let printing_status = format!(
            "{:>9} --> {:<9}{:4.2}% {is_better}\t{:>6.2}s\t{cmd}",
            crate::cmds::byte2size(best_filesize as u64),
            crate::cmds::byte2size(buff_filesize as u64),
            buff_percentage_of_best,
            &buff.duration.as_secs_f32(),
            is_better = if better { "* " } else { "" },
            cmd = &buff.get_cmd(),
        );
        println!("{}", printing_status);

        if better {
            best = buff;
            best_filesize = buff_filesize;
        }
    }

    if best_filesize == img_filesize {
        return Ok((std::fs::read(input_path)?, "copy".to_string()));
    }

    Ok((best.image.to_owned(), best.extension.to_string()))
}

fn process_manga_image(_img: DynamicImage, _input_path: PathBuf, _format: Format) -> BResult<()> {
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
// ) -> BResult<DynamicImage> {
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
) -> BResult<PossibleMonochromeImageBundle> {
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

fn jpegtran_grayscale(filepath: &Path, save_path: &Path) -> BResult<DynamicImage> {
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

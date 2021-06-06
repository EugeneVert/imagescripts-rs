use std::{
    error::Error,
    ffi::OsString,
    path::{Path, PathBuf},
};

use rayon::iter::{ParallelBridge, ParallelIterator};
use structopt::StructOpt;

#[path = "../utils.rs"]
mod utils;

#[rustfmt::skip]
#[derive(StructOpt, Debug)]
#[structopt(name = "imagescripts-rs find", about = " ")]
struct Opt {
    #[structopt(required = false, default_value = "./*", display_order = 0)]
    input: Vec<String>,
    #[structopt(short = "s", long = "size", required = false, default_value = "3508", display_order = 0)]
    px_size: u32,
    #[structopt(short = "p")]
    png_sort: bool,
    #[structopt(short = "p:s")]
    png_px_size: u32,
    #[structopt(long, default_value = "0")]
    nproc: usize,
}

pub fn main(args: Vec<OsString>) -> Result<(), Box<dyn Error>> {
    let opt = Opt::from_iter(args);
    let paths = Paths {
        out_dir: Path::new("./").join(opt.px_size.to_string()),
        out_dir_png: PathBuf::from("./PNG"),
        out_dir_png_size: Path::new("./PNG").join(opt.png_px_size.to_string()),
    };
    utils::mkdir(&paths.out_dir_png);
    utils::mkdir(&paths.out_dir_png_size);

    let mut images = opt.input.to_owned();
    utils::ims_init(&mut images, &paths.out_dir, Some(opt.nproc));

    images
        .iter()
        .par_bridge()
        .for_each(|img| process_image(&img, &paths, &opt).unwrap());

    Ok(())
}

struct Paths {
    out_dir: PathBuf,
    out_dir_png: PathBuf,
    out_dir_png_size: PathBuf,
}

fn process_image(img: &str, paths: &Paths, opt: &Opt) -> Result<(), Box<dyn Error>> {
    let img_dimmensions = image::image_dimensions(&img)?;
    let img_filename = Path::new(img).file_name().unwrap().to_str().unwrap();
    let save_path: Option<PathBuf>;
    println!("File: {}\nSize: {:?}", img, img_dimmensions);

    if opt.png_sort && img.ends_with(".png") {
        if img_dimmensions.0 > opt.png_px_size || img_dimmensions.1 > opt.png_px_size {
            save_path = Some(paths.out_dir_png_size.join(img_filename));
        } else {
            save_path = Some(paths.out_dir_png.join(img_filename));
        }
    } else if img_dimmensions.0 > opt.px_size || img_dimmensions.1 > opt.px_size {
        save_path = Some(paths.out_dir.join(img_filename));
    } else {
        save_path = None;
    }

    if let Some(to) = save_path {
        std::fs::rename(img, to)?;
    }

    // if !image_is_colorfull(img_image, opt.threshold) {
    //     let save_path = format!(
    //         "{}/{}",
    //         out_dir,
    //         Path::new(img).file_name().unwrap().to_str().unwrap()
    //     );
    //     std::fs::rename(img, save_path)?;
    // }

    Ok(())
}

// fn image_is_colorfull(img: image::DynamicImage, threshold: usize) -> bool {
//     let thumb_size = 32;
//     if img.color().has_color() {
//         let thumb = img.resize(thumb_size, thumb_size, image::imageops::Nearest);

//         let mut is_colorfull = false;
//         let res = thumb
//             .into_rgba8()
//             .pixels()
//             .par_bridge()
//             .find_map_any(|pix| {
//                 let pix = pix.0;
//                 let pix_r = *pix.get(0).unwrap() as i32;
//                 let pix_g = *pix.get(1).unwrap() as i32;
//                 let pix_b = *pix.get(2).unwrap() as i32;
//                 if std::cmp::max(
//                     std::cmp::max((pix_r - pix_g).abs(), (pix_r - pix_b).abs()),
//                     (pix_g - pix_b).abs(),
//                 ) > threshold as i32
//                 {
//                     return Some(());
//                 }
//                 None
//             });
//         if res.is_some() {
//             is_colorfull = true;
//         }
//         return is_colorfull;
//     }
//     //else
//     true
// }

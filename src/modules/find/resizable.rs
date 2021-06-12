use std::{
    error::Error,
    ffi::OsString,
    path::{Path, PathBuf},
};

use rayon::iter::{ParallelBridge, ParallelIterator};
use structopt::StructOpt;

use crate::modules::utils;

#[rustfmt::skip]
#[derive(StructOpt, Debug)]
#[structopt(name = "imagescripts-rs find", about = " ")]
struct Opt {
    #[structopt(required = false, default_value = "./*", display_order = 0)]
    input: Vec<String>,
    #[structopt(short = "s", long = "size", required = false, default_value = "3508", display_order = 0)]
    px_size: u32,
    #[structopt(long = "p")]
    png_sort: bool,
    #[structopt(long = "p:s", default_value="1754")]
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

    dir_del_if_empty(&paths.out_dir_png_size)?;
    dir_del_if_empty(&paths.out_dir_png)?;
    dir_del_if_empty(&paths.out_dir)?;

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

    Ok(())
}

fn dir_del_if_empty(d: &Path) -> Result<(), Box<dyn Error>> {
    if std::fs::read_dir(d)?.count() == 0 {
        println!("Rm dir: {:?}", &d);
        std::fs::remove_dir(d)?;
    }

    Ok(())
}

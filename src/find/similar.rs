use std::{
    collections::HashMap,
    error::Error,
    ffi::OsString,
    path::{Path, PathBuf},
};

use clap::{AppSettings, Parser};
use img_hash::{HashAlg, HasherConfig, ImageHash};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use crate::utils::{self, mkdir};

#[rustfmt::skip]
#[derive(Parser, Debug)]
#[clap(setting = AppSettings::AllowNegativeNumbers)]
struct Opt {
    /// input image paths
    #[clap(required = false, default_value = "./*", display_order = 0)]
    input: Vec<PathBuf>,
    /// output directory path
    // #[clap(short, required = false, default_value = "./monochrome", display_order = 0)]
    // out_dir: PathBuf,
    // /// max_diff
    // #[clap(short, default_value = "12")]
    // max_diff: u32,
    /// no_move
    #[clap(short)]
    no_move: bool

}

pub fn main(args: Vec<OsString>) -> Result<(), Box<dyn Error>> {
    let opt = Opt::parse_from(args);

    let mut images = opt.input.to_owned();
    if images[0].to_string_lossy() == "./*" {
        utils::input_get_from_cwd(&mut images)?;
        utils::input_filter_images(&mut images);
    }

    let hasher = HasherConfig::new()
        .preproc_dct()
        .hash_alg(HashAlg::Mean)
        .hash_size(16, 16);
    let res: HashMap<PathBuf, ImageHash> = images
        .par_iter()
        .map(|img| {
            (
                img.to_path_buf(),
                gen_hash(img, &hasher)
                    .unwrap_or_else(|_| panic!("Error processing image: {}", &img.display())),
            )
        })
        .collect();
    println!("HashMap computed");

    let mut inp = String::new();
    let mut similar = Vec::new();
    println!("Select max_diff; 'n' to continue");
    // TODO result groups thummbnails | Imagemagick montage?
    while &inp != "n" {
        inp.clear();
        std::io::stdin().read_line(&mut inp)?;
        inp.pop();
        if let Ok(d) = inp.parse() {
            similar = group_similar(&res, d);
        };
    }

    if !opt.no_move {
        move_simmilar(similar)?;
    }
    Ok(())
}

fn move_simmilar(similar: Vec<Vec<PathBuf>>) -> std::io::Result<()> {
    for group in similar {
        if group.len() == 1 {
            continue;
        }
        let group_dir = &group[0].with_extension("");
        mkdir(group_dir).unwrap();
        for image in group {
            println!("{}", &image.display());
            std::fs::rename(&image, group_dir.join(image.file_name().unwrap()))?;
        }
    }
    Ok(())
}

fn group_similar(res: &HashMap<PathBuf, ImageHash>, max_diff: u32) -> Vec<Vec<PathBuf>> {
    let mut groups: Vec<Vec<PathBuf>> = Vec::new();
    for (p, h) in res {
        let mut group_found = false;
        for group in &mut groups {
            for memeber in group.clone() {
                if (h.dist(res.get(&memeber).unwrap())) <= max_diff {
                    group.push(p.to_path_buf());
                    group_found = true;
                    break;
                }
            }
            if group_found {
                break;
            }
        }
        if !group_found {
            groups.push(vec![p.to_path_buf()])
        }
    }

    for i in groups.iter_mut() {
        i.sort_unstable();
        println!("{:?}", i)
    }
    groups
}

fn gen_hash(img: &Path, hasher: &img_hash::HasherConfig) -> Result<ImageHash, Box<dyn Error>> {
    let img = match img.extension().unwrap_or_default() {
        x if x == "jxl" => {
            image_jxl_decode(img).map(|t| image::open(t.path()))
        },
        x if x == "avif" => {
            image_avif_decode(img).map(|t| image::open(t.path()))
        },
        _ => Ok(image::open(img)),
    }??;
    Ok(hasher.to_hasher().hash_image(&img))
}

fn image_jxl_decode(i: &Path) -> Result<tempfile::NamedTempFile, Box<dyn Error>> {
    let tf_out = tempfile::Builder::new().suffix(".png").tempfile()?;
    let outp = std::process::Command::new("djxl")
        .arg(i)
        .arg(tf_out.path())
        .output()?;
    command_print_if_error(&outp)?;
    Ok(tf_out)
}

fn image_avif_decode(i: &Path) -> Result<tempfile::NamedTempFile, Box<dyn Error>> {
    let tf_out = tempfile::Builder::new().suffix(".png").tempfile()?;
    let outp = std::process::Command::new("avifdec")
        .args(["-d", "8", "--png-compress", "0"])
        .arg(i)
        .arg(tf_out.path())
        .output()?;
    command_print_if_error(&outp)?;
    Ok(tf_out)
}

fn command_print_if_error(output: &std::process::Output) -> std::io::Result<()> {
    if output.status.success() {
        Ok(())
    } else {
        let o = [&output.stderr, "\n".as_bytes(), &output.stdout].concat();
        println!("{}", std::str::from_utf8(&o).unwrap());
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Command returned error status",
        ))
    }
}

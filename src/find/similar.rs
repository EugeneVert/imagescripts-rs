use std::{
    collections::HashMap,
    error::Error,
    fs::File,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use clap::Args;
use image_hasher::{HashAlg, Hasher, HasherConfig, ImageHash};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use serde::{Deserialize, Serialize};
use zip::write::FileOptions;

use crate::utils::{self, mkdir};

#[rustfmt::skip]
#[derive(Args, Debug, Clone)]
pub struct Opt {
    /// input image paths
    #[arg(required = false, default_value = "./*", display_order = 0)]
    input: Vec<PathBuf>,
    /// save image hashes to zipped json file
    #[arg(short)]
    storage: Option<PathBuf>,
    /// no_move
    #[arg(short)]
    no_move: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct JsonData {
    map: HashMap<String, String>,
}

pub fn main(opt: Opt) -> Result<(), Box<dyn Error>> {
    let mut images = opt.input.to_owned();
    if images[0].to_string_lossy() == "./*" {
        images = utils::read_cwd()?;
        utils::filter_images(&mut images);
    }

    // Load saved hashes from zipped json
    let mut map = Arc::new(RwLock::new(HashMap::<String, String>::new()));
    load_map(&opt.storage, &mut map)?;

    let hasher = HasherConfig::new()
        .preproc_dct()
        .hash_alg(HashAlg::Mean)
        .hash_size(16, 16)
        .to_hasher();

    // Process images
    let res: HashMap<PathBuf, ImageHash> = images
        .par_iter()
        .map(|img| {
            (
                img.to_path_buf(),
                gen_hash(img, Arc::clone(&map), &hasher)
                    .unwrap_or_else(|_| panic!("Error processing image: {}", &img.display())),
            )
        })
        .collect();
    println!("HashMap computed");

    save_map(&opt.storage, map)?;

    // Interactive diff selection
    let mut inp = String::new();
    let mut similar = Vec::new();
    println!("Select max_diff; 'n' to continue");
    // TODO result groups thumbnails | Imagemagick montage?
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

fn load_map(
    storage: &Option<PathBuf>,
    map: &mut Arc<RwLock<HashMap<String, String>>>,
) -> Result<(), Box<dyn Error>> {
    if let Some(ref storage) = storage {
        if storage.exists() {
            let fr = File::open(storage)?;
            let mut fz = zip::read::ZipArchive::new(fr)?;
            let f = fz.by_name("data.json")?;
            let data: JsonData = serde_json::from_reader(f)?;
            *map = Arc::new(RwLock::new(data.map));
        }
    }
    Ok(())
}

fn save_map(
    storage: &Option<PathBuf>,
    map: Arc<RwLock<HashMap<String, String>>>,
) -> Result<(), Box<dyn Error>> {
    if let Some(ref storage) = storage {
        let fw = File::create(storage)?;
        let mut fz = zip::write::ZipWriter::new(fw);
        fz.start_file(
            "data.json",
            FileOptions::default().compression_level(Some(9)),
        )?;
        serde_json::to_writer_pretty(
            fz,
            &JsonData {
                map: map.read().unwrap().to_owned(),
            },
        )
        .unwrap();
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

fn gen_hash(
    img: &Path,
    map: Arc<RwLock<HashMap<String, String>>>,
    hasher: &Hasher,
) -> Result<ImageHash, Box<dyn Error>> {
    let hash: ImageHash;
    let filename = img.file_name().unwrap().to_str().unwrap();

    let r = map.read().unwrap();
    let found = r.get(filename).map(|s| s.to_owned());
    drop(r);

    if let Some(h) = found {
        hash = ImageHash::from_base64(&h).unwrap();
    } else {
        let img = match img.extension().unwrap_or_default() {
            x if x == "jxl" => image_jxl_decode(img).map(|t| image::open(t.path())),
            x if x == "avif" => image_avif_decode(img).map(|t| image::open(t.path())),
            _ => Ok(image::open(img)),
        }??;
        let h = hasher.hash_image(&img);
        map.write()
            .unwrap()
            .insert(filename.to_owned(), h.to_base64());
        hash = h;
    }

    Ok(hash)
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

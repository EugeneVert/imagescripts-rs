use std::{
    collections::HashMap,
    error::Error,
    ffi::OsStr,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    sync::RwLock,
};

use clap::Args;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use serde::{Deserialize, Serialize};

use crate::{csv_output, utils};

type BytesIO = Vec<u8>;

#[derive(Args, Debug, Clone)]
pub struct Opt {
    /// input image paths
    #[arg(default_value = "./*", display_order = 0)]
    input: Vec<PathBuf>,
    #[arg(short, default_value = "./out")]
    out_dir: PathBuf,
    /// Commands from json config
    #[arg(short, num_args = 1..)]
    cmds: Vec<String>,
    /// Path to json file with cmds config
    #[arg(long)]
    cmds_config_json: Option<PathBuf>,
    /// (KiB) tolerance of commands to the following ones{n}
    /// {n} (when not saving all results)
    #[arg(short, long, default_value = "100", allow_negative_numbers = true)]
    tolerance: usize,
    /// save all encoded images (Not only the best compressed one)
    #[arg(long = "save")]
    save_all: bool,
    #[arg(long)]
    no_progress: bool,
    /// save information to csv table
    #[arg(long = "csv")]
    csv_save: bool,
    /// path for csv table
    #[arg(long = "csv_path", default_value = "./res.csv")]
    csv_path: PathBuf,
    /// number simultaneously processed images
    #[arg(long, default_value = "1")]
    nproc: usize,
    /// number simultaneously executed cmds for each image
    #[arg(long)]
    nproc_cmd: Option<usize>,
}

pub fn main(opt: Opt) -> Result<(), Box<dyn Error>> {
    // if args.is_empty() {
    //     args = std::env::args_os().collect();
    // }
    let mut images = opt.input.to_owned();
    utils::ims_init(&mut images, &opt.out_dir, opt.nproc_cmd)?;

    // write csv header with cmds
    if opt.csv_save {
        let mut csv_output = csv_output::CsvOutput::new(&opt.csv_path)?;
        // csv header row
        csv_output.write_cmds_header(&opt.cmds)?;
    }

    let settings = match &opt.cmds_config_json {
        None => settings_load(&dirs::config_dir().unwrap().join("vert/cmds_settings.json")),
        Some(x) => settings_load(x),
    }?;

    let resultmap = RwLock::new(HashMap::<String, usize>::new());
    let threadpool = rayon::ThreadPoolBuilder::new()
        .num_threads(opt.nproc)
        .build()?;
    threadpool.install(|| {
        images
            .par_iter()
            .for_each(|image| match process_image(image, &opt, &settings) {
                Ok(res) => *resultmap.write().unwrap().entry(res).or_default() += 1,
                Err(e) => println!("Can't process image {}: {}", &image.display(), &e),
            })
    });

    println!("\nstats: \ncount\t cmd");
    resultmap
        .read()
        .unwrap()
        .iter()
        .for_each(|(cmd, count)| println!("{}\t {}", count, cmd));

    Ok(())
}

/// Generate results from cmds and compare/save/output them
fn process_image(
    img: &Path,
    opt: &Opt,
    settings: &HashMap<String, EncodeSetting>,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    let img_filesize = img.metadata()?.len() as usize;
    let img_dimensions = image::image_dimensions(&img)?;
    let px_count = img_dimensions.0 * img_dimensions.1;
    let tolerance = opt.tolerance * 1024; // KiB

    let out_dir = &opt.out_dir;

    // csv | open writer, push orig image filename&size
    let cmds_count = opt.cmds.len();
    let mut csv_row = vec![String::new(); cmds_count * 2 + 2];
    let mut csv_output = if opt.csv_save {
        csv_row[0] = img.to_string_lossy().to_string();
        csv_row[1] = img_filesize.to_string();
        Some(csv_output::CsvOutput::new(&opt.csv_path)?)
    } else {
        None
    };

    let mut res_filesize: usize = 0;

    let mut res_buff = &ImageBuffer::default();
    // generate results in ImageBuffers for each cmd
    let enc_img_buffers: Vec<ImageBuffer> = opt
        .cmds
        .par_iter()
        .map(|cmd| {
            let mut buff = ImageBuffer::new(cmd, settings);
            buff.image_generate(img).map(|_| buff)
        })
        .collect::<Result<_, _>>()?;

    if !opt.no_progress {
        println!("{}", &img.display());
    }

    // Caclculate & print info for each ImageBuffer
    for (i, buff) in enc_img_buffers.iter().enumerate() {
        let buff_filesize = buff.get_size();
        let buff_bpp = (buff_filesize * 8) as f64 / px_count as f64;
        let percentage_of_original = format!("{:.2}", (100 * buff_filesize / img_filesize));
        let better = buff_filesize != 0
            && buff_filesize < img_filesize
            && (res_filesize == 0
                || (res_filesize as i64 - buff_filesize as i64) > tolerance as i64);

        if !opt.no_progress {
            let printing_status = format!(
                "{}\n{} --> {}\t{:6.2}bpp\t{}% {is_better}\t{:>6.2}s",
                &buff.get_cmd(),
                byte2size(img_filesize as u64),
                byte2size(buff_filesize as u64),
                &buff_bpp,
                percentage_of_original,
                &buff.duration.as_secs_f32(),
                is_better = if better { "* " } else { "" },
            );
            println!("{}", printing_status);
        }

        if opt.csv_save {
            csv_row[2 + i] = buff_filesize.to_string();
            csv_row[2 + cmds_count + i] = percentage_of_original;
        }

        if opt.save_all {
            // save each buffer to save_path
            if buff_filesize == 0 {
                continue;
            }
            let save_path = out_dir.join(format!(
                "{}_{}.{}",
                img.file_stem()
                    .and_then(OsStr::to_str)
                    .unwrap_or_else(|| panic!("No filestem: {}", img.display())),
                i,
                &buff.extension
            ));
            let mut f = std::fs::File::create(save_path)?;
            f.write_all(&buff.image)?;
            continue;
        }

        if better {
            res_buff = buff;
            res_filesize = buff_filesize;
        }
    }

    if let Some(csv_output) = csv_output.as_mut() {
        csv_output.writer.write_record(&csv_row)?;
        csv_output.writer.flush()?;
    }

    if opt.save_all {
        return Ok("".into());
    }

    // save res_buf
    if res_filesize == img_filesize {
        std::fs::copy(img, out_dir.join(img.file_name().unwrap()))?;
        if !opt.no_progress {
            println!("Save: Copy input");
        }
        return Ok("Copy input".into());
    }

    let save_path = out_dir.join(format!(
        "{}.{}",
        img.file_stem()
            .and_then(OsStr::to_str)
            .ok_or_else(|| format!("No filestem: {}", img.display()))?,
        &res_buff.extension
    ));

    let mut f = std::fs::File::create(save_path)?;
    f.write_all(&res_buff.image)?;
    // if !opt.no_progress {
    //     println!("Save: {}", &res_buff.get_cmd());
    // }
    println!();

    Ok(res_buff.get_cmd())
}

#[derive(Default, Debug, Clone)]
struct ImageBuffer {
    image: BytesIO,
    /// Encoder command
    encoder: String,
    /// Get image [from stdout | temporary file]
    output_from_stdout: bool,
    /// Result image file extension (suffix)
    extension: String,
    /// execution time
    duration: core::time::Duration,
}

impl ImageBuffer {
    fn new(cmd: &str, settings: &HashMap<String, EncodeSetting>) -> ImageBuffer {
        let (name, args) = match cmd.split_once('(') {
            Some(s) => s,
            None => (cmd, " "),
        };
        let args: Vec<&str> = (args[..args.len() - 1]).split(',').collect();

        let mut cmd = settings.get(name).unwrap().clone();

        for (i, v) in args.iter().enumerate() {
            cmd.encode = cmd.encode.replace(&format!("%{}%", i + 1), v);
        }

        ImageBuffer {
            encoder: cmd.encode,
            extension: cmd.ext,
            output_from_stdout: cmd.output_from_stdout.is_some(),
            ..Default::default()
        }
    }

    fn get_size(&self) -> usize {
        core::mem::size_of_val(&self.image[..])
    }

    fn get_cmd(&self) -> String {
        self.encoder.to_string()
    }

    fn image_generate(&mut self, img_path: &Path) -> Result<(), Box<dyn Error + Send + Sync>> {
        let time_start = std::time::Instant::now();
        self.gen_from_cmd(img_path)?;
        self.duration = time_start.elapsed();
        Ok(())
    }

    fn gen_from_cmd(&mut self, img_path: &Path) -> std::io::Result<()> {
        let mut split = self.encoder.split_whitespace();
        let encoder = split.next().ok_or(std::io::ErrorKind::InvalidData)?;

        if self.output_from_stdout {
            let output = std::process::Command::new(encoder)
                .args(split)
                .arg(img_path)
                .output()?;
            self.image = output.stdout;
        } else {
            let buffer = tempfile::Builder::new()
                .suffix(&format!(".{}", self.extension))
                .tempfile()?;
            let outp = std::process::Command::new(encoder)
                .arg(img_path)
                .args(split)
                .arg(buffer.path())
                .output()?;
            command_print_if_error(&outp)?;
            self.image = std::fs::read(buffer.path())?;
            buffer.close()?;
            // println!("{}", std::str::from_utf8(&output.stderr).unwrap());
        }
        Ok(())
    }
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

fn byte2size(num: u64) -> String {
    let mut num_f = num as f64;
    for unit in ["", "K", "M", "G"].iter() {
        if num_f < 1024.0 {
            return format!("{:3.1}{}iB", num_f, unit);
        }
        num_f /= 1024.0;
    }
    format!("{:3.1}TiB", num_f)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EncodeSetting {
    encode: String,
    ext: String,
    output_from_stdout: Option<()>,
}

fn settings_load(file: &Path) -> Result<HashMap<String, EncodeSetting>, Box<dyn Error>> {
    if !file.exists() {
        let mut writer = File::create(&file)?;
        writer.write_all(
r#"{
  "cjxl_d": {
    "encode": "cjxl -d %1% -j 0 --patches=0",
    "ext": "jxl"
  },
  "cjxl_l": {
    "encode": "cjxl -d 0 -j 0 -e %1% --patches=0",
    "ext": "jxl"
  },
  "cjxl_tr": {
    "encode": "cjxl -d 0 -j 1 -e %1%",
    "ext": "jxl"
  },
  "cavif_q": {
    "encode": "cavif -Q %1%",
    "ext": "avif"
  },
  "avif_q": {
    "encode": "avifenc --min 0 --max 63 -d 10 -s 4 -j 8 -a end-usage=q -a cq-level=%1% -a color:enable-chroma-deltaq=1 -a color:deltaq-mode=3 -a color:aq-mode=1 -a color:qm-min=0 -a tune=ssim",
    "ext": "avif"
  }
}"#.as_bytes()
        )?;
    }
    let reader = File::open(file)?;
    let json: HashMap<String, EncodeSetting> = serde_json::from_reader(reader)?;
    Ok(json)
}

use std::{
    collections::HashMap,
    error::Error,
    ffi::OsStr,
    io::Write,
    path::{Path, PathBuf},
    sync::RwLock,
};

use clap::Parser;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use crate::{csv_output, utils};

type BytesIO = Vec<u8>;

#[derive(Parser, Debug, Clone)]
pub struct Opt {
    /// input image paths
    #[clap(default_value = "./*", display_order = 0)]
    input: Vec<PathBuf>,
    #[clap(short, default_value = "./out")]
    out_dir: PathBuf,
    /// {preset}:{args} {n}
    /// avaible presets:    {n}
    /// cjxl, avifenc, cafiv, cjpeg, cwebp, png {n}
    /// custom cmd format:  {n}
    /// "{extension}:{encoder}[>(if output to stdout)]:{args}"
    #[clap(short, multiple_values(true))]
    cmds: Vec<String>,
    /// (KiB) tolerance of commands to the following ones{n}
    /// {n} (when not saving all results)
    #[clap(short, long, default_value = "100")]
    tolerance: usize,
    /// save all encoded images (Not only the best compressed one)
    #[clap(long = "save")]
    save_all: bool,
    #[clap(long)]
    no_progress: bool,
    /// save information to csv table
    #[clap(long = "csv")]
    csv_save: bool,
    /// path for csv table
    #[clap(long = "csv_path", default_value = "./res.csv")]
    csv_path: PathBuf,
    /// number simultaneously processed images
    #[clap(long, default_value = "1")]
    nproc: usize,
    /// number simultaneously executed cmds for each image
    #[clap(long)]
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

    let resultmap = RwLock::new(HashMap::<String, usize>::new());
    let threadpool = rayon::ThreadPoolBuilder::new()
        .num_threads(opt.nproc)
        .build()?;
    threadpool.install(|| {
        images
            .par_iter()
            .for_each(|image| match process_image(image, &opt) {
                Ok(res) => *resultmap.write().unwrap().entry(res).or_default() += 1,
                Err(e) => println!("Can't process image {}: {}", &image.display(), &e),
            })
    });

    println!("stats: \ncount\t cmd");
    resultmap
        .read()
        .unwrap()
        .iter()
        .for_each(|(cmd, count)| println!("{}\t {}", count, cmd));

    Ok(())
}

/// Generate results from cmds and compare/save/output them
fn process_image(img: &Path, opt: &Opt) -> Result<String, Box<dyn Error + Send + Sync>> {
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
            let mut buff = ImageBuffer::new(cmd);
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
                &buff.output_extension
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

    if !opt.no_progress {
        println!();
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
        &res_buff.output_extension
    ));

    let mut f = std::fs::File::create(save_path)?;
    f.write_all(&res_buff.image)?;
    if !opt.no_progress {
        println!("Save: {}\n", &res_buff.get_cmd());
    }

    Ok(res_buff.get_cmd())
}

#[derive(Default, Debug, Clone)]
struct ImageBuffer {
    image: BytesIO,
    /// Encoder command (e.g. `cjxl`)
    encoder: String,
    /// Encoder arguments
    args: Vec<String>,
    /// Get image [from stdout | temporary file]
    output_from_stdout: bool,
    /// Result image file extension (suffix)
    output_extension: String,
    /// execution time
    duration: core::time::Duration,
}

impl ImageBuffer {
    fn new(cmd: &str) -> ImageBuffer {
        let split_indexes = cmd.match_indices(':').map(|m| m.0).collect::<Vec<usize>>();
        let preset = match split_indexes.len() {
            1 => true,
            2 => false,
            _ => panic!("Error parsing cmd {}", &cmd),
        };
        // let preset = !cmd.contains(">:");
        if preset {
            let (encoder, args) = (
                cmd[..split_indexes[0]].to_owned(),
                cmd[split_indexes[0] + 1..].to_owned(),
            );
            let mut ib = ImageBuffer {
                encoder: encoder.to_owned(),
                args: args
                    .replace("\\.", ":")
                    .split(' ')
                    .map(|s| s.to_owned())
                    .collect(),
                ..Default::default()
            };
            ib.match_preset(&encoder);
            ib
        } else {
            let (output_extension, mut encoder, args) = (
                cmd[..split_indexes[0]].to_owned(),
                cmd[split_indexes[0] + 1..split_indexes[1]].to_owned(),
                cmd[split_indexes[1] + 1..].to_owned(),
            );

            let output_from_stdout = encoder.ends_with('>');
            if output_from_stdout {
                encoder.pop();
            };

            ImageBuffer {
                encoder,
                args: args
                    .replace("\\.", ":")
                    .split(' ')
                    .map(|s| s.to_owned())
                    .collect(),
                output_from_stdout,
                output_extension,
                ..Default::default()
            }
        }
    }

    // fn new_empty() -> Self {
    //     ImageBuffer {
    //         image: Vec::new(),
    //         cmd: "",
    //         encoder: String::new(),
    //         args: Vec::new(),
    //         output_from_stdout: false,
    //         output_extension: String::new(),
    //         duration: core::time::Duration::new(0, 0),
    //     }
    // }

    fn get_size(&self) -> usize {
        core::mem::size_of_val(&self.image[..])
    }

    fn get_cmd(&self) -> String {
        self.encoder.to_string() + " " + &self.args.join(" ")
    }

    fn image_generate(&mut self, img_path: &Path) -> Result<(), Box<dyn Error + Send + Sync>> {
        let time_start = std::time::Instant::now();
        self.gen_from_cmd(img_path)?;
        self.duration = time_start.elapsed();
        Ok(())
    }

    fn gen_from_cmd(&mut self, img_path: &Path) -> std::io::Result<()> {
        // no arguments -> return None
        if self.args.contains(&"".into()) {
            self.args.pop();
        }

        if self.output_from_stdout {
            let output = std::process::Command::new(&self.encoder)
                .args(&self.args)
                .arg(img_path)
                .output()?;
            self.image = output.stdout;
        } else {
            let buffer = tempfile::Builder::new()
                .suffix(&format!(".{}", self.output_extension))
                .tempfile()?;
            let outp = std::process::Command::new(&self.encoder)
                .arg(img_path)
                .args(&self.args)
                .arg(buffer.path())
                .output()?;
            command_print_if_error(&outp)?;
            self.image = std::fs::read(buffer.path())?;
            buffer.close()?;
            // println!("{}", std::str::from_utf8(&output.stderr).unwrap());
        }
        Ok(())
    }

    fn match_preset(&mut self, preset: &str) {
        let output_extension: &'static str;
        match preset {
            "cjpeg" => {
                output_extension = "jpg";
                self.output_from_stdout = true;
            }
            // TODO use image crate to convert into png
            "png" => {
                output_extension = "png";
            }
            "cjxl" => {
                output_extension = "jxl";
            }
            "avifenc" => {
                output_extension = "avif";
            }
            "cavif" => {
                output_extension = "avif";
            }
            "cwebp" => {
                output_extension = "webp";
            }
            _ => panic!("match error, cmd '{}' not supported", &preset),
        }
        self.output_extension = output_extension.to_string();
        self.encoder = preset.to_string();
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
    return format!("{:3.1}TiB", num_f);
}

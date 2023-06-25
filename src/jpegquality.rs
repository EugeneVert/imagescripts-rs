// Based on jpegquality by Neal Krawetz

use std::{
    error::Error,
    fs::File,
    io::{BufReader, Bytes, Read},
    path::Path,
};

pub fn jpeg_quality(filepath: &Path) -> Result<f32, Box<dyn Error>> {
    let file = std::fs::File::open(filepath)?;
    let mut reader = std::io::BufReader::new(file).bytes();

    let mut header = [0; 15];
    for h in &mut header {
        *h = reader.next().unwrap()?;
    }

    if (header[0] != 0xFF) || (header[1] != 0xD8) {
        return Err("Not a supported JPEG file".into());
    }

    let mut quality_avg = [0.0; 3];
    loop {
        let marker = read_jpeg_marker(&mut reader)?;
        if marker == 0xff00 {
            return Ok(quality_avg[0]);
        }
        let mut box_len = reader.next().unwrap()? as usize * 256 + reader.next().unwrap()? as usize;

        if box_len >= 3 {
            box_len -= 2;
        } else {
            continue;
        }
        if marker != 0xffdb {
            reader.nth(box_len - 1);
            continue;
        }

        if box_len % 65 != 0 {
            return Err("Wrong size for quantization table".into());
        }

        while box_len > 0 {
            let precision = reader.next().unwrap()?;
            box_len -= 1;
            let index = precision & 0x0f;
            // precision = (precision & 0xf0) / 16;
            // println!(
            //     "  Precision: {}; Table index: {} ({})\n",
            //     precision,
            //     index,
            //     if index > 0 {
            //         "chrominance"
            //     } else {
            //         "luminance"
            //     }
            // );

            let mut total: usize = 0;
            let mut total_num = 0;

            while (box_len > 0) && (total_num < 64) {
                let i = reader.next().unwrap()?;
                if total_num != 0 {
                    total += i as usize;
                }
                box_len -= 1;
                total_num += 1;
            }
            total_num -= 1;
            if index < 3 {
                quality_avg[index as usize] = 100.0 - total as f32 / total_num as f32;
                // println!("Esitmated quality level: {}", quality_avg[index as usize]);
                for i in (index + 1)..3 {
                    quality_avg[i as usize] = quality_avg[index as usize];
                }
            }

            if index > 0 {
                let diff = (quality_avg[0] - quality_avg[1]).abs() * 0.49
                    + (quality_avg[0] - quality_avg[2]).abs() * 0.49;
                let quality_f = (quality_avg[0] + quality_avg[1] + quality_avg[2]) / 3.0 + diff;
                // println!("Average quality: {}", quality_f);
                return Ok(quality_f);
            }
        }
    }
}

fn read_jpeg_marker(reader: &mut Bytes<BufReader<File>>) -> std::io::Result<u32> {
    let mut b1 = 0;
    let mut b2 = 0;

    read_b1(reader, &mut b1)?;
    read_b2(reader, &mut b1, &mut b2)
}

fn read_b1(reader: &mut Bytes<BufReader<File>>, b1: &mut u8) -> std::io::Result<()> {
    *b1 = reader.next().unwrap()?;
    while *b1 != 0xff {
        *b1 = match reader.next() {
            Some(val) => val?,
            None => return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "")),
        }
    }
    Ok(())
}

fn read_b2(reader: &mut Bytes<BufReader<File>>, b1: &mut u8, b2: &mut u8) -> std::io::Result<u32> {
    *b2 = reader.next().unwrap()?;
    if *b2 == 0xff {
        read_b2(reader, b1, b2)?;
    }
    if *b2 == 0x00 {
        read_b1(reader, b1)?;
    }
    Ok(*b1 as u32 * 256 + *b2 as u32)
}

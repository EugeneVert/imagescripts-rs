use std::{
    error::Error,
    io::{BufRead, BufReader, Read, Seek},
    path::PathBuf,
};

use clap::Args;

type Result<T> = std::io::Result<T>;

const PNG_HEADER: &[u8] = b"\x89PNG\r\n\x1a\n";
const CRC_LENGTH: u32 = 4;

#[derive(Args, Debug, Clone)]
pub struct Opt {
    input: PathBuf,
}

pub fn main(opt: Opt) -> Result<()> {
    let f = std::fs::File::open(opt.input)?;
    let mut br = BufReader::new(f);
    if decode(&mut br)? {
        println!("true");
    } else {
        println!("false");
    };

    Ok(())
}

#[derive(Debug)]
struct ChunkHeaderData {
    length: u32,
    typ: [u8; 4],
}

impl ChunkHeaderData {
    fn new(reader: &mut (impl Read + Seek)) -> Result<Self> {
        let length = read_uint32(reader)?;
        let mut t = [0, 0, 0, 0];
        reader.read_exact(&mut t)?;
        Ok(Self { length, typ: t })
    }
}

fn read_uint32(reader: &mut (impl Read + Seek)) -> Result<u32> {
    let mut u = [0, 0, 0, 0];
    reader.read_exact(&mut u)?;
    Ok(u32::from_be_bytes(u))
}

fn decode(reader: &mut (impl Read + Seek)) -> Result<bool> {
    let mut header = [0u8; PNG_HEADER.len()];
    reader.read_exact(&mut header)?;
    if header != PNG_HEADER {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "PNG header not found",
        ));
    }

    loop {
        let chd = ChunkHeaderData::new(reader)?;
        // debug!("{}, {}", core::str::from_utf8(&chd.t).unwrap(), chd.length);
        match &chd.typ {
            b"acTL" => {
                let length = read_uint32(reader)?;
                return Ok(length >= 2);
            }
            b"IDAT" => {
                return Ok(false);
            }
            _ => {
                reader.seek(std::io::SeekFrom::Current((chd.length + CRC_LENGTH) as i64))?;
            }
        }
    }
}

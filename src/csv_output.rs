use std::{fs, path::Path};

use csv::Writer;

#[derive(Debug)]
pub struct CsvOutput {
    // path: PathBuf,
    pub writer: Writer<fs::File>,
}

impl CsvOutput {
    pub fn new(path: &Path) -> csv::Result<Self> {
        let file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .append(true)
            .open(path)?;
        Ok(Self {
            // path: path.to_owned(),
            writer: csv::WriterBuilder::new().delimiter(b'\t').from_writer(file),
        })
    }

    pub fn write_cmds_header(&mut self, cmds: &[String]) -> csv::Result<()> {
        let mut csv_row = Vec::from(["", ""]);
        for cmd in cmds {
            csv_row.push(cmd);
        }
        csv_row.extend(vec!["%"; cmds.len()]);
        self.writer.write_record(csv_row)?;
        self.writer.flush()?;
        Ok(())
    }
}

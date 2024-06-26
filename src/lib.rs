use std::error::Error;

pub mod find {
    pub mod bpp;
    pub mod detailed;
    pub mod monochrome;
    pub mod resizable;
    pub mod similar;
}
pub mod cmds;
pub mod convert;
pub mod csv_output;
pub mod gen;
pub mod is_apng;
pub mod jpegquality;
pub mod utils;

pub mod args;

pub type BResult<T> = std::result::Result<T, Box<dyn Error + Sync + Send + 'static>>;

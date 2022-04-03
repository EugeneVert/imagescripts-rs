#[path = "modules"]
pub mod modules {
    pub mod find {
        pub mod bpp;
        pub mod monochrome;
        pub mod resizable;
        pub mod similar;
        pub mod detailed;
    }
    pub mod generate {
        pub mod ffmpeg_concat;
        pub mod video;
        pub mod zip2video;
    }
    pub mod cmds;
    pub mod utils;
}

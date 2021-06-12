#[path = "modules"]
pub mod modules {
    pub mod find {
        pub mod bpp;
        pub mod grayscale;
        pub mod resizable;
    }
    pub mod generate {
        pub mod video;
        pub mod zip2video;
    }
    pub mod cmds;
    pub mod utils;
}

use clap::{Parser, Subcommand};

use crate::*;

#[derive(Parser, Clone, Debug)]
pub struct Opt {
    #[command(subcommand)]
    pub subcommand: Commands,
}

#[derive(Subcommand, Clone, Debug)]
pub enum Commands {
    Find {
        #[command(subcommand)]
        subcommand: SelectableFind,
    },
    Gen {
        #[command(subcommand)]
        subcommand: SelectableGen,
    },
    Cmds(cmds::Opt),
    Convert(convert::Opt),
    ShellCompletions,
}

#[derive(Subcommand, Clone, Debug)]
pub enum SelectableFind {
    Bpp(find::bpp::Opt),
    Monochrome(find::monochrome::Opt),
    Resizable(find::resizable::Opt),
    Similar(find::similar::Opt),
    Detailed(find::detailed::Opt),
}

#[derive(Subcommand, Clone, Debug)]
pub enum SelectableGen {
    FfmpegConcat(gen::ffmpeg_concat::Opt),
    Video(gen::video::Opt),
    Zip2video(gen::zip2video::Opt),
}

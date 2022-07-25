use clap::{Parser, Subcommand};

use crate::*;

#[derive(Parser, Clone, Debug)]
#[clap(disable_help_subcommand(true))]
pub struct Opt {
    #[clap(subcommand)]
    pub subcommand: Commands,
}

#[derive(Subcommand, Clone, Debug)]
pub enum Commands {
    Find {
        #[clap(subcommand)]
        subcommand: SelectableFind,
    },
    Gen {
        #[clap(subcommand)]
        subcommand: SelectableGen,
    },
    Cmds(cmds::Opt),
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

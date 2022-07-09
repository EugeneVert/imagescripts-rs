use clap::{IntoApp, StructOpt};
use clap_complete::{generate, Shell};
use ims_rs::*;
use std::{error::Error, fs, io};

fn main() -> Result<(), Box<dyn Error>> {
    let opt = args::Opt::parse();

    match opt.subcommand {
        args::Commands::Find { subcommand } => match subcommand {
            args::SelectableFind::Bpp(opt) => find::bpp::main(opt)?,
            args::SelectableFind::Monocrome(opt) => find::monochrome::main(opt)?,
            args::SelectableFind::Resizable(opt) => find::resizable::main(opt)?,
            args::SelectableFind::Similar(opt) => find::similar::main(opt)?,
            args::SelectableFind::Detailed(opt) => find::detailed::main(opt)?,
        },
        args::Commands::Gen { subcommand } => match subcommand {
            args::SelectableGen::FfmpegConcat(opt) => gen::ffmpeg_concat::main(opt)?,
            args::SelectableGen::Video(opt) => gen::video::main(opt)?,
            args::SelectableGen::Zip2video(opt) => gen::zip2video::main(opt)?,
        },
        args::Commands::Cmds(opt) => cmds::main(opt)?,
        args::Commands::ShellCompletions => install_shell_completions()?,
    }
    Ok(())
}

fn install_shell_completions() -> io::Result<()> {
    let p_zsh = dirs::home_dir().unwrap().join(".zsh/zfunctions/_ims-rs");
    let mut f_zsh = fs::File::create(&p_zsh)?;
    generate(Shell::Zsh, &mut args::Opt::command(), "ims-rs", &mut f_zsh);
    println!("Zsh completions installed: {}", &p_zsh.display());
    // let mut f_bash = fs::File::create(
    //     dirs::home_dir().join(".local/share/bash-completion/completions/pixiv-scripts"),
    // )?;
    // generate(Shell::Bash, &mut Opt::command(), "pixiv-scripts", &mut f_bash);
    Ok(())
}

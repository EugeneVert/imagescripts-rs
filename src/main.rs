use clap::{CommandFactory, Parser};
use clap_complete::{generate, Shell};
use ims_rs::*;
use std::{env, fs, io};

fn main() -> BResult<()> {
    let opt = args::Opt::parse();

    match opt.subcommand {
        args::Commands::Find { subcommand } => match subcommand {
            args::SelectableFind::Bpp(opt) => find::bpp::main(opt)?,
            args::SelectableFind::Monochrome(opt) => find::monochrome::main(opt)?,
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
        args::Commands::Convert(opt) => convert::main(opt)?,
        args::Commands::IsApng(opt) => is_apng::main(opt)?,
        args::Commands::ShellCompletions => gen_shell_completions()?,
    }
    Ok(())
}

fn gen_shell_completions() -> io::Result<()> {
    let p_zsh = env::current_dir()?.join("_ims-rs");
    let mut f_zsh = fs::File::create(&p_zsh)?;
    generate(Shell::Zsh, &mut args::Opt::command(), "ims-rs", &mut f_zsh);
    println!("Zsh completions installed: {}", &p_zsh.display());
    // generate(Shell::Bash, &mut Opt::command(), "pixiv-scripts", &mut f_bash);
    Ok(())
}

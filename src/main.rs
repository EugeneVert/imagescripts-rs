use imagescripts_rs::modules;
use std::ffi::OsString;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<OsString> = std::env::args_os().collect();
    if args.len() < 2 {
        print_modules();
        return Ok(());
    }

    let selector_module = args[1].to_str().unwrap();
    let selector_submodule = match args.get(2) {
        Some(x) => x.to_str().unwrap(),
        None => "None",
    };
    println!("{} {}", selector_module, selector_submodule);

    let args_ind;
    if ["cmds", "size"].contains(&selector_module) {
        args_ind = 1;
    } else {
        args_ind = 2;
    }

    let args4module = args[args_ind..args.len()].to_vec();
    // println!("{:?}", args4module);

    match selector_module {
        "find" => {
            match selector_submodule {
                "bpp" => modules::find::bpp::main(args4module)?,
                "monochrome" => modules::find::monochrome::main(args4module)?,
                "resizable" => modules::find::resizable::main(args4module)?,
                // "samesize" => {}
                // "simmilar" => {}
                _ => (print_err(&selector_submodule)),
            };
        }
        "gen" => match selector_submodule {
            "ffmpeg_concat" => modules::generate::ffmpeg_concat::main(args4module)?,
            "video" => modules::generate::video::main(args4module)?,
            "zip2video" => modules::generate::zip2video::main(args4module)?,
            _ => (print_err(&selector_submodule)),
        },
        "cmds" => modules::cmds::main(args4module)?,
        // TODO: "size" => {}
        _ => (print_err(&selector_submodule)),
    };
    Ok(())
}

fn print_modules() {
    println!(
        "\
Avaible options:
    cmds
    find ---\\
        bpp
        monochrome
        resizable
    gen  ---\\
        ffmpeg_concat
        video
        zip2video"
    );
}

fn print_err(x: &str) {
    println!("{}", "No such option: ".to_string() + x);
    print_modules();
}

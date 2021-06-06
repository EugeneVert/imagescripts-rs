
use std::ffi::OsString;
use imagescripts_rs::modules;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<OsString> = std::env::args_os().collect();

    let selector_module = args[1].to_str().unwrap();
    let selector_submodule = match args.get(2) {
        Some(x) => x.to_str().unwrap(),
        None => "None",
    };
    println!("{:?} {:?}", selector_module, selector_submodule);

    let args_ind;
    if ["cmds", "size"].contains(&selector_module) {
        args_ind = 1;
    } else {
        args_ind = 2;
    }

    let args4module = args[args_ind..args.len()].to_vec();
    println!("{:?}", args4module);

    match selector_module {
        "find" => {
            match selector_submodule {
                "bpp" => modules::find::bpp::main(args4module)?,
                "grayscale" => modules::find::grayscale::main(args4module)?,
                "resizeble" => modules::find::resizable::main(args4module)?,
                "samesize" => {}
                "simmilar" => {}
                _ => (print_err()),
            };
        }
        "cmds" => modules::cmds::main(args4module)?,
        "size" => {}
        _ => (print_err()),
    };
    Ok(())
}

fn print_err() {
    println!("No such option")
}

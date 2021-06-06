
use std::ffi::OsString;

use clap::{App, Arg};

pub fn main(args: Vec<OsString>) -> std::io::Result<()> {
    // if args.is_empty() {
    //     args = std::env::args_os().collect();
    // }
    let matches = App::new("imagescripts-rs")
        .about(" ")
        .arg(
            Arg::with_name("path")
                .takes_value(true)
                .required(false)
                .default_value(".")
                .display_order(0),
        )
        .arg(
            Arg::with_name("lesser")
                .short("l")
                .takes_value(true)
                .required(true)
                .conflicts_with("bigger")
        )
        .arg(
            Arg::with_name("bigger")
                .short("b")
                .takes_value(true)
                .required(true)
                .conflicts_with("lesser")
        )
        .arg(
            Arg::with_name("mv")
                .short("m")
                .long("mv")
                .takes_value(false),
        )
        .get_matches_from(args);        

    println!("{:?}", matches);
        
    

    Ok(())
}

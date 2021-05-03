mod error;
mod ifd;
mod metadata;

use crate::error::Error;
use crate::metadata::MetadataParser;
use clap::{crate_version, App, Arg};
use log::LevelFilter;

fn main() -> Result<(), Error> {
    let matches = App::new("DarkMagic")
        .version(crate_version!())
        .author("Christopher Berner")
        .arg(
            Arg::with_name("v")
                .short("v")
                .multiple(true)
                .help("Sets the level of verbosity"),
        )
        .arg(
            Arg::with_name("INPUT_FILE")
                .help("Sets the input file to use")
                .required(true)
                .index(1),
        )
        .get_matches();

    let verbosity: u64 = matches.occurrences_of("v");
    let log_level = match verbosity {
        0 => LevelFilter::Error,
        1 => LevelFilter::Warn,
        2 => LevelFilter::Info,
        3 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    };

    env_logger::builder()
        .format_timestamp_nanos()
        .filter_level(log_level)
        .init();

    let path = matches.value_of("INPUT_FILE").unwrap();

    let parser = MetadataParser::new();
    println!("{:?}", parser.read_file(path)?);

    Ok(())
}

use std::path::PathBuf;

use bpaf::Parser;
use frost_codegen::errors::Error;
use frost_codegen::run;
use frost_codegen::Opts;

fn build_parser() -> impl bpaf::Parser<Opts> {
    let input_paths = bpaf::short('i')
        .long("input_path")
        .help(
            "Path to a folder containing ros msg files. Can be supplied multiple times. (searches recursively)",
        )
        .argument::<PathBuf>("INPUT_PATH")
        .many();

    let output_path = bpaf::short('o')
        .long("output_path")
        .help("Path to a folder which will contain generated Rust files.")
        .argument::<PathBuf>("OUTPUT_PATH");

    bpaf::construct!(Opts {
        input_paths,
        output_path
    })
}
fn main() -> Result<(), Error> {
    let opts = build_parser().to_options().run();
    run(opts)
}

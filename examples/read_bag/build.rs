use std::env;
use std::path::Path;
use std::path::PathBuf;

use frost_codegen::Opts;

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("msgs.rs");

    frost_codegen::run(Opts {
        input_paths: vec![
            PathBuf::from("fixtures/dummy_msgs"),
            PathBuf::from("../../std_msgs"),
        ],
        output_path: dest_path,
    })
    .unwrap();
    println!("cargo:rerun-if-changed=build.rs");
}

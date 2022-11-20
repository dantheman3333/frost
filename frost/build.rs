extern crate rustc_version;

use rustc_version::{version_meta, Channel};

fn main() {
    // when the nightly toolchain is used, set a "nightly" cfg
    // so that the benchmarks can be compiled and otherwise ignored
    if version_meta().unwrap().channel == Channel::Nightly {
        println!("cargo:rustc-cfg=nightly");
    }
}

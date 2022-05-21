use std::env;
use std::process::Command;

pub fn main() {
    if env::var("PROFILE") != Ok("release".to_owned()) {
        Command::new("./tests/scripts/python_gen_test/generate.sh")
            .output()
            .expect("Failed to generate test fixtures");
    }
}

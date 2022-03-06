use std::io;

use clap::Parser;

use frost::Bag;

#[derive(Parser, Debug)]
#[clap(version)]
struct Args {
    file_path: String,
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    let bag = Bag::from(args.file_path)?;
    println!("{:?}", bag.file_path);

    Ok(())
}

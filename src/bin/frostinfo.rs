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

    
    println!("{0: <13}{1}", "path:", bag.file_path.to_string_lossy());
    println!("{0: <13}{1}", "version:", bag.version);
    println!("{0: <13}{1:.2}s", "duration:", bag.duration().as_secs());

    Ok(())
}

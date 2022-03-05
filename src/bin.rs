use std::{io, env};

use frost::Bag;

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let file_path = args.get(1).expect("Please enter a path.");

    let bag = Bag::from(file_path)?;
    println!("{:?}", bag.file_path);

    Ok(())
}

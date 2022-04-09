use std::io;

use clap::Parser;

use frost::Bag;

#[derive(Parser, Debug)]
#[clap(version)]
struct Args {
    file_path: String,
}

fn max_data_type_len(bag: &Bag) -> usize {
    bag.connection_data.values().map(|d| d.data_type.len()).max().unwrap_or(0)
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    let bag = Bag::from(args.file_path)?;
    println!("{:?}", bag.file_path);

    println!("{0: <13}{1}", "path:", bag.file_path.to_string_lossy());
    println!("{0: <13}{1}", "version:", bag.version);
    println!("{0: <13}{1:.2}s", "duration:", bag.duration().as_secs());
    println!("{0: <13}{1}", "start:", bag.start_time().unwrap());
    println!("{0: <13}{1}", "end:", bag.end_time().unwrap());
    println!("{0: <13}{1}", "messages:", bag.message_count());
    println!("{0: <13}{1}", "compression:", "TODO");

    let max_type_len = max_data_type_len(&bag);
    for (i, connection_data) in bag.connection_data.values().enumerate(){
        let col_display = if i == 0 {"types:"} else {""};
        println!("{0: <13}{1: <max_type_len$} [{2}]", col_display, connection_data.data_type, connection_data.md5sum);
    }

    Ok(())
}

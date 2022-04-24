use std::io;
use std::path::PathBuf;

use bpaf::*;

use frost::Bag;

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct Options {
    only_topics: bool,
    file_path: PathBuf,
}

fn make_parser() -> Parser<Options> {
    let only_topics = short('t').long("topics").help("Only print topics").switch();
    let file_path = positional_os("FILE").map(PathBuf::from);
    construct!(Options {
        only_topics,
        file_path
    })
}

fn max_type_len(bag: &Bag) -> usize {
    bag.connection_data
        .values()
        .map(|d| d.data_type.len())
        .max()
        .unwrap_or(0)
}

fn max_topic_len(bag: &Bag) -> usize {
    bag.connection_data
        .values()
        .map(|d| d.topic.len())
        .max()
        .unwrap_or(0)
}

fn print_topics(bag: &Bag) {
    for topic in bag.topics() {
        println!("{topic}");
    }
}

fn print_all(bag: &Bag) {
    println!("{:?}", bag.file_path);

    println!("{0: <13}{1}", "path:", bag.file_path.to_string_lossy());
    println!("{0: <13}{1}", "version:", bag.version);
    println!("{0: <13}{1:.2}s", "duration:", bag.duration().as_secs());
    println!("{0: <13}{1}", "start:", bag.start_time().unwrap());
    println!("{0: <13}{1}", "end:", bag.end_time().unwrap());
    println!("{0: <13}{1}", "messages:", bag.message_count());
    println!("{0: <13}{1}", "compression:", "TODO");

    let max_type_len = max_type_len(&bag);
    for (i, connection_data) in bag.connection_data.values().enumerate() {
        let col_display = if i == 0 { "types:" } else { "" };
        println!(
            "{0: <13}{1: <max_type_len$} [{2}]",
            col_display, connection_data.data_type, connection_data.md5sum
        );
    }

    let max_topic_len = max_topic_len(&bag);
    for (i, connection_data) in bag.connection_data.values().enumerate() {
        let col_display = if i == 0 { "topics:" } else { "" };
        let msg_count = bag
            .index_data
            .get(&connection_data.connection_id)
            .map_or_else(|| 0, |data| data.len());
        println!(
            "{0: <13}{1: <max_topic_len$} {2:>10} msgs : {3}",
            col_display, connection_data.topic, msg_count, connection_data.data_type
        );
    }
}

fn main() -> io::Result<()> {
    let args = Info::default().for_parser(make_parser()).run();

    let bag = Bag::from(args.file_path)?;

    if args.only_topics {
        print_topics(&bag);
    } else {
        print_all(&bag);
    }

    Ok(())
}

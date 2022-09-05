use std::collections::HashSet;
use std::io;
use std::path::PathBuf;

use bpaf::*;

use frost::Bag;

#[derive(Debug, Clone)]
struct InfoOptions {
    only_topics: bool,
    file_path: PathBuf,
}

#[derive(Clone, Debug)]
enum Command {
    Info(InfoOptions),
}

fn make_parser() -> Parser<Command> {
    let only_topics = short('t').long("topics").help("Only print topics").switch();
    let file_path = positional_os("FILE").map(PathBuf::from);
    let info_parser = construct!(InfoOptions {
        only_topics,
        file_path
    });
    let info_options: OptionParser<InfoOptions> = Info::default()
        .descr("Options for frost info")
        .for_parser(info_parser);

    command("info", Some("rosbag information"), info_options).map(Command::Info)
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
    for (i, (data_type, md5sum)) in bag
        .connection_data
        .values()
        .map(|data| (data.data_type.clone(), data.md5sum.clone()))
        .collect::<HashSet<_>>()
        .into_iter()
        .enumerate()
    {
        let col_display = if i == 0 { "types:" } else { "" };
        println!(
            "{0: <13}{1: <max_type_len$} [{2}]",
            col_display, data_type, md5sum
        );
    }

    let max_topic_len = max_topic_len(&bag);
    for (i, (topic, data_type)) in bag.topics_and_types().iter().enumerate() {
        let col_display = if i == 0 { "topics:" } else { "" };
        let msg_count = bag.topic_message_count(topic).unwrap_or(0);
        println!(
            "{0: <13}{1: <max_topic_len$} {2:>10} msgs : {3}",
            col_display, topic, msg_count, data_type
        );
    }
}

fn main() -> io::Result<()> {
    let args = Info::default()
        .descr("An info utility for rosbags")
        .for_parser(make_parser())
        .run();

    match args {
        Command::Info(info_args) => {
            let bag = Bag::from(info_args.file_path)?;

            if info_args.only_topics {
                print_topics(&bag);
            } else {
                print_all(&bag);
            }
        }
    }

    Ok(())
}

use std::collections::HashSet;
use std::path::PathBuf;

use bpaf::*;
use itertools::Itertools;

use frost::errors::Error;
use frost::Bag;

#[derive(Clone, Debug)]
enum Opts {
    TopicOptions { file_path: PathBuf },
    InfoOptions { file_path: PathBuf, use_epoch: bool },
}

fn file_parser() -> impl Parser<PathBuf> {
    positional::<PathBuf>("FILE")
}

fn args() -> Opts {
    let file_path = file_parser();
    let use_epoch = long("epoch").help("Print times as epoch seconds").switch();
    let info_cmd = construct!(Opts::InfoOptions {
        use_epoch,
        file_path,
    })
    .to_options()
    .descr("Print rosbag information")
    .command("info");
    let file_path = file_parser();
    let topics_cmd = construct!(Opts::TopicOptions { file_path })
        .to_options()
        .descr("Print rosbag topics")
        .command("topics");

    let parser = construct!([info_cmd, topics_cmd]);
    parser.to_options().version(env!("CARGO_PKG_VERSION")).run()
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
    for topic in bag.topics().into_iter().sorted() {
        println!("{topic}");
    }
}

fn print_all(bag: &Bag, use_epoch: bool) {
    let start_time = bag.start_time().unwrap();
    let end_time = bag.end_time().unwrap();

    println!("{0: <13}{1}", "path:", bag.file_path.to_string_lossy());
    println!("{0: <13}{1}", "version:", bag.version);
    println!("{0: <13}{1:.2}s", "duration:", bag.duration().as_secs());
    println!(
        "{0: <13}{1}",
        "start:",
        if use_epoch {
            f32::from(start_time).to_string()
        } else {
            start_time.as_datetime().to_string()
        }
    );
    println!(
        "{0: <13}{1}",
        "end:",
        if use_epoch {
            f32::from(end_time).to_string()
        } else {
            end_time.as_datetime().to_string()
        }
    );
    println!("{0: <13}{1}", "messages:", bag.message_count());
    println!("{0: <13}{1}", "compression:", "TODO");

    let max_type_len = max_type_len(&bag);
    for (i, (data_type, md5sum)) in bag
        .connection_data
        .values()
        .map(|data| (data.data_type.clone(), data.md5sum.clone()))
        .collect::<HashSet<_>>()
        .into_iter()
        .sorted_by(|a, b| Ord::cmp(&a.0, &b.0))
        .enumerate()
    {
        let col_display = if i == 0 { "types:" } else { "" };
        println!(
            "{0: <13}{1: <max_type_len$} [{2}]",
            col_display, data_type, md5sum
        );
    }

    let max_topic_len = max_topic_len(&bag);
    for (i, (topic, data_type)) in bag
        .topics_and_types()
        .into_iter()
        .sorted_by(|a, b| Ord::cmp(&a.0, &b.0))
        .enumerate()
    {
        let col_display = if i == 0 { "topics:" } else { "" };
        let msg_count = bag.topic_message_count(topic).unwrap_or(0);
        println!(
            "{0: <13}{1: <max_topic_len$} {2:>10} msgs : {3}",
            col_display, topic, msg_count, data_type
        );
    }
}

fn main() -> Result<(), Error> {
    let args = args();

    match args {
        Opts::TopicOptions { file_path } => {
            let bag = Bag::from(file_path)?;
            print_topics(&bag);
        }
        Opts::InfoOptions {
            use_epoch,
            file_path,
        } => {
            let bag = Bag::from(file_path)?;
            print_all(&bag, use_epoch);
        }
    }

    Ok(())
}

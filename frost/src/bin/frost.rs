use std::collections::HashSet;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use bpaf::*;
use itertools::Itertools;

use frost::errors::Error;
use frost::Bag;

#[derive(Clone, Debug)]
enum Opts {
    TopicOptions { file_path: PathBuf },
    InfoOptions { file_path: PathBuf },
}

fn file_parser() -> impl Parser<PathBuf> {
    positional::<PathBuf>("FILE")
}

fn args() -> Opts {
    let file_path = file_parser();
    let info_cmd = construct!(Opts::InfoOptions { file_path })
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

fn print_topics(bag: &Bag, writer: &mut impl Write) -> Result<(), Error> {
    for topic in bag.topics().into_iter().sorted() {
        writer.write_all(format!("{topic}\n").as_bytes())?
    }
    Ok(())
}

fn print_all(bag: &Bag, writer: &mut impl Write) -> Result<(), Error> {
    let start_time = bag.start_time().expect("Bag does not have a start time");
    let end_time = bag.end_time().expect("Bag does not have a end time");

    writer
        .write_all(format!("{0: <13}{1}\n", "path:", bag.file_path.to_string_lossy()).as_bytes())?;
    writer.write_all(format!("{0: <13}{1}\n", "version:", bag.version).as_bytes())?;
    writer.write_all(
        format!("{0: <13}{1:.2}s\n", "duration:", bag.duration().as_secs()).as_bytes(),
    )?;
    writer.write_all(
        format!(
            "{0: <13}{1} ({2:.6})\n",
            "start:",
            start_time.as_datetime(),
            f64::from(start_time)
        )
        .as_bytes(),
    )?;
    writer.write_all(
        format!(
            "{0: <13}{1} ({2:.6})\n",
            "end:",
            end_time.as_datetime(),
            f64::from(end_time)
        )
        .as_bytes(),
    )?;
    writer.write_all(format!("{0: <13}{1}\n", "messages:", bag.message_count()).as_bytes())?;
    writer.write_all(format!("{0: <13}TODO\n", "compression:").as_bytes())?;

    let max_type_len = max_type_len(bag);
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
        writer.write_all(
            format!(
                "{0: <13}{1: <max_type_len$} [{2}]\n",
                col_display, data_type, md5sum
            )
            .as_bytes(),
        )?;
    }

    let max_topic_len = max_topic_len(bag);
    for (i, (topic, data_type)) in bag
        .topics_and_types()
        .into_iter()
        .sorted_by(|a, b| Ord::cmp(&a.0, &b.0))
        .enumerate()
    {
        let col_display = if i == 0 { "topics:" } else { "" };
        let msg_count = bag.topic_message_count(topic).unwrap_or(0);
        writer.write_all(
            format!(
                "{0: <13}{1: <max_topic_len$} {2:>10} msgs : {3}\n",
                col_display, topic, msg_count, data_type
            )
            .as_bytes(),
        )?;
    }
    Ok(())
}

fn main() -> Result<(), Error> {
    let args = args();

    let stdout = std::io::stdout();
    let lock = stdout.lock();
    let mut writer = BufWriter::new(lock);

    match args {
        Opts::TopicOptions { file_path } => {
            let bag = Bag::from(file_path)?;
            print_topics(&bag, &mut writer)
        }
        Opts::InfoOptions { file_path } => {
            let bag = Bag::from(file_path)?;
            print_all(&bag, &mut writer)
        }
    }
}

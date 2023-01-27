use std::collections::HashSet;
use std::io::{BufWriter, Read, Seek, Write};
use std::path::PathBuf;

use bpaf::*;
use itertools::Itertools;

use frost::errors::Error;
use frost::Bag;

#[derive(Clone, Debug)]
enum Opts {
    TopicOptions { file_path: PathBuf },
    TypeOptions { file_path: PathBuf },
    InfoOptions { minimal: bool, file_path: PathBuf },
}

fn file_parser() -> impl Parser<PathBuf> {
    positional::<PathBuf>("FILE").complete_shell(ShellComp::File { mask: None })
}

fn args() -> Opts {
    let file_path = file_parser();
    let minimal = short('m')
        .long("minimal")
        .help("Show minimal info (no topics)")
        .switch();
    let info_cmd = construct!(Opts::InfoOptions { minimal, file_path })
        .to_options()
        .descr("Print rosbag information")
        .command("info");
    let file_path = file_parser();
    let topics_cmd = construct!(Opts::TopicOptions { file_path })
        .to_options()
        .descr("Print rosbag topics")
        .command("topics");
    let file_path = file_parser();
    let types_cmd = construct!(Opts::TypeOptions { file_path })
        .to_options()
        .descr("Print rosbag types")
        .command("types");
    let parser = construct!([info_cmd, topics_cmd, types_cmd]);
    parser.to_options().version(env!("CARGO_PKG_VERSION")).run()
}

fn max_type_len(bag: &Bag<impl Read + Seek>) -> usize {
    bag.connection_data
        .values()
        .map(|d| d.data_type.len())
        .max()
        .unwrap_or(0)
}

fn max_topic_len(bag: &Bag<impl Read + Seek>) -> usize {
    bag.connection_data
        .values()
        .map(|d| d.topic.len())
        .max()
        .unwrap_or(0)
}

fn print_topics(bag: &Bag<impl Read + Seek>, writer: &mut impl Write) -> Result<(), Error> {
    for topic in bag.topics().into_iter().sorted() {
        writer.write_all(format!("{topic}\n").as_bytes())?
    }
    Ok(())
}

fn print_types(bag: &Bag<impl Read + Seek>, writer: &mut impl Write) -> Result<(), Error> {
    for topic in bag.types().into_iter().sorted() {
        writer.write_all(format!("{topic}\n").as_bytes())?
    }
    Ok(())
}

fn human_bytes(bytes: u64) -> String {
    let units = ["bytes", "KB", "MB", "GB"];

    let mut unit = units[0];
    let mut remainder = bytes as f64;

    for u in units {
        unit = u;
        if remainder < 1024.0 {
            break;
        }
        remainder = remainder / 1024.0;
    }

    if unit == "bytes" {
        format!("{bytes} bytes")
    } else {
        format!("{remainder:.2} {unit} ({bytes} bytes)")
    }
}

fn print_all(
    bag: &Bag<impl Read + Seek>,
    minimal: bool,
    writer: &mut impl Write,
) -> Result<(), Error> {
    let start_time = bag.start_time().expect("Bag does not have a start time");
    let end_time = bag.end_time().expect("Bag does not have a end time");

    writer.write_all(
        format!(
            "{0: <13}{1}\n",
            "path:",
            bag.file_path
                .as_ref()
                .map_or_else(|| "None".to_string(), |p| p.to_string_lossy().into_owned())
        )
        .as_bytes(),
    )?;
    writer.write_all(format!("{0: <13}{1}\n", "version:", bag.version).as_bytes())?;
    writer.write_all(
        format!("{0: <13}{1:.2}s\n", "duration:", bag.duration().as_secs()).as_bytes(),
    )?;
    writer.write_all(
        format!(
            "{0: <13}{1} ({2:.6})\n",
            "start:",
            start_time.as_datetime().unwrap_or_default(),
            f64::from(start_time)
        )
        .as_bytes(),
    )?;
    writer.write_all(
        format!(
            "{0: <13}{1} ({2:.6})\n",
            "end:",
            end_time.as_datetime().unwrap_or_default(),
            f64::from(end_time)
        )
        .as_bytes(),
    )?;

    writer.write_all(format!("{0: <13}{1}\n", "size:", human_bytes(bag.size)).as_bytes())?;

    writer.write_all(format!("{0: <13}{1}\n", "messages:", bag.message_count()).as_bytes())?;

    let compression_info = bag.compression_info();

    let total_chunks: usize = compression_info.iter().map(|info| info.chunk_count).sum();
    let max_compression_name = compression_info
        .iter()
        .map(|info| info.name.len())
        .max()
        .unwrap_or(0);
    for (i, info) in compression_info.iter().enumerate() {
        let col_display = if i == 0 { "compression:" } else { "" };
        writer.write_all(
            format!(
                "{0: <13}{1: <max_compression_name$} [{2}/{3} chunks; {4:.2}%]\n",
                col_display,
                info.name,
                info.chunk_count,
                total_chunks,
                (100f64 * info.total_compressed as f64 / info.total_uncompressed as f64)
            )
            .as_bytes(),
        )?;
    }

    if minimal {
        return Ok(());
    }

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
        Opts::InfoOptions { minimal, file_path } => {
            let bag = Bag::from(file_path)?;
            print_all(&bag, minimal, &mut writer)
        }
        Opts::TypeOptions { file_path } => {
            let bag = Bag::from(file_path)?;
            print_types(&bag, &mut writer)
        }
    }
}

use bpaf::*;
use std::{collections::HashMap, fs, io, path::PathBuf};
use walkdir::WalkDir;

use serde_derive::{Deserialize, Serialize};
use serde_xml_rs;

#[macro_use]
extern crate lazy_static;

#[derive(Debug)]
enum CodegenError {
    IoError(io::Error),
    XmlError(serde_xml_rs::Error),
}

impl From<io::Error> for CodegenError {
    fn from(err: io::Error) -> CodegenError {
        CodegenError::IoError(err)
    }
}

impl From<serde_xml_rs::Error> for CodegenError {
    fn from(err: serde_xml_rs::Error) -> CodegenError {
        CodegenError::XmlError(err)
    }
}

type Result<T> = std::result::Result<T, CodegenError>;

// TODO: support constants
#[derive(Debug, PartialEq)]
struct Field {
    data_type: String,
    name: String,
    is_array: bool,
}

fn builtin_mappings(data_type: &str) -> Option<&'static str> {
    lazy_static! {
        static ref MAPPING: HashMap<&'static str, &'static str> = vec![
            ("bool", "bool"),
            ("byte", "u8"),
            ("char", "char"),
            ("float32", "f32"),
            ("float64", "f64"),
            ("int8", "i8"),
            ("int16", "i16"),
            ("int32", "i32"),
            ("int64", "i64"),
            ("uint8", "u8"),
            ("uint16", "u16"),
            ("uint32", "u32"),
            ("uint64", "u64"),
            ("string", "String"),
            ("time", "Time"), // Type manually generated
            ("duration", "Duration") // Type manually generated
        ].into_iter().collect();
    }
    MAPPING.get(data_type).map(|s| *s)
}

struct RosMsg {
    name: String,
    fields: Vec<Field>,
}

impl RosMsg {
    fn new(path: &PathBuf) -> Result<Self> {
        todo!()
    }
}

fn convert_line(line: &str) -> Option<Field> {
    let line = line.trim();

    if line.starts_with("#") {
        return None;
    }

    if line.is_empty() {
        None
    } else {
        let mut parts = line.split(" ");

        let mut data_type = parts
            .next()
            .expect(&format!("Expected a data type in {}", &line));

        let is_array = data_type.ends_with("[]");

        if is_array {
            data_type = &data_type[..data_type.len() - 2];
        }

        let data_type = builtin_mappings(&data_type)
            .unwrap_or(&data_type)
            .to_owned();

        let mut name = parts
            .next()
            .expect(&format!("expected a name {}", &line))
            .to_owned();

        if name.contains("=") {
            let mut name_parts = name.split("=");
            name = name_parts.next().unwrap().to_owned();
        } else if name.contains("\"") {
            let mut name_parts = name.split("\"");
            name = name_parts.next().unwrap().to_owned();
        }

        Some(Field {
            data_type,
            name,
            is_array,
        })
    }
}

#[derive(Clone, Debug)]
struct Opts {
    input_path: PathBuf,
    output_path: PathBuf,
}

fn build_parser() -> Parser<Opts> {
    let input_path = short('i')
        .long("input_path")
        .help("Path to a root folder containing ros msg files.")
        .argument_os("INPUT_PATH")
        .map(PathBuf::from);
    let output_path = short('o')
        .long("output_path")
        .help("Path to a folder which will contain generated Rust files.")
        .argument_os("OUTPUT_PATH")
        .map(PathBuf::from);
    construct!(Opts {
        input_path,
        output_path
    })
}

// Helper struct for parsing package.xml
#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Package {
    name: String,
}

fn get_package_name(path: &PathBuf) -> Result<String> {
    let xml_string = fs::read_to_string(&path)?;
    let res: Package = serde_xml_rs::from_str(&xml_string)?;
    Ok(res.name)
}

fn main() -> Result<()> {
    let opts = Info::default().for_parser(build_parser()).run();

    let mut packages = HashMap::<PathBuf, String>::new();
    let mut msgs = HashMap::<String, PathBuf>::new();

    for entry in WalkDir::new(opts.input_path).into_iter() {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let metadata = match entry.metadata() {
            Ok(data) => data,
            Err(_) => continue,
        };

        if !metadata.is_file() {
            continue;
        }

        let abs_path = entry.into_path().canonicalize().unwrap();

        if abs_path.ends_with("package.xml") {
            packages.insert(
                abs_path.parent().unwrap().into(),
                get_package_name(&abs_path)?,
            );
        } else if abs_path.ends_with(".msg") {
        }
    }

    println!("{:?}", packages);

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{convert_line, Field};

    #[test]
    fn test_convert_line() {
        let line = "uint32 data_offset        # padding elements at front of data";
        let expected = Field {
            data_type: "u32".to_owned(),
            name: "data_offset".to_owned(),
            is_array: false,
        };
        assert_eq!(convert_line(line).unwrap(), expected);

        let line = "time stamp";
        let expected = Field {
            data_type: "Time".to_owned(),
            name: "stamp".to_owned(),
            is_array: false,
        };
        assert_eq!(convert_line(line).unwrap(), expected);

        let line = "duration dur";
        let expected = Field {
            data_type: "Duration".to_owned(),
            name: "dur".to_owned(),
            is_array: false,
        };
        assert_eq!(convert_line(line).unwrap(), expected);

        // with constants
        let line = "uint32 data_offset=6 # some comment";
        let expected = Field {
            data_type: "u32".to_owned(),
            name: "data_offset".to_owned(),
            is_array: false,
        };
        assert_eq!(convert_line(line).unwrap(), expected);

        let line = "float32 npi=-3.14# some comment";
        let expected = Field {
            data_type: "f32".to_owned(),
            name: "npi".to_owned(),
            is_array: false,
        };
        assert_eq!(convert_line(line).unwrap(), expected);

        let line = "string FOO=foo";
        let expected = Field {
            data_type: "String".to_owned(),
            name: "FOO".to_owned(),
            is_array: false,
        };
        assert_eq!(convert_line(line).unwrap(), expected);

        let line = "string FOO=\"some comment\" which should be ignored";
        let expected = Field {
            data_type: "String".to_owned(),
            name: "FOO".to_owned(),
            is_array: false,
        };
        assert_eq!(convert_line(line).unwrap(), expected);

        //arrays
        let line = "uint32[] data # some comment";
        let expected = Field {
            data_type: "u32".to_owned(),
            name: "data".to_owned(),
            is_array: true,
        };
        assert_eq!(convert_line(line).unwrap(), expected);

        // empty
        assert_eq!(convert_line("# this is a comment"), None);
        assert_eq!(convert_line(" "), None);
    }
}

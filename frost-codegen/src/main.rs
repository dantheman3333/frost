use bpaf::*;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufWriter, Write};
use std::process::Command;
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{self, BufReader},
    path::PathBuf,
};
use walkdir::WalkDir;

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
            ("string", "std::string::String"),
            ("time", "frost::time::Time"), // Type manually generated
            ("duration", "frost::time::RosDuration") // Type manually generated
        ].into_iter().collect();
    }
    MAPPING.get(data_type).copied()
}

#[derive(Debug)]
struct RosMsg {
    name: String,
    fields: Vec<Field>,
}

impl RosMsg {
    fn new(path: &PathBuf) -> Result<Self> {
        let file = File::open(path)?;
        let lines = BufReader::new(file).lines();
        let fields = lines
            .filter_map(|line| match line {
                Ok(line) => convert_line(&line),
                Err(_) => None,
            })
            .collect::<Vec<Field>>();
        Ok(RosMsg {
            name: path.file_stem().unwrap().to_string_lossy().into_owned(),
            fields,
        })
    }

    fn as_struct_definition(&self) -> String {
        let mut buf = String::new();
        buf.push_str(&format!("pub struct {} {{", &self.name));

        self.fields.iter().for_each(|field| {
            buf.push_str("pub ");
            buf.push_str(&field.name);
            buf.push_str(": ");
            if field.is_array {
                buf.push_str(&format!("Vec<{}>", field.data_type));
            } else {
                buf.push_str(&field.data_type);
            }
            buf.push(',')
        });

        buf.push('}');

        buf
    }
}

fn convert_line(line: &str) -> Option<Field> {
    let line = line.trim();

    if line.starts_with('#') {
        return None;
    }

    if line.is_empty() {
        None
    } else {
        let mut parts = line.split_whitespace();

        let mut data_type = parts
            .next()
            .unwrap_or_else(|| panic!("Expected a data type in {}", &line));

        let is_array = data_type.ends_with("[]");

        if is_array {
            data_type = &data_type[..data_type.len() - 2];
        }

        let data_type = builtin_mappings(data_type).unwrap_or(data_type).to_owned();

        let mut name = parts
            .next()
            .unwrap_or_else(|| panic!("expected a name {}", &line))
            .to_owned();

        if name.contains('=') {
            let mut name_parts = name.split('=');
            name = name_parts.next().unwrap().to_owned();
        } else if name.contains('\"') {
            let mut name_parts = name.split('\"');
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

fn build_parser() -> impl Parser<Opts> {
    let input_path = short('i')
        .long("input_path")
        .help("Path to a root folder containing ros msg files.")
        .argument::<PathBuf>("INPUT_PATH");

    let output_path = short('o')
        .long("output_path")
        .help("Path to a folder which will contain generated Rust files.")
        .argument::<PathBuf>("OUTPUT_PATH");

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
    let xml_string = fs::read_to_string(path)?;
    let res: Package = serde_xml_rs::from_str(&xml_string)?;
    Ok(res.name)
}

fn fmt_file(path: &PathBuf) -> Result<()> {
    let mut fmt_cmd = Command::new("rustfmt");
    fmt_cmd.arg(path).output()?;
    Ok(())
}

fn write_all(
    out_path: &PathBuf,
    mods: HashMap<String, String>,
    msgs: Vec<(PathBuf, RosMsg)>,
) -> Result<()> {
    let file = File::create(out_path)?;
    let mut writer = BufWriter::new(file);

    writer.write_all(b"#![allow(clippy::all)]\n")?;

    writer.write_all(b"/// This file is autogenerated. Do not edit by hand!\n")?;

    writer.write_all(b"pub mod msgs {")?;

    for package in mods.values() {
        writer.write_all(format!("pub mod {package} {{").as_bytes())?;

        // TODO: redo msgs structure, don't keep looping
        for (msg_path, msg) in msgs.iter() {
            let parent_path = msg_path.parent().unwrap();
            if parent_path.file_stem().unwrap() != "msg" {
                continue;
            }
            let parent_path = parent_path.parent().unwrap();
            match mods.get(parent_path.to_str().unwrap_or("")) {
                Some(cur_package) => {
                    if package != cur_package {
                        continue;
                    }
                    writer.write_all(
                        "#[derive(Clone, Debug, serde::Deserialize, PartialEq)]".as_bytes(),
                    )?;
                    writer.write_all(msg.as_struct_definition().as_bytes())?;
                    write!(writer, "impl frost::msgs::Msg for {} {{}}", msg.name)?;
                }
                None => println!("WARN: missing package for {}", &msg_path.to_string_lossy()),
            };
        }

        writer.write_all("}".as_bytes())?;
    }

    writer.write_all("}".as_bytes())?;

    Ok(())
}

fn get_mods_and_msgs(
    input_path: &PathBuf,
) -> Result<(HashMap<String, String>, Vec<(PathBuf, RosMsg)>)> {
    let mut packages = HashMap::<String, String>::new();
    let mut msgs = Vec::<(PathBuf, RosMsg)>::new();

    for entry in WalkDir::new(input_path).into_iter() {
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

        if abs_path.extension().is_none() {
            continue;
        }

        if abs_path.file_name().unwrap() == "package.xml" {
            let Ok(package_name) = get_package_name(&abs_path) else {
                continue
            };
            packages.insert(
                abs_path.parent().unwrap().to_string_lossy().into_owned(),
                package_name,
            );
        } else if abs_path.extension().unwrap() == "msg" {
            let msg = RosMsg::new(&abs_path)?;
            msgs.push((abs_path, msg));
        }
    }
    Ok((packages, msgs))
}

fn main() -> Result<()> {
    let opts = build_parser().to_options().run();

    let (mods, msgs) = get_mods_and_msgs(&opts.input_path)?;

    write_all(&opts.output_path, mods, msgs)?;
    fmt_file(&opts.output_path)?;
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
            data_type: "frost::time::Time".to_owned(),
            name: "stamp".to_owned(),
            is_array: false,
        };
        assert_eq!(convert_line(line).unwrap(), expected);

        let line = "duration dur";
        let expected = Field {
            data_type: "frost::time::RosDuration".to_owned(),
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
            data_type: "std::string::String".to_owned(),
            name: "FOO".to_owned(),
            is_array: false,
        };
        assert_eq!(convert_line(line).unwrap(), expected);

        let line = "string FOO=\"some comment\" which should be ignored";
        let expected = Field {
            data_type: "std::string::String".to_owned(),
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

        let line = "int64[]           data          # array of data";
        let expected = Field {
            data_type: "i64".to_owned(),
            name: "data".to_owned(),
            is_array: true,
        };
        assert_eq!(convert_line(line).unwrap(), expected);

        // empty
        assert_eq!(convert_line("# this is a comment"), None);
        assert_eq!(convert_line(" "), None);
    }
}

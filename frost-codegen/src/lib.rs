use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::env;
use std::io::{BufWriter, Write};
use std::process::Command;
use std::{
    collections::HashMap,
    fs::{self, File},
    path::PathBuf,
};
use walkdir::WalkDir;

pub mod errors;
use errors::Error;
mod parsing;
use parsing::{parse, Statement};

#[macro_use]
extern crate lazy_static;

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
            ("Header", "crate::msgs::std_msgs::Header"),
            ("time", "frost::time::Time"), // Type manually generated
            ("duration", "frost::time::RosDuration") // Type manually generated
        ].into_iter().collect();
    }
    MAPPING.get(data_type).copied()
}

#[derive(Debug)]
struct RosMsg {
    name: String,
    statements: Vec<Statement>,
}

impl RosMsg {
    fn new(path: &PathBuf) -> Result<Self, Error> {
        let text = fs::read_to_string(path)?;

        Ok(RosMsg {
            name: path.file_stem().unwrap().to_string_lossy().into_owned(),
            statements: parse(&text)?,
        })
    }

    fn as_struct_definition(&self) -> String {
        if self.statements.is_empty() && self.name != "Empty" {
            println!("WARN: {} is has no fields (parsed incorrectly?)", self.name);
        }

        let mut buf = String::new();
        buf.push_str(&format!("pub struct {} {{", &self.name));

        self.statements
            .iter()
            .filter(|stmt| matches!(stmt, Statement::Field { .. }))
            .for_each(|stmt| {
                if let Statement::Field { msg_type, name } = stmt {
                    let full_type_name = match &msg_type.package_name {
                        Some(package_name) => {
                            "crate::msgs::".to_owned() + &package_name + "::" + &msg_type.name
                        }
                        None => builtin_mappings(&msg_type.name)
                            .unwrap_or(&msg_type.name)
                            .to_owned(),
                    };
                    if let Some(size) = msg_type.array_size {
                        if size > 32 {
                            buf.push_str("#[serde(with = \"serde_big_array::BigArray\")]");
                        }
                    }
                    buf.push_str("pub r#"); // use raw identifiers just in case
                    buf.push_str(name);
                    buf.push_str(": ");
                    if msg_type.is_array {
                        match msg_type.array_size {
                            Some(size) => {
                                buf.push_str(&format!("[{}; {}]", &full_type_name, size));
                            }
                            None => buf.push_str(&format!("Vec<{}>", &full_type_name)),
                        }
                    } else {
                        buf.push_str(&full_type_name);
                    }
                    buf.push(',')
                }
            });

        buf.push('}'); // end struct

        // constants, if any
        buf.push_str(&format!("impl {} {{", &self.name));

        self.statements.iter().for_each(|stmt| {
            if let Statement::Constant {
                msg_type,
                name,
                value,
            } = stmt
            {
                let full_type_name = match &msg_type.package_name {
                    Some(package_name) => package_name.clone() + "::" + &msg_type.name,
                    None => builtin_mappings(&msg_type.name)
                        .unwrap_or(&msg_type.name)
                        .to_owned(),
                };
                buf.push_str("pub const r#"); // use raw identifiers just in case
                buf.push_str(name);
                buf.push_str(": ");

                if &msg_type.name.to_lowercase() == "string" {
                    buf.push_str("&'static str")
                } else {
                    buf.push_str(&full_type_name);
                }
                buf.push('=');

                match msg_type.name.to_lowercase().as_ref() {
                    "string" => {
                        buf.push('&');
                        buf.push('"');
                        buf.push_str(&value.replace("\"", "\\\""));
                        buf.push('"')
                    }
                    "bool" => {
                        if value == "1" {
                            buf.push_str("true");
                        } else {
                            buf.push_str("false");
                        }
                    }
                    "float32" | "float64" => {
                        buf.push_str(value);
                        if !value.contains('.') {
                            buf.push('.');
                        }
                    }
                    _ => {
                        buf.push_str(value);
                    }
                }
                buf.push(';')
            }
        });
        buf.push('}'); // end impl

        buf
    }
}

#[derive(Clone, Debug)]
pub struct Opts {
    pub input_paths: Vec<PathBuf>,
    pub output_path: PathBuf,
}

// Helper struct for parsing package.xml
#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Package {
    name: String,
}

fn get_package_name(path: &PathBuf) -> Result<String, Error> {
    let xml_string = fs::read_to_string(path)?;
    let res: Package = serde_xml_rs::from_str(&xml_string)?;
    Ok(res.name)
}

fn fmt_file(path: &PathBuf) -> Result<(), Error> {
    let rustfmt_path = env::var("RUSTFMT_PATH").unwrap_or("rustfmt".into());
    let mut fmt_cmd = Command::new(&rustfmt_path);
    fmt_cmd.arg(path).output()?;
    Ok(())
}

fn write_all(
    out_path: &PathBuf,
    mods: BTreeMap<String, String>,
    msgs: Vec<(PathBuf, RosMsg)>,
) -> Result<(), Error> {
    let file = File::create(out_path)?;
    let mut writer = BufWriter::new(file);

    writer.write_all(b"#[allow(unknown_lints)]\n")?;
    writer.write_all(b"#[allow(clippy::all)]\n")?;
    writer.write_all(b"#[allow(dead_code)]\n")?;
    writer.write_all(b"#[allow(missing_docs)]\n")?;
    writer.write_all(b"#[allow(non_camel_case_types)]\n")?;
    writer.write_all(b"#[allow(non_snake_case)]\n")?;
    writer.write_all(b"#[allow(non_upper_case_globals)]\n")?;

    writer.write_all(b"/// This file is autogenerated. Do not edit by hand!\n")?;

    writer.write_all(b"pub mod msgs {")?;

    let mut seen = HashSet::new();

    for package in mods.values() {
        if seen.contains(package) {
            continue;
        }
        seen.insert(package.clone());

        let mut msgs_in_package = Vec::new();

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
                    msgs_in_package.push(msg);
                }
                None => println!("WARN: missing package for {}", &msg_path.to_string_lossy()),
            };
        }

        if msgs_in_package.is_empty() {
            continue;
        }

        writer.write_all(format!("pub mod r#{package} {{").as_bytes())?;

        for msg in msgs_in_package {
            writer
                .write_all("#[derive(Clone, Debug, serde::Deserialize, PartialEq)]".as_bytes())?;
            writer.write_all(msg.as_struct_definition().as_bytes())?;
            write!(writer, "impl frost::msgs::Msg for {} {{}}", msg.name)?;
        }

        writer.write_all("}".as_bytes())?;
    }

    writer.write_all("}".as_bytes())?;

    Ok(())
}

fn get_mods_and_msgs(
    input_paths: &[PathBuf],
) -> Result<(BTreeMap<String, String>, Vec<(PathBuf, RosMsg)>), Error> {
    let mut packages = BTreeMap::<String, String>::new();
    let mut msgs = Vec::<(PathBuf, RosMsg)>::new();

    input_paths.iter().for_each(|input_path| {
        if !input_path.exists() {
            panic!("{} directory not found", input_path.to_string_lossy());
        }
        for entry in WalkDir::new(input_path).into_iter() {
            let Ok(entry) = entry else {
                continue;
            };

            let Ok(metadata) = entry.metadata() else {
                continue;
            };

            if !metadata.is_file() {
                continue;
            }

            let abs_path = entry.into_path().canonicalize().unwrap();

            let Some(extension) = abs_path.extension() else {
                continue;
            };

            if abs_path.file_name().unwrap() == "package.xml" {
                let Ok(package_name) = get_package_name(&abs_path) else {
                    continue;
                };
                packages.insert(
                    abs_path.parent().unwrap().to_string_lossy().into_owned(),
                    package_name,
                );
            } else if extension == "msg" {
                let msg = RosMsg::new(&abs_path).unwrap();
                msgs.push((abs_path, msg));
            }
        }
    });
    msgs.sort_by(|(_, a_msg), (_, b_msg)| a_msg.name.cmp(&b_msg.name));
    Ok((packages, msgs))
}

pub fn run(opts: Opts) -> Result<(), Error> {
    let (mods, msgs) = get_mods_and_msgs(&opts.input_paths)?;

    println!("Found {} message definitions", msgs.len());

    write_all(&opts.output_path, mods, msgs)?;
    fmt_file(&opts.output_path)
}

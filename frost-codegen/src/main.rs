use std::collections::HashMap;

#[macro_use]
extern crate lazy_static;

#[derive(Debug, PartialEq)]
struct Field {
    data_type: String,
    name: String,
    constant: Option<String>,
    is_array: bool
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


fn convert_line(line: &str) -> Option<Field> {
    let line = line.trim();

    if line.starts_with("#"){
        return None
    }

    if line.is_empty() {
        None
    } else {
        let mut parts = line.split(" ");

        let mut data_type = parts.next().expect(&format!("Expected a data type in {}", &line));

        let is_array = data_type.ends_with("[]");

        if is_array {
            data_type = &data_type[..data_type.len()-2];
        }

        let data_type = builtin_mappings(&data_type).unwrap_or(&data_type).to_owned();

        let name = parts.next().expect(&format!("expected a name {}", line)).to_owned();

        if name.contains("="){
            let mut name_parts = name.split("=");
            let name = name_parts.next().unwrap().to_owned();

            if data_type == "String" {
                // strings are special-cased, take everything as the value, trimmed
                let constant = line.split("=").nth(1).expect(&format!("expected a value for the assignment {}", line)).trim().to_owned();
                Some(Field{data_type, name, constant: Some(constant), is_array})
            }else{
                let constant = name_parts.next().expect(&format!("expected a value for the assignment {}", line)).to_owned();
                let end = constant.find('#').unwrap_or(constant.len());
                let constant = constant[..end].to_owned();

                Some(Field{data_type, name, constant: Some(constant), is_array})
            }
        }else{
            Some(Field{data_type, name, constant: None, is_array})
        }
    }
}

fn main() {
    println!("Hello, world!");
}

//
#[cfg(test)]
mod tests {
    use crate::{Field, convert_line};

    #[test]
    fn test_convert_line(){
        let line = "uint32 data_offset        # padding elements at front of data";
        let expected = Field{
            data_type: "u32".to_owned(),
            name: "data_offset".to_owned(),
            constant: None,
            is_array: false
        };
        assert_eq!(convert_line(line).unwrap(), expected);

        let line = "time stamp";
        let expected = Field{
            data_type: "Time".to_owned(),
            name: "stamp".to_owned(),
            constant: None,
            is_array: false
        };
        assert_eq!(convert_line(line).unwrap(), expected);

        let line = "duration dur";
        let expected = Field{
            data_type: "Duration".to_owned(),
            name: "dur".to_owned(),
            constant: None,
            is_array: false
        };
        assert_eq!(convert_line(line).unwrap(), expected);

        // with constants
        let line = "uint32 data_offset=6 # some comment";
        let expected = Field{
            data_type: "u32".to_owned(),
            name: "data_offset".to_owned(),
            constant: Some("6".to_owned()),
            is_array: false
        };
        assert_eq!(convert_line(line).unwrap(), expected);

        let line = "float32 npi=-3.14# some comment";
        let expected = Field{
            data_type: "f32".to_owned(),
            name: "npi".to_owned(),
            constant: Some("-3.14".to_owned()),
            is_array: false
        };
        assert_eq!(convert_line(line).unwrap(), expected);

        let line = "string FOO=foo";
        let expected = Field{
            data_type: "String".to_owned(),
            name: "FOO".to_owned(),
            constant: Some("foo".to_owned()),
            is_array: false
        };
        assert_eq!(convert_line(line).unwrap(), expected);

        let line = "string FOO=\"some comment\" which should be ignored";
        let expected = Field{
            data_type: "String".to_owned(),
            name: "FOO".to_owned(),
            constant: Some("\"some comment\" which should be ignored".to_owned()),
            is_array: false
        };
        assert_eq!(convert_line(line).unwrap(), expected);

        //arrays
        let line = "uint32[] data # some comment";
        let expected = Field{
            data_type: "u32".to_owned(),
            name: "data".to_owned(),
            constant: None,
            is_array: true
        };
        assert_eq!(convert_line(line).unwrap(), expected);

        // empty
        assert_eq!(convert_line("# this is a comment"), None);
        assert_eq!(convert_line(" "), None);
    }
}

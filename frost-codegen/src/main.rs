use std::collections::HashMap;

#[macro_use]
extern crate lazy_static;

struct Field {
    data_type: String,
    name: String,
    constant: String,
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
            ("time", "Time"), // Type manually generated
            ("duration", "Duration") // Type manually generated
        ].into_iter().collect();
    }
    MAPPING.get(data_type).map(|s| *s)
}

fn remove_comments(line: &str) -> &str {
    let comment_pos = line.find('#').unwrap_or(line.len());
    &line[0..comment_pos]
}

fn convert_line(line: &str) -> Option<Field> {
    let comment_pos = line.find('#').unwrap_or(line.len());
    let line = line[0..comment_pos].trim();

    if line.is_empty() {
        None
    } else {
        let mut parts = line.split(" ");

        let data_type = parts.next().expect(&format!("Expected a data type in {}", &line));
        todo!()
    }
}

fn main() {
    println!("Hello, world!");
}

//
#[cfg(test)]
mod tests {
    use crate::remove_comments;

    #[test]
    fn test_remove_comments() {
        let line = "uint32 data_offset        # padding elements at front of data";
        assert_eq!(remove_comments(line), "uint32 data_offset        ");

        let line = "# one big comment";
        assert_eq!(remove_comments(line), "");

        let line = "float32 data";
        assert_eq!(remove_comments(line), line)
    }
}

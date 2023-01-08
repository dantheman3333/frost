#![allow(dead_code)]
use chumsky::prelude::*;

#[derive(Debug, PartialEq)]
pub(crate) struct Type {
    pub(crate) package_name: Option<String>,
    pub(crate) name: String,
    pub(crate) is_array: bool,
    pub(crate) array_size: Option<usize>,
}

pub(crate) struct Field {
    pub(crate) msg_type: Type,
    pub(crate) name: String,
}

#[derive(Debug, PartialEq)]
pub(crate) enum Statement {
    Field {
        msg_type: Type,
        name: String,
    },
    Constant {
        msg_type: Type,
        name: String,
        value: String,
    },
}

impl Statement {
    pub fn get_type(&self) -> &Type {
        match self {
            Statement::Field { msg_type, .. } => msg_type,
            Statement::Constant { msg_type, .. } => msg_type,
        }
    }
    pub fn get_name(&self) -> &str {
        match self {
            Statement::Field { name, .. } => name,
            Statement::Constant { name, .. } => name,
        }
    }
}

fn parser() -> impl Parser<char, Vec<Statement>, Error = Simple<char>> {
    let comment = just("#").then(take_until(just('\n'))).padded();

    let package = text::ident()
        .then_ignore(just("/"))
        .labelled("package")
        .or_not();

    let type_name = package
        .then(text::ident())
        .then(
            text::digits(10)
                .or_not()
                .delimited_by(just("["), just("]"))
                .or_not(),
        )
        .padded()
        .labelled("msg type")
        .map(|((package, type_name), array)| Type {
            package_name: package,
            name: type_name,
            is_array: array.is_some(),
            array_size: array
                .flatten()
                .map(|digits: String| digits.parse().unwrap_or_default()),
        });

    let name = type_name
        .then(text::ident())
        .then_ignore(just('='))
        .then(take_until(just('\n')))
        .padded()
        .map(|((msg_type, name), value)| Statement::Constant {
            msg_type,
            name,
            value: value.0.into_iter().collect::<String>().trim().to_owned(),
        })
        .or(type_name
            .then(text::ident())
            .padded()
            .map(|(msg_type, value)| Statement::Field {
                msg_type,
                name: value,
            }));

    name.padded_by(comment.repeated().or_not()).repeated()
}

pub(crate) fn parse(text: &str) -> Result<Vec<Statement>, Vec<Simple<char>>> {
    parser().parse(text)
}

#[cfg(test)]
mod tests {
    use super::{parse, Statement, Type};

    #[test]
    fn test_parse_field() {
        let text = r#"# this is a comment
        #another comment 
        time start#comment
        # comment comment
        uint32 world3
        "#;

        let actual = parse(text).unwrap();

        let expected = vec![
            Statement::Field {
                msg_type: Type {
                    package_name: None,
                    name: "time".into(),
                    is_array: false,
                    array_size: None,
                },
                name: "start".into(),
            },
            Statement::Field {
                msg_type: Type {
                    package_name: None,
                    name: "uint32".into(),
                    is_array: false,
                    array_size: None,
                },
                name: "world3".into(),
            },
        ];

        assert_eq!(expected, actual)
    }

    #[test]
    fn test_parse_array() {
        let text = r#"# this is a comment
        #another comment 
        uint8[] numbers
        uint16[10] ten_numbers
        # comment comment
        
        "#;

        let actual = parse(text).unwrap();

        let expected = vec![
            Statement::Field {
                msg_type: Type {
                    package_name: None,
                    name: "uint8".into(),
                    is_array: true,
                    array_size: None,
                },
                name: "numbers".into(),
            },
            Statement::Field {
                msg_type: Type {
                    package_name: None,
                    name: "uint16".into(),
                    is_array: true,
                    array_size: Some(10),
                },
                name: "ten_numbers".into(),
            },
        ];

        assert_eq!(expected, actual)
    }

    #[test]
    fn test_parse_package() {
        let text = r#"custom_pkg/SomeMsg data
        custom_pkg/SomeMsgArr[255] arr"
        "#;

        let actual = parse(text).unwrap();

        let expected = vec![
            Statement::Field {
                msg_type: Type {
                    package_name: Some("custom_pkg".into()),
                    name: "SomeMsg".into(),
                    is_array: false,
                    array_size: None,
                },
                name: "data".into(),
            },
            Statement::Field {
                msg_type: Type {
                    package_name: Some("custom_pkg".into()),
                    name: "SomeMsgArr".into(),
                    is_array: true,
                    array_size: Some(255),
                },
                name: "arr".into(),
            },
        ];

        assert_eq!(expected, actual)
    }

    #[test]
    fn test_parse_constant() {
        let text = r##"custom_pkg/SomeMsg data=bar
        int32 Y=-123
        string EXAMPLE="#comments" are ignored, and leading and trailing whitespace removed   
        "##;

        let actual = parse(text).unwrap();

        let expected = vec![
            Statement::Constant {
                msg_type: Type {
                    package_name: Some("custom_pkg".into()),
                    name: "SomeMsg".into(),
                    is_array: false,
                    array_size: None,
                },
                name: "data".into(),
                value: "bar".into(),
            },
            Statement::Constant {
                msg_type: Type {
                    package_name: None,
                    name: "int32".into(),
                    is_array: false,
                    array_size: None,
                },
                name: "Y".into(),
                value: "-123".into(),
            },
            Statement::Constant {
                msg_type: Type {
                    package_name: None,
                    name: "string".into(),
                    is_array: false,
                    array_size: None,
                },
                name: "EXAMPLE".into(),
                value: "\"#comments\" are ignored, and leading and trailing whitespace removed"
                    .into(),
            },
        ];

        assert_eq!(expected, actual)
    }
}

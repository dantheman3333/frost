use std::{error, fmt, io};

#[derive(Debug)]
pub enum Error {
    IoError(io::Error),
    XmlError(serde_xml_rs::Error),
    ParserError(Vec<chumsky::prelude::Simple<char>>),
}

impl error::Error for Error {}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::IoError(err)
    }
}

impl From<serde_xml_rs::Error> for Error {
    fn from(err: serde_xml_rs::Error) -> Error {
        Error::XmlError(err)
    }
}

impl From<Vec<chumsky::prelude::Simple<char>>> for Error {
    fn from(err: Vec<chumsky::prelude::Simple<char>>) -> Error {
        Error::ParserError(err)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::IoError(e) => e.fmt(f),
            Error::XmlError(e) => e.fmt(f),
            Error::ParserError(errors) => {
                for e in errors {
                    e.fmt(f)?
                }
                Ok(())
            }
        }
    }
}

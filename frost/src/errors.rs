use std::error;
use std::fmt;
use std::io;

use crate::OpCode;

#[derive(Debug)]
pub struct FrostError {
    kind: FrostErrorKind,
}

impl FrostError {
    pub(crate) fn new(kind: FrostErrorKind) -> FrostError {
        FrostError { kind }
    }
}
#[derive(Debug)]
pub enum FrostErrorKind {
    NotARosbag(String),
    UnindexedBag,
    InvalidOpCode(u8),
    InvalidHeaderOp(OpCode),
    MissingHeaderOp,
    InvalidBag(&'static str),
    MissingFieldSeparator(Vec<u8>),
    InvalidRecordOp {
        expected: u8,
        actual: u8,
    },
    InvalidRecordField {
        record_name: &'static str,
        field: String,
    },
    MissingRecordField {
        record_name: &'static str,
        field: &'static str,
    },
    MissingRecords {
        record_name: &'static str,
        expected: usize,
        actual: usize,
    },
    Io(io::Error),
}

impl fmt::Display for FrostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            FrostErrorKind::NotARosbag(ref s) => {
                write!(f, "invalid rosbag v2 header '{s}'")
            }
            FrostErrorKind::Io(ref e) => e.fmt(f),
            FrostErrorKind::InvalidRecordOp { expected, actual } => {
                write!(
                    f,
                    "Expected op '{:#04x}', found '{:#04x}'",
                    expected, actual
                )
            }
            FrostErrorKind::InvalidRecordField {
                record_name,
                ref field,
            } => {
                write!(f, "Unexpected field '{field}' in '{record_name}'")
            }
            FrostErrorKind::MissingRecordField { record_name, field } => {
                write!(f, "Missing field '{field}' in '{record_name}'")
            }
            FrostErrorKind::UnindexedBag => write!(f, "Unindexed bag"),
            FrostErrorKind::InvalidHeaderOp(ref op) => {
                write!(f, "Invalid op code in header '{:#04x}'", *op as u8)
            }
            FrostErrorKind::MissingRecords {
                record_name,
                expected,
                actual,
            } => {
                write!(f, "Expected '{expected}' '{record_name}', found '{actual}'")
            }
            FrostErrorKind::InvalidOpCode(op) => write!(f, "Unknown OpCode '{:#04x}'", op),
            FrostErrorKind::MissingHeaderOp => write!(f, "Header is missing OpCode"),
            FrostErrorKind::MissingFieldSeparator(ref buf) => {
                write!(f, "Buffer is missing the field separator: {:?}", buf)
            }
            FrostErrorKind::InvalidBag(s) => write!(f, "Invalid bag: {s}"),
        }
    }
}

impl error::Error for FrostError {}

impl From<io::Error> for FrostError {
    fn from(e: io::Error) -> FrostError {
        FrostError {
            kind: FrostErrorKind::Io(e),
        }
    }
}

use std::borrow::Cow;
use std::error;
use std::fmt;
use std::io;

#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
}

impl Error {
    pub(crate) fn new(kind: ErrorKind) -> Error {
        Error { kind }
    }
    pub fn kind(&self) -> &ErrorKind {
        &self.kind
    }
}
#[derive(Debug)]
pub enum ErrorKind {
    NotARosbag,
    UnindexedBag,
    InvalidBag(Cow<'static, str>),
    Deserialization(serde_rosmsg::Error),
    Decompression(lz4_flex::block::DecompressError),
    Io(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            ErrorKind::NotARosbag => {
                write!(f, "invalid rosbag v2 header")
            }
            ErrorKind::Io(ref e) => e.fmt(f),
            ErrorKind::UnindexedBag => write!(f, "unindexed bag"),
            ErrorKind::InvalidBag(ref cow) => write!(f, "invalid bag: {cow}"),
            ErrorKind::Deserialization(ref e) => e.fmt(f),
            ErrorKind::Decompression(ref e) => e.fmt(f),
        }
    }
}

impl error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Error {
        Error {
            kind: ErrorKind::Io(e),
        }
    }
}

impl From<serde_rosmsg::Error> for Error {
    fn from(e: serde_rosmsg::Error) -> Error {
        Error {
            kind: ErrorKind::Deserialization(e),
        }
    }
}

impl From<lz4_flex::block::DecompressError> for Error {
    fn from(e: lz4_flex::block::DecompressError) -> Error {
        Error {
            kind: ErrorKind::Decompression(e),
        }
    }
}

use std::error;
use std::fmt;
use std::io;

#[derive(Debug)]
pub struct FrostError {
    kind: FrostErrorKind,
}

impl FrostError {
    pub(crate) fn new(kind: FrostErrorKind) -> FrostError {
        FrostError { kind }
    }
    pub fn kind(&self) -> &FrostErrorKind {
        &self.kind
    }
}
#[derive(Debug)]
pub enum FrostErrorKind {
    NotARosbag,
    UnindexedBag,
    InvalidBag(&'static str),
    Deserialization(serde_rosmsg::Error),
    Io(io::Error),
}

impl fmt::Display for FrostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            FrostErrorKind::NotARosbag => {
                write!(f, "invalid rosbag v2 header")
            }
            FrostErrorKind::Io(ref e) => e.fmt(f),
            FrostErrorKind::UnindexedBag => write!(f, "unindexed bag"),
            FrostErrorKind::InvalidBag(s) => write!(f, "invalid bag: {s}"),
            FrostErrorKind::Deserialization(ref e) => e.fmt(f),
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

impl From<serde_rosmsg::Error> for FrostError {
    fn from(e: serde_rosmsg::Error) -> FrostError {
        FrostError {
            kind: FrostErrorKind::Deserialization(e),
        }
    }
}

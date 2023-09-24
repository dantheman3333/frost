use std::borrow::Cow;

use serde;
use serde::de;
use serde_rosmsg;

use crate::errors::{Error, ErrorKind};
use crate::{ChunkHeaderLoc, DecompressedBag};

pub trait Msg {}

pub struct MessageView<'a> {
    pub topic: &'a str,
    pub(crate) bag: &'a DecompressedBag,
    pub(crate) chunk_loc: ChunkHeaderLoc,
    pub(crate) start_index: usize,
    pub(crate) end_index: usize,
}

impl<'a> MessageView<'a> {
    /// Returns the raw bytes of the entire Chunk that holds the message
    fn chunk_bytes(&self) -> Result<&'a [u8], Error> {
        self.bag
            .chunk_bytes
            .get(&self.chunk_loc)
            .map(|vec| vec.as_slice())
            .ok_or_else(|| {
                Error::new(ErrorKind::InvalidBag(Cow::Borrowed(
                    "Supplied chunk loc for msg view doesn't exist",
                )))
            })
    }

    /// Returns the raw bytes of the entire ROS message
    pub fn raw_bytes(&self) -> Result<&'a [u8], Error> {
        Ok(&self.chunk_bytes()?[self.start_index..self.end_index])
    }

    /// Turns a `MessageView` into a Rust struct
    pub fn instantiate<'de, T>(&self) -> Result<T, Error>
    where
        T: Msg,
        T: de::Deserialize<'de>,
    {
        serde_rosmsg::from_slice(&self.raw_bytes()?).map_err(|e| e.into())
    }
}

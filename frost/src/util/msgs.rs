use std::borrow::Cow;
use std::io::{Read, Seek};

use serde;
use serde::de;
use serde_rosmsg;

use crate::errors::{Error, ErrorKind};
use crate::{Bag, ChunkHeaderLoc};

pub trait Msg {}

pub struct MessageView<'a, R: Read + Seek> {
    pub topic: &'a str,
    pub(crate) bag: &'a Bag<R>,
    pub(crate) chunk_loc: ChunkHeaderLoc,
    pub(crate) start_index: usize,
    pub(crate) end_index: usize,
}

impl<'a, R: Read + Seek> MessageView<'a, R> {
    /// Returns the raw bytes of the entire ROS message
    pub fn raw_bytes(&self) -> Result<&'a [u8], Error> {
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

    /// Turns a `MessageView` into a Rust struct
    pub fn instantiate<'de, T>(&self) -> Result<T, Error>
    where
        T: Msg,
        T: de::Deserialize<'de>,
    {
        let bytes = self.raw_bytes()?;
        serde_rosmsg::from_slice(&bytes[self.start_index..self.end_index]).map_err(|e| e.into())
    }
}

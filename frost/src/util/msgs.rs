use serde;
use serde::de;
use serde_rosmsg;

use crate::errors::{FrostError, FrostErrorKind};
use crate::{Bag, ChunkHeaderLoc};

pub trait Msg {}

pub struct MessageView<'a> {
    pub topic: &'a str,
    pub(crate) bag: &'a Bag,
    pub(crate) chunk_loc: ChunkHeaderLoc,
    pub(crate) start_index: usize,
    pub(crate) end_index: usize,
}

impl<'a> MessageView<'a> {
    pub fn instantiate<'de, T>(&self) -> Result<T, FrostError>
    where
        T: Msg,
        T: de::Deserialize<'de>,
    {
        let bytes = self
            .bag
            .chunk_bytes
            .get(&self.chunk_loc)
            .ok_or(FrostError::new(FrostErrorKind::InvalidBag(
                "Supplied chunk loc for msg view doesn't exist",
            )))?;
        serde_rosmsg::from_slice(&bytes[self.start_index..self.end_index]).map_err(|e| e.into())
    }
}

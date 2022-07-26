use serde;
use serde::de;
use serde_rosmsg;

pub trait Msg {}

#[derive(Debug)]
pub struct MessageView {
    // TODO: use a str
    pub topic: String,
    // TODO: don't copy, use a slice
    pub bytes: Vec<u8>,
}

impl MessageView {
    pub fn instantiate<'de, T>(&self) -> Result<T, serde_rosmsg::Error>
    where
        T: Msg,
        T: de::Deserialize<'de>,
    {
        serde_rosmsg::from_slice(self.bytes.as_slice())
    }
}

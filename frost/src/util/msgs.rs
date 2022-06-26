use serde;
use serde::de;
use serde_derive::Deserialize;
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
    pub fn instantiate<'de, T>(self) -> Result<T, serde_rosmsg::Error>
    where
        T: Msg,
        T: de::Deserialize<'de>,
    {
        serde_rosmsg::from_slice(self.bytes.as_slice())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq)]
pub struct Time {
    pub secs: u32,
    pub nsecs: u32,
}

impl Msg for Time {}

#[derive(Debug, Deserialize, PartialEq)]
pub struct Duration {
    pub secs: u32,
    pub nsecs: u32,
}
impl Msg for Duration {}

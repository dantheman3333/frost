use serde_derive::Deserialize;

#[derive(Clone, Copy, Debug, Deserialize, Eq)]
pub struct Time {
    pub secs: u32,
    pub nsecs: u32,
}

#[derive(Debug, Deserialize, PartialEq)]
struct Header {
    id: u32,
    time: Time,
    frame_id: String,
}

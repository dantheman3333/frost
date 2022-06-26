use serde_derive::Deserialize;

use crate::util::msgs::Msg;

#[derive(Clone, Copy, Debug, Deserialize, Eq)]
pub struct Time {
    pub secs: u32,
    pub nsecs: u32,
}

impl Msg for Time {}

#[derive(Debug, Deserialize, PartialEq)]
pub struct Header {
    id: u32,
    time: Time,
    frame_id: String,
}
impl Msg for Header {}

#[derive(Debug, Deserialize, PartialEq)]
pub struct StdString {
    data: String,
}
impl Msg for StdString {}

#[derive(Debug, Deserialize, PartialEq)]
pub struct MultiArrayLayout {
    dim: Vec<MultiArrayDimension>,
    data_offset: u32,
}
impl Msg for MultiArrayLayout {}

#[derive(Debug, Deserialize, PartialEq)]
pub struct MultiArrayDimension {
    label: String,
    size: u32,
    stride: u32,
}
impl Msg for MultiArrayDimension {}

#[derive(Debug, Deserialize, PartialEq)]
pub struct Float64MultiArray {
    layout: MultiArrayLayout,
    data: Vec<f64>,
}
impl Msg for Float64MultiArray {}

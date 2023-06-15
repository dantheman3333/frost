/// This file is autogenerated. Do not edit by hand!
pub mod msgs {
    pub mod std_msgs {
        #[derive(Clone, Debug, serde::Deserialize, PartialEq)]
        pub struct Bool {
            pub r#data: bool,
        }
        impl Bool {}
        impl frost::msgs::Msg for Bool {}
        #[derive(Clone, Debug, serde::Deserialize, PartialEq)]
        pub struct Byte {
            pub r#data: u8,
        }
        impl Byte {}
        impl frost::msgs::Msg for Byte {}
        #[derive(Clone, Debug, serde::Deserialize, PartialEq)]
        pub struct ByteMultiArray {
            pub r#layout: MultiArrayLayout,
            pub r#data: Vec<u8>,
        }
        impl ByteMultiArray {}
        impl frost::msgs::Msg for ByteMultiArray {}
        #[derive(Clone, Debug, serde::Deserialize, PartialEq)]
        pub struct Char {
            pub r#data: char,
        }
        impl Char {}
        impl frost::msgs::Msg for Char {}
        #[derive(Clone, Debug, serde::Deserialize, PartialEq)]
        pub struct ColorRGBA {
            pub r#r: f32,
            pub r#g: f32,
            pub r#b: f32,
            pub r#a: f32,
        }
        impl ColorRGBA {}
        impl frost::msgs::Msg for ColorRGBA {}
        #[derive(Clone, Debug, serde::Deserialize, PartialEq)]
        pub struct Duration {
            pub r#data: frost::time::RosDuration,
        }
        impl Duration {}
        impl frost::msgs::Msg for Duration {}
        #[derive(Clone, Debug, serde::Deserialize, PartialEq)]
        pub struct Empty {}
        impl Empty {}
        impl frost::msgs::Msg for Empty {}
        #[derive(Clone, Debug, serde::Deserialize, PartialEq)]
        pub struct Float32 {
            pub r#data: f32,
        }
        impl Float32 {}
        impl frost::msgs::Msg for Float32 {}
        #[derive(Clone, Debug, serde::Deserialize, PartialEq)]
        pub struct Float32MultiArray {
            pub r#layout: MultiArrayLayout,
            pub r#data: Vec<f32>,
        }
        impl Float32MultiArray {}
        impl frost::msgs::Msg for Float32MultiArray {}
        #[derive(Clone, Debug, serde::Deserialize, PartialEq)]
        pub struct Float64 {
            pub r#data: f64,
        }
        impl Float64 {}
        impl frost::msgs::Msg for Float64 {}
        #[derive(Clone, Debug, serde::Deserialize, PartialEq)]
        pub struct Float64MultiArray {
            pub r#layout: MultiArrayLayout,
            pub r#data: Vec<f64>,
        }
        impl Float64MultiArray {}
        impl frost::msgs::Msg for Float64MultiArray {}
        #[derive(Clone, Debug, serde::Deserialize, PartialEq)]
        pub struct Header {
            pub r#seq: u32,
            pub r#stamp: frost::time::Time,
            pub r#frame_id: std::string::String,
        }
        impl Header {}
        impl frost::msgs::Msg for Header {}
        #[derive(Clone, Debug, serde::Deserialize, PartialEq)]
        pub struct Int16 {
            pub r#data: i16,
        }
        impl Int16 {}
        impl frost::msgs::Msg for Int16 {}
        #[derive(Clone, Debug, serde::Deserialize, PartialEq)]
        pub struct Int16MultiArray {
            pub r#layout: MultiArrayLayout,
            pub r#data: Vec<i16>,
        }
        impl Int16MultiArray {}
        impl frost::msgs::Msg for Int16MultiArray {}
        #[derive(Clone, Debug, serde::Deserialize, PartialEq)]
        pub struct Int32 {
            pub r#data: i32,
        }
        impl Int32 {}
        impl frost::msgs::Msg for Int32 {}
        #[derive(Clone, Debug, serde::Deserialize, PartialEq)]
        pub struct Int32MultiArray {
            pub r#layout: MultiArrayLayout,
            pub r#data: Vec<i32>,
        }
        impl Int32MultiArray {}
        impl frost::msgs::Msg for Int32MultiArray {}
        #[derive(Clone, Debug, serde::Deserialize, PartialEq)]
        pub struct Int64 {
            pub r#data: i64,
        }
        impl Int64 {}
        impl frost::msgs::Msg for Int64 {}
        #[derive(Clone, Debug, serde::Deserialize, PartialEq)]
        pub struct Int64MultiArray {
            pub r#layout: MultiArrayLayout,
            pub r#data: Vec<i64>,
        }
        impl Int64MultiArray {}
        impl frost::msgs::Msg for Int64MultiArray {}
        #[derive(Clone, Debug, serde::Deserialize, PartialEq)]
        pub struct Int8 {
            pub r#data: i8,
        }
        impl Int8 {}
        impl frost::msgs::Msg for Int8 {}
        #[derive(Clone, Debug, serde::Deserialize, PartialEq)]
        pub struct Int8MultiArray {
            pub r#layout: MultiArrayLayout,
            pub r#data: Vec<i8>,
        }
        impl Int8MultiArray {}
        impl frost::msgs::Msg for Int8MultiArray {}
        #[derive(Clone, Debug, serde::Deserialize, PartialEq)]
        pub struct MultiArrayDimension {
            pub r#label: std::string::String,
            pub r#size: u32,
            pub r#stride: u32,
        }
        impl MultiArrayDimension {}
        impl frost::msgs::Msg for MultiArrayDimension {}
        #[derive(Clone, Debug, serde::Deserialize, PartialEq)]
        pub struct MultiArrayLayout {
            pub r#dim: Vec<MultiArrayDimension>,
            pub r#data_offset: u32,
        }
        impl MultiArrayLayout {}
        impl frost::msgs::Msg for MultiArrayLayout {}
        #[derive(Clone, Debug, serde::Deserialize, PartialEq)]
        pub struct String {
            pub r#data: std::string::String,
        }
        impl String {}
        impl frost::msgs::Msg for String {}
        #[derive(Clone, Debug, serde::Deserialize, PartialEq)]
        pub struct Time {
            pub r#data: frost::time::Time,
        }
        impl Time {}
        impl frost::msgs::Msg for Time {}
        #[derive(Clone, Debug, serde::Deserialize, PartialEq)]
        pub struct UInt16 {
            pub r#data: u16,
        }
        impl UInt16 {}
        impl frost::msgs::Msg for UInt16 {}
        #[derive(Clone, Debug, serde::Deserialize, PartialEq)]
        pub struct UInt16MultiArray {
            pub r#layout: MultiArrayLayout,
            pub r#data: Vec<u16>,
        }
        impl UInt16MultiArray {}
        impl frost::msgs::Msg for UInt16MultiArray {}
        #[derive(Clone, Debug, serde::Deserialize, PartialEq)]
        pub struct UInt32 {
            pub r#data: u32,
        }
        impl UInt32 {}
        impl frost::msgs::Msg for UInt32 {}
        #[derive(Clone, Debug, serde::Deserialize, PartialEq)]
        pub struct UInt32MultiArray {
            pub r#layout: MultiArrayLayout,
            pub r#data: Vec<u32>,
        }
        impl UInt32MultiArray {}
        impl frost::msgs::Msg for UInt32MultiArray {}
        #[derive(Clone, Debug, serde::Deserialize, PartialEq)]
        pub struct UInt64 {
            pub r#data: u64,
        }
        impl UInt64 {}
        impl frost::msgs::Msg for UInt64 {}
        #[derive(Clone, Debug, serde::Deserialize, PartialEq)]
        pub struct UInt64MultiArray {
            pub r#layout: MultiArrayLayout,
            pub r#data: Vec<u64>,
        }
        impl UInt64MultiArray {}
        impl frost::msgs::Msg for UInt64MultiArray {}
        #[derive(Clone, Debug, serde::Deserialize, PartialEq)]
        pub struct UInt8 {
            pub r#data: u8,
        }
        impl UInt8 {}
        impl frost::msgs::Msg for UInt8 {}
        #[derive(Clone, Debug, serde::Deserialize, PartialEq)]
        pub struct UInt8MultiArray {
            pub r#layout: MultiArrayLayout,
            pub r#data: Vec<u8>,
        }
        impl UInt8MultiArray {}
        impl frost::msgs::Msg for UInt8MultiArray {}
    }
}
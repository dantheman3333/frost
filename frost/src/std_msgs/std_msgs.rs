use std::borrow::Cow;

use zerocopy::{FromBytes, LittleEndian, Unaligned, U32};

#[derive(FromBytes, Unaligned)]
#[repr(C)]
struct MsgString<'a> {
    msg_len: U32<LittleEndian>,
    data_len: U32<LittleEndian>,
    data: Cow<'a, str>,
}

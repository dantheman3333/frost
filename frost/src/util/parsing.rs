use std::io::{self, Read};

use crate::errors::ParseError;

#[inline(always)]
pub fn parse_u8(buf: &[u8]) -> Result<u8, ParseError> {
    parse_u8_at(buf, 0)
}

#[inline(always)]
pub fn parse_u8_at(buf: &[u8], index: usize) -> Result<u8, ParseError> {
    let bytes = buf.get(index..index + 1).ok_or_else(|| {
        eprintln!("Buffer is not large enough to parse 1 byte");
        ParseError::BufferTooSmall
    })?;
    Ok(u8::from_le_bytes(bytes.try_into().unwrap()))
}

#[inline(always)]
pub fn parse_le_u32(buf: &[u8]) -> Result<u32, ParseError> {
    parse_le_u32_at(buf, 0)
}

#[inline(always)]
pub fn parse_le_u32_at(buf: &[u8], index: usize) -> Result<u32, ParseError> {
    let bytes = buf.get(index..index + 4).ok_or_else(|| {
        eprintln!("Buffer is not large enough to parse 4 bytes");
        ParseError::BufferTooSmall
    })?;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}

#[inline(always)]
pub fn parse_le_u64(buf: &[u8]) -> Result<u64, ParseError> {
    parse_le_u64_at(buf, 0)
}

#[inline(always)]
pub fn parse_le_u64_at(buf: &[u8], index: usize) -> Result<u64, ParseError> {
    let bytes = buf.get(index..index + 8).ok_or_else(|| {
        eprintln!("Buffer is not large enough to parse 8 bytes");
        ParseError::BufferTooSmall
    })?;
    Ok(u64::from_le_bytes(bytes.try_into().unwrap()))
}

#[inline(always)]
pub fn get_lengthed_bytes(reader: &mut impl Read) -> Result<Vec<u8>, ParseError> {
    // Get a vector of bytes from a reader when the first 4 bytes are the length
    // Ex: with <header_len><header> or <data_len><data>, this function returns either header or data
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf).map_err(|e| {
        eprintln!("could not read the 4 byte length field, not enough bytes {e}");
        ParseError::BufferTooSmall
    })?;

    let len = u32::from_le_bytes(len_buf);
    let mut bytes = vec![0u8; len as usize];
    reader.read_exact(&mut bytes).map_err(|e| {
        eprintln!("could not read the supplied length of {len}, not enough bytes {e}");
        ParseError::BufferTooSmall
    })?;

    Ok(bytes)
}

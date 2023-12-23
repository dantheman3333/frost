#![allow(dead_code)]

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;
use std::io::{self, prelude::*, BufReader, Cursor};
use std::path::{Path, PathBuf};
use std::time::Duration;

type ConnectionID = u32;
type ChunkHeaderLoc = u64;

use errors::{Error, ErrorKind, ParseError};

use itertools::Itertools;
pub use util::msgs;
use util::parsing::get_lengthed_bytes;
pub use util::query;
pub use util::time;

pub mod errors;
mod util;
use util::query::{BagIter, Query};
use util::time::Time;

/// Metadata about a bag.
/// Unlike [DecompressedBag], `BagMetadata` is constructed without loading chunks/messages.
///
/// Example
/// ```rust
/// use std::path::PathBuf;
/// use frost::BagMetadata;
///
/// let file_path = PathBuf::from("/some/path");
/// if let Ok(metadata) = BagMetadata::from_file(file_path) {
///     for topic in metadata.topics() {
///         println!("{topic}");
///     }
/// }
/// ```
pub struct BagMetadata {
    /// The path to the file, if loaded from one.
    pub file_path: Option<PathBuf>,
    /// The version, but only `ROSBAG V2.0` is supported.
    pub version: String,
    pub(crate) chunk_metadata: BTreeMap<ChunkHeaderLoc, ChunkMetadata>,
    #[doc(hidden)]
    /// likely to be made crate private soon
    pub connection_data: BTreeMap<ConnectionID, ConnectionData>,
    pub(crate) index_data: BTreeMap<ConnectionID, Vec<IndexData>>,
    /// The number of bytes seen on-disk when using [BagMetadata::from_file] or the length of the slice passed into [BagMetadata::from_bytes].
    pub num_bytes: u64,
}

/// Represents an owned and decompresed Bag in memory.
pub struct DecompressedBag {
    pub metadata: BagMetadata,
    pub(crate) chunk_bytes: BTreeMap<ChunkHeaderLoc, Vec<u8>>,
}

#[derive(Debug)]
/// Statistics about a type of compression used in a bag.
pub struct CompressionInfo {
    pub name: String,
    pub chunk_count: usize,
    pub total_compressed: usize,
    pub total_uncompressed: usize,
}

#[derive(Debug)]
#[repr(u8)]
enum OpCode {
    BagHeader = 0x03,
    ChunkHeader = 0x05,
    ConnectionHeader = 0x07,
    MessageData = 0x02,
    IndexDataHeader = 0x04,
    ChunkInfoHeader = 0x06,
}

impl OpCode {
    fn from(byte: u8) -> Result<OpCode, ParseError> {
        match byte {
            0x03 => Ok(OpCode::BagHeader),
            0x05 => Ok(OpCode::ChunkHeader),
            0x07 => Ok(OpCode::ConnectionHeader),
            0x02 => Ok(OpCode::MessageData),
            0x04 => Ok(OpCode::IndexDataHeader),
            0x06 => Ok(OpCode::ChunkInfoHeader),
            _ => {
                eprintln!("invalid op code {byte:x}");
                Err(ParseError::InvalidOpCode)
            }
        }
    }
}

#[inline(always)]
/// Returns None on EOF
fn read_le_u32(reader: &mut impl Read) -> Option<u32> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf).ok()?;
    Some(u32::from_le_bytes(len_buf))
}

#[inline(always)]
fn field_sep_index(buf: &[u8]) -> Result<usize, ParseError> {
    buf.iter()
        .position(|&b| b == b'=')
        .ok_or(ParseError::MissingFieldSeparator)
}

#[inline(always)]
fn parse_field(buf: &[u8], i: usize) -> Result<(usize, &[u8], &[u8]), ParseError> {
    let mut i = i;
    let field_len = util::parsing::parse_le_u32_at(buf, i)? as usize;
    i += 4;
    let sep_pos = i + field_sep_index(&buf[i..i + field_len])?;

    let name = &buf[i..sep_pos];
    let value = &buf[(sep_pos + 1)..(i + field_len)];

    i += field_len;
    Ok((i, name, value))
}

fn version_check(reader: &mut impl Read) -> Result<String, Error> {
    let mut buf = [0u8; 13];
    let expected = b"#ROSBAG V2.0\n";
    reader.read_exact(&mut buf)?;
    if buf == *expected {
        Ok("2.0".into())
    } else {
        Err(Error::new(ErrorKind::NotARosbag))
    }
}

#[derive(Debug)]
struct BagHeader {
    index_pos: u64,
    conn_count: u32,
    chunk_count: u32,
}

impl BagHeader {
    fn from(buf: &[u8]) -> Result<BagHeader, ParseError> {
        let mut i = 0;

        let mut index_pos = None;
        let mut conn_count = None;
        let mut chunk_count = None;

        loop {
            let (new_index, name, value) = parse_field(buf, i)?;
            i = new_index;

            match name {
                b"index_pos" => index_pos = Some(util::parsing::parse_le_u64(value)?),
                b"conn_count" => conn_count = Some(util::parsing::parse_le_u32(value)?),
                b"chunk_count" => chunk_count = Some(util::parsing::parse_le_u32(value)?),
                b"op" => {
                    let op = util::parsing::parse_u8(value)?;
                    if op != OpCode::BagHeader as u8 {
                        eprintln!("expected a BagHeader OpCode when parsing BagHeader");
                        return Err(ParseError::UnexpectedOpCode);
                    }
                }
                other => {
                    eprintln!(
                        "unexpected field: {} in 'BagHeader'",
                        String::from_utf8_lossy(other)
                    );
                    return Err(ParseError::UnexpectedField);
                }
            }

            if i >= buf.len() {
                break;
            }
        }

        Ok(BagHeader {
            index_pos: index_pos.ok_or_else(|| {
                eprintln!("missing index_pos when parsing a BagHeader");
                ParseError::MissingField
            })?,
            conn_count: conn_count.ok_or_else(|| {
                eprintln!("missing conn_count when parsing a BagHeader");
                ParseError::MissingField
            })?,
            chunk_count: chunk_count.ok_or_else(|| {
                eprintln!("missing chunk_count when parsing a BagHeader");
                ParseError::MissingField
            })?,
        })
    }
}

/// Struct to store everything about a Chunk
///
/// As ChunkHeader and ChunkInfoHeaders are separate, after parsing all records, combine that info into a Chunk
struct ChunkMetadata {
    compression: String,
    uncompressed_size: u32,
    compressed_size: u32,
    chunk_header_pos: u64,
    chunk_data_pos: u64,
    start_time: Time,
    end_time: Time,
    connection_count: u32,
    message_counts: BTreeMap<ConnectionID, u32>,
}

struct ChunkHeader {
    compression: String,
    uncompressed_size: u32,
    compressed_size: u32,
    chunk_header_pos: u64,
    chunk_data_pos: u64,
}

impl ChunkHeader {
    fn from(
        buf: &[u8],
        chunk_header_pos: u64,
        chunk_data_pos: u64,
        compressed_size: u32,
    ) -> Result<ChunkHeader, ParseError> {
        let mut i = 0;

        let mut compression = None;
        let mut size = None;

        loop {
            let (new_index, name, value) = parse_field(buf, i)?;
            i = new_index;

            match name {
                b"compression" => compression = Some(String::from_utf8_lossy(value).to_string()),
                b"size" => size = Some(util::parsing::parse_le_u32(value)?),
                b"op" => {
                    let op = util::parsing::parse_u8(value)?;
                    if op != OpCode::ChunkHeader as u8 {
                        eprintln!("expected a ChunkHeader OpCode when parsing ChunkHeader");
                        return Err(ParseError::UnexpectedOpCode);
                    }
                }
                other => {
                    eprintln!(
                        "unexpected field: {} in 'ChunkHeader'",
                        String::from_utf8_lossy(other)
                    );
                    return Err(ParseError::UnexpectedField);
                }
            }

            if i >= buf.len() {
                break;
            }
        }

        Ok(ChunkHeader {
            compression: compression.ok_or_else(|| {
                eprintln!("missing compression when parsing a ChunkHeader");
                ParseError::MissingField
            })?,
            uncompressed_size: size.ok_or_else(|| {
                eprintln!("missing uncompressed_size when parsing a ChunkHeader");
                ParseError::MissingField
            })?,
            chunk_header_pos,
            chunk_data_pos,
            compressed_size,
        })
    }
}

struct ChunkInfoHeader {
    version: u32,
    chunk_header_pos: u64,
    // timestamp of earliest message in the chunk
    start_time: Time,
    // timestamp of latest message in the chunk
    end_time: Time,
    // number of connections in the chunk
    connection_count: u32,
}

impl ChunkInfoHeader {
    fn from(buf: &[u8]) -> Result<ChunkInfoHeader, ParseError> {
        let mut i = 0;

        let mut version = None;
        let mut chunk_header_pos = None;
        let mut start_time = None;
        let mut end_time = None;
        let mut connection_count = None;

        loop {
            let (new_index, name, value) = parse_field(buf, i)?;
            i = new_index;

            match name {
                b"ver" => version = Some(util::parsing::parse_le_u32(value)?),
                b"chunk_pos" => chunk_header_pos = Some(util::parsing::parse_le_u64(value)?),
                b"start_time" => start_time = Some(Time::from(value)?),
                b"end_time" => end_time = Some(Time::from(value)?),
                b"count" => connection_count = Some(util::parsing::parse_le_u32(value)?),
                b"op" => {
                    let op = util::parsing::parse_u8(value)?;
                    if op != OpCode::ChunkInfoHeader as u8 {
                        eprintln!("expected a ChunkInfoHeader OpCode when parsing ChunkInfoHeader");
                        return Err(ParseError::UnexpectedOpCode);
                    }
                }
                other => {
                    eprintln!(
                        "unexpected field: {} in ChunkInfoHeader",
                        String::from_utf8_lossy(other)
                    );
                    return Err(ParseError::UnexpectedField);
                }
            }

            if i >= buf.len() {
                break;
            }
        }

        Ok(ChunkInfoHeader {
            version: version.ok_or_else(|| {
                eprintln!("missing ver when parsing a ChunkInfoHeader");
                ParseError::MissingField
            })?,
            chunk_header_pos: chunk_header_pos.ok_or_else(|| {
                eprintln!("missing chunk_pos when parsing a ChunkInfoHeader");
                ParseError::MissingField
            })?,
            start_time: start_time.ok_or_else(|| {
                eprintln!("missing start_time when parsing a ChunkInfoHeader");
                ParseError::MissingField
            })?,
            end_time: end_time.ok_or_else(|| {
                eprintln!("missing end_time when parsing a ChunkInfoHeader");
                ParseError::MissingField
            })?,
            connection_count: connection_count.ok_or_else(|| {
                eprintln!("missing count when parsing a ChunkInfoHeader");
                ParseError::MissingField
            })?,
        })
    }
}

struct ChunkInfoData {
    connection_id: ConnectionID,
    count: u32,
}

impl ChunkInfoData {
    fn from(buf: &[u8]) -> Result<ChunkInfoData, ParseError> {
        Ok(ChunkInfoData {
            connection_id: util::parsing::parse_le_u32_at(buf, 0)?,
            count: util::parsing::parse_le_u32_at(buf, 4)?,
        })
    }
}

#[derive(Debug)]
struct ConnectionHeader {
    connection_id: u32,
    topic: String,
}

impl ConnectionHeader {
    fn from(buf: &[u8]) -> Result<ConnectionHeader, ParseError> {
        let mut i = 0;

        let mut topic = None;
        let mut connection_id = None;

        loop {
            let (new_index, name, value) = parse_field(buf, i)?;
            i = new_index;

            match name {
                b"topic" => topic = Some(String::from_utf8_lossy(value).to_string()),
                b"conn" => connection_id = Some(util::parsing::parse_le_u32(value)?),
                b"op" => {
                    let op = util::parsing::parse_u8(value)?;
                    if op != OpCode::ConnectionHeader as u8 {
                        eprintln!(
                            "expected a ConnectionHeader OpCode when parsing ConnectionHeader"
                        );
                        return Err(ParseError::UnexpectedOpCode);
                    }
                }
                other => {
                    eprintln!(
                        "unexpected field: {} in ConnectionHeader",
                        String::from_utf8_lossy(other)
                    );
                    return Err(ParseError::UnexpectedField);
                }
            }

            if i >= buf.len() {
                break;
            }
        }

        Ok(ConnectionHeader {
            connection_id: connection_id.ok_or_else(|| {
                eprintln!("missing conn when parsing a ConnectionHeader");
                ParseError::MissingField
            })?,
            topic: topic.ok_or_else(|| {
                eprintln!("missing topic when parsing a ConnectionHeader");
                ParseError::MissingField
            })?,
        })
    }
}

#[doc(hidden)] // likey to be made crate private
#[derive(Debug)]
///Store metadata for connections, including topic, conn id, md5, etc.
pub struct ConnectionData {
    pub connection_id: u32,
    pub topic: String,
    pub data_type: String,
    pub md5sum: String,
    pub message_definition: String,
    pub caller_id: Option<String>,
    pub latching: bool,
}

impl ConnectionData {
    fn from(buf: &[u8], connection_id: u32, topic: String) -> Result<ConnectionData, ParseError> {
        let mut i = 0;

        let mut data_type = None;
        let mut md5sum = None;
        let mut message_definition = None;
        let mut caller_id = None;
        let mut latching = false;

        loop {
            let (new_index, name, value) = parse_field(buf, i)?;
            i = new_index;

            match name {
                b"topic" => (),
                b"type" => data_type = Some(String::from_utf8_lossy(value).to_string()),
                b"md5sum" => md5sum = Some(String::from_utf8_lossy(value).to_string()),
                b"message_definition" => {
                    message_definition = Some(String::from_utf8_lossy(value).to_string())
                }
                b"callerid" => caller_id = Some(String::from_utf8_lossy(value).to_string()),
                b"latching" => latching = value == b"1",
                other => {
                    eprintln!(
                        "unexpected field: {} in ConnectionData",
                        String::from_utf8_lossy(other)
                    );
                    return Err(ParseError::UnexpectedField);
                }
            }

            if i >= buf.len() {
                break;
            }
        }

        Ok(ConnectionData {
            connection_id,
            topic,
            data_type: data_type.ok_or_else(|| {
                eprintln!("missing type when parsing a ConnectionData");
                ParseError::MissingField
            })?,
            md5sum: md5sum.ok_or_else(|| {
                eprintln!("missing md5sum when parsing a ConnectionData");
                ParseError::MissingField
            })?,
            message_definition: message_definition.ok_or_else(|| {
                eprintln!("missing message_definition when parsing a ConnectionData");
                ParseError::MissingField
            })?,
            caller_id,
            latching,
        })
    }
}

struct IndexDataHeader {
    version: u32, //must be 1
    connection_id: ConnectionID,
    count: u32, // number of messages on conn in the preceding chunk
}

impl IndexDataHeader {
    fn from(buf: &[u8]) -> Result<IndexDataHeader, ParseError> {
        let mut i = 0;

        let mut version = None;
        let mut connection_id = None;
        let mut count = None;

        loop {
            let (new_index, name, value) = parse_field(buf, i)?;
            i = new_index;

            match name {
                b"ver" => version = Some(util::parsing::parse_le_u32(value)?),
                b"conn" => connection_id = Some(util::parsing::parse_le_u32(value)?),
                b"count" => count = Some(util::parsing::parse_le_u32(value)?),
                b"op" => {
                    let op = util::parsing::parse_u8(value)?;
                    if op != OpCode::IndexDataHeader as u8 {
                        eprintln!("expected a IndexDataHeader OpCode when parsing IndexDataHeader");
                        return Err(ParseError::UnexpectedOpCode);
                    }
                }
                other => {
                    eprintln!(
                        "unexpected field: {} in IndexDataHeader",
                        String::from_utf8_lossy(other)
                    );
                    return Err(ParseError::UnexpectedField);
                }
            }

            if i >= buf.len() {
                break;
            }
        }

        Ok(IndexDataHeader {
            version: version.ok_or_else(|| {
                eprintln!("missing ver when parsing a IndexDataHeader");
                ParseError::MissingField
            })?,
            connection_id: connection_id.ok_or_else(|| {
                eprintln!("missing conn when parsing a IndexDataHeader");
                ParseError::MissingField
            })?,
            count: count.ok_or_else(|| {
                eprintln!("missing count when parsing a IndexDataHeader");
                ParseError::MissingField
            })?,
        })
    }
}

#[derive(Debug, Clone)]
///Stores data about messages and where they are in the bag
pub(crate) struct IndexData {
    conn_id: ConnectionID,
    ///start position of the chunk in the file
    chunk_header_pos: ChunkHeaderLoc,
    ///time at which the message was received
    time: Time,
    ///offset of message data record in uncompressed chunk data   
    offset: usize,
}

impl IndexData {
    fn from(
        buf: &[u8],
        chunk_header_pos: u64,
        conn_id: ConnectionID,
    ) -> Result<IndexData, ParseError> {
        Ok(IndexData {
            chunk_header_pos,
            time: Time::from(buf)?,
            offset: util::parsing::parse_le_u32_at(buf, 8)? as usize,
            conn_id,
        })
    }
}

#[derive(Debug)]
struct MessageDataHeader {
    ///ID for connection on which message arrived
    conn: ConnectionID,
    ///Time at which the message was received
    time: Time,
}

impl MessageDataHeader {
    fn from(buf: &[u8]) -> Result<MessageDataHeader, ParseError> {
        let mut i = 0;

        let mut conn = None;
        let mut time = None;

        loop {
            let (new_index, name, value) = parse_field(buf, i)?;
            i = new_index;

            match name {
                b"conn" => conn = Some(util::parsing::parse_le_u32(value)?),
                b"time" => time = Some(Time::from(value)?),
                b"op" => {
                    let op = util::parsing::parse_u8(value)?;
                    if op != OpCode::MessageData as u8 {
                        eprintln!("expected a MessageData OpCode when parsing MessageData");
                        return Err(ParseError::UnexpectedOpCode);
                    }
                }
                other => {
                    eprintln!(
                        "unexpected field: {} in MessageDataHeader",
                        String::from_utf8_lossy(other)
                    );
                    return Err(ParseError::UnexpectedField);
                }
            }

            if i >= buf.len() {
                break;
            }
        }

        Ok(MessageDataHeader {
            conn: conn.ok_or_else(|| {
                eprintln!("missing conn when parsing a IndexDataHeader");
                ParseError::MissingField
            })?,
            time: time.ok_or_else(|| {
                eprintln!("missing time when parsing a IndexDataHeader");
                ParseError::MissingField
            })?,
        })
    }
}

impl BagMetadata {
    /// Read bag metadata from a file path.
    pub fn from_file<P>(file_path: P) -> Result<Self, Error>
    where
        P: AsRef<Path> + Into<PathBuf>,
    {
        let path: PathBuf = file_path.as_ref().into();
        let file = File::open(file_path)?;
        let file_size = file.metadata()?.len();

        let reader = BufReader::new(file);

        let mut bag = Self::from_reader(reader)?;
        bag.file_path = Some(path);
        bag.num_bytes = file_size;
        Ok(bag)
    }

    /// Read bag metadata from an existing byte slice.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        let reader = Cursor::new(bytes);
        let mut bag = Self::from_reader(reader)?;
        bag.num_bytes = bytes.len() as u64;
        Ok(bag)
    }

    fn from_reader<R: Read + Seek>(mut reader: R) -> Result<BagMetadata, Error> {
        let version = version_check(&mut reader)?;

        let (chunk_metadata, connection_data, index_data) = parse_records(&mut reader)?;

        Ok(BagMetadata {
            version,
            file_path: None,
            chunk_metadata,
            connection_data,
            index_data,
            num_bytes: 0,
        })
    }

    fn topic_to_connection_ids(&self) -> BTreeMap<String, Vec<ConnectionID>> {
        self.connection_data
            .values()
            .fold(BTreeMap::new(), |mut acc, data| {
                acc.entry(data.topic.clone())
                    .or_default()
                    .push(data.connection_id);
                acc
            })
    }

    fn type_to_connection_ids(&self) -> BTreeMap<String, Vec<ConnectionID>> {
        self.connection_data
            .values()
            .fold(BTreeMap::new(), |mut acc, data| {
                acc.entry(data.data_type.clone())
                    .or_default()
                    .push(data.connection_id);
                acc
            })
    }

    pub fn start_time(&self) -> Option<Time> {
        self.chunk_metadata
            .values()
            .map(|meta| meta.start_time)
            .min()
    }

    pub fn end_time(&self) -> Option<Time> {
        self.chunk_metadata.values().map(|meta| meta.end_time).max()
    }

    pub fn duration(&self) -> Duration {
        let start = self.start_time().unwrap_or(time::ZERO);
        let end = self.end_time().unwrap_or(time::ZERO);
        end.dur(&start)
    }

    pub fn message_count(&self) -> usize {
        self.index_data.values().map(|v| v.len()).sum()
    }

    pub fn topic_message_counts(&self) -> BTreeMap<String, usize> {
        let topic_to_ids = self.topic_to_connection_ids();
        topic_to_ids
            .iter()
            .map(|(topic, conn_ids)| {
                (
                    topic.clone(),
                    conn_ids
                        .iter()
                        .map(|id| self.index_data.get(id).map_or_else(|| 0, |data| data.len()))
                        .sum(),
                )
            })
            .collect()
    }

    /// Returns statistics about all of the compression types used in the bag.
    pub fn compression_info(&self) -> Vec<CompressionInfo> {
        let mut acc = HashMap::<&str, CompressionInfo>::new();

        for metadata in self.chunk_metadata.values() {
            let info =
                acc.entry(metadata.compression.as_str())
                    .or_insert_with(|| CompressionInfo {
                        name: metadata.compression.clone(),
                        total_compressed: 0,
                        total_uncompressed: 0,
                        chunk_count: 0,
                    });
            info.chunk_count += 1;
            info.total_compressed += metadata.compressed_size as usize;
            info.total_uncompressed += metadata.uncompressed_size as usize;
        }

        acc.into_values()
            .sorted_by(|a, b| b.total_compressed.cmp(&a.total_compressed))
            .collect()
    }

    pub fn topics(&self) -> Vec<&str> {
        self.connection_data
            .values()
            .map(|d| d.topic.as_ref())
            .unique()
            .collect()
    }

    pub fn topics_and_types(&self) -> HashSet<(&str, &str)> {
        self.connection_data
            .values()
            .map(|data| (&*data.topic, &*data.data_type))
            .collect()
    }

    pub fn types(&self) -> HashSet<&str> {
        self.connection_data
            .values()
            .map(|data| data.data_type.as_str())
            .collect()
    }
}

fn parse_bag_header<R: Read + Seek>(
    header_buf: &[u8],
    reader: &mut R,
) -> Result<BagHeader, ParseError> {
    let bag_header = BagHeader::from(header_buf)?;

    if bag_header.index_pos == 0 {
        return Err(ParseError::UnindexedBag);
    }

    let data_len = read_le_u32(reader).ok_or_else(|| ParseError::UnexpectedEOF)?;
    // Skip bag header padding
    reader
        .seek(io::SeekFrom::Current(data_len as i64))
        .map_err(|_e| {
            eprintln!("could not seek {data_len} bytes");
            ParseError::BufferTooSmall
        })?;

    Ok(bag_header)
}

fn parse_connection<R: Read + Seek>(
    header_buf: &[u8],
    reader: &mut R,
) -> Result<ConnectionData, ParseError> {
    let connection_header = ConnectionHeader::from(header_buf)?;
    let data = get_lengthed_bytes(reader)?;
    ConnectionData::from(
        &data,
        connection_header.connection_id,
        connection_header.topic,
    )
}

fn parse_chunk<R: Read + Seek>(
    header_buf: &[u8],
    reader: &mut R,
    chunk_header_pos: u64,
) -> Result<ChunkHeader, ParseError> {
    let data_len = read_le_u32(reader).ok_or_else(|| ParseError::UnexpectedEOF)?;
    let chunk_data_pos = reader.stream_position().unwrap();

    let chunk_header = ChunkHeader::from(header_buf, chunk_header_pos, chunk_data_pos, data_len)?;

    // skip reading the chunk
    reader
        .seek(io::SeekFrom::Current(data_len as i64))
        .map_err(|_e| {
            eprintln!("could not seek {data_len} bytes");
            ParseError::UnexpectedEOF
        })?;
    Ok(chunk_header)
}

fn parse_chunk_info<R: Read + Seek>(
    header_buf: &[u8],
    reader: &mut R,
) -> Result<(ChunkInfoHeader, Vec<ChunkInfoData>), ParseError> {
    let chunk_info_header = ChunkInfoHeader::from(header_buf)?;
    let data = get_lengthed_bytes(reader)?;

    let chunk_info_data: Vec<ChunkInfoData> = data
        .windows(8)
        .step_by(8)
        .flat_map(ChunkInfoData::from)
        .collect();

    if chunk_info_data.len() != chunk_info_header.connection_count as usize {
        eprintln!("missing chunk info data");
        return Err(ParseError::MissingRecord);
    }

    Ok((chunk_info_header, chunk_info_data))
}

fn parse_index<R: Read + Seek>(
    header_buf: &[u8],
    reader: &mut R,
    chunk_header_pos: u64,
) -> Result<(ConnectionID, Vec<IndexData>), ParseError> {
    let index_data_header = IndexDataHeader::from(header_buf)?;
    let data = get_lengthed_bytes(reader)?;

    let index_data: Vec<IndexData> = data
        .windows(12)
        .step_by(12)
        .flat_map(|buf| IndexData::from(buf, chunk_header_pos, index_data_header.connection_id))
        .collect();

    if index_data.len() != index_data_header.count as usize {
        eprintln!("missing index data");
        return Err(ParseError::MissingRecord);
    }

    Ok((index_data_header.connection_id, index_data))
}

fn parse_records<R: Read + Seek>(
    reader: &mut R,
) -> Result<
    (
        BTreeMap<ChunkHeaderLoc, ChunkMetadata>,
        BTreeMap<ConnectionID, ConnectionData>,
        BTreeMap<ConnectionID, Vec<IndexData>>,
    ),
    ParseError,
> {
    let mut bag_header: Option<BagHeader> = None;
    let mut chunk_headers: Vec<ChunkHeader> = Vec::new();
    let mut chunk_infos: Vec<(ChunkInfoHeader, Vec<ChunkInfoData>)> = Vec::new();
    let mut connections: Vec<ConnectionData> = Vec::new();
    let mut index_data: BTreeMap<ConnectionID, Vec<IndexData>> = BTreeMap::new();

    let mut last_chunk_header_pos = None;

    loop {
        let Some(header_len) = read_le_u32(reader) else {
            break;
        };

        // TODO: benchmark and compare reading into a map or stack-local map crate
        let mut header_buf = vec![0u8; header_len as usize];
        reader.read_exact(&mut header_buf).map_err(|e| {
            eprintln!("{e}");
            ParseError::BufferTooSmall
        })?;

        let op = read_header_op(&header_buf)?;

        match op {
            OpCode::BagHeader => {
                bag_header = Some(parse_bag_header(&header_buf, reader)?);
            }
            OpCode::ChunkHeader => {
                let chunk_header_pos =
                    reader.stream_position().unwrap() - header_buf.len() as u64 - 4; // subtract header and header len
                let chunk_header = parse_chunk(&header_buf, reader, chunk_header_pos)?;
                last_chunk_header_pos = Some(chunk_header_pos);
                chunk_headers.push(chunk_header);
            }
            OpCode::IndexDataHeader => {
                let chunk_header_pos = last_chunk_header_pos.ok_or_else(|| {
                    eprintln!("expected a Chunk before reading IndexData");
                    ParseError::InvalidBag
                })?;
                let (connection_id, mut data) = parse_index(&header_buf, reader, chunk_header_pos)?;
                index_data
                    .entry(connection_id)
                    .or_insert_with(Vec::new)
                    .append(&mut data);
            }
            OpCode::ConnectionHeader => {
                connections.push(parse_connection(&header_buf, reader)?);
            }
            OpCode::ChunkInfoHeader => {
                chunk_infos.push(parse_chunk_info(&header_buf, reader)?);
            }
            OpCode::MessageData => {
                eprintln!("unexpected `MessageData` op at the record level");
                return Err(ParseError::InvalidOpCode);
            }
        }
    }

    let bag_header = bag_header.ok_or_else(|| {
        eprintln!("missing BagHeader");
        ParseError::InvalidBag
    })?;

    if bag_header.chunk_count as usize != chunk_headers.len() {
        eprintln!(
            "missing chunks - expected {}, found {}",
            bag_header.chunk_count,
            chunk_headers.len()
        );
        return Err(ParseError::InvalidBag);
    }
    if bag_header.chunk_count as usize != chunk_infos.len() {
        eprintln!(
            "missing chunk information headers - expected {}, found {}",
            bag_header.chunk_count,
            chunk_infos.len()
        );
        return Err(ParseError::InvalidBag);
    }
    if bag_header.conn_count as usize != connections.len() {
        eprintln!(
            "missing connections - expected {}, found {}",
            bag_header.conn_count,
            connections.len()
        );
        return Err(ParseError::InvalidBag);
    }

    let chunk_metadata: BTreeMap<ChunkHeaderLoc, ChunkMetadata> = chunk_headers
        .into_iter()
        .flat_map(|chunk_header| {
            chunk_infos
                .iter()
                .find(|(chunk_info_header, _)| {
                    chunk_header.chunk_header_pos == chunk_info_header.chunk_header_pos
                })
                .map(|(chunk_info_header, chunk_data)| ChunkMetadata {
                    compression: chunk_header.compression,
                    uncompressed_size: chunk_header.uncompressed_size,
                    compressed_size: chunk_header.compressed_size,
                    chunk_header_pos: chunk_header.chunk_header_pos,
                    chunk_data_pos: chunk_header.chunk_data_pos,
                    start_time: chunk_info_header.start_time,
                    end_time: chunk_info_header.end_time,
                    connection_count: chunk_info_header.connection_count,
                    message_counts: chunk_data
                        .iter()
                        .map(|data| (data.connection_id, data.count))
                        .collect::<BTreeMap<ConnectionID, u32>>(),
                })
        })
        .map(|metadata| (metadata.chunk_header_pos, metadata))
        .collect();
    let connection_data: BTreeMap<ConnectionID, ConnectionData> = connections
        .into_iter()
        .map(|data| (data.connection_id, data))
        .collect();
    Ok((chunk_metadata, connection_data, index_data))
}

#[inline(always)]
fn read_header_op(buf: &[u8]) -> Result<OpCode, ParseError> {
    let mut i = 0;
    loop {
        let (new_index, name, value) = parse_field(buf, i)?;
        i = new_index;

        if name == b"op" {
            let op = util::parsing::parse_u8(value)?;
            return OpCode::from(op);
        }

        if i >= buf.len() {
            break;
        }
    }
    Err(ParseError::MissingHeaderOp)
}

impl DecompressedBag {
    /// Creates a bag from a vector of bytes.
    /// This will copy the bytes even if it is a decompressed bag.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        let mut reader = Cursor::new(&bytes);

        let version: String = version_check(&mut reader)?;
        let (chunk_metadata, connection_data, index_data) = parse_records(&mut reader)?;

        let chunk_bytes = populate_chunk_bytes(&chunk_metadata, bytes)?;

        Ok(DecompressedBag {
            metadata: BagMetadata {
                version,
                file_path: None,
                chunk_metadata,
                connection_data,
                index_data,
                num_bytes: bytes.len() as u64,
            },
            chunk_bytes,
        })
    }

    pub fn from_file<P>(file_path: P) -> Result<Self, Error>
    where
        P: AsRef<Path> + Into<PathBuf>,
    {
        let path: PathBuf = file_path.as_ref().into();
        let file = File::open(file_path)?;

        let mut reader = BufReader::new(file);

        let mut bytes = Vec::<u8>::new();
        reader.read_to_end(&mut bytes)?;

        let mut bag = Self::from_bytes(&bytes)?;
        bag.metadata.file_path = Some(path);

        Ok(bag)
    }

    pub fn read_messages(&self, query: &Query) -> Result<BagIter, Error> {
        BagIter::new(self, query)
    }
}

fn populate_chunk_bytes(
    chunk_metadata: &BTreeMap<u64, ChunkMetadata>,
    bag_bytes: &[u8],
) -> Result<BTreeMap<ChunkHeaderLoc, Vec<u8>>, Error> {
    let mut chunk_bytes = BTreeMap::new();
    //TODO: parallelization
    for (chunk_loc, metadata) in chunk_metadata.iter() {
        let chunk_start = metadata.chunk_data_pos as usize;
        let chunk_end = chunk_start + metadata.compressed_size as usize;
        let buf = &bag_bytes[chunk_start..chunk_end];

        match metadata.compression.as_str() {
            "none" => {
                chunk_bytes.insert(*chunk_loc, buf.to_vec());
            }
            "lz4" => {
                // TODO: figure out what are these bytes I'm removing..
                let decompressed = lz4_flex::decompress(
                    &buf[11..(buf.len() - 8)],
                    metadata.uncompressed_size as usize,
                )?;
                chunk_bytes.insert(*chunk_loc, decompressed);
            }
            other => {
                eprintln!("unsupported compression: {}", other);
                return Err(Error::from(ParseError::InvalidBag));
            }
        }
    }
    Ok(chunk_bytes)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::{field_sep_index, version_check};

    const DECOMPRESSED: &[u8] = include_bytes!("../tests/fixtures/decompressed.bag");

    #[test]
    fn test_version_check() {
        let mut reader = Cursor::new(DECOMPRESSED);
        assert!(version_check(&mut reader).is_ok())
    }

    #[test]
    fn test_field_sep_position() {
        let buf = b"hello=banana";
        assert_eq!(field_sep_index(buf).unwrap(), 5);
        assert_eq!(field_sep_index(&buf[2..8]).unwrap(), 3);

        let buf = b"theresnosep";
        assert!(field_sep_index(buf).is_err());
    }
}

#![allow(dead_code)]
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;
use std::io::{self, prelude::*, BufReader, Cursor};
use std::path::{Path, PathBuf};
use std::time::Duration;

type ConnectionID = u32;
type ChunkHeaderLoc = u64;

use errors::{Error, ErrorKind};

use itertools::Itertools;
pub use util::msgs;
use util::parsing::get_lengthed_bytes;
pub use util::query;
pub use util::time;

pub mod errors;
mod util;
use util::query::{BagIter, Query};
use util::time::Time;

pub struct Bag<R: Read + Seek> {
    pub file_path: Option<PathBuf>,
    reader: R,
    pub version: String,
    pub(crate) chunk_metadata: BTreeMap<ChunkHeaderLoc, ChunkMetadata>,
    pub(crate) chunk_bytes: BTreeMap<ChunkHeaderLoc, Vec<u8>>,
    pub connection_data: BTreeMap<ConnectionID, ConnectionData>,
    pub(crate) index_data: BTreeMap<ConnectionID, Vec<IndexData>>,
    pub size: u64,
}

#[derive(Debug)]
pub struct CompressionInfo {
    pub name: String,
    pub chunk_count: usize,
    pub total_compressed: usize,
    pub total_uncompressed: usize,
}

#[derive(Debug)]
#[repr(u8)]
pub enum OpCode {
    BagHeader = 0x03,
    ChunkHeader = 0x05,
    ConnectionHeader = 0x07,
    MessageData = 0x02,
    IndexDataHeader = 0x04,
    ChunkInfoHeader = 0x06,
}

impl OpCode {
    fn from(byte: u8) -> Result<OpCode, Error> {
        match byte {
            0x03 => Ok(OpCode::BagHeader),
            0x05 => Ok(OpCode::ChunkHeader),
            0x07 => Ok(OpCode::ConnectionHeader),
            0x02 => Ok(OpCode::MessageData),
            0x04 => Ok(OpCode::IndexDataHeader),
            0x06 => Ok(OpCode::ChunkInfoHeader),
            _ => Err(Error::new(ErrorKind::InvalidBag(Cow::Owned(format!(
                "invalid op code: {:x}",
                byte
            ))))),
        }
    }
}

#[inline(always)]
fn read_le_u32(reader: &mut impl Read) -> io::Result<u32> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf)?;
    Ok(u32::from_le_bytes(len_buf))
}

#[inline(always)]
fn field_sep_index(buf: &[u8]) -> Result<usize, Error> {
    buf.iter().position(|&b| b == b'=').ok_or_else(|| {
        Error::new(ErrorKind::InvalidBag(Cow::Borrowed(
            "missing field separator",
        )))
    })
}

#[inline(always)]
fn parse_field(buf: &[u8], i: usize) -> Result<(usize, &[u8], &[u8]), Error> {
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
    fn from(buf: &[u8]) -> Result<BagHeader, Error> {
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
                        return Err(Error::new(ErrorKind::InvalidBag(Cow::Borrowed(
                            "expected BagHeader op",
                        ))));
                    }
                }
                other => {
                    return Err(Error::new(ErrorKind::InvalidBag(Cow::Owned(format!(
                        "unexpected field: {}",
                        String::from_utf8_lossy(other)
                    )))));
                }
            }

            if i >= buf.len() {
                break;
            }
        }

        Ok(BagHeader {
            index_pos: index_pos.ok_or_else(|| {
                Error::new(ErrorKind::InvalidBag(Cow::Borrowed(
                    "expected index_pos in BagHeader",
                )))
            })?,
            conn_count: conn_count.ok_or_else(|| {
                Error::new(ErrorKind::InvalidBag(Cow::Borrowed(
                    "expected conn_count in BagHeader",
                )))
            })?,
            chunk_count: chunk_count.ok_or_else(|| {
                Error::new(ErrorKind::InvalidBag(Cow::Borrowed(
                    "expected chunk_count in BagHeader",
                )))
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
    ) -> Result<ChunkHeader, Error> {
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
                        return Err(Error::new(ErrorKind::InvalidBag(Cow::Borrowed(
                            "expected 'ChunkHeader' op",
                        ))));
                    }
                }
                other => {
                    return Err(Error::new(ErrorKind::InvalidBag(Cow::Owned(format!(
                        "unexpected field: {}",
                        String::from_utf8_lossy(other)
                    )))));
                }
            }

            if i >= buf.len() {
                break;
            }
        }

        Ok(ChunkHeader {
            compression: compression.ok_or_else(|| {
                Error::new(ErrorKind::InvalidBag(Cow::Borrowed(
                    "expected 'compression' in 'ChunkHeader'",
                )))
            })?,
            uncompressed_size: size.ok_or_else(|| {
                Error::new(ErrorKind::InvalidBag(Cow::Borrowed(
                    "expected 'uncompressed_size' in 'ChunkHeader'",
                )))
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
    fn from(buf: &[u8]) -> Result<ChunkInfoHeader, Error> {
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
                        return Err(Error::new(ErrorKind::InvalidBag(Cow::Borrowed(
                            "expected 'ChunkInfoHeader' op",
                        ))));
                    }
                }
                other => {
                    return Err(Error::new(ErrorKind::InvalidBag(Cow::Owned(format!(
                        "unexpected field: {}",
                        String::from_utf8_lossy(other)
                    )))));
                }
            }

            if i >= buf.len() {
                break;
            }
        }

        Ok(ChunkInfoHeader {
            version: version.ok_or_else(|| {
                Error::new(ErrorKind::InvalidBag(Cow::Borrowed(
                    "expected 'version' in 'ChunkInfoHeader'",
                )))
            })?,
            chunk_header_pos: chunk_header_pos.ok_or_else(|| {
                Error::new(ErrorKind::InvalidBag(Cow::Borrowed(
                    "expected 'chunk_header_pos' in 'ChunkInfoHeader'",
                )))
            })?,
            start_time: start_time.ok_or_else(|| {
                Error::new(ErrorKind::InvalidBag(Cow::Borrowed(
                    "expected 'start_time' in 'ChunkInfoHeader'",
                )))
            })?,
            end_time: end_time.ok_or_else(|| {
                Error::new(ErrorKind::InvalidBag(Cow::Borrowed(
                    "expected 'end_time' in 'ChunkInfoHeader'",
                )))
            })?,
            connection_count: connection_count.ok_or_else(|| {
                Error::new(ErrorKind::InvalidBag(Cow::Borrowed(
                    "expected 'connection_count' in 'ChunkInfoHeader'",
                )))
            })?,
        })
    }
}

struct ChunkInfoData {
    connection_id: ConnectionID,
    count: u32,
}

impl ChunkInfoData {
    fn from(buf: &[u8]) -> io::Result<ChunkInfoData> {
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
    fn from(buf: &[u8]) -> Result<ConnectionHeader, Error> {
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
                        return Err(Error::new(ErrorKind::InvalidBag(Cow::Borrowed(
                            "expected 'ConnectionHeader' op",
                        ))));
                    }
                }
                other => {
                    return Err(Error::new(ErrorKind::InvalidBag(Cow::Owned(format!(
                        "unexpected field: {}",
                        String::from_utf8_lossy(other)
                    )))));
                }
            }

            if i >= buf.len() {
                break;
            }
        }

        Ok(ConnectionHeader {
            connection_id: connection_id.ok_or_else(|| {
                Error::new(ErrorKind::InvalidBag(Cow::Borrowed(
                    "expected 'connection_id' in 'ConnectionHeader'",
                )))
            })?,
            topic: topic.ok_or_else(|| {
                Error::new(ErrorKind::InvalidBag(Cow::Borrowed(
                    "expected 'topic' in 'ConnectionHeader'",
                )))
            })?,
        })
    }
}

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
    fn from(buf: &[u8], connection_id: u32, topic: String) -> Result<ConnectionData, Error> {
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
                    return Err(Error::new(ErrorKind::InvalidBag(Cow::Owned(format!(
                        "unexpected field: {}",
                        String::from_utf8_lossy(other)
                    )))));
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
                Error::new(ErrorKind::InvalidBag(Cow::Borrowed(
                    "expected 'data_type' in 'ConnectionData'",
                )))
            })?,
            md5sum: md5sum.ok_or_else(|| {
                Error::new(ErrorKind::InvalidBag(Cow::Borrowed(
                    "expected 'md5sum' in 'ConnectionData'",
                )))
            })?,
            message_definition: message_definition.ok_or_else(|| {
                Error::new(ErrorKind::InvalidBag(Cow::Borrowed(
                    "expected 'message_definition' in 'ConnectionData'",
                )))
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
    fn from(buf: &[u8]) -> Result<IndexDataHeader, Error> {
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
                        return Err(Error::new(ErrorKind::InvalidBag(Cow::Borrowed(
                            "expected 'IndexDataHeader' op",
                        ))));
                    }
                }
                other => {
                    return Err(Error::new(ErrorKind::InvalidBag(Cow::Owned(format!(
                        "unexpected field: {}",
                        String::from_utf8_lossy(other)
                    )))));
                }
            }

            if i >= buf.len() {
                break;
            }
        }

        Ok(IndexDataHeader {
            version: version.ok_or_else(|| {
                Error::new(ErrorKind::InvalidBag(Cow::Borrowed(
                    "expected 'version' in 'IndexDataHeader'",
                )))
            })?,
            connection_id: connection_id.ok_or_else(|| {
                Error::new(ErrorKind::InvalidBag(Cow::Borrowed(
                    "expected 'connection_id' in 'IndexDataHeader'",
                )))
            })?,
            count: count.ok_or_else(|| {
                Error::new(ErrorKind::InvalidBag(Cow::Borrowed(
                    "expected 'count' in 'IndexDataHeader'",
                )))
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
    fn from(buf: &[u8], chunk_header_pos: u64, conn_id: ConnectionID) -> io::Result<IndexData> {
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
    fn from(buf: &[u8]) -> Result<MessageDataHeader, Error> {
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
                        return Err(Error::new(ErrorKind::InvalidBag(Cow::Borrowed(
                            "expected 'MessageDataHeader' op",
                        ))));
                    }
                }
                other => {
                    return Err(Error::new(ErrorKind::InvalidBag(Cow::Owned(format!(
                        "unexpected field: {}",
                        String::from_utf8_lossy(other)
                    )))));
                }
            }

            if i >= buf.len() {
                break;
            }
        }

        Ok(MessageDataHeader {
            conn: conn.ok_or_else(|| {
                Error::new(ErrorKind::InvalidBag(Cow::Borrowed(
                    "expected 'conn' in 'MessageDataHeader'",
                )))
            })?,
            time: time.ok_or_else(|| {
                Error::new(ErrorKind::InvalidBag(Cow::Borrowed(
                    "expected 'time' in 'MessageDataHeader'",
                )))
            })?,
        })
    }
}

impl Bag<BufReader<File>> {
    pub fn from<P>(file_path: P) -> Result<Self, Error>
    where
        P: AsRef<Path> + Into<PathBuf>,
    {
        let path: PathBuf = file_path.as_ref().into();
        let file = File::open(file_path)?;
        let file_size = file.metadata()?.len();

        let reader = BufReader::new(file);

        let mut bag = Self::from_reader(reader)?;
        bag.file_path = Some(path);
        bag.size = file_size;
        Ok(bag)
    }
}

impl<'a> Bag<Cursor<&'a [u8]>> {
    pub fn from_bytes(bytes: &'a [u8]) -> Result<Self, Error> {
        let reader = Cursor::new(bytes);
        let mut bag = Self::from_reader(reader)?;
        bag.size = bytes.len() as u64;
        Ok(bag)
    }
}

impl<R: Read + Seek> Bag<R> {
    fn from_reader(mut reader: R) -> Result<Bag<R>, Error> {
        let version = version_check(&mut reader)?;

        let (chunk_metadata, connection_data, index_data) = Bag::parse_records(&mut reader)?;

        Ok(Bag {
            version,
            file_path: None,
            reader,
            chunk_metadata,
            chunk_bytes: BTreeMap::new(),
            connection_data,
            index_data,
            size: 0, // will be set in constructor
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

    pub fn read_messages(&mut self, query: &Query) -> Result<BagIter<R>, Error> {
        BagIter::new(self, query)
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

    pub fn topic_message_count(&self, topic: &str) -> Option<usize> {
        self.topic_to_connection_ids().get(topic).map(|conn_ids| {
            conn_ids
                .iter()
                .map(|id| self.index_data.get(id).map_or_else(|| 0, |data| data.len()))
                .sum()
        })
    }

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

    pub fn topics_and_types(&self) -> HashSet<(&String, &String)> {
        self.connection_data
            .values()
            .map(|data| (&data.topic, &data.data_type))
            .collect()
    }

    pub fn types(&self) -> HashSet<&str> {
        self.connection_data
            .values()
            .map(|data| data.data_type.as_str())
            .collect()
    }

    pub(crate) fn populate_chunk_bytes(&mut self) -> Result<(), Error> {
        if !self.chunk_bytes.is_empty() {
            return Ok(());
        }

        //TODO: compressed bags, parallelization
        for (chunk_loc, metadata) in self.chunk_metadata.iter() {
            let mut buf = vec![0u8; metadata.compressed_size as usize];
            self.reader
                .seek(std::io::SeekFrom::Start(metadata.chunk_data_pos))?;
            self.reader.read_exact(&mut buf[..])?;

            match metadata.compression.as_str() {
                "none" => {
                    self.chunk_bytes.insert(*chunk_loc, buf);
                }
                "lz4" => {
                    // TODO: figure out what are these bytes I'm removing..
                    let decompressed = lz4_flex::decompress(
                        &buf[11..(buf.len() - 8)],
                        metadata.uncompressed_size as usize,
                    )?;
                    self.chunk_bytes.insert(*chunk_loc, decompressed);
                }
                other => {
                    return Err(Error::new(ErrorKind::InvalidBag(Cow::Owned(format!(
                        "unsupported compression: {}",
                        other
                    )))))
                }
            }
        }
        Ok(())
    }

    fn parse_bag_header(header_buf: &[u8], reader: &mut R) -> Result<BagHeader, Error> {
        let bag_header = BagHeader::from(header_buf)?;

        if bag_header.index_pos == 0 {
            return Err(Error::new(ErrorKind::UnindexedBag));
        }

        let data_len = read_le_u32(reader)?;
        reader.seek(io::SeekFrom::Current(data_len as i64))?; // Skip bag header padding

        Ok(bag_header)
    }

    fn parse_connection(header_buf: &[u8], reader: &mut R) -> Result<ConnectionData, Error> {
        let connection_header = ConnectionHeader::from(header_buf)?;
        let data = get_lengthed_bytes(reader)?;
        ConnectionData::from(
            &data,
            connection_header.connection_id,
            connection_header.topic,
        )
    }

    fn parse_chunk(
        header_buf: &[u8],
        reader: &mut R,
        chunk_header_pos: u64,
    ) -> Result<ChunkHeader, Error> {
        let data_len = read_le_u32(reader)?;
        let chunk_data_pos = reader.stream_position()?;

        let chunk_header =
            ChunkHeader::from(header_buf, chunk_header_pos, chunk_data_pos, data_len)?;

        reader.seek(io::SeekFrom::Current(data_len as i64))?; // skip reading the chunk
        Ok(chunk_header)
    }

    fn parse_chunk_info(
        header_buf: &[u8],
        reader: &mut R,
    ) -> Result<(ChunkInfoHeader, Vec<ChunkInfoData>), Error> {
        let chunk_info_header = ChunkInfoHeader::from(header_buf)?;
        let data = get_lengthed_bytes(reader)?;

        let chunk_info_data: Vec<ChunkInfoData> = data
            .windows(8)
            .step_by(8)
            .flat_map(ChunkInfoData::from)
            .collect();

        if chunk_info_data.len() != chunk_info_header.connection_count as usize {
            return Err(Error::new(ErrorKind::InvalidBag(Cow::Borrowed(
                "missing chunk info data",
            ))));
        }

        Ok((chunk_info_header, chunk_info_data))
    }

    fn parse_index(
        header_buf: &[u8],
        reader: &mut R,
        chunk_header_pos: u64,
    ) -> Result<(ConnectionID, Vec<IndexData>), Error> {
        let index_data_header = IndexDataHeader::from(header_buf)?;
        let data = get_lengthed_bytes(reader)?;

        let index_data: Vec<IndexData> = data
            .windows(12)
            .step_by(12)
            .flat_map(|buf| IndexData::from(buf, chunk_header_pos, index_data_header.connection_id))
            .collect();

        if index_data.len() != index_data_header.count as usize {
            return Err(Error::new(ErrorKind::InvalidBag(Cow::Borrowed(
                "missing index data",
            ))));
        }

        Ok((index_data_header.connection_id, index_data))
    }

    fn parse_records(
        reader: &mut R,
    ) -> Result<
        (
            BTreeMap<ChunkHeaderLoc, ChunkMetadata>,
            BTreeMap<ConnectionID, ConnectionData>,
            BTreeMap<ConnectionID, Vec<IndexData>>,
        ),
        Error,
    > {
        let mut bag_header: Option<BagHeader> = None;
        let mut chunk_headers: Vec<ChunkHeader> = Vec::new();
        let mut chunk_infos: Vec<(ChunkInfoHeader, Vec<ChunkInfoData>)> = Vec::new();
        let mut connections: Vec<ConnectionData> = Vec::new();
        let mut index_data: BTreeMap<ConnectionID, Vec<IndexData>> = BTreeMap::new();

        let mut last_chunk_header_pos = None;

        loop {
            let maybe_header_len = read_le_u32(reader);
            if let Err(e) = maybe_header_len {
                match e.kind() {
                    io::ErrorKind::UnexpectedEof => break,
                    _ => return Err(Error::new(ErrorKind::Io(e))),
                }
            }
            let header_len = maybe_header_len.unwrap();

            // TODO: benchmark and compare reading into a map or stack-local map crate
            let mut header_buf = vec![0u8; header_len as usize];
            reader.read_exact(&mut header_buf)?;

            let op = read_header_op(&header_buf)?;

            match op {
                OpCode::BagHeader => {
                    bag_header = Some(Bag::parse_bag_header(&header_buf, reader)?);
                }
                OpCode::ChunkHeader => {
                    let chunk_header_pos = reader.stream_position()? - header_buf.len() as u64 - 4; // subtract header and header len
                    let chunk_header = Bag::parse_chunk(&header_buf, reader, chunk_header_pos)?;
                    last_chunk_header_pos = Some(chunk_header_pos);
                    chunk_headers.push(chunk_header);
                }
                OpCode::IndexDataHeader => {
                    let chunk_header_pos = last_chunk_header_pos.ok_or_else(|| {
                        Error::new(ErrorKind::InvalidBag(Cow::Borrowed(
                            "Expected a Chunk before reading IndexData",
                        )))
                    })?;
                    let (connection_id, mut data) =
                        Bag::parse_index(&header_buf, reader, chunk_header_pos)?;
                    index_data
                        .entry(connection_id)
                        .or_insert_with(Vec::new)
                        .append(&mut data);
                }
                OpCode::ConnectionHeader => {
                    connections.push(Bag::parse_connection(&header_buf, reader)?);
                }
                OpCode::ChunkInfoHeader => {
                    chunk_infos.push(Bag::parse_chunk_info(&header_buf, reader)?);
                }
                OpCode::MessageData => {
                    return Err(Error::new(ErrorKind::InvalidBag(Cow::Borrowed(
                        "unexpected `MessageData` op at the record level",
                    ))))
                }
            }
        }

        let bag_header = bag_header
            .ok_or_else(|| Error::new(ErrorKind::InvalidBag(Cow::Borrowed("Missing BagHeader"))))?;
        if bag_header.chunk_count as usize != chunk_headers.len() {
            return Err(Error::new(ErrorKind::InvalidBag(Cow::Owned(format!(
                "missing chunks - expected {}, found {}",
                bag_header.chunk_count,
                chunk_headers.len()
            )))));
        }
        if bag_header.chunk_count as usize != chunk_infos.len() {
            return Err(Error::new(ErrorKind::InvalidBag(Cow::Owned(format!(
                "missing chunk information headers - expected {}, found {}",
                bag_header.chunk_count,
                chunk_infos.len()
            )))));
        }
        if bag_header.conn_count as usize != connections.len() {
            return Err(Error::new(ErrorKind::InvalidBag(Cow::Owned(format!(
                "missing connections - expected {}, found {}",
                bag_header.conn_count,
                connections.len()
            )))));
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
}

#[inline(always)]
fn read_header_op(buf: &[u8]) -> Result<OpCode, Error> {
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
    Err(Error::new(ErrorKind::InvalidBag(Cow::Borrowed(
        "Missing header op",
    ))))
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

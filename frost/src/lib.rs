#![allow(dead_code)]
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, prelude::*, BufReader, ErrorKind};
use std::path::{Path, PathBuf};
use std::time::Duration;

type ConnectionID = u32;
type ChunkHeaderLoc = u64;

mod util;
use util::msgs::Time;
use util::query::{BagIter, Query};
use util::time;

pub struct Bag {
    pub file_path: PathBuf,
    pub version: String,
    chunk_metadata: BTreeMap<ChunkHeaderLoc, ChunkMetadata>,
    chunk_bytes: BTreeMap<ChunkHeaderLoc, Vec<u8>>,
    pub connection_data: BTreeMap<ConnectionID, ConnectionData>,
    pub(crate) index_data: BTreeMap<ConnectionID, Vec<IndexData>>,
    topic_to_connection_ids: BTreeMap<String, Vec<ConnectionID>>,
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
    fn from(byte: u8) -> io::Result<OpCode> {
        match byte {
            0x03 => Ok(OpCode::BagHeader),
            0x05 => Ok(OpCode::ChunkHeader),
            0x07 => Ok(OpCode::ConnectionHeader),
            0x02 => Ok(OpCode::MessageData),
            0x04 => Ok(OpCode::IndexDataHeader),
            0x06 => Ok(OpCode::ChunkInfoHeader),
            other => Err(io::Error::new(
                ErrorKind::InvalidInput,
                format!("Unknown op code {:#04x}", other),
            )),
        }
    }
}

fn read_le_u32<R: Read + Seek>(reader: &mut R) -> io::Result<u32> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf)?;
    Ok(u32::from_le_bytes(len_buf))
}

fn field_sep_index(buf: &[u8]) -> io::Result<usize> {
    buf.iter().position(|&b| b == b'=').ok_or_else(|| {
        io::Error::new(
            ErrorKind::InvalidData,
            format!("Expected '=' in buffer: {:?}", &buf),
        )
    })
}

fn parse_field(buf: &[u8], i: usize) -> io::Result<(usize, &[u8], &[u8])> {
    let mut i = i;
    let field_len = util::parsing::parse_le_u32_at(buf, i)? as usize;
    i += 4;
    let sep_pos = i + field_sep_index(&buf[i..i + field_len])?;

    let name = &buf[i..sep_pos];
    let value = &buf[(sep_pos + 1)..(i + field_len)];

    i += field_len;
    Ok((i, name, value))
}

#[derive(Debug)]
struct BagHeader {
    index_pos: u64,
    conn_count: u32,
    chunk_count: u32,
}

impl BagHeader {
    fn from(buf: &[u8]) -> io::Result<BagHeader> {
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
                        return Err(io::Error::new(
                            ErrorKind::InvalidData,
                            format!("Expected op {:?}, found {:?}", OpCode::BagHeader, op),
                        ));
                    }
                }
                _ => {
                    return Err(io::Error::new(
                        ErrorKind::InvalidData,
                        format!(
                            "Unexpected field {} in BagHeader",
                            String::from_utf8_lossy(name)
                        ),
                    ))
                }
            }

            if i >= buf.len() {
                break;
            }
        }

        Ok(BagHeader {
            index_pos: index_pos.ok_or_else(|| {
                io::Error::new(
                    ErrorKind::InvalidData,
                    "Missing field 'index_pos' in BagHeader",
                )
            })?,
            conn_count: conn_count.ok_or_else(|| {
                io::Error::new(
                    ErrorKind::InvalidData,
                    "Missing field 'conn_count' in BagHeader",
                )
            })?,
            chunk_count: chunk_count.ok_or_else(|| {
                io::Error::new(
                    ErrorKind::InvalidData,
                    "Missing field 'chunk_count' in BagHeader",
                )
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
    ) -> io::Result<ChunkHeader> {
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
                        return Err(io::Error::new(
                            ErrorKind::InvalidData,
                            format!("Expected op {:?}, found {:?}", OpCode::ChunkHeader, op),
                        ));
                    }
                }
                _ => {
                    return Err(io::Error::new(
                        ErrorKind::InvalidData,
                        format!(
                            "Unexpected field {} in ChunkHeader",
                            String::from_utf8_lossy(name)
                        ),
                    ))
                }
            }

            if i >= buf.len() {
                break;
            }
        }

        Ok(ChunkHeader {
            compression: compression.ok_or_else(|| {
                io::Error::new(
                    ErrorKind::InvalidData,
                    "Missing field 'compression' in ChunkHeader",
                )
            })?,
            uncompressed_size: size.ok_or_else(|| {
                io::Error::new(
                    ErrorKind::InvalidData,
                    "Missing field 'size' in ChunkHeader",
                )
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
    fn from(buf: &[u8]) -> io::Result<ChunkInfoHeader> {
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
                        return Err(io::Error::new(
                            ErrorKind::InvalidData,
                            format!("Expected op {:?}, found {:?}", OpCode::ChunkInfoHeader, op),
                        ));
                    }
                }
                _ => {
                    return Err(io::Error::new(
                        ErrorKind::InvalidData,
                        format!(
                            "Unexpected field {} in ChunkInfoHeader",
                            String::from_utf8_lossy(name)
                        ),
                    ))
                }
            }

            if i >= buf.len() {
                break;
            }
        }

        Ok(ChunkInfoHeader {
            version: version.ok_or_else(|| {
                io::Error::new(
                    ErrorKind::InvalidData,
                    "Missing field 'ver' in ChunkInfoHeader",
                )
            })?,
            chunk_header_pos: chunk_header_pos.ok_or_else(|| {
                io::Error::new(
                    ErrorKind::InvalidData,
                    "Missing field 'chunk_header_pos' in ChunkInfoHeader",
                )
            })?,
            start_time: start_time.ok_or_else(|| {
                io::Error::new(
                    ErrorKind::InvalidData,
                    "Missing field 'start_time' in ChunkInfoHeader",
                )
            })?,
            end_time: end_time.ok_or_else(|| {
                io::Error::new(
                    ErrorKind::InvalidData,
                    "Missing field 'end_time' in ChunkInfoHeader",
                )
            })?,
            connection_count: connection_count.ok_or_else(|| {
                io::Error::new(
                    ErrorKind::InvalidData,
                    "Missing field 'count' in ChunkInfoHeader",
                )
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
    fn from(buf: &[u8]) -> io::Result<ConnectionHeader> {
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
                        return Err(io::Error::new(
                            ErrorKind::InvalidData,
                            format!("Expected op {:?}, found {:?}", OpCode::ConnectionHeader, op),
                        ));
                    }
                }
                _ => {
                    return Err(io::Error::new(
                        ErrorKind::InvalidData,
                        format!(
                            "Unexpected field {} in ConnectionHeader",
                            String::from_utf8_lossy(name)
                        ),
                    ))
                }
            }

            if i >= buf.len() {
                break;
            }
        }

        Ok(ConnectionHeader {
            connection_id: connection_id.ok_or_else(|| {
                io::Error::new(
                    ErrorKind::InvalidData,
                    "Missing field 'connection_id' in ConnectionHeader",
                )
            })?,
            topic: topic.ok_or_else(|| {
                io::Error::new(
                    ErrorKind::InvalidData,
                    "Missing field 'topic' in ConnectionHeader",
                )
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
    fn from(buf: &[u8], connection_id: u32, topic: String) -> io::Result<ConnectionData> {
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
                _ => {
                    return Err(io::Error::new(
                        ErrorKind::InvalidData,
                        format!(
                            "Unexpected field {} in ConnectionData",
                            String::from_utf8_lossy(name)
                        ),
                    ))
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
                io::Error::new(
                    ErrorKind::InvalidData,
                    "Missing field 'data_type' in ConnectionData",
                )
            })?,
            md5sum: md5sum.ok_or_else(|| {
                io::Error::new(
                    ErrorKind::InvalidData,
                    "Missing field 'md5sum' in ConnectionData",
                )
            })?,
            message_definition: message_definition.ok_or_else(|| {
                io::Error::new(
                    ErrorKind::InvalidData,
                    "Missing field 'message_definition' in ConnectionData",
                )
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
    fn from(buf: &[u8]) -> io::Result<IndexDataHeader> {
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
                        return Err(io::Error::new(
                            ErrorKind::InvalidData,
                            format!("Expected op {:?}, found {:?}", OpCode::IndexDataHeader, op),
                        ));
                    }
                }
                _ => {
                    return Err(io::Error::new(
                        ErrorKind::InvalidData,
                        format!(
                            "Unexpected field {} in IndexDataHeader",
                            String::from_utf8_lossy(name)
                        ),
                    ))
                }
            }

            if i >= buf.len() {
                break;
            }
        }

        Ok(IndexDataHeader {
            version: version.ok_or_else(|| {
                io::Error::new(
                    ErrorKind::InvalidData,
                    "Missing field 'ver' in IndexDataHeader",
                )
            })?,
            connection_id: connection_id.ok_or_else(|| {
                io::Error::new(
                    ErrorKind::InvalidData,
                    "Missing field 'conn' in IndexDataHeader",
                )
            })?,
            count: count.ok_or_else(|| {
                io::Error::new(
                    ErrorKind::InvalidData,
                    "Missing field 'count' in IndexDataHeader",
                )
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
    fn from(buf: &[u8]) -> io::Result<MessageDataHeader> {
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
                        return Err(io::Error::new(
                            ErrorKind::InvalidData,
                            format!("Expected op {:?}, found {:?}", OpCode::MessageData, op),
                        ));
                    }
                }
                _ => {
                    return Err(io::Error::new(
                        ErrorKind::InvalidData,
                        format!(
                            "Unexpected field {} in MessageData",
                            String::from_utf8_lossy(name)
                        ),
                    ))
                }
            }

            if i >= buf.len() {
                break;
            }
        }

        Ok(MessageDataHeader {
            conn: conn.ok_or_else(|| {
                io::Error::new(
                    ErrorKind::InvalidData,
                    "Missing field 'conn' in MessageData",
                )
            })?,
            time: time.ok_or_else(|| {
                io::Error::new(
                    ErrorKind::InvalidData,
                    "Missing field 'chunk_header_pos' in MessageData",
                )
            })?,
        })
    }
}

impl Bag {
    pub fn from<P>(file_path: P) -> io::Result<Bag>
    where
        P: AsRef<Path> + Into<PathBuf>,
    {
        let path = file_path.as_ref().into();
        let file = File::open(file_path)?;

        let mut reader = BufReader::new(file);

        let version = Bag::version_check(&mut reader)?;

        let (chunk_metadata, connection_data, index_data) = Bag::parse_records(&mut reader)?;

        let topic_to_ids: BTreeMap<String, Vec<ConnectionID>> =
            connection_data
                .values()
                .fold(BTreeMap::new(), |mut acc, data| {
                    acc.entry(data.topic.clone())
                        .or_insert(Vec::new())
                        .push(data.connection_id);
                    acc
                });

        Ok(Bag {
            version,
            file_path: path,
            chunk_metadata,
            chunk_bytes: BTreeMap::new(),
            connection_data,
            index_data,
            topic_to_connection_ids: topic_to_ids,
        })
    }

    pub fn read_messages(&mut self, query: &Query) -> BagIter {
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
        match self.topic_to_connection_ids.get(topic) {
            Some(conn_ids) => Some(
                conn_ids
                    .iter()
                    .map(|id| self.index_data.get(id).map_or_else(|| 0, |data| data.len()))
                    .sum(),
            ),
            None => None,
        }
    }

    pub fn topics(&self) -> Vec<&String> {
        self.topic_to_connection_ids.keys().collect()
    }

    pub fn topics_and_types(&self) -> Vec<(&String, &String)> {
        self.connection_data
            .values()
            .map(|data| (&data.topic, &data.data_type))
            .collect()
    }

    fn populate_chunk_bytes(&mut self, chunk_loc: ChunkHeaderLoc) {
        let file = File::open(&self.file_path).unwrap();
        let mut reader = BufReader::new(file);
        let metadata = self
            .chunk_metadata
            .get(&chunk_loc)
            .expect(format!("Tried to get a chunk that doesn't exist {}", chunk_loc).as_ref());

        let mut buf = vec![0u8; metadata.uncompressed_size as usize];
        reader
            .seek(std::io::SeekFrom::Start(metadata.chunk_data_pos))
            .expect(format!("Failed to seek to {}", metadata.chunk_data_pos).as_ref());
        reader
            .read_exact(&mut buf[..])
            .expect(format!("Failed to read chunk {} num bytes {}", chunk_loc, buf.len()).as_ref());
        self.chunk_bytes.insert(chunk_loc, buf);
    }
    ///Reads a chunk from disc
    ///
    ///Does not do anything if a chunk has already been read
    pub(crate) fn get_chunk_bytes<'a>(&'a mut self, chunk_loc: ChunkHeaderLoc) -> Vec<u8> {
        if !self.chunk_bytes.contains_key(&chunk_loc) {
            self.populate_chunk_bytes(chunk_loc);
        }
        self.chunk_bytes.get(&chunk_loc).unwrap().clone()
    }

    fn version_check<R: Read + Seek>(reader: &mut R) -> io::Result<String> {
        let mut buf = [0u8; 13];
        let expected = b"#ROSBAG V2.0\n";
        reader.read_exact(&mut buf)?;
        if buf == *expected {
            Ok("2.0".into())
        } else {
            Err(io::Error::new(
                ErrorKind::InvalidData,
                format!(
                    "Got unexpected rosbag version: {}",
                    String::from_utf8_lossy(&buf)
                ),
            ))
        }
    }

    fn get_lengthed_bytes<R: Read + Seek>(reader: &mut R) -> io::Result<Vec<u8>> {
        // Get a vector of bytes from a reader when the first 4 bytes are the length
        // Ex: with <header_len><header> or <data_len><data>, this function returns either header or data
        let mut len_buf = [0u8; 4];
        reader.read_exact(&mut len_buf)?;

        let len = u32::from_le_bytes(len_buf);
        let mut bytes = vec![0u8; len as usize];
        reader.read_exact(&mut bytes)?;

        Ok(bytes)
    }

    fn parse_bag_header<R: Read + Seek>(
        header_buf: &[u8],
        reader: &mut R,
    ) -> io::Result<BagHeader> {
        let bag_header = BagHeader::from(header_buf)?;

        if bag_header.index_pos == 0 {
            return Err(io::Error::new(ErrorKind::InvalidData, "Unindexed bag"));
        }

        let data_len = read_le_u32(reader)?;
        reader.seek(io::SeekFrom::Current(data_len as i64))?; // Skip bag header padding

        Ok(bag_header)
    }

    fn parse_connection<R: Read + Seek>(
        header_buf: &[u8],
        reader: &mut R,
    ) -> io::Result<ConnectionData> {
        let connection_header = ConnectionHeader::from(header_buf)?;
        let data = Bag::get_lengthed_bytes(reader)?;
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
    ) -> io::Result<ChunkHeader> {
        let data_len = read_le_u32(reader)?;
        let chunk_data_pos = reader.stream_position()?;

        let chunk_header =
            ChunkHeader::from(header_buf, chunk_header_pos, chunk_data_pos, data_len)?;

        reader.seek(io::SeekFrom::Current(data_len as i64))?; // skip reading the chunk
        Ok(chunk_header)
    }

    fn parse_chunk_info<R: Read + Seek>(
        header_buf: &[u8],
        reader: &mut R,
    ) -> io::Result<(ChunkInfoHeader, Vec<ChunkInfoData>)> {
        let chunk_info_header = ChunkInfoHeader::from(header_buf)?;
        let data = Bag::get_lengthed_bytes(reader)?;

        let chunk_info_data: Vec<ChunkInfoData> = data
            .windows(8)
            .step_by(8)
            .flat_map(ChunkInfoData::from)
            .collect();

        if chunk_info_data.len() != chunk_info_header.connection_count as usize {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                format!(
                    "Expected {} chunk_info_data, found {}",
                    chunk_info_header.connection_count,
                    chunk_info_data.len()
                ),
            ));
        }

        Ok((chunk_info_header, chunk_info_data))
    }

    fn parse_index<R: Read + Seek>(
        header_buf: &[u8],
        reader: &mut R,
        chunk_header_pos: u64,
    ) -> io::Result<(ConnectionID, Vec<IndexData>)> {
        let index_data_header = IndexDataHeader::from(header_buf)?;
        let data = Bag::get_lengthed_bytes(reader)?;

        let index_data: Vec<IndexData> = data
            .windows(12)
            .step_by(12)
            .flat_map(|buf| IndexData::from(buf, chunk_header_pos, index_data_header.connection_id))
            .collect();

        if index_data.len() != index_data_header.count as usize {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                format!(
                    "Expected {} IndexData, found {}",
                    index_data_header.count,
                    index_data.len()
                ),
            ));
        }

        Ok((index_data_header.connection_id, index_data))
    }

    fn parse_records<R: Read + Seek>(
        reader: &mut R,
    ) -> io::Result<(
        BTreeMap<ChunkHeaderLoc, ChunkMetadata>,
        BTreeMap<ConnectionID, ConnectionData>,
        BTreeMap<ConnectionID, Vec<IndexData>>,
    )> {
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
                    ErrorKind::UnexpectedEof => break,
                    _ => return Err(e),
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
                    let chunk_header_pos = reader.stream_position()? - header_buf.len() as u64 - 4; // substract header and header len
                    let chunk_header = Bag::parse_chunk(&header_buf, reader, chunk_header_pos)?;
                    last_chunk_header_pos = Some(chunk_header_pos);
                    chunk_headers.push(chunk_header);
                }
                OpCode::IndexDataHeader => {
                    let chunk_header_pos = last_chunk_header_pos.ok_or_else(|| {
                        io::Error::new(
                            ErrorKind::InvalidData,
                            "Expected a Chunk before reading IndexData",
                        )
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
                _ => {
                    return Err(io::Error::new(
                        ErrorKind::InvalidData,
                        format!("Unexpected header op code at the record level: {:?}", op),
                    ))
                }
            }
        }

        let bag_header = bag_header
            .ok_or_else(|| io::Error::new(ErrorKind::InvalidData, "Missing BagHeader"))?;
        if bag_header.chunk_count as usize != chunk_headers.len() {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                format!(
                    "Expected {} ChunkHeader, found {}",
                    bag_header.chunk_count,
                    chunk_headers.len()
                ),
            ));
        }
        if bag_header.chunk_count as usize != chunk_infos.len() {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                format!(
                    "Expected {} ChunkInfoHeader, found {}",
                    bag_header.chunk_count,
                    chunk_infos.len()
                ),
            ));
        }
        if bag_header.conn_count as usize != connections.len() {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                format!(
                    "Expected {} Connections, found {}",
                    bag_header.conn_count,
                    connections.len()
                ),
            ));
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

fn read_header_op(buf: &[u8]) -> io::Result<OpCode> {
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
    Err(io::Error::new(
        ErrorKind::InvalidData,
        "No opcode field found",
    ))
}

#[cfg(test)]
mod tests {
    use std::{
        fs::File,
        io::{BufReader, Write},
        path::PathBuf,
    };

    use tempfile::{tempdir, TempDir};

    use crate::{
        field_sep_index,
        util::{
            msgs::{Msg, Time},
            query::Query,
        },
        Bag,
    };

    fn write_test_fixture() -> (TempDir, PathBuf) {
        let bytes = include_bytes!("../tests/fixtures/test.bag");

        let tmp_dir = tempdir().unwrap();
        let file_path = tmp_dir.path().join("test.bag");
        {
            let mut tmp_file = File::create(file_path.clone()).unwrap();
            tmp_file.write(bytes).unwrap();
        }
        (tmp_dir, file_path)
    }
    #[test]
    fn version_check() {
        let (_tmp_dir, file_path) = write_test_fixture();
        let file = File::open(file_path).unwrap();
        let mut reader = BufReader::new(file);
        assert!(Bag::version_check(&mut reader).is_ok())
    }

    #[test]
    fn bag_from() {
        let (_tmp_dir, file_path) = write_test_fixture();
        Bag::from(file_path).unwrap();
    }

    #[test]
    fn bag_iter() {
        let (_tmp_dir, file_path) = write_test_fixture();
        let mut bag = Bag::from(file_path).unwrap();

        let query = Query::all();
        let count = bag.read_messages(&query).count();
        assert_eq!(count, 2000);

        let query = Query::new().with_topics(&vec!["/chatter"]).build();
        let count = bag.read_messages(&query).count();
        assert_eq!(count, 1000);
    }

    #[test]
    fn msg_reading() {
        let (_tmp_dir, file_path) = write_test_fixture();
        let mut bag = Bag::from(file_path).unwrap();

        let query = Query::all();
        let count = bag.read_messages(&query).count();
        assert_eq!(count, 2000);

        impl Msg for String {}

        for msg_view in bag.read_messages(&query) {
            match msg_view.topic.as_str() {
                "/chatter" => {
                    let _msg = msg_view.instantiate::<String>().unwrap();
                }
                "/time" => {
                    let _msg = msg_view.instantiate::<Time>().unwrap();
                }
                &_ => panic!("Test fixture should only have these two"),
            }
        }
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

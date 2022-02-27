#![allow(dead_code)]
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, prelude::*, BufReader, ErrorKind};
use std::path::{Path, PathBuf};

type ConnectionID = u32;

struct Bag {
    file_path: PathBuf,
}

struct Time {
    secs: u32,
    nsecs: u32
}

impl Time {
    fn new(secs: u32, nsecs: u32) -> Time {
        Time { secs, nsecs }
    }
    fn from(buf: &[u8]) -> io::Result<Time> {
        let secs = parse_le_u32(buf)?;
        let nsecs = parse_le_u32_at(buf, 4)?;
        Ok(Time { secs, nsecs })
    }
}

struct Record {
    header_len: u32,
    header: Box<[u8]>,
    data_len: u32,
    data: Box<[u8]>
}

#[derive(Debug)]
struct BagHeader{
    // offset of first record after the chunk section
    index_pos: u64,
    // number of unique connections in the file
    conn_count: u32,
    // number of chunk records in the file 
    chunk_count: u32,
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
            other => Err(io::Error::new(ErrorKind::InvalidInput, format!("Unknown op code {:#04x}", other)))
        }
    }
}

fn parse_u8(buf: &[u8]) -> io::Result<u8>{
    parse_u8_at(buf, 0)
}
fn parse_u8_at(buf: &[u8], index: usize) -> io::Result<u8>{
    let bytes = buf.get(index..index+1)
        .ok_or_else(||io::Error::new(ErrorKind::InvalidInput, "Buffer is not large enough to parse 1 bytes"))?;
    Ok(u8::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_le_u32<R: Read + Seek>(reader: &mut R)-> io::Result<u32> {
    let mut len_buf= [0u8; 4];
    reader.read_exact(&mut len_buf)?;
    Ok(u32::from_le_bytes(len_buf))
}

fn parse_le_u32(buf: &[u8]) -> io::Result<u32>{
    parse_le_u32_at(buf, 0)
}
fn parse_le_u32_at(buf: &[u8], index: usize) -> io::Result<u32>{
    let bytes = buf.get(index..index+4)
        .ok_or_else(||io::Error::new(ErrorKind::InvalidInput, "Buffer is not large enough to parse 4 bytes"))?;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}

fn parse_le_u64(buf: &[u8]) -> io::Result<u64>{
    parse_le_u64_at(buf, 0)
}
fn parse_le_u64_at(buf: &[u8], index: usize) -> io::Result<u64>{
    let bytes = buf.get(index..index+8)
    .ok_or_else(||io::Error::new(ErrorKind::InvalidInput, "Buffer is not large enough to parse 8 bytes"))?;
    Ok(u64::from_le_bytes(bytes.try_into().unwrap()))
}

fn field_sep_index(buf: &[u8]) -> io::Result<usize> {
    buf.iter().position(|&b| b == b'=')
        .ok_or_else(||io::Error::new(ErrorKind::InvalidData, format!("Expected '=' in buffer: {:?}", &buf)))
}

fn parse_field(buf: &[u8], i: usize) -> io::Result<(usize, &[u8], &[u8])>{
    let mut i = i;
    let field_len = parse_le_u32_at(buf, i)? as usize;
    i += 4;
    let sep_pos = i + field_sep_index(&buf[i..i+field_len])?;
    
    let name = &buf[i..sep_pos];
    let value  = &buf[(sep_pos+1)..(i+field_len)];
    
    i += field_len;
    Ok((i, name, value))
}


impl BagHeader {
    fn from(buf: &[u8]) -> io::Result<BagHeader>{
        let mut i = 0;
        
        let mut index_pos = None;
        let mut conn_count = None;
        let mut chunk_count = None;

        loop {
            let (new_index, name, value) = parse_field(buf, i)?;
            i = new_index;

            match name {
                b"index_pos" => index_pos = Some(parse_le_u64(value)?),
                b"conn_count" => conn_count = Some(parse_le_u32(value)?),
                b"chunk_count" => chunk_count = Some(parse_le_u32(value)?),
                b"op" => {
                    let op = parse_u8(value)?;
                    if op != OpCode::BagHeader as u8 {
                        return Err(io::Error::new(ErrorKind::InvalidData, format!("Expected op {:?}, found {:?}", OpCode::BagHeader, op)))
                    }
                }
                _ => return Err(io::Error::new(ErrorKind::InvalidData, format!("Unexpected field {} in BagHeader", String::from_utf8_lossy(name))))
            }
        
            if i >= buf.len(){
                break;
            }
        }

        Ok(BagHeader{
         index_pos: index_pos.ok_or_else(||io::Error::new(ErrorKind::InvalidData, "Missing field 'index_pos' in BagHeader"))?,
         conn_count: conn_count.ok_or_else(||io::Error::new(ErrorKind::InvalidData, "Missing field 'conn_count' in BagHeader"))?,
         chunk_count: chunk_count.ok_or_else(||io::Error::new(ErrorKind::InvalidData, "Missing field 'chunk_count' in BagHeader"))?
        })
    }
}

/// Struct to store everything about a Chunk
/// As ChunkHeader and ChunkInfoHeaders are separate, after parsing all records, combine that info into a Chunk
struct ChunkMetadata {
    compression: String,
    uncompressed_size: usize,
    compressed_size: usize,
    chunk_pos: usize,
    start_time: Time,
    end_time: Time,
    connection_count: usize,
}

struct ChunkHeader {
    compression: String,
    uncompressed_size: u32,
}

impl ChunkHeader {
    fn from(buf: &[u8]) -> io::Result<ChunkHeader>{
        let mut i = 0;
        
        let mut compression = None;
        let mut size = None;

        loop {
            let (new_index, name, value) = parse_field(buf, i)?;
            i = new_index;

            match name {
                b"compression" => compression = Some(String::from_utf8_lossy(value).to_string()),
                b"size" => size = Some(parse_le_u32(value)?),
                b"op" => {
                    let op = parse_u8(value)?;
                    if op != OpCode::ChunkHeader as u8 {
                        return Err(io::Error::new(ErrorKind::InvalidData, format!("Expected op {:?}, found {:?}", OpCode::ChunkHeader, op)))
                    }
                }
                _ => return Err(io::Error::new(ErrorKind::InvalidData, format!("Unexpected field {} in ChunkHeader", String::from_utf8_lossy(name))))
            }
        
            if i >= buf.len(){
                break;
            }
        }

        Ok(ChunkHeader{
            compression: compression.ok_or_else(||io::Error::new(ErrorKind::InvalidData, "Missing field 'compression' in ChunkHeader"))?,
            uncompressed_size: size.ok_or_else(||io::Error::new(ErrorKind::InvalidData, "Missing field 'size' in ChunkHeader"))?,
        })
    }
}


struct ChunkInfoHeader{
    version: u32,
    chunk_pos: u64,
    // timestamp of earliest message in the chunk
    start_time: Time,
    // timestamp of latest message in the chunk
    end_time: Time,
    // number of connections in the chunk 
    connection_count: u32,
}

impl ChunkInfoHeader {
    fn from(buf: &[u8]) -> io::Result<ChunkInfoHeader>{
        let mut i = 0;
        
        let mut version = None;
        let mut chunk_pos = None;
        let mut start_time = None;
        let mut end_time = None;
        let mut connection_count = None;

        loop {
            let (new_index, name, value) = parse_field(buf, i)?;
            i = new_index;
            
            match name {
                b"ver" => version =  Some(parse_le_u32(value)?),
                b"chunk_pos" => chunk_pos = Some(parse_le_u64(value)?),
                b"start_time" => start_time = Some(Time::from(value)?),
                b"end_time" => end_time = Some(Time::from(value)?),
                b"count" => connection_count = Some(parse_le_u32(value)?),
                b"op" => {
                    let op = parse_u8(value)?;
                    if op != OpCode::ChunkInfoHeader as u8 {
                        return Err(io::Error::new(ErrorKind::InvalidData, format!("Expected op {:?}, found {:?}", OpCode::ChunkInfoHeader, op)))
                    }
                }
                _ => return Err(io::Error::new(ErrorKind::InvalidData, format!("Unexpected field {} in ChunkInfoHeader", String::from_utf8_lossy(name))))
            }
        
            if i >= buf.len(){
                break;
            }
        }

        Ok(ChunkInfoHeader{
            version: version.ok_or_else(||io::Error::new(ErrorKind::InvalidData, "Missing field 'ver' in ChunkInfoHeader"))?,
            chunk_pos: chunk_pos.ok_or_else(||io::Error::new(ErrorKind::InvalidData, "Missing field 'chunk_pos' in ChunkInfoHeader"))?,
            start_time: start_time.ok_or_else(||io::Error::new(ErrorKind::InvalidData, "Missing field 'start_time' in ChunkInfoHeader"))?,
            end_time: end_time.ok_or_else(||io::Error::new(ErrorKind::InvalidData, "Missing field 'end_time' in ChunkInfoHeader"))?,
            connection_count: connection_count.ok_or_else(||io::Error::new(ErrorKind::InvalidData, "Missing field 'count' in ChunkInfoHeader"))?,
        })
    }
}

struct ChunkInfoData {
    connection_id: ConnectionID,
    // number of messages that arrived on this connection in the chunk 
    count: u32,
}

impl ChunkInfoData {
    fn from(buf: &[u8]) -> io::Result<ChunkInfoData>{
        Ok(ChunkInfoData{
            connection_id: parse_le_u32_at(buf, 0)?,
            count: parse_le_u32_at(buf, 4)?
        })
    }
}

#[derive(Debug)]
struct ConnectionHeader{
    connection_id: u32, 
    topic: String,
}

impl ConnectionHeader{
    fn from(buf: &[u8]) -> io::Result<ConnectionHeader>{
        let mut i = 0;
        
        let mut topic = None;
        let mut connection_id = None;

        loop {
            let (new_index, name, value) = parse_field(buf, i)?;
            i = new_index;
            
            match name {
                b"topic" => topic = Some(String::from_utf8_lossy(value).to_string()),
                b"conn" => connection_id = Some(parse_le_u32(value)?),
                b"op" => {
                    let op = parse_u8(value)?;
                    if op != OpCode::ConnectionHeader as u8 {
                        return Err(io::Error::new(ErrorKind::InvalidData, format!("Expected op {:?}, found {:?}", OpCode::ConnectionHeader, op)))
                    }
                }
                _ => return Err(io::Error::new(ErrorKind::InvalidData, format!("Unexpected field {} in ConnectionHeader", String::from_utf8_lossy(name))))
            }
        
            if i >= buf.len(){
                break;
            }
        }

        Ok(ConnectionHeader{
            connection_id: connection_id.ok_or_else(||io::Error::new(ErrorKind::InvalidData, "Missing field 'connection_id' in ConnectionHeader"))?,
            topic: topic.ok_or_else(||io::Error::new(ErrorKind::InvalidData, "Missing field 'topic' in ConnectionHeader"))?,
        })
    }
}

#[derive(Debug)]
struct ConnectionData {
    connection_id: u32, 
    topic: String,
    data_type: String,
    md5sum: String,
    message_definition: String,
    caller_id: Option<String>,
    latching: bool
}

impl ConnectionData {
    fn from(buf: &[u8], connection_id: u32, topic: String) -> io::Result<ConnectionData>{
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
                b"md5sum" => md5sum =  Some(String::from_utf8_lossy(value).to_string()),
                b"message_definition" => message_definition =  Some(String::from_utf8_lossy(value).to_string()),
                b"callerid" => caller_id =  Some(String::from_utf8_lossy(value).to_string()),
                b"latching" => latching =  value == b"1",
                _ => return Err(io::Error::new(ErrorKind::InvalidData, format!("Unexpected field {} in ConnectionData", String::from_utf8_lossy(name))))
            }
        
            if i >= buf.len(){
                break;
            }
        }

        Ok(ConnectionData{
            connection_id,
            topic,
            data_type: data_type.ok_or_else(||io::Error::new(ErrorKind::InvalidData, "Missing field 'data_type' in ConnectionData"))?,
            md5sum: md5sum.ok_or_else(||io::Error::new(ErrorKind::InvalidData, "Missing field 'md5sum' in ConnectionData"))?,
            message_definition: message_definition.ok_or_else(||io::Error::new(ErrorKind::InvalidData, "Missing field 'message_definition' in ConnectionData"))?,
            caller_id,
            latching,
        })
    }
}

struct IndexDataHeader {
    version: u32, //must be 1
    connection_id: ConnectionID,
    count: u32 // number of messages on conn in the preceding chunk 
}

impl IndexDataHeader {
    fn from(buf: &[u8]) -> io::Result<IndexDataHeader>{
        let mut i = 0;
        
        let mut version = None;
        let mut connection_id = None;
        let mut count = None;

        loop {
            let (new_index, name, value) = parse_field(buf, i)?;
            i = new_index;
            
            match name {
                b"ver" => version =  Some(parse_le_u32(value)?),
                b"conn" => connection_id = Some(parse_le_u32(value)?),
                b"count" => count = Some(parse_le_u32(value)?),
                b"op" => {
                    let op = parse_u8(value)?;
                    if op != OpCode::IndexDataHeader as u8 {
                        return Err(io::Error::new(ErrorKind::InvalidData, format!("Expected op {:?}, found {:?}", OpCode::IndexDataHeader, op)))
                    }
                }
                _ => return Err(io::Error::new(ErrorKind::InvalidData, format!("Unexpected field {} in IndexDataHeader", String::from_utf8_lossy(name))))
            }
        
            if i >= buf.len(){
                break;
            }
        }

        Ok(IndexDataHeader{
            version: version.ok_or_else(||io::Error::new(ErrorKind::InvalidData, "Missing field 'ver' in IndexDataHeader"))?,
            connection_id: connection_id.ok_or_else(||io::Error::new(ErrorKind::InvalidData, "Missing field 'conn' in IndexDataHeader"))?,
            count: count.ok_or_else(||io::Error::new(ErrorKind::InvalidData, "Missing field 'count' in IndexDataHeader"))?,
        })
    }
}

struct IndexData {
    chunk_pos: u64, // start position of the chunk in the file
    time: Time,       // time at which the message was received 
    offset: usize,      // offset of message data record in uncompressed chunk data 
}

impl IndexData {
    fn from(buf: &[u8], chunk_pos: u64) -> io::Result<IndexData>{
        Ok(IndexData{
            chunk_pos,
            time: Time::from(buf)?,
            offset: parse_le_u32_at(buf, 8)? as usize
        })
    }
}


struct MessageData {
    // ID for connection on which message arrived 
    conn: u32,
    // time at which the message was received 
    time: u64
}

impl MessageData {
    fn from(buf: &[u8]) -> io::Result<MessageData>{
        let mut i = 0;
        
        let mut conn = None;
        let mut time = None;

        loop {
            let (new_index, name, value) = parse_field(buf, i)?;
            i = new_index;
            
            match name {
                b"conn" => conn =  Some(parse_le_u32(value)?),
                b"time" => time = Some(parse_le_u64(value)?),
                _ => return Err(io::Error::new(ErrorKind::InvalidData, format!("Unexpected field {} in MessageData", String::from_utf8_lossy(name))))
            }
        
            if i >= buf.len(){
                break;
            }
        }

        Ok(MessageData{
            conn: conn.ok_or_else(||io::Error::new(ErrorKind::InvalidData, "Missing field 'conn' in MessageData"))?,
            time: time.ok_or_else(||io::Error::new(ErrorKind::InvalidData, "Missing field 'chunk_pos' in MessageData"))?,
        })
    }
}

impl Bag {
    fn from<P: Into<PathBuf> + AsRef<Path>>(file_path: P) -> io::Result<Bag> {
        let path: PathBuf = file_path.as_ref().into(); 
        let file  = File::open(file_path)?; 
        
        let mut reader = BufReader::new(file);
        
        Bag::version_check(&mut reader)?;

        Bag::parse_records(&mut reader)?;

        

        Ok(Bag {
            file_path: path
        })
    }

    fn version_check<R: Read + Seek>(reader: &mut R) -> io::Result<()> {
        let mut buf = [0u8; 13];
        let expected = b"#ROSBAG V2.0\n";
        reader.read_exact(&mut buf)?;
        if buf == *expected {
            Ok(())
        } else {
            Err(io::Error::new(ErrorKind::InvalidData, format!("Got unexpected rosbag version: {}", String::from_utf8_lossy(&buf))))
        }
    }

    fn get_lengthed_bytes<R: Read + Seek>(reader: &mut R) -> io::Result<Vec<u8>> {
        // Get a vector of bytes from a reader when the first 4 bytes are the length
        // Ex: with <header_len><header> or <data_len><data>, this function returns either header or data
        let mut len_buf= [0u8; 4];
        reader.read_exact(&mut len_buf)?;
    
        let len = u32::from_le_bytes(len_buf);
        let mut bytes = vec![0u8; len as usize];
        reader.read_exact(&mut bytes)?;

        Ok(bytes)
    }

    fn parse_bag_header<R: Read + Seek>(header_buf: &[u8], reader: &mut R) -> io::Result<BagHeader> {
        let bag_header = BagHeader::from(header_buf)?;

        if bag_header.index_pos == 0 {
            return Err(io::Error::new(ErrorKind::InvalidData, "Unindexed bag"))
        }

        let data_len = read_le_u32(reader)?;
        reader.seek(io::SeekFrom::Current(data_len as i64))?; // Skip bag header padding
 
        Ok(bag_header)
    }

    fn parse_connection<R: Read + Seek>(header_buf: &[u8], reader: &mut R) -> io::Result<ConnectionData> {
        let connection_header = ConnectionHeader::from(header_buf)?;
        let data = Bag::get_lengthed_bytes(reader)?; 
        ConnectionData::from(&data, connection_header.connection_id, connection_header.topic)
    }

    fn parse_chunk<R: Read + Seek>(header_buf: &[u8], reader: &mut R) -> io::Result<(u64, ChunkHeader)> {
        let chunk_header = ChunkHeader::from(header_buf)?;
        let data_len = read_le_u32(reader)?;
        let chunk_pos = reader.stream_position()?;
        reader.seek(io::SeekFrom::Current(data_len as i64))?; // skip reading the chunk
        Ok((chunk_pos, chunk_header))
    }

    fn parse_chunk_info<R: Read + Seek>(header_buf: &[u8], reader: &mut R) -> io::Result<(ChunkInfoHeader, Vec<ChunkInfoData>)> {
        let chunk_info_header = ChunkInfoHeader::from(header_buf)?;
        let data = Bag::get_lengthed_bytes(reader)?;

        let chunk_info_data: Vec<ChunkInfoData> = data.windows(8).step_by(8).flat_map(ChunkInfoData::from).collect();

        if chunk_info_data.len() != chunk_info_header.connection_count as usize {
            return Err(io::Error::new(ErrorKind::InvalidData, format!("Expected {} chunk_info_data, found {}", chunk_info_header.connection_count, chunk_info_data.len())))
        }

        Ok((chunk_info_header, chunk_info_data))
    }

    fn parse_index<R: Read + Seek>(header_buf: &[u8], reader: &mut R, chunk_pos: u64) -> io::Result<(ConnectionID, Vec<IndexData>)> {
        let index_data_header = IndexDataHeader::from(header_buf)?;
        let data = Bag::get_lengthed_bytes(reader)?; 
        
        let index_data: Vec<IndexData> = data.windows(12).step_by(12).flat_map(|buf| IndexData::from(buf, chunk_pos)).collect();

        if index_data.len() != index_data_header.count as usize {
            return Err(io::Error::new(ErrorKind::InvalidData, format!("Expected {} IndexData, found {}", index_data_header.count, index_data.len())))
        }

        Ok((index_data_header.connection_id, index_data))
    }

    fn parse_records<R: Read + Seek>(reader: &mut R) -> io::Result<()> {
        let mut bag_header: Option<BagHeader> = None;
        let mut chunk_headers: BTreeMap<u64, ChunkHeader> = BTreeMap::new();
        let mut chunk_infos: Vec<(ChunkInfoHeader, Vec<ChunkInfoData>)> = Vec::new();
        let mut connections: Vec<ConnectionData> = Vec::new();
        let mut index_data: BTreeMap<ConnectionID, Vec<IndexData>> = BTreeMap::new();

        let mut last_chunk_pos = None;

        loop {
            let maybe_header_len = read_le_u32(reader);
            if let Err(e) = maybe_header_len {
                match e.kind() {
                    ErrorKind::UnexpectedEof => break,
                    _ => return Err(e)
                }
            }
            let header_len = maybe_header_len.unwrap();
    
            let mut header_buf = vec![0u8; header_len as usize];
            reader.read_exact(&mut header_buf)?;
    
            let op = read_header_op(&header_buf)?;
            println!("Header is {:?}", op);

            match op {
                OpCode::BagHeader => {
                    bag_header = Some(Bag::parse_bag_header(&header_buf, reader)?);
                }
                OpCode::ChunkHeader => {
                    let (chunk_pos, chunk_header) = Bag::parse_chunk(&header_buf, reader)?;
                    last_chunk_pos = Some(chunk_pos);
                    chunk_headers.insert(chunk_pos, chunk_header);
                }
                OpCode::IndexDataHeader => {
                    let chunk_pos = last_chunk_pos.ok_or_else(|| io::Error::new(ErrorKind::InvalidData, "Expected a Chunk before reading IndexData"))?;
                    let (connection_id, mut data) = Bag::parse_index(&header_buf, reader, chunk_pos)?;
                    index_data.entry(connection_id).or_insert_with(Vec::new).append(&mut data);
                }
                OpCode::ConnectionHeader => {
                    connections.push(Bag::parse_connection(&header_buf, reader)?);
                }
                OpCode::ChunkInfoHeader => {
                    chunk_infos.push(Bag::parse_chunk_info(&header_buf, reader)?);
                }
                _ => return Err(io::Error::new(ErrorKind::InvalidData, format!("Unexpected header op code at the record level: {:?}", op)))
            }
        }

        println!("{:?}", bag_header);
        Ok(())
    }
}

fn read_header_op(buf: &[u8]) -> io::Result<OpCode>{
    let mut i = 0;
    loop {
        let (new_index, name, value) = parse_field(buf, i)?;
        i = new_index;
        
        if name == b"op" {
            let op = parse_u8(value)?;
            return OpCode::from(op)
        }
        
        if i >= buf.len(){
            break;
        }
    }
    Err(io::Error::new(ErrorKind::InvalidData, "No opcode field found"))
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::{Write, BufReader}, path::PathBuf};

    use tempfile::{tempdir, TempDir};

    use crate::{Bag, field_sep_index};

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
        let file  = File::open(file_path).unwrap(); 
        let mut reader = BufReader::new(file);
        assert!(Bag::version_check(&mut reader).is_ok())
    }

    #[test]
    fn bag_from() {
        let (_tmp_dir, file_path) = write_test_fixture();
        Bag::from(file_path).unwrap();
    }

    #[test]
    fn test_field_sep_position(){
        let buf = b"hello=banana";
        assert_eq!(field_sep_index(buf).unwrap(), 5);
        assert_eq!(field_sep_index(&buf[2..8]).unwrap(), 3);

        let buf = b"theresnosep";
        assert!(field_sep_index(buf).is_err());
    }
}

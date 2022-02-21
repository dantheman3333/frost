#![allow(dead_code)]
use std::borrow::Cow;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, prelude::*, BufReader, ErrorKind};
use std::path::{Path, PathBuf};



struct Bag {
    file_path: PathBuf,
    file: File,
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
        let secs = read_le_u32(buf)?;
        let nsecs = read_le_u32_at(buf, 4)?;
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
    BagHeaderOp = 0x03,
    ChunkHeaderOp = 0x05,
    ConnectionHeaderOp = 0x07,
    MessageDataOp = 0x02,
    IndexDataHeaderOp = 0x04,
    ChunkInfoHeaderOp = 0x06,
}

impl OpCode {
    fn from(byte: u8) -> io::Result<OpCode> {
        match byte {
            0x03 => Ok(OpCode::BagHeaderOp),
            0x05 => Ok(OpCode::ChunkHeaderOp),
            0x07 => Ok(OpCode::ConnectionHeaderOp),
            0x02 => Ok(OpCode::MessageDataOp),
            0x04 => Ok(OpCode::IndexDataHeaderOp),
            0x06 => Ok(OpCode::ChunkInfoHeaderOp),
            other => Err(io::Error::new(ErrorKind::InvalidInput, format!("Unknown op code {:#04x}", other)))
        }
    }
}

fn read_u8(buf: &[u8]) -> io::Result<u8>{
    read_u8_at(buf, 0)
}
fn read_u8_at(buf: &[u8], index: usize) -> io::Result<u8>{
    let bytes = buf.get(index..index+1)
        .ok_or(io::Error::new(ErrorKind::InvalidInput, "Buffer is not large enough to parse 1 bytes"))?;
    Ok(u8::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_le_u32(buf: &[u8]) -> io::Result<u32>{
    read_le_u32_at(buf, 0)
}
fn read_le_u32_at(buf: &[u8], index: usize) -> io::Result<u32>{
    let bytes = buf.get(index..index+4)
        .ok_or(io::Error::new(ErrorKind::InvalidInput, "Buffer is not large enough to parse 4 bytes"))?;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_le_u64(buf: &[u8]) -> io::Result<u64>{
    read_le_u64_at(buf, 0)
}
fn read_le_u64_at(buf: &[u8], index: usize) -> io::Result<u64>{
    let bytes = buf.get(index..index+8)
    .ok_or(io::Error::new(ErrorKind::InvalidInput, "Buffer is not large enough to parse 8 bytes"))?;
    Ok(u64::from_le_bytes(bytes.try_into().unwrap()))
}

fn field_sep_index(buf: &[u8]) -> io::Result<usize> {
    buf.iter().position(|&b| b == b'=')
        .ok_or(io::Error::new(ErrorKind::InvalidData, format!("Expected '=' in buffer: {:?}", &buf)))
}

fn parse_field<'a>(buf: &'a[u8], i: usize) -> io::Result<(usize, &'a[u8], &'a[u8])>{
    let mut i = i;
    let field_len = read_le_u32_at(buf, i)? as usize;
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
                b"index_pos" => index_pos = Some(read_le_u64(value)?),
                b"conn_count" => conn_count = Some(read_le_u32(value)?),
                b"chunk_count" => chunk_count = Some(read_le_u32(value)?),
                b"op" => {
                    let op = read_u8(value)?;
                    if op != OpCode::BagHeaderOp as u8 {
                        return Err(io::Error::new(ErrorKind::InvalidData, format!("Expected op {:?}, found {:?}", OpCode::BagHeaderOp, op)))
                    }
                }
                _ => return Err(io::Error::new(ErrorKind::InvalidData, format!("Unexpected field {} in BagHeader", String::from_utf8_lossy(name))))
            }
        
            if i >= buf.len(){
                break;
            }
        }

        Ok(BagHeader{
         index_pos: index_pos.ok_or(io::Error::new(ErrorKind::InvalidData, format!("Missing field 'index_pos' in BagHeader")))?,
         conn_count: conn_count.ok_or(io::Error::new(ErrorKind::InvalidData, format!("Missing field 'conn_count' in BagHeader")))?,
         chunk_count: chunk_count.ok_or(io::Error::new(ErrorKind::InvalidData, format!("Missing field 'chunk_count' in BagHeader")))?
        })
    }
}

struct ChunkHeader<'a> {
    // compression type for the data 
    compression: Cow<'a, str>,
    // size in bytes of the uncompressed chunk 
    size: u32,
}

impl ChunkHeader<'_> {
    fn from(buf: &[u8]) -> io::Result<ChunkHeader>{
        let mut i = 0;
        
        let mut compression = None;
        let mut size = None;

        loop {
            let (new_index, name, value) = parse_field(buf, i)?;
            i = new_index;

            match name {
                b"compression" => compression = Some(String::from_utf8_lossy(value)),
                b"size" => size = Some(read_le_u32(value)?),
                b"op" => {
                    let op = read_u8(value)?;
                    if op != OpCode::ChunkHeaderOp as u8 {
                        return Err(io::Error::new(ErrorKind::InvalidData, format!("Expected op {:?}, found {:?}", OpCode::ChunkHeaderOp, op)))
                    }
                }
                _ => return Err(io::Error::new(ErrorKind::InvalidData, format!("Unexpected field {} in ChunkHeader", String::from_utf8_lossy(name))))
            }
        
            if i >= buf.len(){
                break;
            }
        }

        Ok(ChunkHeader{
            compression: compression.ok_or(io::Error::new(ErrorKind::InvalidData, format!("Missing field 'compression' in ChunkHeader")))?,
            size: size.ok_or(io::Error::new(ErrorKind::InvalidData, format!("Missing field 'size' in ChunkHeader")))?,
        })
    }
}

struct ConnectionHeader{
    connection_id: u32, 
    topic: String
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
                b"conn" => connection_id = Some(read_le_u32(value)?),
                b"op" => {
                    let op = read_u8(value)?;
                    if op != OpCode::ConnectionHeaderOp as u8 {
                        return Err(io::Error::new(ErrorKind::InvalidData, format!("Expected op {:?}, found {:?}", OpCode::ConnectionHeaderOp, op)))
                    }
                }
                _ => return Err(io::Error::new(ErrorKind::InvalidData, format!("Unexpected field {} in ConnectionHeader", String::from_utf8_lossy(name))))
            }
        
            if i >= buf.len(){
                break;
            }
        }

        Ok(ConnectionHeader{
            connection_id: connection_id.ok_or(io::Error::new(ErrorKind::InvalidData, format!("Missing field 'connection_id' in ConnectionHeader")))?,
            topic: topic.ok_or(io::Error::new(ErrorKind::InvalidData, format!("Missing field 'topic' in ConnectionHeader")))?,
        })
    }
}

struct ConnectionData {
    connection_id: u32, 
    topic: String,
    md5sum: String,
    message_definition: String,
    caller_id: Option<String>,
    latching: bool
}

impl ConnectionData {
    fn from(buf: &[u8], connection_id: u32, topic: String) -> io::Result<ConnectionData>{
        let mut i = 0;
        
        let mut md5sum = None;
        let mut message_definition = None;
        let mut caller_id = None;
        let mut latching = false;

        loop {
            let (new_index, name, value) = parse_field(buf, i)?;
            i = new_index;
            
            match name {
                b"topic" => (),
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
            md5sum: md5sum.ok_or(io::Error::new(ErrorKind::InvalidData, format!("Missing field 'md5sum' in ConnectionData")))?,
            message_definition: message_definition.ok_or(io::Error::new(ErrorKind::InvalidData, format!("Missing field 'message_definition' in ConnectionData")))?,
            caller_id,
            latching,
        })
    }
}

struct IndexDataHeader {
    version: u32, //must be 1
    connection_id: u32,
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
                b"ver" => version =  Some(read_le_u32(value)?),
                b"conn" => connection_id = Some(read_le_u32(value)?),
                b"count" => count = Some(read_le_u32(value)?),
                b"op" => {
                    let op = read_u8(value)?;
                    if op != OpCode::IndexDataHeaderOp as u8 {
                        return Err(io::Error::new(ErrorKind::InvalidData, format!("Expected op {:?}, found {:?}", OpCode::IndexDataHeaderOp, op)))
                    }
                }
                _ => return Err(io::Error::new(ErrorKind::InvalidData, format!("Unexpected field {} in IndexDataHeader", String::from_utf8_lossy(name))))
            }
        
            if i >= buf.len(){
                break;
            }
        }

        Ok(IndexDataHeader{
            version: version.ok_or(io::Error::new(ErrorKind::InvalidData, format!("Missing field 'ver' in IndexDataHeader")))?,
            connection_id: connection_id.ok_or(io::Error::new(ErrorKind::InvalidData, format!("Missing field 'conn' in IndexDataHeader")))?,
            count: count.ok_or(io::Error::new(ErrorKind::InvalidData, format!("Missing field 'count' in IndexDataHeader")))?,
        })
    }
}

struct IndexData {
    time: Time,       // time at which the message was received 
    chunk_pos: usize, // start position of the chunk in the file
    offset: u32,      // offset of message data record in uncompressed chunk data 
}

impl IndexData {
    fn from(buf: &[u8], chunk_pos: usize) -> io::Result<IndexData>{
        Ok(IndexData{
            time: Time::from(buf)?,
            chunk_pos: chunk_pos,
            offset: read_le_u32_at(buf, 8)?
        })
    }
}

struct ChunkInfoHeader{
    version: u32,
    chunk_pos: u64,
    // timestamp of earliest message in the chunk
    start_time: u64,
    // timestamp of latest message in the chunk
    end_time: u64,
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
                b"version" => version =  Some(read_le_u32(value)?),
                b"chunk_pos" => chunk_pos = Some(read_le_u64(value)?),
                b"start_time" => start_time = Some(read_le_u64(value)?),
                b"end_time" => end_time = Some(read_le_u64(value)?),
                b"connection_count" => connection_count = Some(read_le_u32(value)?),
                b"op" => {
                    let op = read_u8(value)?;
                    if op != OpCode::ChunkInfoHeaderOp as u8 {
                        return Err(io::Error::new(ErrorKind::InvalidData, format!("Expected op {:?}, found {:?}", OpCode::ChunkInfoHeaderOp, op)))
                    }
                }
                _ => return Err(io::Error::new(ErrorKind::InvalidData, format!("Unexpected field {} in ChunkInfoHeader", String::from_utf8_lossy(name))))
            }
        
            if i >= buf.len(){
                break;
            }
        }

        Ok(ChunkInfoHeader{
            version: version.ok_or(io::Error::new(ErrorKind::InvalidData, format!("Missing field 'version' in ChunkInfoHeader")))?,
            chunk_pos: chunk_pos.ok_or(io::Error::new(ErrorKind::InvalidData, format!("Missing field 'chunk_pos' in ChunkInfoHeader")))?,
            start_time: start_time.ok_or(io::Error::new(ErrorKind::InvalidData, format!("Missing field 'start_time' in ChunkInfoHeader")))?,
            end_time: end_time.ok_or(io::Error::new(ErrorKind::InvalidData, format!("Missing field 'end_time' in ChunkInfoHeader")))?,
            connection_count: connection_count.ok_or(io::Error::new(ErrorKind::InvalidData, format!("Missing field 'connection_count' in ChunkInfoHeader")))?,
        })
    }
}

struct ChunkInfo {
    // docs are inconsistent "little-endian long integer 4 bytes" 
    // connection id
    conn: u32,
    // number of messages that arrived on this connection in the chunk 
    count: u32,
}

impl ChunkInfo {
    fn from(buf: &[u8]) -> io::Result<ChunkInfo>{
        let mut i = 0;
        
        let mut conn = None;
        let mut count = None;

        loop {
            let (new_index, name, value) = parse_field(buf, i)?;
            i = new_index;
            
            match name {
                b"conn" => conn =  Some(read_le_u32(value)?),
                b"count" => count = Some(read_le_u32(value)?),
                _ => return Err(io::Error::new(ErrorKind::InvalidData, format!("Unexpected field {} in ChunkInfo", String::from_utf8_lossy(name))))
            }
        
            if i >= buf.len(){
                break;
            }
        }

        Ok(ChunkInfo{
            conn: conn.ok_or(io::Error::new(ErrorKind::InvalidData, format!("Missing field 'conn' in ChunkInfo")))?,
            count: count.ok_or(io::Error::new(ErrorKind::InvalidData, format!("Missing field 'chunk_pos' in ChunkInfo")))?,
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
                b"conn" => conn =  Some(read_le_u32(value)?),
                b"time" => time = Some(read_le_u64(value)?),
                _ => return Err(io::Error::new(ErrorKind::InvalidData, format!("Unexpected field {} in MessageData", String::from_utf8_lossy(name))))
            }
        
            if i >= buf.len(){
                break;
            }
        }

        Ok(MessageData{
            conn: conn.ok_or(io::Error::new(ErrorKind::InvalidData, format!("Missing field 'conn' in MessageData")))?,
            time: time.ok_or(io::Error::new(ErrorKind::InvalidData, format!("Missing field 'chunk_pos' in MessageData")))?,
        })
    }
}

impl Bag {
    fn from<P: Into<PathBuf> + AsRef<Path>>(file_path: P) -> io::Result<Bag> {
        let path: PathBuf = file_path.as_ref().into(); 
        let file  = File::open(file_path)?; 
        
        let mut reader = BufReader::new(file);
        
        Bag::version_check(&mut reader)?;

        let bag_header = Bag::parse_bag_header(&mut reader)?;



        println!("{:?}", bag_header);

        todo!()
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
    
        let len = u32::from_le_bytes(len_buf.try_into().unwrap());
        let mut bytes = vec![0u8; len as usize];
        reader.read_exact(&mut bytes)?;

        Ok(bytes)
    }

    fn parse_bag_header<R: Read + Seek>(reader: &mut R) -> io::Result<BagHeader> {
        let bag_header = BagHeader::from(&Bag::get_lengthed_bytes(reader)?)?;

        if bag_header.index_pos == 0 {
            return Err(io::Error::new(ErrorKind::InvalidData, "Unindexed bag"))
        }
        //TODO: BufReader's .seek() always discards the buffer
        reader.seek(std::io::SeekFrom::Start(bag_header.index_pos))?;

        Ok(bag_header)
    }

    fn parse_connection<R: Read + Seek>(reader: &mut R) -> io::Result<ConnectionData> {
        let connection_header = ConnectionHeader::from(&Bag::get_lengthed_bytes(reader)?)?;
        let data = Bag::get_lengthed_bytes(reader)?; 
        ConnectionData::from(&data, connection_header.connection_id, connection_header.topic.to_string())
    }

    fn parse_records<R: Read + Seek>(self, reader: &mut R) -> io::Result<()> {
        loop {
            let mut len_buf= [0u8; 4];
            if let Err(e) = reader.read_exact(&mut len_buf) {
                match e.kind() {
                    ErrorKind::UnexpectedEof => return Ok(()),
                    _ => return Err(e)
                }
            }
            let header_len = u32::from_le_bytes(len_buf.try_into().unwrap());
    
            let mut header = vec![0u8; header_len as usize];
            reader.read_exact(&mut header)?;
    
            let op = read_header_op(&header);
            match op {
                Ok(op) => println!("Header is {:?}", op),
                Err(_) => println!("Unknown header!")
            }
    
            reader.read_exact(&mut len_buf)?;
            let data_len = u32::from_le_bytes(len_buf.try_into().unwrap());  
            let mut data = vec![0u8; data_len as usize];
            reader.read_exact(&mut data)?;
            
        }
    }

    fn parse_record<R: Read + Seek>(self, reader: &mut R) -> io::Result<Record> {
        let mut len_buf= [0u8; 4];
        reader.read_exact(&mut len_buf)?;
        let header_len = u32::from_le_bytes(len_buf.try_into().unwrap());

        let mut header = vec![0u8; header_len as usize];
        reader.read_exact(&mut header)?;

        let h = BagHeader::from(&header);
        println!("{:?}", h);

        reader.read_exact(&mut len_buf)?;
        let data_len = u32::from_le_bytes(len_buf.try_into().unwrap());
        
        let mut data = vec![0u8; data_len as usize];
        reader.read_exact(&mut data)?;

        Ok(Record{ header_len, header: header.into_boxed_slice(), data_len, data: data.into_boxed_slice()})
    }
}

fn read_header_op(buf: &[u8]) -> io::Result<OpCode>{
    let mut i = 0;
    loop {
        let (new_index, name, value) = parse_field(buf, i)?;
        i = new_index;
        
        if name == b"op" {
            let op = read_u8(value)?;
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
    use std::{fs::File, io::{Write, BufReader, BufRead}, path::PathBuf};

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
    fn parse_header() {
        let (_tmp_dir, file_path) = write_test_fixture();
        let bag = Bag::from(&file_path).unwrap();

        let file = File::open(bag.file_path.clone()).unwrap();
        let mut bufreader = BufReader::new(file);
        // skip version check
        bufreader.read_line(&mut String::new()).unwrap();

        bag.parse_record(&mut bufreader).unwrap();
    }

    #[test]
    fn parse_all() {
        let (_tmp_dir, file_path) = write_test_fixture();
        let bag = Bag::from(&file_path).unwrap();

        let file = File::open(bag.file_path.clone()).unwrap();
        let mut bufreader = BufReader::new(file);
        // skip version check
        bufreader.read_line(&mut String::new()).unwrap();

        bag.parse_records(&mut bufreader).unwrap();
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

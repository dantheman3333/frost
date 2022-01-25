#![allow(dead_code)]
use std::fs::File;
use std::io::{self, prelude::*, BufReader, ErrorKind};
use std::path::{Path, PathBuf};


trait Readable {
    fn read_bag(message_type: Option<Vec<String>>) -> dyn Iterator<Item = dyn Message>;
}

trait Message {}
struct Bag {
    file_path: PathBuf,
    file: File,
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
}

fn read_u8(buf: &[u8], index: usize) -> io::Result<u8>{
    let bytes = buf.get(index..index+1)
        .ok_or(io::Error::new(ErrorKind::InvalidInput, "Buffer is not large enough to parse 1 bytes"))?;
    Ok(u8::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_le_u32(buf: &[u8], index: usize) -> io::Result<u32>{
    let bytes = buf.get(index..index+4)
        .ok_or(io::Error::new(ErrorKind::InvalidInput, "Buffer is not large enough to parse 4 bytes"))?;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_le_u64(buf: &[u8], index: usize) -> io::Result<u64>{
    let bytes = buf.get(index..index+8)
    .ok_or(io::Error::new(ErrorKind::InvalidInput, "Buffer is not large enough to parse 8 bytes"))?;
    Ok(u64::from_le_bytes(bytes.try_into().unwrap()))
}

fn field_sep_index(buf: &[u8]) -> io::Result<usize> {
    buf.iter().position(|&b| b == b'=')
        .ok_or(io::Error::new(ErrorKind::InvalidData, format!("Expected '=' in buffer: {:?}", &buf)))
}


impl BagHeader {
    fn from(buf: &[u8]) -> io::Result<BagHeader>{
        let mut i = 0;
        
        let mut index_pos = None;
        let mut conn_count = None;
        let mut chunk_count = None;

        loop {
            let field_len = read_le_u32(buf, i)? as usize;
            i += 4;
            let sep_pos = i + field_sep_index(&buf[i..i+field_len])?;
            
            let name = &buf[i..sep_pos];
            let value  = &buf[(sep_pos+1)..(i+field_len)];
            
            match name {
                b"index_pos" => index_pos = Some(read_le_u64(value, 0)?),
                b"conn_count" => conn_count = Some(read_le_u32(value, 0)?),
                b"chunk_count" => chunk_count = Some(read_le_u32(value, 0)?),
                b"op" => {
                    let op = read_u8(value, 0)?;
                    if op != OpCode::BagHeaderOp as u8 {
                        return Err(io::Error::new(ErrorKind::InvalidData, format!("Expected op {:?}, found {:?}", OpCode::BagHeaderOp, op)))
                    }
                }
                _ => return Err(io::Error::new(ErrorKind::InvalidData, format!("Expected field {} in BagHeader", String::from_utf8_lossy(name))))
            }
            i += field_len;
        
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

struct ChunkHeader {
    // compression type for the data 
    compression: String,
    // size in bytes of the uncompressed chunk 
    size: u32,
}

struct ConnectionHeader {
    // unique connection ID 
    conn: u32,
    // topic on which the messages are stored 
    topic: String
}

struct IndexDataHeader {
    // index data record version 
    ver: u32,
    // connection ID 
    conn: u32,
    // number of messages on conn in the preceding chunk 
    count: u32
}

struct IndexData {
    // time at which the message was received 
    time: u64,
    // offset of message data record in uncompressed chunk data 
    offset: u32,
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

struct ChunkInfo {
    // docs are inconsistent little-endian long integer 4 bytes 
    // connection id
    conn: u32,
    // number of messages that arrived on this connection in the chunk 
    count: u32,
}

struct MessageData {
    // ID for connection on which message arrived 
    conn: u32,
    // time at which the message was received 
    time: u64
}
impl Bag {
    fn from<P: Into<PathBuf> + AsRef<Path>>(file_path: P) -> io::Result<Bag> {
        Ok(Bag {
            file_path: file_path.as_ref().into(),
            file: File::open(file_path)?,
        })
    }

    fn version_check(self) -> io::Result<()> {
        let mut reader = BufReader::new(self.file);
        let mut buf = String::new();

        match reader.read_line(&mut buf) {
            Ok(_) => {
                let line = buf.trim_end();
                if line == "#ROSBAG V2.0" {
                    Ok(())
                } else {
                    Err(io::Error::new(ErrorKind::InvalidData, format!("Got unexpected version data: {}", line)))
                }
            }
            Err(e) => Err(e),
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
        let bag = Bag::from(&file_path).unwrap();
        assert!(bag.version_check().is_ok())
    }

    #[test]
    fn parse_header() {
        let (_tmp_dir, file_path) = write_test_fixture();
        let bag = Bag::from(&file_path).unwrap();

        let file = File::open(bag.file_path.clone()).unwrap();
        let mut bufreader = BufReader::new(file);
        // skip version check
        bufreader.read_line(&mut String::new()).unwrap();

        bag.parse_record(&mut bufreader);
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

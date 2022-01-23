use std::fs::File;
use std::io::{self, prelude::*, BufReader, ErrorKind};
use std::path::{Path, PathBuf};

use ascii::AsciiStr;
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

struct BagHeader{
    // offset of first record after the chunk section
    index_pos: u64,
    // number of unique connections in the file
    conn_count: u32,
    // number of chunk records in the file 
    chunk_count: u32,
}
fn read_le_u32(buf: &[u8], index: usize) -> io::Result<u32>{
    let bytes = buf.get(index..index+4).ok_or(io::Error::new(ErrorKind::InvalidInput, "Buffer is not large enough to parse 4 bytes"))?;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}

impl BagHeader {
    fn from(buf: &[u8]) -> io::Result<BagHeader>{
        let i = 0;
        loop {
            let field_len = read_le_u32(buf, i)? as usize;
            let sep_pos = buf[i..field_len].iter().position(|&b| b == b'=').ok_or(io::Error::new(ErrorKind::InvalidData, format!("Expected = in string: {:?}", &buf[i..i+field_len])))?;
            
            

            if i >= buf.len(){
                break;
            }
        }

        todo!()
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

    use crate::Bag;

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
}

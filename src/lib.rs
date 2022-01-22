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
        print!("header len {}\n", header_len);

        let mut header = vec![0u8; header_len as usize];
        reader.read_exact(&mut header)?;

        reader.read_exact(&mut len_buf)?;
        let data_len = u32::from_le_bytes(len_buf.try_into().unwrap());
        print!("data len {}\n", data_len);
        
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

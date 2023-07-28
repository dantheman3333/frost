use std::{fs::File, io::Write, path::PathBuf};

use frost::errors::ErrorKind;
use frost::query::Query;
use frost::Bag;

use tempfile::{tempdir, TempDir};

mod common;
use common::msgs::std_msgs;

const DECOMPRESSED: &[u8] = include_bytes!("fixtures/decompressed.bag");
const COMPRESSED_LZ4: &[u8] = include_bytes!("fixtures/compressed_lz4.bag");

fn write_test_fixture(bytes: &[u8]) -> (TempDir, PathBuf) {
    let tmp_dir = tempdir().unwrap();
    let file_path = tmp_dir.path().join("test.bag");
    {
        let mut tmp_file = File::create(file_path.clone()).unwrap();
        tmp_file.write(bytes).unwrap();
    }
    (tmp_dir, file_path)
}

#[test]
fn bag_iter_from_file() {
    for (bytes, name) in [
        (DECOMPRESSED, "decompressed"),
        (COMPRESSED_LZ4, "compressed_lz4"),
    ]
    .iter()
    {
        let (_tmp_dir, file_path) = write_test_fixture(bytes);
        let mut bag = Bag::from_file(file_path).unwrap();

        let query = Query::all();
        let count = bag.read_messages(&query).unwrap().count();
        assert_eq!(count, 300, "{name}");

        let query = Query::new().with_topics(&["/chatter"]);
        let count = bag.read_messages(&query).unwrap().count();
        assert_eq!(count, 100, "{name}");
    }
}

#[test]
fn bag_iter_from_bytes() {
    for (bytes, name) in [
        (DECOMPRESSED, "decompressed"),
        (COMPRESSED_LZ4, "compressed_lz4"),
    ]
    .iter()
    {
        let mut bag = Bag::from_bytes(bytes).unwrap();

        let query = Query::all();
        let count = bag.read_messages(&query).unwrap().count();
        assert_eq!(count, 300, "{name}");

        let query = Query::new().with_topics(&["/chatter"]);
        let count = bag.read_messages(&query).unwrap().count();
        assert_eq!(count, 100, "{name}");

        let query = Query::new().with_topics(&["/array"]);
        let count = bag.read_messages(&query).unwrap().count();
        assert_eq!(count, 100, "{name}");

        let query = Query::new().with_types(&["std_msgs/String"]);
        let count = bag.read_messages(&query).unwrap().count();
        assert_eq!(count, 100, "{name}");
        bag.read_messages(&query).unwrap().for_each(|msg_view| {
            assert_eq!(msg_view.topic, "/chatter");
        });

        let query = Query::new()
            .with_topics(&["/chatter"])
            .with_types(&["std_msgs/String"]);
        let count = bag.read_messages(&query).unwrap().count();
        assert_eq!(count, 100, "{name}");

        let query = Query::new()
            .with_topics(&["/time"])
            .with_types(&["std_msgs/Time"]);
        let count = bag.read_messages(&query).unwrap().count();
        assert_eq!(count, 100, "{name}");

        let query = Query::new()
            .with_topics(&["/chatter"])
            .with_types(&["std_msgs/Time"]);
        let count = bag.read_messages(&query).unwrap().count();
        assert_eq!(count, 0, "{name}");

        let query = Query::new().with_types(&["std_msgs/Time", "std_msgs/String"]);
        let count = bag.read_messages(&query).unwrap().count();
        assert_eq!(count, 200, "{name}");
    }
}

#[test]
fn msg_reading() {
    for (bytes, name) in [
        (DECOMPRESSED, "decompressed"),
        (COMPRESSED_LZ4, "compressed_lz4"),
    ]
    .iter()
    {
        let mut bag = Bag::from_bytes(bytes).unwrap();

        let query = Query::new().with_topics(&["/chatter"]);

        for (i, msg_view) in bag.read_messages(&query).unwrap().enumerate() {
            let msg = msg_view.instantiate::<std_msgs::String>().unwrap();
            assert_eq!(msg.data, format!("foo_{i}"), "{name}")
        }

        let query = Query::new().with_topics(&["/time"]);
        let count = bag.read_messages(&query).unwrap().count();
        assert_eq!(count, 100, "{name}");

        for (i, msg_view) in bag.read_messages(&query).unwrap().enumerate() {
            let msg = msg_view.instantiate::<std_msgs::Time>().unwrap();
            assert_eq!(msg.data.secs, i as u32, "{name}");
        }

        let query = Query::new().with_topics(&["/array"]);
        let count = bag.read_messages(&query).unwrap().count();
        assert_eq!(count, 100, "{name}");

        for msg_view in bag.read_messages(&query).unwrap() {
            let msg = msg_view
                .instantiate::<std_msgs::Float64MultiArray>()
                .unwrap();
            assert_eq!(msg.data, vec![3.14, 3.14, 3.14], "{name}");
        }
    }
}

#[test]
fn msg_reading_wrong_type() {
    for (bytes, name) in [
        (DECOMPRESSED, "decompressed"),
        (COMPRESSED_LZ4, "compressed_lz4"),
    ]
    .iter()
    {
        let mut bag = Bag::from_bytes(bytes).unwrap();

        let query = Query::new().with_topics(&["/chatter"]);
        let msg_view = bag.read_messages(&query).unwrap().last().unwrap();

        // Try to read a string as a Time
        let res = msg_view.instantiate::<std_msgs::Time>();
        assert!(
            matches!(res.unwrap_err().kind(), ErrorKind::Deserialization(_)),
            "{name}"
        )
    }
}

use std::{fs::File, io::Write, path::PathBuf};

use frost::query::Query;
use frost::Bag;
use tempfile::{tempdir, TempDir};

mod msgs;
use self::msgs::msgs::std_msgs;

fn setup_fixture(tmp_dir: &TempDir) -> PathBuf {
    let bytes = include_bytes!("../fixtures/test.bag");

    let file_path = tmp_dir.path().join("test.bag");

    let mut tmp_file = File::create(&file_path).unwrap();
    tmp_file.write(bytes).unwrap();

    return file_path;
}
fn main() {
    let tmp_dir = tempdir().unwrap();
    let bag_path = setup_fixture(&tmp_dir);

    let mut bag = Bag::from(bag_path).unwrap();

    let query = Query::all();
    let count = bag.read_messages(&query).count();
    assert_eq!(count, 200);

    for msg_view in bag.read_messages(&query) {
        match msg_view.topic {
            "/chatter" => {
                let msg = msg_view.instantiate::<std_msgs::String>().unwrap();
                assert!(msg.data.starts_with("foo_"))
            }
            "/array" => {
                let msg = msg_view
                    .instantiate::<std_msgs::Float64MultiArray>()
                    .unwrap();
                assert_eq!(msg.data, vec![0f64, 0f64, 0f64]);
            }
            &_ => panic!("Test fixture should only have '/chatter' and '/array'"),
        }
    }

    let query = Query::new().with_topics(&["/chatter"]);
    let count = bag.read_messages(&query).count();
    assert_eq!(count, 100);

    let msg_view = bag.read_messages(&query).last().unwrap();
    let msg = msg_view.instantiate::<std_msgs::String>().unwrap();
    println!("Last {} message is {}", &msg_view.topic, msg.data);
}

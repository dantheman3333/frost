use std::f32::consts::PI;
use std::{fs::File, io::Write, path::PathBuf};

use frost::query::Query;
use frost::Bag;
use tempfile::{tempdir, TempDir};

include!(concat!(env!("OUT_DIR"), "/msgs.rs"));

use self::msgs::dummy_msgs;
use self::msgs::std_msgs;

fn setup_fixture(tmp_dir: &TempDir) -> PathBuf {
    let bytes = include_bytes!("../fixtures/test.bag");

    let file_path = tmp_dir.path().join("test.bag");

    let mut tmp_file = File::create(&file_path).unwrap();
    tmp_file.write_all(bytes).unwrap();

    file_path
}
fn main() {
    let tmp_dir = tempdir().unwrap();
    let bag_path = setup_fixture(&tmp_dir);

    let mut bag = Bag::from(bag_path).unwrap();

    let query = Query::all();
    let count = bag.read_messages(&query).unwrap().count();
    assert_eq!(count, 200);

    for msg_view in bag.read_messages(&query).unwrap() {
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
    let count = bag.read_messages(&query).unwrap().count();
    assert_eq!(count, 100);

    let msg_view = bag.read_messages(&query).unwrap().last().unwrap();
    let msg = msg_view.instantiate::<std_msgs::String>().unwrap();
    println!("Last {} message is {}", &msg_view.topic, msg.data);

    let query = Query::new().with_types(&["std_msgs/Float64MultiArray"]);
    let count = bag.read_messages(&query).unwrap().count();
    assert_eq!(count, 100);
    println!(
        "There are {} messages with type std_msgs/Float64MultiArray",
        count
    );

    // check msg constants (type not in bag)
    assert_eq!(dummy_msgs::Dummy::PI, PI);
    assert_eq!(dummy_msgs::Dummy::N_PI, -PI);
    assert_eq!(dummy_msgs::Dummy::HELLO, "\"WORLD\"");

    println!("Dummy.msg's HELLO field is {}", dummy_msgs::Dummy::HELLO);
}

use frost::query::Query;
use frost::Bag;

mod msgs;

fn main() {

    let mut bag = Bag::from("../..").unwrap();

    let query = Query::all();
    let count = bag.read_messages(&query).count();
    assert_eq!(count, 2000);

    for msg_view in bag.read_messages(&query) {
        match msg_view.topic.as_str() {
            "/chatter" => {
                let _msg = msg_view.instantiate::<msgs::std_msgs::String>().unwrap();
            }
            "/time" => {
                let _msg = msg_view.instantiate::<Time>().unwrap();
            }
            &_ => panic!("Test fixture should only have these two"),
        }
    }
}

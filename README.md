# frost
rosbag + rust = frost

Read rosbags using Rust. Supports custom types via code-generation.

Example:

```rust
    let mut bag = Bag::from(bag_path).unwrap();

    let query = Query::all();
    let count = bag.read_messages(&query).count();
    assert_eq!(count, 200);

    for msg_view in bag.read_messages(&query) {
        match msg_view.topic.as_str() {
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
            &_ => panic!("Test fixture should only have these two"),
        }
    }

    let query = Query::new().with_topics(&vec!["/chatter"]);
    let count = bag.read_messages(&query).count();
    assert_eq!(count, 100);

    let msg_view = bag.read_messages(&query).last().unwrap();
    let msg = msg_view.instantiate::<std_msgs::String>().unwrap();
    println!("Last {} message is {}", &msg_view.topic, msg.data);
```

Currently does not support:
- compressed bags
- default values in ros msgs
- probably a lot more

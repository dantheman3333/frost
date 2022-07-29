# frost
rosbag + rust = frost

Read rosbags using Rust. Supports custom types via code-generation.

As a binary:
```bash
cargo run --release --bin frost info -- ./examples/read_bag/fixtures/test.bag
```
```
path:        ./examples/read_bag/fixtures/test.bag
version:     2.0
duration:    99s
start:       0.000001
end:         99.0001
messages:    200
compression: TODO
types:       std_msgs/String            [992ce8a1687cec8c8bd883ec73ca41d1]
             std_msgs/Float64MultiArray [4b7d974086d4060e7db4613a7e6c3ba4]
topics:      /chatter        100 msgs : std_msgs/String
             /array          100 msgs : std_msgs/Float64MultiArray
```

As a library:

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

TODO:
- reading compressed bags
- default values in ros msgs
- better errors
- bag writing

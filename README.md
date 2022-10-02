# frost
rosbag + rust = frost

Read rosbags using Rust. Supports custom types via code-generation.

## As a binary

```bash
cargo run --release --bin frost -- info ./examples/read_bag/fixtures/test.bag
# or, if you've installed the binary
frost info ./examples/read_bag/fixtures/test.bag
```
```bash
path:        ./examples/read_bag/fixtures/test.bag
version:     2.0
duration:    99s
start:       1970-01-01 00:00:00.000001 UTC
end:         1970-01-01 00:01:39.000100 UTC
messages:    200
compression: TODO
types:       std_msgs/Float64MultiArray [4b7d974086d4060e7db4613a7e6c3ba4]
             std_msgs/String            [992ce8a1687cec8c8bd883ec73ca41d1]
topics:      /array          100 msgs : std_msgs/Float64MultiArray
             /chatter        100 msgs : std_msgs/String
```
To print epoch seconds instead of UTC strings, pass the `--epoch` argument. 

There are more commands than the standard `rosbag info`, such as the `topics` command, which will just print the topics in the bag.
```bash
frost topics ./examples/read_bag/fixtures/test.bag
```
```bash
/array
/chatter
```

### Why use this over the normal `rosbag info`?

The `topics` command allows you to see topics without extra noise that `info` provides, and on large bags this program is around 4x faster, with subsequent runs up to 15x faster.


## Installation
**Note**: if you do not have Rust or Cargo installed, follow the guide [here](https://www.rust-lang.org/tools/install).

```bash
cargo install --git https://github.com/kramer425/frost.git frost
```

If you would like to set up `bash` auto-completion for `frost` arguments, run:
```bash
frost --bpaf-complete-style-bash >> ~/.bash_completion
source ~/.bash_completion
```
Or, for `zsh` (untested):
```zsh
frost --bpaf-complete-style-zsh > ~/.zsh/_frost
source ~/.zsh/_frost
```

## As a library

When using it as a library, code-generation is required to convert ros .msg files to Rust structs. 
See the full example and code-generation steps [here](examples/read_bag).

```rust
  let mut bag = Bag::from(bag_path).unwrap();

  let query = Query::all();
  let count = bag.read_messages(&query).unwrap().count();
  assert_eq!(count, 200);

  for msg_view in bag.read_messages(&query).unwrap() {
      match msg_view.topic {
          "/chatter" => {
              let msg = msg_view.instantiate::<std_msgs::String>().unwrap();
              // because frost has ros msg -> rust struct generation,
              // you can safely access your data with the correct rust types: 
              // `msg.data` is a `std::String` so you have access to `starts_with`
              assert!(msg.data.starts_with("foo_"))
          }
          "/array" => {
              let msg = msg_view
                  .instantiate::<std_msgs::Float64MultiArray>()
                  .unwrap();
              assert_eq!(msg.data, vec![0f64, 0f64, 0f64]);
          }
          &_ => {}
      }
  }

  let query = Query::new().with_topics(&["/chatter"]);
  let count = bag.read_messages(&query).unwrap().count();
  assert_eq!(count, 100);
```

## TODO

- reading bz2-compressed bags with `read_messages`
    - `frost info/topics` command line works with all compression modes and lz4-compressed bags works with `read_messages`
- default values in ros msgs
- better errors
- a `frost echo` command
- bag writing

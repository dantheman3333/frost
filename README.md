# frost
rosbag + rust = frost

Read rosbags using Rust. Supports custom types via code-generation.

## As a binary

```bash
frost info ./examples/read_bag/fixtures/test.bag
# or, if you are building from source:
cargo run --release --bin frost -- info ./examples/read_bag/fixtures/test.bag
```
```bash
path:        ./examples/read_bag/fixtures/test.bag
version:     2.0
duration:    99s
start:       2022-10-16 20:40:59.000001 UTC (1665952859.000001)
end:         2022-10-16 20:42:38.000100 UTC (1665952958.000100)
size:        11.53 KiB (11808 bytes)
messages:    200
compression: lz4 [1/1 chunks; 18.53%]
types:       std_msgs/Float64MultiArray [4b7d974086d4060e7db4613a7e6c3ba4]
             std_msgs/String            [992ce8a1687cec8c8bd883ec73ca41d1]
topics:      /array          100 msgs : std_msgs/Float64MultiArray
             /chatter        100 msgs : std_msgs/String
```

There are more commands than the standard `rosbag info`, such as the `info --minimal` subcommand, which will leave out the types and topics. Or, the `topics` command, which will just print the topics in the bag:
```bash
frost topics ./examples/read_bag/fixtures/test.bag
```
```bash
/array
/chatter
```
And, the `types` command:
```bash
frost types ./examples/read_bag/fixtures/test.bag
```
```bash
std_msgs/Float64MultiArray
std_msgs/String
```

## Why use this over the normal `rosbag info`?

### Speed:
This program is around *4x to 120x+* faster (larger the bag, faster the speedup).
On a compressed 25GB bag, the vanilla `rosbag` can take 20+ minutes, while `frost` returns in less than ~10 seconds. 

### More commands
The `topics` and `types` commands allow you to see the information you need without extra noise that `info` provides

--------------------------------------------------------
## Installation
- Copy the binary URL from the [Releases](https://github.com/kramer425/frost/releases/) page.
- In a terminal:
```bash
wget <URL> -O frost
chmod +x frost
sudo mv frost /usr/local/bin # or elsewhere
frost --help # check if it's in your path
```


### Building from source
**Note**: if you do not have Rust or Cargo installed, follow the guide [here](https://www.rust-lang.org/tools/install).

```bash
cargo install --all-features --git https://github.com/kramer425/frost.git frost 
```

#### Optional Build Features:
`frost` has some optional features for the binary. If you wish to not include them, remove the `--all-features` flag for `cargo install`.  
- color
  - enables colors in the help menu
  - if you build with colors enabled but wish to disable them, you can set the env var `NO_COLOR=1`

#### Autocomplete Setup:
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
--------------------------------------------------------

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
- a `frost echo` command
- bag writing

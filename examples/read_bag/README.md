
To run this example:
```bash
cargo run --example read_bag
```

To regenerate sample bags, run:
```bash
./examples/read_bag/scripts/generate_fixtures.sh
```

Make sure you have the `std_msgs` (git submodule in the root of the project) checked out:
```bash
git submodule update --init
```

To rerun code-generation for the `std_msgs` and example messages, run:
```bash
cargo run --bin frost-codegen -- -i std_msgs/ -i examples/read_bag/fixtures/dummy_msgs/ -o ./examples/read_bag/src/msgs.rs
```

The generation recursively scans every `.msg` file in the supplied `std_msgs` and dummy_msgs directories and create Rust structs for them, which will be used by the `serde_rosmsg` crate for deserialization. 
The autogenerate file can be found [here](src/msgs.rs). 
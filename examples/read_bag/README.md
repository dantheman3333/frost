
To run this example:
```bash
cargo run --release --example read_bag
```

To regenerate sample bags, run:
```bash
./examples/read_bag/scripts/generate_fixtures.sh
```

To rerun code-generation for the `std_msgs` (git submodule in the root of the project), run:
```bash
cargo run --release --bin frost-codegen -- -i std_msgs -o ./examples/read_bag/src/msgs.rs
```

The generation recursively scans every `.msg` file in the supplied `std_msgs` directory and creates Rust structs for them, which will be used by the `serde_rosmsg` crate for deserialization. 
The autogenerate file can be found [here](src/msgs.rs). 
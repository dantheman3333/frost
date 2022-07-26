
To run this example:
```bash
cargo run --example read_bag
```

To regenerate sample bags, run:
```bash
./examples/read_bag/scripts/generate_fixtures.sh
```

To rerun code-generation for the std_msgs, run:
```bash
cargo run --bin frost-codegen -- -i std_msgs -o ./examples/read_bag/src/msgs.rs
```
# CKB Merkle Mountain Range Bug Reproduce

## How to reproduce the bug of CKB Merkle Mountain Range?

Step into the directory:
```shell
cd bug-reproducer
```

Run one of the following command to build the binary:
```shell
# Build with ckb-merkle-mountain-range="0.2.0"
cargo build --release --features "mmr_0_2"
# Build with ckb-merkle-mountain-range="0.3.0"
cargo build --release --features "mmr_0_3"
# Build with ckb-merkle-mountain-range="0.4.0"
cargo build --release --features "mmr_0_4"
# Build with ckb-merkle-mountain-range="0.5.0"
cargo build --release --features "mmr_0_5"
```

Run the following command to reproduce the bug:
```shell
RUST_LOG=info,mmr_bug=trace target/release/mmr-bug-reproducer target/demo.db
```
The first argument is a path for the database file.

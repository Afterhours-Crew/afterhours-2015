# Afterhours 2015

Server implementation for Need for Speed (2015), with offline play and custom
servers as the goals. This is the canonical home for runtime code and its
first-party dependencies.

Components arrive incrementally once their behavior is understood and tested.
**There is no runnable game server in this repository yet.** The current
workspace contains `nfs-storage`: account-owned inventory, garage slots and
persistent tables, with in-memory and SQLite adapters. Its tests use generated
data and temporary databases; game files and service access are unnecessary.

See the [component guide](crates/storage/README.md) and
[contribution and promotion process](CONTRIBUTING.md).

## Build and test

Install Rust through rustup and a native C toolchain (MSVC Build Tools on Windows,
or a C compiler on Linux). `rust-toolchain.toml` selects Rust 1.98.1, rustfmt and
Clippy. SQLite is built from the bundled dependency; no database service is needed.
Cargo downloads locked dependencies on the first build.

```sh
cargo build --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

Once the toolchain and dependencies are cached, add `--offline` to Cargo build,
Clippy and test commands. No private repository, account profile or recording is
needed. Passing these tests establishes storage behavior only, not offline play
or launcher independence.

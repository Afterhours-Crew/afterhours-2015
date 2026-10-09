# Afterhours 2015

Server implementation for Need for Speed (2015), with offline play and custom
servers as the goals. This is the canonical home for runtime code and its
first-party dependencies.

Components arrive incrementally once their behavior is understood and tested.
**There is no runnable game server in this repository yet.** The workspace owns
these implemented components:

| Component | Responsibility |
| --- | --- |
| [nfs-fire2](crates/fire2/README.md) | Bounded stream framing and incremental decoding |
| [nfs-heat2](crates/heat2/README.md) | Bounded tagged fields, containers and canonical encoding |
| [nfs-protocol](crates/protocol/README.md) | Typed backend payloads and bit-oriented world codecs |
| [nfs-world-core](crates/world-core/README.md) | Transport, application state, File transfers, item models and startup registration codecs |
| [nfs-lsx-codec](crates/lsx-codec/README.md) | Launcher framing, XML, envelopes and transform codecs |
| [nfs-services](crates/services/README.md) | Current account stats and reputation, client-state/telemetry handling, local recommendations/wrap listings, inventory, persistent-table views and content loaders |
| [nfs-storage](crates/storage/README.md) | Account-owned inventory, garage slots and tables, with memory and SQLite adapters |

Tests use constructed inputs, standard cryptographic vectors and temporary
databases. Game files and service access are unnecessary. See the
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
needed. Passing these tests establishes the covered component behavior, not
offline play or launcher independence. Session integration, scene replication,
launcher policy and the runnable server remain unfinished.

## License

This repository is licensed under the Mozilla Public License, version 2.0. See
[LICENSE](LICENSE). Each source file carries the MPL notice, so modified copies
of those files must stay under the MPL when distributed, while new files that
only use these crates may carry another license.

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
| [nfs-services](crates/services/README.md) | Owned startup groups, current account stats and reputation, client-state/telemetry handling, local recommendations/wrap listings, inventory, persistent-table views and content loaders |
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

The world attribute service consumes a committed readiness permit, validates the
current group-to-world association and builds a typed notification. The association
becomes visible only after both frames write; retries ACK without republishing.
Bounds, failed writes and foreign policies close the session. This supports the
single current-world association, not arbitrary attribute edits.

The local self-mesh service generates the validation notification and correlated
ACK from the accepted world binding. It reports publication only after the pair
writes, closes on failed writes or premature retries, and bounds repeat requests.
This does not replace the independent host reliable-synchronization gate.

The initial matchmaking diagnostic uses bounded local search policy and accepted
session IDs. Its notification publishes only after a complete write; retries do
not publish twice. It does not allocate a world or assert a successful match.

The world setup service constructs the complete typed setup from bounded deployment
policy, accepted player state, fresh allocation and injected match measurements.
It retains no recorded reply. Publication waits for the complete write; failed
writes and premature retries invalidate the world.

Fresh matchmaking/world allocations use injected raw entropy, local ID domains
and a caller-owned loopback endpoint. Matchmaking acknowledgements bind an
admitted current group/player context, commit only after complete writes and
bound retries. Typed admission validates the complete supported request form
against explicit rule policy and the initialized group's current user and network
binding. Configuration contains named rules and local limits, never serialized
requests. This covers a single local member; broader matchmaking remains work.

## License

This repository is licensed under the Mozilla Public License, version 2.0. See
[LICENSE](LICENSE). Each source file carries the MPL notice, so modified copies
of those files must stay under the MPL when distributed, while new files that
only use these crates may carry another license.

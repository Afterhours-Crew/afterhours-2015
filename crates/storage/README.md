# nfs-storage

Synchronous persistence boundary for account-owned inventory, five ordered garage
slots and typed persistent tables. Server callers must run storage operations off
async workers. `MemoryRepository` and `SqliteRepository` implement the same
`InventoryRepository` interface and share transition validation.

- Account identifiers come from owned local configuration. Each SQLite account
  uses its own database; no game account credentials or shipped profile is needed.
- A batch validates the final item graph, garage references and bounded tables
  atomically. Failed batches leave the previous state intact.
- Batch IDs provide retry detection within a bounded history; generations reject
  stale updates. Callers must keep a batch ID associated with the same operation.
- SQLite schema version 3 migrates earlier inventory/garage layouts. Ownership
  and version checks precede adoption; successful writes commit before returning.
- Definitions and class-specific item bodies are caller-supplied data. This crate
  ships no item catalog, extracted game content or wire protocol implementation.

```sh
cargo test -p nfs-storage --locked
```

The contract suite covers adapter equivalence, rollback, retry handling, account
isolation, resource limits, schema migration and reopening durable state. It uses
synthetic values and temporary storage. Process crashes, power loss and gameplay
persistence are separate acceptance tests and are not claimed here.

# nfs-services

State-owned services built on the storage port and typed protocol models.

- Client-state acknowledgement validates a complete canonical request and returns
  an empty reply with its correlation. Only modes 1/3 with normal status are
  accepted; this reports no menu-readiness or persistent-state transition.
- Inventory loading initializes an empty account once, validates item graphs,
  preserves later progress and produces replies only from committed state.
- Persistent tables initialize missing schemas transactionally, preserve sparse
  saved cells and expose garage slots as a projection of the same account state.
- Versioned item-definition and table-schema loaders validate bounded input and
  its build identity. Callers supply their own content; no catalog is bundled.

Time and local account identity are explicit inputs. Repository/file operations
are synchronous; a socket edge must dispatch them to blocking workers. These
libraries provide no listener and require no service access or recording.
The local initial garage policy selects the lowest owned root vehicle ID;
it is an explicit local policy, not verified official fresh-account selection.

Tests construct item definitions, rows and accounts, including separate-account
initialization races, failed commits, retries and SQLite restart persistence.
Run the workspace Cargo gates in the root README. No game install is required.

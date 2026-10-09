# nfs-services

State-owned services built on the storage port and typed protocol models.

- Client-state acknowledgement validates a complete canonical request and returns
  an empty reply with its correlation. Only modes 1/3 with normal status are
  accepted; this reports no menu-readiness or persistent-state transition.
- Telemetry (`4/171`) validates canonical requests, pins the first accepted
  connection identities and retains at most 16 diagnostic samples. It returns a
  correlated header-only acknowledgement without forwarding data or changing
  account state. Requests contain at most eight reports and 512 body bytes.
- In-game recommendations (`2050/21`) answer only for the current persona,
  encoding empty speed-wall maps for stat types 0 and 1. Shared-wrap listings
  (`2052/23`) likewise require the current persona and supported item system,
  returning a zero total for the empty local catalog. Populated catalogs and
  wrap user-list queries are explicitly unsupported.
- Inventory loading initializes an empty account once, validates item graphs,
  preserves later progress and produces replies only from committed state.
- ItemBuilder begin-update validates a complete bounded call against the current
  scene binding and an owned Player (distinct from its Participant). Its server behavior is a silent no-op;
  it neither changes inventory nor accepts commit or customization mutations.
- Persistent tables initialize missing schemas transactionally, preserve sparse
  saved cells and expose garage slots as a projection of the same account state.
- Stats definitions (`7/4`) use a bounded `nfs-stat-definitions` version1 catalog.
  Its groups name a persistent table and ordered integer columns. Current Stats
  queries (`7/16`, followed by `7/50`) read those columns from a committed account
  view, bind the authenticated persona and echo the requested view ID. Missing
  state, other identities, unknown groups and unsupported periods get no success.
  Static default strings never substitute for missing account values.
- Reputation projects level and adjacent score thresholds from the current
  six persisted scores and an injected increasing threshold list. Current Stats
  use that calculated level even when the saved level is stale. The same model
  can drive world presentation; score accrual is outside this component.
- Menu Stats and Awards (`2050/71`) project the same account's collection and
  objective flags, gameplay counters, medals, SpeedList counts and reputation.
  Version1 account metadata stores record names/screenshots/timestamps, race
  records, social counters and entitlement declarations in six bounded local
  tables. Explicit initialization creates empty local history without grants;
  partial or malformed history fails. Metadata operations can share one atomic
  repository batch with related progression changes. The response requires the
  authenticated persona and a single committed generation. Gameplay reward and
  social mutation policies are outside this read service.
- Startup groups own a single authenticated member, fresh injected identifiers,
  and create/mesh/finalize/initialize transitions. Public and private group
  requests have explicit validation; setup fields come from current identity,
  network data and a bounded `nfs-group-policy` version1 deployment document.
  No captured request or response is a configuration input. Immediate retries
  repeat only the correlated reply; membership notifications occur once.
  The edge must commit each output batch after writing it or abort on failure.
  An initialized, committed group exposes an immutable matchmaking handoff.
  Multiplayer membership, matchmaking and driving worlds are outside this service.
- Versioned item-definition and table-schema loaders validate bounded input and
  its build identity. Callers supply their own content; no catalog is bundled.

Time and local account identity are explicit inputs. Repository/file operations
are synchronous; a socket edge must dispatch them to blocking workers. These
libraries provide no listener and require no service access or recording.
The caller creates separate service state for each connection and supplies its
authenticated local persona. These handlers accept one complete frame; stream
assembly and session-readiness gating belong to the calling edge. Invalid input
produces an error without advancing service state.
The local initial garage policy selects the lowest owned root vehicle ID;
it is an explicit local policy, not verified official fresh-account selection.

Tests construct item definitions, rows and accounts, including separate-account
initialization races, failed commits, retries and SQLite restart persistence.
Control-service tests cover malformed and noncanonical requests, resource bounds,
identity changes, connection isolation, repeated requests and correlated replies.
Run the workspace Cargo gates in the root README. No game install is required.

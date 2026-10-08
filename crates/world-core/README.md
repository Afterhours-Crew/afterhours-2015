# nfs-world-core

Socket-free world components: bit writing, transport transforms, datagram
framing and cursors, handshake, reliable link, application admission/control
state, File records and transfer state, and item/inventory models.

Each connection owns its state. Callers supply data, clocks, identifiers, random
values, keys and resource policies. No MAC template, extracted item catalog,
profile, replay queue, scene replication or network listener is bundled. The
application model currently covers a single-peer host connection; this is not a
complete multiplayer world implementation.

Tests cover loss, retries, ordering, rollover, limits, cancellation and constructed
transfers. [The rollover vector](tests/fixtures/transport/README.md) is synthetic.
These checks do not establish offline game acceptance.

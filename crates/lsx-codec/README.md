# nfs-lsx-codec

Bounded launcher stream framing, XML parsing, typed LSX envelopes and the
parameter-derived transform. This library has no socket or session controller.

Transform tests include NIST AES known answers and constructed compatibility
cases. Resource limits and malformed-input rejection are explicit. The codecs
are compatibility code, not a general-purpose security library.

Some startup message directions and request-field placements remain unverified
on the wire, as marked in the API. This crate supplies no authentication,
configuration or startup success policy, and does not prove launcher independence.

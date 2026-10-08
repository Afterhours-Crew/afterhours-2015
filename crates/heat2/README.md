# nfs-heat2

Bounded structural decoding, typed views and canonical writing for Heat2 tagged
fields. Limits cover bytes, depth and container sizes. Unknown fields retain
their bytes; typed adapters decide which shapes are supported.

Constructed tests exercise scalar widths, floats, strings, lists, maps, unions,
object IDs and malformed input. This crate supplies no reply or account policy.

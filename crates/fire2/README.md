# nfs-fire2

Socket-free Fire2 framing with borrowed decoded views, an incremental decoder,
explicit byte limits and preservation of opaque header fields. The caller owns
connections, deadlines, backpressure and message routing.

Constructed tests cover partial and concatenated frames, bounded buffering,
length failures and exact encoding. No TLS, capture reader or listener is included.

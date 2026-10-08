# Synthetic rollover vector

`rollover.hex` contains three independently generated datagrams for cursor
rollover tests. It contains no captured traffic or game data.

- World UUID: `11111111-2222-3333-4444-555555555555`.
- Local UUID: `66666666-7777-8888-9999-aaaaaaaaaaaa`.
- MAC template: ascending bytes 0 through 63.
- Initial cipher cursor: 32763 eight-byte units.
- Destination index: 9; channel: 0.
- Payloads: ascending bytes starting at zero, of lengths 27, 40 and 3.

The test constructs matching keys and plaintext independently and checks
decryption, cursor continuity, loss and rejection behavior.

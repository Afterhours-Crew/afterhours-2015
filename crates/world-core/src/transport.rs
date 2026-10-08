// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! CommUDP datagram codec.
//!
//! ```text
//! initial: 81 01 | 1ccc cccc cccc cccc | sender u32 | index u16 | peer_known u8 | encrypted
//! ordinary: 0iii iiii iiii iiii | 1ccc cccc cccc cccc | encrypted
//! encrypted = descriptors (u16 each: size << 4 | channel) | mac[12] | part bytes
//! ```
//!
//! `c` is the low 15 bits of the sender's cipher position in 8-byte units. The
//! epoch-zero offset is `cursor * 8` keystream bytes past the RC4 drop; the next
//! cursor is `(cursor + ceil(encrypted_len / 8)) & 0x7fff`; cipher state continues
//! across that rollover. The MAC covers the header, the
//! descriptors, twelve zero bytes and the part bytes.
use crate::crypto::{KEY_LEN, MAC_LEN, MAC_TEMPLATE_LEN, RC4_DROP, Rc4, mac};

pub const MAX_DATAGRAM: usize = 1264;
pub const MAX_PARTS: usize = 8;
pub const MAX_CHANNEL: u8 = 7;
pub const MAX_PART_BYTES: usize = 0xfff;
/// Cursors and tunnel indices are 15-bit values.
pub const MAX_CURSOR: u16 = 0x7fff;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Key,
    Length,
    Header,
    Index,
    Descriptor,
    Authentication,
    Cursor,
    /// Behind the directional stream, including the ambiguous half-range.
    Stale,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Header {
    Initial {
        sender: u32,
        index: u16,
        peer_known: bool,
        cursor: u16,
    },
    Ordinary {
        index: u16,
        cursor: u16,
    },
}

impl Header {
    pub fn cursor(self) -> u16 {
        match self {
            Self::Initial { cursor, .. } | Self::Ordinary { cursor, .. } => cursor,
        }
    }

    pub fn index(self) -> u16 {
        match self {
            Self::Initial { index, .. } | Self::Ordinary { index, .. } => index,
        }
    }

    fn len(self) -> usize {
        match self {
            Self::Initial { .. } => 11,
            Self::Ordinary { .. } => 4,
        }
    }

    pub fn bytes(self) -> Result<Vec<u8>, Error> {
        if self.cursor() > MAX_CURSOR || self.index() > MAX_CURSOR {
            return Err(Error::Index);
        }
        let mut out = Vec::with_capacity(self.len());
        match self {
            Self::Initial {
                sender,
                index,
                peer_known,
                cursor,
            } => {
                out.extend_from_slice(&0x8101u16.to_be_bytes());
                out.extend_from_slice(&(cursor | 0x8000).to_be_bytes());
                out.extend_from_slice(&sender.to_be_bytes());
                out.extend_from_slice(&index.to_be_bytes());
                out.push(u8::from(peer_known));
            }
            Self::Ordinary { index, cursor } => {
                out.extend_from_slice(&index.to_be_bytes());
                out.extend_from_slice(&(cursor | 0x8000).to_be_bytes());
            }
        }
        Ok(out)
    }

    /// Parse the cleartext header of a datagram.
    pub fn parse(raw: &[u8]) -> Result<Self, Error> {
        if !(4..=MAX_DATAGRAM).contains(&raw.len()) {
            return Err(Error::Length);
        }
        let first = u16::from_be_bytes([raw[0], raw[1]]);
        let mode = u16::from_be_bytes([raw[2], raw[3]]);
        if mode & 0x8000 == 0 {
            return Err(Error::Header);
        }
        let cursor = mode & MAX_CURSOR;
        if first & 0x8000 == 0 {
            return Ok(Self::Ordinary {
                index: first,
                cursor,
            });
        }
        if first != 0x8101 || raw.len() < 11 || raw[10] > 1 {
            return Err(Error::Header);
        }
        let index = u16::from_be_bytes([raw[8], raw[9]]);
        if index > MAX_CURSOR {
            return Err(Error::Index);
        }
        Ok(Self::Initial {
            sender: u32::from_be_bytes([raw[4], raw[5], raw[6], raw[7]]),
            index,
            peer_known: raw[10] != 0,
            cursor,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Part {
    pub channel: u8,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Decoded {
    pub header: Header,
    pub parts: Vec<Part>,
    /// Encrypted length; determines how far the sender's cursor advanced.
    pub encrypted_len: usize,
}

/// Cursor after sending `encrypted_len` encrypted bytes from `cursor`.
pub fn advance(cursor: u16, encrypted_len: usize) -> Result<u16, Error> {
    if cursor > MAX_CURSOR || encrypted_len > MAX_DATAGRAM {
        return Err(Error::Cursor);
    }
    Ok((cursor + encrypted_len.div_ceil(8) as u16) & MAX_CURSOR)
}

impl Decoded {
    pub fn next_cursor(&self) -> Result<u16, Error> {
        advance(self.header.cursor(), self.encrypted_len)
    }
}

/// Per-session cipher and MAC state. Holds secrets; no Debug.
pub struct Codec {
    key: [u8; KEY_LEN],
    mac_key: [u8; MAC_TEMPLATE_LEN],
}

impl Drop for Codec {
    fn drop(&mut self) {
        self.key.fill(0);
        self.mac_key.fill(0);
    }
}

impl Codec {
    /// `key` is the session key (see [`crate::crypto::session_key`]); `template`
    /// is the build's 64-byte MAC template.
    pub fn new(key: [u8; KEY_LEN], template: [u8; MAC_TEMPLATE_LEN]) -> Result<Self, Error> {
        if key.contains(&0) || !key.is_ascii() {
            return Err(Error::Key);
        }
        let mac_key = Rc4::new(&key, RC4_DROP)
            .apply(&template)
            .try_into()
            .map_err(|_| Error::Key)?;
        Ok(Self { key, mac_key })
    }

    fn keystream(&self, cursor: u16) -> Rc4 {
        let mut cipher = Rc4::new(&self.key, RC4_DROP);
        cipher.skip(usize::from(cursor) * 8);
        cipher
    }

    /// Continue one direction from its epoch-zero handshake cursor. Thereafter
    /// use this stream exclusively: a wire cursor alone loses the cipher epoch.
    pub fn stream(&self, cursor: u16) -> Result<Stream, Error> {
        if cursor > MAX_CURSOR {
            return Err(Error::Cursor);
        }
        Ok(Stream {
            cipher: self.keystream(cursor),
            mac_key: self.mac_key,
            cursor,
            units: u64::from(cursor),
        })
    }

    /// Epoch-zero random access, for handshake and short independent fixtures.
    pub fn decode(&self, raw: &[u8]) -> Result<Decoded, Error> {
        let header = Header::parse(raw)?;
        Self::decode_with(&self.mac_key, raw, &mut self.keystream(header.cursor()))
    }

    fn decode_with(
        mac_key: &[u8; MAC_TEMPLATE_LEN],
        raw: &[u8],
        cipher: &mut Rc4,
    ) -> Result<Decoded, Error> {
        let header = Header::parse(raw)?;
        let hlen = header.len();
        if raw.len() < hlen + MAC_LEN + 2 {
            return Err(Error::Length);
        }
        let plain = cipher.apply(&raw[hlen..]);
        let mut descriptors = Vec::with_capacity(MAX_PARTS);
        let mut total = 0;
        let mut complete = false;
        for i in 0..MAX_PARTS {
            if plain.len() < i * 2 + 2 + MAC_LEN {
                break;
            }
            let word = u16::from_be_bytes([plain[i * 2], plain[i * 2 + 1]]);
            let channel = (word & 15) as u8;
            let size = usize::from(word >> 4);
            if channel > MAX_CHANNEL {
                return Err(Error::Descriptor);
            }
            total += 2 + size;
            descriptors.push((channel, size));
            if total == plain.len() - MAC_LEN {
                complete = true;
                break;
            }
            if total > plain.len() - MAC_LEN {
                return Err(Error::Descriptor);
            }
        }
        if !complete {
            return Err(Error::Descriptor);
        }
        let mpos = descriptors.len() * 2;
        let mut message = Vec::with_capacity(raw.len());
        message.extend_from_slice(&raw[..hlen]);
        message.extend_from_slice(&plain[..mpos]);
        message.extend_from_slice(&[0; MAC_LEN]);
        message.extend_from_slice(&plain[mpos + MAC_LEN..]);
        let expected = mac(mac_key, &message);
        let diff = expected
            .iter()
            .zip(&plain[mpos..mpos + MAC_LEN])
            .fold(0u8, |d, (a, b)| d | (a ^ b));
        if diff != 0 {
            return Err(Error::Authentication);
        }
        let mut parts = Vec::with_capacity(descriptors.len());
        let mut pos = mpos + MAC_LEN;
        for (channel, size) in descriptors {
            parts.push(Part {
                channel,
                bytes: plain[pos..pos + size].to_vec(),
            });
            pos += size;
        }
        Ok(Decoded {
            header,
            parts,
            encrypted_len: plain.len(),
        })
    }

    pub fn encode(&self, header: Header, parts: &[Part]) -> Result<Vec<u8>, Error> {
        Self::encode_with(
            &self.mac_key,
            header,
            parts,
            &mut self.keystream(header.cursor()),
        )
    }

    fn encode_with(
        mac_key: &[u8; MAC_TEMPLATE_LEN],
        header: Header,
        parts: &[Part],
        cipher: &mut Rc4,
    ) -> Result<Vec<u8>, Error> {
        if parts.is_empty() || parts.len() > MAX_PARTS {
            return Err(Error::Descriptor);
        }
        let head = header.bytes()?;
        let mut descriptors = Vec::with_capacity(parts.len() * 2);
        let mut payload = Vec::new();
        for part in parts {
            if part.channel > MAX_CHANNEL || part.bytes.len() > MAX_PART_BYTES {
                return Err(Error::Descriptor);
            }
            descriptors.extend_from_slice(
                &(((part.bytes.len() as u16) << 4) | u16::from(part.channel)).to_be_bytes(),
            );
            payload.extend_from_slice(&part.bytes);
        }
        if head.len() + descriptors.len() + MAC_LEN + payload.len() > MAX_DATAGRAM {
            return Err(Error::Length);
        }
        let mut message = head.clone();
        message.extend_from_slice(&descriptors);
        message.extend_from_slice(&[0; MAC_LEN]);
        message.extend_from_slice(&payload);
        let mut plain = descriptors;
        plain.extend_from_slice(&mac(mac_key, &message));
        plain.extend_from_slice(&payload);
        let mut out = head;
        out.extend_from_slice(&cipher.apply(&plain));
        Ok(out)
    }
}

/// One authenticated direction of a tunnel. Cipher state, padded cursor and
/// progress commit together only after successful validation. No Debug: secrets.
/// Like the existing Link policy, reordered datagrams are discarded; native
/// previous-state retry is outside this forward receiver's support.
#[derive(Clone)]
pub struct Stream {
    cipher: Rc4,
    mac_key: [u8; MAC_TEMPLATE_LEN],
    cursor: u16,
    units: u64,
}
impl Drop for Stream {
    fn drop(&mut self) {
        self.mac_key.fill(0);
    }
}
impl Stream {
    pub fn cursor(&self) -> u16 {
        self.cursor
    }
    pub fn units(&self) -> u64 {
        self.units
    }

    pub fn decode(&mut self, raw: &[u8]) -> Result<Decoded, Error> {
        let header = Header::parse(raw)?;
        let delta = header.cursor().wrapping_sub(self.cursor) & MAX_CURSOR;
        // Signed 15-bit distance; bounds skip work to <128KiB.
        if delta >= 0x4000 {
            return Err(Error::Stale);
        }
        let mut cipher = self.cipher.clone();
        cipher.skip(usize::from(delta) * 8);
        let decoded = Codec::decode_with(&self.mac_key, raw, &mut cipher)?;
        let units = self
            .units
            .checked_add(u64::from(delta) + decoded.encrypted_len.div_ceil(8) as u64)
            .ok_or(Error::Cursor)?;
        cipher.skip((8 - decoded.encrypted_len % 8) % 8);
        self.cursor = decoded.next_cursor()?;
        self.units = units;
        self.cipher = cipher;
        Ok(decoded)
    }

    pub fn encode(&mut self, header: Header, parts: &[Part]) -> Result<Vec<u8>, Error> {
        if header.cursor() != self.cursor {
            return Err(Error::Cursor);
        }
        let mut cipher = self.cipher.clone();
        let wire = Codec::encode_with(&self.mac_key, header, parts, &mut cipher)?;
        let len = wire.len() - header.len();
        let units = self
            .units
            .checked_add(len.div_ceil(8) as u64)
            .ok_or(Error::Cursor)?;
        cipher.skip((8 - len % 8) % 8);
        self.cursor = advance(self.cursor, len)?;
        self.units = units;
        self.cipher = cipher;
        Ok(wire)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::session_key;

    pub(crate) fn codec() -> Codec {
        let key = session_key(
            "11111111-2222-3333-4444-555555555555",
            "66666666-7777-8888-9999-aaaaaaaaaaaa",
        )
        .unwrap();
        Codec::new(key, std::array::from_fn(|i| i as u8)).unwrap()
    }

    #[test]
    fn ordinary_and_initial_datagrams_round_trip() {
        let c = codec();
        let parts = vec![
            Part {
                channel: 0,
                bytes: vec![1, 2, 3],
            },
            Part {
                channel: 3,
                bytes: vec![9; 40],
            },
        ];
        for header in [
            Header::Ordinary {
                index: 0x1234,
                cursor: 17,
            },
            Header::Initial {
                sender: 0xdead_beef,
                index: 5,
                peer_known: true,
                cursor: 0,
            },
        ] {
            let wire = c.encode(header, &parts).unwrap();
            let decoded = c.decode(&wire).unwrap();
            assert_eq!(decoded.header, header);
            assert_eq!(decoded.parts, parts);
            assert_eq!(decoded.encrypted_len, wire.len() - header.len());
        }
    }

    #[test]
    fn any_flipped_bit_fails_authentication() {
        let c = codec();
        let wire = c
            .encode(
                Header::Ordinary {
                    index: 1,
                    cursor: 2,
                },
                &[Part {
                    channel: 0,
                    bytes: vec![0; 20],
                }],
            )
            .unwrap();
        for i in 4..wire.len() {
            let mut bad = wire.clone();
            bad[i] ^= 1;
            assert!(c.decode(&bad).is_err(), "bit flip at {i} accepted");
        }
    }

    #[test]
    fn the_cursor_selects_the_keystream() {
        let c = codec();
        let parts = [Part {
            channel: 0,
            bytes: vec![0; 8],
        }];
        let a = c
            .encode(
                Header::Ordinary {
                    index: 1,
                    cursor: 3,
                },
                &parts,
            )
            .unwrap();
        let b = c
            .encode(
                Header::Ordinary {
                    index: 1,
                    cursor: 4,
                },
                &parts,
            )
            .unwrap();
        assert_ne!(a[4..], b[4..]);
        // Moving the header cursor without re-encrypting breaks authentication.
        let mut moved = a.clone();
        moved[2..4].copy_from_slice(&(4u16 | 0x8000).to_be_bytes());
        assert!(matches!(
            c.decode(&moved),
            Err(Error::Authentication | Error::Descriptor)
        ));
    }

    #[test]
    fn cursor_advance_rounds_up_to_eight_byte_units() {
        assert_eq!(advance(0, 33), Ok(5));
        assert_eq!(advance(4, 22), Ok(7));
        assert_eq!(advance(MAX_CURSOR, 1), Ok(0));
        assert_eq!(advance(MAX_CURSOR - 4, 41), Ok(1));
        assert_eq!(advance(MAX_CURSOR + 1, 1), Err(Error::Cursor));
        assert_eq!(advance(0, usize::MAX), Err(Error::Cursor));
    }

    #[test]
    fn malformed_headers_and_limits_are_rejected() {
        assert_eq!(Header::parse(&[0, 1, 0, 0]), Err(Error::Header));
        assert_eq!(Header::parse(&[0x81, 0x01, 0x80, 0]), Err(Error::Header));
        assert_eq!(Header::parse(&[0; 3]), Err(Error::Length));
        let c = codec();
        let big = Part {
            channel: 0,
            bytes: vec![0; MAX_DATAGRAM],
        };
        assert!(
            c.encode(
                Header::Ordinary {
                    index: 0,
                    cursor: 0
                },
                &[big]
            )
            .is_err()
        );
        assert_eq!(
            c.encode(
                Header::Ordinary {
                    index: 0,
                    cursor: 0
                },
                &[Part {
                    channel: 8,
                    bytes: vec![]
                }]
            ),
            Err(Error::Descriptor)
        );
    }
}

#[cfg(test)]
#[path = "transport/stream_tests.rs"]
mod stream_tests;

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Application envelope: MSB-first fields and ten-bit sequences.
//! This profile assumes untransformed bytes and rejects nonzero excluded tails.
//! Transform selection, authentication, connection state and admission are separate.
use super::{BitSpan, history::Sequence};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidLimit,
    InputLimit,
    Truncated,
    UnsupportedTail,
    ChecksumMismatch,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "application envelope {self:?}")
    }
}
impl std::error::Error for Error {}

/// Explicitly select the observed zero-tail, untransformed wire profile.
/// The size bound is caller policy, not a universal limit learned from the game.
#[derive(Clone, Copy, Debug)]
pub struct ZeroTail {
    max_bytes: usize,
}
impl ZeroTail {
    pub const fn new(max_bytes: usize) -> Result<Self, Error> {
        if max_bytes < 4 || max_bytes > usize::MAX / 8 {
            return Err(Error::InvalidLimit);
        }
        Ok(Self { max_bytes })
    }

    /// Decode one complete, already separated application body without allocation.
    /// Kind 20 verifies the two-byte checksum, including physical padding bytes.
    /// Other kinds retain opaque payloads without inventing an integrity check.
    /// Success neither authenticates a peer nor identifies its active transform.
    pub fn decode(self, bytes: &[u8]) -> Result<Envelope<'_>, Error> {
        if bytes.len() > self.max_bytes {
            return Err(Error::InputLimit);
        }
        if bytes.len() < 4 {
            return Err(Error::Truncated);
        }
        let wire = BitSpan::new(bytes, 0, bytes.len() * 8).map_err(|_| Error::Truncated)?;
        let field = |offset, width| wire.read_u32(offset, width).map_err(|_| Error::Truncated);
        if field(3, 3)? != 0 {
            return Err(Error::UnsupportedTail);
        }
        let last_bits = match field(0, 3)? {
            0 => 8,
            n => n as usize,
        };
        let effective_bits = (bytes.len() - 1) * 8 + last_bits;
        if effective_bits < 28 {
            return Err(Error::Truncated);
        }
        let connection_selector = field(6, 14)? as u16;
        let kind = field(20, 8)? as u8;
        let (sequence, payload) = if kind == 20 {
            if effective_bits < 96 {
                return Err(Error::Truncated);
            }
            let end = bytes.len() - 2;
            if checksum(&bytes[..end]) != u16::from_be_bytes([bytes[end], bytes[end + 1]]) {
                return Err(Error::ChecksumMismatch);
            }
            let sequence = SequenceHeader {
                // Ten-bit extraction proves both constructor preconditions.
                number: Sequence::new(field(28, 10)? as u16).ok_or(Error::Truncated)?,
                acknowledgement: Sequence::new(field(38, 10)? as u16).ok_or(Error::Truncated)?,
                history: field(48, 32)?,
            };
            let payload = BitSpan::new(&bytes[10..end], 0, effective_bits - 96)
                .map_err(|_| Error::Truncated)?;
            (Some(sequence), payload)
        } else {
            let payload =
                BitSpan::new(bytes, 28, effective_bits - 28).map_err(|_| Error::Truncated)?;
            (None, payload)
        };
        Ok(Envelope {
            connection_selector,
            kind,
            effective_bits,
            sequence,
            payload,
        })
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct SequenceHeader {
    number: Sequence,
    acknowledgement: Sequence,
    history: u32,
}
impl SequenceHeader {
    pub fn number(self) -> Sequence {
        self.number
    }
    pub fn acknowledgement(self) -> Sequence {
        self.acknowledgement
    }
    pub fn history(self) -> u32 {
        self.history
    }
}
impl fmt::Debug for SequenceHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SequenceHeader").finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct Envelope<'a> {
    connection_selector: u16,
    kind: u8,
    effective_bits: usize,
    sequence: Option<SequenceHeader>,
    payload: BitSpan<'a>,
}
impl<'a> Envelope<'a> {
    pub fn connection_selector(self) -> u16 {
        self.connection_selector
    }
    /// Numeric wire kind; no unverified handshake names are assigned.
    pub fn kind(self) -> u8 {
        self.kind
    }
    pub fn effective_bits(self) -> usize {
        self.effective_bits
    }
    pub fn sequence(self) -> Option<SequenceHeader> {
        self.sequence
    }
    /// Exact payload extent. For kind 20, backing bytes exclude header/checksum
    /// but may include up to seven unused final bits. Other kinds borrow the
    /// complete body at bit offset 28. Always respect the BitSpan, not bytes.len.
    pub fn payload(self) -> BitSpan<'a> {
        self.payload
    }
}
impl fmt::Debug for Envelope<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Envelope")
            .field("kind", &self.kind)
            .field("effective_bits", &self.effective_bits)
            .field("sequenced", &self.sequence.is_some())
            .field("payload", &self.payload)
            .finish_non_exhaustive()
    }
}

/// wrapping-byte integrity check. Pass all physical bytes before the final
/// two checksum bytes, including partial-payload padding. This is not a MAC.
/// No allocation; time is linear in the supplied slice length.
pub fn checksum(bytes: &[u8]) -> u16 {
    let (mut a, mut b) = (0_u8, 0_u8);
    for &byte in bytes {
        a = a.wrapping_add(byte);
        b = b.wrapping_add(a);
    }
    u16::from_be_bytes([255_u8.wrapping_sub(b).wrapping_sub(a), b])
}

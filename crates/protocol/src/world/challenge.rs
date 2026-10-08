// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Application startup kinds 13, 15 and 16.
//! The selector-zero, untransformed profile retains unused physical final bits.
//! It does not authenticate a peer, validate the opaque answer or own a session.
use super::{
    BitSpan,
    envelope::{self, ZeroTail},
};
use std::fmt;

pub const MAX_OPTIONAL_BYTES: usize = 1024;
pub const MAX_BODY_BYTES: usize = 1051;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InputLimit,
    Envelope(envelope::Error),
    WrongKind,
    NonzeroSelector,
    PayloadLength,
    Truncated,
    OptionalLimit,
    OptionalChecksumMismatch,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "application challenge {self:?}")
    }
}
impl std::error::Error for Error {}

fn payload(bytes: &[u8], kind: u8) -> Result<(BitSpan<'_>, u8), Error> {
    if bytes.len() > MAX_BODY_BYTES {
        return Err(Error::InputLimit);
    }
    let e = ZeroTail::new(MAX_BODY_BYTES)
        .unwrap()
        .decode(bytes)
        .map_err(Error::Envelope)?;
    if e.kind() != kind {
        return Err(Error::WrongKind);
    }
    if e.connection_selector() != 0 {
        return Err(Error::NonzeroSelector);
    }
    let unused = bytes.len() * 8 - e.effective_bits();
    let padding = if unused == 0 {
        0
    } else {
        bytes[bytes.len() - 1] & ((1 << unused) - 1)
    };
    Ok((e.payload(), padding))
}
fn field(p: BitSpan<'_>, offset: usize, width: u8) -> Result<u32, Error> {
    p.read_u32(offset, width).map_err(|_| Error::Truncated)
}
fn optional(p: BitSpan<'_>, offset: usize) -> Result<Vec<u8>, Error> {
    if field(p, offset, 1)? == 0 {
        if p.len() != offset + 1 {
            return Err(Error::PayloadLength);
        }
        return Ok(vec![]);
    }
    let expected = field(p, offset + 1, 16)? as u16;
    let len = field(p, offset + 17, 10)? as usize + 1;
    if p.len() != offset + 27 + len * 8 {
        return Err(Error::PayloadLength);
    }
    let value = (0..len)
        .map(|i| field(p, offset + 27 + i * 8, 8).map(|v| v as u8))
        .collect::<Result<Vec<_>, _>>()?;
    if envelope::checksum(&value) != expected {
        return Err(Error::OptionalChecksumMismatch);
    }
    Ok(value)
}
fn own_optional(bytes: &[u8]) -> Result<Vec<u8>, Error> {
    if bytes.len() > MAX_OPTIONAL_BYTES {
        return Err(Error::OptionalLimit);
    }
    Ok(bytes.to_vec())
}

/// Two opaque native 32-bit fields, in the observed `+c4`, then `+c0` order.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct Request13 {
    fields: [u32; 2],
    padding: u8,
}
impl Request13 {
    pub const fn new(fields: [u32; 2]) -> Self {
        Self { fields, padding: 0 }
    }
    pub const fn fields(&self) -> [u32; 2] {
        self.fields
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        let (p, padding) = payload(bytes, 13)?;
        if p.len() != 64 {
            return Err(Error::PayloadLength);
        }
        Ok(Self {
            fields: [field(p, 0, 32)?, field(p, 32, 32)?],
            padding,
        })
    }
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::new(13);
        w.put(self.fields[0], 32);
        w.put(self.fields[1], 32);
        w.finish(self.padding)
    }
}
impl fmt::Debug for Request13 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Request13").finish_non_exhaustive()
    }
}

/// A native 32-bit challenge and bounded optional data. Freshness, peer binding
/// and random generation are caller policy, not codec acceptance.
#[derive(Clone, Eq, PartialEq)]
pub struct Challenge15 {
    value: u32,
    optional: Vec<u8>,
    padding: u8,
}
impl Challenge15 {
    pub fn new(value: u32, optional: &[u8]) -> Result<Self, Error> {
        Ok(Self {
            value,
            optional: own_optional(optional)?,
            padding: 0,
        })
    }
    pub const fn value(&self) -> u32 {
        self.value
    }
    pub fn optional(&self) -> &[u8] {
        &self.optional
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        let (p, padding) = payload(bytes, 15)?;
        Ok(Self {
            value: field(p, 0, 32)?,
            optional: optional(p, 32)?,
            padding,
        })
    }
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::new(15);
        w.put(self.value, 32);
        w.optional(&self.optional);
        w.finish(self.padding)
    }
}
impl fmt::Debug for Challenge15 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Challenge15")
            .field("optional_bytes", &self.optional.len())
            .finish_non_exhaustive()
    }
}

/// Structural kind16 observation. The 20-byte field is not interpreted or
/// validated against a challenge; decoding cannot establish authentication.
#[derive(Clone, Eq, PartialEq)]
pub struct OpaqueAnswer16 {
    opaque_field: [u8; 20],
    optional: Vec<u8>,
    padding: u8,
}
impl OpaqueAnswer16 {
    pub fn new(opaque_field: [u8; 20], optional: &[u8]) -> Result<Self, Error> {
        Ok(Self {
            opaque_field,
            optional: own_optional(optional)?,
            padding: 0,
        })
    }
    pub const fn opaque_field(&self) -> &[u8; 20] {
        &self.opaque_field
    }
    pub fn optional(&self) -> &[u8] {
        &self.optional
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        let (p, padding) = payload(bytes, 16)?;
        let mut opaque_field = [0; 20];
        for (i, byte) in opaque_field.iter_mut().enumerate() {
            *byte = field(p, i * 8, 8)? as u8;
        }
        Ok(Self {
            opaque_field,
            optional: optional(p, 160)?,
            padding,
        })
    }
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::new(16);
        w.bytes(&self.opaque_field);
        w.optional(&self.optional);
        w.finish(self.padding)
    }
}
impl fmt::Debug for OpaqueAnswer16 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpaqueAnswer16")
            .field("optional_bytes", &self.optional.len())
            .finish_non_exhaustive()
    }
}

struct Writer {
    bytes: Vec<u8>,
    bits: usize,
}
impl Writer {
    fn new(kind: u8) -> Self {
        let mut w = Self {
            bytes: Vec::new(),
            bits: 0,
        };
        w.put(0, 6);
        w.put(0, 14);
        w.put(u32::from(kind), 8);
        w
    }
    fn put(&mut self, value: u32, width: usize) {
        for i in (0..width).rev() {
            if self.bits.is_multiple_of(8) {
                self.bytes.push(0);
            }
            self.bytes[self.bits / 8] |= (((value >> i) & 1) as u8) << (7 - self.bits % 8);
            self.bits += 1;
        }
    }
    fn bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.put(u32::from(*byte), 8);
        }
    }
    fn optional(&mut self, bytes: &[u8]) {
        self.put(u32::from(!bytes.is_empty()), 1);
        if !bytes.is_empty() {
            self.put(u32::from(envelope::checksum(bytes)), 16);
            self.put(bytes.len() as u32 - 1, 10);
            self.bytes(bytes);
        }
    }
    fn finish(mut self, padding: u8) -> Vec<u8> {
        self.bytes[0] |= ((self.bits % 8) as u8) << 5;
        let last = self.bytes.len() - 1;
        self.bytes[last] |= padding;
        self.bytes
    }
}

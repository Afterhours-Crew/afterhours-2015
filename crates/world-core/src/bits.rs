// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! MSB-first bit writer for application envelopes and handler frames. The
//! matching reader is [`nfs_protocol::world::BitSpan`].

use nfs_protocol::world::BitSpan;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BitWriter {
    bytes: Vec<u8>,
    bits: usize,
}

impl BitWriter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Bits written so far.
    pub fn len(&self) -> usize {
        self.bits
    }

    pub fn is_empty(&self) -> bool {
        self.bits == 0
    }

    /// Append the low `width` bits of `value`, most significant first.
    pub fn put(&mut self, value: u64, width: usize) -> &mut Self {
        for i in (0..width).rev() {
            if self.bits.is_multiple_of(8) {
                self.bytes.push(0);
            }
            self.bytes[self.bits / 8] |= (((value >> i) & 1) as u8) << (7 - self.bits % 8);
            self.bits += 1;
        }
        self
    }

    pub fn put_bool(&mut self, value: bool) -> &mut Self {
        self.put(u64::from(value), 1)
    }

    pub fn put_f32(&mut self, value: f32) -> &mut Self {
        self.put(u64::from(value.to_bits()), 32)
    }

    pub fn put_bytes(&mut self, bytes: &[u8]) -> &mut Self {
        for byte in bytes {
            self.put(u64::from(*byte), 8);
        }
        self
    }

    /// Copy every bit of `span`, bit by bit.
    pub fn put_span(&mut self, span: BitSpan<'_>) -> &mut Self {
        for i in 0..span.len() {
            let bit = span.read_u32(i, 1).unwrap_or(0);
            self.put(u64::from(bit), 1);
        }
        self
    }

    /// Zero bits up to the next byte boundary; returns how many were added.
    pub fn align(&mut self) -> usize {
        let pad = (8 - self.bits % 8) % 8;
        self.put(0, pad);
        pad
    }

    /// The bytes written so far; the last byte is zero-padded below `len` bits.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// View the written bits.
    pub fn span(&self) -> BitSpan<'_> {
        BitSpan::new(&self.bytes, 0, self.bits).expect("writer bounds are consistent")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_msb_first_across_byte_boundaries() {
        let mut w = BitWriter::new();
        w.put(0b101, 3).put(0xff, 8).put(0, 2);
        assert_eq!(w.len(), 13);
        assert_eq!(w.bytes(), &[0b1011_1111, 0b1110_0000]);
        assert_eq!(w.span().read_u32(3, 8).unwrap(), 0xff);
        assert_eq!(w.align(), 3);
        assert_eq!(w.len(), 16);
        assert_eq!(w.align(), 0);
    }

    #[test]
    fn copies_unaligned_spans_exactly() {
        let source = [0b0110_1001, 0b1100_0011];
        let span = BitSpan::new(&source, 3, 9).unwrap();
        let mut w = BitWriter::new();
        w.put(1, 1).put_span(span);
        assert_eq!(w.len(), 10);
        // 1 | 01001 1100
        assert_eq!(w.bytes(), &[0b1010_0111, 0b0000_0000]);
        let mut f = BitWriter::new();
        f.put_f32(60.0).put_bool(true).put_bytes(&[0xa5]);
        assert_eq!(f.span().read_u32(0, 32).unwrap(), 60.0f32.to_bits());
        assert_eq!(f.span().read_u32(33, 8).unwrap(), 0xa5);
    }
}

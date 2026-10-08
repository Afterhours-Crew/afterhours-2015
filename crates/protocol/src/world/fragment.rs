//! Bounded application fragment headers and reassembly.
//! Per-connection reassembly only; no application history or delivery tokens.
use super::BitSpan;
use std::fmt;

pub const MAX_FRAGMENT_BITS: usize = 32_767;
pub const MAX_FRAME_BITS: usize = MAX_FRAGMENT_BITS * 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Wire(super::Error),
    FragmentLength,
    InvalidOrdinal,
    InvalidLimit,
    FrameLimit,
    Allocation,
}
impl From<super::Error> for Error {
    fn from(error: super::Error) -> Self {
        Self::Wire(error)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "application fragment {self:?}")
    }
}
impl std::error::Error for Error {}

/// A validated 16/6/15/1-bit fragment header plus exact borrowed data bits.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct Fragment<'a> {
    frame: u16,
    ordinal: u8,
    final_fragment: bool,
    data: BitSpan<'a>,
}
impl<'a> Fragment<'a> {
    /// The input must delimit exactly one listener body. Excess bits are an
    /// error of this bounded API, not a claim about all client malformed inputs.
    pub fn decode(input: BitSpan<'a>) -> Result<Self, Error> {
        let frame = input.read_u32(0, 16)? as u16;
        let ordinal = input.read_u32(16, 6)? as u8;
        let length = input.read_u32(22, 15)? as usize;
        let final_fragment = input.read_u32(37, 1)? != 0;
        let data = input.after(38)?;
        if data.len() != length {
            return Err(Error::FragmentLength);
        }
        Self::new(frame, ordinal, final_fragment, data)
    }

    /// Construct validated fields without serializing a header. No data copy.
    pub fn new(
        frame: u16,
        ordinal: u8,
        final_fragment: bool,
        data: BitSpan<'a>,
    ) -> Result<Self, Error> {
        if ordinal >= 64 {
            return Err(Error::InvalidOrdinal);
        }
        if data.len() > MAX_FRAGMENT_BITS {
            return Err(Error::FragmentLength);
        }
        Ok(Self {
            frame,
            ordinal,
            final_fragment,
            data,
        })
    }

    pub fn frame(self) -> u16 {
        self.frame
    }
    pub fn ordinal(self) -> u8 {
        self.ordinal
    }
    pub fn is_final(self) -> bool {
        self.final_fragment
    }
    pub fn data(self) -> BitSpan<'a> {
        self.data
    }
}
impl fmt::Debug for Fragment<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Fragment")
            .field("data_bits", &self.data.len())
            .field("final_fragment", &self.final_fragment)
            .finish_non_exhaustive()
    }
}

/// Borrowed accumulated data when the final flag invokes the downstream reader.
/// This does not latch the frame closed in the observed client. A subsequent
/// matching ordinal can extend it and a later final flag can deliver again.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct Frame<'a> {
    frame: u16,
    data: BitSpan<'a>,
    fragments: u8,
}
impl<'a> Frame<'a> {
    pub fn frame(self) -> u16 {
        self.frame
    }
    pub fn data(self) -> BitSpan<'a> {
        self.data
    }
    pub fn fragments(self) -> u8 {
        self.fragments
    }
}
impl fmt::Debug for Frame<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Frame")
            .field("bits", &self.data.len())
            .field("fragments", &self.fragments)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
#[must_use]
pub enum Outcome<'a> {
    Pending,
    Discarded,
    Complete(Frame<'a>),
}

/// One directional connection's alternate-listener state. No implicit global
/// registry, queue or default resource limit. Caller owns expiry and connection
/// routing. Borrowed completion prevents mutation until its view is released.
pub struct Assembler {
    limit: usize,
    previous: Option<u16>,
    next: u8,
    discarded: bool,
    bytes: Vec<u8>,
    bits: usize,
    fragments: u8,
}
impl Assembler {
    /// Explicit local policy, at most the wire's 64 * 32767 bits. Zero permits
    /// only empty fragments. Storage grows on demand and allocation is fallible.
    pub fn new(max_frame_bits: usize) -> Result<Self, Error> {
        if max_frame_bits > MAX_FRAME_BITS {
            return Err(Error::InvalidLimit);
        }
        Ok(Self {
            limit: max_frame_bits,
            previous: None,
            next: 0,
            discarded: false,
            bytes: Vec::new(),
            bits: 0,
            fragments: 0,
        })
    }

    pub fn buffered_bits(&self) -> usize {
        self.bits
    }

    /// Cancel current assembly (e.g. expiry or malformed input). Releases bytes
    /// but remembers the last identity: that frame cannot resume or restart.
    /// This is caller policy; no timeout value is inferred from the client.
    pub fn discard(&mut self) {
        self.discarded = true;
        self.next = 0;
        self.bytes = Vec::new();
        self.bits = 0;
        self.fragments = 0;
    }

    /// Forget all state and release storage for a new connection lifetime.
    pub fn reset(&mut self) {
        self.discard();
        self.previous = None;
        self.discarded = false;
    }

    pub fn push(&mut self, fragment: Fragment<'_>) -> Result<Outcome<'_>, Error> {
        let admissible = if self.previous != Some(fragment.frame) {
            fragment.ordinal == 0
        } else {
            fragment.ordinal == self.next && !self.discarded
        };
        self.previous = Some(fragment.frame);
        if !admissible {
            self.discard();
            return Ok(Outcome::Discarded);
        }
        if fragment.ordinal == 0 {
            self.bytes.clear();
            self.bits = 0;
            self.fragments = 0;
        }
        // Both operands are bounded by the wire's small fixed maxima.
        let new_bits = self.bits + fragment.data.len();
        if new_bits > self.limit {
            self.discard();
            return Err(Error::FrameLimit);
        }
        let byte_count = new_bits.div_ceil(8);
        if self
            .bytes
            .try_reserve_exact(byte_count - self.bytes.len())
            .is_err()
        {
            self.discard();
            return Err(Error::Allocation);
        }
        self.bytes.resize(byte_count, 0);
        for i in 0..fragment.data.len() {
            // BitSpan validates source bounds; the target is within new_bits.
            let source = fragment.data.start() + i;
            let bit = (fragment.data.bytes()[source / 8] >> (7 - source % 8)) & 1;
            let target = self.bits + i;
            self.bytes[target / 8] |= bit << (7 - target % 8);
        }
        self.bits = new_bits;
        self.fragments += 1;
        self.next = fragment.ordinal + 1; // 64 intentionally cannot match a wire ordinal.
        self.discarded = false;
        if fragment.final_fragment {
            Ok(Outcome::Complete(Frame {
                frame: fragment.frame,
                data: BitSpan::new(&self.bytes, 0, self.bits)?,
                fragments: self.fragments,
            }))
        } else {
            Ok(Outcome::Pending)
        }
    }
}
impl fmt::Debug for Assembler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Assembler")
            .field("limit", &self.limit)
            .field("buffered_bits", &self.bits)
            .field("discarded", &self.discarded)
            .finish_non_exhaustive()
    }
}

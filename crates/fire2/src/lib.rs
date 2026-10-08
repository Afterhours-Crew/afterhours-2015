//! Pure, bounded Fire2 framing.
//!
//! Metadata and body remain opaque: this crate does not implement heat2, service
//! handlers, transport, or authentication. Numeric categories, routing fields and
//! reserved bytes are retained without assigning unverified semantics.
//!
//! [`decode`] borrows one complete frame from a caller-owned buffer. [`Decoder`]
//! incrementally buffers at most one bounded frame, consumes only its bytes, and
//! returns a borrowed view. The caller owns backpressure and subsequent frames.

use std::fmt;

pub const HEADER_LEN: usize = 16;

/// Non-length header fields. Out-of-range values are rejected when encoding.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Fields {
    pub routing_a: u16,
    pub routing_b: u16,
    /// Wire width is 24 bits; the correlation interpretation is inferred.
    pub correlation: u32,
    /// Three raw bits, including the unresolved values 6 and 7.
    pub category: u8,
    /// Five raw bits.
    pub slot: u8,
    /// Preserved even when nonzero; no receiver validation was observed.
    pub reserved: [u8; 2],
}

/// A frame view; lengths are derived from the slices when encoding.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Frame<'a> {
    pub fields: Fields,
    pub metadata: &'a [u8],
    pub body: &'a [u8],
}

// Payloads may contain credentials or account state. Debug prints lengths only.
impl fmt::Debug for Frame<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Frame")
            .field("fields", &self.fields)
            .field("metadata_len", &self.metadata.len())
            .field("body_len", &self.body.len())
            .finish()
    }
}

/// Resource policy, independent of the client's format-width limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    max_frame: usize,
    max_metadata: usize,
    max_body: usize,
}

impl Limits {
    /// `max_frame` includes the header. Zero metadata/body limits are valid.
    pub fn new(max_frame: usize, max_metadata: usize, max_body: usize) -> Result<Self, Error> {
        if max_frame < HEADER_LEN {
            return Err(Error::InvalidLimits);
        }
        Ok(Self {
            max_frame,
            max_metadata,
            max_body,
        })
    }

    pub fn max_frame(self) -> usize {
        self.max_frame
    }
    pub fn max_metadata(self) -> usize {
        self.max_metadata
    }
    pub fn max_body(self) -> usize {
        self.max_body
    }

    fn total(self, metadata: usize, body: usize) -> Result<usize, Error> {
        check_limit("metadata", metadata, self.max_metadata)?;
        check_limit("body", body, self.max_body)?;
        let total = HEADER_LEN
            .checked_add(metadata)
            .and_then(|length| length.checked_add(body))
            .ok_or(Error::LengthOverflow)?;
        check_limit("frame", total, self.max_frame)?;
        Ok(total)
    }
}

impl Default for Limits {
    /// 512 KiB total is a local policy, not a verified protocol maximum.
    fn default() -> Self {
        Self {
            max_frame: 512 * 1024,
            max_metadata: u16::MAX as usize,
            max_body: 512 * 1024 - HEADER_LEN,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidLimits,
    FieldOutOfRange(&'static str),
    LimitExceeded {
        region: &'static str,
        actual: usize,
        maximum: usize,
    },
    LengthOverflow,
    AllocationFailed,
    /// Discard this stream or explicitly reset it after an earlier decode error.
    DecoderFailed,
    UnexpectedEof {
        received: usize,
        expected: usize,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLimits => write!(f, "frame limit must allow the 16-byte header"),
            Self::FieldOutOfRange(field) => write!(f, "{field} exceeds its wire width"),
            Self::LimitExceeded {
                region,
                actual,
                maximum,
            } => write!(f, "{region} length {actual} exceeds limit {maximum}"),
            Self::LengthOverflow => {
                write!(f, "frame length cannot be represented on this platform")
            }
            Self::AllocationFailed => write!(f, "could not allocate the bounded frame buffer"),
            Self::DecoderFailed => write!(f, "decoder requires reset after a previous failure"),
            Self::UnexpectedEof { received, expected } => {
                write!(f, "stream ended after {received} of {expected} frame bytes")
            }
        }
    }
}

impl std::error::Error for Error {}

fn check_limit(region: &'static str, actual: usize, maximum: usize) -> Result<(), Error> {
    if actual > maximum {
        Err(Error::LimitExceeded {
            region,
            actual,
            maximum,
        })
    } else {
        Ok(())
    }
}

struct Header {
    fields: Fields,
    metadata_len: usize,
    total_len: usize,
}

impl Header {
    fn parse(bytes: &[u8; HEADER_LEN], limits: Limits) -> Result<Self, Error> {
        let body_len = usize::try_from(u32::from_be_bytes(bytes[0..4].try_into().unwrap()))
            .map_err(|_| Error::LengthOverflow)?;
        let metadata_len = usize::from(u16::from_be_bytes([bytes[4], bytes[5]]));
        let total_len = limits.total(metadata_len, body_len)?;
        Ok(Self {
            fields: Fields {
                routing_a: u16::from_be_bytes([bytes[6], bytes[7]]),
                routing_b: u16::from_be_bytes([bytes[8], bytes[9]]),
                correlation: u32::from_be_bytes([0, bytes[10], bytes[11], bytes[12]]),
                category: bytes[13] >> 5,
                slot: bytes[13] & 0x1f,
                reserved: [bytes[14], bytes[15]],
            },
            metadata_len,
            total_len,
        })
    }
}

/// A complete frame and the exact byte count to remove from caller-owned input.
#[derive(Debug, PartialEq, Eq)]
pub struct Decoded<'a> {
    pub frame: Frame<'a>,
    pub consumed: usize,
}

/// Decode one frame without allocating or consuming incomplete input.
///
/// Length limits are checked as soon as the complete header is available, even
/// when the advertised payload is absent. Trailing concatenated frames are left
/// to the caller. `Ok(None)` means more bytes are required, not a valid EOF.
pub fn decode(input: &[u8], limits: Limits) -> Result<Option<Decoded<'_>>, Error> {
    let Some(bytes) = input.first_chunk::<HEADER_LEN>() else {
        return Ok(None);
    };
    let header = Header::parse(bytes, limits)?;
    if input.len() < header.total_len {
        return Ok(None);
    }
    let body_start = HEADER_LEN + header.metadata_len;
    Ok(Some(Decoded {
        frame: Frame {
            fields: header.fields,
            metadata: &input[HEADER_LEN..body_start],
            body: &input[body_start..header.total_len],
        },
        consumed: header.total_len,
    }))
}

/// Encode a frame, validating all widths and limits before allocating.
pub fn encode(frame: Frame<'_>, limits: Limits) -> Result<Vec<u8>, Error> {
    let metadata_len = u16::try_from(frame.metadata.len())
        .map_err(|_| Error::FieldOutOfRange("metadata length"))?;
    let body_len =
        u32::try_from(frame.body.len()).map_err(|_| Error::FieldOutOfRange("body length"))?;
    let fields = frame.fields;
    if fields.correlation > 0x00ff_ffff {
        return Err(Error::FieldOutOfRange("correlation"));
    }
    if fields.category > 7 {
        return Err(Error::FieldOutOfRange("category"));
    }
    if fields.slot > 31 {
        return Err(Error::FieldOutOfRange("slot"));
    }
    let total = limits.total(frame.metadata.len(), frame.body.len())?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(total)
        .map_err(|_| Error::AllocationFailed)?;
    output.extend_from_slice(&body_len.to_be_bytes());
    output.extend_from_slice(&metadata_len.to_be_bytes());
    output.extend_from_slice(&fields.routing_a.to_be_bytes());
    output.extend_from_slice(&fields.routing_b.to_be_bytes());
    output.extend_from_slice(&fields.correlation.to_be_bytes()[1..]);
    output.push((fields.category << 5) | fields.slot);
    output.extend_from_slice(&fields.reserved);
    output.extend_from_slice(frame.metadata);
    output.extend_from_slice(frame.body);
    Ok(output)
}

/// Incremental result. Always advance the supplied chunk by `consumed`.
#[derive(Debug)]
pub struct Step<'a> {
    pub consumed: usize,
    pub frame: Option<Frame<'a>>,
}

/// One decoder per input stream. Holds at most one frame and no output queue.
///
/// The next `push` clears a previously completed frame. A returned frame borrows
/// this decoder, so Rust prevents reuse until the caller releases the view.
/// Errors poison the decoder: it never guesses how to resynchronize malformed
/// input. [`Self::reset`] is intended for a new stream, not skipping bad bytes.
pub struct Decoder {
    limits: Limits,
    buffer: Vec<u8>,
    expected: Option<usize>,
    complete: bool,
    failed: bool,
}

impl Decoder {
    pub fn new(limits: Limits) -> Self {
        Self {
            limits,
            buffer: Vec::new(),
            expected: None,
            complete: false,
            failed: false,
        }
    }

    pub fn buffered_len(&self) -> usize {
        self.buffer.len()
    }

    pub fn reset(&mut self) {
        self.buffer.clear();
        self.expected = None;
        self.complete = false;
        self.failed = false;
    }

    fn append(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.buffer
            .try_reserve_exact(bytes.len())
            .map_err(|_| Error::AllocationFailed)?;
        self.buffer.extend_from_slice(bytes);
        Ok(())
    }

    fn ingest(&mut self, input: &[u8]) -> Result<usize, Error> {
        if self.failed {
            return Err(Error::DecoderFailed);
        }
        if self.complete {
            self.reset();
        }
        let mut consumed = 0;
        if self.expected.is_none() {
            // Reserve once for the header, then once for the validated frame.
            // Byte-at-a-time input must not cause one reallocation per byte.
            self.buffer
                .try_reserve_exact(HEADER_LEN - self.buffer.len())
                .map_err(|_| Error::AllocationFailed)?;
            consumed = (HEADER_LEN - self.buffer.len()).min(input.len());
            self.append(&input[..consumed])?;
            let Some(bytes) = self.buffer.first_chunk::<HEADER_LEN>() else {
                return Ok(consumed);
            };
            self.expected = Some(Header::parse(bytes, self.limits)?.total_len);
            self.buffer
                .try_reserve_exact(self.expected.unwrap() - self.buffer.len())
                .map_err(|_| Error::AllocationFailed)?;
        }
        // The expected length only exists after the header has passed all limits.
        let expected = self.expected.unwrap();
        let count = (expected - self.buffer.len()).min(input.len() - consumed);
        self.append(&input[consumed..consumed + count])?;
        self.complete = self.buffer.len() == expected;
        Ok(consumed + count)
    }

    pub fn push(&mut self, input: &[u8]) -> Result<Step<'_>, Error> {
        let consumed = match self.ingest(input) {
            Ok(count) => count,
            Err(error) => {
                self.failed = true;
                return Err(error);
            }
        };
        let frame = if self.complete {
            decode(&self.buffer, self.limits)?.map(|decoded| decoded.frame)
        } else {
            None
        };
        Ok(Step { consumed, frame })
    }

    /// Check stream EOF. An empty stream or an already-emitted frame is complete.
    pub fn finish(&self) -> Result<(), Error> {
        if self.failed {
            return Err(Error::DecoderFailed);
        }
        if self.buffer.is_empty() || self.complete {
            return Ok(());
        }
        Err(Error::UnexpectedEof {
            received: self.buffer.len(),
            expected: self.expected.unwrap_or(HEADER_LEN),
        })
    }
}

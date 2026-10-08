// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! NUL-delimited messages on the launcher stream : the sender
//! writes the string plus one NUL; the receiver scans for the first zero
//! byte, consumes through it and rescans the remainder, so fragments and
//! concatenated messages both work.

use std::fmt;

/// Local ceilings for one connection: the largest message and the pending
/// bytes kept while waiting for a delimiter.
pub const MAX_FRAME: usize = 1 << 16;
pub const MAX_PENDING: usize = MAX_FRAME * 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// A frame or the pending buffer exceeded its ceiling; the input that
    /// overflowed is not kept.
    Bound,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "lsx frame {self:?}")
    }
}
impl std::error::Error for Error {}

/// Receive-side reassembly.
#[derive(Clone, Debug, Default)]
pub struct Frames {
    pending: Vec<u8>,
}

impl Frames {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append received bytes. Fails without keeping anything when the pending
    /// buffer would exceed [`MAX_PENDING`] or any message reaches [`MAX_FRAME`]
    /// bytes (which reserves one byte for its delimiter).
    pub fn feed(&mut self, bytes: &[u8]) -> Result<(), Error> {
        if bytes.len() > MAX_PENDING || self.pending.len() > MAX_PENDING - bytes.len() {
            return Err(Error::Bound);
        }
        let mut length = 0;
        for &byte in self.pending.iter().chain(bytes) {
            if byte == 0 {
                length = 0;
            } else {
                length += 1;
                if length >= MAX_FRAME {
                    return Err(Error::Bound);
                }
            }
        }
        self.pending.extend_from_slice(bytes);
        Ok(())
    }

    /// The next complete message without its delimiter, if one is pending.
    pub fn next_frame(&mut self) -> Option<Vec<u8>> {
        let end = self.pending.iter().position(|b| *b == 0)?;
        let frame = self.pending[..end].to_vec();
        self.pending.drain(..=end);
        Some(frame)
    }

    pub fn pending(&self) -> usize {
        self.pending.len()
    }
}

/// One message as sent: the bytes plus the trailing NUL. `None` when the
/// message is too large or itself contains a NUL (it could not be delimited).
pub fn encode(message: &[u8]) -> Option<Vec<u8>> {
    if message.len() >= MAX_FRAME || message.contains(&0) {
        return None;
    }
    let mut out = Vec::with_capacity(message.len() + 1);
    out.extend_from_slice(message);
    out.push(0);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_split_of_two_frames_and_a_tail_yields_the_same_frames() {
        let wire = b"alpha\0beta\0tail";
        for split in 0..=wire.len() {
            let mut frames = Frames::new();
            frames.feed(&wire[..split]).unwrap();
            let mut seen = Vec::new();
            while let Some(f) = frames.next_frame() {
                seen.push(f);
            }
            frames.feed(&wire[split..]).unwrap();
            while let Some(f) = frames.next_frame() {
                seen.push(f);
            }
            assert_eq!(seen, [b"alpha".to_vec(), b"beta".to_vec()]);
            assert_eq!(frames.pending(), 4);
        }
    }

    #[test]
    fn empty_frames_and_unfinished_tails() {
        let mut frames = Frames::new();
        frames.feed(b"\0\0A\0unfinished").unwrap();
        assert_eq!(frames.next_frame(), Some(vec![]));
        assert_eq!(frames.next_frame(), Some(vec![]));
        assert_eq!(frames.next_frame(), Some(b"A".to_vec()));
        assert_eq!(frames.next_frame(), None);
        assert_eq!(frames.pending(), 10);
    }

    #[test]
    fn bounds_reject_without_mutating() {
        let mut frames = Frames::new();
        frames.feed(b"A").unwrap();
        assert_eq!(frames.feed(&vec![b'B'; MAX_PENDING]), Err(Error::Bound));
        assert_eq!(frames.pending(), 1);
        assert_eq!(encode(b"<LSX/>"), Some(b"<LSX/>\0".to_vec()));
        assert_eq!(encode(b"a\0b"), None);
        assert_eq!(encode(&vec![b'x'; MAX_FRAME]), None);
    }

    #[test]
    fn per_frame_limit_applies_to_fragmented_and_concatenated_input_atomically() {
        let maximum = vec![b'x'; MAX_FRAME - 1];
        let wire = encode(&maximum).unwrap();
        let mut frames = Frames::new();
        frames.feed(&maximum).unwrap();
        assert_eq!(frames.feed(b"x\0"), Err(Error::Bound));
        assert_eq!(frames.pending(), maximum.len());
        frames.feed(b"\0").unwrap();
        assert_eq!(frames.next_frame(), Some(maximum.clone()));
        frames
            .feed(&[wire.as_slice(), wire.as_slice()].concat())
            .unwrap();
        assert_eq!(frames.next_frame(), Some(maximum.clone()));
        assert_eq!(frames.next_frame(), Some(maximum));

        frames.feed(b"retained\0").unwrap();
        let mut invalid = b"valid\0".to_vec();
        invalid.extend(vec![b'x'; MAX_FRAME]);
        invalid.push(0);
        assert_eq!(frames.feed(&invalid), Err(Error::Bound));
        assert_eq!(frames.next_frame(), Some(b"retained".to_vec()));
        assert_eq!(frames.pending(), 0);
    }
}

//! Component initialization and serial arithmetic.
use super::{BitSpan, Error};
use std::fmt;

/// A validated 10-bit component serial. Zero is valid on input, although the
/// initialization writer skips it on wrap. No default or total ordering is
/// provided: initial component state must come from the caller's evidence.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct Serial(u16);

impl Serial {
    pub const fn new(value: u16) -> Option<Self> {
        if value < 1024 {
            Some(Self(value))
        } else {
            None
        }
    }

    pub const fn value(self) -> u16 {
        self.0
    }

    /// The context-0 receive predicate; equality passes. This does not suppress
    /// duplicates, resolve an object, authorize a method or mutate stored state.
    /// Only compare serials belonging to the same established component lifetime.
    pub const fn accepts(self, incoming: Self) -> bool {
        incoming.0.wrapping_sub(self.0) & 1023 < 512
    }

    /// Value emitted on the next initialization-writer invocation. This is not
    /// an ordinary-RPC counter or a promise that the creation was delivered.
    pub const fn next_initialization(self) -> Self {
        let next = (self.0 + 1) & 1023;
        Self(if next == 0 { 1 } else { next })
    }

    pub(super) fn read(input: BitSpan<'_>, offset: usize) -> Result<Self, Error> {
        // The bit reader guarantees a value in 0..=1023.
        Ok(Self(input.read_u32(offset, 10)? as u16))
    }
}

impl fmt::Debug for Serial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Serial").finish_non_exhaustive()
    }
}

/// Fixed 19-bit selector/serial initialization prefix, followed by an opaque
/// borrowed suffix. Does not register a component or consume its remaining body.
#[derive(Eq, PartialEq)]
pub struct Initialization<'a> {
    selector: u16,
    serial: Serial,
    remaining: BitSpan<'a>,
}

impl<'a> Initialization<'a> {
    /// Constant work, no allocation. The supplied span is the exclusive bound;
    /// a later component, following frame or byte padding cannot repair truncation.
    pub fn decode(input: BitSpan<'a>) -> Result<Self, Error> {
        Ok(Self {
            selector: input.read_u32(0, 9)? as u16,
            serial: Serial::read(input, 9)?,
            remaining: input.after(19)?,
        })
    }

    pub fn selector(&self) -> u16 {
        self.selector
    }

    pub fn serial(&self) -> Serial {
        self.serial
    }

    pub fn remaining(&self) -> BitSpan<'a> {
        self.remaining
    }
}

impl fmt::Debug for Initialization<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Initialization")
            .field("remaining", &self.remaining)
            .finish_non_exhaustive()
    }
}

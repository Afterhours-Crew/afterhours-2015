//! Type-47 entity RPC envelope and routing prefix.
//! No method execution, target resolution or serial state.
use super::{BitSpan, Error};
use std::fmt;

mod component;
pub use component::{Initialization, Serial};

/// Caller bounds may be stricter than the 8-bit reference count and the
/// supported build's 256-byte inline storage. There are no implicit defaults.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    pub max_input_bits: usize,
    pub max_references: usize,
    pub max_payload_bytes: usize,
}

/// These direction/context correspondences are inferred by the producer,
/// consumer and corpus agreement. Keep the choice explicit for other contexts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RouteProfile {
    ClientReceive,
    ClientSend,
}

/// Borrowed payload and remaining bits; references are the original 13-bit IDs.
/// Empty/null reference lists are preserved, not treated as valid dispatch.
#[derive(Eq, PartialEq)]
pub struct Envelope<'a> {
    words: [u32; 2],
    references: Vec<u16>,
    payload: BitSpan<'a>,
    remaining: BitSpan<'a>,
}

impl fmt::Debug for Envelope<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Envelope")
            .field("references", &self.references.len())
            .field("payload_bits", &self.payload.len())
            .field("remaining", &self.remaining)
            .finish_non_exhaustive()
    }
}

impl<'a> Envelope<'a> {
    /// Start at the type-47 body (after its message type). Strictly rejects
    /// encoded sizes over 256 even if the original range reader did not clamp.
    pub fn decode(input: BitSpan<'a>, limits: Limits) -> Result<Self, Error> {
        if input.len() > limits.max_input_bits {
            return Err(Error::InputLimit);
        }
        let words = [input.read_u32(0, 32)?, input.read_u32(32, 32)?];
        let count = input.read_u32(64, 8)? as usize;
        if count > limits.max_references {
            return Err(Error::ReferenceLimit);
        }
        let mut cursor = 72;
        let mut references = Vec::new();
        for _ in 0..count {
            references.push(input.read_u32(cursor, 13)? as u16);
            cursor += 13;
        }
        let size = input.read_u32(cursor, 9)? as usize;
        cursor += 9;
        if size > 256 || size > limits.max_payload_bytes {
            return Err(Error::PayloadLimit);
        }
        let payload = input.slice(cursor, size * 8)?;
        Ok(Self {
            words,
            references,
            payload,
            remaining: input.after(cursor + size * 8)?,
        })
    }

    /// The two leading uninterpreted fields; do not assign sequence semantics.
    pub fn words(&self) -> [u32; 2] {
        self.words
    }

    /// First reference routes the call; subsequent ones may be method arguments.
    /// Lifetime, null and ownership interpretation belongs to the caller.
    pub fn references(&self) -> &[u16] {
        &self.references
    }

    pub fn payload(&self) -> BitSpan<'a> {
        self.payload
    }

    pub fn remaining(&self) -> BitSpan<'a> {
        self.remaining
    }

    /// Decode only the inline routing fields. Even an envelope with no target
    /// can contain this syntax; this operation never resolves or dispatches it.
    pub fn route(&self, profile: RouteProfile) -> Result<Route<'a>, Error> {
        let selector = self.payload.read_u32(0, 9)? as u16;
        let (serial, offset) = match profile {
            RouteProfile::ClientReceive => (Some(Serial::read(self.payload, 9)?), 19),
            RouteProfile::ClientSend => (None, 9),
        };
        let method = self.payload.read_u32(offset, 32)?;
        Ok(Route {
            selector,
            serial,
            method,
            arguments: self.payload.after(offset + 32)?,
        })
    }
}

/// Opaque inline arguments can include padding. Side references remain in the
/// envelope and may carry additional arguments even when no inline bits remain.
#[derive(Eq, PartialEq)]
pub struct Route<'a> {
    selector: u16,
    serial: Option<Serial>,
    method: u32,
    arguments: BitSpan<'a>,
}

impl fmt::Debug for Route<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Route")
            .field("has_serial", &self.serial.is_some())
            .field("argument_bits", &self.arguments.len())
            .finish_non_exhaustive()
    }
}

impl<'a> Route<'a> {
    pub fn selector(&self) -> u16 {
        self.selector
    }
    pub fn serial(&self) -> Option<Serial> {
        self.serial
    }
    /// Raw component-local index, including unknown/out-of-range values. Before
    /// dispatch, enforce the observed 32-slot bound AND that a wrapper exists.
    /// This codec performs no table allocation, lookup or unchecked indexing.
    pub fn method_index(&self) -> u32 {
        self.method
    }
    pub fn arguments(&self) -> BitSpan<'a> {
        self.arguments
    }
}

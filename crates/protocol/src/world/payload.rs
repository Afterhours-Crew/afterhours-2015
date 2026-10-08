// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Application payload prefixes and raw parameter representations.
//! Decode only after selecting a sequenced envelope and checking its integrity.
//! This module neither generates timing replies nor assigns parameter policy.
use super::{BitSpan, Error};
use std::fmt;

/// Raw optional 20-bit value and 31-bit nonnegative float representation.
/// Their policy meaning is not established. Preserve unusual float bits exactly.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct Advertised {
    value: u32,
    float_bits: u32,
}
impl Advertised {
    pub fn value(self) -> u32 {
        self.value
    }
    pub fn float_bits(self) -> u32 {
        self.float_bits
    }
}
impl fmt::Debug for Advertised {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Advertised").finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum Route<'a> {
    Queued {
        advertised: Option<Advertised>,
        body: BitSpan<'a>,
    },
    Control {
        subtype: u8,
        /// Subtype 2: one timestamp. Subtype 3: echoed and sender timestamps.
        /// Unknown subtypes: neither. Values are exact IEEE-754 bit patterns.
        timestamps: [Option<u64>; 2],
        body: BitSpan<'a>,
    },
}
impl<'a> Route<'a> {
    /// Consume at most 133 prefix bits; preserve all trailing bits, including
    /// those following a known control. No allocation or unbounded iteration.
    pub fn decode(payload: BitSpan<'a>) -> Result<Self, Error> {
        if payload.read_u32(0, 1)? == 0 {
            let advertised = if payload.read_u32(1, 1)? == 1 {
                Some(Advertised {
                    value: payload.read_u32(2, 20)?,
                    float_bits: payload.read_u32(22, 31)?,
                })
            } else {
                None
            };
            Ok(Self::Queued {
                advertised,
                body: payload.after(if advertised.is_some() { 53 } else { 2 })?,
            })
        } else {
            let subtype = payload.read_u32(1, 4)? as u8;
            let word = |offset| -> Result<u64, Error> {
                Ok((u64::from(payload.read_u32(offset, 32)?) << 32)
                    | u64::from(payload.read_u32(offset + 32, 32)?))
            };
            let (timestamps, prefix) = match subtype {
                2 => ([Some(word(5)?), None], 69),
                3 => ([Some(word(5)?), Some(word(69)?)], 133),
                _ => ([None, None], 5),
            };
            Ok(Self::Control {
                subtype,
                timestamps,
                body: payload.after(prefix)?,
            })
        }
    }
}
impl fmt::Debug for Route<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Queued { advertised, body } => f
                .debug_struct("Queued")
                .field("advertised", &advertised.is_some())
                .field("body", body)
                .finish_non_exhaustive(),
            Self::Control { subtype, body, .. } => f
                .debug_struct("Control")
                .field("subtype", subtype)
                .field("body", body)
                .finish_non_exhaustive(),
        }
    }
}

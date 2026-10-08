// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Directional setup profiles and packed-value decoding.
use crate::world::{BitSpan, Error};
use std::fmt;

/// Directions are from the NFS client viewpoint; never auto-detect a profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SetupProfile {
    /// Normal receive setup; historical selection is inferred from corpus fit.
    ClientReceivePacked,
    /// Optional packed-value path; not validated against live traffic.
    ClientReceiveForcedRaw,
    /// Outgoing setup changes reader context without consuming wire bits.
    ClientSend,
}

/// Encoded setup values, without sign extension, scaling, origin or float casts.
/// Missing axes retain absence; Debug omits all values.
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum Setup {
    ContextOnly,
    Packed {
        /// Original tag, including tag zero on a first present Y/Z axis.
        tag: Option<u8>,
        width: Option<u8>,
        axes: [Option<u32>; 3],
    },
    RawEscape([u32; 3]),
    ForcedRaw([Option<u32>; 3]),
}

impl fmt::Debug for Setup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ContextOnly => "Setup::ContextOnly",
            Self::Packed { .. } => "Setup::Packed { .. }",
            Self::RawEscape(_) => "Setup::RawEscape { .. }",
            Self::ForcedRaw(_) => "Setup::ForcedRaw { .. }",
        })
    }
}

fn decode_setup(input: BitSpan<'_>, profile: SetupProfile) -> Result<(Setup, BitSpan<'_>), Error> {
    let mut cursor = 0;
    let mut take = |width| {
        let value = input.read_u32(cursor, width)?;
        cursor += usize::from(width);
        Ok::<_, Error>(value)
    };
    let value = match profile {
        SetupProfile::ClientSend => Setup::ContextOnly,
        SetupProfile::ClientReceiveForcedRaw => {
            let mut axes = [None; 3];
            for axis in &mut axes {
                if take(1)? != 0 {
                    *axis = Some(take(32)?);
                }
            }
            Setup::ForcedRaw(axes)
        }
        SetupProfile::ClientReceivePacked => {
            let mut axes = [None; 3];
            let mut tag = None;
            let mut width = None;
            for (index, axis) in axes.iter_mut().enumerate() {
                if take(1)? == 0 {
                    continue;
                }
                let bits = match width {
                    Some(bits) => bits,
                    None => {
                        let selected = take(3)? as u8;
                        if index == 0 && selected == 0 {
                            let words = [take(32)?, take(32)?, take(32)?];
                            return Ok((Setup::RawEscape(words), input.after(cursor)?));
                        }
                        let bits = [1, 3, 5, 7, 9, 11, 14][usize::from(selected.max(1) - 1)];
                        tag = Some(selected);
                        width = Some(bits);
                        bits
                    }
                };
                *axis = Some(take(bits)?);
            }
            Setup::Packed { tag, width, axes }
        }
    };
    Ok((value, input.after(cursor)?))
}

/// First creation/update header only. No lifetime lookup or body validation.
#[derive(Eq, PartialEq)]
pub struct FirstRecord<'a> {
    setup: Setup,
    object_id: u32,
    creation: bool,
    body: BitSpan<'a>,
}

impl fmt::Debug for FirstRecord<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FirstRecord")
            .field("setup", &self.setup)
            .field("creation", &self.creation)
            .field("body", &self.body)
            .finish_non_exhaustive()
    }
}

impl<'a> FirstRecord<'a> {
    pub(super) fn decode(
        input: BitSpan<'a>,
        id_bits: u8,
        profile: SetupProfile,
    ) -> Result<Self, Error> {
        let (setup, input) = decode_setup(input, profile)?;
        let object_id = input.read_u32(0, id_bits)?;
        let creation = input.read_u32(usize::from(id_bits), 1)? != 0;
        Ok(Self {
            setup,
            object_id,
            creation,
            body: input.after(usize::from(id_bits) + 1)?,
        })
    }

    pub fn setup(&self) -> Setup {
        self.setup
    }

    pub fn object_id(&self) -> u32 {
        self.object_id
    }

    pub fn is_creation(&self) -> bool {
        self.creation
    }

    pub fn body(&self) -> BitSpan<'a> {
        self.body
    }
}

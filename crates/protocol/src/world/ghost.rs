//! Ghost section prefix codec.
//! The first false deletion bit is consumed before direction-specific setup.
//! Zero/all-deleted counts have no final false bit. Unknown bodies stay opaque.

use super::{BitSpan, Error};

mod setup;
pub use setup::{FirstRecord, Setup, SetupProfile};

/// Widths must come from the connection's observed profile, not guessed bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Profile {
    count_bits: u8,
    id_bits: u8,
}

impl Profile {
    /// supported build profile. Not a default for other builds.
    pub const NFS16_92AA6FF4: Self = Self {
        count_bits: 13,
        id_bits: 13,
    };

    /// Support explicit 1..=32-bit profiles; zero-width fields are unverified.
    pub fn new(count_bits: u8, id_bits: u8) -> Result<Self, Error> {
        if !(1..=32).contains(&count_bits) || !(1..=32).contains(&id_bits) {
            return Err(Error::InvalidWidth);
        }
        Ok(Self {
            count_bits,
            id_bits,
        })
    }
}

/// Caller resource policy, independent of the wire profile. No implicit defaults.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    /// Bounds the entire supplied range, including its still-opaque suffix.
    pub max_input_bits: usize,
    pub max_records: u32,
    /// Maximum stored deletion IDs. No allocation is made for unknown bodies.
    pub max_deletions: usize,
}

/// Validated header and leading deletions. This is not a complete Ghost section.
/// Fields are immutable so the remaining record count cannot underflow.
#[derive(Eq, PartialEq)]
pub struct Prefix<'a> {
    float_bits: Option<u32>,
    flag: bool,
    record_count: u32,
    deleted: Vec<u32>,
    id_bits: u8,
    remaining: BitSpan<'a>,
}

impl std::fmt::Debug for Prefix<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Prefix")
            .field("has_float", &self.float_bits.is_some())
            .field("flag", &self.flag)
            .field("record_count", &self.record_count)
            .field("deletions", &self.deleted.len())
            .field("remaining", &self.remaining)
            .finish()
    }
}

impl<'a> Prefix<'a> {
    /// Decode at an already established Ghost handler boundary. Success does not
    /// validate record bodies or imply that the whole frame has been consumed.
    pub fn decode(input: BitSpan<'a>, profile: Profile, limits: Limits) -> Result<Self, Error> {
        if input.len() > limits.max_input_bits {
            return Err(Error::InputLimit);
        }
        let mut cursor = 0;
        let mut take = |width| {
            let value = input.read_u32(cursor, width)?;
            cursor += usize::from(width);
            Ok::<_, Error>(value)
        };
        let float_bits = if take(1)? != 0 { Some(take(31)?) } else { None };
        let flag = take(1)? != 0;
        let record_count = take(profile.count_bits)?;
        if record_count > limits.max_records {
            return Err(Error::RecordLimit);
        }
        let mut deleted = Vec::new();
        let mut records_left = record_count;
        while records_left != 0 {
            if take(1)? == 0 {
                break;
            }
            if deleted.len() == limits.max_deletions {
                return Err(Error::DeletionLimit);
            }
            // Read before allocating: an advertised count alone allocates nothing.
            deleted.push(take(profile.id_bits)?);
            records_left -= 1;
        }
        Ok(Self {
            float_bits,
            flag,
            record_count,
            deleted,
            id_bits: profile.id_bits,
            remaining: input.after(cursor)?,
        })
    }

    /// The optional 31-bit unsigned representation, preserved without float casts.
    pub fn float_bits(&self) -> Option<u32> {
        self.float_bits
    }

    /// Observed one-bit context flag; no gameplay meaning is assigned.
    pub fn flag(&self) -> bool {
        self.flag
    }

    pub fn record_count(&self) -> u32 {
        self.record_count
    }

    pub fn deleted(&self) -> &[u32] {
        &self.deleted
    }

    pub fn remaining_records(&self) -> u32 {
        self.record_count - self.deleted.len() as u32
    }

    /// Nonempty records: starts at setup, before the first creation/update ID.
    /// No records: starts at the next handler or frame padding. Never align it
    /// automatically; the caller owns listener ordering and the frame boundary.
    pub fn remaining(&self) -> BitSpan<'a> {
        self.remaining
    }

    /// Decode setup and the first non-deletion header, leaving its body opaque.
    /// The profile is explicit: a captured bit stream cannot choose the runtime
    /// force-raw switch. Empty sections never invoke setup or inspect the suffix.
    pub fn first_record(&self, setup: SetupProfile) -> Result<Option<FirstRecord<'a>>, Error> {
        if self.remaining_records() == 0 {
            return Ok(None);
        }
        FirstRecord::decode(self.remaining, self.id_bits, setup).map(Some)
    }
}

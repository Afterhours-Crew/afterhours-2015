// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Evidence-backed backend payloads and bounded world codecs. No service policy.
//! Strings borrow bytes without assuming UTF-8. Optional fields preserve absence;
//! decoded unknown fields preserve their complete bytes. Canonical encoding sorts
//! struct tags, retains collection order, and rejects duplicate tags/map keys.
use nfs_heat2::{Encoder, Item, Kind, Limits, Value};
use std::{collections::BTreeSet, fmt};

pub mod association;
pub mod authentication;
pub mod autolog;
pub mod challenge;
pub mod gamemanager;
pub mod items;
pub mod kickback;
pub mod metadata;
pub mod qos;
pub mod redirector;
pub mod speedlist;
pub mod stats;
pub mod users;
pub mod util;
pub mod world;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Wire(nfs_heat2::Error),
    WrongType { tag: Option<[u8; 3]> },
    InvalidInteger { tag: Option<[u8; 3]> },
    DuplicateTag([u8; 3]),
    KnownTagInUnknown([u8; 3]),
    DuplicateMapKey,
    CollectionLimit,
    ValueLimit,
    ByteStringLimit,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "protocol {self:?}")
    }
}
impl std::error::Error for Error {}
impl From<nfs_heat2::Error> for Error {
    fn from(error: nfs_heat2::Error) -> Self {
        Self::Wire(error)
    }
}
impl Error {
    fn with_tag(self, tag: [u8; 3]) -> Self {
        match self {
            Self::WrongType { tag: None } => Self::WrongType { tag: Some(tag) },
            Self::InvalidInteger { tag: None } => Self::InvalidInteger { tag: Some(tag) },
            other => other,
        }
    }
}

struct Budget {
    limits: Limits,
    remaining: usize,
}
impl Budget {
    fn take(&mut self, count: usize) -> Result<(), Error> {
        if count > self.remaining {
            return Err(Error::ValueLimit);
        }
        self.remaining -= count;
        Ok(())
    }
    fn collection(&mut self, count: usize, multiplier: usize) -> Result<(), Error> {
        if count > self.limits.max_collection {
            return Err(Error::CollectionLimit);
        }
        self.take(count.checked_mul(multiplier).ok_or(Error::ValueLimit)?)
    }
}

trait Wire<'a>: Sized {
    const KIND: Kind;
    fn read(item: Item<'a>) -> Result<Self, Error>;
    fn write(&self, tag: [u8; 3], writer: &mut Encoder) -> Result<(), nfs_heat2::Error>;
    fn validate(&self, budget: &mut Budget) -> Result<(), Error> {
        budget.take(1)
    }
    fn unknown_count(&self) -> usize {
        0
    }
}

impl<'a> Wire<'a> for &'a [u8] {
    const KIND: Kind = Kind::String;
    fn read(item: Item<'a>) -> Result<Self, Error> {
        if let Value::String(bytes) = item.value() {
            Ok(bytes)
        } else {
            Err(Error::WrongType { tag: None })
        }
    }
    fn write(&self, tag: [u8; 3], w: &mut Encoder) -> Result<(), nfs_heat2::Error> {
        w.string(tag, self)
    }
    fn validate(&self, budget: &mut Budget) -> Result<(), Error> {
        budget.take(1)?;
        check_string(self, budget.limits)
    }
}

/// Borrowed opaque bytes. Unlike strings, blobs have no trailing NUL on the wire.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct Blob<'a>(pub &'a [u8]);
impl fmt::Debug for Blob<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Blob")
            .field("bytes", &self.0.len())
            .finish()
    }
}
impl<'a> Wire<'a> for Blob<'a> {
    const KIND: Kind = Kind::Blob;
    fn read(item: Item<'a>) -> Result<Self, Error> {
        if let Value::Blob(bytes) = item.value() {
            Ok(Self(bytes))
        } else {
            Err(Error::WrongType { tag: None })
        }
    }
    fn write(&self, tag: [u8; 3], w: &mut Encoder) -> Result<(), nfs_heat2::Error> {
        w.blob(tag, self.0)
    }
    fn validate(&self, budget: &mut Budget) -> Result<(), Error> {
        budget.take(1)?;
        if self.0.len() > budget.limits.max_byte_string {
            return Err(Error::ByteStringLimit);
        }
        Ok(())
    }
}

fn check_string(bytes: &[u8], limits: Limits) -> Result<(), Error> {
    if bytes.len() >= limits.max_byte_string {
        Err(Error::ByteStringLimit)
    } else {
        Ok(())
    }
}

fn integer(item: Item<'_>) -> Result<i64, Error> {
    if let Value::Integer(value) = item.value() {
        Ok(value)
    } else {
        Err(Error::WrongType { tag: None })
    }
}
macro_rules! integer_type {
    ($t:ty) => {
        impl<'a> Wire<'a> for $t {
            const KIND: Kind = Kind::Integer;
            fn read(item: Item<'a>) -> Result<Self, Error> {
                Self::try_from(integer(item)?).map_err(|_| Error::InvalidInteger { tag: None })
            }
            fn write(&self, tag: [u8; 3], w: &mut Encoder) -> Result<(), nfs_heat2::Error> {
                w.integer(tag, i64::from(*self))
            }
        }
    };
}
integer_type!(u8);
integer_type!(u16);
integer_type!(u32);
integer_type!(i32);
integer_type!(i64);
// : uint64 and int64 descriptor classes select identical heat2 callbacks.
// Preserve the 64-bit pattern; the wire sign bit is not an unsigned range check.
impl<'a> Wire<'a> for u64 {
    const KIND: Kind = Kind::Integer;
    fn read(item: Item<'a>) -> Result<Self, Error> {
        Ok(Self::from_ne_bytes(integer(item)?.to_ne_bytes()))
    }
    fn write(&self, tag: [u8; 3], w: &mut Encoder) -> Result<(), nfs_heat2::Error> {
        w.integer(tag, i64::from_ne_bytes(self.to_ne_bytes()))
    }
}
impl<'a> Wire<'a> for bool {
    const KIND: Kind = Kind::Integer;
    fn read(item: Item<'a>) -> Result<Self, Error> {
        match integer(item)? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Error::InvalidInteger { tag: None }),
        }
    }
    fn write(&self, tag: [u8; 3], w: &mut Encoder) -> Result<(), nfs_heat2::Error> {
        w.integer(tag, i64::from(*self))
    }
}

fn unique_keys<'a>(keys: impl Iterator<Item = &'a [u8]>) -> Result<(), Error> {
    let mut seen = BTreeSet::new();
    for key in keys {
        if !seen.insert(key) {
            return Err(Error::DuplicateMapKey);
        }
    }
    Ok(())
}

// Scalar names and raw tags come from the recorded client metadata.
macro_rules! schema {
    ($name:ident { $($field:ident : $ty:ty => $tag:expr),+ $(,)? }) => {
        schema!(@impl $name { $($field: $ty => $tag),+ }, nfs_heat2::decode, Encoder::finish);
    };
    ($name:ident { $($field:ident : $ty:ty => $tag:expr),+ $(,)? }, $decode:path, $finish:path) => {
        schema!(@impl $name { $($field: $ty => $tag),+ }, $decode, $finish);
    };
    (@impl $name:ident { $($field:ident : $ty:ty => $tag:expr),+ }, $decode:path, $finish:path) => {
        #[derive(Default)]
        pub struct $name<'a> {
            $(pub $field: Option<$ty>,)+
            pub unknown: Vec<Field<'a>>,
        }
        impl fmt::Debug for $name<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($name)).field("unknown_fields", &self.unknown.len()).finish_non_exhaustive()
            }
        }
        impl<'a> $name<'a> {
            pub const FIELD_TAGS: &'static [[u8; 3]] = &[$($tag),+];
            pub const FIELD_KINDS: &'static [Kind] = &[$(<$ty as Wire>::KIND),+];
            pub fn unknown_field_count(&self) -> usize {
                self.unknown.len() $(+ self.$field.as_ref().map_or(0, Wire::unknown_count))+
            }
            pub fn decode(bytes: &'a [u8], limits: Limits) -> Result<Self, Error> {
                Self::from_fields($decode(bytes, limits)?.fields())
            }
            fn from_fields(fields: Fields<'a>) -> Result<Self, Error> {
                let mut result = Self::default();
                let mut seen = BTreeSet::new();
                for field in fields {
                    let field = field?;
                    if !seen.insert(field.tag()) { return Err(Error::DuplicateTag(field.tag())); }
                    match field.tag() {
                        $(tag if tag == $tag => result.$field = Some(<$ty as Wire>::read(field.item()).map_err(|e| e.with_tag(tag))?),)+
                        _ => result.unknown.push(field),
                    }
                }
                Ok(result)
            }
            /// Canonical tag order. Absence is retained; no schema defaults are invented.
            pub fn encode(&self, limits: Limits) -> Result<Vec<u8>, Error> {
                self.validate_fields(&mut crate::Budget { limits, remaining: limits.max_values })?;
                let mut writer = Encoder::new(limits);
                self.write_fields(&mut writer)?;
                Ok($finish(writer)?)
            }
            fn validate_fields(&self, budget: &mut crate::Budget) -> Result<(), Error> {
                if self.unknown.len() > budget.remaining { return Err(Error::ValueLimit); }
                let mut seen = BTreeSet::new();
                for field in &self.unknown {
                    if Self::FIELD_TAGS.contains(&field.tag()) { return Err(Error::KnownTagInUnknown(field.tag())); }
                    if !seen.insert(field.tag()) { return Err(Error::DuplicateTag(field.tag())); }
                    let limits = Limits { max_values: budget.remaining, ..budget.limits };
                    budget.take(nfs_heat2::decode(field.as_bytes(), limits)?.stats().values)?;
                }
                $(if let Some(value) = &self.$field { value.validate(budget)?; })+
                Ok(())
            }
            pub(crate) fn write_fields(&self, writer: &mut Encoder) -> Result<(), nfs_heat2::Error> {
                let mut unknown: Vec<_> = self.unknown.iter().copied().collect();
                unknown.sort_by_key(|field| field.tag());
                let mut unknown = unknown.into_iter().peekable();
                $(
                    while unknown.peek().is_some_and(|field| field.tag() < $tag) {
                        if let Some(field) = unknown.next() { writer.raw_field(field)?; }
                    }
                    if let Some(value) = &self.$field { value.write($tag, writer)?; }
                )+
                for field in unknown { writer.raw_field(field)?; }
                Ok(())
            }
        }
        impl<'a> Wire<'a> for $name<'a> {
            const KIND: Kind = Kind::Struct;
            fn read(item: Item<'a>) -> Result<Self, Error> {
                Self::from_fields(item.fields().ok_or(Error::WrongType { tag: None })?)
            }
            fn write(&self, tag: [u8; 3], w: &mut Encoder) -> Result<(), nfs_heat2::Error> {
                w.structure(tag, |w| self.write_fields(w))
            }
            fn validate(&self, budget: &mut crate::Budget) -> Result<(), Error> {
                budget.take(1)?;
                self.validate_fields(budget)
            }
            fn unknown_count(&self) -> usize { self.unknown_field_count() }
        }
    }
}
pub(crate) use schema;

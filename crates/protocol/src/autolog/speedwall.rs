// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! SpeedWallBriefInfo members and nested row graph.
//! Optional/unknown fields and unknown enum values are retained. No ranking policy.
use crate::{Error, Wire, check_string, schema, unique_keys};
use nfs_heat2::{Encoder, Field, Fields, Item, Kind, Limits, Value};
use std::{collections::BTreeSet, fmt};

pub const GET_SPEED_WALL_BRIEF_INFO: u16 = 79;

/// Preserve binary32 bits, including NaNs, without arithmetic normalization.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct RatingBits(pub u32);
impl fmt::Debug for RatingBits {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RatingBits(<redacted>)")
    }
}
impl<'a> Wire<'a> for RatingBits {
    const KIND: Kind = Kind::FloatBits;
    fn read(item: Item<'a>) -> Result<Self, Error> {
        match item.value() {
            Value::FloatBits(bits) => Ok(Self(bits)),
            _ => Err(Error::WrongType { tag: None }),
        }
    }
    fn write(&self, tag: [u8; 3], w: &mut Encoder) -> Result<(), nfs_heat2::Error> {
        w.float_bits(tag, self.0)
    }
}

#[derive(Default)]
pub struct RowIntegers<'a>(pub Vec<(&'a [u8], i32)>);
impl fmt::Debug for RowIntegers<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("RowIntegersLength")
            .field(&self.0.len())
            .finish()
    }
}
impl<'a> Wire<'a> for RowIntegers<'a> {
    const KIND: Kind = Kind::Map;
    fn read(item: Item<'a>) -> Result<Self, Error> {
        if !matches!(
            item.value(),
            Value::Map {
                key: Kind::String,
                value: Kind::Integer,
                ..
            }
        ) {
            return Err(Error::WrongType { tag: None });
        }
        let mut es = item.elements().ok_or(Error::WrongType { tag: None })?;
        let mut entries = Vec::new();
        while let Some(k) = es.next() {
            entries.push((
                <&[u8]>::read(k?)?,
                i32::read(es.next().ok_or(Error::WrongType { tag: None })??)?,
            ));
        }
        unique_keys(entries.iter().map(|(k, _)| *k))?;
        Ok(Self(entries))
    }
    fn write(&self, tag: [u8; 3], w: &mut Encoder) -> Result<(), nfs_heat2::Error> {
        w.string_integer_map(tag, self.0.iter().map(|(k, v)| (*k, i64::from(*v))))
    }
    fn validate(&self, b: &mut crate::Budget) -> Result<(), Error> {
        b.take(1)?;
        b.collection(self.0.len(), 2)?;
        for (k, _) in &self.0 {
            check_string(k, b.limits)?;
        }
        unique_keys(self.0.iter().map(|(k, _)| *k))
    }
}

#[derive(Default)]
pub struct RowStrings<'a>(pub Vec<(&'a [u8], &'a [u8])>);
impl fmt::Debug for RowStrings<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("RowStringsLength")
            .field(&self.0.len())
            .finish()
    }
}
impl<'a> Wire<'a> for RowStrings<'a> {
    const KIND: Kind = Kind::Map;
    fn read(item: Item<'a>) -> Result<Self, Error> {
        if !matches!(
            item.value(),
            Value::Map {
                key: Kind::String,
                value: Kind::String,
                ..
            }
        ) {
            return Err(Error::WrongType { tag: None });
        }
        let mut es = item.elements().ok_or(Error::WrongType { tag: None })?;
        let mut entries = Vec::new();
        while let Some(k) = es.next() {
            entries.push((
                <&[u8]>::read(k?)?,
                <&[u8]>::read(es.next().ok_or(Error::WrongType { tag: None })??)?,
            ));
        }
        unique_keys(entries.iter().map(|(k, _)| *k))?;
        Ok(Self(entries))
    }
    fn write(&self, tag: [u8; 3], w: &mut Encoder) -> Result<(), nfs_heat2::Error> {
        w.string_map(tag, &self.0)
    }
    fn validate(&self, b: &mut crate::Budget) -> Result<(), Error> {
        b.take(1)?;
        b.collection(self.0.len(), 2)?;
        for (k, v) in &self.0 {
            check_string(k, b.limits)?;
            check_string(v, b.limits)?;
        }
        unique_keys(self.0.iter().map(|(k, _)| *k))
    }
}

schema!(BlazeUser {
    blaze_id: i64 => [0x8a,0xca,0x73],
    persona_name: &'a [u8] => [0xc2,0x5b,0xa1],
    relation_type: i32 => [0xd7,0x2d,0x39],
});
schema!(InGameSpeedWallResponseRow {
    blaze_user: BlazeUser<'a> => [0x8a,0xcd,0x73],
    stats_flt: super::SettingsFloatBits<'a> => [0xcf,0x48,0x66],
    stats_int: RowIntegers<'a> => [0xcf,0x48,0x69],
    stats_str: RowStrings<'a> => [0xcf,0x48,0x73],
});
schema!(SpeedWallBriefInfoRequest {
    blaze_id: i64 => [0x8a,0xca,0x64],
    speed_wall_id: u32 => [0xcf,0x7a,0x64],
    speed_wall_stat_type: i32 => [0xcf,0x7c,0xf4],
});
schema!(SpeedWallBriefInfoResponse {
    rank: i32 => [0xca,0x1b,0xab],
    rating: RatingBits => [0xca,0x1d,0x00],
    speed_wall: InGameSpeedWallResponseRow<'a> => [0xca,0xfd,0xc0],
    speed_wall_id: u32 => [0xcf,0x7a,0x64],
    speed_wall_stat_type: i32 => [0xcf,0x7c,0xf4],
    total_count: i32 => [0xd2,0x3b,0xb4],
});

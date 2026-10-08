// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Stats keyscope configuration subset, independent of catalog policy.
//! Integer pairs retain wire order. Their interval semantics remain inferred.
use crate::{Error, Wire, schema};
use nfs_heat2::{Encoder, Field, Fields, Item, Kind, Limits, Value};
use std::{collections::BTreeSet, fmt};

mod async_group;
pub use async_group::{
    EntityIds, EntityStats, EntityStatsList, GET_STATS_ASYNC_NOTIFICATION,
    GET_STATS_BY_GROUP_ASYNC, GetStatsByGroupRequest, KeyScopedStatValues, StatStrings, StatValues,
};
mod stat_group;
pub use stat_group::{
    GET_STAT_GROUP, GetStatGroupRequest, KeyScopeNameValueMap, ObjectType, StatDescSummary,
    StatDescSummaryList, StatGroupResponse,
};

pub const COMPONENT: u16 = 7;
pub const GET_KEY_SCOPES_MAP: u16 = 15;

/// Only the observed empty request is supported. No extra fields are acknowledged.
#[derive(Debug, Default)]
pub struct GetKeyScopesMapRequest;

impl GetKeyScopesMapRequest {
    pub fn decode(bytes: &[u8], limits: Limits) -> Result<Self, Error> {
        let document = nfs_heat2::decode(bytes, limits)?;
        if document.fields().next().is_some() {
            return Err(Error::WrongType { tag: None });
        }
        Ok(Self)
    }

    pub fn encode(&self, limits: Limits) -> Result<Vec<u8>, Error> {
        Ok(Encoder::new(limits).finish()?)
    }
}

/// Native map<int64_t,int64_t>, including negative and full-width values.
#[derive(Default)]
pub struct KeyScopeValues(pub Vec<(i64, i64)>);

impl fmt::Debug for KeyScopeValues {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("KeyScopeValuesLength")
            .field(&self.0.len())
            .finish()
    }
}

fn unique_integers(entries: &[(i64, i64)]) -> Result<(), Error> {
    let mut seen = BTreeSet::new();
    for (key, _) in entries {
        if !seen.insert(*key) {
            return Err(Error::DuplicateMapKey);
        }
    }
    Ok(())
}

impl<'a> Wire<'a> for KeyScopeValues {
    const KIND: Kind = Kind::Map;
    fn read(item: Item<'a>) -> Result<Self, Error> {
        if !matches!(
            item.value(),
            Value::Map {
                key: Kind::Integer,
                value: Kind::Integer,
                ..
            }
        ) {
            return Err(Error::WrongType { tag: None });
        }
        let mut elements = item.elements().ok_or(Error::WrongType { tag: None })?;
        let mut entries = Vec::new();
        while let Some(key) = elements.next() {
            let key = i64::read(key?)?;
            let value = i64::read(elements.next().ok_or(Error::WrongType { tag: None })??)?;
            entries.push((key, value));
        }
        unique_integers(&entries)?;
        Ok(Self(entries))
    }
    fn write(&self, tag: [u8; 3], writer: &mut Encoder) -> Result<(), nfs_heat2::Error> {
        writer.integer_map(tag, self.0.iter().copied())
    }
    fn validate(&self, budget: &mut crate::Budget) -> Result<(), Error> {
        budget.take(1)?;
        budget.collection(self.0.len(), 2)?;
        unique_integers(&self.0)
    }
}

schema!(KeyScopeItem {
    aggregate_key_value: i64 => [0x86, 0x7a, 0xf9],
    enable_aggregation: bool => [0x96, 0xe8, 0x67],
    key_scope_values: KeyScopeValues => [0xaf, 0x3d, 0xac],
});

#[derive(Default)]
pub struct KeyScopesMap<'a>(pub Vec<(&'a [u8], KeyScopeItem<'a>)>);

impl fmt::Debug for KeyScopesMap<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("KeyScopesMapLength")
            .field(&self.0.len())
            .finish()
    }
}

impl<'a> Wire<'a> for KeyScopesMap<'a> {
    const KIND: Kind = Kind::Map;
    fn read(item: Item<'a>) -> Result<Self, Error> {
        if !matches!(
            item.value(),
            Value::Map {
                key: Kind::String,
                value: Kind::Struct,
                ..
            }
        ) {
            return Err(Error::WrongType { tag: None });
        }
        let mut elements = item.elements().ok_or(Error::WrongType { tag: None })?;
        let mut entries = Vec::new();
        while let Some(key) = elements.next() {
            let key = <&[u8]>::read(key?)?;
            let value =
                KeyScopeItem::read(elements.next().ok_or(Error::WrongType { tag: None })??)?;
            entries.push((key, value));
        }
        crate::unique_keys(entries.iter().map(|(key, _)| *key))?;
        Ok(Self(entries))
    }
    fn write(&self, tag: [u8; 3], writer: &mut Encoder) -> Result<(), nfs_heat2::Error> {
        writer.string_struct_map(tag, &self.0, |writer, value| value.write_fields(writer))
    }
    fn validate(&self, budget: &mut crate::Budget) -> Result<(), Error> {
        budget.take(1)?;
        // Count string keys here; each item counts its struct and nested fields.
        budget.collection(self.0.len(), 1)?;
        for (key, value) in &self.0 {
            crate::check_string(key, budget.limits)?;
            value.validate(budget)?;
        }
        crate::unique_keys(self.0.iter().map(|(key, _)| *key))
    }
    fn unknown_count(&self) -> usize {
        self.0
            .iter()
            .map(|(_, value)| value.unknown_field_count())
            .sum()
    }
}

schema!(KeyScopes { key_scopes_map: KeyScopesMap<'a> => [0xaf, 0x3a, 0x74] });

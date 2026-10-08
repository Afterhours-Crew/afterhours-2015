//! Stats group-definition messages, independent of catalog policy.
//! These definitions contain default strings, not a player's current stat values.
use crate::{Error, Wire, schema};
use nfs_heat2::{Encoder, Field, Fields, Item, Kind, Limits, Value};
use std::{collections::BTreeSet, fmt};

pub const GET_STAT_GROUP: u16 = 4;

/// Native ObjectType: two zero-extended 16-bit members in wire order.

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ObjectType(pub u16, pub u16);

impl fmt::Debug for ObjectType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ObjectType(<redacted>)")
    }
}

impl<'a> Wire<'a> for ObjectType {
    const KIND: Kind = Kind::IntegerPair;
    fn read(item: Item<'a>) -> Result<Self, Error> {
        let Value::IntegerPair([first, second]) = item.value() else {
            return Err(Error::WrongType { tag: None });
        };
        Ok(Self(
            u16::try_from(first).map_err(|_| Error::InvalidInteger { tag: None })?,
            u16::try_from(second).map_err(|_| Error::InvalidInteger { tag: None })?,
        ))
    }
    fn write(&self, tag: [u8; 3], writer: &mut Encoder) -> Result<(), nfs_heat2::Error> {
        writer.integer_pair(tag, [i64::from(self.0), i64::from(self.1)])
    }
}

schema!(GetStatGroupRequest { name: &'a [u8] => [0xba, 0x1b, 0x65] });

/// Native string-to-int64 map. Wire order and signed sentinel values are retained.
#[derive(Default)]
pub struct KeyScopeNameValueMap<'a>(pub Vec<(&'a [u8], i64)>);

impl fmt::Debug for KeyScopeNameValueMap<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("KeyScopeNameValueMapLength")
            .field(&self.0.len())
            .finish()
    }
}

impl<'a> Wire<'a> for KeyScopeNameValueMap<'a> {
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
        let mut elements = item.elements().ok_or(Error::WrongType { tag: None })?;
        let mut entries = Vec::new();
        while let Some(key) = elements.next() {
            let key = <&[u8]>::read(key?)?;
            let value = i64::read(elements.next().ok_or(Error::WrongType { tag: None })??)?;
            entries.push((key, value));
        }
        crate::unique_keys(entries.iter().map(|(key, _)| *key))?;
        Ok(Self(entries))
    }
    fn write(&self, tag: [u8; 3], writer: &mut Encoder) -> Result<(), nfs_heat2::Error> {
        writer.string_integer_map(tag, self.0.iter().copied())
    }
    fn validate(&self, budget: &mut crate::Budget) -> Result<(), Error> {
        budget.take(1)?;
        budget.collection(self.0.len(), 2)?;
        for (key, _) in &self.0 {
            crate::check_string(key, budget.limits)?;
        }
        crate::unique_keys(self.0.iter().map(|(key, _)| *key))
    }
}

schema!(StatDescSummary {
    category: &'a [u8] => [0x8e, 0x1d, 0x27],
    default_value: &'a [u8] => [0x92, 0x6b, 0x34],
    derived: bool => [0x93, 0x2d, 0xa4],
    format: &'a [u8] => [0x9b, 0x2b, 0x74],
    kind: &'a [u8] => [0xae, 0x9b, 0xa4],
    long_desc: &'a [u8] => [0xb2, 0x4c, 0xe3],
    metadata: &'a [u8] => [0xb6, 0x5d, 0x21],
    name: &'a [u8] => [0xba, 0x1b, 0x65],
    short_desc: &'a [u8] => [0xce, 0x4c, 0xe3],
    stat_type: i32 => [0xd3, 0x9c, 0x25],
});

#[derive(Default)]
pub struct StatDescSummaryList<'a>(pub Vec<StatDescSummary<'a>>);

impl fmt::Debug for StatDescSummaryList<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("StatDescSummaryListLength")
            .field(&self.0.len())
            .finish()
    }
}

impl<'a> Wire<'a> for StatDescSummaryList<'a> {
    const KIND: Kind = Kind::List;
    fn read(item: Item<'a>) -> Result<Self, Error> {
        if !matches!(
            item.value(),
            Value::List {
                element: Kind::Struct,
                ..
            }
        ) {
            return Err(Error::WrongType { tag: None });
        }
        let values = item
            .elements()
            .ok_or(Error::WrongType { tag: None })?
            .map(|item| StatDescSummary::read(item?))
            .collect::<Result<_, _>>()?;
        Ok(Self(values))
    }
    fn write(&self, tag: [u8; 3], writer: &mut Encoder) -> Result<(), nfs_heat2::Error> {
        writer.struct_list(tag, &self.0, |writer, value| value.write_fields(writer))
    }
    fn validate(&self, budget: &mut crate::Budget) -> Result<(), Error> {
        budget.take(1)?;
        budget.collection(self.0.len(), 0)?;
        for value in &self.0 {
            value.validate(budget)?;
        }
        Ok(())
    }
    fn unknown_count(&self) -> usize {
        self.0
            .iter()
            .map(StatDescSummary::unknown_field_count)
            .sum()
    }
}

schema!(StatGroupResponse {
    category_name: &'a [u8] => [0x8e, 0xe8, 0x6d],
    desc: &'a [u8] => [0x92, 0x5c, 0xe3],
    entity_type: ObjectType => [0x97, 0x4e, 0x70],
    key_scope_name_value_map: KeyScopeNameValueMap<'a> => [0xaf, 0x3d, 0x6d],
    metadata: &'a [u8] => [0xb6, 0x5d, 0x21],
    name: &'a [u8] => [0xba, 0x1b, 0x65],
    stat_descs: StatDescSummaryList<'a> => [0xcf, 0x48, 0x74],
});

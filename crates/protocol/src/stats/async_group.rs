//! native asynchronous Stats query and result payloads.
//! RPC acknowledgement and final notification are separate protocol events.
//! Current values are account state, never inferred from definition defaults.
use super::{KeyScopeNameValueMap, ObjectType};
use crate::{Error, Wire, schema};
use nfs_heat2::{Encoder, Field, Fields, Item, Kind, Limits, Value};
use std::{collections::BTreeSet, fmt};

pub const GET_STATS_BY_GROUP_ASYNC: u16 = 16;
pub const GET_STATS_ASYNC_NOTIFICATION: u16 = 50;
#[derive(Default)]
pub struct EntityIds(pub Vec<i64>);
impl fmt::Debug for EntityIds {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("EntityIdsLength")
            .field(&self.0.len())
            .finish()
    }
}
impl<'a> Wire<'a> for EntityIds {
    const KIND: Kind = Kind::List;
    fn read(item: Item<'a>) -> Result<Self, Error> {
        if !matches!(
            item.value(),
            Value::List {
                element: Kind::Integer,
                ..
            }
        ) {
            return Err(Error::WrongType { tag: None });
        }
        Ok(Self(
            item.elements()
                .ok_or(Error::WrongType { tag: None })?
                .map(|i| i64::read(i?))
                .collect::<Result<_, _>>()?,
        ))
    }
    fn write(&self, tag: [u8; 3], w: &mut Encoder) -> Result<(), nfs_heat2::Error> {
        w.integer_list(tag, self.0.iter().copied())
    }
    fn validate(&self, budget: &mut crate::Budget) -> Result<(), Error> {
        budget.take(1)?;
        budget.collection(self.0.len(), 1)
    }
}

#[derive(Default)]
pub struct StatStrings<'a>(pub Vec<&'a [u8]>);
impl fmt::Debug for StatStrings<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("StatStringsLength")
            .field(&self.0.len())
            .finish()
    }
}
impl<'a> Wire<'a> for StatStrings<'a> {
    const KIND: Kind = Kind::List;
    fn read(item: Item<'a>) -> Result<Self, Error> {
        if !matches!(
            item.value(),
            Value::List {
                element: Kind::String,
                ..
            }
        ) {
            return Err(Error::WrongType { tag: None });
        }
        Ok(Self(
            item.elements()
                .ok_or(Error::WrongType { tag: None })?
                .map(|i| <&[u8]>::read(i?))
                .collect::<Result<_, _>>()?,
        ))
    }
    fn write(&self, tag: [u8; 3], w: &mut Encoder) -> Result<(), nfs_heat2::Error> {
        w.string_list(tag, self.0.iter().copied())
    }
    fn validate(&self, budget: &mut crate::Budget) -> Result<(), Error> {
        budget.take(1)?;
        budget.collection(self.0.len(), 1)?;
        for value in &self.0 {
            crate::check_string(value, budget.limits)?;
        }
        Ok(())
    }
}

schema!(GetStatsByGroupRequest {
    entity_ids: EntityIds => [0x96,0x99,0x00],
    key_scope_name_value_map: KeyScopeNameValueMap<'a> => [0xaf,0x3d,0x6d],
    group_name: &'a [u8] => [0xba,0x1b,0x65],
    period_ctr: i32 => [0xc2,0x3d,0x32],
    period_offset: i32 => [0xc2,0xf9,0xa6],
    period_id: i32 => [0xc3,0x2a,0x64],
    period_type: i32 => [0xc3,0x4e,0x70],
    time: i32 => [0xd2,0x9b,0x65],
    view_id: u32 => [0xda,0x99,0x00],
});

schema!(EntityStats {
    entity_id: i64 => [0x96,0x99,0x00],
    entity_type: ObjectType => [0x97,0x4e,0x70],
    period_offset: i32 => [0xc2,0xf9,0xa6],
    stat_values: StatStrings<'a> => [0xcf,0x48,0x74],
});
#[derive(Default)]
pub struct EntityStatsList<'a>(pub Vec<EntityStats<'a>>);

impl fmt::Debug for EntityStatsList<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("EntityStatsListLength")
            .field(&self.0.len())
            .finish()
    }
}

impl<'a> Wire<'a> for EntityStatsList<'a> {
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
            .map(|item| EntityStats::read(item?))
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
        self.0.iter().map(EntityStats::unknown_field_count).sum()
    }
}

schema!(StatValues { entity_stats: EntityStatsList<'a> => [0xcf,0x48,0x74] });

schema!(KeyScopedStatValues {
    group_name: &'a [u8] => [0x9f,0x2b,0xad],
    key_string: &'a [u8] => [0xae,0x5e,0x40],
    last: bool => [0xb2,0x1c,0xf4],
    stat_values: StatValues<'a> => [0xcf,0x4c,0xc0],
    view_id: u32 => [0xda,0x99,0x00],
});

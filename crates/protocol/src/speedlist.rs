//! native startup catalog query. This is not SpeedList matchmaking support.
//! The request's BLIS tag means blazeId; LSPL is an optional list of type IDs.
use crate::{Error, Wire, schema};
use nfs_heat2::{Encoder, Field, Fields, Item, Kind, Limits, Value};
use std::{collections::BTreeSet, fmt};

pub const COMPONENT: u16 = 2055;
pub const GET_SPEED_LIST_TYPE: u16 = 1;

#[derive(Default)]
pub struct TypeIds(pub Vec<u32>);
impl fmt::Debug for TypeIds {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("TypeIdsLength").field(&self.0.len()).finish()
    }
}
impl<'a> Wire<'a> for TypeIds {
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
                .map(|i| u32::read(i?))
                .collect::<Result<_, _>>()?,
        ))
    }
    fn write(&self, tag: [u8; 3], w: &mut Encoder) -> Result<(), nfs_heat2::Error> {
        w.integer_list(tag, self.0.iter().map(|v| i64::from(*v)))
    }
    fn validate(&self, budget: &mut crate::Budget) -> Result<(), Error> {
        budget.take(1)?;
        budget.collection(self.0.len(), 1)
    }
}
schema!(SpeedListTypeRequest {
    blaze_id: i64 => [0x8a, 0xca, 0x73],
    list_of_speed_list_types_requested: TypeIds => [0xb3, 0x3c, 0x2c],
});
schema!(SpeedListTypeInstance {
    type_description_string_id: &'a [u8] => [0x92, 0xea, 0x64],
    type_localised_name_string_id: &'a [u8] => [0xb2, 0xf8, 0xee],
    speed_list_type_id: u32 => [0xce, 0xca, 0x64],
    type_texture_id: &'a [u8] => [0xd2, 0x5a, 0x64],
});

#[derive(Default)]
pub struct SpeedListTypes<'a>(pub Vec<(u32, SpeedListTypeInstance<'a>)>);
impl fmt::Debug for SpeedListTypes<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("SpeedListTypesLength")
            .field(&self.0.len())
            .finish()
    }
}
impl SpeedListTypes<'_> {
    fn unique(&self) -> Result<(), Error> {
        let mut seen = BTreeSet::new();
        for (key, _) in &self.0 {
            if !seen.insert(*key) {
                return Err(Error::DuplicateMapKey);
            }
        }
        Ok(())
    }
}
impl<'a> Wire<'a> for SpeedListTypes<'a> {
    const KIND: Kind = Kind::Map;
    fn read(item: Item<'a>) -> Result<Self, Error> {
        if !matches!(
            item.value(),
            Value::Map {
                key: Kind::Integer,
                value: Kind::Struct,
                ..
            }
        ) {
            return Err(Error::WrongType { tag: None });
        }
        let mut elements = item.elements().ok_or(Error::WrongType { tag: None })?;
        let mut entries = Vec::new();
        while let Some(key) = elements.next() {
            entries.push((
                u32::read(key?)?,
                SpeedListTypeInstance::read(
                    elements.next().ok_or(Error::WrongType { tag: None })??,
                )?,
            ));
        }
        let map = Self(entries);
        map.unique()?;
        Ok(map)
    }
    fn write(&self, tag: [u8; 3], w: &mut Encoder) -> Result<(), nfs_heat2::Error> {
        w.integer_struct_map(tag, &self.0, |w, item| item.write_fields(w))
    }
    fn validate(&self, budget: &mut crate::Budget) -> Result<(), Error> {
        budget.take(1)?;
        budget.collection(self.0.len(), 1)?;
        self.unique()?;
        for (_, value) in &self.0 {
            value.validate(budget)?;
        }
        Ok(())
    }
    fn unknown_count(&self) -> usize {
        self.0.iter().map(|(_, v)| v.unknown_field_count()).sum()
    }
}
schema!(SpeedListTypeResponse {
    blaze_id: i64 => [0x8a, 0xca, 0x64],
    speed_list_types: SpeedListTypes<'a> => [0xcf, 0x0b, 0x33],
});

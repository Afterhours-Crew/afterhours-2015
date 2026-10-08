// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Initial UserAdded subset.
//! Selected addresses and present dynamic client data are explicitly unsupported.
use super::{ObjectId, UserIdentification};
use crate::{Error, Wire, schema};
use nfs_heat2::{Encoder, Field, Fields, Item, Kind, Limits, Value};
use std::{collections::BTreeSet, fmt};

/// Explicitly unset ADDR (selector 127), distinct from an absent field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnsetNetworkAddress;

impl<'a> Wire<'a> for UnsetNetworkAddress {
    const KIND: Kind = Kind::Union;
    fn read(item: Item<'a>) -> Result<Self, Error> {
        if matches!(item.value(), Value::Union { discriminant: 127 }) {
            Ok(Self)
        } else {
            Err(Error::WrongType { tag: None })
        }
    }
    fn write(&self, tag: [u8; 3], w: &mut Encoder) -> Result<(), nfs_heat2::Error> {
        w.unset_union(tag)
    }
}

/// CVAR with its presence byte zero, distinct from the CVAR field being absent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AbsentClientData;

impl<'a> Wire<'a> for AbsentClientData {
    const KIND: Kind = Kind::Variable;
    fn read(item: Item<'a>) -> Result<Self, Error> {
        if matches!(item.value(), Value::Variable { type_id: None }) {
            Ok(Self)
        } else {
            Err(Error::WrongType { tag: None })
        }
    }
    fn write(&self, tag: [u8; 3], w: &mut Encoder) -> Result<(), nfs_heat2::Error> {
        w.absent_variable(tag)
    }
}

/// DMAP preserves entry order and rejects duplicate u32 keys.
#[derive(Default)]
pub struct ExtendedDataMap(pub Vec<(u32, i64)>);

impl fmt::Debug for ExtendedDataMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ExtendedDataMapLength")
            .field(&self.0.len())
            .finish()
    }
}

fn unique_numeric_keys(entries: &[(u32, i64)]) -> Result<(), Error> {
    let mut seen = BTreeSet::new();
    for (key, _) in entries {
        if !seen.insert(*key) {
            return Err(Error::DuplicateMapKey);
        }
    }
    Ok(())
}

impl<'a> Wire<'a> for ExtendedDataMap {
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
            let key = u32::read(key?)?;
            let value = i64::read(elements.next().ok_or(Error::WrongType { tag: None })??)?;
            entries.push((key, value));
        }
        unique_numeric_keys(&entries)?;
        Ok(Self(entries))
    }
    fn write(&self, tag: [u8; 3], w: &mut Encoder) -> Result<(), nfs_heat2::Error> {
        w.integer_map(
            tag,
            self.0.iter().map(|(key, value)| (i64::from(*key), *value)),
        )
    }
    fn validate(&self, budget: &mut crate::Budget) -> Result<(), Error> {
        budget.take(1)?;
        budget.collection(self.0.len(), 2)?;
        unique_numeric_keys(&self.0)
    }
}

#[derive(Default)]
pub struct LatencyList(pub Vec<i32>);

impl fmt::Debug for LatencyList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("LatencyListLength")
            .field(&self.0.len())
            .finish()
    }
}

impl<'a> Wire<'a> for LatencyList {
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
        let values = item
            .elements()
            .ok_or(Error::WrongType { tag: None })?
            .map(|item| i32::read(item?))
            .collect::<Result<_, _>>()?;
        Ok(Self(values))
    }
    fn write(&self, tag: [u8; 3], w: &mut Encoder) -> Result<(), nfs_heat2::Error> {
        w.integer_list(tag, self.0.iter().map(|v| i64::from(*v)))
    }
    fn validate(&self, budget: &mut crate::Budget) -> Result<(), Error> {
        budget.take(1)?;
        budget.collection(self.0.len(), 1)
    }
}

#[derive(Default)]
pub struct ObjectIdList(pub Vec<ObjectId>);

impl fmt::Debug for ObjectIdList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ObjectIdListLength")
            .field(&self.0.len())
            .finish()
    }
}

impl<'a> Wire<'a> for ObjectIdList {
    const KIND: Kind = Kind::List;
    fn read(item: Item<'a>) -> Result<Self, Error> {
        if !matches!(
            item.value(),
            Value::List {
                element: Kind::IntegerTriple,
                ..
            }
        ) {
            return Err(Error::WrongType { tag: None });
        }
        let values = item
            .elements()
            .ok_or(Error::WrongType { tag: None })?
            .map(|item| ObjectId::read(item?))
            .collect::<Result<_, _>>()?;
        Ok(Self(values))
    }
    fn write(&self, tag: [u8; 3], w: &mut Encoder) -> Result<(), nfs_heat2::Error> {
        w.integer_triple_list(
            tag,
            self.0.iter().map(|v| [i64::from(v.0), i64::from(v.1), v.2]),
        )
    }
    fn validate(&self, budget: &mut crate::Budget) -> Result<(), Error> {
        budget.take(1)?;
        budget.collection(self.0.len(), 1)
    }
}

// Blaze::Util::NetworkQosData. NAT enum variants are not assigned here.
schema!(NetworkQosData {
    bandwidth_error_code: u32 => [0x8b, 0x7a, 0x32],
    downstream_bits_per_second: u32 => [0x92, 0x2c, 0x33],
    nat_error_code: u32 => [0xba, 0x1a, 0x32],
    nat_type: i64 => [0xba, 0x1d, 0x34],
    upstream_bits_per_second: u32 => [0xd6, 0x2c, 0x33],
});

// Only the explicitly initial ADDR/CVAR forms are supported. Other fields retain
// widths. HWFG is a u32 bitfield; unknown bits stay.
schema!(UserSessionExtendedDataInitial {
    address: UnsetNetworkAddress => [0x86, 0x49, 0x32],
    best_ping_site_alias: &'a [u8] => [0x8b, 0x0c, 0xc0],
    country: &'a [u8] => [0x8f, 0x4e, 0x40],
    client_data: AbsentClientData => [0x8f, 0x68, 0x72],
    data_map: ExtendedDataMap => [0x92, 0xd8, 0x70],
    hardware_flags: u32 => [0xa3, 0x79, 0xa7],
    isp: &'a [u8] => [0xa7, 0x3c, 0x00],
    latency_list: LatencyList => [0xc3, 0x3b, 0x2d],
    qos_data: NetworkQosData<'a> => [0xc6, 0x48, 0x74],
    time_zone: &'a [u8] => [0xd3, 0xa0, 0x00],
    user_info_attribute: u64 => [0xd6, 0x1d, 0x34],
    blaze_object_id_list: ObjectIdList => [0xd6, 0xcc, 0xf4],
});

// Blaze::NotifyUserAdded is DATA/USER, not UserData's EDAT/FLGS/USER.
schema!(NotifyUserAddedInitial {
    extended_data: UserSessionExtendedDataInitial<'a> => [0x92, 0x1d, 0x21],
    user_info: UserIdentification<'a> => [0xd7, 0x39, 0x72],
});

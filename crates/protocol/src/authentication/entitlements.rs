// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Authentication entitlement schemas, without authorization or catalog policy.
//! Class-5 enum values use the native signed-dword writer; unknown i32 values
//! remain intact. Entitlement IDs use bit-preserving uint64 encoding.
use crate::{Error, Wire, schema};
use nfs_heat2::{Encoder, Field, Fields, Item, Kind, Limits, Value};
use std::{collections::BTreeSet, fmt};

pub const COMPONENT: u16 = 1;
pub const LIST_USER_ENTITLEMENTS2: u16 = 29;

schema!(Entitlement {
    device_uri: &'a [u8] => [0x92, 0x5d, 0xa9],
    grant_date: &'a [u8] => [0x9e, 0x48, 0x79],
    group_name: &'a [u8] => [0x9e, 0xe8, 0x6d],
    id: u64 => [0xa6, 0x40, 0x00],
    is_consumable: bool => [0xa7, 0x38, 0xef],
    persona_id: i64 => [0xc2, 0x99, 0x00],
    project_id: &'a [u8] => [0xc2, 0xaa, 0x64],
    product_catalog: i32 => [0xc3, 0x28, 0xe1],
    product_id: &'a [u8] => [0xc3, 0x2a, 0x64],
    status: i32 => [0xcf, 0x48, 0x74],
    status_reason_code: i32 => [0xcf, 0x4c, 0xa3],
    entitlement_tag: &'a [u8] => [0xd2, 0x19, 0xc0],
    termination_date: &'a [u8] => [0xd2, 0x48, 0x79],
    entitlement_type: i32 => [0xd3, 0x9c, 0x25],
    use_count: u32 => [0xd6, 0x3b, 0xb4],
    version: u32 => [0xda, 0x5c, 0x80],
});

#[derive(Default)]
pub struct EntitlementList<'a>(pub Vec<Entitlement<'a>>);

impl fmt::Debug for EntitlementList<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("EntitlementListLength")
            .field(&self.0.len())
            .finish()
    }
}

impl<'a> Wire<'a> for EntitlementList<'a> {
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
        Ok(Self(
            item.elements()
                .ok_or(Error::WrongType { tag: None })?
                .map(|item| Entitlement::read(item?))
                .collect::<Result<_, _>>()?,
        ))
    }

    fn write(&self, tag: [u8; 3], w: &mut Encoder) -> Result<(), nfs_heat2::Error> {
        w.struct_list(tag, &self.0, |w, value| value.write_fields(w))
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
        self.0.iter().map(Entitlement::unknown_field_count).sum()
    }
}

schema!(Entitlements {
    entitlements: EntitlementList<'a> => [0xba, 0xcc, 0xf4],
});

/// Ordered strings, with absence distinct from an empty list. Duplicates and
/// arbitrary byte strings are preserved; selector policy belongs to the caller.
#[derive(Default)]
pub struct GroupNameList<'a>(pub Vec<&'a [u8]>);

impl fmt::Debug for GroupNameList<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("GroupNameListLength")
            .field(&self.0.len())
            .finish()
    }
}

impl<'a> Wire<'a> for GroupNameList<'a> {
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
                .map(|item| <&[u8]>::read(item?))
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

schema!(ListUserEntitlements2Request {
    user_id: i64 => [0x8b, 0x5a, 0x64],
    end_grant_date: &'a [u8] => [0x96, 0x79, 0x21],
    page_no: u16 => [0x97, 0x0c, 0xee],
    page_size: u16 => [0x97, 0x0c, 0xfa],
    entitlement_tag: &'a [u8] => [0x97, 0x48, 0x67],
    end_termination_date: &'a [u8] => [0x97, 0x49, 0x21],
    group_name_list: GroupNameList<'a> => [0x9e, 0xeb, 0x33],
    has_authorized_persona: bool => [0xa2, 0x1d, 0x70],
    project_id: &'a [u8] => [0xc2, 0xaa, 0x64],
    product_id: &'a [u8] => [0xc3, 0x2a, 0x64],
    recursive_search: bool => [0xca, 0x58, 0xf5],
    start_grant_date: &'a [u8] => [0xce, 0x79, 0x21],
    status: i32 => [0xcf, 0x48, 0x74],
    start_termination_date: &'a [u8] => [0xcf, 0x49, 0x21],
    entitlement_type: i32 => [0xd3, 0x9c, 0x25],
});

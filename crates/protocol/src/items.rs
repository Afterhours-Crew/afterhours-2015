// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! ItemsSystem ensurePlayerInventory request. No inventory mutation or
//! response policy is implemented: saved empty replies omit native AWIT, which
//! can carry awarded items. Omission does not establish initialization semantics.
use crate::{Error, Wire, schema};
use nfs_heat2::{Encoder, Field, Fields, Item, Kind, Limits, Value};
use std::{collections::BTreeSet, fmt};

pub const COMPONENT: u16 = 2052;
pub const ENSURE_PLAYER_INVENTORY: u16 = 19;
schema!(LicenseSourceData {
    license: &'a [u8] => [0xb2,0x98,0xc0],
    source: &'a [u8] => [0xcf,0x28,0xc0],
});
#[derive(Default)]
pub struct LicenseSources<'a>(pub Vec<LicenseSourceData<'a>>);
impl fmt::Debug for LicenseSources<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("LicenseSourcesLength")
            .field(&self.0.len())
            .finish()
    }
}
impl<'a> Wire<'a> for LicenseSources<'a> {
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
                .map(|v| LicenseSourceData::read(v?))
                .collect::<Result<_, _>>()?,
        ))
    }
    fn write(&self, tag: [u8; 3], w: &mut Encoder) -> Result<(), nfs_heat2::Error> {
        w.struct_list(tag, &self.0, |w, v| v.write_fields(w))
    }
    fn validate(&self, b: &mut crate::Budget) -> Result<(), Error> {
        b.take(1)?;
        b.collection(self.0.len(), 0)?;
        for v in &self.0 {
            v.validate(b)?;
        }
        Ok(())
    }
    fn unknown_count(&self) -> usize {
        self.0
            .iter()
            .map(LicenseSourceData::unknown_field_count)
            .sum()
    }
}
schema!(EnsurePlayerInventoryRequest {
    item_system_name: &'a [u8] => [0xa7,0x3b,0xad],
    available_licenses: LicenseSources<'a> => [0xb2,0x98,0xf3],
});

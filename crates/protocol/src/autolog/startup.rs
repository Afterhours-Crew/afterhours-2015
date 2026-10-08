// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! native Autolog startup models. Population and history policy are separate.
use crate::{Error, Wire, schema};
use nfs_heat2::{Encoder, Field, Fields, Item, Kind, Limits, Value};
use std::{collections::BTreeSet, fmt};

pub const GET_RECENT_PLAYERS: u16 = 70;
pub const GET_KILL_SWITCHES: u16 = 75;
pub const GET_RANDOM_PLAYERS: u16 = 76;

#[derive(Default)]
pub struct PlayerIds(pub Vec<i64>);
impl fmt::Debug for PlayerIds {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("PlayerIdsLength")
            .field(&self.0.len())
            .finish()
    }
}
impl<'a> Wire<'a> for PlayerIds {
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
pub struct StringList<'a>(pub Vec<&'a [u8]>);
impl fmt::Debug for StringList<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("StringListLength")
            .field(&self.0.len())
            .finish()
    }
}
impl<'a> Wire<'a> for StringList<'a> {
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

schema!(KillSwitchRequest { blaze_id: i64 => [0x8a,0xca,0x64] });
schema!(KillSwitchResponse { kill_switch_list: StringList<'a> => [0xaf,0x3b,0x29] });
schema!(RandomPlayersRequest { max_players: u32 => [0xb7,0x0b,0x21] });
schema!(RandomPlayersResponse { blaze_ids: PlayerIds => [0x8a,0xca,0x64] });
schema!(GetRecentPlayersRequest {
    include_first_party_friends: bool => [0x9b,0x0d,0x39],
    max_players_to_return: i32 => [0xb6,0x1e,0x30],
    players: PlayerIds => [0xc2,0xcc,0xf4],
    sort_order: i32 => [0xd3,0x9c,0x25],
});

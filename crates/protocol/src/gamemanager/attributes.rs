//! ATTR/GID graph: string-to-string attributes and a uint64 game ID.
//! Codecs do not grant attribute-write authority.
use crate::{Error, Wire, schema, util::ConfigEntries};
use nfs_heat2::{Encoder, Field, Fields, Item, Kind, Limits};
use std::{collections::BTreeSet, fmt};

pub const SET_GAME_ATTRIBUTES: u16 = 7;
pub const NOTIFY_GAME_ATTRIB_CHANGE: u16 = 80;

schema!(SetGameAttributesRequest {
    game_attribs: ConfigEntries<'a> => [0x87, 0x4d, 0x32],
    game_id: u64 => [0x9e, 0x99, 0x00],
});
schema!(NotifyGameAttribChange {
    game_attribs: ConfigEntries<'a> => [0x87, 0x4d, 0x32],
    game_id: u64 => [0x9e, 0x99, 0x00],
});

//! UserSettingsSaveRequest: signed UID and opaque strings.
//! Account selection and persistence are service policy.
use crate::{Error, Wire, schema};
use nfs_heat2::{Encoder, Field, Fields, Item, Kind, Limits};
use std::{collections::BTreeSet, fmt};

pub const USER_SETTINGS_SAVE: u16 = 11;

schema!(UserSettingsSaveRequest {
    data: &'a [u8] => [0x92, 0x1d, 0x21],
    key: &'a [u8] => [0xae, 0x5e, 0x40],
    user_id: i64 => [0xd6, 0x99, 0x00],
});

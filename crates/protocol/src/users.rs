// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! UserSessions identity and authentication notification payloads.
//! Domain identity, authentication policy and notification scheduling stay separate.
use crate::{Blob, Error, Wire, schema};
use nfs_heat2::{Encoder, Field, Fields, Item, Kind, Limits, Value};
use std::{collections::BTreeSet, fmt};

mod user_data;
pub use user_data::UserData;

pub const COMPONENT: u16 = 30722;
pub const LOOKUP_USER: u16 = 12;
pub const USER_AUTHENTICATED: u16 = 8;
pub const USER_ADDED: u16 = 2;
pub const USER_UPDATED: u16 = 5;
/// Request category 0, distinct from USER_AUTHENTICATED category 2.
pub const UPDATE_HARDWARE_FLAGS: u16 = 8;
pub const UPDATE_NETWORK_INFO: u16 = 20;
/// Notification category 2; route binding confidence is documented in.
pub const USER_SESSION_EXTENDED_DATA_UPDATE: u16 = 1;

mod added;
pub use added::{
    AbsentClientData, ExtendedDataMap, LatencyList, NetworkQosData, NotifyUserAddedInitial,
    ObjectIdList, UnsetNetworkAddress, UserSessionExtendedDataInitial,
};

mod network;
pub use network::{
    IpAddress, IpPairAddress, NAT_INFO_ONLY, NETWORK_ADDRESS_ONLY, NetworkAddress, NetworkInfo,
    PingSiteLatencyMap, UPDATE_METRICS, UpdateHardwareFlagsRequest, UpdateNetworkInfoRequest,
    UserSessionExtendedDataNetwork, UserSessionExtendedDataUpdate, VOIP_HEADSET_STATUS,
};

/// named masks; other flag bits are retained rather than discarded.
pub const USER_STATUS_SUBSCRIBED: u32 = 1;
pub const USER_STATUS_ONLINE: u32 = 2;

// recovers Blaze::UserStatus metadata. Binding it to captured 30722/5
// remains code/shape inference; this codec does not implement notification policy.
schema!(UserStatus {
    status_flags: u32 => [0x9a, 0xc9, 0xf3],
    blaze_id: i64 => [0xa6, 0x40, 0x00],
});

/// ObjectId in wire order: two zero-extended 16-bit parts, then
/// the unchanged 64-bit integer pattern. Part meanings are intentionally unnamed.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ObjectId(pub u16, pub u16, pub i64);

impl fmt::Debug for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ObjectId(<redacted>)")
    }
}

impl<'a> Wire<'a> for ObjectId {
    const KIND: Kind = Kind::IntegerTriple;

    fn read(item: Item<'a>) -> Result<Self, Error> {
        let Value::IntegerTriple([first, second, value]) = item.value() else {
            return Err(Error::WrongType { tag: None });
        };
        Ok(Self(
            u16::try_from(first).map_err(|_| Error::InvalidInteger { tag: None })?,
            u16::try_from(second).map_err(|_| Error::InvalidInteger { tag: None })?,
            value,
        ))
    }

    fn write(&self, tag: [u8; 3], w: &mut Encoder) -> Result<(), nfs_heat2::Error> {
        w.integer_triple(tag, [i64::from(self.0), i64::from(self.1), self.2])
    }
}

// 16-member Blaze::UserSessionLoginInfo matches the saved
// UserAuthenticated bodies. It is distinct from Authentication::UserLoginInfo.
// PLAT/USTP retain raw integer enums; variants and requiredness are not inferred.
schema!(UserSessionLoginInfo {
    is_first_console_login: bool => [0x46, 0x3b, 0xee],
    account_locale: u32 => [0x86, 0xcb, 0xe3],
    blaze_user_id: i64 => [0x8b, 0x5a, 0x64],
    connection_group_object_id: ObjectId => [0x8e, 0x7a, 0x64],
    display_name: &'a [u8] => [0x93, 0x3b, 0xad],
    is_first_login: bool => [0x9b, 0x2c, 0xf4],
    session_key: &'a [u8] => [0xae, 0x5e, 0x40],
    last_authenticated: u32 => [0xb2, 0x1c, 0xf4],
    last_login_date_time: i64 => [0xb2, 0xcb, 0xe7],
    email: &'a [u8] => [0xb6, 0x1a, 0x6c],
    persona_namespace: &'a [u8] => [0xba, 0x1c, 0xf0],
    persona_id: i64 => [0xc2, 0x99, 0x00],
    client_platform: i64 => [0xc2, 0xc8, 0x74],
    user_id: i64 => [0xd6, 0x99, 0x00],
    user_session_type: i64 => [0xd7, 0x3d, 0x30],
    external_id: u64 => [0xe3, 0x29, 0x66],
});

schema!(UserIdentification {
    account_id: i64 => [0x86, 0x99, 0x00],
    account_locale: u32 => [0x86, 0xcb, 0xe3],
    external_blob: Blob<'a> => [0x97, 0x88, 0xa2],
    external_id: u64 => [0x97, 0x8a, 0x64],
    blaze_id: i64 => [0xa6, 0x40, 0x00],
    name: &'a [u8] => [0xba, 0x1b, 0x65],
    persona_namespace: &'a [u8] => [0xba, 0x1c, 0xf0],
    origin_persona_id: u64 => [0xbf, 0x2a, 0x67],
    pid_id: i64 => [0xc2, 0x99, 0x29],
});

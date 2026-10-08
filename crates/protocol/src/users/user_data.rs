// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! UserData wrapper; EDAT is the supported network-data subset.
//! UserDataFlags is a uint32 field. This wire model is independent of
//! lookup authorization, identity ownership and live status.
use super::{UserIdentification, UserSessionExtendedDataNetwork};
use crate::{Error, Wire, schema};
use nfs_heat2::{Encoder, Field, Fields, Item, Kind, Limits};
use std::{collections::BTreeSet, fmt};

schema!(UserData {
    extended_data: UserSessionExtendedDataNetwork<'a> => [0x96,0x48,0x74],
    status_flags: u32 => [0x9a,0xc9,0xf3],
    user_info: UserIdentification<'a> => [0xd7,0x39,0x72],
});

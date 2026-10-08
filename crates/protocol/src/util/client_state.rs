// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! native ClientState metadata: MODE and STAT are signed 32-bit enums.
//! Unknown enum values and absent fields remain representable. A menu-mode
//! report does not prove that the game has reached an interactive menu.
use crate::{Error, Wire, schema};
use nfs_heat2::{Encoder, Field, Fields, Item, Kind, Limits};
use std::{collections::BTreeSet, fmt};

schema!(ClientState {
    mode: i32 => [0xb6, 0xf9, 0x25],
    status: i32 => [0xcf, 0x48, 0x74],
});

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Matchmaking payloads. Wire forms only: no allocation, matching,
//! topology or readiness policy.
use super::{GameIds, PlayerIds};
use crate::{Error, Wire, schema, users::ObjectId};
use nfs_heat2::{Encoder, Field, Fields, Item, Kind, Limits};
use std::{collections::BTreeSet, fmt};

pub const START_MATCHMAKING: u16 = 13;
pub const NOTIFY_MATCHMAKING_SESSION_CONNECTION_VALIDATED: u16 = 11;
pub const NOTIFY_GAME_PLAYER_STATE_CHANGE: u16 = 116;
pub const NOTIFY_PLAYER_JOIN_COMPLETED: u16 = 30;

schema!(StartMatchmakingResponse {
    external_session_correlation_id: &'a [u8] => [0x8e,0xfa,0x64],
    external_session_name: &'a [u8] => [0x97,0x3b,0xad],
    session_id: u64 => [0xb7,0x3a,0x64],
    scid: &'a [u8] => [0xce,0x3a,0x64],
    external_session_template_name: &'a [u8] => [0xcf,0x4b,0x6e],
});
// schema, not a server error selection policy.
schema!(MatchmakingCriteriaError {
    err_message: &'a [u8] => [0xb7,0x39,0xc0],
});

schema!(MatchmakingSetupContext {
    fit_score: u32 => [0x9a,0x9d,0x00],
    game_entry_type: i32 => [0x9e,0x5b,0xb4],
    max_possible_fit_score: u32 => [0xb6,0x1e,0x26],
    scenario_id: u64 => [0xb7,0x38,0xe4],
    session_id: u64 => [0xb7,0x3a,0x64],
    matchmaking_result: i32 => [0xcb,0x3b,0x34],
    time_to_match: i64 => [0xd3,0x4b,0x40],
    user_session_id: u64 => [0xd7,0x3a,0x64],
});

schema!(ConnectionValidationResults {
    avoid_player_id_list: PlayerIds => [0x86,0xcc,0xf4],
    fail_count: u16 => [0x9a,0x3b,0xb4],
    avoid_game_id_list: GameIds => [0x9e,0x99,0x2c],
    network_topology: i32 => [0xbb,0x4b,0xf0],
    tier: u16 => [0xd2,0x99,0x72],
});

schema!(NotifyMatchmakingSessionConnectionValidated {
    connection_validated_results: ConnectionValidationResults<'a> => [0x8e,0xfb,0xb6],
    dispatch_session_finished: bool => [0x92,0x9c,0xf0],
    game_id: u64 => [0x9e,0x99,0x00],
    user_group_id: ObjectId => [0x9f,0x2a,0x64],
    scenario_id: u64 => [0xb7,0x38,0xe4],
    session_id: u64 => [0xb7,0x3a,0x64],
    qos_validation_performed: bool => [0xc7,0x3d,0xb2],
    user_session_id: u64 => [0xd7,0x3a,0x64],
});

schema!(NotifyGamePlayerStateChange {
    game_id: u64 => [0x9e,0x99,0x00],
    player_id: i64 => [0xc2,0x99,0x00],
    player_state: i32 => [0xcf,0x48,0x74],
});

schema!(NotifyPlayerJoinCompleted {
    game_id: u64 => [0x9e,0x99,0x00],
    player_id: i64 => [0xc2,0x99,0x00],
    joined_game_timestamp: i64 => [0xd2,0x9b,0x65],
});

//! Login schemas with bit-preserving uint64 fields.
//! Credentials and session data are bytes here; no authentication policy is implied.
pub mod entitlements;

use crate::{Blob, Error, Wire, schema};
use nfs_heat2::{Encoder, Field, Fields, Item, Kind, Limits};
use std::{collections::BTreeSet, fmt};

pub const COMPONENT: u16 = 1;
pub const LOGIN: u16 = 10;

schema!(LoginRequest {
    auth_code: &'a [u8] => [0x87, 0x5d, 0x28],
    external_blob: Blob<'a> => [0x97, 0x8d, 0x22],
    external_id: u64 => [0x97, 0x8d, 0x29],
});
schema!(PersonaDetails {
    display_name: &'a [u8] => [0x93, 0x3b, 0xad],
    last_authenticated: u32 => [0xb2, 0x1c, 0xf4],
    persona_id: i64 => [0xc2, 0x99, 0x00],
    client_platform: i64 => [0xc2, 0xc8, 0x74],
    status: i64 => [0xcf, 0x48, 0x73],
    external_id: u64 => [0xe3, 0x29, 0x66],
});
// The two enum fields above preserve raw values: variants/ranges remain unknown.
schema!(UserLoginInfo {
    is_first_console_login: bool => [0x46, 0x3b, 0xee],
    blaze_user_id: i64 => [0x8b, 0x5a, 0x64],
    is_first_login: bool => [0x9b, 0x2c, 0xf4],
    session_key: &'a [u8] => [0xae, 0x5e, 0x40],
    last_login_date_time: i64 => [0xb2, 0xcb, 0xe7],
    email: &'a [u8] => [0xb6, 0x1a, 0x6c],
    persona_details: PersonaDetails<'a> => [0xc2, 0x4d, 0x2c],
    user_id: i64 => [0xd6, 0x99, 0x00],
});
schema!(LoginResponse {
    is_anonymous: bool => [0x86, 0xeb, 0xee],
    needs_legal_doc: bool => [0xbb, 0x4b, 0xf3],
    user_login_info: UserLoginInfo<'a> => [0xce, 0x5c, 0xf3],
    is_of_legal_contact_age: bool => [0xcf, 0x08, 0x6d],
    is_underage: bool => [0xd6, 0xe9, 0x32],
});

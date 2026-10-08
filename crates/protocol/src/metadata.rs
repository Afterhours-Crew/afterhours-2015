//! Fire2's heat2 metadata region, separate from the framing library and message body.
use crate::{Error, Wire, schema};
use nfs_heat2::{Encoder, Field, Fields, Item, Kind, Limits};
use std::{collections::BTreeSet, fmt};

schema!(Fire2Metadata {
    context: u64 => [0x8e, 0xed, 0x38],
    error_code: i32 => [0x97, 0x2c, 0xa3],
    session_key: &'a [u8] => [0xce, 0xb9, 0x79],
});

/// names for the two observed nonzero statuses, verified in their component
/// name methods. This is deliberately incomplete and supplies no retry policy.
/// Keep the original error_code; a missing name does not imply success.
pub const fn error_name(component: u16, code: i32) -> Option<&'static str> {
    match (component, code) {
        (30_722, 0x0001_7802) => Some("USER_ERR_USER_NOT_FOUND"),
        (2_050, 0x001b_0802) => Some("AUTOLOG_ERR_NEWS_JSON_DECODE"),
        _ => None,
    }
}

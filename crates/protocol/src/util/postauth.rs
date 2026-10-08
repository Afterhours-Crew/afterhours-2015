//! runtime metadata; these are wire models, without endpoint or telemetry policy.
use crate::{Error, Wire, schema};
use nfs_heat2::{Encoder, Field, Fields, Item, Kind, Limits};
use std::{collections::BTreeSet, fmt};

schema!(PostAuthRequest {
    dirty_sock_user_index: i32 => [0x93, 0x3d, 0x69],
    unique_device_id: &'a [u8] => [0xd6, 0x4a, 0x64],
});
schema!(GetTelemetryServerResponse {
    address: &'a [u8] => [0x86, 0x4c, 0xb3],
    is_anonymous: bool => [0x86, 0xeb, 0xee],
    disable: &'a [u8] => [0x92, 0x9c, 0xe1],
    enable_disconnect_telemetry: bool => [0x96, 0x48, 0xf4],
    filter: &'a [u8] => [0x9a, 0x9b, 0x34],
    locale: u32 => [0xb2, 0xf8, 0xc0],
    underage: bool => [0xb6, 0x9b, 0xb2],
    no_toggle_ok: &'a [u8] => [0xba, 0xfb, 0xeb],
    port: u32 => [0xc2, 0xfc, 0xb4],
    send_delay: u32 => [0xce, 0x4b, 0x39],
    session_id: &'a [u8] => [0xce, 0x5c, 0xf3],
    key: &'a [u8] => [0xce, 0xb9, 0x79],
    send_percentage: u32 => [0xcf, 0x08, 0xf4],
    use_server_time: &'a [u8] => [0xcf, 0x4a, 0x6d],
    telemetry_service_name: &'a [u8] => [0xcf, 0x6b, 0xad],
});
schema!(GetTickerServerResponse {
    address: &'a [u8] => [0x86, 0x4c, 0xb3],
    port: u32 => [0xc2, 0xfc, 0xb4],
    key: &'a [u8] => [0xce, 0xb9, 0x79],
});
// TelemetryOpt is an observed enum descriptor. Preserve its integer value;
// named variants and a narrower accepted range have not been established.
schema!(UserOptions {
    telemetry_opt: i64 => [0xd2, 0xdb, 0xf0],
    user_id: i64 => [0xd6, 0x99, 0x00],
});
schema!(PostAuthResponse {
    telemetry_server: GetTelemetryServerResponse<'a> => [0xd2, 0x5b, 0x25],
    ticker_server: GetTickerServerResponse<'a> => [0xd2, 0x98, 0xeb],
    user_options: UserOptions<'a> => [0xd7, 0x2b, 0xf0],
});

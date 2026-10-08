//! Group payloads with typed fields, collections and unions.
//! These models carry no session policy or world-readiness claim.
use crate::{
    Blob, Error, Wire, schema,
    users::{NetworkAddress, NetworkQosData, ObjectId, UserIdentification},
    util::ConfigEntries,
};
use nfs_heat2::{Encoder, Field, Fields, Item, Kind, Limits};
use std::{collections::BTreeSet, fmt};
mod collections;
pub use collections::*;
mod layout;
pub const CREATE_GAME: u16 = 1;
pub const NOTIFY_GAME_SETUP: u16 = 20;
pub const UPDATE_MESH_CONNECTION: u16 = 29;

schema!(ScenarioInfo {
    scenario_name: &'a [u8] => [0xce, 0x39, 0x6e],
    scenario_version: u32 => [0xce, 0x39, 0x76],
    scenario_variant: i32 => [0xce, 0x3d, 0xa1],
    sub_session_name: &'a [u8] => [0xcf, 0x58, 0xae],
});

schema!(RoleCriteria {
    role_entry_criteria_map: ConfigEntries<'a> => [0x8f, 0x2a, 0x74],
    role_capacity: u16 => [0xca, 0x38, 0x70],
});

schema!(RoleInformation {
    role_criteria_map: RoleCriteriaMap<'a> => [0x8f, 0x2a, 0x74],
    multi_role_criteria: ConfigEntries<'a> => [0xca, 0x3c, 0xb4],
});

schema!(HostInfo {
    connection_group_id: u64 => [0x8e, 0xfb, 0xa7],
    connection_slot_id: u8 => [0x8f, 0x3a, 0x64],
    player_id: i64 => [0xa3, 0x0a, 0x64],
    user_session_id: u64 => [0xa3, 0x39, 0x73],
    slot_id: u8 => [0xa3, 0x3b, 0x34],
});

schema!(GameCreationData {
    game_attribs: ConfigEntries<'a> => [0x87, 0x4d, 0x32],
    entry_criteria_map: ConfigEntries<'a> => [0x8f, 0x2a, 0x74],
    game_mod_register: u32 => [0x9e, 0xdc, 0xa7],
    game_name: &'a [u8] => [0x9e, 0xe8, 0x6d],
    game_settings: u32 => [0x9f, 0x39, 0x74],
    network_topology: i32 => [0xbb, 0x4b, 0xf0],
    max_player_capacity: u16 => [0xc2, 0xd8, 0x78],
    min_player_capacity: u16 => [0xc2, 0xda, 0x6e],
    presence_mode: i32 => [0xc3, 0x29, 0x73],
    queue_capacity: u16 => [0xc6, 0x38, 0x70],
    role_information: RoleInformation<'a> => [0xca, 0xe9, 0xaf],
    external_session_template_name: &'a [u8] => [0xcf, 0x4b, 0x6e],
    voip_network: i32 => [0xda, 0xfa, 0x70],
});

schema!(PerPlayerJoinData {
    is_optional_player: bool => [0xa7, 0x29, 0x70],
    player_attributes: ConfigEntries<'a> => [0xc2, 0xce, 0x61],
    role: &'a [u8] => [0xca, 0xcb, 0xad],
    user: UserIdentification<'a> => [0xd7, 0x3a, 0x64],
});

schema!(PlayerJoinData {
    group_id: ObjectId => [0x8b, 0x4c, 0x2c],
    default_role: &'a [u8] => [0x92, 0x6c, 0xac],
    game_entry_type: i32 => [0x9e, 0x5b, 0xb4],
    player_data_list: PlayerJoinList<'a> => [0xc2, 0xc9, 0x2c],
    slot_type: i32 => [0xce, 0xcb, 0xf4],
    team_id: u16 => [0xd2, 0x99, 0x00],
    team_index: u16 => [0xd2, 0x99, 0x38],
});

schema!(CommonGameRequestData {
    game_type: i32 => [0x9e, 0x7d, 0x39],
    game_protocol_version_string: &'a [u8] => [0x9f, 0x69, 0x72],
    originating_scenario_id: u64 => [0xbf, 0x3a, 0x64],
    player_network_address: NetworkAddress<'a> => [0xc2, 0xe9, 0x74],
    scenario_info: ScenarioInfo<'a> => [0xce, 0x3a, 0x6f],
    x_lspnetwork_address: NetworkAddress<'a> => [0xe2, 0xe9, 0x74],
});

schema!(CreateGameRequest {
    admin_player_list: PlayerIds => [0x86, 0x4b, 0x6e],
    common_game_data: CommonGameRequestData<'a> => [0x8e, 0xd9, 0xe4],
    game_ping_site_alias: &'a [u8] => [0x9e, 0x3d, 0x32],
    game_creation_data: GameCreationData<'a> => [0x9e, 0xd8, 0xe4],
    game_report_name: &'a [u8] => [0x9f, 0x4e, 0x70],
    game_status_url: &'a [u8] => [0x9f, 0x5c, 0xac],
    mesh_attribs: ConfigEntries<'a> => [0xb6, 0x1d, 0x32],
    server_not_resetable: bool => [0xbb, 0x29, 0x73],
    slot_capacities: CapacityList => [0xc2, 0x38, 0x70],
    persisted_game_id: &'a [u8] => [0xc2, 0x7a, 0x64],
    persisted_game_id_secret: Blob<'a> => [0xc2, 0x7c, 0xe3],
    player_join_data: PlayerJoinData<'a> => [0xc2, 0xca, 0xa4],
    team_ids: CapacityList => [0xd2, 0x99, 0x33],
});

schema!(CreateGameResponse {
    game_id: u64 => [0x9e, 0x99, 0x00],
    joined_reserved_player_identifications: UserIdentificationList<'a> => [0xca, 0x5a, 0x40],
});

schema!(ReplicatedGameData {
    admin_player_list: PlayerIds => [0x86, 0x4b, 0x6e],
    owns_first_party_presence: bool => [0x87, 0x0c, 0xb3],
    game_attribs: ConfigEntries<'a> => [0x87, 0x4d, 0x32],
    slot_capacities: CapacityList => [0x8e, 0x1c, 0x00],
    external_session_correlation_id: &'a [u8] => [0x8e, 0xfa, 0x64],
    entry_criteria_map: ConfigEntries<'a> => [0x8f, 0x2a, 0x74],
    create_time: i64 => [0x8f, 0x4a, 0x6d],
    dedicated_server_host_info: HostInfo<'a> => [0x92, 0x8c, 0xf4],
    dedicated_server_host_network_address_list: NetworkAddressList<'a> => [0x92, 0xe9, 0x74],
    external_session_name: &'a [u8] => [0x97, 0x3b, 0xad],
    game_type: i32 => [0x9e, 0x7d, 0x39],
    game_id: u64 => [0x9e, 0x99, 0x00],
    game_mod_register: u32 => [0x9e, 0xdc, 0xa7],
    game_name: &'a [u8] => [0x9e, 0xe8, 0x6d],
    game_protocol_version_hash: u64 => [0x9f, 0x0d, 0xa8],
    game_settings: u32 => [0x9f, 0x39, 0x74],
    game_reporting_id: u64 => [0x9f, 0x3a, 0x64],
    game_state: i32 => [0x9f, 0x3d, 0x21],
    game_report_name: &'a [u8] => [0x9f, 0x4e, 0x70],
    game_status_url: &'a [u8] => [0x9f, 0x5c, 0xac],
    topology_host_network_address_list: NetworkAddressList<'a> => [0xa2, 0xe9, 0x74],
    mesh_attribs: ConfigEntries<'a> => [0xb6, 0x1d, 0x32],
    max_player_capacity: u16 => [0xb6, 0x38, 0x70],
    min_player_capacity: u16 => [0xb6, 0xe8, 0xf0],
    np_session_id: &'a [u8] => [0xbb, 0x0c, 0xe9],
    network_qos_data: NetworkQosData<'a> => [0xbb, 0x1b, 0xf3],
    server_not_resetable: bool => [0xbb, 0x29, 0x73],
    network_topology: i32 => [0xbb, 0x4b, 0xf0],
    persisted_game_id: &'a [u8] => [0xc2, 0x7a, 0x64],
    persisted_game_id_secret: Blob<'a> => [0xc2, 0x7c, 0xf2],
    platform_host_info: HostInfo<'a> => [0xc2, 0x8c, 0xf4],
    presence_mode: i32 => [0xc3, 0x29, 0x73],
    ping_site_alias: &'a [u8] => [0xc3, 0x38, 0x73],
    queue_capacity: u16 => [0xc6, 0x38, 0x70],
    role_information: RoleInformation<'a> => [0xca, 0xe9, 0xaf],
    scid: &'a [u8] => [0xce, 0x3a, 0x64],
    shared_seed: u32 => [0xce, 0x59, 0x64],
    external_session_template_name: &'a [u8] => [0xcf, 0x4b, 0x6e],
    topology_host_info: HostInfo<'a> => [0xd2, 0x8c, 0xf4],
    team_ids: CapacityList => [0xd2, 0x99, 0x33],
    uuid: &'a [u8] => [0xd7, 0x5a, 0x64],
    voip_network: i32 => [0xda, 0xfa, 0x70],
    game_protocol_version_string: &'a [u8] => [0xdb, 0x3d, 0x32],
    xnet_nonce: Blob<'a> => [0xe2, 0xeb, 0xa3],
    xnet_session: Blob<'a> => [0xe3, 0x39, 0x73],
}, layout::decode_game, layout::finish_game);

schema!(ReplicatedGamePlayer {
    custom_data: Blob<'a> => [0x8a, 0xcb, 0xe2],
    connection_group_id: u64 => [0x8e, 0xfb, 0xa7],
    connection_slot_id: u8 => [0x8f, 0x3a, 0x64],
    dirty_sock_user_index: i32 => [0x93, 0x3d, 0x69],
    external_blob: Blob<'a> => [0x97, 0x88, 0xac],
    external_id: u64 => [0x97, 0x8a, 0x64],
    game_id: u64 => [0x9e, 0x99, 0x00],
    has_join_first_party_game_session_permission: bool => [0xaa, 0x6c, 0x33],
    joined_via_matchmaking: bool => [0xab, 0x6b, 0x6d],
    account_locale: u32 => [0xb2, 0xf8, 0xc0],
    player_name: &'a [u8] => [0xba, 0x1b, 0x65],
    persona_namespace: &'a [u8] => [0xba, 0x1c, 0xf0],
    player_attribs: ConfigEntries<'a> => [0xc2, 0x1d, 0x34],
    player_id: i64 => [0xc2, 0x99, 0x00],
    network_address: NetworkAddress<'a> => [0xc2, 0xe9, 0x74],
    player_settings: u32 => [0xc3, 0x39, 0x74],
    reservation_creation_timestamp: i64 => [0xca, 0x3c, 0xa5],
    role_name: &'a [u8] => [0xca, 0xfb, 0x25],
    slot_id: u8 => [0xce, 0x99, 0x00],
    slot_type: i32 => [0xce, 0xcb, 0xf4],
    player_state: i32 => [0xcf, 0x48, 0x74],
    team_index: u16 => [0xd2, 0x99, 0x38],
    joined_game_timestamp: i64 => [0xd2, 0x9b, 0x65],
    user_group_id: ObjectId => [0xd6, 0x7a, 0x64],
    player_session_id: u64 => [0xd6, 0x99, 0x00],
    uuid: &'a [u8] => [0xd7, 0x5a, 0x64],
});

schema!(QosSettings {
    duration_ms: u32 => [0x93, 0x5c, 0xa1],
    interval_ms: u32 => [0xa6, 0xed, 0x36],
    packet_size: u32 => [0xce, 0x9e, 0xa5],
});

schema!(MeshConnectionQosInfo {
    packet_loss: PacketLossBits => [0xb2, 0xfc, 0xf3],
    latency_ms: u32 => [0xc2, 0x9b, 0xa7],
});

schema!(UpdateMeshConnectionRequest {
    player_net_connection_flags: u32 => [0x9a, 0xc9, 0xf3],
    game_id: u64 => [0x9e, 0x99, 0x00],
    qos_info: MeshConnectionQosInfo<'a> => [0xc6, 0xfc, 0xe9],
    source_group_id: ObjectId => [0xce, 0x39, 0xc0],
    player_net_connection_status: i32 => [0xcf, 0x48, 0x74],
    target_group_id: ObjectId => [0xd2, 0x39, 0xc0],
});

schema!(DatalessSetupContext {
    setup_context: i32 => [0x92, 0x3d, 0x38],
});

schema!(NotifyGameSetup {
    game_data: ReplicatedGameData<'a> => [0x9e, 0x1b, 0x65],
    is_lockable_for_preferred_joins: bool => [0xb2, 0x6c, 0x2a],
    game_mode_attribute_name: &'a [u8] => [0xb6, 0xe8, 0x6d],
    game_roster: PlayerRoster<'a> => [0xc3, 0x2b, 0xf3],
    qos_settings: QosSettings<'a> => [0xc6, 0xfc, 0xf3],
    perform_qos_validation: bool => [0xc6, 0xfc, 0xf6],
    game_queue: PlayerRoster<'a> => [0xc7, 0x59, 0x75],
    game_setup_reason: GameSetupReason<'a> => [0xca, 0x58, 0x73],
    qos_telemetry_interval: i64 => [0xd2, 0x5b, 0x2d],
}, layout::decode_setup, layout::finish_setup);

//! Matchmaking status payload graph.
//! No successful-match, world, notification-order or UI-effect policy.
use super::{CapacityList, EmptyVariableCustomRulePrefs};
use crate::{Error, Wire, autolog::StringList, schema};
use nfs_heat2::{
    Document, Encoder, Field, Fields, Item, Kind, Limits, Member, Schema, Type, TypeId, Value,
};
use std::{collections::BTreeSet, fmt};
pub const NOTIFY_MATCHMAKING_ASYNC_STATUS: u16 = 12;
// 20 members.
schema!(MatchmakingAsyncStatus {
    create_game_status: CreateGameStatus<'a> => [0x8e, 0x7c, 0xc0],
    variable_custom_async_status: EmptyVariableCustomRulePrefs => [0x8f, 0x68, 0x72],
    find_game_status: FindGameStatus<'a> => [0x9a, 0x7c, 0xc0],
    game_attribute_rule_status_map: GameAttributeRuleStatusMap<'a> => [0x9e, 0x1c, 0xed],
    geo_location_rule_status: GeoLocationRuleStatus<'a> => [0x9e, 0x5b, 0xf3],
    host_balance_rule_status: HostBalanceRuleStatus<'a> => [0xa2, 0x2c, 0xa4],
    host_viability_rule_status: HostViabilityRuleStatus<'a> => [0xa3, 0x6c, 0xa4],
    player_attribute_rule_status_map: PlayerAttributeRuleStatusMap<'a> => [0xc2, 0x1c, 0xed],
    player_count_rule_status: PlayerCountRuleStatus<'a> => [0xc2, 0xc8, 0xee],
    player_slot_utilization_rule_status: PlayerSlotUtilizationRuleStatus<'a> => [0xc2, 0xcd, 0x74],
    ping_site_rule_status: PingSiteRuleStatus<'a> => [0xc3, 0x3c, 0xb3],
    rank_rule_status: RankRuleStatus<'a> => [0xcb, 0x29, 0x21],
    team_balance_rule_status: TeamBalanceRuleStatus<'a> => [0xd2, 0x2c, 0xb3],
    team_composition_rule_status: TeamCompositionRuleStatus<'a> => [0xd2, 0x3c, 0x33],
    team_min_size_rule_status: TeamMinSizeRuleStatus<'a> => [0xd2, 0xdc, 0xf3],
    total_player_slots_rule_status: TotalPlayerSlotsRuleStatus<'a> => [0xd2, 0xfd, 0x33],
    team_ued_position_parity_rule_status: TeamUEDPositionParityRuleStatus<'a> => [0xd3, 0x0c, 0x33],
    team_ued_balance_rule_status: TeamUEDBalanceRuleStatus<'a> => [0xd3, 0x58, 0xb3],
    ued_rule_status_map: UEDRuleStatusMap<'a> => [0xd6, 0x59, 0x33],
    virtual_game_rule_status: VirtualGameRuleStatus<'a> => [0xda, 0x7c, 0xb3],
}, decode_async, finish_async);
// 3 members.
schema!(CreateGameStatus {
    evaluate_status: u32 => [0x97, 0x6c, 0xf4],
    num_of_matchmaking_session: u32 => [0xb6, 0xdc, 0xee],
    num_of_matched_players: u32 => [0xba, 0xfb, 0x70],
});
// 1 members.
schema!(FindGameStatus {
    num_of_games: u32 => [0x9e, 0xed, 0x6d],
});
// 1 members.
schema!(GeoLocationRuleStatus {
    max_distance: u32 => [0x92, 0x9c, 0xf4],
});
// 1 members.
schema!(HostBalanceRuleStatus {
    matched_host_balance_value: i32 => [0x8b, 0x68, 0x6c],
});
// 1 members.
schema!(HostViabilityRuleStatus {
    matched_host_viability_value: i32 => [0xdb, 0x68, 0x6c],
});
// 2 members.
schema!(PlayerCountRuleStatus {
    max_player_count_accepted: u16 => [0xc2, 0xd8, 0x78],
    min_player_count_accepted: u16 => [0xc2, 0xda, 0x6e],
});
// 2 members.
schema!(PlayerSlotUtilizationRuleStatus {
    max_percent_full_accepted: u8 => [0xc2, 0xd8, 0x78],
    min_percent_full_accepted: u8 => [0xc2, 0xda, 0x6e],
});
// 1 members.
schema!(PingSiteRuleStatus {
    matched_values: StringList<'a> => [0xda, 0x1b, 0x35],
});
// 1 members.
schema!(RankRuleStatus {
    matched_rank_flags: u8 => [0xcb, 0x68, 0x6c],
});
// 1 members.
schema!(TeamBalanceRuleStatus {
    max_team_size_difference_accepted: u16 => [0xce, 0x4a, 0x66],
});
// 3 members.
schema!(TeamCompositionRuleStatus {
    acceptable_compositions_for_my_team: CapacityList => [0xb7, 0x9d, 0x2d],
    rule_name: &'a [u8] => [0xba, 0x1b, 0x65],
    acceptable_compositions_for_other_teams: CapacityList => [0xbf, 0x4d, 0x2d],
});
// 1 members.
schema!(TeamMinSizeRuleStatus {
    team_min_size_accepted: u16 => [0xc2, 0x3b, 0xb4],
});
// 2 members.
schema!(TotalPlayerSlotsRuleStatus {
    max_total_player_slots_accepted: u16 => [0xc2, 0xd8, 0x78],
    min_total_player_slots_accepted: u16 => [0xc2, 0xda, 0x6e],
});
// 5 members.
schema!(TeamUEDPositionParityRuleStatus {
    max_ued_difference_accepted_bottom_players: u64 => [0x8a, 0x4a, 0x66],
    bottom_players_counted: u16 => [0x8a, 0xfd, 0x2e],
    rule_name: &'a [u8] => [0xba, 0x1b, 0x65],
    max_ued_difference_accepted_top_players: u64 => [0xd2, 0x4a, 0x66],
    top_players_counted: u16 => [0xd2, 0xfc, 0x2e],
});
// 3 members.
schema!(TeamUEDBalanceRuleStatus {
    my_ued_value: i64 => [0xb7, 0x59, 0x64],
    rule_name: &'a [u8] => [0xba, 0x1b, 0x65],
    max_team_ued_difference_accepted: u64 => [0xce, 0x4a, 0x66],
});
// 1 members.
schema!(VirtualGameRuleStatus {
    matched_virtualized_flags: u8 => [0xdb, 0x68, 0x6c],
});
// 2 members.
schema!(GameAttributeRuleStatus {
    rule_name: &'a [u8] => [0xba, 0x1b, 0x65],
    matched_values: StringList<'a> => [0xda, 0x1b, 0x35],
});
// 2 members.
schema!(PlayerAttributeRuleStatus {
    rule_name: &'a [u8] => [0xba, 0x1b, 0x65],
    matched_values: StringList<'a> => [0xda, 0x1b, 0x35],
});
// 4 members.
schema!(UEDRuleStatus {
    max_ued_accepted: i64 => [0x86, 0xd8, 0x78],
    min_ued_accepted: i64 => [0x86, 0xda, 0x6e],
    my_ued_value: i64 => [0xb7, 0x59, 0x64],
    rule_name: &'a [u8] => [0xba, 0x1b, 0x65],
});
// ASIL is a struct list.
schema!(NotifyMatchmakingAsyncStatus {
    matchmaking_async_status_list: MatchmakingAsyncStatusList<'a> => [0x87, 0x3a, 0x6c],
    scenario_id: u64 => [0xb7, 0x38, 0xe4],
    session_id: u64 => [0xb7, 0x3a, 0x64],
    user_session_id: u64 => [0xd7, 0x3a, 0x64],
}, decode_notify, finish_notify);
macro_rules! rule_map {
    ($name:ident, $element:ident) => {
        /// Native string keys retain order; duplicate keys are rejected.
        #[derive(Default)]
        pub struct $name<'a>(pub Vec<(&'a [u8], $element<'a>)>);
        impl fmt::Debug for $name<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_tuple(stringify!($name))
                    .field(&self.0.len())
                    .finish()
            }
        }
        impl<'a> Wire<'a> for $name<'a> {
            const KIND: Kind = Kind::Map;
            fn read(item: Item<'a>) -> Result<Self, Error> {
                if !matches!(
                    item.value(),
                    Value::Map {
                        key: Kind::String,
                        value: Kind::Struct,
                        ..
                    }
                ) {
                    return Err(Error::WrongType { tag: None });
                }
                let mut elements = item.elements().ok_or(Error::WrongType { tag: None })?;
                let mut entries = Vec::new();
                while let Some(key) = elements.next() {
                    entries.push((
                        <&[u8]>::read(key?)?,
                        $element::read(elements.next().ok_or(Error::WrongType { tag: None })??)?,
                    ));
                }
                crate::unique_keys(entries.iter().map(|(key, _)| *key))?;
                Ok(Self(entries))
            }
            fn write(&self, tag: [u8; 3], w: &mut Encoder) -> Result<(), nfs_heat2::Error> {
                w.string_struct_map(tag, &self.0, |w, value| value.write_fields(w))
            }
            fn validate(&self, budget: &mut crate::Budget) -> Result<(), Error> {
                budget.take(1)?;
                budget.collection(self.0.len(), 1)?;
                crate::unique_keys(self.0.iter().map(|(key, _)| *key))?;
                for (key, value) in &self.0 {
                    crate::check_string(key, budget.limits)?;
                    value.validate(budget)?;
                }
                Ok(())
            }
            fn unknown_count(&self) -> usize {
                self.0
                    .iter()
                    .map(|(_, value)| value.unknown_field_count())
                    .sum()
            }
        }
    };
}
rule_map!(GameAttributeRuleStatusMap, GameAttributeRuleStatus);
rule_map!(PlayerAttributeRuleStatusMap, PlayerAttributeRuleStatus);
rule_map!(UEDRuleStatusMap, UEDRuleStatus);
macro_rules! struct_list {
    ($name:ident, $element:ident) => {
        #[derive(Default)]
        pub struct $name<'a>(pub Vec<$element<'a>>);
        impl fmt::Debug for $name<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_tuple(stringify!($name))
                    .field(&self.0.len())
                    .finish()
            }
        }
        impl<'a> Wire<'a> for $name<'a> {
            const KIND: Kind = Kind::List;
            fn read(item: Item<'a>) -> Result<Self, Error> {
                if !matches!(
                    item.value(),
                    Value::List {
                        element: Kind::Struct,
                        ..
                    }
                ) {
                    return Err(Error::WrongType { tag: None });
                }
                Ok(Self(
                    item.elements()
                        .ok_or(Error::WrongType { tag: None })?
                        .map(|i| $element::read(i?))
                        .collect::<Result<_, _>>()?,
                ))
            }
            fn write(&self, tag: [u8; 3], w: &mut Encoder) -> Result<(), nfs_heat2::Error> {
                w.struct_list(tag, &self.0, |w, value| value.write_fields(w))
            }
            fn validate(&self, budget: &mut crate::Budget) -> Result<(), Error> {
                budget.take(1)?;
                budget.collection(self.0.len(), 0)?;
                for value in &self.0 {
                    value.validate(budget)?;
                }
                Ok(())
            }
            fn unknown_count(&self) -> usize {
                self.0.iter().map(Wire::unknown_count).sum()
            }
        }
    };
}
struct_list!(MatchmakingAsyncStatusList, MatchmakingAsyncStatus);

const fn member(tag: [u8; 3], ty: usize) -> Member {
    Member {
        tag,
        ty: TypeId(ty),
    }
}
const TYPES: &[Type<'static>] = &[
    Type::Scalar(Kind::String), // 0
    Type::Struct(&[]),          // 1: typed leaves
    Type::Map {
        key: TypeId(0),
        value: TypeId(1),
    }, // 2: string -> status struct
    Type::Scalar(Kind::Variable), // 3
    Type::Map {
        key: TypeId(0),
        value: TypeId(3),
    }, // 4: absent/empty CVAR only
    Type::Struct(&[
        member([0x8f, 0x68, 0x72], 4),
        member([0x9e, 0x1c, 0xed], 2),
        member([0xc2, 0x1c, 0xed], 2),
        member([0xd6, 0x59, 0x33], 2),
    ]), // 5: MatchmakingAsyncStatus
    Type::List(TypeId(5)),      // 6
    Type::Struct(&[member([0x87, 0x3a, 0x6c], 6)]), // 7: notification
];
fn layout() -> Schema<'static> {
    Schema::new(TYPES).expect("constant status graph")
}
fn decode_async(bytes: &[u8], limits: Limits) -> Result<Document<'_>, nfs_heat2::Error> {
    nfs_heat2::decode_with_schema(bytes, limits, layout(), TypeId(5))
}
fn finish_async(w: Encoder) -> Result<Vec<u8>, nfs_heat2::Error> {
    w.finish_with_schema(layout(), TypeId(5))
}
fn decode_notify(bytes: &[u8], limits: Limits) -> Result<Document<'_>, nfs_heat2::Error> {
    nfs_heat2::decode_with_schema(bytes, limits, layout(), TypeId(7))
}
fn finish_notify(w: Encoder) -> Result<Vec<u8>, nfs_heat2::Error> {
    w.finish_with_schema(layout(), TypeId(7))
}

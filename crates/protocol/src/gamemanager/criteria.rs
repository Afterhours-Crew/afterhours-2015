//! StartMatchmaking input graph. Pure byte models, no acceptance policy.
//! Native TimeValue remains signed; no units or timing defaults are assigned.
use super::{CommonGameRequestData, GameCreationData, GameIds, PlayerIds, PlayerJoinData};
use crate::{
    Error, Wire,
    autolog::StringList,
    schema,
    users::{ObjectId, ObjectIdList},
};
use nfs_heat2::{
    Document, Encoder, Field, Fields, Item, Kind, Limits, Member, Schema, Type, TypeId, Value,
};
use std::{collections::BTreeSet, fmt};

schema!(PingSiteRulePrefs { min_fit_threshold_name: &'a [u8] => [0xd2, 0x8b, 0x24] });

schema!(RankedGameRulePrefs {
    min_fit_threshold_name: &'a [u8] => [0xd2, 0x8b, 0x24],
    desired_ranked_game_value: i32 => [0xda, 0x1b, 0x35],
});

schema!(RosterSizeRulePrefs {
    max_player_count: u16 => [0xc2, 0x38, 0x70],
    min_player_count: u16 => [0xc2, 0xda, 0x6e],
});

schema!(HostBalancingRulePrefs { min_fit_threshold_name: &'a [u8] => [0xd2, 0x8b, 0x24] });

schema!(HostViabilityRulePrefs { min_fit_threshold_name: &'a [u8] => [0xd2, 0x8b, 0x24] });

schema!(GeoLocationRuleCriteria { min_fit_threshold_name: &'a [u8] => [0xd2, 0x8b, 0x24] });

schema!(GameNameRuleCriteria { search_string: &'a [u8] => [0xcf, 0x58, 0xb3] });

schema!(VirtualGameRulePrefs {
    min_fit_threshold_name: &'a [u8] => [0xd2, 0x8b, 0x24],
    desired_virtual_game_value: i32 => [0xda, 0x1b, 0x35],
});

schema!(AvoidGamesRuleCriteria { game_id_list: GameIds => [0x9e, 0x99, 0x2c] });

schema!(AvoidPlayersRuleCriteria {
    avoid_list: PlayerIds => [0x86, 0xcc, 0xf4],
    avoid_list_ids: ObjectIdList => [0x87, 0x3d, 0x33],
});

schema!(PreferredPlayersRuleCriteria {
    preferred_list: PlayerIds => [0xc2, 0xcc, 0xf4],
    preferred_list_id: ObjectId => [0xc3, 0x39, 0x74],
    require_preferred_player: bool => [0xca, 0x5c, 0x70],
});

schema!(PreferredGamesRuleCriteria {
    preferred_list: GameIds => [0xc2, 0xcc, 0xf4],
    require_preferred_game: bool => [0xca, 0x5c, 0x70],
});

schema!(GameAttributeRuleCriteria {
    min_fit_threshold_name: &'a [u8] => [0xd2, 0x8b, 0x24],
    desired_values: StringList<'a> => [0xda, 0x1b, 0x35],
});

schema!(PlayerAttributeRuleCriteria {
    min_fit_threshold_name: &'a [u8] => [0xd2, 0x8b, 0x24],
    desired_values: StringList<'a> => [0xda, 0x1b, 0x35],
});

schema!(UEDRuleCriteria {
    client_ued_search_value: i64 => [0x8f, 0x68, 0x6c],
    override_ued_value: i64 => [0xbf, 0x68, 0x6c],
    threshold_name: &'a [u8] => [0xd2, 0x8b, 0x24],
});

schema!(ModRuleCriteria {
    is_enabled: bool => [0xa7, 0x39, 0x6e],
    desired_mod_register: u32 => [0xb6, 0xf9, 0x33],
});

schema!(PlayerCountRuleCriteria {
    is_single_group_match: u8 => [0xa7, 0x3c, 0xe7],
    max_player_count: u16 => [0xc2, 0x38, 0x70],
    desired_player_count: u16 => [0xc2, 0x3b, 0xb4],
    min_player_count: u16 => [0xc2, 0xda, 0x6e],
    range_offset_list_name: &'a [u8] => [0xd2, 0x8b, 0x24],
});

schema!(TotalPlayerSlotsRuleCriteria {
    desired_total_player_slots: u16 => [0x92, 0x5c, 0xf3],
    max_total_player_slots: u16 => [0xb6, 0x1e, 0x33],
    min_total_player_slots: u16 => [0xb6, 0x9b, 0xb3],
    range_offset_list_name: &'a [u8] => [0xd2, 0x8b, 0x24],
});

schema!(FreePlayerSlotsRuleCriteria {
    max_free_player_slots: u16 => [0xb6, 0x1e, 0x33],
    min_free_player_slots: u16 => [0xb6, 0x9b, 0xb3],
});

schema!(PlayerSlotUtilizationRuleCriteria {
    desired_percent_full: u8 => [0x92, 0x5c, 0xf0],
    max_percent_full: u8 => [0xb6, 0x1e, 0x30],
    min_percent_full: u8 => [0xb6, 0x9b, 0xb0],
    range_offset_list_name: &'a [u8] => [0xd2, 0x8b, 0x24],
});

schema!(TeamBalanceRulePrefs {
    max_team_size_difference_allowed: u16 => [0xce, 0x4a, 0x66],
    range_offset_list_name: &'a [u8] => [0xd2, 0x8b, 0x24],
});

schema!(TeamUEDPositionParityRulePrefs {
    rule_name: &'a [u8] => [0xba, 0x1b, 0x65],
    range_offset_list_name: &'a [u8] => [0xd2, 0x8b, 0x24],
});

schema!(TeamUEDBalanceRulePrefs {
    rule_name: &'a [u8] => [0xba, 0x1b, 0x65],
    range_offset_list_name: &'a [u8] => [0xd2, 0x8b, 0x24],
});

schema!(TeamCompositionRulePrefs {
    rule_name: &'a [u8] => [0xba, 0x1b, 0x65],
    min_fit_threshold_name: &'a [u8] => [0xd2, 0x8b, 0x24],
});

schema!(TeamMinSizeRulePrefs {
    team_min_size: u16 => [0xc2, 0x3b, 0xb4],
    range_offset_list_name: &'a [u8] => [0xd2, 0x8b, 0x24],
});

schema!(TeamCountRulePrefs { team_count: u16 => [0xd2, 0x3b, 0xb4] });

schema!(ReputationRulePrefs { reputation_requirement: i32 => [0xca, 0x5c, 0x32] });

schema!(MatchmakingCriteriaData {
    avoid_games_rule_criteria: AvoidGamesRuleCriteria<'a> => [0x86, 0x78, 0x6d],
    avoid_players_rule_criteria: AvoidPlayersRuleCriteria<'a> => [0x87, 0x0b, 0x32],
    variable_custom_rule_prefs: EmptyVariableCustomRulePrefs => [0x8f, 0x68, 0x72],
    free_player_slots_rule_criteria: FreePlayerSlotsRuleCriteria<'a> => [0x9b, 0x29, 0x73],
    game_attribute_rule_criteria_map: GameAttributeRuleCriteriaMap<'a> => [0x9e, 0x1c, 0xa3],
    geo_location_rule_criteria: GeoLocationRuleCriteria<'a> => [0x9e, 0x5b, 0xc0],
    game_name_rule_criteria: GameNameRuleCriteria<'a> => [0x9e, 0xe8, 0x6d],
    mod_rule_criteria: ModRuleCriteria<'a> => [0xb6, 0xf9, 0x32],
    host_balancing_rule_prefs: HostBalancingRulePrefs<'a> => [0xba, 0x1d, 0x00],
    player_attribute_rule_criteria_map: PlayerAttributeRuleCriteriaMap<'a> => [0xc2, 0x1c, 0xa3],
    player_count_rule_criteria: PlayerCountRuleCriteria<'a> => [0xc2, 0x3b, 0xb4],
    player_slot_utilization_rule_criteria: PlayerSlotUtilizationRuleCriteria<'a> => [0xc2, 0x3d, 0x26],
    preferred_games_rule_criteria: PreferredGamesRuleCriteria<'a> => [0xc2, 0x7c, 0x80],
    preferred_players_rule_criteria: PreferredPlayersRuleCriteria<'a> => [0xc3, 0x0b, 0x32],
    ping_site_rule_prefs: PingSiteRulePrefs<'a> => [0xc3, 0x3c, 0x80],
    ranked_game_rule_prefs: RankedGameRulePrefs<'a> => [0xca, 0x1b, 0xab],
    reputation_rule_prefs: ReputationRulePrefs<'a> => [0xca, 0x5c, 0x00],
    roster_size_rule_prefs: RosterSizeRulePrefs<'a> => [0xcb, 0x3e, 0xb2],
    team_balance_rule_prefs: TeamBalanceRulePrefs<'a> => [0xd2, 0x2c, 0x80],
    team_count_rule_prefs: TeamCountRulePrefs<'a> => [0xd2, 0x3b, 0xb2],
    team_composition_rule_prefs: TeamCompositionRulePrefs<'a> => [0xd2, 0x3c, 0x80],
    team_min_size_rule_prefs: TeamMinSizeRulePrefs<'a> => [0xd2, 0xdc, 0xf2],
    total_player_slots_rule_criteria: TotalPlayerSlotsRuleCriteria<'a> => [0xd2, 0xfd, 0x33],
    team_ued_position_parity_rule_prefs: TeamUEDPositionParityRulePrefs<'a> => [0xd3, 0x0c, 0x35],
    team_ued_balance_rule_prefs: TeamUEDBalanceRulePrefs<'a> => [0xd3, 0x5c, 0x80],
    ued_rule_criteria_map: UEDRuleCriteriaMap<'a> => [0xd6, 0x59, 0x00],
    host_viability_rule_prefs: HostViabilityRulePrefs<'a> => [0xda, 0x98, 0x62],
    virtual_game_rule_prefs: VirtualGameRulePrefs<'a> => [0xda, 0x9c, 0xb4],
}, decode_criteria, finish_criteria);

schema!(MatchmakingSessionData {
    session_duration: i64 => [0x93, 0x5c, 0x80],
    debug_freeze_decay: bool => [0x9b, 0x2b, 0xfa],
    session_mode: i32 => [0xb6, 0xf9, 0x25],
    external_mm_session_template_name: &'a [u8] => [0xb7, 0x3d, 0x2e],
    pseudo_request: bool => [0xc3, 0x39, 0x2f],
    starting_decay_age: i64 => [0xce, 0x49, 0x63],
    start_delay: i64 => [0xce, 0x49, 0x6c],
});

schema!(StartMatchmakingRequest {
    common_game_data: CommonGameRequestData<'a> => [0x8e, 0xd9, 0xe4],
    criteria_data: MatchmakingCriteriaData<'a> => [0x8f, 0x29, 0x00],
    game_creation_data: GameCreationData<'a> => [0x9e, 0xd8, 0xe4],
    player_join_data: PlayerJoinData<'a> => [0xc2, 0xca, 0xa4],
    session_data: MatchmakingSessionData<'a> => [0xcf, 0x39, 0x34],
}, decode_request, finish_request);

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
rule_map!(GameAttributeRuleCriteriaMap, GameAttributeRuleCriteria);
rule_map!(PlayerAttributeRuleCriteriaMap, PlayerAttributeRuleCriteria);
rule_map!(UEDRuleCriteriaMap, UEDRuleCriteria);

/// Explicitly empty CVAR map, distinct from its absence. Nonempty variable-rule
/// maps require the unvalidated bare-variable callback and fail closed in Heat2.
#[derive(Default, Clone, Copy, Debug, Eq, PartialEq)]
pub struct EmptyVariableCustomRulePrefs;
impl<'a> Wire<'a> for EmptyVariableCustomRulePrefs {
    const KIND: Kind = Kind::Map;
    fn read(item: Item<'a>) -> Result<Self, Error> {
        if matches!(
            item.value(),
            Value::Map {
                key: Kind::String,
                value: Kind::Variable,
                count: 0
            }
        ) {
            Ok(Self)
        } else {
            Err(Error::WrongType { tag: None })
        }
    }
    fn write(&self, tag: [u8; 3], w: &mut Encoder) -> Result<(), nfs_heat2::Error> {
        let bytes = [tag[0], tag[1], tag[2], Kind::Map as u8, 1, 7, 0];
        let doc = nfs_heat2::decode(&bytes, w.limits())?;
        w.raw_field(doc.fields().next().expect("one empty map")?)
    }
    fn validate(&self, budget: &mut crate::Budget) -> Result<(), Error> {
        budget.take(1)?;
        budget.collection(0, 2)
    }
}

// Native collection-byte 3 is resolved by descriptor topology, never guessed.
// Struct leaves deliberately omit scalar members: tagged types are checked by
// their typed Wire readers. Unknown collection-byte 3 remains SchemaRequired.
const fn member(tag: [u8; 3], ty: usize) -> Member {
    Member {
        tag,
        ty: TypeId(ty),
    }
}
const TYPES: &[Type<'static>] = &[
    Type::Scalar(Kind::String), // 0
    Type::Struct(&[]),          // 1: tagged, unambiguous rule/player leaves
    Type::List(TypeId(1)),      // 2: PerPlayerJoinData
    Type::Map {
        key: TypeId(0),
        value: TypeId(1),
    }, // 3: string -> struct
    Type::Struct(&[member([0x8f, 0x2a, 0x74], 3)]), // 4: RoleInformation
    Type::Struct(&[member([0xca, 0xe9, 0xaf], 4)]), // 5: GameCreationData
    Type::Struct(&[member([0xc2, 0xc9, 0x2c], 2)]), // 6: PlayerJoinData
    Type::Struct(&[
        member([0x8f, 0x68, 0x72], 10),
        member([0x9e, 0x1c, 0xa3], 3),
        member([0xc2, 0x1c, 0xa3], 3),
        member([0xd6, 0x59, 0x00], 3),
    ]), // 7: MatchmakingCriteriaData
    Type::Struct(&[
        member([0x8e, 0xd9, 0xe4], 1),
        member([0x8f, 0x29, 0x00], 7),
        member([0x9e, 0xd8, 0xe4], 5),
        member([0xc2, 0xca, 0xa4], 6),
        member([0xcf, 0x39, 0x34], 1),
    ]), // 8: StartMatchmakingRequest
    Type::Scalar(Kind::Variable), // 9
    Type::Map {
        key: TypeId(0),
        value: TypeId(9),
    }, // 10: CVAR, empty only
];
fn schema() -> Schema<'static> {
    Schema::new(TYPES).expect("constant matchmaking input layout")
}
fn decode_criteria(bytes: &[u8], limits: Limits) -> Result<Document<'_>, nfs_heat2::Error> {
    nfs_heat2::decode_with_schema(bytes, limits, schema(), TypeId(7))
}
fn finish_criteria(w: Encoder) -> Result<Vec<u8>, nfs_heat2::Error> {
    w.finish_with_schema(schema(), TypeId(7))
}
fn decode_request(bytes: &[u8], limits: Limits) -> Result<Document<'_>, nfs_heat2::Error> {
    nfs_heat2::decode_with_schema(bytes, limits, schema(), TypeId(8))
}
fn finish_request(w: Encoder) -> Result<Vec<u8>, nfs_heat2::Error> {
    w.finish_with_schema(schema(), TypeId(8))
}

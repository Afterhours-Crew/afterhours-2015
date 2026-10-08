// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! native stats-and-awards schemas. These describe saved account state,
//! not a default profile, reward policy, persistence or entitlement authority.
//! Integer enum map keys remain numeric so unknown enum values can round-trip.
use crate::{Error, Wire, schema};
use nfs_heat2::{Encoder, Field, Fields, Item, Kind, Limits, Value};
use std::{collections::BTreeSet, fmt};
pub const GET_STATS_AND_AWARDS: u16 = 71;

fn unique_keys(keys: impl Iterator<Item = i32>) -> Result<(), Error> {
    let mut seen = BTreeSet::new();
    for key in keys {
        if !seen.insert(key) {
            return Err(Error::DuplicateMapKey);
        }
    }
    Ok(())
}

macro_rules! integer_map {
    ($name:ident, $value:ty) => {
        #[derive(Default)]
        pub struct $name(pub Vec<(i32, $value)>);
        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_tuple(stringify!($name))
                    .field(&self.0.len())
                    .finish()
            }
        }
        impl<'a> Wire<'a> for $name {
            const KIND: Kind = Kind::Map;
            fn read(item: Item<'a>) -> Result<Self, Error> {
                if !matches!(
                    item.value(),
                    Value::Map {
                        key: Kind::Integer,
                        value: Kind::Integer,
                        ..
                    }
                ) {
                    return Err(Error::WrongType { tag: None });
                }
                let mut elements = item.elements().ok_or(Error::WrongType { tag: None })?;
                let mut entries = Vec::new();
                while let Some(key) = elements.next() {
                    entries.push((
                        i32::read(key?)?,
                        <$value>::read(elements.next().ok_or(Error::WrongType { tag: None })??)?,
                    ));
                }
                unique_keys(entries.iter().map(|(k, _)| *k))?;
                Ok(Self(entries))
            }
            fn write(&self, tag: [u8; 3], w: &mut Encoder) -> Result<(), nfs_heat2::Error> {
                w.integer_map(
                    tag,
                    self.0.iter().map(|(k, v)| (i64::from(*k), i64::from(*v))),
                )
            }
            fn validate(&self, budget: &mut crate::Budget) -> Result<(), Error> {
                budget.take(1)?;
                budget.collection(self.0.len(), 2)?;
                unique_keys(self.0.iter().map(|(k, _)| *k))
            }
        }
    };
}
integer_map!(PositionsBreakdown, u32);
integer_map!(StatsAndAwardsEntitlements, bool);

macro_rules! struct_list {
    ($name:ident,$element:ident) => {
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
                w.struct_list(tag, &self.0, |w, v| v.write_fields(w))
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
struct_list!(StatsAndAwardsActivities, StatsAndAwardsActivity);
struct_list!(StatsAndAwardsCollectibles, StatsAndAwardsCollectible);
struct_list!(StatsAndAwardsEvents, StatsAndAwardsEvent);
struct_list!(StatsAndAwardsObjectives, StatsAndAwardsProgressionObjective);

schema!(StatsAndAwardsRequest {
    blaze_id: i64 => [0x8a,0xca,0x64],
});

schema!(StatsAndAwardsActivity {
    collected: bool => [0x8e,0xfb,0x70],
    persistence_key: u32 => [0xc2,0xb9,0x79],
    record_name: &'a [u8] => [0xca,0x58,0xee],
    screenshot_id: u64 => [0xce,0x3a,0x64],
    last_modified: u64 => [0xd2,0x9b,0x65],
});

schema!(StatsAndAwardsCollectible {
    collected: bool => [0x8e,0xfb,0x70],
    persistence_key: u32 => [0xc2,0xb9,0x79],
    record_name: &'a [u8] => [0xca,0x58,0xee],
    screenshot_id: u64 => [0xce,0x3a,0x64],
    last_modified: u64 => [0xd2,0x9b,0x65],
});

schema!(StatsAndAwardsEvent {
    attempts: u32 => [0x86,0x3d,0x29],
    position: u32 => [0x8e,0xfb,0x70],
    event_id: u32 => [0x97,0x6a,0x64],
    record_name: &'a [u8] => [0xca,0x58,0xee],
    screenshot_id: u64 => [0xce,0x3a,0x64],
    last_modified: u64 => [0xd2,0x9b,0x65],
    type_string: &'a [u8] => [0xd3,0x3d,0x32],
});

schema!(StatsAndAwardsGeneralStats {
    biggest_fine_escaped: u32 => [0x8a,0x79,0xa5],
    biggest_fine: u32 => [0x8a,0x79,0xae],
    cash_earned: u32 => [0x8e,0x58,0x72],
    distance_driven: u32 => [0x92,0x49,0x24],
    distance_drifted: u32 => [0x92,0x4c,0xa9],
    favorite_car_id: u32 => [0x9a,0x3a,0x64],
    time_played: u64 => [0xd3,0x0b,0x21],
    top_speed: u32 => [0xd3,0x3c,0x25],
});

schema!(StatsAndAwardsKickbacks {
    total_received_likes: u32 => [0xca,0x58,0xec],
    reward_level: u32 => [0xca,0xc9,0x76],
    total_sent_likes: u32 => [0xce,0xca,0x6b],
    screenshot_count: u32 => [0xcf,0x38,0xc0],
});

schema!(StatsAndAwardsProgressionObjective {
    active: bool => [0x86,0x3d,0x29],
    completed: bool => [0x8e,0xfb,0x70],
    persistence_key: u64 => [0xc2,0xb9,0x79],
    last_modified: u64 => [0xd2,0x9b,0x65],
});

schema!(StatsAndAwardsRepScores {
    build_score: u32 => [0x8b,0x38,0xef],
    crew_score: u32 => [0x8f,0x38,0xef],
    outlaw_score: u32 => [0xbf,0x38,0xef],
    rep_level: u32 => [0xca,0xc9,0x76],
    rep_score: u32 => [0xce,0x3b,0xf2],
    speed_score: u32 => [0xcf,0x0c,0xe3],
    style_score: u32 => [0xcf,0x38,0xef],
    last_modified: u64 => [0xd2,0x9b,0x65],
});

schema!(PrestigeMedalStats {
    build: i32 => [0x8a,0xd9,0x2c],
    crew: i32 => [0x8e,0xd9,0x2c],
    final_event: i32 => [0x9a,0x5b,0x64],
    outlaw: i32 => [0xbe,0xd9,0x2c],
    speed: i32 => [0xce,0xd9,0x2c],
    style: i32 => [0xcf,0x4b,0x64],
});

schema!(SpeedListStats {
    finished_count: u32 => [0x9a,0x3b,0xb4],
    positions_breakdown: PositionsBreakdown => [0x9b,0x08,0xad],
    started_count: u32 => [0xce,0x3b,0xb4],
});

schema!(StatsAndAwardsResponse {
    activities: StatsAndAwardsActivities<'a> => [0x86,0x3d,0x29],
    blaze_id: i64 => [0x8a,0xca,0x64],
    collectibles: StatsAndAwardsCollectibles<'a> => [0x8e,0xfb,0x25],
    race_events: StatsAndAwardsEvents<'a> => [0x97,0x69,0x6e],
    general_stats: StatsAndAwardsGeneralStats<'a> => [0x9f,0x3d,0x21],
    kickbacks: StatsAndAwardsKickbacks<'a> => [0xae,0x98,0xeb],
    prestige_medal_stats: PrestigeMedalStats<'a> => [0xc2,0xdc,0xc0],
    progression_objectives: StatsAndAwardsObjectives<'a> => [0xc2,0xf8,0xaa],
    rep_scores: StatsAndAwardsRepScores<'a> => [0xcb,0x38,0xef],
    speed_list_stats: SpeedListStats<'a> => [0xce,0xcc,0xe1],
    entitlements: StatsAndAwardsEntitlements => [0xcf,0x48,0x69],
});

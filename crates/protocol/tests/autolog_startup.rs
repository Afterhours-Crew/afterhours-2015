use nfs_heat2::{Encoder, Limits};
use nfs_protocol::autolog::*;
fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn independent_requests_match_native_tags_and_recorded_nonpersonal_values() {
    let bytes = hex("8aca640000");
    let q = KillSwitchRequest::decode(&bytes, Limits::default()).unwrap();
    assert_eq!(q.blaze_id, Some(0));
    assert_eq!(q.encode(Limits::default()).unwrap(), bytes);
    let bytes = hex("b70b210032");
    let q = RandomPlayersRequest::decode(&bytes, Limits::default()).unwrap();
    assert_eq!(q.max_players, Some(50));
    assert_eq!(q.encode(Limits::default()).unwrap(), bytes);
    let bytes = hex("9b0d390000b61e300000d39c250000");
    let q = GetRecentPlayersRequest::decode(&bytes, Limits::default()).unwrap();
    assert_eq!(
        (
            q.include_first_party_friends,
            q.max_players_to_return,
            q.sort_order
        ),
        (Some(false), Some(0), Some(0))
    );
    assert!(q.players.is_none());
    assert_eq!(q.encode(Limits::default()).unwrap(), bytes);
}
#[test]
fn independent_response_lists_preserve_order_and_explicit_empty() {
    let bytes = hex("af3b29040102027800027900");
    let m = KillSwitchResponse::decode(&bytes, Limits::default()).unwrap();
    assert_eq!(
        m.kill_switch_list.as_ref().unwrap().0,
        vec![b"x".as_slice(), b"y".as_slice()]
    );
    assert_eq!(m.encode(Limits::default()).unwrap(), bytes);
    let bytes = hex("8aca640400022941");
    let m = RandomPlayersResponse::decode(&bytes, Limits::default()).unwrap();
    assert_eq!(m.blaze_ids.as_ref().unwrap().0, vec![41, -1]);
    assert_eq!(m.encode(Limits::default()).unwrap(), bytes);
    let bytes = hex("8aca64040000");
    let m = RandomPlayersResponse::decode(&bytes, Limits::default()).unwrap();
    assert!(m.blaze_ids.unwrap().0.is_empty());
    assert!(
        RandomPlayersResponse::decode(&[], Limits::default())
            .unwrap()
            .blaze_ids
            .is_none()
    );
}
#[test]
fn native_integer_widths_and_unknown_enum_values_are_retained() {
    for value in [0, u32::MAX] {
        let q = RandomPlayersRequest {
            max_players: Some(value),
            ..Default::default()
        };
        let b = q.encode(Limits::default()).unwrap();
        assert_eq!(
            RandomPlayersRequest::decode(&b, Limits::default())
                .unwrap()
                .max_players,
            Some(value)
        );
    }
    for value in [-1, 4294967296] {
        let mut w = Encoder::new(Limits::default());
        w.integer([0xb7, 0x0b, 0x21], value).unwrap();
        assert!(RandomPlayersRequest::decode(&w.finish().unwrap(), Limits::default()).is_err());
    }
    for value in [i32::MIN, i32::MAX] {
        let q = GetRecentPlayersRequest {
            max_players_to_return: Some(value),
            sort_order: Some(value),
            players: Some(PlayerIds(vec![i64::MIN, i64::MAX])),
            ..Default::default()
        };
        let b = q.encode(Limits::default()).unwrap();
        let q = GetRecentPlayersRequest::decode(&b, Limits::default()).unwrap();
        assert_eq!(q.sort_order, Some(value));
        assert_eq!(q.players.unwrap().0, vec![i64::MIN, i64::MAX]);
    }
}
#[test]
fn malformed_lists_types_duplicates_and_bool_fail() {
    for s in [
        "8aca64040101027800",
        "8aca64040002",
        "8aca6404000180",
        "8aca640400008aca64040000",
    ] {
        assert!(RandomPlayersResponse::decode(&hex(s), Limits::default()).is_err());
    }
    assert!(KillSwitchResponse::decode(&hex("af3b2904000100"), Limits::default()).is_err());
    assert!(GetRecentPlayersRequest::decode(&hex("9b0d390002"), Limits::default()).is_err());
}
#[test]
fn unknowns_and_bounds_apply_to_collections() {
    let b = hex("8aca640400022941ffffff0001");
    let m = RandomPlayersResponse::decode(&b, Limits::default()).unwrap();
    assert_eq!(m.unknown_field_count(), 1);
    assert_eq!(m.encode(Limits::default()).unwrap(), b);
    for limits in [
        Limits {
            max_collection: 1,
            ..Default::default()
        },
        Limits {
            max_values: 2,
            ..Default::default()
        },
        Limits {
            max_bytes: 7,
            ..Default::default()
        },
    ] {
        assert!(RandomPlayersResponse::decode(&b, limits).is_err());
        assert!(m.encode(limits).is_err());
    }
}

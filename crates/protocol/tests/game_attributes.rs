//! native descriptor tags with independent public-safe synthetic bytes.
use nfs_heat2::{Encoder, Limits};
use nfs_protocol::{Error, gamemanager::*, util::ConfigEntries};

fn literal() -> Vec<u8> {
    // ATTR(string -> string, one entry), GID(uint64). Synthetic G1=25/G2=57.
    let mut b = vec![0x87, 0x4d, 0x32, 5, 1, 1, 1, 14];
    b.extend_from_slice(b"gameSessionId\0");
    b.extend_from_slice(&[3, b'5', b'7', 0, 0x9e, 0x99, 0, 0, 25]);
    b
}
#[test]
fn independently_packed_native_attribute_pair_exact() {
    let b = literal();
    assert_eq!(
        SetGameAttributesRequest::FIELD_TAGS,
        &[[0x87, 0x4d, 0x32], [0x9e, 0x99, 0]]
    );
    assert_eq!(
        NotifyGameAttribChange::FIELD_TAGS,
        SetGameAttributesRequest::FIELD_TAGS
    );
    let q = SetGameAttributesRequest::decode(&b, Limits::default()).unwrap();
    assert_eq!(q.game_id, Some(25));
    assert_eq!(
        q.game_attribs.as_ref().unwrap().0,
        vec![(b"gameSessionId".as_slice(), b"57".as_slice())]
    );
    assert_eq!(q.unknown_field_count(), 0);
    assert_eq!(q.encode(Limits::default()).unwrap(), b);
    let n = NotifyGameAttribChange::decode(&b, Limits::default()).unwrap();
    assert_eq!(n.game_id, q.game_id);
    assert_eq!(n.encode(Limits::default()).unwrap(), b);
}
#[test]
fn absent_fields_and_empty_map_remain_distinct() {
    let q = SetGameAttributesRequest::decode(&[], Limits::default()).unwrap();
    assert!(q.game_id.is_none() && q.game_attribs.is_none());
    let q = SetGameAttributesRequest {
        game_attribs: Some(ConfigEntries(vec![])),
        ..Default::default()
    };
    let b = q.encode(Limits::default()).unwrap();
    let q = SetGameAttributesRequest::decode(&b, Limits::default()).unwrap();
    assert!(q.game_attribs.unwrap().0.is_empty());
    assert!(q.game_id.is_none());
}
#[test]
fn unknown_fields_preserved_and_duplicate_known_fields_rejected() {
    let mut b = literal();
    b.extend_from_slice(&[0xff, 0xff, 0xff, 0, 42]);
    for is_notification in [false, true] {
        let (unknown, out) = if is_notification {
            let n = NotifyGameAttribChange::decode(&b, Limits::default()).unwrap();
            (
                n.unknown_field_count(),
                n.encode(Limits::default()).unwrap(),
            )
        } else {
            let q = SetGameAttributesRequest::decode(&b, Limits::default()).unwrap();
            (
                q.unknown_field_count(),
                q.encode(Limits::default()).unwrap(),
            )
        };
        assert_eq!(unknown, 1);
        assert_eq!(out, b);
    }
    let mut b = literal();
    b.extend_from_slice(&[0x9e, 0x99, 0, 0, 26]);
    assert!(matches!(
        SetGameAttributesRequest::decode(&b, Limits::default()),
        Err(Error::DuplicateTag(_))
    ));
}
#[test]
fn native_map_types_duplicate_keys_and_uint64_pattern_checked() {
    for at in [3, 4, 5] {
        let mut b = literal();
        b[at] = 0;
        assert!(SetGameAttributesRequest::decode(&b, Limits::default()).is_err());
    }
    let mut e = Encoder::new(Limits::default());
    e.string_map([0x87, 0x4d, 0x32], &[(b"same", b"a"), (b"same", b"b")])
        .unwrap();
    let b = e.finish().unwrap();
    assert!(matches!(
        SetGameAttributesRequest::decode(&b, Limits::default()),
        Err(Error::DuplicateMapKey)
    ));
    for value in [0, i64::MAX as u64, 1 << 63, u64::MAX] {
        let q = SetGameAttributesRequest {
            game_id: Some(value),
            ..Default::default()
        };
        let b = q.encode(Limits::default()).unwrap();
        assert_eq!(
            SetGameAttributesRequest::decode(&b, Limits::default())
                .unwrap()
                .game_id,
            Some(value)
        );
    }
}
#[test]
fn truncation_and_resource_limits_checked_without_required_field_policy() {
    let b = literal();
    for n in 1..26 {
        assert!(
            SetGameAttributesRequest::decode(&b[..n], Limits::default()).is_err(),
            "prefix {n}"
        );
    }
    for limits in [
        Limits {
            max_bytes: b.len() - 1,
            ..Limits::default()
        },
        Limits {
            max_collection: 0,
            ..Limits::default()
        },
        Limits {
            max_values: 2,
            ..Limits::default()
        },
        Limits {
            max_byte_string: 13,
            ..Limits::default()
        },
    ] {
        assert!(SetGameAttributesRequest::decode(&b, limits).is_err());
    }
    let mut b = literal();
    b[29] = 1; // GID string instead of integer.
    assert!(SetGameAttributesRequest::decode(&b, Limits::default()).is_err());
}
#[test]
fn independent_full_wire_partial_and_coalesced_frames() {
    let b = literal();
    let mut q = (b.len() as u32).to_be_bytes().to_vec();
    q.extend_from_slice(&[0, 0, 0, 4, 0, 7, 0, 0, 54, 0, 0, 0]);
    q.extend_from_slice(&b);
    let mut notification = q.clone();
    notification[9] = 80;
    notification[12] = 0;
    notification[13] = 0x40;
    for n in 0..q.len() {
        assert!(
            nfs_fire2::decode(&q[..n], nfs_fire2::Limits::default())
                .unwrap()
                .is_none()
        );
    }
    let mut joined = q.clone();
    joined.extend_from_slice(&notification);
    let d = nfs_fire2::decode(&joined, nfs_fire2::Limits::default())
        .unwrap()
        .unwrap();
    assert_eq!(d.consumed, q.len());
    assert_eq!(
        SetGameAttributesRequest::decode(d.frame.body, Limits::default())
            .unwrap()
            .game_id,
        Some(25)
    );
    let n = nfs_fire2::decode(&joined[d.consumed..], nfs_fire2::Limits::default())
        .unwrap()
        .unwrap();
    assert_eq!(
        (
            n.frame.fields.routing_b,
            n.frame.fields.category,
            n.frame.fields.correlation
        ),
        (80, 2, 0)
    );
    assert_eq!(
        NotifyGameAttribChange::decode(n.frame.body, Limits::default())
            .unwrap()
            .game_id,
        Some(25)
    );
}

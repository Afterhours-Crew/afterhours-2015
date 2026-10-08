use nfs_heat2::{Encoder, Kind, Limits, decode};
use nfs_protocol::{Error, stats::*};

fn hex(text: &str) -> Vec<u8> {
    let text: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

// Hand-derived from native tags/types and integer/string encoding.
// Synthetic key x; aggregate -1; enabled; two pairs (-2,3), (64,65).
// No saved catalog bytes, IDs, profile values or credentials are embedded.
const GOLDEN: &str =
    "af3a7405 0103 01 027800 867af90041 96e8670001 af3dac05 0000 02 4203 80018101 00";

fn example() -> KeyScopes<'static> {
    KeyScopes {
        key_scopes_map: Some(KeyScopesMap(vec![(
            b"x",
            KeyScopeItem {
                aggregate_key_value: Some(-1),
                enable_aggregation: Some(true),
                key_scope_values: Some(KeyScopeValues(vec![(-2, 3), (64, 65)])),
                ..Default::default()
            },
        )])),
        ..Default::default()
    }
}

#[test]
fn independent_nested_map_fixture_and_native_tags() {
    let bytes = hex(GOLDEN);
    assert_eq!(example().encode(Limits::default()).unwrap(), bytes);
    let decoded = KeyScopes::decode(&bytes, Limits::default()).unwrap();
    let (name, item) = &decoded.key_scopes_map.as_ref().unwrap().0[0];
    assert_eq!(*name, b"x");
    assert_eq!(item.aggregate_key_value, Some(-1));
    assert_eq!(item.enable_aggregation, Some(true));
    assert_eq!(
        item.key_scope_values.as_ref().unwrap().0,
        vec![(-2, 3), (64, 65)]
    );
    assert_eq!(decoded.unknown_field_count(), 0);
    assert_eq!(decoded.encode(Limits::default()).unwrap(), bytes);
    assert_eq!(
        KeyScopeItem::FIELD_TAGS,
        &[[0x86, 0x7a, 0xf9], [0x96, 0xe8, 0x67], [0xaf, 0x3d, 0xac]]
    );
    assert_eq!(
        KeyScopeItem::FIELD_KINDS,
        &[Kind::Integer, Kind::Integer, Kind::Map]
    );
    assert_eq!((COMPONENT, GET_KEY_SCOPES_MAP), (7, 15));
}

#[test]
fn request_is_strictly_empty_and_absence_differs_from_empty_map() {
    assert!(GetKeyScopesMapRequest::decode(&[], Limits::default()).is_ok());
    assert!(
        GetKeyScopesMapRequest
            .encode(Limits::default())
            .unwrap()
            .is_empty()
    );
    assert!(GetKeyScopesMapRequest::decode(&hex("af3a7405010300"), Limits::default()).is_err());
    assert!(GetKeyScopesMapRequest::decode(&[0xaf], Limits::default()).is_err());
    let absent = KeyScopes::decode(&[], Limits::default()).unwrap();
    assert!(absent.key_scopes_map.is_none());
    let bytes = hex("af3a7405010300");
    let empty = KeyScopes::decode(&bytes, Limits::default()).unwrap();
    assert!(empty.key_scopes_map.as_ref().unwrap().0.is_empty());
    assert_eq!(empty.encode(Limits::default()).unwrap(), bytes);
}

#[test]
fn signed_full_width_values_have_independent_wire_encodings() {
    for (wire, value) in [
        ("40", i64::MIN),
        ("bfffffffffffffffff01", i64::MAX),
        ("41", -1),
    ] {
        let bytes = hex(&format!("867af900{wire}"));
        let item = KeyScopeItem::decode(&bytes, Limits::default()).unwrap();
        assert_eq!(item.aggregate_key_value, Some(value));
        assert_eq!(item.encode(Limits::default()).unwrap(), bytes);
    }
    let item = KeyScopeItem {
        key_scope_values: Some(KeyScopeValues(vec![
            (i64::MIN, i64::MAX),
            (i64::MAX, i64::MIN),
        ])),
        ..Default::default()
    };
    let bytes = hex("af3dac05000002 40 bfffffffffffffffff01 bfffffffffffffffff01 40");
    assert_eq!(item.encode(Limits::default()).unwrap(), bytes);
    assert_eq!(
        KeyScopeItem::decode(&bytes, Limits::default())
            .unwrap()
            .key_scope_values
            .unwrap()
            .0,
        item.key_scope_values.unwrap().0
    );
}

#[test]
fn duplicate_fields_and_map_keys_are_rejected_on_read_and_write() {
    assert!(matches!(
        KeyScopeItem::decode(&hex("867af90000 867af90001"), Limits::default()),
        Err(Error::DuplicateTag(_))
    ));
    assert!(matches!(
        KeyScopeItem::decode(&hex("af3dac05000002 0102 0103"), Limits::default()),
        Err(Error::DuplicateMapKey)
    ));
    assert!(matches!(
        KeyScopes::decode(&hex("af3a7405010302 02780000 02780000"), Limits::default()),
        Err(Error::DuplicateMapKey)
    ));
    let mut value = example();
    value
        .key_scopes_map
        .as_mut()
        .unwrap()
        .0
        .push((b"x", KeyScopeItem::default()));
    assert!(matches!(
        value.encode(Limits::default()),
        Err(Error::DuplicateMapKey)
    ));
    let item = KeyScopeItem {
        key_scope_values: Some(KeyScopeValues(vec![(1, 2), (1, 3)])),
        ..Default::default()
    };
    assert!(matches!(
        item.encode(Limits::default()),
        Err(Error::DuplicateMapKey)
    ));
}

#[test]
fn wrong_kinds_and_invalid_booleans_are_rejected() {
    for bytes in [
        "867af9010100",
        "96e8670002",
        "96e8670041",
        "af3dac05010000",
        "af3dac05000100",
    ] {
        assert!(KeyScopeItem::decode(&hex(bytes), Limits::default()).is_err());
    }
    for bytes in ["af3a740000", "af3a7405000300", "af3a7405010000"] {
        assert!(KeyScopes::decode(&hex(bytes), Limits::default()).is_err());
    }
}

#[test]
fn every_truncated_nested_fixture_and_bad_count_fails() {
    let bytes = hex(GOLDEN);
    for end in 1..bytes.len() {
        assert!(
            KeyScopes::decode(&bytes[..end], Limits::default()).is_err(),
            "prefix {end}"
        );
    }
    let mut bad = bytes.clone();
    bad[6] = 2;
    assert!(KeyScopes::decode(&bad, Limits::default()).is_err());
    let mut bad = bytes;
    bad.pop();
    assert!(KeyScopes::decode(&bad, Limits::default()).is_err());
}

#[test]
fn resource_limits_cover_both_maps_depth_strings_values_and_bytes() {
    let bytes = hex(GOLDEN);
    let stats = decode(&bytes, Limits::default()).unwrap().stats();
    let tight = Limits {
        max_bytes: bytes.len(),
        max_depth: stats.max_depth,
        max_values: stats.values,
        max_collection: 2,
        max_byte_string: 2,
    };
    assert!(KeyScopes::decode(&bytes, tight).is_ok());
    assert_eq!(example().encode(tight).unwrap(), bytes);
    for limits in [
        Limits {
            max_bytes: bytes.len() - 1,
            ..tight
        },
        Limits {
            max_depth: stats.max_depth - 1,
            ..tight
        },
        Limits {
            max_values: stats.values - 1,
            ..tight
        },
        Limits {
            max_collection: 1,
            ..tight
        },
        Limits {
            max_byte_string: 1,
            ..tight
        },
    ] {
        assert!(KeyScopes::decode(&bytes, limits).is_err());
        assert!(example().encode(limits).is_err());
    }
    let mut two = example();
    two.key_scopes_map
        .as_mut()
        .unwrap()
        .0
        .push((b"y", KeyScopeItem::default()));
    assert!(
        two.encode(Limits {
            max_collection: 1,
            ..Limits::default()
        })
        .is_err()
    );
}

#[test]
fn unknown_fields_survive_at_root_and_nested_item() {
    let mut w = Encoder::new(Limits::default());
    w.string_struct_map([0xaf, 0x3a, 0x74], &[(b"x".as_slice(), ())], |w, _| {
        w.integer([0x86, 0x7a, 0xf9], -1)?;
        w.string([0xff, 0xff, 0xff], b"nested")
    })
    .unwrap();
    w.integer([0xff, 0xff, 0xff], 7).unwrap();
    let bytes = w.finish().unwrap();
    let value = KeyScopes::decode(&bytes, Limits::default()).unwrap();
    assert_eq!(value.unknown_field_count(), 2);
    assert_eq!(value.encode(Limits::default()).unwrap(), bytes);
    assert_eq!(
        value.key_scopes_map.as_ref().unwrap().0[0].1.unknown.len(),
        1
    );
    let text = format!("{value:?} {:?}", value.key_scopes_map.as_ref().unwrap());
    assert!(!text.contains("nested"));
}

#[test]
fn injected_known_or_duplicate_unknown_fields_fail() {
    let bytes = hex("867af90000");
    let d = decode(&bytes, Limits::default()).unwrap();
    let mut item = KeyScopeItem::default();
    item.unknown.push(d.fields().next().unwrap().unwrap());
    assert!(matches!(
        item.encode(Limits::default()),
        Err(Error::KnownTagInUnknown(_))
    ));
    let bytes = hex("ffffff0001");
    let d = decode(&bytes, Limits::default()).unwrap();
    let field = d.fields().next().unwrap().unwrap();
    let item = KeyScopeItem {
        unknown: vec![field, field],
        ..Default::default()
    };
    assert!(matches!(
        item.encode(Limits::default()),
        Err(Error::DuplicateTag(_))
    ));
}

#[test]
fn non_utf8_names_and_collection_order_are_preserved() {
    let v = KeyScopes {
        key_scopes_map: Some(KeyScopesMap(vec![
            (b"z", KeyScopeItem::default()),
            (
                &[255],
                KeyScopeItem {
                    key_scope_values: Some(KeyScopeValues(vec![(5, -2), (-3, 1)])),
                    ..Default::default()
                },
            ),
        ])),
        ..Default::default()
    };
    let bytes = v.encode(Limits::default()).unwrap();
    let d = KeyScopes::decode(&bytes, Limits::default()).unwrap();
    let entries = &d.key_scopes_map.as_ref().unwrap().0;
    assert_eq!(entries[0].0, b"z");
    assert_eq!(entries[1].0, &[255]);
    assert_eq!(
        entries[1].1.key_scope_values.as_ref().unwrap().0,
        vec![(5, -2), (-3, 1)]
    );
    assert_eq!(d.encode(Limits::default()).unwrap(), bytes);
}

#[test]
fn bounded_mutations_never_panic_and_successes_roundtrip() {
    let bytes = hex(GOLDEN);
    let limits = Limits {
        max_bytes: 64,
        max_depth: 5,
        max_values: 64,
        max_collection: 16,
        max_byte_string: 32,
    };
    for i in 0..bytes.len() {
        for value in [0, 1, 3, 5, 63, 64, 127, 128, 255] {
            let mut mutated = bytes.clone();
            mutated[i] = value;
            if let Ok(model) = KeyScopes::decode(&mutated, limits) {
                let encoded = model.encode(limits).unwrap();
                let reread = KeyScopes::decode(&encoded, limits).unwrap();
                assert_eq!(reread.encode(limits).unwrap(), encoded);
            }
        }
    }
}

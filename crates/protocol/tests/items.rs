// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use nfs_heat2::Limits;
use nfs_protocol::items::*;
fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}
// Hand-packed native ISNM:string + LICS:list<struct{LIC,SRC}> with synthetic strings.
fn fixture() -> Vec<u8> {
    hex("a73bad01027300b298f3040301b298c001026c00cf28c00102780000")
}
#[test]
fn independent_license_fixture_matches_native_shape() {
    let b = fixture();
    let m = EnsurePlayerInventoryRequest::decode(&b, Limits::default()).unwrap();
    assert_eq!(m.item_system_name, Some(b"s".as_slice()));
    let l = &m.available_licenses.as_ref().unwrap().0;
    assert_eq!(l.len(), 1);
    assert_eq!(
        (l[0].license, l[0].source),
        (Some(b"l".as_slice()), Some(b"x".as_slice()))
    );
    assert_eq!(m.unknown_field_count(), 0);
    assert_eq!(m.encode(Limits::default()).unwrap(), b);
    assert_eq!((COMPONENT, ENSURE_PLAYER_INVENTORY), (2052, 19));
}
#[test]
fn empty_absent_and_duplicate_license_rows_are_structural_not_award_policy() {
    let absent = EnsurePlayerInventoryRequest::decode(&[], Limits::default()).unwrap();
    assert!(absent.available_licenses.is_none());
    let m = EnsurePlayerInventoryRequest {
        available_licenses: Some(LicenseSources(vec![])),
        ..Default::default()
    };
    assert_eq!(m.encode(Limits::default()).unwrap(), hex("b298f3040300"));
    let m = EnsurePlayerInventoryRequest {
        available_licenses: Some(LicenseSources(vec![
            LicenseSourceData::default(),
            LicenseSourceData::default(),
        ])),
        ..Default::default()
    };
    let b = m.encode(Limits::default()).unwrap();
    assert_eq!(
        EnsurePlayerInventoryRequest::decode(&b, Limits::default())
            .unwrap()
            .available_licenses
            .unwrap()
            .0
            .len(),
        2
    );
}
#[test]
fn nested_unknowns_survive_and_wrong_shapes_fail() {
    let b = hex("b298f3040301ffffff000100");
    let m = EnsurePlayerInventoryRequest::decode(&b, Limits::default()).unwrap();
    assert_eq!(m.unknown_field_count(), 1);
    assert_eq!(m.encode(Limits::default()).unwrap(), b);
    for b in [
        hex("a73bad0000"),
        hex("b298f304000100"),
        hex("b298f3040301b298c0000000"),
        hex("a73bad010100a73bad010100"),
    ] {
        assert!(EnsurePlayerInventoryRequest::decode(&b, Limits::default()).is_err());
    }
}
#[test]
fn truncation_depth_string_and_collection_budgets_fail() {
    let b = fixture();
    let m = EnsurePlayerInventoryRequest::decode(&b, Limits::default()).unwrap();
    for cut in 8..b.len() {
        assert!(EnsurePlayerInventoryRequest::decode(&b[..cut], Limits::default()).is_err());
    }
    for limits in [
        Limits {
            max_collection: 0,
            ..Default::default()
        },
        Limits {
            max_depth: 1,
            ..Default::default()
        },
        Limits {
            max_values: 3,
            ..Default::default()
        },
        Limits {
            max_byte_string: 1,
            ..Default::default()
        },
        Limits {
            max_bytes: b.len() - 1,
            ..Default::default()
        },
    ] {
        assert!(EnsurePlayerInventoryRequest::decode(&b, limits).is_err());
        assert!(m.encode(limits).is_err());
    }
}

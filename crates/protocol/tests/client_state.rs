// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Hand-packed tags and integer encoding; no private account data.
use nfs_heat2::{Encoder, Limits};
use nfs_protocol::util::ClientState;

const MENU: &[u8] = &[0xb6, 0xf9, 0x25, 0, 1, 0xcf, 0x48, 0x74, 0, 0];

#[test]
fn independent_menu_fixture_and_absence() {
    let model = ClientState::decode(MENU, Limits::default()).unwrap();
    assert_eq!((model.mode, model.status), (Some(1), Some(0)));
    assert_eq!(model.unknown_field_count(), 0);
    assert_eq!(model.encode(Limits::default()).unwrap(), MENU);
    let absent = ClientState::decode(&[], Limits::default()).unwrap();
    assert_eq!((absent.mode, absent.status), (None, None));
}

#[test]
fn enum_storage_accepts_unknown_i32_values_but_rejects_overflow() {
    for value in [i32::MIN, -1, 4, i32::MAX] {
        let model = ClientState {
            mode: Some(value),
            status: Some(value),
            ..Default::default()
        };
        let bytes = model.encode(Limits::default()).unwrap();
        let decoded = ClientState::decode(&bytes, Limits::default()).unwrap();
        assert_eq!((decoded.mode, decoded.status), (Some(value), Some(value)));
    }
    for value in [i64::from(i32::MIN) - 1, i64::from(i32::MAX) + 1] {
        for tag in ClientState::FIELD_TAGS {
            let mut writer = Encoder::new(Limits::default());
            writer.integer(*tag, value).unwrap();
            assert!(ClientState::decode(&writer.finish().unwrap(), Limits::default()).is_err());
        }
    }
}

#[test]
fn unknown_fields_survive_and_duplicate_tags_fail() {
    let mut bytes = MENU.to_vec();
    bytes.extend_from_slice(&[0xf3, 0xff, 0xff, 0, 7]);
    let decoded = ClientState::decode(&bytes, Limits::default()).unwrap();
    assert_eq!(decoded.unknown_field_count(), 1);
    assert_eq!(decoded.encode(Limits::default()).unwrap(), bytes);
    bytes.extend_from_slice(&MENU[..5]);
    assert!(ClientState::decode(&bytes, Limits::default()).is_err());
}

#[test]
fn malformed_types_truncation_and_resource_bounds_fail() {
    for cut in [1, 2, 3, 4, 6, 7, 8, 9] {
        assert!(ClientState::decode(&MENU[..cut], Limits::default()).is_err());
    }
    assert!(ClientState::decode(&[0xb6, 0xf9, 0x25, 1, 1, 0], Limits::default()).is_err());
    let limits = Limits {
        max_values: 1,
        ..Limits::default()
    };
    assert!(ClientState::decode(MENU, limits).is_err());
    assert!(
        ClientState::decode(
            MENU,
            Limits {
                max_bytes: 9,
                ..Limits::default()
            }
        )
        .is_err()
    );
    assert!(
        ClientState::decode(MENU, Limits::default())
            .unwrap()
            .encode(limits)
            .is_err()
    );
}

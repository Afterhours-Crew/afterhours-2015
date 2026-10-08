//! fixtures are hand-packed from the wire shapes; keys/values are
//! synthetic. They contain no owner's settings or captured profile bytes.
use nfs_heat2::Limits;
use nfs_protocol::{
    autolog::GetTimeLimitedFeaturesRequest,
    util::{ConfigEntries, UserSettingsLoadAllResponse},
};

// SMAP, map<string,string>, two ordered pairs: b->2 then a->1.
const SETTINGS: &[u8] = &[
    0xce, 0xd8, 0x70, 5, 1, 1, 2, 2, b'b', 0, 2, b'2', 0, 2, b'a', 0, 2, b'1', 0,
];
const FEATURE: &[u8] = &[0x92, 0x59, 0x21, 1, 1, 0];

#[test]
fn independent_settings_fixture_preserves_order_and_absence() {
    let limits = Limits::default();
    let settings = UserSettingsLoadAllResponse::decode(SETTINGS, limits).unwrap();
    let map = &settings.data_map.as_ref().unwrap().0;
    assert_eq!(map, &vec![(b"b".as_slice(), b"2".as_slice()), (b"a", b"1")]);
    assert_eq!(settings.unknown_field_count(), 0);
    assert_eq!(settings.encode(limits).unwrap(), SETTINGS);
    let empty = UserSettingsLoadAllResponse {
        data_map: Some(ConfigEntries(vec![])),
        ..Default::default()
    };
    assert_eq!(
        empty.encode(limits).unwrap(),
        [0xce, 0xd8, 0x70, 5, 1, 1, 0]
    );
    assert!(
        UserSettingsLoadAllResponse::decode(&[], limits)
            .unwrap()
            .data_map
            .is_none()
    );
    let independent = UserSettingsLoadAllResponse {
        data_map: Some(ConfigEntries(vec![(b"z", b"other")])),
        ..Default::default()
    };
    assert_ne!(independent.encode(limits).unwrap(), SETTINGS);
    assert_eq!(settings.encode(limits).unwrap(), SETTINGS);
}

#[test]
fn feature_empty_string_is_present_and_matches_observed_shape() {
    let limits = Limits::default();
    let request = GetTimeLimitedFeaturesRequest::decode(FEATURE, limits).unwrap();
    assert_eq!(request.deda, Some(b"".as_slice()));
    assert_eq!(request.unknown_field_count(), 0);
    assert_eq!(request.encode(limits).unwrap(), FEATURE);
    assert!(
        GetTimeLimitedFeaturesRequest::decode(&[], limits)
            .unwrap()
            .deda
            .is_none()
    );
    // The codec can preserve other strings; a future handler must independently
    // gate the observed empty request before sending its observed empty reply.
    let other = GetTimeLimitedFeaturesRequest {
        deda: Some(b"opaque"),
        ..Default::default()
    };
    let wire = other.encode(limits).unwrap();
    assert_eq!(
        GetTimeLimitedFeaturesRequest::decode(&wire, limits)
            .unwrap()
            .deda,
        Some(b"opaque".as_slice())
    );
}

#[test]
fn partial_duplicate_wrong_type_and_unknown_fields_are_checked() {
    let limits = Limits::default();
    for end in 1..SETTINGS.len() {
        assert!(UserSettingsLoadAllResponse::decode(&SETTINGS[..end], limits).is_err());
    }
    for end in 1..FEATURE.len() {
        assert!(GetTimeLimitedFeaturesRequest::decode(&FEATURE[..end], limits).is_err());
    }
    assert!(UserSettingsLoadAllResponse::decode(&[SETTINGS, SETTINGS].concat(), limits).is_err());
    assert!(GetTimeLimitedFeaturesRequest::decode(&[FEATURE, FEATURE].concat(), limits).is_err());
    for malformed in [
        vec![0xce, 0xd8, 0x70, 0, 0],
        vec![0xce, 0xd8, 0x70, 5, 0, 1, 0],
        vec![0xce, 0xd8, 0x70, 5, 1, 0, 0],
        vec![
            0xce, 0xd8, 0x70, 5, 1, 1, 2, 2, b'a', 0, 1, 0, 2, b'a', 0, 1, 0,
        ],
    ] {
        assert!(UserSettingsLoadAllResponse::decode(&malformed, limits).is_err());
    }
    assert!(GetTimeLimitedFeaturesRequest::decode(&[0x92, 0x59, 0x21, 0, 0], limits).is_err());
    let unknown = [0xff, 0xff, 0xff, 0, 1];
    let extended = [SETTINGS, &unknown].concat();
    let model = UserSettingsLoadAllResponse::decode(&extended, limits).unwrap();
    assert_eq!(model.unknown_field_count(), 1);
    assert_eq!(model.encode(limits).unwrap(), extended);
    let extended = [FEATURE, &unknown].concat();
    let model = GetTimeLimitedFeaturesRequest::decode(&extended, limits).unwrap();
    assert_eq!(model.unknown_field_count(), 1);
    assert_eq!(model.encode(limits).unwrap(), extended);
}

#[test]
fn bounded_containers_and_redacted_debug() {
    let limits = Limits::default();
    let settings = UserSettingsLoadAllResponse::decode(SETTINGS, limits).unwrap();
    for restricted in [
        Limits {
            max_collection: 1,
            ..limits
        },
        Limits {
            max_values: 2,
            ..limits
        },
        Limits {
            max_bytes: SETTINGS.len() - 1,
            ..limits
        },
        Limits {
            max_byte_string: 0,
            ..limits
        },
    ] {
        assert!(UserSettingsLoadAllResponse::decode(SETTINGS, restricted).is_err());
        assert!(settings.encode(restricted).is_err());
    }
    let duplicate = UserSettingsLoadAllResponse {
        data_map: Some(ConfigEntries(vec![(b"a", b"1"), (b"a", b"2")])),
        ..Default::default()
    };
    assert!(duplicate.encode(limits).is_err());
    let secret = GetTimeLimitedFeaturesRequest {
        deda: Some(b"synthetic-secret"),
        ..Default::default()
    };
    assert!(!format!("{secret:?}").contains("synthetic-secret"));
    assert!(
        secret
            .encode(Limits {
                max_byte_string: 2,
                ..limits
            })
            .is_err()
    );
}

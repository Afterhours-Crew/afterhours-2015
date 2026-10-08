use nfs_protocol::kickback::*;
fn hex(s: &str) -> Vec<u8> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}
// native tags/types and grammar; hand-assembled invented IDs/name.
fn golden() -> Vec<u8> {
    hex("a64cc0 04 03 01 a64000 00 2a c29900 00 07 ca58ee 01 04 696d6700 00")
}
#[test]
fn independent_native_gallery_bytes_and_explicit_empty_are_distinct_from_absent() {
    let b = golden();
    let m = GetSnapshotGalleryTilesResponse::decode(&b, Default::default()).unwrap();
    assert_eq!(m.unknown_field_count(), 0);
    assert_eq!(m.encode(Default::default()).unwrap(), b);
    let v = &m.tile_identifiers.unwrap().0[0];
    assert_eq!(
        (v.screenshot_id, v.persona_id, v.record_name),
        (Some(42), Some(7), Some(b"img".as_slice()))
    );
    let empty = hex("a64cc0040300");
    let e = GetSnapshotGalleryTilesResponse::decode(&empty, Default::default()).unwrap();
    assert!(e.tile_identifiers.as_ref().unwrap().0.is_empty());
    assert_eq!(e.encode(Default::default()).unwrap(), empty);
    assert!(
        GetSnapshotGalleryTilesResponse::decode(&[], Default::default())
            .unwrap()
            .tile_identifiers
            .is_none()
    );
    assert!(
        GetSnapshotGalleryTilesRequest::decode(&[], Default::default())
            .unwrap()
            .encode(Default::default())
            .unwrap()
            .is_empty()
    );
    assert!(
        GetSnapshotGalleryTilesRequest::decode(&hex("a640000000"), Default::default()).is_err()
    );
}
#[test]
fn all_u64_bits_and_signed_persona_are_preserved_with_unknown_nested_fields() {
    for (id, pid) in [(0, i64::MIN), (u64::MAX, -1), (1 << 63, i64::MAX)] {
        let m = GetSnapshotGalleryTilesResponse {
            tile_identifiers: Some(SnapshotGalleryTiles(vec![SnapshotGalleryTileIdentifier {
                screenshot_id: Some(id),
                persona_id: Some(pid),
                record_name: Some(b""),
                ..Default::default()
            }])),
            ..Default::default()
        };
        let b = m.encode(Default::default()).unwrap();
        let r = GetSnapshotGalleryTilesResponse::decode(&b, Default::default()).unwrap();
        let v = &r.tile_identifiers.unwrap().0[0];
        assert_eq!((v.screenshot_id, v.persona_id), (Some(id), Some(pid)));
    }
    let mut b = golden();
    b.splice(b.len() - 1..b.len() - 1, [255, 255, 255, 0, 1]);
    let m = GetSnapshotGalleryTilesResponse::decode(&b, Default::default()).unwrap();
    assert_eq!(m.unknown_field_count(), 1);
    assert_eq!(m.encode(Default::default()).unwrap(), b);
}
#[test]
fn duplicate_wrong_type_truncated_nested_values_and_resource_limits_are_rejected() {
    let g = golden();
    let mut duplicate = g.clone();
    duplicate.extend_from_slice(&g);
    for b in [
        duplicate,
        hex("a64cc00400012a"),
        g[..g.len() - 1].to_vec(),
        hex("a64cc0040301a64000002aa64000002a00"),
    ] {
        assert!(GetSnapshotGalleryTilesResponse::decode(&b, Default::default()).is_err());
    }
    for lim in [
        nfs_heat2::Limits {
            max_bytes: g.len() - 1,
            ..Default::default()
        },
        nfs_heat2::Limits {
            max_depth: 1,
            ..Default::default()
        },
        nfs_heat2::Limits {
            max_collection: 0,
            ..Default::default()
        },
        nfs_heat2::Limits {
            max_values: 3,
            ..Default::default()
        },
        nfs_heat2::Limits {
            max_byte_string: 3,
            ..Default::default()
        },
    ] {
        assert!(GetSnapshotGalleryTilesResponse::decode(&g, lim).is_err());
        let m = GetSnapshotGalleryTilesResponse::decode(&g, Default::default()).unwrap();
        assert!(m.encode(lim).is_err());
    }
}
#[test]
fn every_frame_split_concatenation_and_bounded_body_mutations() {
    let b = golden();
    let w = nfs_fire2::encode(
        nfs_fire2::Frame {
            fields: nfs_fire2::Fields {
                routing_a: 2053,
                routing_b: 22,
                category: 1,
                ..Default::default()
            },
            metadata: &[],
            body: &b,
        },
        Default::default(),
    )
    .unwrap();
    for n in 0..w.len() {
        assert!(
            nfs_fire2::decode(&w[..n], Default::default())
                .unwrap()
                .is_none()
        );
    }
    let both = [w.as_slice(), w.as_slice()].concat();
    let d = nfs_fire2::decode(&both, Default::default())
        .unwrap()
        .unwrap();
    assert_eq!(d.consumed, w.len());
    assert_eq!(
        nfs_fire2::decode(&both[d.consumed..], Default::default())
            .unwrap()
            .unwrap()
            .consumed,
        w.len()
    );
    for i in 0..b.len() {
        for mask in [1, 0x80, 0xff] {
            let mut v = b.clone();
            v[i] ^= mask;
            if let Ok(m) = GetSnapshotGalleryTilesResponse::decode(&v, Default::default()) {
                let e = m.encode(Default::default()).unwrap();
                assert!(GetSnapshotGalleryTilesResponse::decode(&e, Default::default()).is_ok());
            }
        }
    }
}

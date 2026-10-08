use nfs_protocol::autolog::FriendsRecommendationsRequest;

#[test]
fn native_blid_int64_matches_independent_synthetic_bytes() {
    // native BLID/int64 descriptor, integer encoding; invented ID42.
    let golden = [0x8a, 0xca, 0x64, 0, 42];
    let m = FriendsRecommendationsRequest::decode(&golden, Default::default()).unwrap();
    assert_eq!(m.blaze_id, Some(42));
    assert_eq!(m.unknown_field_count(), 0);
    assert_eq!(m.encode(Default::default()).unwrap(), golden);
    for id in [i64::MIN, -1, 0, i64::MAX] {
        let wire = FriendsRecommendationsRequest {
            blaze_id: Some(id),
            ..Default::default()
        }
        .encode(Default::default())
        .unwrap();
        assert_eq!(
            FriendsRecommendationsRequest::decode(&wire, Default::default())
                .unwrap()
                .blaze_id,
            Some(id)
        );
    }
}

#[test]
fn absent_unknown_duplicate_type_and_resource_boundaries() {
    assert_eq!(
        FriendsRecommendationsRequest::decode(&[], Default::default())
            .unwrap()
            .blaze_id,
        None
    );
    let unknown = [0x8a, 0xca, 0x64, 0, 42, 0xff, 0xff, 0xff, 0, 1];
    let m = FriendsRecommendationsRequest::decode(&unknown, Default::default()).unwrap();
    assert_eq!(m.unknown_field_count(), 1);
    assert_eq!(m.encode(Default::default()).unwrap(), unknown);
    for bad in [
        vec![0x8a, 0xca, 0x64, 0],
        vec![0x8a, 0xca, 0x64, 1, 1, 0],
        vec![0x8a, 0xca, 0x64, 0, 1, 0x8a, 0xca, 0x64, 0, 2],
    ] {
        assert!(FriendsRecommendationsRequest::decode(&bad, Default::default()).is_err());
    }
    for max_bytes in 0..5 {
        assert!(
            FriendsRecommendationsRequest::decode(
                &[0x8a, 0xca, 0x64, 0, 42],
                nfs_heat2::Limits {
                    max_bytes,
                    ..Default::default()
                }
            )
            .is_err()
        );
    }
}

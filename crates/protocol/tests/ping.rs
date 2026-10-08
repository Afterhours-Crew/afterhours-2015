// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use nfs_heat2::Limits;
use nfs_protocol::util::PingResponse;

#[test]
fn timestamp_matches_independently_packed_heat2_boundaries() {
    for (value, wire) in [
        (0, vec![0xcf, 0x4a, 0x6d, 0, 0]),
        (64, vec![0xcf, 0x4a, 0x6d, 0, 0x80, 1]),
        (
            u32::MAX,
            vec![0xcf, 0x4a, 0x6d, 0, 0xbf, 0xff, 0xff, 0xff, 0x1f],
        ),
    ] {
        let model = PingResponse {
            server_time: Some(value),
            ..Default::default()
        };
        assert_eq!(model.encode(Limits::default()).unwrap(), wire);
        assert_eq!(
            PingResponse::decode(&wire, Limits::default())
                .unwrap()
                .server_time,
            Some(value)
        );
        for end in 1..wire.len() {
            assert!(PingResponse::decode(&wire[..end], Limits::default()).is_err());
        }
    }
}

#[test]
fn timestamp_absence_unknowns_types_and_ranges_are_distinct() {
    assert_eq!(
        PingResponse::decode(&[], Limits::default())
            .unwrap()
            .server_time,
        None
    );
    let unknown = [0xff, 0xff, 0xff, 0, 1];
    let model = PingResponse::decode(&unknown, Limits::default()).unwrap();
    assert_eq!(model.unknown_field_count(), 1);
    assert_eq!(model.encode(Limits::default()).unwrap(), unknown);
    for invalid in [
        vec![0xcf, 0x4a, 0x6d, 0, 0x41],
        vec![0xcf, 0x4a, 0x6d, 0, 0x80, 0x80, 0x80, 0x80, 0x20],
        vec![0xcf, 0x4a, 0x6d, 1, 1, 0],
        vec![0xcf, 0x4a, 0x6d, 0, 0, 0xcf, 0x4a, 0x6d, 0, 1],
    ] {
        assert!(PingResponse::decode(&invalid, Limits::default()).is_err());
    }
}

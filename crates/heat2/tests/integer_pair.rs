// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use nfs_heat2::{Encoder, ErrorKind, Limits, Value, decode};

const TAG: [u8; 3] = [0x97, 0x4e, 0x70];

#[test]
fn independent_pair_bytes_include_negative_zero_and_full_signed_width() {
    for (values, body) in [
        ([0, 65535], vec![0, 0xbf, 0xff, 7]),
        ([-1, 64], vec![0x41, 0x80, 1]),
        (
            [i64::MIN, i64::MAX],
            vec![
                0x40, 0xbf, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 1,
            ],
        ),
    ] {
        let golden = [TAG.as_slice(), &[8], &body].concat();
        let mut writer = Encoder::new(Limits::default());
        writer.integer_pair(TAG, values).unwrap();
        assert_eq!(writer.finish().unwrap(), golden);
        let model = decode(&golden, Limits::default()).unwrap();
        assert_eq!(model.stats().values, 1);
        assert_eq!(
            model.fields().next().unwrap().unwrap().item().value(),
            Value::IntegerPair(values)
        );
    }
}

#[test]
fn exact_byte_and_value_limits_poison_partial_writes() {
    let exact = Limits {
        max_bytes: 8,
        max_values: 1,
        max_depth: 0,
        ..Limits::default()
    };
    let mut writer = Encoder::new(exact);
    writer.integer_pair(TAG, [0, 65535]).unwrap();
    assert_eq!(writer.finish().unwrap().len(), 8);
    for limits in [
        Limits {
            max_bytes: 7,
            ..exact
        },
        Limits {
            max_values: 0,
            ..exact
        },
    ] {
        let mut writer = Encoder::new(limits);
        let error = writer.integer_pair(TAG, [0, 65535]).unwrap_err();
        assert_eq!(writer.integer(TAG, 0).unwrap_err(), error);
        assert_eq!(writer.finish().unwrap_err(), error);
    }
    let mut writer = Encoder::new(exact);
    let error = writer.integer_pair([0, 1, 2], [0, 0]).unwrap_err();
    assert_eq!(error.kind, ErrorKind::InvalidTag);
    assert_eq!(writer.finish().unwrap_err(), error);
}

#[test]
fn nested_pair_depth_and_values_count_the_container_once() {
    let exact = Limits {
        max_bytes: 11,
        max_values: 2,
        max_depth: 1,
        ..Limits::default()
    };
    let mut writer = Encoder::new(exact);
    writer
        .structure([0xba, 0x1b, 0x65], |writer| {
            writer.integer_pair(TAG, [0, 0])
        })
        .unwrap();
    let bytes = writer.finish().unwrap();
    assert_eq!(bytes.len(), 11);
    assert_eq!(decode(&bytes, exact).unwrap().stats().values, 2);
    for limits in [
        Limits {
            max_depth: 0,
            ..exact
        },
        Limits {
            max_values: 1,
            ..exact
        },
        Limits {
            max_bytes: 10,
            ..exact
        },
    ] {
        let mut writer = Encoder::new(limits);
        let error = writer
            .structure([0xba, 0x1b, 0x65], |writer| {
                writer.integer_pair(TAG, [0, 0])
            })
            .unwrap_err();
        assert_eq!(writer.finish().unwrap_err(), error);
        assert!(decode(&bytes, limits).is_err());
    }
}

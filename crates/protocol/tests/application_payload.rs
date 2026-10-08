// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Independent literals and alignment/bounds/representation cases.
use nfs_protocol::world::{BitSpan, payload::Route};

const REQUEST: [u8; 9] = [0x91, 0xff, 0x80, 0, 0, 0, 0, 0, 0];
const REPLY: [u8; 17] = [
    0x99, 0xff, 0x80, 0, 0, 0, 0, 0, 0x02, 0, 0, 0, 0, 0, 0, 0, 0,
];
const OPTIONAL: [u8; 8] = [0x6a, 0xf3, 0x79, 0xfc, 0, 0, 0x05, 0xa3];
fn span(bytes: &[u8], bits: usize) -> BitSpan<'_> {
    BitSpan::new(bytes, 0, bits).unwrap()
}
fn shifted(bytes: &[u8], offset: usize) -> Vec<u8> {
    let mut result = vec![0; bytes.len() + 2];
    for bit in 0..bytes.len() * 8 {
        result[(bit + offset) / 8] |=
            ((bytes[bit / 8] >> (7 - bit % 8)) & 1) << (7 - (bit + offset) % 8);
    }
    result
}
#[test]
fn fixed_controls_retain_exact_timestamp_words() {
    for (bytes, bits, subtype, expected) in [
        (&REQUEST[..], 69, 2, [Some(0x3ff0000000000000), None]),
        (
            &REPLY[..],
            133,
            3,
            [Some(0x3ff0000000000000), Some(0x4000000000000000)],
        ),
    ] {
        let Route::Control {
            subtype: actual,
            timestamps,
            body,
        } = Route::decode(span(bytes, bits)).unwrap()
        else {
            panic!()
        };
        assert_eq!(actual, subtype);
        assert_eq!(timestamps, expected);
        assert!(body.is_empty());
        assert_eq!(body.start(), bits);
    }
}
#[test]
fn optional_parameters_stop_at_bit_53() {
    let Route::Queued { advertised, body } = Route::decode(span(&OPTIONAL, 64)).unwrap() else {
        panic!()
    };
    let advertised = advertised.unwrap();
    assert_eq!(
        (advertised.value(), advertised.float_bits()),
        (0xabcde, 0x3f800000)
    );
    assert_eq!(
        (body.start(), body.len(), body.read_u32(0, 11).unwrap()),
        (53, 11, 0x5a3)
    );
    let Route::Queued { advertised, body } = Route::decode(span(&[0x2a], 8)).unwrap() else {
        panic!()
    };
    assert!(advertised.is_none());
    assert_eq!(
        (body.start(), body.len(), body.read_u32(0, 6).unwrap()),
        (2, 6, 42)
    );
}
#[test]
fn every_prefix_truncation_and_all_bit_alignments_are_bounded() {
    for (bytes, needed) in [
        (&REQUEST[..], 69),
        (&REPLY[..], 133),
        (&OPTIONAL[..], 53),
        (&[0][..], 2),
        (&[0xf8][..], 5),
    ] {
        for offset in 0..8 {
            let storage = shifted(bytes, offset);
            for length in 0..needed {
                assert!(Route::decode(BitSpan::new(&storage, offset, length).unwrap()).is_err());
            }
            let route = Route::decode(BitSpan::new(&storage, offset, needed).unwrap()).unwrap();
            let body = match route {
                Route::Queued { body, .. } | Route::Control { body, .. } => body,
            };
            assert_eq!(body.start(), offset + needed);
            assert!(body.is_empty());
            assert!(body.read_u32(0, 1).is_err());
            let normalized = Route::decode(span(bytes, needed)).unwrap();
            match (route, normalized) {
                (Route::Queued { advertised: a, .. }, Route::Queued { advertised: b, .. }) => {
                    assert_eq!(a, b)
                }
                (
                    Route::Control {
                        subtype: a,
                        timestamps: x,
                        ..
                    },
                    Route::Control {
                        subtype: b,
                        timestamps: y,
                        ..
                    },
                ) => assert_eq!((a, x), (b, y)),
                _ => panic!(),
            }
        }
    }
}
#[test]
fn all_unknown_controls_and_known_trailing_bits_remain_opaque() {
    for subtype in 0..16 {
        if matches!(subtype, 2 | 3) {
            continue;
        }
        let bytes = [0x80 | subtype << 3 | 5];
        let Route::Control {
            subtype: actual,
            timestamps,
            body,
        } = Route::decode(span(&bytes, 8)).unwrap()
        else {
            panic!()
        };
        assert_eq!(actual, subtype);
        assert_eq!(timestamps, [None, None]);
        assert_eq!((body.len(), body.read_u32(0, 3).unwrap()), (3, 5));
    }
    let mut request = REQUEST;
    request[8] |= 7;
    let Route::Control { body, .. } = Route::decode(span(&request, 72)).unwrap() else {
        panic!()
    };
    assert_eq!(body.read_u32(0, 3).unwrap(), 7);
}
#[test]
fn special_float_patterns_and_timestamp_words_are_not_normalized() {
    for value in [0, 1, 0x7f800000, 0x7fc12345, 0x7fffffff] {
        let packed = (1_u64 << 62) | (u64::from(value) << 11);
        let bytes = packed.to_be_bytes();
        let Route::Queued { advertised, .. } = Route::decode(span(&bytes, 53)).unwrap() else {
            panic!()
        };
        assert_eq!(advertised.unwrap().float_bits(), value);
    }
    for value in [
        0,
        1,
        0x8000000000000000,
        0x7ff0000000000000,
        0x7ff8123456789abc,
        u64::MAX,
    ] {
        let shifted_word = shifted(&value.to_be_bytes(), 5);
        let mut bytes = shifted_word;
        bytes[0] |= 0x90;
        let Route::Control { timestamps, .. } = Route::decode(span(&bytes, 69)).unwrap() else {
            panic!()
        };
        assert_eq!(timestamps, [Some(value), None]);
    }
}
#[test]
fn arbitrary_views_remain_bounded_and_debug_omits_values() {
    let mut seed = 0x1234abcd_u32;
    for round in 0..10000 {
        let mut bytes = [0; 25];
        for byte in &mut bytes {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            *byte = seed as u8;
        }
        let start = round % 8;
        let len = round % 190;
        if let Ok(route) = Route::decode(BitSpan::new(&bytes, start, len).unwrap()) {
            let body = match route {
                Route::Queued { body, .. } | Route::Control { body, .. } => body,
            };
            assert_eq!(body.start() + body.len(), start + len);
            assert!(body.read_u32(body.len(), 1).is_err());
        }
    }
    let route = Route::decode(span(&OPTIONAL, 64)).unwrap();
    let Route::Queued { advertised, .. } = route else {
        panic!()
    };
    let text = format!(
        "{route:?} {advertised:?} {:?}",
        Route::decode(span(&REQUEST, 69)).unwrap()
    );
    for private in ["703710", "1065353216", "4607182418800017408", "abcde"] {
        assert!(!text.contains(private));
    }
}

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use nfs_heat2::{Encoder, ErrorKind, Limits, Value, decode};

const TAG: [u8; 3] = [0x8e, 0x7a, 0x64];

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn independently_derived_integer_triple_vectors() {
    // integer writer/reader and type-9 callback; no codec generated these bytes.
    for (values, expected) in [
        ([0, 0, 0], "8e7a6409000000"),
        ([1, 2, 3], "8e7a6409010203"),
        ([63, 64, -64], "8e7a64093f8001c001"),
        ([65535, 64, i64::MIN], "8e7a6409bfff07800140"),
        ([-1, i64::MAX, i64::MIN], "8e7a640941bfffffffffffffffff0140"),
    ] {
        let bytes = hex(expected);
        let limits = Limits {
            max_bytes: bytes.len(),
            max_values: 1,
            max_depth: 0,
            max_collection: 0,
            max_byte_string: 0,
        };
        let mut writer = Encoder::new(limits);
        writer.integer_triple(TAG, values).unwrap();
        assert_eq!(writer.finish().unwrap(), bytes);
        let document = decode(&bytes, limits).unwrap();
        assert_eq!(document.stats().values, 1);
        assert_eq!(
            document.fields().next().unwrap().unwrap().item().value(),
            Value::IntegerTriple(values)
        );
        for cut in 1..bytes.len() {
            assert!(decode(&bytes[..cut], limits).is_err());
        }
    }
}

#[test]
fn integer_triple_writer_obeys_limits_and_stays_failed() {
    for (limits, kind) in [
        (
            Limits {
                max_values: 0,
                ..Limits::default()
            },
            ErrorKind::ValueLimit,
        ),
        (
            Limits {
                max_bytes: 6,
                ..Limits::default()
            },
            ErrorKind::InputLimit,
        ),
    ] {
        let mut writer = Encoder::new(limits);
        let error = writer.integer_triple(TAG, [0; 3]).unwrap_err();
        assert_eq!(error.kind, kind);
        assert_eq!(writer.integer(TAG, 1).unwrap_err(), error);
        assert_eq!(writer.finish().unwrap_err(), error);
    }
    let mut writer = Encoder::new(Limits::default());
    assert_eq!(
        writer.integer_triple([0; 3], [0; 3]).unwrap_err().kind,
        ErrorKind::InvalidTag
    );
    assert!(writer.finish().is_err());

    let mut writer = Encoder::new(Limits {
        max_depth: 0,
        ..Limits::default()
    });
    assert_eq!(
        writer
            .structure(TAG, |w| w.integer_triple(TAG, [0; 3]))
            .unwrap_err()
            .kind,
        ErrorKind::DepthLimit
    );
    assert!(writer.finish().is_err());
}

#[test]
fn triple_malformed_values_and_nested_accounting() {
    for payload in [
        "",
        "00",
        "0000",
        "000080",
        "808080808080808080020000",
        "00008080808080808080808000",
    ] {
        assert!(decode(&hex(&format!("8e7a6409{payload}")), Limits::default()).is_err());
    }
    let limits = Limits {
        max_values: 2,
        max_depth: 1,
        ..Limits::default()
    };
    let mut writer = Encoder::new(limits);
    writer
        .structure(TAG, |w| w.integer_triple(TAG, [1, 2, 3]))
        .unwrap();
    let bytes = writer.finish().unwrap();
    assert_eq!(bytes, hex("8e7a64038e7a640901020300"));
    assert_eq!(decode(&bytes, limits).unwrap().stats().values, 2);
    assert!(
        decode(
            &bytes,
            Limits {
                max_values: 1,
                ..limits
            }
        )
        .is_err()
    );
}

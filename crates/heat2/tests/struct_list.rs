// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use nfs_heat2::{Encoder, Error, ErrorKind, Limits, Value, decode};

const LIST: [u8; 3] = [0x86, 0xcc, 0xf4];
const SCALAR: [u8; 3] = [0xd3, 0x9c, 0x25];

#[test]
fn independently_packed_struct_list_preserves_element_order_and_empty_elements() {
    // ALST list<struct> count 3: {TYPE=2}, {}, {TYPE=1}; each 00 terminates.
    let golden = [
        0x86, 0xcc, 0xf4, 4, 3, 3, 0xd3, 0x9c, 0x25, 0, 2, 0, 0, 0xd3, 0x9c, 0x25, 0, 1, 0,
    ];
    let mut w = Encoder::new(Limits::default());
    w.struct_list(LIST, &[Some(2), None, Some(1)], |w, value| {
        if let Some(value) = value {
            w.integer(SCALAR, *value)?;
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(w.finish().unwrap(), golden);
    let root = decode(&golden, Limits::default()).unwrap();
    assert_eq!(root.stats().values, 6);
    let list = root.fields().next().unwrap().unwrap();
    let entries: Vec<_> = list
        .item()
        .elements()
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(entries.len(), 3);
    assert!(matches!(entries[0].value(), Value::Struct));
    assert_eq!(entries[1].fields().unwrap().count(), 0);
    let mut w = Encoder::new(Limits::default());
    w.struct_list(LIST, &[] as &[()], |_, _| unreachable!())
        .unwrap();
    assert_eq!(w.finish().unwrap(), [0x86, 0xcc, 0xf4, 4, 3, 0]);
}

#[test]
fn exact_budgets_reject_one_over_and_poison_the_writer() {
    let limits = Limits {
        max_bytes: 12,
        max_depth: 2,
        max_values: 3,
        max_collection: 1,
        ..Limits::default()
    };
    let mut w = Encoder::new(limits);
    w.struct_list(LIST, &[1], |w, value| w.integer(SCALAR, *value))
        .unwrap();
    assert_eq!(w.finish().unwrap().len(), 12);
    for smaller in [
        Limits {
            max_bytes: 11,
            ..limits
        },
        Limits {
            max_depth: 1,
            ..limits
        },
        Limits {
            max_values: 2,
            ..limits
        },
        Limits {
            max_collection: 0,
            ..limits
        },
    ] {
        let mut w = Encoder::new(smaller);
        let failure = w
            .struct_list(LIST, &[1], |w, value| w.integer(SCALAR, *value))
            .unwrap_err();
        assert_eq!(w.integer(SCALAR, 0).unwrap_err(), failure);
        assert_eq!(w.finish().unwrap_err(), failure);
    }
}

#[test]
fn closure_error_and_ignored_nested_error_never_expose_partial_output() {
    let error = Error {
        offset: 17,
        kind: ErrorKind::InvalidString,
    };
    let mut w = Encoder::new(Limits::default());
    assert_eq!(w.struct_list(LIST, &[1], |_, _| Err(error)), Err(error));
    assert_eq!(w.finish(), Err(error));

    let mut w = Encoder::new(Limits::default());
    // Even a callback that ignores an invalid nested field leaves the writer failed.
    let _ = w.struct_list(LIST, &[1], |w, _| {
        let _ = w.integer([0, 0, 0], 1);
        Ok(())
    });
    assert_eq!(w.finish().unwrap_err().kind, ErrorKind::InvalidTag);
}

#[test]
fn nested_struct_lists_obey_depth_and_malformed_counts_fail_decode() {
    let mut w = Encoder::new(Limits::default());
    w.struct_list(LIST, &[()], |w, _| {
        w.struct_list(LIST, &[()], |w, _| w.integer(SCALAR, 1))
    })
    .unwrap();
    let bytes = w.finish().unwrap();
    let limits = Limits {
        max_depth: 4,
        max_values: 5,
        max_collection: 1,
        ..Limits::default()
    };
    assert_eq!(decode(&bytes, limits).unwrap().stats().values, 5);
    assert!(
        decode(
            &bytes,
            Limits {
                max_depth: 3,
                ..limits
            }
        )
        .is_err()
    );
    let mut too_few = bytes.clone();
    too_few[5] = 2;
    assert!(decode(&too_few, limits).is_err());
    let mut too_many = bytes.clone();
    too_many[5] = 0;
    assert!(decode(&too_many, limits).is_err());
    for end in 1..bytes.len() {
        assert!(decode(&bytes[..end], limits).is_err());
    }
}

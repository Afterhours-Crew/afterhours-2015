// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use nfs_heat2::{Encoder, ErrorKind, Limits, decode};

const ADDRESS: [u8; 3] = [0x86, 0x49, 0x32];
const MEMBER: [u8; 3] = [0xda, 0x1b, 0x35];
const MAP: [u8; 3] = [0xba, 0xcb, 0x70];

fn hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn independent_tagged_union_and_string_integer_map_bytes() {
    let mut w = Encoder::new(Limits::default());
    w.struct_union(ADDRESS, 2, MEMBER, |w| w.integer([0xb6, 0x18, 0xe9], -1))
        .unwrap();
    w.string_integer_map(
        MAP,
        [(b"a".as_slice(), -1), (&[0xff, 0], i64::MIN)].into_iter(),
    )
    .unwrap();
    let bytes = w.finish().unwrap();
    assert_eq!(
        bytes,
        hex("8649320602da1b3503b618e9004100bacb70050100020261004103ff000040")
    );
    let doc = decode(&bytes, Limits::default()).unwrap();
    assert_eq!(doc.stats().values, 8);
    assert_eq!(doc.stats().max_depth, 2);
    for end in 1..bytes.len() {
        if end != 15 {
            assert!(decode(&bytes[..end], Limits::default()).is_err());
        }
    }
}

#[test]
fn selected_union_depth_values_selector_and_callback_failure_are_bounded() {
    for (limits, expected) in [
        (
            Limits {
                max_depth: 1,
                ..Limits::default()
            },
            ErrorKind::DepthLimit,
        ),
        (
            Limits {
                max_values: 2,
                ..Limits::default()
            },
            ErrorKind::ValueLimit,
        ),
        (
            Limits {
                max_bytes: 13,
                ..Limits::default()
            },
            ErrorKind::InputLimit,
        ),
    ] {
        let mut w = Encoder::new(limits);
        let e = w
            .struct_union(ADDRESS, 2, MEMBER, |w| w.integer(MAP, 0))
            .unwrap_err();
        assert_eq!(e.kind, expected);
        assert_eq!(w.unset_union(ADDRESS).unwrap_err(), e);
        assert_eq!(w.finish().unwrap_err(), e);
    }
    let limits = Limits {
        max_depth: 2,
        max_values: 3,
        max_bytes: 15,
        ..Limits::default()
    };
    let mut w = Encoder::new(limits);
    w.struct_union(ADDRESS, 2, MEMBER, |w| w.integer(MAP, 0))
        .unwrap();
    let bytes = w.finish().unwrap();
    assert_eq!(decode(&bytes, limits).unwrap().stats().values, 3);
    let mut w = Encoder::new(Limits::default());
    assert_eq!(
        w.struct_union(ADDRESS, 127, MEMBER, |_| Ok(()))
            .unwrap_err()
            .kind,
        ErrorKind::InvalidSelector
    );
    assert!(w.finish().is_err());
    let mut w = Encoder::new(Limits::default());
    let expected = nfs_heat2::Error {
        offset: 5,
        kind: ErrorKind::InvalidSchema,
    };
    assert_eq!(
        w.struct_union(ADDRESS, 2, MEMBER, |_| Err(expected))
            .unwrap_err(),
        expected
    );
    assert_eq!(w.finish().unwrap_err(), expected);
}

struct Liar {
    remaining: usize,
    promised: usize,
}
impl Iterator for Liar {
    type Item = (&'static [u8], i64);
    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            None
        } else {
            self.remaining -= 1;
            Some((b"x", 0))
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.promised, Some(self.promised))
    }
}
impl ExactSizeIterator for Liar {}

#[test]
fn string_integer_map_enforces_iterator_and_all_resource_limits() {
    for promised in [0, 2] {
        let mut w = Encoder::new(Limits::default());
        let e = w
            .string_integer_map(
                MAP,
                Liar {
                    remaining: 1,
                    promised,
                },
            )
            .unwrap_err();
        assert_eq!(e.kind, ErrorKind::CollectionCountMismatch);
        assert_eq!(w.finish().unwrap_err(), e);
    }
    for (limits, expected) in [
        (
            Limits {
                max_collection: 0,
                ..Limits::default()
            },
            ErrorKind::CollectionLimit,
        ),
        (
            Limits {
                max_values: 2,
                ..Limits::default()
            },
            ErrorKind::ValueLimit,
        ),
        (
            Limits {
                max_depth: 0,
                ..Limits::default()
            },
            ErrorKind::DepthLimit,
        ),
        (
            Limits {
                max_byte_string: 1,
                ..Limits::default()
            },
            ErrorKind::ByteStringLimit,
        ),
        (
            Limits {
                max_bytes: 9,
                ..Limits::default()
            },
            ErrorKind::InputLimit,
        ),
    ] {
        let mut w = Encoder::new(limits);
        let e = w
            .string_integer_map(MAP, [(b"x".as_slice(), -1)].into_iter())
            .unwrap_err();
        assert_eq!(e.kind, expected);
        assert_eq!(w.finish().unwrap_err(), e);
    }
    let limits = Limits {
        max_collection: 1,
        max_values: 3,
        max_depth: 1,
        max_byte_string: 2,
        max_bytes: 11,
    };
    let mut w = Encoder::new(limits);
    w.string_integer_map(MAP, [(b"x".as_slice(), -1)].into_iter())
        .unwrap();
    assert_eq!(
        decode(&w.finish().unwrap(), limits).unwrap().stats().values,
        3
    );
    let limits = Limits {
        max_collection: 0,
        max_values: 1,
        max_depth: 0,
        ..Limits::default()
    };
    let mut w = Encoder::new(limits);
    w.string_integer_map(MAP, [].into_iter()).unwrap();
    assert_eq!(w.finish().unwrap(), hex("bacb7005010000"));
}

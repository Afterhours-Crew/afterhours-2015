use nfs_heat2::{Encoder, ErrorKind, Limits, Value, decode};
const TAG: [u8; 3] = [0x9e, 0xeb, 0x33];

#[test]
fn independently_packed_strings_retain_order_duplicates_empty_and_non_utf8() {
    // GNLS list<string>, count 4: ff00, empty, A, A; terminators excluded from values.
    let golden = [
        0x9e, 0xeb, 0x33, 4, 1, 4, 3, 0xff, 0, 0, 1, 0, 2, 65, 0, 2, 65, 0,
    ];
    let input: &[&[u8]] = &[&[0xff, 0], b"", b"A", b"A"];
    let mut w = Encoder::new(Limits::default());
    w.string_list(TAG, input.iter().copied()).unwrap();
    assert_eq!(w.finish().unwrap(), golden);
    let root = decode(&golden, Limits::default()).unwrap();
    assert_eq!(root.stats().values, 5);
    let field = root.fields().next().unwrap().unwrap();
    let actual: Vec<_> = field
        .item()
        .elements()
        .unwrap()
        .map(|v| match v.unwrap().value() {
            Value::String(value) => value,
            _ => panic!("wrong kind"),
        })
        .collect();
    assert_eq!(actual, input);
    let mut w = Encoder::new(Limits::default());
    w.string_list(TAG, [].into_iter()).unwrap();
    assert_eq!(w.finish().unwrap(), [0x9e, 0xeb, 0x33, 4, 1, 0]);
}

#[test]
fn exact_budgets_reject_one_over_and_leave_failure_sticky() {
    let limits = Limits {
        max_bytes: 9,
        max_depth: 1,
        max_values: 2,
        max_collection: 1,
        max_byte_string: 2,
    };
    let mut w = Encoder::new(limits);
    w.string_list(TAG, [b"A".as_slice()].into_iter()).unwrap();
    let bytes = w.finish().unwrap();
    assert_eq!(bytes.len(), 9);
    for smaller in [
        Limits {
            max_bytes: 8,
            ..limits
        },
        Limits {
            max_depth: 0,
            ..limits
        },
        Limits {
            max_values: 1,
            ..limits
        },
        Limits {
            max_collection: 0,
            ..limits
        },
        Limits {
            max_byte_string: 1,
            ..limits
        },
    ] {
        let mut w = Encoder::new(smaller);
        let error = w
            .string_list(TAG, [b"A".as_slice()].into_iter())
            .unwrap_err();
        assert_eq!(w.integer(TAG, 0).unwrap_err(), error);
        assert_eq!(w.finish().unwrap_err(), error);
        assert!(decode(&bytes, smaller).is_err());
    }
}

struct Miscount {
    remaining: usize,
    advertised: usize,
}
impl Iterator for Miscount {
    type Item = &'static [u8];
    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            None
        } else {
            self.remaining -= 1;
            Some(b"a")
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.advertised, Some(self.advertised))
    }
}
impl ExactSizeIterator for Miscount {}
#[test]
fn dishonest_iterator_count_and_invalid_tag_never_expose_partial_bytes() {
    for (remaining, advertised) in [(0, 1), (2, 1), (1, 0)] {
        let mut w = Encoder::new(Limits::default());
        let error = w
            .string_list(
                TAG,
                Miscount {
                    remaining,
                    advertised,
                },
            )
            .unwrap_err();
        assert_eq!(error.kind, ErrorKind::CollectionCountMismatch);
        assert_eq!(w.finish().unwrap_err(), error);
    }
    let mut w = Encoder::new(Limits::default());
    assert_eq!(
        w.string_list([0, 0, 0], [].into_iter()).unwrap_err().kind,
        ErrorKind::InvalidTag
    );
    assert!(w.finish().is_err());
}

#[test]
fn nested_depth_counts_truncations_and_bad_string_terminator_fail() {
    let bytes = [0x9e, 0xeb, 0x33, 4, 1, 1, 2, 65, 0];
    for end in 1..bytes.len() {
        assert!(decode(&bytes[..end], Limits::default()).is_err());
    }
    for (offset, value) in [(5, 0), (5, 2), (6, 0), (6, 3), (8, 1)] {
        let mut bad = bytes;
        bad[offset] = value;
        assert!(decode(&bad, Limits::default()).is_err());
    }
    let mut w = Encoder::new(Limits {
        max_depth: 1,
        ..Limits::default()
    });
    assert!(
        w.structure(TAG, |w| w.string_list(TAG, [b"A".as_slice()].into_iter()))
            .is_err()
    );
    assert!(w.finish().is_err());
}

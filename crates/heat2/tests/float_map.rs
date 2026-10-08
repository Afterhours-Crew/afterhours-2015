use nfs_heat2::{Encoder, ErrorKind, Limits, Value};

#[test]
fn float_map_preserves_network_order_nan_payload_and_negative_zero() {
    let mut w = Encoder::new(Limits::default());
    w.string_float_bits_map(
        [0xcf, 0x48, 0x66],
        [(b"a".as_slice(), 0x7fc12345), (b"b", 0x80000000)].into_iter(),
    )
    .unwrap();
    let b = w.finish().unwrap();
    assert_eq!(
        b,
        [
            0xcf, 0x48, 0x66, 5, 1, 10, 2, 2, b'a', 0, 0x7f, 0xc1, 0x23, 0x45, 2, b'b', 0, 0x80, 0,
            0, 0
        ]
    );
    let d = nfs_heat2::decode(&b, Limits::default()).unwrap();
    let f = d.fields().next().unwrap().unwrap();
    let values: Vec<_> = f
        .item()
        .elements()
        .unwrap()
        .map(|x| x.unwrap().value())
        .collect();
    assert_eq!(values[1], Value::FloatBits(0x7fc12345));
    assert_eq!(values[3], Value::FloatBits(0x80000000));
}
#[test]
fn float_map_enforces_every_budget_and_latches_failure() {
    let entries = [(b"key".as_slice(), 0x3f800000)];
    for l in [
        Limits {
            max_bytes: 10,
            ..Default::default()
        },
        Limits {
            max_values: 2,
            ..Default::default()
        },
        Limits {
            max_collection: 0,
            ..Default::default()
        },
        Limits {
            max_depth: 0,
            ..Default::default()
        },
        Limits {
            max_byte_string: 3,
            ..Default::default()
        },
    ] {
        let mut w = Encoder::new(l);
        let e = w
            .string_float_bits_map([0xcf, 0x48, 0x66], entries.into_iter())
            .unwrap_err();
        assert_eq!(w.integer([0x8a, 0xca, 0x64], 1).unwrap_err(), e);
        assert_eq!(w.finish().unwrap_err(), e);
    }
}
struct WrongCount {
    left: usize,
    reported: usize,
}
impl Iterator for WrongCount {
    type Item = (&'static [u8], u32);
    fn next(&mut self) -> Option<Self::Item> {
        if self.left == 0 {
            None
        } else {
            self.left -= 1;
            Some((b"k", 0))
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.reported, Some(self.reported))
    }
}
impl ExactSizeIterator for WrongCount {}
#[test]
fn inconsistent_iterators_fail_without_publishing_partial_frames() {
    for (left, reported) in [(0, 1), (2, 1)] {
        let mut w = Encoder::new(Limits::default());
        assert_eq!(
            w.string_float_bits_map([0xcf, 0x48, 0x66], WrongCount { left, reported })
                .unwrap_err()
                .kind,
            ErrorKind::CollectionCountMismatch
        );
        assert!(w.finish().is_err());
    }
}

use nfs_heat2::{Encoder, ErrorKind, Limits, decode};

const MAP: [u8; 3] = [0x92, 0xd8, 0x70];
const LIST: [u8; 3] = [0xd6, 0xcc, 0xf4];

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn independent_numeric_map_and_object_list_goldens() {
    let mut writer = Encoder::new(Limits::default());
    writer
        .integer_map(MAP, [(0, -1), (i64::from(u32::MAX), i64::MIN)].into_iter())
        .unwrap();
    let bytes = writer.finish().unwrap();
    assert_eq!(bytes, hex("92d870050000020041bfffffff1f40"));
    assert_eq!(decode(&bytes, Limits::default()).unwrap().stats().values, 5);
    let mut writer = Encoder::new(Limits::default());
    writer
        .integer_triple_list(LIST, [[0, 65535, -1], [1, 2, i64::MIN]].into_iter())
        .unwrap();
    let bytes = writer.finish().unwrap();
    assert_eq!(bytes, hex("d6ccf404090200bfff0741010240"));
    assert_eq!(decode(&bytes, Limits::default()).unwrap().stats().values, 3);
}

#[test]
fn unset_union_absent_variable_and_empty_collections() {
    let mut writer = Encoder::new(Limits {
        max_values: 4,
        max_depth: 0,
        max_collection: 0,
        ..Limits::default()
    });
    writer.unset_union([0x86, 0x49, 0x32]).unwrap();
    writer.absent_variable([0x8f, 0x68, 0x72]).unwrap();
    writer.integer_map(MAP, [].into_iter()).unwrap();
    writer.integer_triple_list(LIST, [].into_iter()).unwrap();
    assert_eq!(
        writer.finish().unwrap(),
        hex("864932067f8f6872070092d87005000000d6ccf4040900")
    );
    for variable in [false, true] {
        for limits in [
            Limits {
                max_values: 0,
                ..Limits::default()
            },
            Limits {
                max_bytes: 4,
                ..Limits::default()
            },
        ] {
            let mut writer = Encoder::new(limits);
            let result = if variable {
                writer.absent_variable(MAP)
            } else {
                writer.unset_union(MAP)
            };
            assert!(result.is_err());
            assert!(writer.finish().is_err());
        }
    }
}

struct Liar<T> {
    actual: std::vec::IntoIter<T>,
    promised: usize,
}
impl<T> Iterator for Liar<T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        self.actual.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.promised, Some(self.promised))
    }
}
impl<T> ExactSizeIterator for Liar<T> {}

#[test]
fn wrong_iterator_lengths_poison_both_writers() {
    for promised in [0, 2] {
        let mut writer = Encoder::new(Limits::default());
        assert_eq!(
            writer
                .integer_map(
                    MAP,
                    Liar {
                        actual: vec![(1, 2)].into_iter(),
                        promised
                    }
                )
                .unwrap_err()
                .kind,
            ErrorKind::CollectionCountMismatch
        );
        assert!(writer.finish().is_err());
        let mut writer = Encoder::new(Limits::default());
        assert_eq!(
            writer
                .integer_triple_list(
                    LIST,
                    Liar {
                        actual: vec![[1, 2, 3]].into_iter(),
                        promised
                    }
                )
                .unwrap_err()
                .kind,
            ErrorKind::CollectionCountMismatch
        );
        assert!(writer.finish().is_err());
    }
}

#[test]
fn collection_limits_count_every_key_value_or_triple() {
    for map in [false, true] {
        let required_values = if map { 5 } else { 3 };
        for (limits, expected) in [
            (
                Limits {
                    max_collection: 1,
                    ..Limits::default()
                },
                ErrorKind::CollectionLimit,
            ),
            (
                Limits {
                    max_values: required_values - 1,
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
                    max_bytes: 7,
                    ..Limits::default()
                },
                ErrorKind::InputLimit,
            ),
        ] {
            let mut writer = Encoder::new(limits);
            let result = if map {
                writer.integer_map(MAP, [(1, 2), (3, 4)].into_iter())
            } else {
                writer.integer_triple_list(LIST, [[1, 2, 3]; 2].into_iter())
            };
            let error = result.unwrap_err();
            assert_eq!(error.kind, expected);
            assert_eq!(writer.integer(MAP, 0).unwrap_err(), error);
            assert_eq!(writer.finish().unwrap_err(), error);
        }
        let limits = Limits {
            max_values: required_values,
            max_collection: 2,
            max_depth: 1,
            ..Limits::default()
        };
        let mut writer = Encoder::new(limits);
        if map {
            writer
                .integer_map(MAP, [(1, 2), (3, 4)].into_iter())
                .unwrap();
        } else {
            writer
                .integer_triple_list(LIST, [[1, 2, 3]; 2].into_iter())
                .unwrap();
        }
        let bytes = writer.finish().unwrap();
        assert_eq!(
            decode(&bytes, limits).unwrap().stats().values,
            required_values
        );
    }
}

#[test]
fn truncated_collections_and_overflow_are_rejected() {
    for text in [
        "92d870050000020041bfffffff1f40",
        "d6ccf404090200bfff0741010240",
    ] {
        let bytes = hex(text);
        for cut in 1..bytes.len() {
            assert!(decode(&bytes[..cut], Limits::default()).is_err());
        }
    }
    for text in [
        "92d87005000041",
        "d6ccf4040941",
        "92d87005000080808080808080808002",
        "d6ccf4040901000080808080808080808002",
    ] {
        assert!(decode(&hex(text), Limits::default()).is_err());
    }
}

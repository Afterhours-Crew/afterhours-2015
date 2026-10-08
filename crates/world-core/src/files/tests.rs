use super::*;

#[test]
fn captured_items_start_and_zero_width_block_index() {
    // client File start: label Items, size5. Payload bytes below
    // are synthetic; no captured inventory or identity is retained here.
    let start = Record::Start {
        name: b"Items".to_vec(),
        bytes: 5,
    };
    let wire = start.encode(None).unwrap();
    assert_eq!(wire.len(), 76);
    assert_eq!(
        wire.bytes(),
        &[0x40, 0x54, 0x97, 0x46, 0x56, 0xd7, 0x30, 0, 0, 0x50]
    );
    assert_eq!(decode(wire.span(), None), Ok((start, 76)));
    let blocks = Record::Blocks {
        first: 0,
        blocks: vec![vec![1, 2, 3, 4, 5]],
    };
    let wire = blocks.encode(Some(5)).unwrap();
    assert_eq!(wire.len(), 46);
    assert_eq!(decode(wire.span(), Some(5)), Ok((blocks, 46)));
    assert_eq!(decode(wire.span(), None), Err(Error::MissingStart));
}

#[test]
fn every_truncated_prefix_and_invalid_range_is_rejected() {
    let records = [
        Record::Start {
            name: b"Items".to_vec(),
            bytes: 257,
        },
        Record::Blocks {
            first: 1,
            blocks: vec![vec![7; 128], vec![8]],
        },
        Record::Finish,
    ];
    for record in records {
        let encoded = record.encode(Some(257)).unwrap();
        for bits in 0..encoded.len() {
            let span = BitSpan::new(encoded.bytes(), 0, bits).unwrap();
            assert!(decode(span, Some(257)).is_err(), "prefix {bits}");
        }
        assert_eq!(
            decode(encoded.span(), Some(257)),
            Ok((record, encoded.len()))
        );
    }
    let mut invalid = BitWriter::new();
    invalid.put(2, 2).put(3, 2).put(0, 4);
    assert_eq!(decode(invalid.span(), Some(257)), Err(Error::Bound));
    assert!(
        Record::Start {
            name: vec![0; MAX_NAME + 1],
            bytes: 1
        }
        .encode(None)
        .is_err()
    );
    assert!(
        Record::Blocks {
            first: usize::MAX,
            blocks: vec![vec![0; 128]]
        }
        .encode(Some(257))
        .is_err()
    );
    assert!(
        Record::Blocks {
            first: 0,
            blocks: vec![vec![0; 128]; 17]
        }
        .encode(Some(4096))
        .is_err()
    );
}

#[test]
fn receiver_reorders_retries_and_conflicts_without_partial_commit() {
    let mut r = Receiver::default();
    let start = Record::Start {
        name: b"Items".to_vec(),
        bytes: 257,
    };
    r.receive(&start).unwrap();
    let last = Record::Blocks {
        first: 2,
        blocks: vec![vec![3]],
    };
    r.receive(&last).unwrap();
    r.receive(&start).unwrap(); // A retransmission cannot discard received data.
    assert_eq!(r.receive(&Record::Finish), Err(Error::Incomplete));
    let conflict = Record::Blocks {
        first: 1,
        blocks: vec![vec![2; 128], vec![4]],
    };
    assert_eq!(r.receive(&conflict), Err(Error::Conflict));
    // The first block of the failed batch must not have been committed.
    r.receive(&Record::Blocks {
        first: 1,
        blocks: vec![vec![9; 128]],
    })
    .unwrap();
    r.receive(&Record::Blocks {
        first: 0,
        blocks: vec![vec![1; 128]],
    })
    .unwrap();
    r.receive(&last).unwrap();
    r.receive(&Record::Finish).unwrap();
    r.receive(&Record::Finish).unwrap();
    assert_eq!(r.receive(&start), Err(Error::Bound)); // Undelivered completion is retained.
    let done = r.take_completed().unwrap();
    assert_eq!(done.name, b"Items");
    assert_eq!(done.bytes, [vec![1; 128], vec![9; 128], vec![3]].concat());
    r.receive(&Record::Finish).unwrap();
    assert!(r.take_completed().is_none());
    r.receive(&start).unwrap();
    assert_eq!(r.size(), Some(257));
}

#[test]
fn independent_receivers_and_resource_bounds() {
    let mut a = Receiver::default();
    let mut b = Receiver::default();
    let start = Record::Start {
        name: b"Items".to_vec(),
        bytes: 1,
    };
    a.receive(&start).unwrap();
    assert_eq!(b.receive(&Record::Finish), Err(Error::MissingStart));
    assert!(
        b.receive(&Record::Start {
            name: vec![],
            bytes: MAX_BYTES + 1
        })
        .is_err()
    );
    b.receive(&Record::Start {
        name: vec![],
        bytes: 0,
    })
    .unwrap();
    b.receive(&Record::Finish).unwrap();
    assert!(b.take_completed().unwrap().bytes.is_empty());
    assert_eq!(a.receive(&Record::Finish), Err(Error::Incomplete));
    assert_eq!(a.size(), Some(1));
}

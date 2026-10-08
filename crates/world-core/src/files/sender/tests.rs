// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use super::*;
use crate::files::Receiver;

fn data(size: usize) -> Completed {
    Completed {
        name: b"Items".to_vec(),
        bytes: (0..size).map(|i| i as u8).collect(),
    }
}

#[test]
fn loss_rejection_and_obsolete_receipts_preserve_exact_output() {
    let expected = data(49_860);
    let mut s = Sender::new(expected.clone(), Policy::default(), 0).unwrap();
    let mut r = Receiver::default();
    let start = s.offer(0).unwrap().unwrap();
    s.sent(&start, 1, 0).unwrap();
    r.receive(&start).unwrap(); // received, but the acknowledgement is lost
    assert_eq!(s.offer(999), Ok(None));
    assert_eq!(s.offer(1000), Ok(Some(start.clone())));
    s.sent(&start, 2, 1000).unwrap();
    assert!(!s.acknowledge(1, true, 1001).unwrap());
    r.receive(&start).unwrap();
    s.acknowledge(2, false, 1002).unwrap();
    assert_eq!(s.offer(1002), Ok(Some(start.clone())));
    s.sent(&start, 3, 1002).unwrap();
    s.acknowledge(3, true, 1003).unwrap();
    let mut kinds = [0usize; 3];
    let mut now = 1004;
    while let Some(record) = s.offer(now).unwrap() {
        let wire = record.encode(s.size()).unwrap();
        let (decoded, used) = crate::files::decode(wire.span(), r.size()).unwrap();
        assert_eq!(used, wire.len());
        r.receive(&decoded).unwrap();
        kinds[match record {
            Record::Start { .. } => 0,
            Record::Blocks { .. } => 1,
            Record::Finish => 2,
        }] += 1;
        s.sent(&record, now, now).unwrap();
        s.acknowledge(now, true, now).unwrap();
        now += 1;
    }
    assert_eq!(kinds, [0, 36, 1]);
    assert!(s.done());
    assert_eq!(s.size(), None);
    assert_eq!(r.take_completed(), Some(expected));
    assert!(!s.acknowledge(1, true, now).unwrap());
}

#[test]
fn deadline_retry_limit_clock_bounds_and_empty_transfer() {
    let policy = Policy {
        retry_ms: 10,
        deadline_ms: 100,
        max_attempts: 2,
        ..Policy::default()
    };
    let mut s = Sender::new(data(128), policy, 20).unwrap();
    assert_eq!(s.offer(19), Err(Error::Clock));
    let start = s.offer(20).unwrap().unwrap();
    assert_eq!(s.sent(&Record::Finish, 1, 20), Err(Error::State));
    s.sent(&start, 1, 20).unwrap();
    s.sent(&start, 2, 30).unwrap();
    assert_eq!(s.offer(40), Err(Error::Attempts));
    assert_eq!((s.size(), s.waiting()), (None, None));
    let mut slow = Sender::<u64>::new(data(1), policy, 0).unwrap();
    assert_eq!(slow.offer(100), Err(Error::Deadline));
    assert_eq!(slow.size(), None);
    assert!(Sender::<u64>::new(data(MAX_BYTES + 1), policy, 0).is_err());
    assert!(
        Sender::<u64>::new(
            data(1),
            Policy {
                retry_ms: 0,
                ..policy
            },
            0
        )
        .is_err()
    );
    let mut empty = Sender::new(data(0), policy, 0).unwrap();
    for (t, record) in [
        (
            1,
            Record::Start {
                name: b"Items".to_vec(),
                bytes: 0,
            },
        ),
        (2, Record::Finish),
    ] {
        assert_eq!(empty.offer(t), Ok(Some(record.clone())));
        empty.sent(&record, t, t).unwrap();
        empty.acknowledge(t, true, t).unwrap();
    }
    assert!(empty.done());
}

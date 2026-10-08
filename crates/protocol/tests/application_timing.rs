use nfs_protocol::world::{
    envelope::{ZeroTail, checksum},
    timing::{Control, Decoded, Error},
};
fn hex(s: &str) -> Vec<u8> {
    s.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 16).unwrap())
        .collect()
}
fn repair(b: &mut [u8]) {
    let n = b.len();
    let c = checksum(&b[..n - 2]).to_be_bytes();
    b[n - 2..].copy_from_slice(&c);
}
#[test]
fn independently_packed_request_fixture() {
    let b = hex("a0009140440555aa55aa90091a2b3c4d5e6f786437");
    let d = Decoded::decode(&b).unwrap();
    assert_eq!(
        (d.selector(), d.number(), d.acknowledgement(), d.history()),
        (9, 17, 5, 0x55aa55aa)
    );
    assert_eq!(
        d.control(),
        Control::Request {
            timestamp: 0x0123456789abcdef
        }
    );
    assert_eq!(
        Decoded::new(9, 17, 5, 0x55aa55aa, d.control())
            .unwrap()
            .encode(),
        b
    );
    assert_eq!(
        ZeroTail::new(29)
            .unwrap()
            .decode(&b)
            .unwrap()
            .effective_bits(),
        165
    );
}
#[test]
fn independently_packed_reply_and_padding_fixture() {
    let b = hex("a0009140440555aa55aa98091a2b3c4d5e6f7ff6e5d4c3b2a19087862a");
    let d = Decoded::decode(&b).unwrap();
    assert_eq!(
        d.control(),
        Control::Reply {
            echo: 0x0123456789abcdef,
            clock: 0xfedcba9876543210
        }
    );
    assert_eq!(d.padding(), 7);
    assert_eq!(d.encode(), b);
    assert_eq!(
        ZeroTail::new(29)
            .unwrap()
            .decode(&b)
            .unwrap()
            .effective_bits(),
        229
    );
}
#[test]
fn physical_padding_is_checksum_covered_and_preserved() {
    for c in [
        Control::Request {
            timestamp: u64::MAX,
        },
        Control::Reply {
            echo: 0,
            clock: u64::MAX,
        },
    ] {
        let zero = Decoded::new(1, 1, 0, 0, c).unwrap().encode();
        for p in 0..8 {
            let mut b = zero.clone();
            let n = b.len();
            b[n - 3] |= p;
            repair(&mut b);
            let d = Decoded::decode(&b).unwrap();
            assert_eq!(d.padding(), p);
            assert_eq!(d.encode(), b);
        }
    }
}
#[test]
fn widths_and_history_are_structural_not_send_policy() {
    for (s, n, a) in [(0, 0, 0), (16384, 0, 0), (1, 1024, 0), (1, 0, 1024)] {
        assert_eq!(
            Decoded::new(s, n, a, 0, Control::Request { timestamp: 0 }),
            Err(Error::Bounds)
        );
    }
    let d = Decoded::new(
        16383,
        1023,
        1023,
        u32::MAX,
        Control::Request {
            timestamp: u64::MAX,
        },
    )
    .unwrap();
    assert_eq!(Decoded::decode(&d.encode()).unwrap(), d);
}
#[test]
fn all_truncations_and_resource_excess_fail() {
    for c in [
        Control::Request { timestamp: 1 },
        Control::Reply { echo: 1, clock: 2 },
    ] {
        let b = Decoded::new(1, 1, 0, 0, c).unwrap().encode();
        for n in 0..b.len() {
            assert!(Decoded::decode(&b[..n]).is_err());
        }
        let mut extra = b;
        extra.push(0);
        assert!(Decoded::decode(&extra).is_err());
    }
    assert!(Decoded::decode(&[0; 30]).is_err());
}
#[test]
fn every_single_bit_corruption_is_refused() {
    let b = Decoded::new(
        1,
        1,
        0,
        0,
        Control::Reply {
            echo: 0xabc,
            clock: 0xdef,
        },
    )
    .unwrap()
    .encode();
    for i in 0..b.len() * 8 {
        let mut bad = b.clone();
        bad[i / 8] ^= 1 << (i % 8);
        assert!(Decoded::decode(&bad).is_err());
    }
}
#[test]
fn wrong_kind_queued_and_unknown_control_fail_with_valid_checksum() {
    let b = Decoded::new(1, 1, 0, 0, Control::Request { timestamp: 0 })
        .unwrap()
        .encode();
    for (i, x) in [(2, 0x10), (10, 0), (10, 0x88)] {
        let mut bad = b.clone();
        bad[i] = x;
        repair(&mut bad);
        assert!(Decoded::decode(&bad).is_err());
    }
}
#[test]
fn no_excluded_tail_or_changed_effective_shape() {
    let b = Decoded::new(1, 1, 0, 0, Control::Request { timestamp: 0 })
        .unwrap()
        .encode();
    for first in [0xa4, 0x80, 0xc0] {
        let mut bad = b.clone();
        bad[0] = first;
        repair(&mut bad);
        assert!(Decoded::decode(&bad).is_err());
    }
}
#[test]
fn raw_clock_words_remain_opaque_and_debug_redacted() {
    let d = Decoded::new(
        9,
        17,
        5,
        1,
        Control::Reply {
            echo: f64::NAN.to_bits(),
            clock: u64::MAX,
        },
    )
    .unwrap();
    assert_eq!(Decoded::decode(&d.encode()).unwrap(), d);
    let debug = format!("{d:?}");
    assert!(!debug.contains("184467") && !debug.contains("NaN"));
    assert_eq!(format!("{:?}", d.control()), "Reply { .. }");
    let e: &dyn std::error::Error = &Error::Shape;
    assert!(e.to_string().contains("Shape"));
}

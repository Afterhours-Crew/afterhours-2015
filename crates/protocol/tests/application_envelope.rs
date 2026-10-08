// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Independently packed literals for the application wire layout.
use nfs_protocol::world::envelope::{Error, ZeroTail, checksum};

const FIXTURE: [u8; 16] = [
    0x01, 0x23, 0x41, 0x45, 0x56, 0xaa, 0x80, 0, 0, 1, 0xde, 0xad, 0xbe, 0xef, 0x7f, 0x1d,
];
fn profile() -> ZeroTail {
    ZeroTail::new(1600).unwrap()
}
fn repair(bytes: &mut [u8]) {
    let end = bytes.len() - 2;
    let value = checksum(&bytes[..end]).to_be_bytes();
    bytes[end..].copy_from_slice(&value);
}

#[test]
fn literal_fields_and_borrowed_payload() {
    let p = profile().decode(&FIXTURE).unwrap();
    assert_eq!(
        (p.connection_selector(), p.kind(), p.effective_bits()),
        (0x1234, 20, 128)
    );
    let s = p.sequence().unwrap();
    assert_eq!(
        (s.number().value(), s.acknowledgement().value(), s.history()),
        (0x155, 0x2aa, 0x80000001)
    );
    assert_eq!((p.payload().start(), p.payload().len()), (0, 32));
    assert_eq!(p.payload().bytes(), &FIXTURE[10..14]);
    assert_eq!(p.payload().bytes().as_ptr(), FIXTURE[10..].as_ptr());
}

#[test]
fn independent_checksum_vectors() {
    assert_eq!(checksum(&[]), 0xff00);
    assert_eq!(checksum(&[1, 2, 3]), 0xef0a);
    assert_eq!(checksum(&[255, 1]), 0x00ff);
    assert_eq!(checksum(&FIXTURE[..14]), 0x7f1d);
}

#[test]
fn truncations_limits_and_integrity_fail_closed() {
    for end in 0..FIXTURE.len() {
        assert!(profile().decode(&FIXTURE[..end]).is_err());
    }
    for bit in (0..20).chain(28..128) {
        let mut bytes = FIXTURE;
        bytes[bit / 8] ^= 1 << (7 - bit % 8);
        assert!(profile().decode(&bytes).is_err(), "bit {bit}");
    }
    assert_eq!(
        ZeroTail::new(15).unwrap().decode(&FIXTURE),
        Err(Error::InputLimit)
    );
    assert!(ZeroTail::new(16).unwrap().decode(&FIXTURE).is_ok());
    for size in [0, 1, 2, 3, usize::MAX, usize::MAX / 8 + 1] {
        assert!(matches!(ZeroTail::new(size), Err(Error::InvalidLimit)));
    }
    assert!(ZeroTail::new(usize::MAX / 8).is_ok());
}

#[test]
fn all_final_remainders_bound_partial_payload_and_padding_is_checked() {
    for remainder in 0..8 {
        let mut bytes = FIXTURE;
        bytes[0] = (bytes[0] & 31) | (remainder << 5);
        repair(&mut bytes);
        let p = profile().decode(&bytes).unwrap();
        let valid = if remainder == 0 {
            32
        } else {
            24 + usize::from(remainder)
        };
        assert_eq!(p.payload().len(), valid);
        assert!(p.payload().read_u32(valid, 1).is_err());
        assert!(p.payload().read_u32(valid, 0).is_ok());
        assert_eq!(p.payload().bytes(), &bytes[10..14]);
        if remainder != 0 {
            bytes[13] ^= 1; // A padding bit, still covered by the checksum.
            assert_eq!(profile().decode(&bytes), Err(Error::ChecksumMismatch));
        }
    }
    // Independent partial literal: 29 valid payload bits, three padding bits.
    let mut bytes = FIXTURE;
    bytes[0] = 0xa1;
    bytes[14..].copy_from_slice(&[0x1f, 0xdd]);
    assert_eq!(profile().decode(&bytes).unwrap().payload().len(), 29);
}

#[test]
fn empty_payload_and_header_only_control_have_exact_bounds() {
    let mut empty = FIXTURE[..10].to_vec();
    empty.extend(checksum(&empty).to_be_bytes());
    let p = profile().decode(&empty).unwrap();
    assert!(p.payload().is_empty());
    assert!(p.payload().bytes().is_empty());
    for remainder in 1..8 {
        empty[0] = (empty[0] & 31) | (remainder << 5);
        repair(&mut empty);
        assert_eq!(profile().decode(&empty), Err(Error::Truncated));
    }
    let p = profile().decode(&[0x80, 0, 0, 0]).unwrap();
    assert_eq!(p.effective_bits(), 28);
    assert!(p.payload().is_empty());
    for first in [0x20, 0x40, 0x60] {
        assert_eq!(profile().decode(&[first, 0, 0, 0]), Err(Error::Truncated));
    }
}

#[test]
fn unknown_kinds_stay_opaque_and_are_not_assumed_checksummed() {
    let bytes = [0x80, 0, 0x0e, 0x70, 0xab];
    let p = profile().decode(&bytes).unwrap();
    assert_eq!((p.kind(), p.effective_bits()), (231, 36));
    assert!(p.sequence().is_none());
    assert_eq!((p.payload().start(), p.payload().len()), (28, 8));
    assert_eq!(p.payload().read_u32(0, 8).unwrap(), 0x0a);
    for kind in 0..=255_u8 {
        if kind == 20 {
            continue;
        }
        let mut bytes = FIXTURE;
        bytes[2] = (bytes[2] & 0xf0) | (kind >> 4);
        bytes[3] = (bytes[3] & 15) | (kind << 4);
        let p = profile().decode(&bytes).unwrap();
        assert_eq!(p.kind(), kind);
        assert!(p.sequence().is_none());
        assert_eq!(p.payload().len(), 100);
    }
}

#[test]
fn all_nonzero_tails_are_unsupported_even_with_valid_checksum() {
    for tail in 1..8 {
        let mut bytes = FIXTURE;
        bytes[0] |= tail << 2;
        repair(&mut bytes);
        assert_eq!(profile().decode(&bytes), Err(Error::UnsupportedTail));
    }
}

#[test]
fn arbitrary_inputs_never_escape_selected_bounds_or_leak_in_debug() {
    let mut seed = 0x9e3779b9_u32;
    for len in 0..10000 {
        let mut bytes = vec![0; len % 1800];
        for byte in &mut bytes {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            *byte = seed as u8;
        }
        if let Ok(p) = profile().decode(&bytes) {
            assert!(bytes.len() <= 1600);
            assert!(p.effective_bits() <= bytes.len() * 8);
            assert!(p.payload().read_u32(p.payload().len(), 1).is_err());
            assert!(p.payload().slice(0, p.payload().len()).is_ok());
        }
    }
    let p = profile().decode(&FIXTURE).unwrap();
    let debug = format!("{p:?} {:?}", p.sequence());
    for secret in [
        "4660",
        "1234",
        "2147483649",
        "deadbeef",
        "222, 173",
        "341",
        "682",
    ] {
        assert!(!debug.contains(secret));
    }
}

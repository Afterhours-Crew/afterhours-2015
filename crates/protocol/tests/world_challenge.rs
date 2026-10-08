// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! independent synthetic literals packed from the native bit graph.
//! No original account, challenge, digest, progression or capture bytes used.
use nfs_protocol::world::{
    challenge::{
        Challenge15, Error, MAX_BODY_BYTES, MAX_OPTIONAL_BYTES, OpaqueAnswer16, Request13,
    },
    envelope::{self, ZeroTail},
};

const REQUEST: [u8; 12] = [
    0x80, 0, 0, 0xd1, 0x23, 0x45, 0x67, 0x89, 0x0a, 0xbc, 0xde, 0xf0,
];
const EMPTY: [u8; 8] = [0xa0, 0, 0, 0xf1, 0x02, 0x03, 0x04, 0];
const OPTIONAL: [u8; 18] = [
    0xe0, 0, 0, 0xf1, 0x02, 0x03, 0x04, 0x0c, 0x7a, 0xa0, 0x0c, 0x02, 0x04, 0x06, 0x08, 0x0a, 0x0c,
    0x0e,
];
const ANSWER: [u8; 35] = [
    0xe0, 0, 0x01, 0, 0, 0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80, 0x90, 0xa0, 0xb0, 0xc0,
    0xd0, 0xe0, 0xf1, 0x01, 0x11, 0x21, 0x3f, 0x1f, 0xc0, 0x0f, 0x43, 0x45, 0x47, 0x49, 0x4b, 0x4d,
    0x4f, 0x50,
];

#[test]
fn independent_request_fields_and_exact_literal() {
    let q = Request13::decode(&REQUEST).unwrap();
    assert_eq!(q.fields(), [0x12345678, 0x90abcdef]);
    assert_eq!(q.encode(), REQUEST);
    assert_eq!(Request13::new(q.fields()).encode(), REQUEST);
    let e = ZeroTail::new(MAX_BODY_BYTES)
        .unwrap()
        .decode(&REQUEST)
        .unwrap();
    assert_eq!(
        (
            e.kind(),
            e.connection_selector(),
            e.effective_bits(),
            e.payload().len()
        ),
        (13, 0, 92, 64)
    );
    assert!(e.sequence().is_none());
}
#[test]
fn native_empty_and_present_challenge_literals() {
    let empty = Challenge15::decode(&EMPTY).unwrap();
    assert_eq!(empty.value(), 0x10203040);
    assert!(empty.optional().is_empty());
    assert_eq!(empty.encode(), EMPTY);
    assert_eq!(Challenge15::new(0x10203040, &[]).unwrap().encode(), EMPTY);
    let c = Challenge15::decode(&OPTIONAL).unwrap();
    assert_eq!(c.value(), 0x10203040);
    assert_eq!(c.optional(), &[1, 2, 3, 4, 5, 6, 7]);
    assert_eq!(c.encode(), OPTIONAL);
    assert_eq!(
        Challenge15::new(c.value(), c.optional()).unwrap().encode(),
        OPTIONAL
    );
    assert_eq!(
        ZeroTail::new(MAX_BODY_BYTES)
            .unwrap()
            .decode(&EMPTY)
            .unwrap()
            .effective_bits(),
        61
    );
    assert_eq!(
        ZeroTail::new(MAX_BODY_BYTES)
            .unwrap()
            .decode(&OPTIONAL)
            .unwrap()
            .effective_bits(),
        143
    );
}
#[test]
fn opaque_answer_literal_is_structural_only() {
    let a = OpaqueAnswer16::decode(&ANSWER).unwrap();
    assert_eq!(
        a.opaque_field(),
        &std::array::from_fn::<_, 20, _>(|i| i as u8)
    );
    assert_eq!(
        a.optional(),
        &[0xa1, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8]
    );
    assert_eq!(a.encode(), ANSWER);
    assert_eq!(
        OpaqueAnswer16::new(*a.opaque_field(), a.optional())
            .unwrap()
            .encode(),
        ANSWER
    );
    let mut changed = ANSWER;
    changed[4] ^= 1;
    let b = OpaqueAnswer16::decode(&changed).unwrap();
    assert_ne!(b.opaque_field(), a.opaque_field()); // No challenge/digest equality is assigned by this byte codec.
}
#[test]
fn nonzero_unused_final_bits_are_retained_exactly() {
    let mut q = REQUEST;
    q[11] |= 0x0b;
    let decoded = Request13::decode(&q).unwrap();
    assert_eq!(decoded.fields(), [0x12345678, 0x90abcdef]);
    assert_eq!(decoded.encode(), q);
    let mut c = EMPTY;
    c[7] |= 7;
    let decoded = Challenge15::decode(&c).unwrap();
    assert_eq!(decoded.value(), 0x10203040);
    assert_eq!(decoded.encode(), c);
    let mut c = OPTIONAL;
    c[17] |= 1;
    assert_eq!(Challenge15::decode(&c).unwrap().encode(), c);
    let mut a = ANSWER;
    a[34] |= 1;
    assert_eq!(OpaqueAnswer16::decode(&a).unwrap().encode(), a);
}
#[test]
fn wrong_kind_selector_and_transform_tail_fail_closed() {
    assert_eq!(Challenge15::decode(&REQUEST), Err(Error::WrongKind));
    assert_eq!(OpaqueAnswer16::decode(&OPTIONAL), Err(Error::WrongKind));
    let mut q = REQUEST;
    q[2] |= 0x10;
    assert_eq!(Request13::decode(&q), Err(Error::NonzeroSelector));
    let mut c = OPTIONAL;
    c[2] |= 0x10;
    assert_eq!(Challenge15::decode(&c), Err(Error::NonzeroSelector));
    let mut a = ANSWER;
    a[2] |= 0x10;
    assert_eq!(OpaqueAnswer16::decode(&a), Err(Error::NonzeroSelector));
    for source in [&REQUEST[..], &OPTIONAL[..], &ANSWER[..]] {
        let mut b = source.to_vec();
        b[0] |= 4;
        let e = if source.len() == 12 {
            Request13::decode(&b).map(|_| ())
        } else if source.len() == 18 {
            Challenge15::decode(&b).map(|_| ())
        } else {
            OpaqueAnswer16::decode(&b).map(|_| ())
        };
        assert_eq!(e, Err(Error::Envelope(envelope::Error::UnsupportedTail)));
    }
}
#[test]
fn all_truncations_and_extra_payload_refuse() {
    for i in 0..REQUEST.len() {
        assert!(Request13::decode(&REQUEST[..i]).is_err());
    }
    for i in 0..EMPTY.len() {
        assert!(Challenge15::decode(&EMPTY[..i]).is_err());
    }
    for i in 0..OPTIONAL.len() {
        assert!(Challenge15::decode(&OPTIONAL[..i]).is_err());
    }
    for i in 0..ANSWER.len() {
        assert!(OpaqueAnswer16::decode(&ANSWER[..i]).is_err());
    }
    let mut extra = REQUEST.to_vec();
    extra.push(0);
    assert_eq!(Request13::decode(&extra), Err(Error::PayloadLength));
    let mut extra = EMPTY.to_vec();
    extra.push(0);
    assert_eq!(Challenge15::decode(&extra), Err(Error::PayloadLength));
    let mut extra = ANSWER.to_vec();
    extra.push(0);
    assert_eq!(OpaqueAnswer16::decode(&extra), Err(Error::PayloadLength));
    let mut changed = REQUEST;
    changed[0] &= 31;
    assert_eq!(Request13::decode(&changed), Err(Error::PayloadLength));
    let mut concatenated = EMPTY.to_vec();
    concatenated.extend_from_slice(&EMPTY);
    assert_eq!(
        Challenge15::decode(&concatenated),
        Err(Error::PayloadLength)
    );
}
#[test]
fn optional_checksum_length_and_presence_are_enforced() {
    let mut c = OPTIONAL;
    c[8] ^= 1;
    assert_eq!(
        Challenge15::decode(&c),
        Err(Error::OptionalChecksumMismatch)
    );
    let mut c = OPTIONAL;
    c[10] ^= 2;
    assert_eq!(Challenge15::decode(&c), Err(Error::PayloadLength));
    let mut c = OPTIONAL;
    c[7] &= !8;
    assert_eq!(Challenge15::decode(&c), Err(Error::PayloadLength));
    let mut a = ANSWER;
    a[24] ^= 1;
    assert_eq!(
        OpaqueAnswer16::decode(&a),
        Err(Error::OptionalChecksumMismatch)
    );
    let mut a = ANSWER;
    a[26] ^= 2;
    assert_eq!(OpaqueAnswer16::decode(&a), Err(Error::PayloadLength));
}
#[test]
fn optional_resource_bounds_and_empty_answer() {
    let data = vec![0x55; MAX_OPTIONAL_BYTES];
    let c = Challenge15::new(0, &data).unwrap();
    assert_eq!(c.encode().len(), 1035);
    assert_eq!(Challenge15::decode(&c.encode()).unwrap().optional(), data);
    let a = OpaqueAnswer16::new([0; 20], &data).unwrap();
    assert_eq!(a.encode().len(), MAX_BODY_BYTES);
    assert_eq!(
        OpaqueAnswer16::decode(&a.encode()).unwrap().optional(),
        data
    );
    let empty = OpaqueAnswer16::new([0xff; 20], &[]).unwrap();
    assert_eq!(empty.encode().len(), 24);
    assert_eq!(OpaqueAnswer16::decode(&empty.encode()), Ok(empty));
    assert_eq!(
        Challenge15::new(0, &vec![0; MAX_OPTIONAL_BYTES + 1]),
        Err(Error::OptionalLimit)
    );
    assert_eq!(
        OpaqueAnswer16::new([0; 20], &vec![0; MAX_OPTIONAL_BYTES + 1]),
        Err(Error::OptionalLimit)
    );
    let too_large = vec![0; MAX_BODY_BYTES + 1];
    assert_eq!(Request13::decode(&too_large), Err(Error::InputLimit));
    assert_eq!(Challenge15::decode(&too_large), Err(Error::InputLimit));
    assert_eq!(OpaqueAnswer16::decode(&too_large), Err(Error::InputLimit));
}
#[test]
fn debug_omits_all_sensitive_fields() {
    assert_eq!(
        format!("{:?}", Request13::decode(&REQUEST).unwrap()),
        "Request13 { .. }"
    );
    assert_eq!(
        format!("{:?}", Challenge15::decode(&OPTIONAL).unwrap()),
        "Challenge15 { optional_bytes: 7, .. }"
    );
    assert_eq!(
        format!("{:?}", OpaqueAnswer16::decode(&ANSWER).unwrap()),
        "OpaqueAnswer16 { optional_bytes: 8, .. }"
    );
}

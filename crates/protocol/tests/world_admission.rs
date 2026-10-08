// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Independent synthetic literals from the native bit graph.
//! No original capture, account, engine/peer token or callback bytes used.
use nfs_protocol::world::admission::{Decoded, Error, MAX_CONNECT_BYTES, Message};

#[test]
fn independent_native_field_order_vectors() {
    // Direct MSB-first 6+14+8 header and32-bit field, not produced by the codec.
    let h1 = [0x80, 0, 0, 0x10, 0x10, 0x20, 0x30, 0x40];
    assert_eq!(
        Decoded::decode(&h1).unwrap().message,
        Message::Host1 {
            engine_token: 0x01020304
        }
    );
    assert_eq!(
        Decoded::new(Message::Host1 {
            engine_token: 0x01020304
        })
        .unwrap()
        .encode()
        .unwrap(),
        h1
    );
    let h9 = [0x80, 0, 0, 0x90];
    assert_eq!(Decoded::decode(&h9).unwrap().message, Message::Host9);
    assert_eq!(Decoded::new(Message::Host9).unwrap().encode().unwrap(), h9);
    let h4 = [0x80, 0, 0, 0x40, 0x10, 0x20, 0x30, 0x40, 0, 0x10, 0, 0];
    assert_eq!(
        Decoded::decode(&h4).unwrap().message,
        Message::Host4 {
            peer_echo: 0x01020304,
            assigned_selector: 1,
            callback_kind: 0,
            opaque: vec![]
        }
    );
    assert_eq!(
        Decoded::new(Message::Host4 {
            peer_echo: 0x01020304,
            assigned_selector: 1,
            callback_kind: 0,
            opaque: vec![]
        })
        .unwrap()
        .encode()
        .unwrap(),
        h4
    );
}
#[test]
fn native_unused_padding_retained_not_zero_required() {
    let h1 = [0x80, 0, 0, 0x10, 0x10, 0x20, 0x30, 0x4f];
    let d = Decoded::decode(&h1).unwrap();
    assert_eq!(d.encode().unwrap(), h1);
    assert_eq!(
        d.message,
        Message::Host1 {
            engine_token: 0x01020304
        }
    );
}
#[test]
fn explicit_client2_u16_field_and_opaque_bytes() {
    // Native body: first32, second32, length16=1, byte0xaa.
    let b = [
        0x80, 0, 0, 0x20, 0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80, 0, 0x1a, 0xa0,
    ];
    let m = Message::Client2 {
        peer_token: 0x01020304,
        engine_echo: 0x05060708,
        opaque: vec![0xaa],
    };
    assert_eq!(Decoded::decode(&b).unwrap().message, m);
    assert_eq!(Decoded::new(m).unwrap().encode().unwrap(), b);
}
#[test]
fn raw_client2_zero_and_maximum_local_blob() {
    for n in [0, MAX_CONNECT_BYTES] {
        let m = Message::Client2 {
            peer_token: 0,
            engine_echo: u32::MAX,
            opaque: vec![0x5a; n],
        };
        let d = Decoded::new(m.clone()).unwrap();
        assert_eq!(Decoded::decode(&d.encode().unwrap()).unwrap().message, m);
    }
    assert_eq!(
        Decoded::new(Message::Client2 {
            peer_token: 0,
            engine_echo: 0,
            opaque: vec![0; MAX_CONNECT_BYTES + 1]
        }),
        Err(Error::Limit)
    );
}
#[test]
fn all_byte_width_callback_values_preserved() {
    let m = Message::Host4 {
        peer_echo: u32::MAX,
        assigned_selector: u16::MAX,
        callback_kind: 255,
        opaque: vec![0xa5; 255],
    };
    // The pure codec does not invent admission or narrow the native16-bit field.
    let d = Decoded::new(m.clone()).unwrap();
    assert_eq!(Decoded::decode(&d.encode().unwrap()).unwrap().message, m);
    assert_eq!(
        Decoded::new(Message::Host4 {
            peer_echo: 0,
            assigned_selector: 1,
            callback_kind: 0,
            opaque: vec![0; 256]
        }),
        Err(Error::Limit)
    );
}
#[test]
fn transformed_client5_and_host6_remain_opaque() {
    for kind in [5, 6] {
        let m = Message::Transformed {
            kind,
            selector: 16383,
            opaque: vec![0x12, 0x34, 0xaa],
        };
        let d = Decoded::new(m.clone()).unwrap();
        assert_eq!(Decoded::decode(&d.encode().unwrap()).unwrap().message, m);
    }
    assert_eq!(
        Decoded::new(Message::Transformed {
            kind: 6,
            selector: 0,
            opaque: vec![]
        }),
        Err(Error::Selector)
    );
    assert_eq!(
        Decoded::new(Message::Transformed {
            kind: 6,
            selector: 16384,
            opaque: vec![]
        }),
        Err(Error::Selector)
    );
}
#[test]
fn selector_zero_enforcement_uses_header_not_payload() {
    let mut b = Decoded::new(Message::Host1 { engine_token: 1 })
        .unwrap()
        .encode()
        .unwrap();
    b[2] |= 0x10;
    assert_eq!(Decoded::decode(&b), Err(Error::Selector));
}
#[test]
fn strict_complete_payload_and_absent_field_fail() {
    let d = Decoded::new(Message::Client2 {
        peer_token: 42,
        engine_echo: 17,
        opaque: vec![1, 2],
    })
    .unwrap()
    .encode()
    .unwrap();
    for len in 0..d.len() {
        assert!(Decoded::decode(&d[..len]).is_err());
    }
    let mut extra = d;
    extra.push(0);
    assert_eq!(Decoded::decode(&extra), Err(Error::Length));
    let mut missing = Decoded::new(Message::Host4 {
        peer_echo: 1,
        assigned_selector: 1,
        callback_kind: 0,
        opaque: vec![],
    })
    .unwrap()
    .encode()
    .unwrap();
    missing.pop();
    assert!(Decoded::decode(&missing).is_err());
}
#[test]
fn body_limit_and_claimed_length_are_bounded_before_allocation() {
    assert_eq!(Decoded::decode(&vec![0; 1039]), Err(Error::Limit));
    let mut b = Decoded::new(Message::Client2 {
        peer_token: 1,
        engine_echo: 2,
        opaque: vec![],
    })
    .unwrap()
    .encode()
    .unwrap();
    // Native length16 starts at bit92; set all16 length bits. No bytes follow.
    b[11] |= 0x0f;
    b[12] = 255;
    b[13] |= 0xf0;
    assert_eq!(Decoded::decode(&b), Err(Error::Limit));
}
#[test]
fn excluded_tail_and_unknown_kind_fail_closed() {
    let mut b = Decoded::new(Message::Host9).unwrap().encode().unwrap();
    b[0] |= 4;
    assert_eq!(Decoded::decode(&b), Err(Error::Envelope));
    let unknown = [0x80, 0, 0, 0xa0];
    assert_eq!(Decoded::decode(&unknown), Err(Error::Kind));
}
#[test]
fn secret_fields_are_never_debugged() {
    let d = Decoded::new(Message::Client2 {
        peer_token: 0xdeadbeef,
        engine_echo: 0xfedcba98,
        opaque: vec![0x97, 0x32, 0x55],
    })
    .unwrap();
    assert_eq!(
        format!("{d:?}"),
        "AdmissionMessage { kind: 2, opaque_bytes: 3, .. }"
    );
}

#[test]
fn concatenated_bodies_need_separate_envelope_boundaries() {
    let first = [0x80, 0, 0, 0x10, 0x10, 0x20, 0x30, 0x40];
    let second = [0x80, 0, 0, 0x90];
    let joined = [first.as_slice(), second.as_slice()].concat();
    assert_eq!(Decoded::decode(&joined), Err(Error::Length));
    assert!(matches!(
        Decoded::decode(&joined[..8]).unwrap().message,
        Message::Host1 { .. }
    ));
    assert_eq!(
        Decoded::decode(&joined[8..]).unwrap().message,
        Message::Host9
    );
}

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use super::*;

fn hex(text: &str) -> Vec<u8> {
    text.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}
fn registrations() -> Registrations {
    Registrations {
        header: 6,
        entries: vec![
            Registration {
                handle: 0,
                next: 1,
                region: 0,
                word: 4,
                byte: 7,
                flag: false,
                enum3: 2,
                word2: 0xaabbccdd,
                bit: true,
                enum2: 1,
                word3: 0x12345678,
                text: b"r".to_vec(),
            },
            Registration {
                handle: 1,
                next: 2,
                region: 1,
                word: 5,
                byte: 8,
                flag: true,
                enum3: 3,
                word2: 0,
                bit: false,
                enum2: 2,
                word3: 0xffffffff,
                text: b"s".to_vec(),
            },
        ],
    }
}
fn cases() -> Vec<(Message, Vec<u8>)> {
    // Independently packed field-width literals; all names, flags and IDs are constructed.
    vec![
        (
            Message::LoadLevel(LoadLevel {
                level: b"test".to_vec(),
                attributes: vec![(b"x".to_vec(), b"y".to_vec())],
                word: 0x11223344,
                text: b"A".to_vec(),
                flags: [true, false, true],
                entries: vec![(b"e".to_vec(), -2, 3)],
                final_word: 0x55667788,
            }),
            hex("011d195cdd0000000040178005e44488cd100141a0000000200b2ffffffff00000001aab33bc40"),
        ),
        (
            Message::Names(SubLevelNames {
                word: 0x10203040,
                entries: vec![(9, b"ab".to_vec())],
            }),
            hex("1020304000000001000900985880"),
        ),
        (
            Message::Registrations(registrations()),
            hex(
                "00000001800000008000000040000000000101caaaef3376891a2b3c002e40002000400020000000a11600000000bfffffffc01730",
            ),
        ),
    ]
}
#[test]
fn independent_startup_vectors_encode_decode_and_reject_partial_or_joined_messages() {
    for (message, bytes) in cases() {
        let decoded = Message::decode(message.target(), &bytes).unwrap();
        assert_eq!(decoded.message, message);
        assert_eq!(message.encode().unwrap(), bytes);
        assert_eq!(decoded.reencode().unwrap(), bytes);
        for end in 0..bytes.len() {
            assert!(Message::decode(message.target(), &bytes[..end]).is_err());
        }
        assert_eq!(
            Message::decode(
                message.target(),
                &[bytes.as_slice(), bytes.as_slice()].concat()
            ),
            Err(Error::Trailing)
        );
    }
}
#[test]
fn physical_padding_is_preserved_by_decode_and_zeroed_by_encoding() {
    let (_, mut bytes) = cases().pop().unwrap();
    let original = Message::decode(63, &bytes).unwrap();
    let pad = bytes.len() * 8 - original.bits;
    assert!(pad > 0);
    *bytes.last_mut().unwrap() |= (1 << pad) - 1;
    let decoded = Message::decode(63, &bytes).unwrap();
    assert_eq!(decoded.message, original.message);
    assert_eq!(decoded.reencode().unwrap(), bytes);
    assert_ne!(decoded.message.encode().unwrap(), bytes);
}
#[test]
fn limits_reject_oversized_counts_strings_and_invalid_enum_widths() {
    assert_eq!(
        Message::decode(28, &hex("00000000ffffffff")),
        Err(Error::Bound)
    );
    assert_eq!(Message::decode(31, &[]), Err(Error::Unsupported));
    assert_eq!(
        Message::decode(85, &vec![0; MAX_BYTES + 1]),
        Err(Error::Bound)
    );
    let mut names = SubLevelNames {
        word: 0,
        entries: vec![(1, vec![b'x'; MAX_STRING + 1])],
    };
    assert_eq!(Message::Names(names.clone()).encode(), Err(Error::Bound));
    names.entries = vec![(1, vec![b'x'; MAX_STRING]); 3];
    assert_eq!(Message::Names(names).encode(), Err(Error::Bound));
    let mut r = registrations();
    r.header = 1 << 34;
    assert_eq!(
        Message::Registrations(r.clone()).encode(),
        Err(Error::Shape)
    );
    r.header = 6;
    r.entries[0].enum3 = 7;
    assert_eq!(
        Message::Registrations(r.clone()).encode(),
        Err(Error::Shape)
    );
    r.entries[0].enum3 = 3;
    r.entries[0].enum2 = 3;
    assert_eq!(Message::Registrations(r).encode(), Err(Error::Shape));
}
#[test]
fn registration_allocation_is_isolated_transactional_and_bounded() {
    let r = registrations();
    let defs = r
        .entries
        .iter()
        .map(RegistrationDefinition::from_record)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let mut a = RegistrationsState::default();
    let mut b = RegistrationsState::default();
    assert_eq!(
        a.prepare(r.header, &defs).unwrap(),
        Message::Registrations(r.clone())
    );
    assert_eq!(
        b.prepare(r.header, &defs).unwrap(),
        Message::Registrations(r.clone())
    );
    a.prepare(r.header, &defs).unwrap();
    assert_eq!((a.next(), b.next()), (4, 2));
    let mut bad = defs.clone();
    bad[1].enum3 = 7;
    assert_eq!(a.prepare(r.header, &bad), Err(Error::Shape));
    assert_eq!(a.next(), 4);
    let mut end = RegistrationsState::with_next(u16::MAX - 1);
    assert_eq!(end.prepare(r.header, &defs), Err(Error::Bound));
    assert_eq!(end.next(), u32::from(u16::MAX - 1));
    let mut unsupported = r.entries[0].clone();
    unsupported.word += 1;
    assert_eq!(
        RegistrationDefinition::from_record(&unsupported),
        Err(Error::Unsupported)
    );
}
#[test]
fn arbitrary_short_inputs_are_bounded_and_never_panic() {
    let mut seed = 0x1234_5678u32;
    for len in 0..256 {
        let bytes = (0..len)
            .map(|_| {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                (seed >> 24) as u8
            })
            .collect::<Vec<_>>();
        for target in [28, 63, 85] {
            if let Ok(d) = Message::decode(target, &bytes) {
                assert_eq!(d.reencode().unwrap(), bytes);
            }
        }
    }
}

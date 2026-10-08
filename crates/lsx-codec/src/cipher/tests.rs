// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use super::*;

fn bytes(hex: &str) -> Vec<u8> {
    hex.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| nibble(p[0]).unwrap() * 16 + nibble(p[1]).unwrap())
        .collect()
}

fn block(hex: &str) -> [u8; 16] {
    bytes(hex).try_into().unwrap()
}

#[test]
fn nist_sp800_38a_ecb_known_answers() {
    // NIST SP 800-38A F.1.1/F.1.2 (accessed 2026-10-05 for ).
    let aes = Aes128::new(block("2b7e151628aed2a6abf7158809cf4f3c"));
    for (plain, cipher) in [
        (
            "6bc1bee22e409f96e93d7e117393172a",
            "3ad77bb40d7a3660a89ecaf32466ef97",
        ),
        (
            "ae2d8a571e03ac9c9eb76fac45af8e51",
            "f5d3d58503b9699de785895a96fdbaaf",
        ),
        (
            "30c81c46a35ce411e5fbc1191a0a52ef",
            "43b1cd7f598ece23881b00e3ed030688",
        ),
        (
            "f69f2445df4f9b17ad2b417be66c3710",
            "7b0c785e27e8ad3f8223207104725dd4",
        ),
    ] {
        assert_eq!(aes.encrypt(block(plain)), block(cipher));
        assert_eq!(aes.decrypt(block(cipher)), block(plain));
    }
}

#[test]
fn e340_derived_length_vectors_round_trip_with_the_native_statuses() {
    // Synthetic plaintexts under the parameter-zero key, not game bytes.
    let aes = Aes128::parameter_zero();
    for (len, cipher, status) in [
        (0, "", None),
        (1, "dccc6f2b042ab165aab7eceea77c196c", Some(1)),
        (15, "31e33487089e8df9770b33365557aba9", Some(15)),
        (
            16,
            "dd4b1a0b47daa7067d0b59d95d58a6ae954f64f2e4e86e9eee82d20216684899",
            Some(-7),
        ),
        (
            17,
            "dd4b1a0b47daa7067d0b59d95d58a6aedccc6f2b042ab165aab7eceea77c196c",
            Some(17),
        ),
        (
            32,
            "dd4b1a0b47daa7067d0b59d95d58a6aedd4b1a0b47daa7067d0b59d95d58a6ae954f64f2e4e86e9eee82d20216684899",
            Some(-7),
        ),
    ] {
        assert_eq!(aes.encode_hex(&vec![b'A'; len]).unwrap(), cipher);
        let decoded = aes.decode_hex(cipher.as_bytes()).unwrap();
        assert_eq!(decoded.status, status, "length {len}");
        assert_eq!(
            decoded.visible,
            vec![b'A'; len],
            "the wrapper exposes the prefix even when the count-16 pad is rejected"
        );
    }
}

#[test]
fn decoder_edges_match_the_saved_branches() {
    let aes = Aes128::parameter_zero();
    assert_eq!(
        aes.decode_hex(b"z").unwrap().status,
        None,
        "one char: no block"
    );
    let mut cipher = aes.encode_hex(b"A").unwrap();
    cipher.push('z');
    assert_eq!(aes.decode_hex(cipher.as_bytes()).unwrap().visible, b"A");
    assert_eq!(
        aes.decode_hex(b"DCCC6F2B042AB165AAB7ECEEA77C196C")
            .unwrap()
            .visible,
        b"A",
        "uppercase pairs convert"
    );
    for pair in [b"zz", b"0z", b" 1", b"+1", b"-1"] {
        assert_eq!(aes.decode_hex(pair), Err(Error::Hex));
    }
    for len in [1, 15, 17, 31] {
        assert_eq!(
            aes.decode_hex(&vec![b'0'; len * 2]).unwrap(),
            Decoded {
                status: Some(-7),
                visible: vec![]
            },
            "misaligned {len}"
        );
    }
    // Count zero is accepted; the C string ends at that zero byte.
    let mut plain = [b'B'; 16];
    plain[15] = 0;
    let cipher = lower_hex(&aes.encrypt(plain));
    assert_eq!(
        aes.decode_hex(cipher.as_bytes()).unwrap(),
        Decoded {
            status: Some(16),
            visible: vec![b'B'; 15]
        }
    );
    // A bad last block keeps the earlier blocks visible.
    let mut last = [b'B'; 16];
    last[15] = 2;
    let cipher = lower_hex(&[aes.encrypt([b'A'; 16]), aes.encrypt(last)].concat());
    assert_eq!(
        aes.decode_hex(cipher.as_bytes()).unwrap(),
        Decoded {
            status: Some(-7),
            visible: vec![b'A'; 16]
        }
    );
    let cipher = aes.encode_hex(b"AB\0CD").unwrap();
    assert_eq!(
        aes.decode_hex(cipher.as_bytes()).unwrap().visible,
        b"AB",
        "an embedded NUL ends the visible string"
    );
    assert_eq!(aes.encode_hex(&vec![0; MAX_PLAIN + 1]), Err(Error::Bound));
    assert_eq!(aes.decode_hex(&vec![b'0'; MAX_HEX + 1]), Err(Error::Bound));
}

#[test]
fn crt_generator_and_key_recipe_match_e327() {
    let mut r = CrtRandom::seeded(7);
    assert_eq!(r.next_output(), 61, "seed 7 gives 61");
    assert_eq!(r.state, 4_029_102);
    assert_eq!(parameter_key(0), zero_parameter_key());
    assert_eq!(zero_parameter_key()[15], 15);
    // A nonzero parameter: seed 7, output 61, reseed (p + 61), 16 low bytes.
    let p = 0x3361;
    let mut r = CrtRandom::seeded(p + 61);
    let expected: [u8; 16] = std::array::from_fn(|_| r.next_output() as u8);
    assert_eq!(parameter_key(p), expected);
    assert_ne!(parameter_key(p), parameter_key(p + 1));
    assert_eq!(parameter_key(u32::MAX), {
        let mut r = CrtRandom::seeded(u32::MAX.wrapping_add(61));
        std::array::from_fn(|_| r.next_output() as u8)
    });
    // The transport parameter from a reply's first two bytes (signed arithmetic).
    assert_eq!(transport_parameter(b"3a"), (0x33 << 8) + 0x61);
    assert_eq!(transport_parameter(b"a"), 0x61 << 8);
    assert_eq!(transport_parameter(b""), 0);
    assert_eq!(
        transport_parameter(&[0x80, 0x80]),
        ((-128i32 << 8) - 128) as u32
    );
    let aes = Aes128::for_parameter(p);
    let cipher = aes.encode_hex(b"<LSX/>").unwrap();
    assert_eq!(
        aes.decode_hex(cipher.as_bytes()).unwrap().visible,
        b"<LSX/>"
    );
    assert_ne!(
        Aes128::parameter_zero()
            .decode_hex(cipher.as_bytes())
            .unwrap()
            .visible,
        b"<LSX/>"
    );
}

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! independent header literal and branch/cleanup regression tests.
use nfs_protocol::world::{
    BitSpan,
    fragment::{Assembler, Error, Fragment, MAX_FRAGMENT_BITS, MAX_FRAME_BITS, Outcome},
};

fn fragment(
    id: u16,
    ordinal: u8,
    final_fragment: bool,
    bytes: &[u8],
    start: usize,
    len: usize,
) -> Fragment<'_> {
    Fragment::new(
        id,
        ordinal,
        final_fragment,
        BitSpan::new(bytes, start, len).unwrap(),
    )
    .unwrap()
}

#[test]
fn independent_literal_decodes_at_every_alignment_with_exclusive_bounds() {
    // 0x1234:16 / 0x15:6 / 3:15 / final=1 / 101:3, independently packed in.
    let literal = [0x12, 0x34, 0x54, 0x00, 0x1e, 0x80];
    for offset in 0..8 {
        let mut bytes = [0_u8; 7];
        for bit in 0..41 {
            bytes[(offset + bit) / 8] |=
                ((literal[bit / 8] >> (7 - bit % 8)) & 1) << (7 - (offset + bit) % 8);
        }
        let result = Fragment::decode(BitSpan::new(&bytes, offset, 41).unwrap()).unwrap();
        assert_eq!(
            (result.frame(), result.ordinal(), result.is_final()),
            (0x1234, 0x15, true)
        );
        assert_eq!(result.data().len(), 3);
        assert_eq!(result.data().read_u32(0, 3).unwrap(), 5);
        assert_eq!(result.data().start(), offset + 38);
        for length in 0..41 {
            assert!(Fragment::decode(BitSpan::new(&bytes, offset, length).unwrap()).is_err());
        }
        assert_eq!(
            Fragment::decode(BitSpan::new(&bytes, offset, 42).unwrap()),
            Err(Error::FragmentLength)
        );
    }
}

#[test]
fn concatenated_fragment_bodies_require_their_own_exact_ranges() {
    let literal = [0x12, 0x34, 0x54, 0x00, 0x1e, 0x80];
    let mut bytes = [0_u8; 11];
    for offset in [0, 41] {
        for bit in 0..41 {
            bytes[(offset + bit) / 8] |=
                ((literal[bit / 8] >> (7 - bit % 8)) & 1) << (7 - (offset + bit) % 8);
        }
    }
    assert!(Fragment::decode(BitSpan::new(&bytes, 0, 82).unwrap()).is_err());
    for start in [0, 41] {
        assert_eq!(
            Fragment::decode(BitSpan::new(&bytes, start, 41).unwrap())
                .unwrap()
                .data()
                .read_u32(0, 3)
                .unwrap(),
            5
        );
    }
}

#[test]
fn field_construction_rejects_nonwire_ordinals_and_lengths() {
    let empty = BitSpan::new(&[], 0, 0).unwrap();
    for ordinal in 0..=u8::MAX {
        assert_eq!(
            Fragment::new(1, ordinal, false, empty).is_ok(),
            ordinal < 64
        );
    }
    let bytes = [0_u8; 4096];
    assert!(Fragment::new(1, 0, false, BitSpan::new(&bytes, 0, 32767).unwrap()).is_ok());
    assert_eq!(
        Fragment::new(1, 0, false, BitSpan::new(&bytes, 0, 32768).unwrap()),
        Err(Error::FragmentLength)
    );
    assert!(matches!(
        Assembler::new(usize::MAX),
        Err(Error::InvalidLimit)
    ));
    assert!(matches!(
        Assembler::new(MAX_FRAME_BITS + 1),
        Err(Error::InvalidLimit)
    ));
}

#[test]
fn nonbyte_fragments_concatenate_without_padding_or_unrelated_source_bits() {
    let mut state = Assembler::new(8).unwrap();
    // Skip unrelated leading bits: 0101 -> 101; 111011 -> 11011.
    assert!(matches!(
        state.push(fragment(7, 0, false, &[0x50], 1, 3)).unwrap(),
        Outcome::Pending
    ));
    let Outcome::Complete(frame) = state.push(fragment(7, 1, true, &[0xec], 1, 5)).unwrap() else {
        panic!("complete")
    };
    assert_eq!(
        (frame.frame(), frame.fragments(), frame.data().len()),
        (7, 2, 8)
    );
    assert_eq!(frame.data().bytes(), &[0xbb]);
}

#[test]
fn final_flag_delivers_but_does_not_latch_the_reader_closed() {
    let mut state = Assembler::new(16).unwrap();
    let Outcome::Complete(first) = state.push(fragment(7, 0, true, &[0xa0], 0, 3)).unwrap() else {
        panic!("first")
    };
    assert_eq!(first.data().read_u32(0, 3).unwrap(), 5);
    let Outcome::Complete(second) = state.push(fragment(7, 1, true, &[0xd8], 0, 5)).unwrap() else {
        panic!("second")
    };
    assert_eq!(
        (second.data().bytes(), second.fragments()),
        (&[0xbb][..], 2)
    );
    // Not observed in the corpus: this is a code-grounded branch fixture.
}

#[test]
fn invalid_order_discards_until_a_different_identity_starts_at_zero() {
    let mut state = Assembler::new(32).unwrap();
    for (id, ordinal, discarded) in [
        (7, 1, true),
        (7, 0, true),
        (8, 0, false),
        (8, 2, true),
        (8, 1, true),
        (9, 0, false),
        (9, 0, true),
        (8, 0, false),
    ] {
        let result = state
            .push(fragment(id, ordinal, false, &[0x80], 0, 1))
            .unwrap();
        assert_eq!(matches!(result, Outcome::Discarded), discarded);
        if discarded {
            assert_eq!(state.buffered_bits(), 0);
        }
    }
}

#[test]
fn size_error_latches_discard_and_releases_buffer_before_recovery() {
    let mut state = Assembler::new(8).unwrap();
    assert!(matches!(
        state.push(fragment(1, 0, false, &[0xe0], 0, 3)).unwrap(),
        Outcome::Pending
    ));
    assert!(matches!(
        state.push(fragment(1, 1, true, &[0xfc], 0, 6)),
        Err(Error::FrameLimit)
    ));
    assert_eq!(state.buffered_bits(), 0);
    for ordinal in [0, 1, 2] {
        assert!(matches!(
            state.push(fragment(1, ordinal, true, &[], 0, 0)).unwrap(),
            Outcome::Discarded
        ));
    }
    let Outcome::Complete(frame) = state.push(fragment(2, 0, true, &[0], 0, 8)).unwrap() else {
        panic!("recover")
    };
    assert_eq!(frame.data().bytes(), &[0]);
}

#[test]
fn explicit_discard_and_reset_have_different_lifetime_meanings() {
    let mut state = Assembler::new(8).unwrap();
    assert!(matches!(
        state.push(fragment(1, 0, false, &[0xa0], 0, 3)).unwrap(),
        Outcome::Pending
    ));
    state.discard();
    assert_eq!(state.buffered_bits(), 0);
    assert!(matches!(
        state.push(fragment(1, 0, true, &[], 0, 0)).unwrap(),
        Outcome::Discarded
    ));
    state.reset();
    let Outcome::Complete(frame) = state.push(fragment(1, 0, true, &[], 0, 0)).unwrap() else {
        panic!("new lifetime")
    };
    assert!(frame.data().is_empty());
    assert_eq!(frame.fragments(), 1);
}

#[test]
fn connection_isolation_and_frame_wrap_preserve_distinct_data() {
    let mut a = Assembler::new(8).unwrap();
    let mut b = Assembler::new(8).unwrap();
    assert!(matches!(
        a.push(fragment(u16::MAX, 0, false, &[0xa0], 0, 4)).unwrap(),
        Outcome::Pending
    ));
    assert!(matches!(
        b.push(fragment(u16::MAX, 0, false, &[0xf0], 0, 4)).unwrap(),
        Outcome::Pending
    ));
    let Outcome::Complete(frame) = b.push(fragment(u16::MAX, 1, true, &[0x50], 0, 4)).unwrap()
    else {
        panic!("b")
    };
    assert_eq!(frame.data().bytes(), &[0xf5]);
    let Outcome::Complete(frame) = a.push(fragment(0, 0, true, &[0x30], 0, 4)).unwrap() else {
        panic!("a")
    };
    assert_eq!(frame.data().bytes(), &[0x30]);
}

#[test]
fn maximum_wire_frame_and_ordinal_do_not_wrap_into_a_second_frame() {
    let mut state = Assembler::new(MAX_FRAME_BITS).unwrap();
    let data = [0xff; 4096];
    for ordinal in 0..64 {
        let result = state
            .push(fragment(
                0,
                ordinal,
                ordinal == 63,
                &data,
                0,
                MAX_FRAGMENT_BITS,
            ))
            .unwrap();
        if let Outcome::Complete(frame) = result {
            assert_eq!(frame.data().len(), MAX_FRAME_BITS);
            assert_eq!(frame.fragments(), 64);
            assert!(frame.data().bytes().iter().all(|x| *x == 255));
        } else {
            assert!(matches!(result, Outcome::Pending));
        }
    }
    assert!(matches!(
        state.push(fragment(0, 0, true, &[], 0, 0)).unwrap(),
        Outcome::Discarded
    ));
    let mut empty = Assembler::new(0).unwrap();
    assert!(matches!(
        empty.push(fragment(1, 0, true, &[], 0, 0)).unwrap(),
        Outcome::Complete(_)
    ));
    assert!(matches!(
        empty.push(fragment(2, 0, true, &[0], 0, 1)),
        Err(Error::FrameLimit)
    ));
}

#[test]
fn arbitrary_bounded_inputs_never_panic_or_escape_bounds() {
    let mut seed = 0x1f82_31a9_u32;
    let mut assembler = Assembler::new(256).unwrap();
    for _ in 0..10_000 {
        let mut bytes = [0_u8; 24];
        for byte in &mut bytes {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            *byte = (seed >> 24) as u8;
        }
        let start = seed as usize % 8;
        let len = (seed >> 4) as usize % (bytes.len() * 8 - start + 1);
        if let Ok(fragment) = Fragment::decode(BitSpan::new(&bytes, start, len).unwrap()) {
            let _result = assembler.push(fragment);
        }
        assert!(assembler.buffered_bits() <= 256);
    }
}

#[test]
fn debug_hides_payload_and_frame_identity() {
    let f = fragment(57005, 23, true, b"secret", 0, 48);
    let mut state = Assembler::new(64).unwrap();
    let complete = state
        .push(fragment(57005, 0, true, b"secret", 0, 48))
        .unwrap();
    let text = format!("{f:?} {complete:?}");
    for forbidden in ["secret", "57005", "dead"] {
        assert!(!text.contains(forbidden));
    }
    assert!(!format!("{state:?}").contains("57005"));
}

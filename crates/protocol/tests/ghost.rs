//! Synthetic, independently laid-out fixtures; no captured bytes or identities.
//! : 1 + optional 31 + 1 + 13 header bits, then leading (1 + 13)
//! deletions, or a single 0 before nonempty setup. See world/ghost.rs provenance.
use nfs_protocol::world::{
    BitSpan, Error,
    ghost::{Limits, Prefix, Profile},
};

const LIMITS: Limits = Limits {
    max_input_bits: 65_536,
    max_records: 8191,
    max_deletions: 8191,
};
const PROFILE: Profile = Profile::NFS16_92AA6FF4;
// Header 0|0|0000000000000, then a suffix 1 which must remain untouched.
const EMPTY: &[u8] = &[0x00, 0x01];
// 0|1|0000000000001|1|1001000110100, then suffix 101.
const DELETE: &[u8] = &[0x40, 0x03, 0x91, 0xa5];
// 0|0|0000000000011|1|0000000000000|1|1111111111111|0, suffix 10110.
const MIXED: &[u8] = &[0x00, 0x07, 0x00, 0x07, 0xff, 0xeb, 0x00];
// 1|0111111100000000000000000000000|1|0000000000001|0, suffix 101011001.
const FLOAT: &[u8] = &[0xbf, 0x80, 0x00, 0x00, 0x80, 0x05, 0x59];

fn decode(bytes: &[u8], valid_bits: usize) -> Prefix<'_> {
    Prefix::decode(BitSpan::new(bytes, 0, valid_bits).unwrap(), PROFILE, LIMITS).unwrap()
}

// Test-only packer of literal bit strings, independent of production decoding.
fn pack(text: &str) -> Vec<u8> {
    let bits: Vec<_> = text.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    let mut bytes = vec![0; bits.len().div_ceil(8)];
    for (i, bit) in bits.into_iter().enumerate() {
        assert!(bit == b'0' || bit == b'1');
        bytes[i / 8] |= (bit - b'0') << (7 - i % 8);
    }
    bytes
}

#[test]
fn zero_count_has_no_deletion_sentinel() {
    let prefix = decode(EMPTY, 16);
    assert_eq!(prefix.float_bits(), None);
    assert!(!prefix.flag());
    assert_eq!(prefix.record_count(), 0);
    assert_eq!(prefix.remaining_records(), 0);
    assert!(prefix.deleted().is_empty());
    assert_eq!(prefix.remaining().start(), 15);
    assert_eq!(prefix.remaining().read_u32(0, 1), Ok(1));
    assert!(decode(EMPTY, 15).remaining().is_empty());
}

#[test]
fn all_deleted_has_no_stop_bit_and_preserves_partial_byte() {
    let prefix = decode(DELETE, 32);
    assert!(prefix.flag());
    assert_eq!(prefix.record_count(), 1);
    assert_eq!(prefix.deleted(), [0x1234]);
    assert_eq!(prefix.remaining_records(), 0);
    assert_eq!(prefix.remaining().start(), 29);
    assert_eq!(prefix.remaining().read_u32(0, 3), Ok(0b101));
    assert!(decode(DELETE, 29).remaining().is_empty());
}

#[test]
fn mixed_records_consume_one_stop_bit_before_opaque_setup() {
    let prefix = decode(MIXED, 49);
    assert_eq!(prefix.record_count(), 3);
    assert_eq!(prefix.deleted(), [0, 8191]);
    assert_eq!(prefix.remaining_records(), 1);
    assert_eq!(prefix.remaining().start(), 44);
    assert_eq!(prefix.remaining().len(), 5);
    assert_eq!(prefix.remaining().read_u32(0, 5), Ok(0b10110));
    assert!(std::ptr::eq(prefix.remaining().bytes(), MIXED));
    // A valid prefix can end before the missing setup/body; it is not full validation.
    assert!(decode(MIXED, 44).remaining().is_empty());
}

#[test]
fn optional_float_uses_31_bits_without_numeric_conversion() {
    let prefix = decode(FLOAT, 56);
    assert_eq!(prefix.float_bits(), Some(0x3f80_0000));
    assert!(prefix.flag());
    assert_eq!(prefix.record_count(), 1);
    assert_eq!(prefix.remaining().start(), 47);
    assert_eq!(prefix.remaining().read_u32(0, 9), Ok(0b101011001));
    // Preserve all bit patterns; no NaN rejection/canonicalization or extra sign bit.
    let prefix = decode(&[0xff, 0xff, 0xff, 0xff, 0x00, 0x03], 48);
    assert_eq!(prefix.float_bits(), Some(0x7fff_ffff));
    assert!(!prefix.flag());
    assert_eq!(prefix.record_count(), 0);
    assert_eq!(prefix.remaining().start(), 46);
}

#[test]
fn every_truncated_prefix_fails_without_consuming_an_external_cursor() {
    for (bytes, required) in [(EMPTY, 15), (DELETE, 29), (MIXED, 44), (FLOAT, 47)] {
        for len in 0..required {
            let input = BitSpan::new(bytes, 0, len).unwrap();
            assert!(matches!(
                Prefix::decode(input, PROFILE, LIMITS),
                Err(Error::Truncated { .. })
            ));
            assert_eq!(input.len(), len);
        }
        assert!(decode(bytes, required).remaining().is_empty());
    }
}

#[test]
fn every_bit_alignment_preserves_unknown_suffix() {
    let text = "0000000000000111000000000000011111111111111010110";
    for start in 0..8 {
        let bytes = pack(&format!("{}{text}11111111", "1".repeat(start)));
        let input = BitSpan::new(&bytes, start, text.len()).unwrap();
        let prefix = Prefix::decode(input, PROFILE, LIMITS).unwrap();
        assert_eq!(prefix.deleted(), [0, 8191]);
        assert_eq!(prefix.remaining().start(), start + 44);
        assert_eq!(prefix.remaining().len(), 5);
        assert_eq!(prefix.remaining().read_u32(0, 5), Ok(0b10110));
        assert!(matches!(
            prefix.remaining().read_u32(0, 6),
            Err(Error::Truncated { .. })
        ));
    }
}

#[test]
fn adjacent_sections_are_not_implicitly_byte_aligned() {
    let bytes = pack("000000000000000 01000000000000111001000110100 101");
    let first = decode(&bytes, 47);
    let second = Prefix::decode(first.remaining(), PROFILE, LIMITS).unwrap();
    assert_eq!(second.deleted(), [0x1234]);
    assert_eq!(second.remaining().start(), 44);
    assert_eq!(second.remaining().read_u32(0, 3), Ok(5));
}

#[test]
fn bounded_input_count_and_deletion_allocation_are_separate_policies() {
    let input = BitSpan::new(MIXED, 0, 49).unwrap();
    assert_eq!(
        Prefix::decode(
            input,
            PROFILE,
            Limits {
                max_input_bits: 48,
                ..LIMITS
            }
        ),
        Err(Error::InputLimit)
    );
    assert_eq!(
        Prefix::decode(
            input,
            PROFILE,
            Limits {
                max_records: 2,
                ..LIMITS
            }
        ),
        Err(Error::RecordLimit)
    );
    assert_eq!(
        Prefix::decode(
            input,
            PROFILE,
            Limits {
                max_deletions: 1,
                ..LIMITS
            }
        ),
        Err(Error::DeletionLimit)
    );
    assert!(
        Prefix::decode(
            input,
            PROFILE,
            Limits {
                max_input_bits: 49,
                max_records: 3,
                max_deletions: 2
            }
        )
        .is_ok()
    );
    let empty = BitSpan::new(EMPTY, 0, 15).unwrap();
    assert!(
        Prefix::decode(
            empty,
            PROFILE,
            Limits {
                max_records: 0,
                max_deletions: 0,
                ..LIMITS
            }
        )
        .is_ok()
    );
    // Nonempty body with no deletions requires no deletion allocation.
    let nonempty = BitSpan::new(FLOAT, 0, 47).unwrap();
    assert!(
        Prefix::decode(
            nonempty,
            PROFILE,
            Limits {
                max_deletions: 0,
                ..LIMITS
            }
        )
        .is_ok()
    );
}

#[test]
fn explicit_profiles_cover_width_edges_without_unbounded_count_allocation() {
    assert_eq!(Profile::new(13, 13), Ok(PROFILE));
    for (count, id) in [(0, 13), (33, 13), (13, 0), (13, 33)] {
        assert_eq!(Profile::new(count, id), Err(Error::InvalidWidth));
    }
    let narrow = pack("0 1 1 1 1");
    let p = Prefix::decode(
        BitSpan::new(&narrow, 0, 5).unwrap(),
        Profile::new(1, 1).unwrap(),
        LIMITS,
    )
    .unwrap();
    assert_eq!(p.deleted(), [1]);
    let huge = pack("0 0 11111111111111111111111111111111 0");
    let span = BitSpan::new(&huge, 0, 35).unwrap();
    let wide = Profile::new(32, 32).unwrap();
    assert_eq!(Prefix::decode(span, wide, LIMITS), Err(Error::RecordLimit));
    let p = Prefix::decode(
        span,
        wide,
        Limits {
            max_records: u32::MAX,
            max_deletions: 0,
            ..LIMITS
        },
    )
    .unwrap();
    assert_eq!(p.remaining_records(), u32::MAX);
    assert!(p.deleted().is_empty());
    let large_id = pack("0 0 00000000000000000000000000000001 1 11111111111111111111111111111111");
    let p = Prefix::decode(BitSpan::new(&large_id, 0, 67).unwrap(), wide, LIMITS).unwrap();
    assert_eq!(p.deleted(), [u32::MAX]);
}

#[test]
fn ranges_and_reads_reject_overflow_and_bytes_outside_valid_bits() {
    assert_eq!(BitSpan::new(&[], 0, 1), Err(Error::InvalidBitRange));
    assert_eq!(
        BitSpan::new(EMPTY, usize::MAX, 1),
        Err(Error::InvalidBitRange)
    );
    assert_eq!(
        BitSpan::new(EMPTY, 1, usize::MAX),
        Err(Error::InvalidBitRange)
    );
    assert_eq!(BitSpan::new(EMPTY, 17, 0), Err(Error::InvalidBitRange));
    assert!(BitSpan::new(EMPTY, 16, 0).unwrap().is_empty());
    let input = BitSpan::new(EMPTY, 1, 3).unwrap();
    assert_eq!(input.read_u32(0, 33), Err(Error::InvalidWidth));
    assert_eq!(input.read_u32(3, 0), Ok(0));
    assert!(matches!(
        input.read_u32(usize::MAX, 1),
        Err(Error::Truncated { .. })
    ));
    assert_eq!(input.after(usize::MAX), Err(Error::InvalidBitRange));
    assert!(matches!(input.read_u32(3, 1), Err(Error::Truncated { .. })));
}

#[test]
fn decoding_separate_connections_has_no_global_state() {
    let first = decode(DELETE, 32);
    let second = decode(EMPTY, 16);
    let third = decode(DELETE, 32);
    assert_eq!(first, third);
    assert!(second.deleted().is_empty());
    assert_eq!(first.deleted(), [0x1234]);
}

#[test]
fn debug_omits_payload_values_and_object_ids() {
    let prefix = decode(DELETE, 32);
    let text = format!("{prefix:?}");
    assert!(!text.contains("4660"));
    assert!(!text.contains("bytes"));
    assert!(text.contains("deletions: 1"));
}

#[test]
fn bounded_mutated_inputs_preserve_invariants_without_panics() {
    let mut state = 0x9e37_79b9u32;
    for i in 0..10_000 {
        let mut bytes = [0; 32];
        for byte in &mut bytes {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            *byte = state as u8;
        }
        let start = i % 8;
        let len = (state as usize % 249).min(256 - start);
        let input = BitSpan::new(&bytes, start, len).unwrap();
        if let Ok(p) = Prefix::decode(
            input,
            PROFILE,
            Limits {
                max_deletions: 8,
                ..LIMITS
            },
        ) {
            assert!(p.deleted().len() <= 8);
            assert!(p.record_count() >= p.deleted().len() as u32);
            assert!(p.remaining().start() >= start + 15);
            assert_eq!(p.remaining().start() + p.remaining().len(), start + len);
            assert!(std::ptr::eq(p.remaining().bytes(), input.bytes()));
        }
    }
}

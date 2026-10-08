//! synthetic bit layouts grounded by reader branches.
//! No capture bytes. Values are preserved as encoded, not interpreted as positions.
use nfs_protocol::world::{
    BitSpan, Error,
    ghost::{Limits, Prefix, Profile, Setup, SetupProfile},
};

const LIMITS: Limits = Limits {
    max_input_bits: 4096,
    max_records: 8191,
    max_deletions: 8191,
};
const PACKED: SetupProfile = SetupProfile::ClientReceivePacked;
const RAW: SetupProfile = SetupProfile::ClientReceiveForcedRaw;
const SEND: SetupProfile = SetupProfile::ClientSend;

fn pack(text: &str) -> Vec<u8> {
    let bits: Vec<_> = text.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    let mut bytes = vec![0; bits.len().div_ceil(8)];
    for (i, bit) in bits.into_iter().enumerate() {
        assert!(bit == b'0' || bit == b'1');
        bytes[i / 8] |= (bit - b'0') << (7 - i % 8);
    }
    bytes
}

fn fixture(setup: &str) -> (Vec<u8>, usize) {
    let setup: String = setup.chars().filter(|c| !c.is_whitespace()).collect();
    // No float, flag false, count 1, stop false; ID 0x1234, creation true; suffix 101.
    let bits = format!("0000000000000010{setup}10010001101001101");
    (pack(&bits), bits.len())
}

fn prefix(bytes: &[u8], start: usize, len: usize) -> Prefix<'_> {
    Prefix::decode(
        BitSpan::new(bytes, start, len).unwrap(),
        Profile::NFS16_92AA6FF4,
        LIMITS,
    )
    .unwrap()
}

#[test]
fn absent_axes_and_send_have_different_exact_boundaries() {
    let (bytes, len) = fixture("000");
    // Literal independent byte layout fixes the test packer's meaning as well.
    assert_eq!(bytes, [0x00, 0x02, 0x12, 0x34, 0xd0]);
    let first = prefix(&bytes, 0, len)
        .first_record(PACKED)
        .unwrap()
        .unwrap();
    assert_eq!(
        first.setup(),
        Setup::Packed {
            tag: None,
            width: None,
            axes: [None; 3]
        }
    );
    assert_eq!(first.object_id(), 0x1234);
    assert!(first.is_creation());
    assert_eq!(first.body().start(), 33);
    assert_eq!(first.body().read_u32(0, 3), Ok(5));
    let (bytes, len) = fixture("");
    let first = prefix(&bytes, 0, len).first_record(SEND).unwrap().unwrap();
    assert_eq!(first.setup(), Setup::ContextOnly);
    assert_eq!(first.object_id(), 0x1234);
    assert_eq!(first.body().start(), 30);
}

#[test]
fn all_packed_width_tags_apply_once_and_are_reused_for_later_axes() {
    for (tag, width) in [(1, 1), (2, 3), (3, 5), (4, 7), (5, 9), (6, 11), (7, 14)] {
        let setup = format!(
            "1{tag:03b}{}1{}1{}",
            "1".repeat(width),
            "0".repeat(width),
            "1".repeat(width)
        );
        let (bytes, len) = fixture(&setup);
        let first = prefix(&bytes, 0, len)
            .first_record(PACKED)
            .unwrap()
            .unwrap();
        let value = (1 << width) - 1;
        assert_eq!(
            first.setup(),
            Setup::Packed {
                tag: Some(tag),
                width: Some(width as u8),
                axes: [Some(value), Some(0), Some(value)],
            }
        );
        assert_eq!(first.body().start(), 36 + 3 * width);
        assert_eq!(first.object_id(), 0x1234);
    }
}

#[test]
fn tag_zero_on_first_y_or_z_is_width_one_not_raw_escape() {
    for (setup, axes) in [
        ("0 1 000 1 0", [None, Some(1), None]),
        ("0 0 1 000 1", [None, None, Some(1)]),
    ] {
        let (bytes, len) = fixture(setup);
        let first = prefix(&bytes, 0, len)
            .first_record(PACKED)
            .unwrap()
            .unwrap();
        assert_eq!(
            first.setup(),
            Setup::Packed {
                tag: Some(0),
                width: Some(1),
                axes
            }
        );
        assert_eq!(first.body().start(), 37);
        assert_eq!(first.object_id(), 0x1234);
    }
}

#[test]
fn absent_early_axes_delay_the_single_width_tag() {
    let (bytes, len) = fixture("0 1 010 101 1 010");
    let first = prefix(&bytes, 0, len)
        .first_record(PACKED)
        .unwrap()
        .unwrap();
    assert_eq!(
        first.setup(),
        Setup::Packed {
            tag: Some(2),
            width: Some(3),
            axes: [None, Some(5), Some(2)]
        }
    );
    assert_eq!(first.body().start(), 42);
    assert_eq!(first.body().read_u32(0, 3), Ok(5));
}

#[test]
fn x_tag_zero_reads_three_unconditional_words_and_no_more_presence_bits() {
    let (bytes, len) = fixture(
        "1 000 01111111110000000000000000000000 10000000000000000000000000000000 11011110101011011011111011101111",
    );
    let first = prefix(&bytes, 0, len)
        .first_record(PACKED)
        .unwrap()
        .unwrap();
    assert_eq!(
        first.setup(),
        Setup::RawEscape([0x7fc0_0000, 0x8000_0000, 0xdead_beef])
    );
    assert_eq!(first.body().start(), 130);
    assert_eq!(first.object_id(), 0x1234);
    assert_eq!(first.body().read_u32(0, 3), Ok(5));
}

#[test]
fn forced_raw_has_one_presence_bit_per_axis_and_no_width_tag() {
    let (bytes, len) =
        fixture("1 01111111110000000000000000000000 0 1 11011110101011011011111011101111");
    let first = prefix(&bytes, 0, len).first_record(RAW).unwrap().unwrap();
    assert_eq!(
        first.setup(),
        Setup::ForcedRaw([Some(0x7fc0_0000), None, Some(0xdead_beef)])
    );
    assert_eq!(first.body().start(), 97);
    assert_eq!(first.object_id(), 0x1234);
    let (bytes, len) = fixture("000");
    assert_eq!(
        prefix(&bytes, 0, len)
            .first_record(RAW)
            .unwrap()
            .unwrap()
            .setup(),
        Setup::ForcedRaw([None; 3])
    );
}

#[test]
fn zero_and_all_deleted_counts_never_consume_setup_or_next_handler() {
    for bytes in [vec![0x00, 0x01], vec![0x40, 0x03, 0x91, 0xa5]] {
        let p = prefix(&bytes, 0, bytes.len() * 8);
        for profile in [PACKED, RAW, SEND] {
            assert_eq!(p.first_record(profile), Ok(None));
        }
    }
}

#[test]
fn all_truncated_setup_and_header_boundaries_fail_without_losing_prefix() {
    for (setup, profile) in [
        ("1 111 11111111111111 1 00000000000000 0", PACKED),
        (
            "1 000 01111111110000000000000000000000 10000000000000000000000000000000 11011110101011011011111011101111",
            PACKED,
        ),
        ("0 1 01111111110000000000000000000000 0", RAW),
        ("", SEND),
    ] {
        let (bytes, len) = fixture(setup);
        let end = len - 3;
        for valid in 16..end {
            let p = prefix(&bytes, 0, valid);
            assert!(matches!(
                p.first_record(profile),
                Err(Error::Truncated { .. })
            ));
            assert_eq!(p.remaining().start(), 16);
        }
        let first = prefix(&bytes, 0, end)
            .first_record(profile)
            .unwrap()
            .unwrap();
        assert!(first.body().is_empty());
    }
}

#[test]
fn unaligned_first_record_uses_the_prefix_id_width_and_preserves_suffix() {
    for start in 0..8 {
        // Explicit one-bit count/ID fixture, no setup, ID 1, creation false, suffix 011.
        let bits = format!("{}001010011", "1".repeat(start));
        let bytes = pack(&bits);
        let p = Prefix::decode(
            BitSpan::new(&bytes, start, 9).unwrap(),
            Profile::new(1, 1).unwrap(),
            LIMITS,
        )
        .unwrap();
        let first = p.first_record(SEND).unwrap().unwrap();
        assert_eq!(first.object_id(), 1);
        assert!(!first.is_creation());
        assert_eq!(first.body().start(), start + 6);
        assert_eq!(first.body().read_u32(0, 3), Ok(3));
    }
}

#[test]
fn debug_omits_id_and_encoded_axis_values() {
    let (bytes, len) = fixture(
        "1 000 01111111110000000000000000000000 10000000000000000000000000000000 11011110101011011011111011101111",
    );
    let first = prefix(&bytes, 0, len)
        .first_record(PACKED)
        .unwrap()
        .unwrap();
    let text = format!("{first:?}");
    assert!(!text.contains("4660"));
    assert!(!text.contains("3735928559"));
    assert!(!text.contains("bytes"));
}

#[test]
fn random_bounded_suffixes_cannot_overrun_or_mutate_the_prefix() {
    let mut state = 0x1289_3571u32;
    for i in 0..10_000 {
        let mut bytes = [0u8; 24];
        bytes[1] = 2; // the independent count-one/stop-false header
        for byte in &mut bytes[2..] {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            *byte = state as u8;
        }
        let valid = 16 + state as usize % 177;
        let p = prefix(&bytes, 0, valid);
        let profile = [PACKED, RAW, SEND][i % 3];
        if let Ok(Some(first)) = p.first_record(profile) {
            assert_eq!(first.body().start() + first.body().len(), valid);
            assert!(first.body().start() >= 30);
            assert!(first.object_id() < 8192);
        }
        assert_eq!(p.remaining().start(), 16);
        assert_eq!(p.remaining_records(), 1);
    }
}

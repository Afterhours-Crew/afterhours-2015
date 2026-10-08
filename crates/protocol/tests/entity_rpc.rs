//! synthetic type-47 fixtures, independent of any production encoder.
use nfs_protocol::world::{
    BitSpan, Error,
    rpc::{Envelope, Limits, RouteProfile},
};

const LIMITS: Limits = Limits {
    max_input_bits: 8192,
    max_references: 255,
    max_payload_bytes: 256,
};
// Words 11223344/55667788; refs 1234,3; 7 bytes 80aaa0000000f5;
// route selector 101, serial 155, method 7, argument bits 10101; outer suffix 10110.
const FIXTURE: &[u8] = &[
    0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x02, 0x91, 0xa0, 0x00, 0xc0, 0xf0, 0x15, 0x54,
    0x00, 0x00, 0x00, 0x1e, 0xb6,
];

fn pack(bits: &str) -> Vec<u8> {
    let mut bytes = vec![0; bits.len().div_ceil(8)];
    for (i, b) in bits.bytes().enumerate() {
        assert!(b == b'0' || b == b'1');
        bytes[i / 8] |= (b - b'0') << (7 - i % 8);
    }
    bytes
}
fn fixture(refs: &[u16], payload: &[u8]) -> (Vec<u8>, usize) {
    let mut text = format!(
        "{:032b}{:032b}{:08b}",
        0x1122_3344u32,
        0x5566_7788u32,
        refs.len()
    );
    for id in refs {
        text.push_str(&format!("{id:013b}"));
    }
    text.push_str(&format!("{:09b}", payload.len()));
    for byte in payload {
        text.push_str(&format!("{byte:08b}"));
    }
    (pack(&text), text.len())
}
fn decode(bytes: &[u8], bits: usize) -> Envelope<'_> {
    Envelope::decode(BitSpan::new(bytes, 0, bits).unwrap(), LIMITS).unwrap()
}

#[test]
fn literal_receive_fixture_preserves_words_side_arguments_and_both_suffixes() {
    let envelope = decode(FIXTURE, 168);
    assert_eq!(envelope.words(), [0x1122_3344, 0x5566_7788]);
    assert_eq!(envelope.references(), [0x1234, 3]);
    assert_eq!(envelope.payload().start(), 107);
    assert_eq!(envelope.payload().len(), 56);
    assert_eq!(envelope.remaining().start(), 163);
    assert_eq!(envelope.remaining().read_u32(0, 5), Ok(0b10110));
    let route = envelope.route(RouteProfile::ClientReceive).unwrap();
    assert_eq!(route.selector(), 0x101);
    assert_eq!(route.serial().map(|serial| serial.value()), Some(0x155));
    assert_eq!(route.method_index(), 7);
    assert_eq!(route.arguments().start(), 158);
    assert_eq!(route.arguments().len(), 5);
    assert_eq!(route.arguments().read_u32(0, 5), Ok(0b10101));
    assert!(route.arguments().read_u32(0, 6).is_err());
    assert!(std::ptr::eq(route.arguments().bytes(), FIXTURE));
}

#[test]
fn send_profile_omits_the_serial_and_retains_unknown_method_index() {
    // Nine selector bits, full 32-bit unknown method, seven opaque argument bits.
    let payload = pack("100101010111111111111111111111111111111111010101");
    let (bytes, bits) = fixture(&[1], &payload);
    let envelope = decode(&bytes, bits);
    let route = envelope.route(RouteProfile::ClientSend).unwrap();
    assert_eq!(route.selector(), 0x12a);
    assert_eq!(route.serial(), None);
    assert_eq!(route.method_index(), u32::MAX);
    assert_eq!(route.arguments().read_u32(0, 7), Ok(0x55));
    assert_eq!(route.arguments().len(), 7);
    assert!(envelope.route(RouteProfile::ClientReceive).is_err());
}

#[test]
fn zero_length_or_absent_targets_are_envelope_syntax_not_dispatch() {
    for refs in [&[][..], &[0][..], &[0, 0, 1][..]] {
        let (bytes, bits) = fixture(refs, &[]);
        let envelope = decode(&bytes, bits);
        assert_eq!(envelope.references(), refs);
        assert!(envelope.payload().is_empty());
        assert!(envelope.route(RouteProfile::ClientReceive).is_err());
    }
    let (bytes, bits) = fixture(&[], &[0; 7]);
    let envelope = decode(&bytes, bits);
    assert!(envelope.references().is_empty());
    assert_eq!(
        envelope
            .route(RouteProfile::ClientReceive)
            .unwrap()
            .method_index(),
        0
    );
}

#[test]
fn every_bit_truncation_fails_and_payload_cannot_read_following_data() {
    for valid in 0..163 {
        let input = BitSpan::new(FIXTURE, 0, valid).unwrap();
        assert!(Envelope::decode(input, LIMITS).is_err());
        assert_eq!(input.len(), valid);
    }
    assert!(decode(FIXTURE, 163).remaining().is_empty());
    for size in 0..7 {
        let (mut bytes, bits) = fixture(&[1], &vec![0; size]);
        bytes.extend_from_slice(&[0xff; 8]);
        let envelope = decode(&bytes, bits + 32);
        assert!(envelope.route(RouteProfile::ClientReceive).is_err());
        assert_eq!(envelope.remaining().len(), 32);
    }
}

#[test]
fn unaligned_and_concatenated_envelopes_keep_exact_boundaries() {
    let text: String = FIXTURE.iter().map(|byte| format!("{byte:08b}")).collect();
    let exact = &text[..163];
    for start in 0..8 {
        let bits = format!("{}{exact}{exact}10101", "1".repeat(start));
        let bytes = pack(&bits);
        let first = Envelope::decode(
            BitSpan::new(&bytes, start, bits.len() - start).unwrap(),
            LIMITS,
        )
        .unwrap();
        let second = Envelope::decode(first.remaining(), LIMITS).unwrap();
        assert_eq!(first.references(), second.references());
        assert_eq!(second.remaining().start(), start + 326);
        assert_eq!(second.remaining().read_u32(0, 5), Ok(0b10101));
        assert_eq!(
            second
                .route(RouteProfile::ClientReceive)
                .unwrap()
                .arguments()
                .read_u32(0, 5),
            Ok(0b10101)
        );
    }
}

#[test]
fn resource_limits_and_256_byte_capacity_are_enforced_before_payload_access() {
    let input = BitSpan::new(FIXTURE, 0, 168).unwrap();
    assert_eq!(
        Envelope::decode(
            input,
            Limits {
                max_input_bits: 167,
                ..LIMITS
            }
        ),
        Err(Error::InputLimit)
    );
    assert_eq!(
        Envelope::decode(
            input,
            Limits {
                max_references: 1,
                ..LIMITS
            }
        ),
        Err(Error::ReferenceLimit)
    );
    assert_eq!(
        Envelope::decode(
            input,
            Limits {
                max_payload_bytes: 6,
                ..LIMITS
            }
        ),
        Err(Error::PayloadLimit)
    );
    let (bytes, bits) = fixture(&[8191; 255], &[0xa5; 256]);
    let envelope = decode(&bytes, bits);
    assert_eq!(envelope.references(), [8191; 255]);
    assert_eq!(envelope.payload().len(), 2048);
    for size in [257, 511] {
        let (bytes, bits) = fixture(&[], &vec![0; size]);
        assert_eq!(
            Envelope::decode(
                BitSpan::new(&bytes, 0, bits).unwrap(),
                Limits {
                    max_payload_bytes: usize::MAX,
                    ..LIMITS
                }
            ),
            Err(Error::PayloadLimit)
        );
    }
}

#[test]
fn bit_subranges_reject_overflow_and_do_not_expose_neighbours() {
    let span = BitSpan::new(FIXTURE, 5, 100).unwrap();
    assert_eq!(span.slice(usize::MAX, 0), Err(Error::InvalidBitRange));
    assert_eq!(span.slice(1, usize::MAX), Err(Error::InvalidBitRange));
    assert_eq!(span.slice(101, 0), Err(Error::InvalidBitRange));
    assert!(span.slice(100, 0).unwrap().is_empty());
    let one = span.slice(12, 1).unwrap();
    assert_eq!(one.start(), 17);
    assert_eq!(one.len(), 1);
    assert!(one.read_u32(0, 2).is_err());
}

#[test]
fn debug_output_never_contains_words_ids_selectors_serials_or_arguments() {
    let envelope = decode(FIXTURE, 168);
    let text = format!(
        "{envelope:?} {:?}",
        envelope.route(RouteProfile::ClientReceive).unwrap()
    );
    for sensitive in [
        "287454020",
        "1432778632",
        "4660",
        "257",
        "341",
        "bytes",
        "10101",
    ] {
        assert!(!text.contains(sensitive));
    }
}

#[test]
fn bounded_random_inputs_and_routes_do_not_panic_or_escape_their_ranges() {
    let mut state = 0x9435_a158u32;
    for i in 0..10_000 {
        let mut bytes = [0; 96];
        for byte in &mut bytes {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            *byte = state as u8;
        }
        let start = i % 8;
        let len = state as usize % (769 - start);
        let input = BitSpan::new(&bytes, start, len).unwrap();
        if let Ok(envelope) = Envelope::decode(input, LIMITS) {
            assert_eq!(
                envelope.remaining().start() + envelope.remaining().len(),
                start + len
            );
            assert!(envelope.payload().len() <= 2048);
            for profile in [RouteProfile::ClientReceive, RouteProfile::ClientSend] {
                if let Ok(route) = envelope.route(profile) {
                    assert_eq!(
                        route.arguments().start() + route.arguments().len(),
                        envelope.remaining().start()
                    );
                }
            }
        }
    }
}

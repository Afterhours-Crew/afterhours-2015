//! : independent literals and direct transcription of the receive
//! branch arithmetic, not round trips through a production encoder.
use nfs_protocol::world::{
    BitSpan, Error,
    rpc::{Initialization, Serial},
};

fn serial(value: u16) -> Serial {
    Serial::new(value).unwrap()
}

fn pack(bits: &str) -> Vec<u8> {
    let mut bytes = vec![0; bits.len().div_ceil(8)];
    for (index, bit) in bits.bytes().enumerate() {
        assert!(bit == b'0' || bit == b'1');
        bytes[index / 8] |= (bit - b'0') << (7 - index % 8);
    }
    bytes
}

#[test]
fn constructor_rejects_every_value_outside_the_wire_width() {
    for value in 0..=u16::MAX {
        match Serial::new(value) {
            Some(result) => {
                assert!(value < 1024);
                assert_eq!(result.value(), value);
            }
            None => assert!(value >= 1024),
        }
    }
}

#[test]
fn all_serial_pairs_match_the_code_backed_branch_arithmetic() {
    let mut accepted = 0;
    for current in 0..1024 {
        for incoming in 0..1024 {
            // Subtract, conditionally add 1024, compare 512.
            // Independent of production's unsigned wrapping/masking operation.
            let mut delta = i32::from(incoming) - i32::from(current);
            if current > incoming {
                delta += 1024;
            }
            let result = serial(current).accepts(serial(incoming));
            assert_eq!(
                result,
                delta < 512,
                "current={current}, incoming={incoming}"
            );
            accepted += usize::from(result);
        }
    }
    assert_eq!(accepted, 524_288);
    assert!(serial(1023).accepts(serial(0)));
    assert!(serial(0).accepts(serial(0)));
    assert!(serial(0).accepts(serial(511)));
    assert!(!serial(0).accepts(serial(512)));
    assert!(!serial(0).accepts(serial(1023)));
}

#[test]
fn every_initialization_successor_skips_zero_without_mutating_current() {
    for value in 0..1024 {
        let current = serial(value);
        let expected = match value {
            1023 => 1,
            _ => value + 1,
        };
        assert_eq!(current.next_initialization().value(), expected);
        assert_eq!(current.value(), value);
        assert!(current.accepts(current.next_initialization()));
    }
}

#[test]
fn literal_initializer_has_nine_selector_and_ten_serial_bits() {
    // 100000001 | 0101010101 | 10101 = selector 257, serial 341, opaque tail.
    let bytes = [0x80, 0xaa, 0xb5];
    let init = Initialization::decode(BitSpan::new(&bytes, 0, 24).unwrap()).unwrap();
    assert_eq!(init.selector(), 257);
    assert_eq!(init.serial(), serial(341));
    assert_eq!(init.remaining().start(), 19);
    assert_eq!(init.remaining().len(), 5);
    assert_eq!(init.remaining().read_u32(0, 5), Ok(21));
    assert!(std::ptr::eq(init.remaining().bytes(), bytes.as_slice()));
}

#[test]
fn every_truncation_respects_the_exclusive_span_even_with_backing_bytes() {
    let bytes = [0xff; 12];
    for start in 0..8 {
        for len in 0..19 {
            let input = BitSpan::new(&bytes, start, len).unwrap();
            assert_eq!(
                Initialization::decode(input),
                Err(if len < 9 {
                    Error::Truncated {
                        offset: 0,
                        width: 9,
                    }
                } else {
                    Error::Truncated {
                        offset: 9,
                        width: 10,
                    }
                })
            );
            assert_eq!(input.len(), len);
        }
        let exact = Initialization::decode(BitSpan::new(&bytes, start, 19).unwrap()).unwrap();
        assert_eq!(exact.selector(), 511);
        assert_eq!(exact.serial(), serial(1023));
        assert!(exact.remaining().is_empty());
        assert!(exact.remaining().read_u32(0, 1).is_err());
    }
}

#[test]
fn all_alignments_and_concatenation_preserve_the_next_initializer() {
    for start in 0..8 {
        let records = concat!("100000001", "0101010101", "000000000", "0000000001", "101");
        let text = format!("{}{records}", "1".repeat(start));
        let bytes = pack(&text);
        let first = Initialization::decode(BitSpan::new(&bytes, start, 41).unwrap()).unwrap();
        assert_eq!(first.selector(), 257);
        assert_eq!(first.serial(), serial(341));
        let second = Initialization::decode(first.remaining()).unwrap();
        assert_eq!(second.selector(), 0);
        assert_eq!(second.serial(), serial(1));
        assert_eq!(second.remaining().start(), start + 38);
        assert_eq!(second.remaining().len(), 3);
        assert_eq!(second.remaining().read_u32(0, 3), Ok(5));
    }
}

#[test]
fn zero_initializer_is_syntactically_valid_without_authorizing_registration() {
    let bytes = [0; 3];
    let init = Initialization::decode(BitSpan::new(&bytes, 0, 19).unwrap()).unwrap();
    assert_eq!(init.selector(), 0);
    assert_eq!(init.serial(), serial(0));
    assert!(init.remaining().is_empty());
}

#[test]
fn arbitrary_initializer_bits_have_bounded_work_and_preserve_suffix() {
    let mut random = 0x9182_7364u32;
    for _ in 0..10_000 {
        random ^= random << 13;
        random ^= random >> 17;
        random ^= random << 5;
        let bytes = random.to_be_bytes();
        let start = (random % 8) as usize;
        let len = ((random >> 8) % (33 - start as u32)) as usize;
        let input = BitSpan::new(&bytes, start, len).unwrap();
        match Initialization::decode(input) {
            Ok(init) => {
                assert!(len >= 19);
                assert!(init.selector() < 512);
                assert!(init.serial().value() < 1024);
                assert_eq!(init.remaining(), input.after(19).unwrap());
            }
            Err(Error::Truncated { .. }) => assert!(len < 19),
            other => panic!("unexpected result: {other:?}"),
        }
    }
}

#[test]
fn debug_output_omits_component_values_and_bytes() {
    let bytes = [0x80, 0xaa, 0xb5];
    let init = Initialization::decode(BitSpan::new(&bytes, 0, 19).unwrap()).unwrap();
    assert_eq!(format!("{:?}", init.serial()), "Serial { .. }");
    assert_eq!(
        format!("{init:?}"),
        "Initialization { remaining: BitSpan { start: 19, len: 0, .. }, .. }"
    );
}

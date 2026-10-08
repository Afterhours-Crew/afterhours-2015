// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use nfs_fire2::{Decoder, Error, Fields, Frame, HEADER_LEN, Limits, decode, encode};

fn hex(text: &str) -> Vec<u8> {
    assert_eq!(text.len() % 2, 0);
    text.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

fn empty_frame() -> Vec<u8> {
    vec![0; HEADER_LEN]
}

fn opaque_frame() -> Vec<u8> {
    hex("00000003000212345678abcdeffffedc1122aabbcc")
}

#[test]
fn synthetic_nonempty_layout_has_independent_byte_expectations() {
    let bytes = opaque_frame();
    let decoded = decode(&bytes, Limits::default()).unwrap().unwrap();
    assert_eq!(decoded.consumed, 21);
    assert_eq!(
        decoded.frame.fields,
        Fields {
            routing_a: 0x1234,
            routing_b: 0x5678,
            correlation: 0xabcdef,
            category: 7,
            slot: 31,
            reserved: [0xfe, 0xdc],
        }
    );
    assert_eq!(decoded.frame.metadata, [0x11, 0x22]);
    assert_eq!(decoded.frame.body, [0xaa, 0xbb, 0xcc]);
    assert_eq!(encode(decoded.frame, Limits::default()).unwrap(), bytes);
    assert_eq!(decoded.frame.metadata.as_ptr(), bytes[16..].as_ptr());
    assert_eq!(decoded.frame.body.as_ptr(), bytes[18..].as_ptr());
}

#[test]
fn every_truncation_is_incomplete_and_every_split_streams() {
    for bytes in [empty_frame(), opaque_frame()] {
        for split in 0..bytes.len() {
            assert_eq!(decode(&bytes[..split], Limits::default()).unwrap(), None);
            let mut decoder = Decoder::new(Limits::default());
            let first = decoder.push(&bytes[..split]).unwrap();
            assert_eq!(first.consumed, split);
            assert_eq!(first.frame, None);
            if split > 0 {
                assert!(matches!(decoder.finish(), Err(Error::UnexpectedEof { .. })));
            }
            let second = decoder.push(&bytes[split..]).unwrap();
            assert_eq!(second.consumed, bytes.len() - split);
            assert_eq!(
                encode(second.frame.unwrap(), Limits::default()).unwrap(),
                bytes
            );
            assert_eq!(decoder.finish(), Ok(()));
        }
    }
}

fn stream(chunks: &[&[u8]], limits: Limits) -> Vec<Vec<u8>> {
    let mut decoder = Decoder::new(limits);
    let mut frames = Vec::new();
    for chunk in chunks {
        let mut remaining = *chunk;
        while !remaining.is_empty() {
            let step = decoder.push(remaining).unwrap();
            assert!(step.consumed > 0 && step.consumed <= remaining.len());
            remaining = &remaining[step.consumed..];
            if let Some(frame) = step.frame {
                frames.push(encode(frame, limits).unwrap());
            }
            assert!(decoder.buffered_len() <= limits.max_frame());
        }
    }
    decoder.finish().unwrap();
    frames
}

#[test]
fn concatenation_and_arbitrary_read_boundaries_preserve_frames() {
    let frames = vec![empty_frame(), opaque_frame(), {
        let mut bytes = empty_frame();
        bytes[13] = 0x20;
        bytes
    }];
    let input = frames.concat();
    let first = decode(&input, Limits::default()).unwrap().unwrap();
    assert_eq!(first.consumed, 16);
    for width in 1..=input.len() {
        assert_eq!(
            stream(&input.chunks(width).collect::<Vec<_>>(), Limits::default()),
            frames
        );
    }
}

#[test]
fn all_category_and_slot_bits_are_preserved() {
    for category in 0..8 {
        for slot in 0..32 {
            let fields = Fields {
                category,
                slot,
                correlation: 0xff_ffff,
                reserved: [0xff, 0xa5],
                ..Fields::default()
            };
            let bytes = encode(
                Frame {
                    fields,
                    metadata: &[],
                    body: &[],
                },
                Limits::default(),
            )
            .unwrap();
            assert_eq!(bytes[13], category * 32 + slot);
            assert_eq!(
                decode(&bytes, Limits::default())
                    .unwrap()
                    .unwrap()
                    .frame
                    .fields,
                fields
            );
        }
    }
}

#[test]
fn encoder_rejects_values_that_do_not_fit_the_wire() {
    for (fields, field) in [
        (
            Fields {
                correlation: 0x100_0000,
                ..Fields::default()
            },
            "correlation",
        ),
        (
            Fields {
                category: 8,
                ..Fields::default()
            },
            "category",
        ),
        (
            Fields {
                slot: 32,
                ..Fields::default()
            },
            "slot",
        ),
    ] {
        assert_eq!(
            encode(
                Frame {
                    fields,
                    metadata: &[],
                    body: &[]
                },
                Limits::default()
            ),
            Err(Error::FieldOutOfRange(field))
        );
    }
    let metadata = vec![0; 65536];
    assert_eq!(
        encode(
            Frame {
                fields: Fields::default(),
                metadata: &metadata,
                body: &[]
            },
            Limits::default()
        ),
        Err(Error::FieldOutOfRange("metadata length"))
    );
}

#[test]
fn individual_and_combined_limits_apply_to_both_directions() {
    let bytes = opaque_frame();
    let frame = decode(&bytes, Limits::default()).unwrap().unwrap().frame;
    for (limits, region) in [
        (Limits::new(21, 1, 3).unwrap(), "metadata"),
        (Limits::new(21, 2, 2).unwrap(), "body"),
        (Limits::new(20, 2, 3).unwrap(), "frame"),
    ] {
        let error = decode(&bytes[..16], limits).unwrap_err();
        assert!(matches!(error, Error::LimitExceeded { region: actual, .. } if actual == region));
        assert_eq!(encode(frame, limits), Err(error.clone()));
        let mut decoder = Decoder::new(limits);
        assert_eq!(decoder.push(&bytes).unwrap_err(), error);
        assert_eq!(decoder.buffered_len(), 16);
    }
    let exact = Limits::new(21, 2, 3).unwrap();
    assert!(decode(&bytes, exact).unwrap().is_some());
    assert_eq!(encode(frame, exact).unwrap(), bytes);
}

#[test]
fn hostile_lengths_fail_on_header_without_waiting_or_allocating_payload() {
    let bytes = hex("ffffffffffff00000000000000000000");
    assert!(matches!(
        decode(&bytes, Limits::default()),
        Err(Error::LimitExceeded { .. })
    ));
    let mut decoder = Decoder::new(Limits::default());
    assert!(decoder.push(&bytes).is_err());
    assert_eq!(decoder.buffered_len(), HEADER_LEN);
    assert_eq!(
        decoder.push(&empty_frame()).unwrap_err(),
        Error::DecoderFailed
    );
    assert_eq!(decoder.finish(), Err(Error::DecoderFailed));
    decoder.reset();
    assert!(decoder.push(&empty_frame()).unwrap().frame.is_some());
}

#[test]
fn length_arithmetic_does_not_wrap_at_u32_boundary() {
    let bytes = hex("ffffffffffff00000000000000000000");
    let limits = Limits::new(usize::MAX, usize::MAX, usize::MAX).unwrap();
    if usize::BITS > 32 {
        // The full 4 GiB-plus frame is incomplete, not an empty wrapped frame.
        assert_eq!(decode(&bytes, limits).unwrap(), None);
    } else {
        assert_eq!(decode(&bytes, limits), Err(Error::LengthOverflow));
    }
}

#[test]
fn maximum_metadata_wire_width_is_supported_when_within_policy() {
    let metadata = vec![0xa5; 65535];
    let bytes = encode(
        Frame {
            fields: Fields::default(),
            metadata: &metadata,
            body: &[],
        },
        Limits::default(),
    )
    .unwrap();
    assert_eq!(&bytes[4..6], &[0xff, 0xff]);
    assert_eq!(
        decode(&bytes, Limits::default())
            .unwrap()
            .unwrap()
            .frame
            .metadata,
        metadata
    );
}

#[test]
fn completed_frame_is_emitted_once_and_empty_input_is_not_eof() {
    let mut decoder = Decoder::new(Limits::default());
    assert_eq!(decoder.finish(), Ok(()));
    assert_eq!(decoder.push(&[]).unwrap().consumed, 0);
    assert!(decoder.push(&empty_frame()).unwrap().frame.is_some());
    let empty = decoder.push(&[]).unwrap();
    assert_eq!(empty.consumed, 0);
    assert_eq!(empty.frame, None);
    assert_eq!(decoder.finish(), Ok(()));
    decoder.push(&[0]).unwrap();
    assert_eq!(
        decoder.finish(),
        Err(Error::UnexpectedEof {
            received: 1,
            expected: 16
        })
    );
    decoder.reset();
    assert_eq!(decoder.finish(), Ok(()));
}

#[test]
fn decoder_leaves_concatenated_backlog_with_caller() {
    let bytes = empty_frame().repeat(10000);
    let mut decoder = Decoder::new(Limits::new(16, 0, 0).unwrap());
    assert_eq!(decoder.push(&bytes).unwrap().consumed, 16);
    assert_eq!(decoder.buffered_len(), 16);
}

#[test]
fn independent_streams_do_not_share_partial_state() {
    let bytes = opaque_frame();
    let mut first = Decoder::new(Limits::default());
    let mut second = Decoder::new(Limits::default());
    first.push(&bytes[..17]).unwrap();
    assert!(second.push(&empty_frame()).unwrap().frame.is_some());
    assert_eq!(first.buffered_len(), 17);
    assert_eq!(
        first.push(&bytes[17..]).unwrap().frame.unwrap().body,
        [0xaa, 0xbb, 0xcc]
    );
}

#[test]
fn invalid_policy_and_payload_debug_redaction() {
    assert_eq!(Limits::new(15, 0, 0), Err(Error::InvalidLimits));
    let frame = Frame {
        fields: Fields::default(),
        metadata: b"private metadata",
        body: b"secret body",
    };
    let debug = format!("{frame:?}");
    assert!(debug.contains("metadata_len"));
    assert!(!debug.contains("private") && !debug.contains("secret"));
}

// Fixed seed for a reproducible bounded corpus, not a claim of coverage-guided fuzzing.
fn random(seed: &mut u64) -> u32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    (*seed >> 16) as u32
}

#[test]
fn generated_frames_survive_fragmentation_and_preserve_opaque_bytes() {
    let mut seed = 0x6e66_7332_3031_3501;
    for _ in 0..2048 {
        let fields = Fields {
            routing_a: random(&mut seed) as u16,
            routing_b: random(&mut seed) as u16,
            correlation: random(&mut seed) & 0xff_ffff,
            category: (random(&mut seed) & 7) as u8,
            slot: (random(&mut seed) & 31) as u8,
            reserved: [random(&mut seed) as u8, random(&mut seed) as u8],
        };
        let metadata: Vec<_> = (0..random(&mut seed) % 64)
            .map(|_| random(&mut seed) as u8)
            .collect();
        let body: Vec<_> = (0..random(&mut seed) % 256)
            .map(|_| random(&mut seed) as u8)
            .collect();
        let frame = Frame {
            fields,
            metadata: &metadata,
            body: &body,
        };
        let bytes = encode(frame, Limits::default()).unwrap();
        assert_eq!(
            decode(&bytes, Limits::default()).unwrap().unwrap().frame,
            frame
        );
        let width = (random(&mut seed) % 32 + 1) as usize;
        assert_eq!(
            stream(&bytes.chunks(width).collect::<Vec<_>>(), Limits::default()),
            vec![bytes]
        );
    }
}

#[test]
fn bounded_arbitrary_byte_corpus_never_panics_or_exceeds_buffer_limit() {
    let limits = Limits::new(512, 128, 384).unwrap();
    let mut seed = 0x6669_7265_325f_667a;
    for iteration in 0..10000 {
        let length = (random(&mut seed) % 600) as usize;
        let mut bytes: Vec<_> = (0..length).map(|_| random(&mut seed) as u8).collect();
        // Mix arbitrary headers with plausible lengths to reach payload paths.
        if iteration % 2 == 0 && length >= 16 {
            bytes[..4].copy_from_slice(&(random(&mut seed) % 500).to_be_bytes());
            bytes[4..6].copy_from_slice(&((random(&mut seed) % 160) as u16).to_be_bytes());
        }
        let whole = decode(&bytes, limits);
        let mut decoder = Decoder::new(limits);
        let incremental = decoder.push(&bytes);
        match whole {
            Ok(Some(decoded)) => {
                let step = incremental.unwrap();
                assert_eq!(step.consumed, decoded.consumed);
                assert_eq!(step.frame, Some(decoded.frame));
                assert_eq!(
                    encode(decoded.frame, limits).unwrap(),
                    bytes[..decoded.consumed]
                );
            }
            Ok(None) => assert_eq!(incremental.unwrap().frame, None),
            Err(error) => assert_eq!(incremental.unwrap_err(), error),
        }
        assert!(decoder.buffered_len() <= limits.max_frame());
    }
}

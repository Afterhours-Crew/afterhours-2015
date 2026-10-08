use super::tests::codec;
use super::*;

fn fixtures() -> Vec<Vec<u8>> {
    include_str!("../../tests/fixtures/transport/rollover.hex")
        .lines()
        .map(|h| {
            (0..h.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&h[i..i + 2], 16).unwrap())
                .collect()
        })
        .collect()
}

#[test]
fn independent_vectors_cross_the_cursor_boundary_without_restarting_the_cipher() {
    let c = codec();
    let mut rx = c.stream(32763).unwrap();
    let mut tx = c.stream(32763).unwrap();
    for (wire, size) in fixtures().iter().zip([27, 40, 3]) {
        let parts = vec![Part {
            channel: 0,
            bytes: (0..size).collect(),
        }];
        let header = Header::Ordinary {
            index: 9,
            cursor: tx.cursor(),
        };
        assert_eq!(tx.encode(header, &parts).unwrap(), *wire);
        let decoded = rx.decode(wire).unwrap();
        assert_eq!(decoded.parts, parts);
        assert_eq!(rx.cursor(), tx.cursor());
        assert_eq!(rx.units(), tx.units());
    }
    assert_eq!(rx.units(), 32763 + 6 + 7 + 3);
    assert_eq!(rx.cursor(), 11);
    assert!(
        c.decode(&fixtures()[1]).is_err(),
        "epoch-zero decoder cannot decode post-wrap packet"
    );
}

#[test]
fn failed_authentication_does_not_consume_cipher_or_cursor() {
    let c = codec();
    let vectors = fixtures();
    let mut rx = c.stream(32763).unwrap();
    rx.decode(&vectors[0]).unwrap();
    let before = (rx.cursor(), rx.units());
    for byte in 0..vectors[1].len() {
        for bit in 0..8 {
            let mut bad = vectors[1].clone();
            bad[byte] ^= 1 << bit;
            assert!(
                rx.decode(&bad).is_err(),
                "changed byte {byte} bit {bit} accepted"
            );
            assert_eq!((rx.cursor(), rx.units()), before);
        }
    }
    assert_eq!(rx.decode(&vectors[1]).unwrap().parts[0].bytes.len(), 40);
}

#[test]
fn lost_packets_can_be_skipped_across_wrap_but_reordered_input_cannot_move_state() {
    let mut rx = codec().stream(32763).unwrap();
    let vectors = fixtures();
    assert_eq!(rx.decode(&vectors[1]).unwrap().parts[0].bytes.len(), 40);
    let before = (rx.cursor(), rx.units());
    assert_eq!(rx.decode(&vectors[0]), Err(Error::Stale));
    assert_eq!(rx.decode(&vectors[1]), Err(Error::Stale));
    assert_eq!((rx.cursor(), rx.units()), before);
    assert!(rx.decode(&vectors[2]).is_ok());
    let mut ambiguous = vectors[2].clone();
    let cursor = rx.cursor().wrapping_add(0x4000) & MAX_CURSOR;
    ambiguous[2..4].copy_from_slice(&(cursor | 0x8000).to_be_bytes());
    assert_eq!(rx.decode(&ambiguous), Err(Error::Stale));
}

#[test]
fn invalid_output_and_input_bounds_leave_directional_state_unchanged() {
    let mut tx = codec().stream(32763).unwrap();
    let before = (tx.cursor(), tx.units());
    assert!(
        tx.encode(
            Header::Ordinary {
                index: 9,
                cursor: 32763
            },
            &[]
        )
        .is_err()
    );
    assert!(
        tx.encode(
            Header::Ordinary {
                index: 9,
                cursor: 32763
            },
            &[Part {
                channel: 0,
                bytes: vec![0; MAX_DATAGRAM]
            }]
        )
        .is_err()
    );
    assert_eq!(
        tx.encode(
            Header::Ordinary {
                index: 9,
                cursor: 0
            },
            &[Part {
                channel: 0,
                bytes: vec![1]
            }]
        ),
        Err(Error::Cursor)
    );
    assert_eq!(tx.decode(&vec![0; MAX_DATAGRAM + 1]), Err(Error::Length));
    assert_eq!((tx.cursor(), tx.units()), before);
    assert_eq!(
        tx.encode(
            Header::Ordinary {
                index: 9,
                cursor: 32763
            },
            &[Part {
                channel: 0,
                bytes: (0..27).collect()
            }]
        )
        .unwrap(),
        fixtures()[0]
    );
}

#[test]
fn separate_directions_survive_multiple_wraps_without_state_growth() {
    let c = codec();
    let mut tx = c.stream(0).unwrap();
    let mut rx = c.stream(0).unwrap();
    let fresh = c.stream(0).unwrap();
    for n in 0..1000 {
        let part = Part {
            channel: 0,
            bytes: vec![n as u8; 1200],
        };
        let wire = tx
            .encode(
                Header::Ordinary {
                    index: 9,
                    cursor: tx.cursor(),
                },
                std::slice::from_ref(&part),
            )
            .unwrap();
        assert_eq!(rx.decode(&wire).unwrap().parts, vec![part]);
    }
    assert_eq!(tx.units(), 152000);
    assert_eq!(rx.units(), 152000);
    assert_eq!(rx.cursor(), (152000 & 32767) as u16);
    assert_eq!((fresh.cursor(), fresh.units()), (0, 0));
}

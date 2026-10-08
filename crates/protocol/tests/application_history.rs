// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Acceptance-history state rules with independently specified sequence gaps.
use nfs_protocol::world::history::{AdvanceError, PeerReports, ReceiveHistory, Sequence};

fn seq(value: u16) -> Sequence {
    Sequence::new(value).unwrap()
}

#[test]
fn sequence_constructor_enforces_wire_width() {
    for value in 0..=u16::MAX {
        let result = Sequence::new(value);
        assert_eq!(result.is_some(), value < 1024);
        if let Some(sequence) = result {
            assert_eq!(sequence.value(), value);
        }
    }
}

#[test]
fn every_pair_matches_the_32_position_gate_and_rejection_is_atomic() {
    let mut admitted = 0;
    for current in 0..1024 {
        for incoming in 0..1024 {
            let distance = (i32::from(incoming) - i32::from(current) + 1024) % 1024;
            let expected = match distance {
                0 => Err(AdvanceError::Repeated),
                1..=32 => Ok(distance as u8),
                _ => Err(AdvanceError::OutsideWindow),
            };
            let mut receive = ReceiveHistory::new(seq(current), 0xa55a_1f3c);
            let before = receive;
            let mut peer = PeerReports::new(seq(current));
            let before_peer = peer;
            assert_eq!(receive.check(seq(incoming)), expected);
            assert_eq!(receive, before, "check must not mutate");
            let committed = receive.commit(seq(incoming), true);
            let reports = peer.consume(seq(incoming), 0x0123_4567);
            match expected {
                Ok(distance) => {
                    admitted += 1;
                    assert_eq!(committed, Ok(distance - 1));
                    let expected_bits = ((0xa55a_1f3c_u64 << distance) | 1) as u32;
                    assert_eq!(receive.bits(), expected_bits);
                    assert_eq!(receive.frontier(), seq(incoming));
                    let batch = reports.unwrap();
                    assert_eq!(batch.len(), usize::from(distance));
                    assert_eq!(peer.frontier(), seq(incoming));
                }
                Err(error) => {
                    assert_eq!(committed, Err(error));
                    assert_eq!(reports, Err(error));
                    assert_eq!(receive, before);
                    assert_eq!(peer, before_peer);
                }
            }
        }
    }
    assert_eq!(admitted, 32_768);
}

#[test]
fn captured_missing_sequence_sets_a_hole_then_ages_out() {
    // Incoming 45 -> 47, then client ack 47 / history fffffffd.
    let mut state = ReceiveHistory::new(seq(45), u32::MAX);
    assert_eq!(state.commit(seq(47), true), Ok(1));
    assert_eq!(state.bits(), 0xffff_fffd);
    for next in 48..77 {
        state.commit(seq(next), true).unwrap();
        assert_eq!(state.bits(), u32::MAX ^ (1 << (next - 46)));
    }
    state.commit(seq(77), true).unwrap();
    assert_eq!(state.bits(), 0x7fff_ffff);
    state.commit(seq(78), true).unwrap();
    assert_eq!(state.bits(), u32::MAX);
}

#[test]
fn received_but_unaccepted_advances_and_cannot_be_reaccepted_as_duplicate() {
    let mut receive = ReceiveHistory::new(seq(8), 0b1111);
    assert_eq!(receive.commit(seq(9), false), Ok(0));
    assert_eq!(receive.bits(), 0b11110);
    let before = receive;
    assert_eq!(receive.commit(seq(9), true), Err(AdvanceError::Repeated));
    assert_eq!(receive, before);
    receive.commit(seq(10), true).unwrap();
    assert_eq!(receive.bits(), 0b111101);
}

#[test]
fn exact_window_discards_old_bits_and_wraps_through_zero() {
    for accepted in [false, true] {
        let mut state = ReceiveHistory::new(seq(1000), u32::MAX);
        assert_eq!(state.commit(seq(8), accepted), Ok(31));
        assert_eq!(state.bits(), u32::from(accepted));
        let before = state;
        assert_eq!(
            state.commit(seq(41), true),
            Err(AdvanceError::OutsideWindow)
        );
        assert_eq!(state, before);
    }
    let mut state = ReceiveHistory::new(seq(1023), 1);
    state.commit(seq(0), true).unwrap();
    assert_eq!(state.frontier().value(), 0);
    assert_eq!(state.bits(), 3);
}

#[test]
fn peer_results_are_oldest_first_across_wrap_and_skip_repeated_reports() {
    let mut peer = PeerReports::new(seq(1022));
    let batch = peer.consume(seq(2), 0b1101).unwrap();
    let observed: Vec<_> = batch
        .iter()
        .map(|x| (x.sequence().value(), x.accepted()))
        .collect();
    assert_eq!(observed, [(1023, true), (0, true), (1, false), (2, true)]);
    assert_eq!(batch.iter().len(), 4);
    assert!(!batch.is_empty());
    assert_eq!(batch.iter().next_back().unwrap().sequence(), seq(2));
    assert_eq!(peer.consume(seq(2), 0), Err(AdvanceError::Repeated));
    assert_eq!(peer.consume(seq(1), 0), Err(AdvanceError::OutsideWindow));
    let next = peer.consume(seq(3), 0).unwrap();
    assert_eq!(next.len(), 1);
    assert!(!next.iter().next().unwrap().accepted());
    // An already returned batch is stable across subsequent state changes.
    assert_eq!(batch.iter().count(), 4);
}

#[test]
fn every_history_bit_maps_to_its_sequence_in_a_full_batch() {
    for bit in 0..32 {
        let mut peer = PeerReports::new(seq(1000));
        let batch = peer.consume(seq(8), 1 << bit).unwrap();
        assert_eq!(batch.len(), 32);
        let statuses: Vec<_> = batch.iter().collect();
        assert_eq!(statuses.iter().filter(|x| x.accepted()).count(), 1);
        for (index, status) in statuses.into_iter().enumerate() {
            assert_eq!(status.sequence().value(), (1001 + index as u16) % 1024);
            assert_eq!(status.accepted(), index == 31 - bit);
        }
    }
}

#[test]
fn connection_instances_and_receive_peer_frontiers_are_independent() {
    let mut a = ReceiveHistory::new(seq(0), 0);
    let mut b = ReceiveHistory::new(seq(0), 0);
    let mut peer_a = PeerReports::new(seq(0));
    let peer_b = PeerReports::new(seq(0));
    a.commit(seq(2), true).unwrap();
    b.commit(seq(1), false).unwrap();
    let reports = peer_a.consume(seq(3), 0b101).unwrap();
    assert_eq!(reports.len(), 3);
    assert_eq!((a.frontier().value(), a.bits()), (2, 1));
    assert_eq!((b.frontier().value(), b.bits()), (1, 0));
    assert_eq!(peer_b.frontier().value(), 0);
    // Reset is explicit construction for a new connection lifetime.
    let fresh = ReceiveHistory::new(seq(0), 0);
    assert_eq!((fresh.frontier().value(), fresh.bits()), (0, 0));
}

#[test]
fn long_mixed_history_matches_a_bounded_boolean_reference_model() {
    let mut state = ReceiveHistory::new(seq(1000), 0);
    let mut newest_first = [false; 32];
    let mut current = 1000_u16;
    let mut seed = 0x4a26_789b_u32;
    for _ in 0..20_000 {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let distance = (seed as usize % 32) + 1;
        let accepted = seed & 0x8000_0000 != 0;
        newest_first.rotate_right(distance % 32);
        newest_first[..distance].fill(false);
        newest_first[0] = accepted;
        current = (current + distance as u16) % 1024;
        state.commit(seq(current), accepted).unwrap();
        let expected = newest_first
            .iter()
            .enumerate()
            .fold(0, |word, (bit, value)| word | (u32::from(*value) << bit));
        assert_eq!(state.bits(), expected);
    }
}

#[test]
fn debug_output_does_not_expose_connection_sequence_or_history_values() {
    let mut peer = PeerReports::new(seq(713));
    let batch = peer.consume(seq(714), 0xdead_beef).unwrap();
    let receive = ReceiveHistory::new(seq(713), 0xdead_beef);
    let text = format!(
        "{:?} {receive:?} {peer:?} {batch:?} {:?}",
        seq(713),
        batch.iter().next()
    );
    for value in ["713", "714", "dead", "3735928559"] {
        assert!(!text.contains(value));
    }
}

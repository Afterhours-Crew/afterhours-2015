// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use nfs_fire2::{Fields, Frame};
use nfs_services::client_state::reply;

const BODY: &[u8] = &[0xb6, 0xf9, 0x25, 0, 1, 0xcf, 0x48, 0x74, 0, 0];
fn wire(correlation: u32, body: &[u8]) -> Vec<u8> {
    nfs_fire2::encode(
        Frame {
            fields: Fields {
                routing_a: 9,
                routing_b: 28,
                correlation,
                ..Default::default()
            },
            metadata: &[],
            body,
        },
        nfs_fire2::Limits::default(),
    )
    .unwrap()
}

#[test]
fn independent_empty_reply_uses_current_correlation_and_is_repeatable() {
    let request = wire(0xabcdef, BODY);
    let expected = [0, 0, 0, 0, 0, 0, 0, 9, 0, 28, 0xab, 0xcd, 0xef, 0x20, 0, 0];
    assert_eq!(reply(&request).unwrap(), expected);
    assert_eq!(reply(&request).unwrap(), expected);
    let other = reply(&wire(3, BODY)).unwrap();
    assert_eq!(other[12], 3);
    assert_eq!(reply(&request).unwrap(), expected);
}

#[test]
fn rejects_unobserved_state_absence_unknowns_and_noncanonical_body() {
    for body in [&[][..], &BODY[..5], &BODY[5..]] {
        assert!(reply(&wire(1, body)).is_err());
    }
    for (index, value) in [(4, 0), (4, 2), (4, 4), (9, 1)] {
        let mut body = BODY.to_vec();
        body[index] = value;
        assert!(reply(&wire(1, &body)).is_err());
    }
    let reversed = [&BODY[5..], &BODY[..5]].concat();
    assert!(reply(&wire(1, &reversed)).is_err());
    let extra = [BODY, &[0xf3, 0xff, 0xff, 0, 0]].concat();
    assert!(reply(&wire(1, &extra)).is_err());
}

#[test]
fn loading_multiplayer_report_gets_current_empty_acknowledgement() {
    // Constructed MODE=3/STAT=0 fields with a caller-selected correlation.
    let body = [0xb6, 0xf9, 0x25, 0, 3, 0xcf, 0x48, 0x74, 0, 0];
    assert_eq!(
        reply(&wire(56, &body)).unwrap(),
        [0, 0, 0, 0, 0, 0, 0, 9, 0, 28, 0, 0, 56, 0x20, 0, 0]
    );
}

#[test]
fn rejects_partial_concatenated_wrong_route_and_header_forms() {
    let request = wire(9, BODY);
    for cut in 0..request.len() {
        assert!(reply(&request[..cut]).is_err());
    }
    assert!(reply(&[request.as_slice(), request.as_slice()].concat()).is_err());
    for index in [6, 7, 8, 9, 13, 14, 15] {
        let mut changed = request.clone();
        changed[index] ^= 1;
        assert!(reply(&changed).is_err());
    }
    let mut nonrequest = request.clone();
    nonrequest[13] = 0x20;
    assert!(reply(&nonrequest).is_err());
    assert!(reply(&wire(9, &[0; 17])).is_err());
    let mut metadata = request;
    metadata[5] = 1;
    assert!(reply(&metadata).is_err());
}

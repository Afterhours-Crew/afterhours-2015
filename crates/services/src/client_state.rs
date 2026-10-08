// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Bounded acknowledgement of normal menu and multiplayer client-state reports.
//! Reports are informational; acknowledgement changes no account state and does
//! not establish interactive readiness. Only modes 1/3 with status 0 are supported.
use nfs_fire2::{Fields, Frame};
use nfs_protocol::util::{self, ClientState};

pub fn reply(bytes: &[u8]) -> Result<Vec<u8>, Error> {
    let limits = nfs_fire2::Limits::new(32, 0, 16).expect("constant limits");
    let decoded = nfs_fire2::decode(bytes, limits)
        .map_err(|_| Error::IneligibleRequest)?
        .ok_or(Error::IneligibleRequest)?;
    let request = decoded.frame;
    let fields = request.fields;
    if decoded.consumed != bytes.len()
        || fields.category != 0
        || fields.routing_a != util::COMPONENT
        || fields.routing_b != util::SET_CLIENT_STATE
        || fields.slot != 0
        || fields.reserved != [0, 0]
        || !request.metadata.is_empty()
    {
        return Err(Error::IneligibleRequest);
    }
    let body_limits = nfs_heat2::Limits {
        max_bytes: 16,
        max_values: 2,
        max_depth: 0,
        ..Default::default()
    };
    let state =
        ClientState::decode(request.body, body_limits).map_err(|_| Error::IneligibleRequest)?;
    if !matches!(state.mode, Some(1 | 3))
        || state.status != Some(0)
        || state.unknown_field_count() != 0
        || state
            .encode(body_limits)
            .map_err(|_| Error::IneligibleRequest)?
            != request.body
    {
        return Err(Error::IneligibleRequest);
    }
    nfs_fire2::encode(
        Frame {
            fields: Fields {
                category: 1,
                ..fields
            },
            metadata: &[],
            body: &[],
        },
        limits,
    )
    .map_err(|_| Error::Encode)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    IneligibleRequest,
    Encode,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "client state {self:?}")
    }
}
impl std::error::Error for Error {}

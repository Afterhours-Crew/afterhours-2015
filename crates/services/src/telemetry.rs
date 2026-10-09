// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Local consumption of GameManager telemetry reports (route 4/171).
//! Each connection pins its first accepted report identities, retains at most
//! sixteen diagnostic samples and returns a correlated header-only acknowledgement.
//! Reports are never forwarded and do not mutate account state.
use nfs_fire2::{Fields, Frame, HEADER_LEN};
use nfs_heat2::{Encoder, Field, Item, Limits, Value};
use std::collections::VecDeque;

pub const COMPONENT: u16 = 4;
pub const REPORT_TELEMETRY: u16 = 171;

/// Observed requests carry one report; the list stays bounded regardless.
pub const MAX_REPORTS: usize = 8;
/// Observed bodies are 73 bytes; the bound covers `MAX_REPORTS` full reports.
pub const MAX_BODY: usize = 512;
/// Recent samples kept per connection for diagnostics.
pub const RETAINED_SAMPLES: usize = 16;

const GID: [u8; 3] = [0x9e, 0x99, 0x00];
const LCID: [u8; 3] = [0xb2, 0x3a, 0x64];
const NTOP: [u8; 3] = [0xbb, 0x4b, 0xf0];
const RPTS: [u8; 3] = [0xcb, 0x0d, 0x33];
const LATC: [u8; 3] = [0xb2, 0x1d, 0x23];
const PKTL: [u8; 3] = [0xc2, 0xbd, 0x2c];
const RCID: [u8; 3] = [0xca, 0x3a, 0x64];
const RCVD: [u8; 3] = [0xca, 0x3d, 0xa4];
const SENT: [u8; 3] = [0xce, 0x5b, 0xb4];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Not a complete, metadata-free category-0 `4/171` request within bounds.
    IneligibleFrame,
    /// The body is not exactly the native request layout in canonical encoding.
    Body,
    /// The report identities differ from the first accepted report on this connection.
    Identity,
    Encode,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "telemetry {self:?}")
    }
}
impl std::error::Error for Error {}

/// One `TelemetryReport` element. Identifiers are wire integers without an
/// assigned width; counters and percentages are validated as non-negative.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Report {
    pub latency_ms: u32,
    pub packet_loss_percent: u8,
    pub remote_connection: i64,
    pub local_packets_received: u32,
    pub remote_packets_sent: u32,
}

/// The decoded request, exactly the four native fields.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Request {
    pub game: i64,
    pub local_connection: i64,
    pub network_topology: i64,
    pub reports: Vec<Report>,
}

fn frame_limits() -> nfs_fire2::Limits {
    nfs_fire2::Limits::new(HEADER_LEN + MAX_BODY, 0, MAX_BODY).expect("constant limits")
}

fn body_limits() -> Limits {
    Limits {
        max_bytes: MAX_BODY,
        // Four root values, the list, and five integers plus one struct per report.
        max_values: 4 + MAX_REPORTS * 6,
        max_depth: 3,
        max_collection: MAX_REPORTS,
        max_byte_string: 0,
    }
}

fn integer(item: Item<'_>) -> Result<i64, Error> {
    match item.value() {
        Value::Integer(value) => Ok(value),
        _ => Err(Error::Body),
    }
}

fn counter(value: i64) -> Result<u32, Error> {
    u32::try_from(value).map_err(|_| Error::Body)
}

fn report(item: Item<'_>) -> Result<Report, Error> {
    let fields = item.fields().ok_or(Error::Body)?;
    let mut latency = None;
    let mut loss = None;
    let mut remote = None;
    let mut received = None;
    let mut sent = None;
    for field in fields {
        let field = field.map_err(|_| Error::Body)?;
        let slot = match field.tag() {
            LATC => &mut latency,
            PKTL => &mut loss,
            RCID => &mut remote,
            RCVD => &mut received,
            SENT => &mut sent,
            _ => return Err(Error::Body),
        };
        if slot.replace(integer(field.item())?).is_some() {
            return Err(Error::Body);
        }
    }
    let loss = loss.ok_or(Error::Body)?;
    if !(0..=100).contains(&loss) {
        return Err(Error::Body);
    }
    Ok(Report {
        latency_ms: counter(latency.ok_or(Error::Body)?)?,
        packet_loss_percent: u8::try_from(loss).map_err(|_| Error::Body)?,
        remote_connection: remote.ok_or(Error::Body)?,
        local_packets_received: counter(received.ok_or(Error::Body)?)?,
        remote_packets_sent: counter(sent.ok_or(Error::Body)?)?,
    })
}

impl Request {
    /// Decode a body that is exactly the native layout, with no unknown or
    /// duplicate fields. Unknown data is rejected rather than acknowledged.
    pub fn decode(body: &[u8]) -> Result<Self, Error> {
        let document = nfs_heat2::decode(body, body_limits()).map_err(|_| Error::Body)?;
        let mut game = None;
        let mut local = None;
        let mut topology = None;
        let mut reports: Option<Vec<Report>> = None;
        for field in document.fields() {
            let field: Field<'_> = field.map_err(|_| Error::Body)?;
            match field.tag() {
                GID if game.is_none() => game = Some(integer(field.item())?),
                LCID if local.is_none() => local = Some(integer(field.item())?),
                NTOP if topology.is_none() => topology = Some(integer(field.item())?),
                RPTS if reports.is_none() => {
                    let item = field.item();
                    let Value::List { count, .. } = item.value() else {
                        return Err(Error::Body);
                    };
                    if count == 0 || count > MAX_REPORTS {
                        return Err(Error::Body);
                    }
                    let mut list = Vec::with_capacity(count);
                    for element in item.elements().ok_or(Error::Body)? {
                        list.push(report(element.map_err(|_| Error::Body)?)?);
                    }
                    if list.len() != count {
                        return Err(Error::Body);
                    }
                    reports = Some(list);
                }
                _ => return Err(Error::Body),
            }
        }
        Ok(Self {
            game: game.ok_or(Error::Body)?,
            local_connection: local.ok_or(Error::Body)?,
            network_topology: topology.ok_or(Error::Body)?,
            reports: reports.ok_or(Error::Body)?,
        })
    }

    /// Canonical native encoding (tags in ascending order, minimal integers).
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        if self.reports.is_empty() || self.reports.len() > MAX_REPORTS {
            return Err(Error::Body);
        }
        let mut writer = Encoder::new(body_limits());
        let result = (|| {
            writer.integer(GID, self.game)?;
            writer.integer(LCID, self.local_connection)?;
            writer.integer(NTOP, self.network_topology)?;
            writer.struct_list(RPTS, &self.reports, |w, r| {
                w.integer(LATC, i64::from(r.latency_ms))?;
                w.integer(PKTL, i64::from(r.packet_loss_percent))?;
                w.integer(RCID, r.remote_connection)?;
                w.integer(RCVD, i64::from(r.local_packets_received))?;
                w.integer(SENT, i64::from(r.remote_packets_sent))
            })
        })();
        result.map_err(|_| Error::Encode)?;
        writer.finish().map_err(|_| Error::Encode)
    }
}

/// Identities pinned by the first accepted report on a connection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Identity {
    game: i64,
    local_connection: i64,
    network_topology: i64,
    remote_connection: i64,
}

/// A retained sample without connection identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Sample {
    pub latency_ms: u32,
    pub packet_loss_percent: u8,
    pub local_packets_received: u32,
    pub remote_packets_sent: u32,
}

/// Per-connection telemetry state: pinned identities and recent samples.
#[derive(Debug, Default)]
pub struct Telemetry {
    identity: Option<Identity>,
    recent: VecDeque<Sample>,
    accepted: u64,
}

impl Telemetry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of acknowledged requests on this connection.
    pub fn accepted(&self) -> u64 {
        self.accepted
    }

    /// Most recent retained samples, oldest first (at most `RETAINED_SAMPLES`).
    pub fn recent(&self) -> impl Iterator<Item = &Sample> {
        self.recent.iter()
    }

    /// The pinned (game, local connection group, remote connection group), if any.
    pub fn identity(&self) -> Option<(i64, i64, i64)> {
        self.identity
            .map(|i| (i.game, i.local_connection, i.remote_connection))
    }

    /// Answer one complete `4/171` request with the observed header-only
    /// acknowledgement, after consuming its reports. Any other input, a
    /// malformed body or a report for different identities is rejected without a
    /// reply and without changing state.
    pub fn reply(&mut self, wire: &[u8]) -> Result<Vec<u8>, Error> {
        let limits = frame_limits();
        let decoded = nfs_fire2::decode(wire, limits)
            .map_err(|_| Error::IneligibleFrame)?
            .ok_or(Error::IneligibleFrame)?;
        let frame = decoded.frame;
        let fields = frame.fields;
        if decoded.consumed != wire.len()
            || fields.category != 0
            || fields.routing_a != COMPONENT
            || fields.routing_b != REPORT_TELEMETRY
            || fields.slot != 0
            || fields.reserved != [0, 0]
            || !frame.metadata.is_empty()
        {
            return Err(Error::IneligibleFrame);
        }
        let request = Request::decode(frame.body)?;
        if request.encode()? != frame.body {
            return Err(Error::Body);
        }
        let first = request.reports.first().ok_or(Error::Body)?;
        let identity = Identity {
            game: request.game,
            local_connection: request.local_connection,
            network_topology: request.network_topology,
            remote_connection: first.remote_connection,
        };
        if request
            .reports
            .iter()
            .any(|r| r.remote_connection != identity.remote_connection)
            || self.identity.is_some_and(|pinned| pinned != identity)
        {
            return Err(Error::Identity);
        }
        let ack = nfs_fire2::encode(
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
        .map_err(|_| Error::Encode)?;
        self.identity = Some(identity);
        for r in &request.reports {
            if self.recent.len() == RETAINED_SAMPLES {
                self.recent.pop_front();
            }
            self.recent.push_back(Sample {
                latency_ms: r.latency_ms,
                packet_loss_percent: r.packet_loss_percent,
                local_packets_received: r.local_packets_received,
                remote_packets_sent: r.remote_packets_sent,
            });
        }
        self.accepted += 1;
        Ok(ack)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(game: i64, remote: i64, received: u32) -> Request {
        Request {
            game,
            local_connection: 0x1_0000_0001,
            network_topology: 1,
            reports: vec![Report {
                latency_ms: 21,
                packet_loss_percent: 0,
                remote_connection: remote,
                local_packets_received: received,
                remote_packets_sent: received,
            }],
        }
    }

    fn frame(fields: Fields, metadata: &[u8], body: &[u8]) -> Vec<u8> {
        nfs_fire2::encode(
            Frame {
                fields,
                metadata,
                body,
            },
            nfs_fire2::Limits::default(),
        )
        .unwrap()
    }

    fn fields(correlation: u32) -> Fields {
        Fields {
            routing_a: COMPONENT,
            routing_b: REPORT_TELEMETRY,
            correlation,
            ..Default::default()
        }
    }

    fn wire(correlation: u32, request: &Request) -> Vec<u8> {
        frame(fields(correlation), &[], &request.encode().unwrap())
    }

    #[test]
    fn canonical_request_round_trips() {
        let original = request(7, 9, 212);
        let body = original.encode().unwrap();
        assert_eq!(Request::decode(&body).unwrap(), original);
        assert!(body.len() < 73);
    }

    #[test]
    fn acknowledgement_is_the_header_only_category_one_frame() {
        let mut state = Telemetry::new();
        let ack = state.reply(&wire(172, &request(7, 9, 212))).unwrap();
        assert_eq!(ack.len(), HEADER_LEN);
        let decoded = nfs_fire2::decode(&ack, nfs_fire2::Limits::default())
            .unwrap()
            .unwrap();
        assert_eq!(
            decoded.frame.fields,
            Fields {
                category: 1,
                ..fields(172)
            }
        );
        assert!(decoded.frame.body.is_empty() && decoded.frame.metadata.is_empty());
        assert_eq!(state.accepted(), 1);
        assert_eq!(state.identity(), Some((7, 0x1_0000_0001, 9)));
        assert_eq!(
            state.recent().copied().collect::<Vec<_>>(),
            vec![Sample {
                latency_ms: 21,
                packet_loss_percent: 0,
                local_packets_received: 212,
                remote_packets_sent: 212,
            }]
        );
    }

    #[test]
    fn repeated_and_subsequent_reports_for_the_same_identities_are_acknowledged() {
        let mut state = Telemetry::new();
        let first = wire(172, &request(7, 9, 212));
        assert!(state.reply(&first).is_ok());
        assert!(state.reply(&first).is_ok());
        assert!(state.reply(&wire(173, &request(7, 9, 497))).is_ok());
        assert_eq!(state.accepted(), 3);
        assert_eq!(state.recent().count(), 3);
    }

    #[test]
    fn different_game_local_topology_or_remote_identities_are_rejected() {
        let mut state = Telemetry::new();
        state.reply(&wire(1, &request(7, 9, 1))).unwrap();
        assert_eq!(
            state.reply(&wire(2, &request(8, 9, 2))),
            Err(Error::Identity)
        );
        assert_eq!(
            state.reply(&wire(3, &request(7, 10, 3))),
            Err(Error::Identity)
        );
        let mut other_local = request(7, 9, 4);
        other_local.local_connection += 1;
        assert_eq!(state.reply(&wire(4, &other_local)), Err(Error::Identity));
        let mut other_topology = request(7, 9, 5);
        other_topology.network_topology = 2;
        assert_eq!(state.reply(&wire(5, &other_topology)), Err(Error::Identity));
        let mut mixed = request(7, 9, 6);
        mixed.reports.push(Report {
            remote_connection: 11,
            ..mixed.reports[0]
        });
        assert_eq!(state.reply(&wire(6, &mixed)), Err(Error::Identity));
        assert_eq!(state.accepted(), 1);
        assert_eq!(state.recent().count(), 1);
    }

    #[test]
    fn connections_are_isolated() {
        let mut a = Telemetry::new();
        let mut b = Telemetry::new();
        a.reply(&wire(1, &request(7, 9, 1))).unwrap();
        b.reply(&wire(1, &request(8, 10, 1))).unwrap();
        assert_eq!(a.identity(), Some((7, 0x1_0000_0001, 9)));
        assert_eq!(b.identity(), Some((8, 0x1_0000_0001, 10)));
    }

    #[test]
    fn retained_samples_are_bounded() {
        let mut state = Telemetry::new();
        for i in 0..(RETAINED_SAMPLES as u32 + 5) {
            state.reply(&wire(i, &request(7, 9, i))).unwrap();
        }
        assert_eq!(state.recent().count(), RETAINED_SAMPLES);
        assert_eq!(state.recent().next().unwrap().local_packets_received, 5);
        assert_eq!(state.accepted(), RETAINED_SAMPLES as u64 + 5);
    }

    #[test]
    fn other_routes_categories_metadata_and_trailing_bytes_are_ineligible() {
        let mut state = Telemetry::new();
        let body = request(7, 9, 1).encode().unwrap();
        let other_route = frame(
            Fields {
                routing_b: 29,
                ..fields(1)
            },
            &[],
            &body,
        );
        assert_eq!(state.reply(&other_route), Err(Error::IneligibleFrame));
        let reply_category = frame(
            Fields {
                category: 1,
                ..fields(1)
            },
            &[],
            &body,
        );
        assert_eq!(state.reply(&reply_category), Err(Error::IneligibleFrame));
        let with_metadata = frame(fields(1), &[0x9e, 0x99, 0x00, 0, 1], &body);
        assert_eq!(state.reply(&with_metadata), Err(Error::IneligibleFrame));
        let mut trailing = wire(1, &request(7, 9, 1));
        trailing.push(0);
        assert_eq!(state.reply(&trailing), Err(Error::IneligibleFrame));
        let slot = frame(
            Fields {
                slot: 1,
                ..fields(1)
            },
            &[],
            &body,
        );
        assert_eq!(state.reply(&slot), Err(Error::IneligibleFrame));
        assert_eq!(
            state.reply(&wire(1, &request(7, 9, 1))[..10]),
            Err(Error::IneligibleFrame)
        );
        assert_eq!(state.accepted(), 0);
    }

    #[test]
    fn malformed_bodies_get_no_acknowledgement() {
        let mut state = Telemetry::new();
        let ok = request(7, 9, 1);
        // Unknown root field (tag between NTOP and RPTS).
        let mut writer = Encoder::new(Limits::default());
        writer.integer(GID, 7).unwrap();
        writer.integer(LCID, 0x1_0000_0001).unwrap();
        writer.integer(NTOP, 1).unwrap();
        writer.integer([0xbb, 0x4c, 0x00], 5).unwrap();
        writer
            .struct_list(RPTS, &ok.reports, |w, r| {
                w.integer(LATC, i64::from(r.latency_ms))?;
                w.integer(PKTL, 0)?;
                w.integer(RCID, r.remote_connection)?;
                w.integer(RCVD, 1)?;
                w.integer(SENT, 1)
            })
            .unwrap();
        let unknown = writer.finish().unwrap();
        assert_eq!(Request::decode(&unknown), Err(Error::Body));
        assert_eq!(
            state.reply(&frame(fields(1), &[], &unknown)),
            Err(Error::Body)
        );
        // Missing report list.
        let mut writer = Encoder::new(Limits::default());
        writer.integer(GID, 7).unwrap();
        writer.integer(LCID, 1).unwrap();
        writer.integer(NTOP, 1).unwrap();
        let missing = writer.finish().unwrap();
        assert_eq!(Request::decode(&missing), Err(Error::Body));
        // Empty report list.
        let mut writer = Encoder::new(Limits::default());
        writer.integer(GID, 7).unwrap();
        writer.integer(LCID, 1).unwrap();
        writer.integer(NTOP, 1).unwrap();
        writer
            .struct_list::<Report>(RPTS, &[], |_, _| Ok(()))
            .unwrap();
        assert_eq!(Request::decode(&writer.finish().unwrap()), Err(Error::Body));
        // Too many reports.
        let mut many = ok.clone();
        many.reports = vec![ok.reports[0]; MAX_REPORTS + 1];
        assert_eq!(many.encode(), Err(Error::Body));
        let mut writer = Encoder::new(Limits::default());
        writer.integer(GID, 7).unwrap();
        writer.integer(LCID, 1).unwrap();
        writer.integer(NTOP, 1).unwrap();
        writer
            .struct_list(RPTS, &many.reports, |w, r| {
                w.integer(LATC, i64::from(r.latency_ms))?;
                w.integer(PKTL, 0)?;
                w.integer(RCID, r.remote_connection)?;
                w.integer(RCVD, 1)?;
                w.integer(SENT, 1)
            })
            .unwrap();
        assert_eq!(Request::decode(&writer.finish().unwrap()), Err(Error::Body));
        // Negative counter and out-of-range loss.
        for (tag, value) in [(LATC, -1), (PKTL, 101), (RCVD, -5), (SENT, i64::MAX)] {
            let mut writer = Encoder::new(Limits::default());
            writer.integer(GID, 7).unwrap();
            writer.integer(LCID, 1).unwrap();
            writer.integer(NTOP, 1).unwrap();
            writer
                .struct_list(RPTS, &[()], |w, _: &()| {
                    w.integer(LATC, if tag == LATC { value } else { 1 })?;
                    w.integer(PKTL, if tag == PKTL { value } else { 0 })?;
                    w.integer(RCID, 9)?;
                    w.integer(RCVD, if tag == RCVD { value } else { 1 })?;
                    w.integer(SENT, if tag == SENT { value } else { 1 })
                })
                .unwrap();
            assert_eq!(
                Request::decode(&writer.finish().unwrap()),
                Err(Error::Body),
                "{tag:?}"
            );
        }
        // Wrong kind for a known tag and a non-canonical trailing byte.
        let mut writer = Encoder::new(Limits::default());
        writer.string(GID, b"7").unwrap();
        writer.integer(LCID, 1).unwrap();
        writer.integer(NTOP, 1).unwrap();
        writer
            .struct_list(RPTS, &ok.reports, |w, r| {
                w.integer(RCID, r.remote_connection)
            })
            .unwrap();
        assert_eq!(Request::decode(&writer.finish().unwrap()), Err(Error::Body));
        let mut padded = ok.encode().unwrap();
        padded.push(0);
        assert_eq!(
            state.reply(&frame(fields(2), &[], &padded)),
            Err(Error::Body)
        );
        // Oversized body is rejected by the frame bound before decoding.
        let huge = vec![0u8; MAX_BODY + 1];
        assert_eq!(
            state.reply(&frame(fields(3), &[], &huge)),
            Err(Error::IneligibleFrame)
        );
        assert_eq!(state.accepted(), 0);
        assert_eq!(state.identity(), None);
    }
}

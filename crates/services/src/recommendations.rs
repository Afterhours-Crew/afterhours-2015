// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! In-game recommendations for an empty local speed-wall catalog (route 2050/21).
//! A query must name the connection's current persona. Responses contain an empty
//! map for each supported stat type and omit the recommendation list. Populated
//! catalogs are unsupported; their rows must never be silently discarded.
use nfs_fire2::{Fields, Frame, HEADER_LEN};
use nfs_heat2::{Encoder, Kind, Limits, Member, Schema, Type, TypeId, Value};

pub const COMPONENT: u16 = 2050;
pub const GET_IN_GAME_RECOMMENDATIONS: u16 = 21;

/// `BLID blazeId`, the same tag as the friends-recommendations query.
const BLID: [u8; 3] = [0x8a, 0xca, 0x64];
const SPWA: [u8; 3] = [0xcf, 0x0d, 0xe1];
/// Collection header byte for map/struct/list values (ambiguous on the wire).
const COLLECTION: u8 = 3;

/// Observed `SpeedWallStatType` keys, in wire order.
pub const STAT_TYPES: [i64; 2] = [0, 1];
/// Observed request body: one minimal signed-64 field.
pub const MAX_REQUEST_BODY: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Not a complete, metadata-free category-0 `2050/21` request within bounds.
    IneligibleFrame,
    /// The body is not exactly a canonical `BLID` field.
    Body,
    /// The query names a different player than this connection's persona.
    Persona,
    /// A store with rows cannot be encoded by this policy.
    Unsupported,
    Encode,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "recommendations {self:?}")
    }
}
impl std::error::Error for Error {}

fn frame_limits() -> nfs_fire2::Limits {
    nfs_fire2::Limits::new(HEADER_LEN + MAX_REQUEST_BODY, 0, MAX_REQUEST_BODY)
        .expect("constant limits")
}

fn body_limits() -> Limits {
    Limits {
        max_bytes: MAX_REQUEST_BODY,
        max_values: 1,
        max_depth: 0,
        max_collection: 0,
        max_byte_string: 0,
    }
}

/// The decoded query: exactly one `BLID` integer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Request {
    pub blaze_id: i64,
}

impl Request {
    pub fn decode(body: &[u8]) -> Result<Self, Error> {
        let document = nfs_heat2::decode(body, body_limits()).map_err(|_| Error::Body)?;
        let mut blaze_id = None;
        for field in document.fields() {
            let field = field.map_err(|_| Error::Body)?;
            match (field.tag(), field.item().value(), blaze_id) {
                (BLID, Value::Integer(value), None) => blaze_id = Some(value),
                _ => return Err(Error::Body),
            }
        }
        Ok(Self {
            blaze_id: blaze_id.ok_or(Error::Body)?,
        })
    }

    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        let mut writer = Encoder::new(body_limits());
        writer
            .integer(BLID, self.blaze_id)
            .map_err(|_| Error::Encode)?;
        writer.finish().map_err(|_| Error::Encode)
    }
}

/// Per-account speed-wall store: one entry list per observed stat type.
/// Only the empty store is encodable; rows are retained as a count so a future
/// row codec cannot silently drop them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpeedWalls {
    rows: [usize; STAT_TYPES.len()],
}

impl Default for SpeedWalls {
    fn default() -> Self {
        Self::empty()
    }
}

impl SpeedWalls {
    pub const fn empty() -> Self {
        Self {
            rows: [0; STAT_TYPES.len()],
        }
    }

    pub fn is_empty(&self) -> bool {
        self.rows.iter().all(|n| *n == 0)
    }

    /// Minimal heat2 integer (the stat types and counts here are small and
    /// non-negative, so one byte each).
    fn small(value: i64) -> Result<u8, Error> {
        u8::try_from(value)
            .ok()
            .filter(|v| *v < 0x40)
            .ok_or(Error::Unsupported)
    }

    fn schema() -> Schema<'static> {
        const ROOT: &[Member] = &[Member {
            tag: SPWA,
            ty: TypeId(1),
        }];
        const WALL: &[Member] = &[Member {
            tag: [0xcf, 0x7a, 0x64], // SWID speedWallId
            ty: TypeId(2),
        }];
        const TYPES: &[Type<'static>] = &[
            Type::Struct(ROOT),
            Type::Map {
                key: TypeId(2),
                value: TypeId(3),
            },
            Type::Scalar(Kind::Integer),
            Type::Map {
                key: TypeId(2),
                value: TypeId(4),
            },
            Type::Struct(WALL),
        ];
        Schema::new(TYPES).expect("constant schema")
    }

    /// `InGameRecommendationsResponse` with `RECM` absent and `SPWA` holding an
    /// empty inner map per stat type. Byte layout follows the heat2 writer rules:
    /// field header (tag, kind 5), key kind, value collection byte, count, then
    /// each key as an integer followed by an inner map header with count 0.
    pub fn encode_response(&self) -> Result<Vec<u8>, Error> {
        if !self.is_empty() {
            return Err(Error::Unsupported);
        }
        let mut body = Vec::with_capacity(4 + 3 + STAT_TYPES.len() * 4);
        body.extend_from_slice(&SPWA);
        body.push(Kind::Map as u8);
        body.push(Kind::Integer as u8);
        body.push(COLLECTION);
        body.push(Self::small(STAT_TYPES.len() as i64)?);
        for stat in STAT_TYPES {
            body.push(Self::small(stat)?);
            body.push(Kind::Integer as u8);
            body.push(COLLECTION);
            body.push(0);
        }
        nfs_heat2::decode_with_schema(
            &body,
            Limits {
                max_bytes: 64,
                max_values: 8,
                max_depth: 3,
                max_collection: 8,
                max_byte_string: 0,
            },
            Self::schema(),
            TypeId(0),
        )
        .map_err(|_| Error::Encode)?;
        Ok(body)
    }
}

/// Per-connection service state.
#[derive(Debug, Default)]
pub struct Recommendations {
    walls: SpeedWalls,
    served: u64,
}

impl Recommendations {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn served(&self) -> u64 {
        self.served
    }

    pub fn speed_walls(&self) -> &SpeedWalls {
        &self.walls
    }

    /// Answer one complete `2050/21` query for `persona` (this connection's
    /// authenticated player). Other players, malformed bodies and any other frame
    /// get no reply and change nothing.
    pub fn reply(&mut self, wire: &[u8], persona: i64) -> Result<Vec<u8>, Error> {
        let limits = frame_limits();
        let decoded = nfs_fire2::decode(wire, limits)
            .map_err(|_| Error::IneligibleFrame)?
            .ok_or(Error::IneligibleFrame)?;
        let frame = decoded.frame;
        let fields = frame.fields;
        if decoded.consumed != wire.len()
            || fields.category != 0
            || fields.routing_a != COMPONENT
            || fields.routing_b != GET_IN_GAME_RECOMMENDATIONS
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
        if persona <= 0 || request.blaze_id != persona {
            return Err(Error::Persona);
        }
        let body = self.walls.encode_response()?;
        let reply = nfs_fire2::encode(
            Frame {
                fields: Fields {
                    category: 1,
                    ..fields
                },
                metadata: &[],
                body: &body,
            },
            nfs_fire2::Limits::new(HEADER_LEN + 64, 0, 64).expect("constant limits"),
        )
        .map_err(|_| Error::Encode)?;
        self.served += 1;
        Ok(reply)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PERSONA: i64 = 1_000_123;

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
            routing_b: GET_IN_GAME_RECOMMENDATIONS,
            correlation,
            ..Default::default()
        }
    }

    fn query(correlation: u32, blaze_id: i64) -> Vec<u8> {
        frame(
            fields(correlation),
            &[],
            &Request { blaze_id }.encode().unwrap(),
        )
    }

    #[test]
    fn empty_store_encodes_the_fifteen_byte_response() {
        let body = SpeedWalls::empty().encode_response().unwrap();
        assert_eq!(
            body,
            [
                0xcf, 0x0d, 0xe1, 5, // SPWA map
                0, 3, 2, // integer keys, collection values, two entries
                0, 0, 3, 0, // stat type 0 -> empty map
                1, 0, 3, 0, // stat type 1 -> empty map
            ]
        );
        assert!(
            nfs_heat2::decode(&body, Limits::default()).is_err(),
            "collection kind requires a schema"
        );
        let document = nfs_heat2::decode_with_schema(
            &body,
            Limits::default(),
            SpeedWalls::schema(),
            TypeId(0),
        )
        .unwrap();
        let field = document.fields().next().unwrap().unwrap();
        assert_eq!(field.tag(), SPWA);
        assert!(matches!(
            field.item().value(),
            Value::Map {
                key: Kind::Integer,
                count: 2,
                ..
            }
        ));
    }

    #[test]
    fn stores_with_rows_are_not_encoded() {
        let mut walls = SpeedWalls::empty();
        walls.rows[1] = 1;
        assert!(!walls.is_empty());
        assert_eq!(walls.encode_response(), Err(Error::Unsupported));
    }

    #[test]
    fn request_round_trips_with_minimal_integer() {
        let request = Request { blaze_id: PERSONA };
        let body = request.encode().unwrap();
        assert_eq!(&body[..4], &[0x8a, 0xca, 0x64, 0]);
        assert_eq!(Request::decode(&body).unwrap(), request);
        assert_eq!(Request::decode(&body[..body.len() - 1]), Err(Error::Body));
        let mut padded = body.clone();
        padded.push(0);
        assert_eq!(Request::decode(&padded), Err(Error::Body));
    }

    #[test]
    fn own_persona_query_is_answered_with_the_empty_response() {
        let mut state = Recommendations::new();
        let reply = state.reply(&query(173, PERSONA), PERSONA).unwrap();
        let decoded = nfs_fire2::decode(&reply, nfs_fire2::Limits::default())
            .unwrap()
            .unwrap();
        assert_eq!(
            decoded.frame.fields,
            Fields {
                category: 1,
                ..fields(173)
            }
        );
        assert!(decoded.frame.metadata.is_empty());
        assert_eq!(decoded.frame.body.len(), 15);
        assert_eq!(reply.len(), 31);
        assert_eq!(state.served(), 1);
        // Repeats are idempotent.
        assert_eq!(state.reply(&query(173, PERSONA), PERSONA).unwrap(), reply);
        assert_eq!(state.served(), 2);
    }

    #[test]
    fn other_players_and_invalid_personas_get_no_reply() {
        let mut state = Recommendations::new();
        assert_eq!(
            state.reply(&query(1, PERSONA + 1), PERSONA),
            Err(Error::Persona)
        );
        assert_eq!(
            state.reply(&query(2, -PERSONA), -PERSONA),
            Err(Error::Persona)
        );
        assert_eq!(state.reply(&query(3, 0), 0), Err(Error::Persona));
        assert_eq!(state.served(), 0);
    }

    #[test]
    fn other_routes_metadata_trailing_bytes_and_malformed_bodies_are_ineligible() {
        let mut state = Recommendations::new();
        let body = Request { blaze_id: PERSONA }.encode().unwrap();
        for wire in [
            frame(
                Fields {
                    routing_b: 26,
                    ..fields(1)
                },
                &[],
                &body,
            ),
            frame(
                Fields {
                    category: 1,
                    ..fields(1)
                },
                &[],
                &body,
            ),
            frame(
                Fields {
                    slot: 2,
                    ..fields(1)
                },
                &[],
                &body,
            ),
            frame(fields(1), &[0x8a, 0xca, 0x64, 0, 1], &body),
            frame(fields(1), &[], &[0u8; MAX_REQUEST_BODY + 1]),
            query(1, PERSONA)[..12].to_vec(),
        ] {
            assert_eq!(state.reply(&wire, PERSONA), Err(Error::IneligibleFrame));
        }
        let mut trailing = query(1, PERSONA);
        trailing.push(0);
        assert_eq!(state.reply(&trailing, PERSONA), Err(Error::IneligibleFrame));
        // Unknown field, wrong kind, duplicate field and empty body.
        let mut writer = Encoder::new(Limits::default());
        writer.integer(BLID, PERSONA).unwrap();
        writer.integer([0x8a, 0xca, 0x65], 1).unwrap();
        let unknown = writer.finish().unwrap();
        assert_eq!(
            state.reply(&frame(fields(2), &[], &unknown), PERSONA),
            Err(Error::Body)
        );
        let mut writer = Encoder::new(Limits::default());
        writer.string(BLID, b"1").unwrap();
        let string = writer.finish().unwrap();
        assert_eq!(
            state.reply(&frame(fields(3), &[], &string), PERSONA),
            Err(Error::Body)
        );
        let mut duplicate = body.clone();
        duplicate.extend_from_slice(&body);
        assert_eq!(Request::decode(&duplicate), Err(Error::Body));
        assert_eq!(
            state.reply(&frame(fields(4), &[], &[]), PERSONA),
            Err(Error::Body)
        );
        // A non-minimal integer (trailing zero continuation byte) decodes to the
        // same persona but is not the canonical encoding.
        let mut nonminimal = body.clone();
        *nonminimal.last_mut().unwrap() |= 0x80;
        nonminimal.push(0);
        assert_eq!(
            Request::decode(&nonminimal).unwrap(),
            Request { blaze_id: PERSONA }
        );
        assert_eq!(
            state.reply(&frame(fields(5), &[], &nonminimal), PERSONA),
            Err(Error::Body)
        );
        assert_eq!(state.served(), 0);
    }

    #[test]
    fn connections_are_isolated() {
        let mut a = Recommendations::new();
        let mut b = Recommendations::new();
        assert!(a.reply(&query(1, PERSONA), PERSONA).is_ok());
        assert_eq!(b.served(), 0);
        assert_eq!(
            b.reply(&query(1, PERSONA + 5), PERSONA),
            Err(Error::Persona)
        );
        assert_eq!(b.reply(&[1, 2, 3], PERSONA), Err(Error::IneligibleFrame));
    }
}

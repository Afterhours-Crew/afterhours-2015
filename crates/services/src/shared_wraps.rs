// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Shared-wrap listings for an empty local catalog (route 2052/23).
//! Requests must name the connection's current persona and supported item system.
//! Responses contain totalCount = 0 and omit sharedWraps. Populated catalogs and
//! user-list queries are unsupported.
use nfs_fire2::{Fields, Frame, HEADER_LEN};
use nfs_heat2::{Encoder, Limits, Value};

pub const COMPONENT: u16 = 2052;
pub const LIST_SHARED_WRAPS: u16 = 23;

pub const ITEM_SYSTEM_NAME: &[u8] = b"Items/GameItemSystem";
/// Native string capacity is 0x101 including the terminator.
pub const MAX_SYSTEM_NAME: usize = 256;
/// Observed bodies are 55 bytes; the bound covers the longest system name.
pub const MAX_REQUEST_BODY: usize = 320;

const CNT: [u8; 3] = [0x8e, 0xed, 0x00];
const CTYP: [u8; 3] = [0x8f, 0x4e, 0x70];
const FLTR: [u8; 3] = [0x9a, 0xcd, 0x32];
const ISNM: [u8; 3] = [0xa7, 0x3b, 0xad];
const PID: [u8; 3] = [0xc2, 0x99, 0x00];
const STRT: [u8; 3] = [0xcf, 0x4c, 0xb4];
const TCNT: [u8; 3] = [0xd2, 0x3b, 0xb4];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Not a complete, metadata-free category-0 `2052/23` request within bounds.
    IneligibleFrame,
    /// The body is not exactly the six observed fields in canonical encoding.
    Body,
    /// Another player's persona or another item system.
    Scope,
    /// A store with shared wraps cannot be encoded by this policy.
    Unsupported,
    Encode,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "shared wraps {self:?}")
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
        max_values: 6,
        max_depth: 0,
        max_collection: 0,
        max_byte_string: MAX_SYSTEM_NAME,
    }
}

/// The decoded query. `filter` keeps its numeric enum value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Request {
    pub count: u32,
    pub car_type_id: u32,
    pub filter: i64,
    pub item_system_name: Vec<u8>,
    pub persona_id: i64,
    pub start: u32,
}

fn unsigned(value: i64) -> Result<u32, Error> {
    u32::try_from(value).map_err(|_| Error::Body)
}

impl Request {
    pub fn decode(body: &[u8]) -> Result<Self, Error> {
        let document = nfs_heat2::decode(body, body_limits()).map_err(|_| Error::Body)?;
        let (mut count, mut car, mut filter, mut name, mut persona, mut start) =
            (None, None, None, None, None, None);
        for field in document.fields() {
            let field = field.map_err(|_| Error::Body)?;
            match (field.tag(), field.item().value()) {
                (CNT, Value::Integer(v)) if count.is_none() => count = Some(unsigned(v)?),
                (CTYP, Value::Integer(v)) if car.is_none() => car = Some(unsigned(v)?),
                (FLTR, Value::Integer(v)) if filter.is_none() => filter = Some(v),
                (ISNM, Value::String(s)) if name.is_none() => {
                    if s.is_empty() || s.len() > MAX_SYSTEM_NAME {
                        return Err(Error::Body);
                    }
                    name = Some(s.to_vec());
                }
                (PID, Value::Integer(v)) if persona.is_none() => persona = Some(v),
                (STRT, Value::Integer(v)) if start.is_none() => start = Some(unsigned(v)?),
                _ => return Err(Error::Body),
            }
        }
        Ok(Self {
            count: count.ok_or(Error::Body)?,
            car_type_id: car.ok_or(Error::Body)?,
            filter: filter.ok_or(Error::Body)?,
            item_system_name: name.ok_or(Error::Body)?,
            persona_id: persona.ok_or(Error::Body)?,
            start: start.ok_or(Error::Body)?,
        })
    }

    /// Canonical encoding in ascending tag order.
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        if self.item_system_name.is_empty() || self.item_system_name.len() > MAX_SYSTEM_NAME {
            return Err(Error::Body);
        }
        let mut w = Encoder::new(body_limits());
        let result = (|| {
            w.integer(CNT, i64::from(self.count))?;
            w.integer(CTYP, i64::from(self.car_type_id))?;
            w.integer(FLTR, self.filter)?;
            w.string(ISNM, &self.item_system_name)?;
            w.integer(PID, self.persona_id)?;
            w.integer(STRT, i64::from(self.start))
        })();
        result.map_err(|_| Error::Encode)?;
        w.finish().map_err(|_| Error::Encode)
    }
}

/// Per-account shared-wrap store. Only the empty store is encodable.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SharedWraps {
    wraps: usize,
}

impl SharedWraps {
    pub const fn empty() -> Self {
        Self { wraps: 0 }
    }

    pub fn is_empty(&self) -> bool {
        self.wraps == 0
    }

    /// `ListSharedWrapsResponse` for a page of an empty store: `TCNT = 0`,
    /// `sharedWraps` absent (the observed 5-byte body).
    pub fn encode_response(&self) -> Result<Vec<u8>, Error> {
        if !self.is_empty() {
            return Err(Error::Unsupported);
        }
        let mut w = Encoder::new(Limits {
            max_bytes: 16,
            max_values: 1,
            max_depth: 0,
            max_collection: 0,
            max_byte_string: 0,
        });
        w.integer(TCNT, 0).map_err(|_| Error::Encode)?;
        w.finish().map_err(|_| Error::Encode)
    }
}

/// Per-connection service state.
#[derive(Debug, Default)]
pub struct Service {
    store: SharedWraps,
    served: u64,
}

impl Service {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn served(&self) -> u64 {
        self.served
    }

    pub fn store(&self) -> &SharedWraps {
        &self.store
    }

    /// Answer one complete `2052/23` query for `persona` and the game's item
    /// system. Any other frame, body, persona or system gets no reply.
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
            || fields.routing_b != LIST_SHARED_WRAPS
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
        if persona <= 0
            || request.persona_id != persona
            || request.item_system_name != ITEM_SYSTEM_NAME
        {
            return Err(Error::Scope);
        }
        let body = self.store.encode_response()?;
        let reply = nfs_fire2::encode(
            Frame {
                fields: Fields {
                    category: 1,
                    ..fields
                },
                metadata: &[],
                body: &body,
            },
            nfs_fire2::Limits::new(HEADER_LEN + 16, 0, 16).expect("constant limits"),
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

    fn request(persona: i64) -> Request {
        Request {
            count: 50,
            car_type_id: 0,
            filter: 3,
            item_system_name: ITEM_SYSTEM_NAME.to_vec(),
            persona_id: persona,
            start: 0,
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
            routing_b: LIST_SHARED_WRAPS,
            correlation,
            ..Default::default()
        }
    }

    fn query(correlation: u32, request: &Request) -> Vec<u8> {
        frame(fields(correlation), &[], &request.encode().unwrap())
    }

    #[test]
    fn request_round_trips_in_tag_order() {
        let original = request(PERSONA);
        let body = original.encode().unwrap();
        assert_eq!(Request::decode(&body).unwrap(), original);
        assert!(body.len() < 55, "{}", body.len());
        let tags: Vec<[u8; 3]> = nfs_heat2::decode(&body, Limits::default())
            .unwrap()
            .fields()
            .map(|f| f.unwrap().tag())
            .collect();
        assert_eq!(tags, [CNT, CTYP, FLTR, ISNM, PID, STRT]);
    }

    #[test]
    fn empty_store_encodes_the_five_byte_response() {
        assert_eq!(
            SharedWraps::empty().encode_response().unwrap(),
            [0xd2, 0x3b, 0xb4, 0, 0]
        );
        let full = SharedWraps { wraps: 1 };
        assert_eq!(full.encode_response(), Err(Error::Unsupported));
    }

    #[test]
    fn own_persona_query_is_answered_and_repeats_are_idempotent() {
        let mut service = Service::new();
        let first = service
            .reply(&query(175, &request(PERSONA)), PERSONA)
            .unwrap();
        let decoded = nfs_fire2::decode(&first, nfs_fire2::Limits::default())
            .unwrap()
            .unwrap();
        assert_eq!(
            decoded.frame.fields,
            Fields {
                category: 1,
                ..fields(175)
            }
        );
        assert_eq!(decoded.frame.body, [0xd2, 0x3b, 0xb4, 0, 0]);
        assert_eq!(first.len(), 21);
        let second = service
            .reply(&query(176, &request(PERSONA)), PERSONA)
            .unwrap();
        assert_eq!(second.len(), 21);
        assert_eq!(service.served(), 2);
        assert!(service.store().is_empty());
    }

    #[test]
    fn other_personas_systems_and_invalid_personas_get_no_reply() {
        let mut service = Service::new();
        assert_eq!(
            service.reply(&query(1, &request(PERSONA + 1)), PERSONA),
            Err(Error::Scope)
        );
        assert_eq!(service.reply(&query(2, &request(0)), 0), Err(Error::Scope));
        let mut other_system = request(PERSONA);
        other_system.item_system_name = b"Items/OtherSystem".to_vec();
        assert_eq!(
            service.reply(&query(3, &other_system), PERSONA),
            Err(Error::Scope)
        );
        assert_eq!(service.served(), 0);
    }

    #[test]
    fn paging_and_filter_values_do_not_change_the_empty_answer() {
        let mut service = Service::new();
        let mut paged = request(PERSONA);
        paged.start = 50;
        paged.count = 10;
        paged.filter = 0;
        paged.car_type_id = 7;
        let reply = service.reply(&query(4, &paged), PERSONA).unwrap();
        assert_eq!(&reply[HEADER_LEN..], [0xd2, 0x3b, 0xb4, 0, 0]);
    }

    #[test]
    fn ineligible_frames_and_malformed_bodies_get_no_reply() {
        let mut service = Service::new();
        let body = request(PERSONA).encode().unwrap();
        for wire in [
            frame(
                Fields {
                    routing_b: 19,
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
                    reserved: [0, 1],
                    ..fields(1)
                },
                &[],
                &body,
            ),
            frame(fields(1), &[0x8e, 0xed, 0x00, 0, 1], &body),
            frame(fields(1), &[], &[0u8; MAX_REQUEST_BODY + 1]),
            query(1, &request(PERSONA))[..20].to_vec(),
        ] {
            assert_eq!(service.reply(&wire, PERSONA), Err(Error::IneligibleFrame));
        }
        let mut trailing = query(1, &request(PERSONA));
        trailing.push(0);
        assert_eq!(
            service.reply(&trailing, PERSONA),
            Err(Error::IneligibleFrame)
        );
        // Missing field, extra (user list) field, wrong kind, negative counter,
        // oversized name and a non-minimal integer.
        let mut w = Encoder::new(Limits::default());
        w.integer(CNT, 50).unwrap();
        w.integer(CTYP, 0).unwrap();
        w.integer(FLTR, 3).unwrap();
        w.string(ISNM, ITEM_SYSTEM_NAME).unwrap();
        w.integer(PID, PERSONA).unwrap();
        let missing = w.finish().unwrap();
        assert_eq!(Request::decode(&missing), Err(Error::Body));
        let mut w = Encoder::new(Limits::default());
        w.integer(CNT, 50).unwrap();
        w.integer(CTYP, 0).unwrap();
        w.integer(FLTR, 3).unwrap();
        w.string(ISNM, ITEM_SYSTEM_NAME).unwrap();
        w.integer(PID, PERSONA).unwrap();
        w.integer(STRT, 0).unwrap();
        w.integer_list([0xd3, 0x5b, 0x00], [PERSONA].into_iter())
            .unwrap();
        let with_users = w.finish().unwrap();
        assert_eq!(
            service.reply(&frame(fields(2), &[], &with_users), PERSONA),
            Err(Error::Body)
        );
        let mut w = Encoder::new(Limits::default());
        w.string(CNT, b"50").unwrap();
        w.integer(CTYP, 0).unwrap();
        w.integer(FLTR, 3).unwrap();
        w.string(ISNM, ITEM_SYSTEM_NAME).unwrap();
        w.integer(PID, PERSONA).unwrap();
        w.integer(STRT, 0).unwrap();
        assert_eq!(Request::decode(&w.finish().unwrap()), Err(Error::Body));
        let mut w = Encoder::new(Limits::default());
        w.integer(CNT, -1).unwrap();
        w.integer(CTYP, 0).unwrap();
        w.integer(FLTR, 3).unwrap();
        w.string(ISNM, ITEM_SYSTEM_NAME).unwrap();
        w.integer(PID, PERSONA).unwrap();
        w.integer(STRT, 0).unwrap();
        assert_eq!(Request::decode(&w.finish().unwrap()), Err(Error::Body));
        let mut long = request(PERSONA);
        long.item_system_name = vec![b'a'; MAX_SYSTEM_NAME + 1];
        assert_eq!(long.encode(), Err(Error::Body));
        let mut nonminimal = body.clone();
        // The final STRT integer is one byte; extend it with a zero continuation.
        *nonminimal.last_mut().unwrap() |= 0x80;
        nonminimal.push(0);
        assert_eq!(Request::decode(&nonminimal).unwrap(), request(PERSONA));
        assert_eq!(
            service.reply(&frame(fields(3), &[], &nonminimal), PERSONA),
            Err(Error::Body)
        );
        assert_eq!(service.served(), 0);
    }

    #[test]
    fn connections_are_isolated() {
        let mut a = Service::new();
        let mut b = Service::new();
        assert!(a.reply(&query(1, &request(PERSONA)), PERSONA).is_ok());
        assert_eq!(b.served(), 0);
        assert_eq!(
            b.reply(&query(1, &request(PERSONA + 1)), PERSONA),
            Err(Error::Scope)
        );
        assert_eq!(b.reply(&[0; 3], PERSONA), Err(Error::IneligibleFrame));
    }
}

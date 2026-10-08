// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Pure latency QoS request and datagram codecs.
//!
//! Query order/canonical spelling, exact probe length and reserved-byte rejection
//! are strict replacement policies. The caller owns HTTP framing, method/body
//! validation, endpoint selection, TLS, datagrams and resource budgets. No time,
//! network I/O, session state or bandwidth/firewall response lives here.

use std::{
    fmt,
    net::{Ipv4Addr, SocketAddrV4},
};

const TARGET_PREFIX: &[u8] = b"/qos/qos?vers=1&qtyp=1&prpt=";
pub const MAX_LATENCY_TARGET_BYTES: usize = TARGET_PREFIX.len() + 5;
pub const MAX_LATENCY_XML_BYTES: usize = 60;
pub const LATENCY_REQUEST_BYTES: usize = 20;
pub const LATENCY_RESPONSE_BYTES: usize = 30;

/// Fixed errors never retain a request, address, ID or tick for logging.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    TargetLength,
    UnsupportedTarget,
    InvalidPort,
    InvalidAddress,
    ProbeLength,
    ZeroRequestId,
    UnsupportedProbeType,
    NonZeroReserved,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "latency QoS {self:?}")
    }
}

impl std::error::Error for Error {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LatencyQuery {
    requested_port: u16,
}

impl LatencyQuery {
    /// The advertised request port, not a substitute for an observed UDP peer.
    pub fn requested_port(self) -> u16 {
        self.requested_port
    }
}

/// Parse a complete HTTP request target in the one observed latency-query form.
///
/// Accept only version 1, type 1 and a canonical decimal nonzero u16 port, in
/// the observed order. A valid shorter decimal port is itself a complete target;
/// detecting an incomplete HTTP request is the transport's responsibility.
pub fn parse_latency_target(target: &[u8]) -> Result<LatencyQuery, Error> {
    if target.len() > MAX_LATENCY_TARGET_BYTES {
        return Err(Error::TargetLength);
    }
    let port = target
        .strip_prefix(TARGET_PREFIX)
        .ok_or(Error::UnsupportedTarget)?;
    if port.is_empty() || port[0] == b'0' {
        return Err(Error::InvalidPort);
    }
    let requested_port = port.iter().try_fold(0_u16, |value, digit| {
        if !digit.is_ascii_digit() {
            return Err(Error::InvalidPort);
        }
        value
            .checked_mul(10)
            .and_then(|value| value.checked_add(u16::from(*digit - b'0')))
            .ok_or(Error::InvalidPort)
    })?;
    Ok(LatencyQuery { requested_port })
}

/// Encode only the minimal type-1 response body, at most 60 ASCII bytes.
///
/// HTTP status, headers and an owned endpoint are supplied by the transport.
/// No loopback/public-address policy is imposed by this byte codec.
pub fn encode_latency_xml(address: Ipv4Addr, port: u16) -> Result<Vec<u8>, Error> {
    validate_endpoint(address, port)?;
    Ok(format!(
        "<qos><qosip>{}</qosip><qosport>{port}</qosport></qos>",
        u32::from_be_bytes(address.octets())
    )
    .into_bytes())
}

/// Validated opaque request bytes; deliberately no Debug implementation.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct LatencyProbe {
    request: [u8; LATENCY_REQUEST_BYTES],
}

impl LatencyProbe {
    /// Accept an exact type-1 datagram. IDs and ticks are preserved as bytes.
    pub fn parse(request: &[u8]) -> Result<Self, Error> {
        let request: [u8; LATENCY_REQUEST_BYTES] =
            request.try_into().map_err(|_| Error::ProbeLength)?;
        if request[..4] == [0; 4] {
            return Err(Error::ZeroRequestId);
        }
        if request[4..8] != [0, 0, 0, 1] {
            return Err(Error::UnsupportedProbeType);
        }
        if request[8..16] != [0; 8] {
            return Err(Error::NonZeroReserved);
        }
        Ok(Self { request })
    }

    /// Echo the probe and append the actual observed peer, in network order.
    /// The transport supplies this peer; no query-port or global-player fallback.
    pub fn encode_reply(
        self,
        observed_peer: SocketAddrV4,
    ) -> Result<[u8; LATENCY_RESPONSE_BYTES], Error> {
        validate_endpoint(*observed_peer.ip(), observed_peer.port())?;
        let mut response = [0_u8; LATENCY_RESPONSE_BYTES];
        response[..LATENCY_REQUEST_BYTES].copy_from_slice(&self.request);
        response[20..24].copy_from_slice(&observed_peer.ip().octets());
        response[24..26].copy_from_slice(&observed_peer.port().to_be_bytes());
        Ok(response)
    }
}

fn validate_endpoint(address: Ipv4Addr, port: u16) -> Result<(), Error> {
    if address.is_unspecified() {
        return Err(Error::InvalidAddress);
    }
    if port == 0 {
        return Err(Error::InvalidPort);
    }
    Ok(())
}

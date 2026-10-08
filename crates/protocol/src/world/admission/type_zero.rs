//! Pure fields for the type 0 identity interface. Caller owns session policy.
//! The outer Decoded retains physical padding for exact round trips.
use super::{Decoded, Message};
use std::fmt;

/// The outer transformed length is u8; Client5's fixed inner prefix uses 11 bytes.
pub const MAX_CALLBACK_BYTES: usize = u8::MAX as usize - 11;
/// Big-endian identity-transform fields, with opaque tokens and retained callback data.
#[derive(Clone, PartialEq, Eq)]
pub struct Client5 {
    pub peer_echo: u32,
    pub client_fresh: u32,
    pub client_selector: u16,
    pub callback: Vec<u8>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
/// Client-word echo followed by an independent injected host word; no acceptance claim.
pub struct Host6 {
    pub client_echo: u32,
    pub host_fresh: u32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Envelope,
    Kind,
    Length,
    Binding,
    Profile,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "type 0 admission {self:?}")
    }
}
impl std::error::Error for Error {}
impl fmt::Debug for Client5 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client5")
            .field("callback_bytes", &self.callback.len())
            .finish_non_exhaustive()
    }
}
impl fmt::Debug for Host6 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Host6 { opaque_fields: redacted }")
    }
}
fn word(bytes: &[u8]) -> u32 {
    u32::from_be_bytes(bytes.try_into().unwrap())
}
impl Client5 {
    /// Read this explicit identity profile from an already-decoded admission envelope.
    pub fn decode(decoded: &Decoded) -> Result<Self, Error> {
        let Message::Transformed {
            kind: 5, opaque, ..
        } = &decoded.message
        else {
            return Err(Error::Kind);
        };
        if opaque.len() < 11 || opaque.len() != 11 + usize::from(opaque[10]) {
            return Err(Error::Length);
        }
        Ok(Self {
            peer_echo: word(&opaque[..4]),
            client_fresh: word(&opaque[4..8]),
            client_selector: u16::from_be_bytes(opaque[8..10].try_into().unwrap()),
            callback: opaque[11..].to_vec(),
        })
    }
    pub fn qualify_empty(
        &self,
        decoded: &Decoded,
        expected_host_selector: u16,
        expected_peer_echo: u32,
    ) -> Result<(), Error> {
        let Message::Transformed {
            kind: 5, selector, ..
        } = &decoded.message
        else {
            return Err(Error::Kind);
        };
        if Self::decode(decoded)? != *self {
            return Err(Error::Binding);
        }
        if *selector != expected_host_selector
            || expected_host_selector == 0
            || expected_host_selector >= 1 << 14
            || self.peer_echo != expected_peer_echo
        {
            return Err(Error::Binding);
        }
        if self.client_selector == 0 || self.client_selector >= 1 << 14 || !self.callback.is_empty()
        {
            return Err(Error::Profile);
        }
        Ok(())
    }
    pub fn new_body(&self, host_selector: u16) -> Result<Vec<u8>, Error> {
        if self.callback.len() > MAX_CALLBACK_BYTES {
            return Err(Error::Length);
        }
        let mut opaque = Vec::with_capacity(11 + self.callback.len());
        opaque.extend(self.peer_echo.to_be_bytes());
        opaque.extend(self.client_fresh.to_be_bytes());
        opaque.extend(self.client_selector.to_be_bytes());
        opaque.push(self.callback.len() as u8);
        opaque.extend_from_slice(&self.callback);
        Decoded::new(Message::Transformed {
            kind: 5,
            selector: host_selector,
            opaque,
        })
        .and_then(|d| d.encode())
        .map_err(|_| Error::Envelope)
    }
}
impl Host6 {
    pub fn decode(decoded: &Decoded) -> Result<Self, Error> {
        let Message::Transformed {
            kind: 6, opaque, ..
        } = &decoded.message
        else {
            return Err(Error::Kind);
        };
        if opaque.len() != 8 {
            return Err(Error::Length);
        }
        Ok(Self {
            client_echo: word(&opaque[..4]),
            host_fresh: word(&opaque[4..]),
        })
    }
    pub fn new_body(self, client_selector: u16) -> Result<Vec<u8>, Error> {
        let mut opaque = Vec::with_capacity(8);
        opaque.extend(self.client_echo.to_be_bytes());
        opaque.extend(self.host_fresh.to_be_bytes());
        Decoded::new(Message::Transformed {
            kind: 6,
            selector: client_selector,
            opaque,
        })
        .and_then(|d| d.encode())
        .map_err(|_| Error::Envelope)
    }
}

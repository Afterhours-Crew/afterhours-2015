//! World admission messages and untransformed zero-tail envelopes.
//! Unused physical bits are preserved. Field ownership and local admission
//! are separate policy.
use super::{BitSpan, envelope::ZeroTail};
use std::fmt;

pub mod type_zero;

/// Explicit local resource bound; the native Client2 length field is 16 bits.
pub const MAX_CONNECT_BYTES: usize = 1024;
pub const MAX_BODY: usize = MAX_CONNECT_BYTES + 14;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Limit,
    Envelope,
    Kind,
    Selector,
    Length,
    Truncated,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "application admission {self:?}")
    }
}
impl std::error::Error for Error {}
/// Names refer to the observed direction/ordering. Tokens, selector allocation,
/// callback transforms and current peer association remain caller-owned policy.
#[derive(Clone, Eq, PartialEq)]
pub enum Message {
    Host1 {
        engine_token: u32,
    },
    Client2 {
        peer_token: u32,
        engine_echo: u32,
        opaque: Vec<u8>,
    },
    Host9,
    Host4 {
        peer_echo: u32,
        assigned_selector: u16,
        callback_kind: u8,
        opaque: Vec<u8>,
    },
    Transformed {
        kind: u8,
        selector: u16,
        opaque: Vec<u8>,
    },
}
impl fmt::Debug for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (kind, bytes) = match self {
            Self::Host1 { .. } => (1, 0),
            Self::Client2 { opaque, .. } => (2, opaque.len()),
            Self::Host4 { opaque, .. } => (4, opaque.len()),
            Self::Host9 => (9, 0),
            Self::Transformed { kind, opaque, .. } => (*kind, opaque.len()),
        };
        f.debug_struct("AdmissionMessage")
            .field("kind", &kind)
            .field("opaque_bytes", &bytes)
            .finish_non_exhaustive()
    }
}
/// An owned structural message with retained unused physical final bits.
/// Decoding does not admit a peer or interpret transformed callback contents.
#[derive(Clone, Eq, PartialEq)]
pub struct Decoded {
    pub message: Message,
    padding: u8,
}
impl fmt::Debug for Decoded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(f)
    }
}
fn field(p: BitSpan<'_>, offset: usize, width: u8) -> Result<u32, Error> {
    p.read_u32(offset, width).map_err(|_| Error::Truncated)
}
fn blob(p: BitSpan<'_>, offset: usize, width: u8, max: usize) -> Result<Vec<u8>, Error> {
    let len = field(p, offset, width)? as usize;
    if len > max {
        return Err(Error::Limit);
    }
    let start = offset + width as usize;
    if p.len() != start + len * 8 {
        return Err(Error::Length);
    }
    (0..len)
        .map(|i| field(p, start + i * 8, 8).map(|v| v as u8))
        .collect()
}
impl Decoded {
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() > MAX_BODY {
            return Err(Error::Limit);
        }
        let e = ZeroTail::new(MAX_BODY)
            .unwrap()
            .decode(bytes)
            .map_err(|_| Error::Envelope)?;
        let p = e.payload();
        let selector = e.connection_selector();
        if matches!(e.kind(), 1 | 2 | 4 | 9) && selector != 0 {
            return Err(Error::Selector);
        }
        let message = match e.kind() {
            1 => {
                if p.len() != 32 {
                    return Err(Error::Length);
                }
                Message::Host1 {
                    engine_token: field(p, 0, 32)?,
                }
            }
            2 => Message::Client2 {
                peer_token: field(p, 0, 32)?,
                engine_echo: field(p, 32, 32)?,
                opaque: blob(p, 64, 16, MAX_CONNECT_BYTES)?,
            },
            4 => Message::Host4 {
                peer_echo: field(p, 0, 32)?,
                assigned_selector: field(p, 32, 16)? as u16,
                callback_kind: field(p, 48, 8)? as u8,
                opaque: blob(p, 56, 8, 255)?,
            },
            9 => {
                if !p.is_empty() {
                    return Err(Error::Length);
                }
                Message::Host9
            }
            5 | 6 => {
                if selector == 0 {
                    return Err(Error::Selector);
                }
                Message::Transformed {
                    kind: e.kind(),
                    selector,
                    opaque: blob(p, 0, 8, 255)?,
                }
            }
            _ => return Err(Error::Kind),
        };
        let unused = bytes.len() * 8 - e.effective_bits();
        let padding = if unused == 0 {
            0
        } else {
            bytes[bytes.len() - 1] & ((1 << unused) - 1)
        };
        Ok(Self { message, padding })
    }
    pub fn new(message: Message) -> Result<Self, Error> {
        let d = Self {
            message,
            padding: 0,
        };
        d.encode()?;
        Ok(d)
    }
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        let (kind, selector) = match &self.message {
            Message::Host1 { .. } => (1, 0),
            Message::Client2 { .. } => (2, 0),
            Message::Host4 { .. } => (4, 0),
            Message::Host9 => (9, 0),
            Message::Transformed { kind, selector, .. } => {
                if !matches!(kind, 5 | 6) {
                    return Err(Error::Kind);
                }
                if *selector == 0 || *selector >= 16384 {
                    return Err(Error::Selector);
                }
                (*kind, *selector)
            }
        };
        let mut w = Writer::new(kind, selector);
        match &self.message {
            Message::Host1 { engine_token } => w.put(*engine_token, 32),
            Message::Client2 {
                peer_token,
                engine_echo,
                opaque,
            } => {
                if opaque.len() > MAX_CONNECT_BYTES {
                    return Err(Error::Limit);
                }
                w.put(*peer_token, 32);
                w.put(*engine_echo, 32);
                w.put(opaque.len() as u32, 16);
                w.bytes(opaque);
            }
            Message::Host4 {
                peer_echo,
                assigned_selector,
                callback_kind,
                opaque,
            } => {
                if opaque.len() > 255 {
                    return Err(Error::Limit);
                }
                w.put(*peer_echo, 32);
                w.put(u32::from(*assigned_selector), 16);
                w.put(u32::from(*callback_kind), 8);
                w.put(opaque.len() as u32, 8);
                w.bytes(opaque);
            }
            Message::Host9 => (),
            Message::Transformed { opaque, .. } => {
                if opaque.len() > 255 {
                    return Err(Error::Limit);
                }
                w.put(opaque.len() as u32, 8);
                w.bytes(opaque);
            }
        }
        Ok(w.finish(self.padding))
    }
}
struct Writer {
    bytes: Vec<u8>,
    bits: usize,
}
impl Writer {
    fn new(kind: u8, selector: u16) -> Self {
        let mut w = Self {
            bytes: vec![],
            bits: 0,
        };
        w.put(0, 6);
        w.put(selector as u32, 14);
        w.put(kind as u32, 8);
        w
    }
    fn put(&mut self, v: u32, width: usize) {
        for i in (0..width).rev() {
            if self.bits.is_multiple_of(8) {
                self.bytes.push(0);
            }
            self.bytes[self.bits / 8] |= (((v >> i) & 1) as u8) << (7 - self.bits % 8);
            self.bits += 1;
        }
    }
    fn bytes(&mut self, b: &[u8]) {
        for v in b {
            self.put(*v as u32, 8);
        }
    }
    fn finish(mut self, padding: u8) -> Vec<u8> {
        let rem = self.bits % 8;
        self.bytes[0] |= (rem as u8) << 5;
        if rem != 0 {
            let last = self.bytes.len() - 1;
            self.bytes[last] |= padding & ((1 << (8 - rem)) - 1);
        }
        self.bytes
    }
}

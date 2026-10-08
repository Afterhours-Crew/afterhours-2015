//! Kind-20 timing controls.
//! Raw timestamp words are opaque. Session clocks, history and write ownership
//! are caller policy; structural decoding does not prove timing acceptance.
use super::{
    envelope::{ZeroTail, checksum},
    payload::Route,
};
use std::fmt;

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum Control {
    Request { timestamp: u64 },
    Reply { echo: u64, clock: u64 },
}
impl fmt::Debug for Control {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Request { .. } => "Request { .. }",
            Self::Reply { .. } => "Reply { .. }",
        })
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Bounds,
    Envelope,
    Kind,
    Shape,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "application timing {self:?}")
    }
}
impl std::error::Error for Error {}

/// Supported zero-tail controls with retained three physical padding bits.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct Decoded {
    selector: u16,
    number: u16,
    acknowledgement: u16,
    history: u32,
    control: Control,
    padding: u8,
}
impl fmt::Debug for Decoded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApplicationTiming")
            .field("control", &self.control)
            .finish_non_exhaustive()
    }
}
impl Decoded {
    pub fn new(
        selector: u16,
        number: u16,
        acknowledgement: u16,
        history: u32,
        control: Control,
    ) -> Result<Self, Error> {
        if !(1..=0x3fff).contains(&selector) || number >= 1024 || acknowledgement >= 1024 {
            return Err(Error::Bounds);
        }
        Ok(Self {
            selector,
            number,
            acknowledgement,
            history,
            control,
            padding: 0,
        })
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        let e = ZeroTail::new(29)
            .unwrap()
            .decode(bytes)
            .map_err(|_| Error::Envelope)?;
        if e.kind() != 20 {
            return Err(Error::Kind);
        }
        let s = e.sequence().ok_or(Error::Shape)?;
        let control = match Route::decode(e.payload()).map_err(|_| Error::Shape)? {
            Route::Control {
                subtype: 2,
                timestamps: [Some(timestamp), None],
                body,
            } if body.is_empty() => Control::Request { timestamp },
            Route::Control {
                subtype: 3,
                timestamps: [Some(echo), Some(clock)],
                body,
            } if body.is_empty() => Control::Reply { echo, clock },
            _ => return Err(Error::Shape),
        };
        let expected = match control {
            Control::Request { .. } => 21,
            Control::Reply { .. } => 29,
        };
        if bytes.len() != expected {
            return Err(Error::Shape);
        }
        let mut d = Self::new(
            e.connection_selector(),
            s.number().value(),
            s.acknowledgement().value(),
            s.history(),
            control,
        )?;
        d.padding = bytes[bytes.len() - 3] & 7;
        Ok(d)
    }
    pub fn encode(&self) -> Vec<u8> {
        let payload_bits: usize = match self.control {
            Control::Request { .. } => 69,
            Control::Reply { .. } => 133,
        };
        let bytes = (80 + payload_bits).div_ceil(8) + 2;
        let mut output = vec![0; bytes];
        let mut put = |offset: usize, width: usize, value: u64| {
            for i in 0..width {
                let k = offset + i;
                output[k / 8] |= (((value >> (width - i - 1)) & 1) as u8) << (7 - k % 8);
            }
        };
        put(0, 3, ((80 + payload_bits + 16) % 8) as u64);
        put(6, 14, u64::from(self.selector));
        put(20, 8, 20);
        put(28, 10, u64::from(self.number));
        put(38, 10, u64::from(self.acknowledgement));
        put(48, 32, u64::from(self.history));
        put(80, 1, 1);
        match self.control {
            Control::Request { timestamp } => {
                put(81, 4, 2);
                put(85, 64, timestamp);
            }
            Control::Reply { echo, clock } => {
                put(81, 4, 3);
                put(85, 64, echo);
                put(149, 64, clock);
            }
        }
        output[bytes - 3] |= self.padding;
        let check = checksum(&output[..bytes - 2]).to_be_bytes();
        output[bytes - 2..].copy_from_slice(&check);
        output
    }
    pub fn selector(self) -> u16 {
        self.selector
    }
    pub fn number(self) -> u16 {
        self.number
    }
    pub fn acknowledgement(self) -> u16 {
        self.acknowledgement
    }
    pub fn history(self) -> u32 {
        self.history
    }
    pub fn control(self) -> Control {
        self.control
    }
    pub fn padding(self) -> u8 {
        self.padding
    }
}

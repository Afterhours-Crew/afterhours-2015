// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Launcher transform: AES-128 on independent 16-byte blocks, count padding,
//! lowercase hex and parameter-derived keys.
//!
//! The encoder adds a full padding block for aligned input. Decoding retains
//! the compatibility behavior of zero-terminated output even when the padding
//! status reports an error. Invalid hex is rejected instead of emulating
//! permissive conversion. These are compatibility codecs, not session policy.
//!
//! The arithmetic is this project's own, written from NIST FIPS 197 sections
//! 4 and 5, with NIST SP 800-38A known-answer tests. No third-party
//! implementation or game bytes are included.

use std::fmt;

/// Local resource ceilings, not wire limits.
pub const MAX_PLAIN: usize = 1 << 16;
pub const MAX_HEX: usize = (MAX_PLAIN + 16) * 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Input beyond the local ceiling.
    Bound,
    /// A hex pair that is not two hex digits. The native `%x` conversion
    /// ignores failures; that behaviour is not modeled, so it is refused.
    Hex,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "lsx cipher {self:?}")
    }
}
impl std::error::Error for Error {}

/// What the game's decoder would make of a hex message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Decoded {
    /// `None`: the wrapper short-circuited before the block decoder (fewer
    /// than two hex characters). Otherwise the block decoder's status: the
    /// plaintext length, or -7 for misalignment or bad padding.
    pub status: Option<i32>,
    /// The bytes the wrapper exposes: everything written up to the first NUL.
    pub visible: Vec<u8>,
}

/// AES-128 with the expanded key schedule.
#[derive(Clone)]
pub struct Aes128 {
    round_keys: [[u8; 16]; 11],
    sbox: [u8; 256],
    inverse_sbox: [u8; 256],
}

impl fmt::Debug for Aes128 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Aes128 { .. }")
    }
}

fn gf(mut a: u8, mut b: u8) -> u8 {
    let mut result = 0;
    for _ in 0..8 {
        if b & 1 != 0 {
            result ^= a;
        }
        a = (a << 1) ^ if a & 0x80 != 0 { 0x1b } else { 0 };
        b >>= 1;
    }
    result
}

fn inverse(a: u8) -> u8 {
    if a == 0 {
        return 0;
    }
    let (mut result, mut factor, mut exponent) = (1, a, 254);
    while exponent != 0 {
        if exponent & 1 != 0 {
            result = gf(result, factor);
        }
        factor = gf(factor, factor);
        exponent >>= 1;
    }
    result
}

impl Aes128 {
    pub fn new(key: [u8; 16]) -> Self {
        let mut sbox = [0; 256];
        let mut inverse_sbox = [0; 256];
        for (i, entry) in sbox.iter_mut().enumerate() {
            let v = inverse(i as u8);
            *entry = v
                ^ v.rotate_left(1)
                ^ v.rotate_left(2)
                ^ v.rotate_left(3)
                ^ v.rotate_left(4)
                ^ 0x63;
            inverse_sbox[usize::from(*entry)] = i as u8;
        }
        let mut bytes = [0; 176];
        bytes[..16].copy_from_slice(&key);
        let mut rcon = 1;
        for word in 4..44 {
            let base = word * 4;
            let mut previous: [u8; 4] = bytes[base - 4..base].try_into().expect("4 bytes");
            if word % 4 == 0 {
                previous.rotate_left(1);
                previous = previous.map(|b| sbox[usize::from(b)]);
                previous[0] ^= rcon;
                rcon = gf(rcon, 2);
            }
            for (i, b) in previous.into_iter().enumerate() {
                bytes[base + i] = bytes[base - 16 + i] ^ b;
            }
        }
        let round_keys =
            std::array::from_fn(|r| bytes[r * 16..r * 16 + 16].try_into().expect("16 bytes"));
        Self {
            round_keys,
            sbox,
            inverse_sbox,
        }
    }

    /// The parameter-zero key the handshake uses on both sides.
    pub fn parameter_zero() -> Self {
        Self::new(zero_parameter_key())
    }

    /// The key for a nonzero transport parameter ; parameter
    /// zero gives [`Self::parameter_zero`].
    pub fn for_parameter(parameter: u32) -> Self {
        Self::new(parameter_key(parameter))
    }

    pub fn encrypt(&self, mut block: [u8; 16]) -> [u8; 16] {
        add_key(&mut block, &self.round_keys[0]);
        for round in 1..=10 {
            block = block.map(|v| self.sbox[usize::from(v)]);
            shift_rows(&mut block, false);
            if round < 10 {
                mix_columns(&mut block, false);
            }
            add_key(&mut block, &self.round_keys[round]);
        }
        block
    }

    pub fn decrypt(&self, mut block: [u8; 16]) -> [u8; 16] {
        add_key(&mut block, &self.round_keys[10]);
        for round in (0..10).rev() {
            shift_rows(&mut block, true);
            block = block.map(|v| self.inverse_sbox[usize::from(v)]);
            add_key(&mut block, &self.round_keys[round]);
            if round != 0 {
                mix_columns(&mut block, true);
            }
        }
        block
    }

    /// The game's encoder: count padding (a full block of 16 for aligned
    /// input), independent blocks, lowercase hex. Empty input gives an empty
    /// string (the encoder's zero-return branch).
    pub fn encode_hex(&self, plain: &[u8]) -> Result<String, Error> {
        if plain.len() > MAX_PLAIN {
            return Err(Error::Bound);
        }
        if plain.is_empty() {
            return Ok(String::new());
        }
        let pad = 16 - plain.len() % 16;
        let mut padded = plain.to_vec();
        padded.resize(plain.len() + pad, pad as u8);
        let bytes: Vec<u8> = padded
            .as_chunks::<16>()
            .0
            .iter()
            .flat_map(|v| self.encrypt(*v))
            .collect();
        Ok(lower_hex(&bytes))
    }

    /// The game's decoder and wrapper (see the module notes). An odd final
    /// character is dropped before conversion, as the native length halving does.
    pub fn decode_hex(&self, hex: &[u8]) -> Result<Decoded, Error> {
        if hex.len() > MAX_HEX {
            return Err(Error::Bound);
        }
        let bytes: Vec<u8> = hex
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| Ok((nibble(pair[0])? << 4) | nibble(pair[1])?))
            .collect::<Result<_, Error>>()?;
        if bytes.is_empty() {
            return Ok(Decoded {
                status: None,
                visible: Vec::new(),
            });
        }
        if !bytes.len().is_multiple_of(16) {
            return Ok(Decoded {
                status: Some(-7),
                visible: Vec::new(),
            });
        }
        let mut written = Vec::with_capacity(bytes.len());
        let blocks = bytes.as_chunks::<16>().0.iter();
        let n = blocks.len();
        let mut status = -7;
        for (i, block) in blocks.enumerate() {
            let plain = self.decrypt(*block);
            if i + 1 != n {
                written.extend_from_slice(&plain);
                continue;
            }
            let count = usize::from(plain[15]);
            // Mode zero allows count 0 and rejects 16 or more.
            if count < 16 && plain[16 - count..].iter().all(|v| usize::from(*v) == count) {
                written.extend_from_slice(&plain[..16 - count]);
                status = (n * 16 - count) as i32;
            }
        }
        let visible = written.into_iter().take_while(|b| *b != 0).collect();
        Ok(Decoded {
            status: Some(status),
            visible,
        })
    }
}

fn add_key(block: &mut [u8; 16], key: &[u8; 16]) {
    for (b, k) in block.iter_mut().zip(key) {
        *b ^= k;
    }
}

fn shift_rows(block: &mut [u8; 16], inverse: bool) {
    let before = *block;
    for row in 0..4 {
        for col in 0..4 {
            let source = if inverse {
                (col + 4 - row) % 4
            } else {
                (col + row) % 4
            };
            block[4 * col + row] = before[4 * source + row];
        }
    }
}

fn mix_columns(block: &mut [u8; 16], inverse: bool) {
    let coefficients = if inverse {
        [14, 11, 13, 9]
    } else {
        [2, 3, 1, 1]
    };
    for column in block.as_chunks_mut::<4>().0 {
        let before = *column;
        for (row, value) in column.iter_mut().enumerate() {
            *value = (0..4)
                .map(|j| gf(before[j], coefficients[(j + 4 - row) % 4]))
                .fold(0, |a, b| a ^ b);
        }
    }
}

fn nibble(b: u8) -> Result<u8, Error> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(Error::Hex),
    }
}

pub fn lower_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    bytes
        .iter()
        .flat_map(|b| {
            [
                char::from(DIGITS[usize::from(b >> 4)]),
                char::from(DIGITS[usize::from(b & 15)]),
            ]
        })
        .collect()
}

/// The parameter-zero key: the transformer constructor stores the loop
/// counter as each of the 16 key bytes.
pub fn zero_parameter_key() -> [u8; 16] {
    std::array::from_fn(|i| i as u8)
}

/// The C runtime generator the game's transformer uses for a nonzero
/// parameter (MSVCR120 `rand`/`srand`, ): state `s' = 214013 s + 2531011`
/// modulo 2^32, output bits 16..30.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CrtRandom {
    state: u32,
}

impl CrtRandom {
    pub fn seeded(seed: u32) -> Self {
        Self { state: seed }
    }

    pub fn next_output(&mut self) -> u32 {
        self.state = self.state.wrapping_mul(214_013).wrapping_add(2_531_011);
        (self.state >> 16) & 0x7fff
    }
}

/// The key for a nonzero transport parameter : seed 7, take one output,
/// reseed with `parameter + output`, then the low byte of 16 outputs. Zero
/// gives the parameter-zero key.
pub fn parameter_key(parameter: u32) -> [u8; 16] {
    if parameter == 0 {
        return zero_parameter_key();
    }
    let mut random = CrtRandom::seeded(7);
    let first = random.next_output();
    random = CrtRandom::seeded(parameter.wrapping_add(first));
    std::array::from_fn(|_| random.next_output() as u8)
}

/// The transport parameter the game derives from the launcher's challenge
/// reply: `(signed(reply[0]) << 8) + signed(reply[1])` over the first two
/// bytes of the reply's response string, as a 32-bit value.
pub fn transport_parameter(reply: &[u8]) -> u32 {
    let byte = |i: usize| i32::from(reply.get(i).copied().unwrap_or(0) as i8);
    ((byte(0) << 8) + byte(1)) as u32
}

#[cfg(test)]
mod tests;

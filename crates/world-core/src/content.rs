//! Bounded startup messages and per-world registration allocation.
//! Unresolved field semantics retain numeric labels. Static content is supplied
//! by the caller; no level definitions or captured replies are bundled.
use crate::bits::BitWriter;
use nfs_protocol::world::BitSpan;

pub mod bindings;

pub const MAX_BYTES: usize = 2048;
pub const MAX_ITEMS: usize = 1024;
pub const MAX_STRING: usize = 1023;
pub const MAX_BATCHES: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Truncated,
    Bound,
    Shape,
    Unsupported,
    Trailing,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadLevel {
    pub level: Vec<u8>,
    pub attributes: Vec<(Vec<u8>, Vec<u8>)>,
    pub word: u32,
    pub text: Vec<u8>,
    pub flags: [bool; 3],
    pub entries: Vec<(Vec<u8>, i32, i32)>,
    pub final_word: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubLevelNames {
    pub word: u32,
    pub entries: Vec<(u16, Vec<u8>)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Registration {
    /// Content-name table index; legacy wire field label.
    pub handle: u16,
    /// Assigned level ID, independent of the name index.
    pub next: u16,
    /// Parent level ID; zero selects the root.
    pub region: u16,
    /// Bundle ID, narrowed to u16 by the supported client.
    pub word: u32,
    pub byte: u8,
    pub flag: bool,
    pub enum3: u8,
    pub word2: u32,
    pub bit: bool,
    pub enum2: u8,
    pub word3: u32,
    pub text: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Registrations {
    pub header: u64,
    pub entries: Vec<Registration>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Message {
    LoadLevel(LoadLevel),
    Names(SubLevelNames),
    Registrations(Registrations),
}

/// All meaningful fields plus the physical tail, retained for reference checks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Decoded {
    pub message: Message,
    pub bits: usize,
    pub padding: u8,
}

struct Reader<'a> {
    span: BitSpan<'a>,
    pos: usize,
}
impl Reader<'_> {
    fn take(&mut self, n: u8) -> Result<u64, Error> {
        if n > 32 {
            return Ok((self.take(n - 32)? << 32) | self.take(32)?);
        }
        let v = self
            .span
            .read_u32(self.pos, n)
            .map_err(|_| Error::Truncated)?;
        self.pos += usize::from(n);
        Ok(u64::from(v))
    }
    fn count(&mut self, min_bits: usize) -> Result<usize, Error> {
        let n = self.take(32)? as usize;
        if n > MAX_ITEMS {
            return Err(Error::Bound);
        }
        if n > (self.span.len() - self.pos) / min_bits {
            return Err(Error::Truncated);
        }
        Ok(n)
    }
    fn string(&mut self) -> Result<Vec<u8>, Error> {
        let n = self.take(10)? as usize;
        if n > (self.span.len() - self.pos) / 8 {
            return Err(Error::Truncated);
        }
        (0..n).map(|_| self.take(8).map(|x| x as u8)).collect()
    }
}

impl Message {
    pub fn target(&self) -> u8 {
        match self {
            Self::LoadLevel(_) => 85,
            Self::Names(_) => 28,
            Self::Registrations(_) => 63,
        }
    }
    pub fn decode(target: u8, bytes: &[u8]) -> Result<Decoded, Error> {
        if bytes.len() > MAX_BYTES {
            return Err(Error::Bound);
        }
        let mut r = Reader {
            span: BitSpan::new(bytes, 0, bytes.len() * 8).map_err(|_| Error::Truncated)?,
            pos: 0,
        };
        let message = match target {
            85 => {
                let level = r.string()?;
                let mut attributes = Vec::new();
                for _ in 0..r.count(20)? {
                    attributes.push((r.string()?, r.string()?));
                }
                let word = r.take(32)? as u32;
                let text = r.string()?;
                let flags = [r.take(1)? != 0, r.take(1)? != 0, r.take(1)? != 0];
                let mut entries = Vec::new();
                for _ in 0..r.count(74)? {
                    entries.push((r.string()?, r.take(32)? as i32, r.take(32)? as i32));
                }
                Self::LoadLevel(LoadLevel {
                    level,
                    attributes,
                    word,
                    text,
                    flags,
                    entries,
                    final_word: r.take(32)? as u32,
                })
            }
            28 => {
                let word = r.take(32)? as u32;
                let mut entries = Vec::new();
                for _ in 0..r.count(26)? {
                    entries.push((r.take(16)? as u16, r.string()?));
                }
                Self::Names(SubLevelNames { word, entries })
            }
            63 => {
                let header = r.take(34)?;
                let mut entries = Vec::new();
                for _ in 0..r.count(169)? {
                    let rec = Registration {
                        handle: r.take(16)? as u16,
                        next: r.take(16)? as u16,
                        region: r.take(16)? as u16,
                        word: r.take(32)? as u32,
                        byte: r.take(8)? as u8,
                        flag: r.take(1)? != 0,
                        enum3: r.take(3)? as u8,
                        word2: r.take(32)? as u32,
                        bit: r.take(1)? != 0,
                        enum2: r.take(2)? as u8,
                        word3: r.take(32)? as u32,
                        text: r.string()?,
                    };
                    if rec.enum3 > 6 || rec.enum2 > 2 {
                        return Err(Error::Shape);
                    }
                    entries.push(rec);
                }
                Self::Registrations(Registrations { header, entries })
            }
            _ => return Err(Error::Unsupported),
        };
        let bits = r.pos;
        let tail = bytes.len() * 8 - bits;
        if tail > 7 {
            return Err(Error::Trailing);
        }
        let padding = if tail == 0 {
            0
        } else {
            r.take(tail as u8)? as u8
        };
        Ok(Decoded {
            message,
            bits,
            padding,
        })
    }

    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        self.encode_padding(0)
    }

    fn encode_padding(&self, padding: u8) -> Result<Vec<u8>, Error> {
        let mut w = Writer(BitWriter::new());
        match self {
            Self::LoadLevel(m) => {
                w.string(&m.level)?;
                w.count(m.attributes.len())?;
                for (k, v) in &m.attributes {
                    w.string(k)?;
                    w.string(v)?;
                }
                w.put(u64::from(m.word), 32)?;
                w.string(&m.text)?;
                for flag in m.flags {
                    w.put(u64::from(flag), 1)?;
                }
                w.count(m.entries.len())?;
                for (text, a, b) in &m.entries {
                    w.string(text)?;
                    w.put(u64::from(*a as u32), 32)?;
                    w.put(u64::from(*b as u32), 32)?;
                }
                w.put(u64::from(m.final_word), 32)?;
            }
            Self::Names(m) => {
                w.put(u64::from(m.word), 32)?;
                w.count(m.entries.len())?;
                for (id, name) in &m.entries {
                    w.put(u64::from(*id), 16)?;
                    w.string(name)?;
                }
            }
            Self::Registrations(m) => {
                w.put(m.header, 34)?;
                w.count(m.entries.len())?;
                for e in &m.entries {
                    if e.enum3 > 6 || e.enum2 > 2 {
                        return Err(Error::Shape);
                    }
                    for (value, width) in [
                        (u64::from(e.handle), 16),
                        (u64::from(e.next), 16),
                        (u64::from(e.region), 16),
                        (u64::from(e.word), 32),
                        (u64::from(e.byte), 8),
                        (u64::from(e.flag), 1),
                        (u64::from(e.enum3), 3),
                        (u64::from(e.word2), 32),
                        (u64::from(e.bit), 1),
                        (u64::from(e.enum2), 2),
                        (u64::from(e.word3), 32),
                    ] {
                        w.put(value, width)?;
                    }
                    w.string(&e.text)?;
                }
            }
        }
        let pad = (8 - w.0.len() % 8) % 8;
        w.put(u64::from(padding), pad)?;
        Ok(w.0.into_bytes())
    }
}
impl Decoded {
    pub fn reencode(&self) -> Result<Vec<u8>, Error> {
        self.message.encode_padding(self.padding)
    }
}

struct Writer(BitWriter);
impl Writer {
    fn put(&mut self, value: u64, width: usize) -> Result<(), Error> {
        if width > 63 || value >= (1u64 << width) {
            return Err(Error::Shape);
        }
        if self.0.len() + width > MAX_BYTES * 8 {
            return Err(Error::Bound);
        }
        self.0.put(value, width);
        Ok(())
    }
    fn count(&mut self, count: usize) -> Result<(), Error> {
        if count > MAX_ITEMS {
            return Err(Error::Bound);
        }
        self.put(count as u64, 32)
    }
    fn string(&mut self, bytes: &[u8]) -> Result<(), Error> {
        if bytes.len() > MAX_STRING {
            return Err(Error::Bound);
        }
        if self.0.len() + 10 + bytes.len() * 8 > MAX_BYTES * 8 {
            return Err(Error::Bound);
        }
        self.put(bytes.len() as u64, 10)?;
        self.0.put_bytes(bytes);
        Ok(())
    }
}

/// Version-one startup form: names, levels and bundles are assigned in sequence.
/// The `region` field is the parent level ID. This restricted importer
/// preserves the startup topology; it is not a general dynamic level allocator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistrationDefinition {
    pub region: u16,
    pub byte: u8,
    pub flag: bool,
    pub enum3: u8,
    pub word2: u32,
    pub bit: bool,
    pub enum2: u8,
    pub word3: u32,
    pub text: Vec<u8>,
}
impl RegistrationDefinition {
    /// The importer refuses forms outside the observed allocation relationships.
    pub fn from_record(r: &Registration) -> Result<Self, Error> {
        if r.handle.checked_add(1) != Some(r.next) || u32::from(r.handle) + 4 != r.word {
            return Err(Error::Unsupported);
        }
        Ok(Self {
            region: r.region,
            byte: r.byte,
            flag: r.flag,
            enum3: r.enum3,
            word2: r.word2,
            bit: r.bit,
            enum2: r.enum2,
            word3: r.word3,
            text: r.text.clone(),
        })
    }
}

/// Per-world allocator. Preparation is transactional: a rejected batch cannot
/// consume handles. A new world starts a separate counter at zero.
#[derive(Clone, Debug, Default)]
pub struct RegistrationsState {
    next: u32,
}
impl RegistrationsState {
    pub fn next(&self) -> u32 {
        self.next
    }
    /// Explicit seed only for replay comparisons of captures beginning midstream.
    pub fn with_next(next: u16) -> Self {
        Self {
            next: u32::from(next),
        }
    }
    pub fn prepare(
        &mut self,
        header: u64,
        definitions: &[RegistrationDefinition],
    ) -> Result<Message, Error> {
        if definitions.len() > MAX_ITEMS {
            return Err(Error::Bound);
        }
        let end = self
            .next
            .checked_add(definitions.len() as u32)
            .ok_or(Error::Bound)?;
        if end > u32::from(u16::MAX) {
            return Err(Error::Bound);
        }
        let entries = definitions
            .iter()
            .enumerate()
            .map(|(i, d)| {
                let handle = self.next + i as u32;
                Registration {
                    handle: handle as u16,
                    next: (handle + 1) as u16,
                    region: d.region,
                    word: handle + 4,
                    byte: d.byte,
                    flag: d.flag,
                    enum3: d.enum3,
                    word2: d.word2,
                    bit: d.bit,
                    enum2: d.enum2,
                    word3: d.word3,
                    text: d.text.clone(),
                }
            })
            .collect();
        let result = Message::Registrations(Registrations { header, entries });
        result.encode()?;
        self.next = end;
        Ok(result)
    }
}

#[cfg(test)]
mod tests;

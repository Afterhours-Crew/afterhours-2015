// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! File transfer record codecs and bounded assembly.
//! Labels identify application consumers; they are never filesystem paths.
use crate::bits::BitWriter;
use nfs_protocol::world::BitSpan;

pub mod sender;

pub const BLOCK_BYTES: usize = 128;
pub const MAX_BLOCKS: usize = 16;
pub const MAX_NAME: usize = 1023;
pub const NATIVE_MAX_BYTES: usize = 0x800000;
/// Local per-transfer resource limit.
pub const MAX_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Truncated,
    Shape,
    Bound,
    MissingStart,
    Conflict,
    Incomplete,
}

#[derive(Clone, Eq, PartialEq)]
pub enum Record {
    Start { name: Vec<u8>, bytes: usize },
    Blocks { first: usize, blocks: Vec<Vec<u8>> },
    Finish,
}

impl std::fmt::Debug for Record {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Start { name, bytes } => f
                .debug_struct("Start")
                .field("name_bytes", &name.len())
                .field("bytes", bytes)
                .finish(),
            Self::Blocks { first, blocks } => f
                .debug_struct("Blocks")
                .field("first", first)
                .field("count", &blocks.len())
                .finish(),
            Self::Finish => f.write_str("Finish"),
        }
    }
}

fn layout(size: Option<usize>) -> Result<(usize, u8), Error> {
    let size = size.ok_or(Error::MissingStart)?;
    if size == 0 || size > NATIVE_MAX_BYTES {
        return Err(Error::Bound);
    }
    let blocks = size.div_ceil(BLOCK_BYTES);
    Ok((size, (usize::BITS - (blocks - 1).leading_zeros()) as u8))
}

struct Reader<'a> {
    input: BitSpan<'a>,
    pos: usize,
}
impl Reader<'_> {
    fn take(&mut self, n: u8) -> Result<u32, Error> {
        if n == 0 {
            return Ok(0);
        }
        let value = self
            .input
            .read_u32(self.pos, n)
            .map_err(|_| Error::Truncated)?;
        self.pos += usize::from(n);
        Ok(value)
    }
    fn bytes(&mut self, n: usize) -> Result<Vec<u8>, Error> {
        (0..n).map(|_| self.take(8).map(|x| x as u8)).collect()
    }
}

/// Consume exactly one handler record without consuming the next handler.
pub fn decode(input: BitSpan<'_>, size: Option<usize>) -> Result<(Record, usize), Error> {
    let mut r = Reader { input, pos: 0 };
    let record = match r.take(2)? {
        1 => {
            let count = r.take(10)? as usize;
            let name = r.bytes(count)?;
            let bytes = r.take(24)? as usize;
            if bytes > NATIVE_MAX_BYTES {
                return Err(Error::Bound);
            }
            Record::Start { name, bytes }
        }
        2 => {
            let (size, width) = layout(size)?;
            let first = r.take(width)? as usize;
            let count = r.take(4)? as usize + 1;
            if first
                .checked_add(count)
                .is_none_or(|end| end > size.div_ceil(BLOCK_BYTES))
            {
                return Err(Error::Bound);
            }
            let blocks = (first..first + count)
                .map(|index| r.bytes(BLOCK_BYTES.min(size - index * BLOCK_BYTES)))
                .collect::<Result<_, _>>()?;
            Record::Blocks { first, blocks }
        }
        3 => Record::Finish,
        _ => return Err(Error::Shape),
    };
    Ok((record, r.pos))
}

impl Record {
    pub fn encode(&self, size: Option<usize>) -> Result<BitWriter, Error> {
        let mut w = BitWriter::new();
        match self {
            Self::Start { name, bytes } => {
                if name.len() > MAX_NAME || *bytes > NATIVE_MAX_BYTES {
                    return Err(Error::Bound);
                }
                w.put(1, 2)
                    .put(name.len() as u64, 10)
                    .put_bytes(name)
                    .put(*bytes as u64, 24);
            }
            Self::Blocks { first, blocks } => {
                let (size, width) = layout(size)?;
                if blocks.is_empty()
                    || blocks.len() > MAX_BLOCKS
                    || first
                        .checked_add(blocks.len())
                        .is_none_or(|end| end > size.div_ceil(BLOCK_BYTES))
                {
                    return Err(Error::Bound);
                }
                w.put(2, 2)
                    .put(*first as u64, usize::from(width))
                    .put(blocks.len() as u64 - 1, 4);
                for (i, block) in blocks.iter().enumerate() {
                    if block.len() != BLOCK_BYTES.min(size - (first + i) * BLOCK_BYTES) {
                        return Err(Error::Shape);
                    }
                    w.put_bytes(block);
                }
            }
            Self::Finish => {
                w.put(3, 2);
            }
        }
        Ok(w)
    }
}

#[derive(Clone)]
struct Active {
    name: Vec<u8>,
    bytes: Vec<u8>,
    present: Vec<bool>,
}

/// Completed application data stays owned until its consumer accepts it.
#[derive(Clone, Eq, PartialEq)]
pub struct Completed {
    pub name: Vec<u8>,
    pub bytes: Vec<u8>,
}
impl std::fmt::Debug for Completed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Completed")
            .field("name_bytes", &self.name.len())
            .field("bytes", &self.bytes.len())
            .finish()
    }
}

/// One incoming handler in one connection. The application history provides
/// frame delivery acknowledgements; these are not successful Items responses.
#[derive(Clone, Default)]
pub struct Receiver {
    active: Option<Active>,
    completed: Option<Completed>,
    finished: bool,
}
impl std::fmt::Debug for Receiver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Receiver")
            .field("size", &self.size())
            .field("completed", &self.completed)
            .finish()
    }
}
impl Receiver {
    pub fn size(&self) -> Option<usize> {
        self.active.as_ref().map(|s| s.bytes.len())
    }
    pub fn completed(&self) -> Option<&Completed> {
        self.completed.as_ref()
    }
    pub fn take_completed(&mut self) -> Option<Completed> {
        self.completed.take()
    }
    /// Apply atomically: failed or conflicting input leaves the transfer intact.
    pub fn receive(&mut self, record: &Record) -> Result<(), Error> {
        match record {
            Record::Start { name, bytes } => {
                if name.len() > MAX_NAME || *bytes > MAX_BYTES {
                    return Err(Error::Bound);
                }
                if let Some(old) = &self.active {
                    return if old.name == *name && old.bytes.len() == *bytes {
                        Ok(())
                    } else {
                        Err(Error::Conflict)
                    };
                }
                if self.completed.is_some() {
                    return Err(Error::Bound);
                }
                self.active = Some(Active {
                    name: name.clone(),
                    bytes: vec![0; *bytes],
                    present: vec![false; bytes.div_ceil(BLOCK_BYTES)],
                });
                self.finished = false;
            }
            Record::Blocks { first, blocks } => {
                let active = self.active.as_mut().ok_or(Error::MissingStart)?;
                if blocks.is_empty()
                    || blocks.len() > MAX_BLOCKS
                    || first
                        .checked_add(blocks.len())
                        .is_none_or(|end| end > active.present.len())
                {
                    return Err(Error::Bound);
                }
                for (i, block) in blocks.iter().enumerate() {
                    let n = first + i;
                    let start = n * BLOCK_BYTES;
                    let end = active.bytes.len().min(start + BLOCK_BYTES);
                    if block.len() != end - start {
                        return Err(Error::Shape);
                    }
                    if active.present[n] && active.bytes[start..end] != *block {
                        return Err(Error::Conflict);
                    }
                }
                for (i, block) in blocks.iter().enumerate() {
                    let n = first + i;
                    let start = n * BLOCK_BYTES;
                    active.bytes[start..start + block.len()].copy_from_slice(block);
                    active.present[n] = true;
                }
            }
            Record::Finish => {
                if self.finished {
                    return Ok(());
                }
                let active = self.active.as_ref().ok_or(Error::MissingStart)?;
                if active.present.iter().any(|p| !p) {
                    return Err(Error::Incomplete);
                }
                let active = self.active.take().ok_or(Error::MissingStart)?;
                self.completed = Some(Completed {
                    name: active.name,
                    bytes: active.bytes,
                });
                self.finished = true;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;

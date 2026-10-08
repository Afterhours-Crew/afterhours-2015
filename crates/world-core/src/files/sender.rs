//! Outgoing File records from owned bytes; no captured runtime templates.
use super::{BLOCK_BYTES, Completed, MAX_BLOCKS, MAX_BYTES, MAX_NAME, Record};

#[cfg(test)]
mod tests;

/// Local transfer policy. Eleven blocks match full record size, while
/// allowing a short final record. Timers are policy, not claimed native values.
#[derive(Clone, Copy, Debug)]
pub struct Policy {
    pub blocks_per_record: usize,
    pub retry_ms: u64,
    pub deadline_ms: u64,
    pub max_attempts: u8,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            blocks_per_record: 11,
            retry_ms: 1_000,
            deadline_ms: 60_000,
            max_attempts: 8,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Bound,
    Clock,
    Deadline,
    Attempts,
    State,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Start,
    Blocks(usize),
    Finish,
    Done,
}

/// The caller supplies fresh receipt identities from its connection. It must
/// cancel an old receipt when retrying and discard this sender on disconnect.
/// Exactly one record is awaiting application acceptance at a time.
pub struct Sender<T> {
    data: Option<Completed>,
    policy: Policy,
    phase: Phase,
    waiting: Option<(T, u64)>,
    attempts: u8,
    started: u64,
    now: u64,
    failed: Option<Error>,
}

impl<T: Copy + Eq> Sender<T> {
    pub fn new(data: Completed, policy: Policy, now: u64) -> Result<Self, Error> {
        if data.name.len() > MAX_NAME
            || data.bytes.len() > MAX_BYTES
            || !(1..=MAX_BLOCKS).contains(&policy.blocks_per_record)
            || policy.retry_ms == 0
            || policy.deadline_ms < policy.retry_ms
            || policy.max_attempts == 0
        {
            return Err(Error::Bound);
        }
        Ok(Self {
            data: Some(data),
            policy,
            phase: Phase::Start,
            waiting: None,
            attempts: 0,
            started: now,
            now,
            failed: None,
        })
    }
    fn clock(&mut self, now: u64) -> Result<(), Error> {
        if now < self.now {
            return Err(Error::Clock);
        }
        self.now = now;
        if let Some(error) = self.failed {
            return Err(error);
        }
        if self.phase != Phase::Done && now.saturating_sub(self.started) >= self.policy.deadline_ms
        {
            self.fail(Error::Deadline);
            return Err(Error::Deadline);
        }
        Ok(())
    }
    fn fail(&mut self, error: Error) {
        self.failed = Some(error);
        self.data = None;
        self.waiting = None;
    }
    pub fn done(&self) -> bool {
        self.phase == Phase::Done
    }
    pub fn waiting(&self) -> Option<T> {
        self.waiting.map(|(t, _)| t)
    }
    pub fn size(&self) -> Option<usize> {
        self.data.as_ref().map(|d| d.bytes.len())
    }
    fn record(&self) -> Option<Record> {
        let data = self.data.as_ref()?;
        Some(match self.phase {
            Phase::Start => Record::Start {
                name: data.name.clone(),
                bytes: data.bytes.len(),
            },
            Phase::Blocks(first) => Record::Blocks {
                first,
                blocks: data.bytes[first * BLOCK_BYTES..]
                    .chunks(BLOCK_BYTES)
                    .take(self.policy.blocks_per_record)
                    .map(<[u8]>::to_vec)
                    .collect(),
            },
            Phase::Finish => Record::Finish,
            Phase::Done => return None,
        })
    }
    /// Inspect the next record. Merely offering it does not consume a send or
    /// retry; socket/window backpressure can leave it pending unchanged.
    pub fn offer(&mut self, now: u64) -> Result<Option<Record>, Error> {
        self.clock(now)?;
        if self.done()
            || self
                .waiting
                .is_some_and(|(_, sent)| now - sent < self.policy.retry_ms)
        {
            return Ok(None);
        }
        if self.attempts >= self.policy.max_attempts {
            self.fail(Error::Attempts);
            return Err(Error::Attempts);
        }
        Ok(self.record())
    }
    /// Associate an actually sent record with its application receipt. A stale
    /// or different record cannot advance or overwrite this transfer.
    pub fn sent(&mut self, record: &Record, token: T, now: u64) -> Result<(), Error> {
        if self.offer(now)?.as_ref() != Some(record) || self.waiting() == Some(token) {
            return Err(Error::State);
        }
        self.waiting = Some((token, now));
        self.attempts += 1;
        Ok(())
    }
    /// Return false for an obsolete receipt, including a previous retry.
    pub fn acknowledge(&mut self, token: T, accepted: bool, now: u64) -> Result<bool, Error> {
        self.clock(now)?;
        if self.waiting() != Some(token) {
            return Ok(false);
        }
        self.waiting = None;
        if !accepted {
            return Ok(true);
        }
        let data = self.data.as_ref().ok_or(Error::State)?;
        self.phase = match self.phase {
            Phase::Start => {
                if data.bytes.is_empty() {
                    Phase::Finish
                } else {
                    Phase::Blocks(0)
                }
            }
            Phase::Blocks(first) => {
                let next = first + self.policy.blocks_per_record;
                if next >= data.bytes.len().div_ceil(BLOCK_BYTES) {
                    Phase::Finish
                } else {
                    Phase::Blocks(next)
                }
            }
            Phase::Finish => Phase::Done,
            Phase::Done => return Err(Error::State),
        };
        self.attempts = 0;
        if self.done() {
            self.data = None;
        }
        Ok(true)
    }
}

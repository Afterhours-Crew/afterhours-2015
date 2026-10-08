//! Application acceptance history.
//! This is separate from CommUDP reliability and RPC component serials.
use std::fmt;

/// Application sequence modulo 1024. Unlike an RPC serial, equal sequences do
/// not advance history, and the admissible forward window is only 32 positions.
/// Initial state is explicit; no total ordering or default is provided.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct Sequence(u16);

impl Sequence {
    pub const fn new(value: u16) -> Option<Self> {
        if value < 1024 {
            Some(Self(value))
        } else {
            None
        }
    }

    pub const fn value(self) -> u16 {
        self.0
    }

    fn advance_to(self, next: Self) -> Result<u8, AdvanceError> {
        let distance = next.0.wrapping_sub(self.0) & 1023;
        match distance {
            0 => Err(AdvanceError::Repeated),
            1..=32 => Ok(distance as u8),
            _ => Err(AdvanceError::OutsideWindow),
        }
    }
}

impl fmt::Debug for Sequence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sequence").finish_non_exhaustive()
    }
}

/// An ignored/rejected advance. No state changes when either case is returned.
/// OutsideWindow combines stale positions and forward jumps beyond 32; this
/// layer has no evidence with which to distinguish them or reconnect a peer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdvanceError {
    Repeated,
    OutsideWindow,
}

impl fmt::Display for AdvanceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "application history {self:?}")
    }
}
impl std::error::Error for AdvanceError {}

/// One connection's local receive frontier and acceptance history.
/// Bit 0 describes the frontier, bit i describes (frontier - i) modulo 1024.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ReceiveHistory {
    frontier: Sequence,
    bits: u32,
}

impl ReceiveHistory {
    pub const fn new(frontier: Sequence, bits: u32) -> Self {
        Self { frontier, bits }
    }

    pub const fn frontier(self) -> Sequence {
        self.frontier
    }

    pub const fn bits(self) -> u32 {
        self.bits
    }

    /// Check admission before handling payloads. This does not reserve a
    /// sequence: connection processing must serialize admission and commit.
    /// Returns the forward distance, in 1..=32.
    pub fn check(&self, incoming: Sequence) -> Result<u8, AdvanceError> {
        self.frontier.advance_to(incoming)
    }

    /// Commit a validated envelope's application outcome, returning the number
    /// of skipped positions (0..=31). Call only after integrity checks and payload
    /// admission. A received but unaccepted payload still advances the frontier.
    /// Skipped positions get zero; exactly 32 discards the previous history.
    pub fn commit(&mut self, incoming: Sequence, accepted: bool) -> Result<u8, AdvanceError> {
        let distance = self.check(incoming)?;
        self.bits = self.bits.checked_shl(u32::from(distance)).unwrap_or(0) | u32::from(accepted);
        self.frontier = incoming;
        Ok(distance - 1)
    }
}

impl fmt::Debug for ReceiveHistory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReceiveHistory").finish_non_exhaustive()
    }
}

/// One newly reported sequence and its peer application acceptance bit.
/// False is nonacceptance, not a diagnosis of physical packet loss.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct Acceptance {
    sequence: Sequence,
    accepted: bool,
}

impl Acceptance {
    pub const fn sequence(self) -> Sequence {
        self.sequence
    }

    pub const fn accepted(self) -> bool {
        self.accepted
    }
}

impl fmt::Debug for Acceptance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Acceptance")
            .field("accepted", &self.accepted)
            .finish_non_exhaustive()
    }
}

/// Allocation-free, immutable result batch, in oldest-to-newest order.
/// The caller must associate results with its own bounded in-flight tokens;
/// this batch neither validates sent sequences nor resends or frees anything.
#[must_use = "process or retain the statuses before discarding this batch"]
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct Acknowledgements {
    frontier: Sequence,
    bits: u32,
    count: u8,
}

impl Acknowledgements {
    pub const fn len(self) -> usize {
        self.count as usize
    }

    pub const fn is_empty(self) -> bool {
        self.count == 0
    }

    pub fn iter(self) -> impl ExactSizeIterator<Item = Acceptance> + DoubleEndedIterator {
        (0..self.count).rev().map(move |age| Acceptance {
            sequence: Sequence(self.frontier.0.wrapping_sub(u16::from(age)) & 1023),
            accepted: (self.bits >> age) & 1 != 0,
        })
    }
}

impl fmt::Debug for Acknowledgements {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Acknowledgements")
            .field("count", &self.count)
            .finish_non_exhaustive()
    }
}

/// One connection's last consumed peer-report frontier. Independent of local
/// receive history; instantiate separately for every connection/lifetime.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct PeerReports {
    frontier: Sequence,
}

impl PeerReports {
    pub const fn new(frontier: Sequence) -> Self {
        Self { frontier }
    }

    pub const fn frontier(self) -> Sequence {
        self.frontier
    }

    /// Consume a validated report and advance exactly once, returning at most
    /// 32 statuses. Retain/process the batch before the next report if downstream
    /// handling has backpressure. Repeated/stale/out-of-window reports leave the
    /// frontier unchanged. Envelope integrity and sent-token bounds are external.
    pub fn consume(
        &mut self,
        acknowledgement: Sequence,
        history: u32,
    ) -> Result<Acknowledgements, AdvanceError> {
        let count = self.frontier.advance_to(acknowledgement)?;
        self.frontier = acknowledgement;
        Ok(Acknowledgements {
            frontier: acknowledgement,
            bits: history,
            count,
        })
    }
}

impl fmt::Debug for PeerReports {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PeerReports").finish_non_exhaustive()
    }
}

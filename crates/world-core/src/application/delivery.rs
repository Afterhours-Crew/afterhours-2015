// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Bounded ownership of application-frame acceptance receipts.
use super::Error;
use nfs_protocol::world::history::Acknowledgements;
use std::collections::BTreeMap;

/// Local monotonically increasing identity, never a modulo wire sequence.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Ticket(u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Status {
    Pending,
    Accepted,
    Rejected,
}

#[derive(Default)]
pub(super) struct Deliveries {
    next: u64,
    frames: BTreeMap<Ticket, Vec<(u16, Option<bool>)>>,
}

impl Deliveries {
    pub fn available(&self) -> Result<(), Error> {
        if self.frames.len() >= 32 || self.next == u64::MAX {
            return Err(Error::Bound);
        }
        Ok(())
    }
    /// Called only after an atomic successful send has reserved its sequences.
    pub fn insert(&mut self, first: u16, count: usize) -> Ticket {
        self.next += 1;
        let ticket = Ticket(self.next);
        self.frames.insert(
            ticket,
            (0..count)
                .map(|i| ((first + i as u16) & 1023, None))
                .collect(),
        );
        ticket
    }
    pub fn apply(&mut self, batch: Acknowledgements) {
        for acceptance in batch.iter() {
            for sequences in self.frames.values_mut() {
                for (sequence, result) in sequences {
                    if *sequence == acceptance.sequence().value() && result.is_none() {
                        *result = Some(acceptance.accepted());
                    }
                }
            }
        }
    }
    /// A wire sequence reused after wrap cannot resolve an old pending receipt.
    pub fn sent(&mut self, first: u16, count: usize) {
        for sequences in self.frames.values_mut() {
            for (sequence, result) in sequences {
                if result.is_none() && usize::from(sequence.wrapping_sub(first) & 1023) < count {
                    *result = Some(false);
                }
            }
        }
    }
    pub fn status(&self, ticket: Ticket) -> Option<Status> {
        let sequences = self.frames.get(&ticket)?;
        Some(if sequences.iter().any(|(_, r)| *r == Some(false)) {
            Status::Rejected
        } else if sequences.iter().all(|(_, r)| *r == Some(true)) {
            Status::Accepted
        } else {
            Status::Pending
        })
    }
    pub fn remove(&mut self, ticket: Ticket) -> Option<Status> {
        let status = self.status(ticket)?;
        self.frames.remove(&ticket);
        Some(status)
    }
}

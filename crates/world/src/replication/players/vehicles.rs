// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use super::*;

impl Players {
    pub fn owned_identity(
        &self,
        connection: u8,
        persona: u64,
        participant: u16,
    ) -> Option<NamedIdentity> {
        if !self.owns_participant(connection, persona, participant) {
            return None;
        }
        match &self.objects.get(participant)?.initial {
            Initial::Participant(value) => Some(value.identity.clone()),
            _ => None,
        }
    }

    pub fn owned_vehicle(
        &self,
        connection: u8,
        persona: u64,
        participant: u16,
        item: u64,
    ) -> Option<u16> {
        self.owns_participant(connection, persona, participant)
            .then(|| self.vehicles.get(&(participant, item)).copied())
            .flatten()
    }

    pub fn create_vehicle(
        &mut self,
        connection: u8,
        persona: u64,
        participant: u16,
        item: u64,
        creation: vehicle::EntityCreation,
        content: &entity::creation::Content,
    ) -> Result<Record, Error> {
        if self.owned_actor(connection, persona, participant).is_none() {
            return Err(Error::UnknownObject);
        }
        if item == 0 {
            return Err(Error::Shape);
        }
        if self.vehicles.contains_key(&(participant, item)) {
            return Err(Error::DuplicateObject);
        }
        let record = self.objects.spawn_vehicle(creation, content)?;
        self.vehicles.insert((participant, item), record.id);
        Ok(record)
    }
}

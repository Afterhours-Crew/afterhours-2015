// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use super::*;
use crate::vehicle_content::population::Owner;
use nfs_protocol::world::{
    BitSpan,
    rpc::{Envelope, Limits, RouteProfile},
};
use nfs_world::{
    content::Message,
    garage::vehicle::Loaded,
    participants::{HostRpc, Lifecycle},
};

#[derive(Default)]
pub(super) struct Output {
    pub messages: Vec<Message>,
    pub sections: Vec<Section>,
    pub replies: Vec<HostRpc>,
}
impl PlayerListener {
    pub(super) fn garage_presence(
        roles: Option<&crate::scene_roles::SceneRoles>,
        players: &Players,
        replies: &[HostRpc],
        presence: &mut nfs_world::garage::presence::Presence,
        persona: u64,
    ) -> Result<Vec<HostRpc>, replication::Error> {
        use nfs_world::{
            garage::presence::COMPONENT,
            participants::Endpoint,
            replication::{Initial, sublevel},
        };
        let mut out = Vec::new();
        for reply in replies {
            let HostRpc::Vehicle(vehicle) = reply else {
                continue;
            };
            let scene = players
                .objects()
                .scene(roles.ok_or(replication::Error::Unsupported)?.garage)
                .ok_or(replication::Error::UnknownObject)?;
            let Some(Initial::SubLevel { fields, .. }) =
                players.objects().get(scene).map(|o| &o.initial)
            else {
                return Err(replication::Error::TypeMismatch);
            };
            let Some(sublevel::Initial::RpcBool { rpc, value: false }) = fields.get(COMPONENT)
            else {
                return Err(replication::Error::TypeMismatch);
            };
            let endpoint = Endpoint {
                scene,
                selector: rpc.selector,
                serial: rpc.serial,
            };
            if let Some(notification) = presence.set(endpoint, vehicle.participant, true, |id| {
                players.owns_participant(HOST_SELECTOR as u8, persona, id)
            })? {
                out.push(HostRpc::GaragePresence(notification));
            }
        }
        Ok(out)
    }
    pub(super) fn populate(
        players: &mut Players,
        participants: &Lifecycle,
        population: Option<&mut Population>,
        inventory: Option<&Inventory>,
        content: Option<&GarageContent>,
        persona: u64,
    ) -> Result<Output, replication::Error> {
        let (Some(population), Some(inventory), Some(content)) = (population, inventory, content)
        else {
            return Ok(Output::default());
        };
        let mut output = Output::default();
        for participant in participants.waiting_garage() {
            let spawned = population.spawn(
                players,
                Owner {
                    connection: HOST_SELECTOR as u8,
                    persona,
                    participant,
                },
                inventory.clone(),
                &content.vehicles,
                &content.layout,
                [0.; 3],
            )?;
            let mut sections = if spawned.records.is_empty() {
                Vec::new()
            } else {
                split_scenes(
                    spawned.records,
                    nfs_world::application::OUTBOUND_FRAME_BITS - 100,
                )?
            };
            for section in &mut sections {
                section.setup = Some(Setup::RawEscape([0; 3]));
            }
            output.sections.extend(sections);
            output.messages.extend(spawned.messages);
            output
                .replies
                .extend(spawned.bindings.into_iter().map(HostRpc::Vehicle));
        }
        Ok(output)
    }
    pub(super) fn vehicle_loaded(
        population: &mut Population,
        players: &Players,
        persona: u64,
        body: BitSpan<'_>,
    ) -> Result<bool, replication::Error> {
        let envelope = Envelope::decode(
            body,
            Limits {
                max_input_bits: 4096,
                max_references: 32,
                max_payload_bytes: 256,
            },
        )
        .map_err(|_| replication::Error::Shape)?;
        let route = envelope
            .route(RouteProfile::ClientSend)
            .map_err(|_| replication::Error::Shape)?;
        let Some(&scene) = envelope.references().first() else {
            return Ok(false);
        };
        if !population.has_endpoint(scene, route.selector()) || route.method_index() != 1 {
            return Ok(false);
        }
        let loaded = Loaded::decode(body)?;
        population.acknowledge(
            players,
            Owner {
                connection: HOST_SELECTOR as u8,
                persona,
                participant: loaded.participant,
            },
            loaded,
        )?;
        Ok(true)
    }
}

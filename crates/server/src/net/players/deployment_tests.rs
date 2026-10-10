// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
use super::*;
use crate::{
    content::{Batch, WorldContent},
    scene_roles::SceneRoles,
};
use nfs_world::{
    content::{LoadLevel, RegistrationDefinition, SubLevelNames, bindings::name_key},
    replication::{
        entity::creation::Asset,
        sublevel::{Content, Kind, Profile, root},
    },
};

fn content() -> WorldContent {
    let names: Vec<_> = (0..14)
        .map(|i| format!("constructed/scene-{i}").into_bytes())
        .collect();
    let keys: Vec<_> = names.iter().map(|name| name_key(name).unwrap()).collect();
    let roles = SceneRoles {
        level: b"constructed/level".to_vec(),
        gameplay: keys[10],
        startup: keys[11],
        garage: keys[12],
        progression: keys[13],
        traffic: keys[..10].try_into().unwrap(),
        customization_timer: Asset {
            bundle: 1,
            type_id: 2,
            local_index: 0,
        },
        streaming_gate: Asset {
            bundle: 2,
            type_id: 3,
            local_index: 0,
        },
    };
    let mut profiles: Vec<_> = root::content()
        .unwrap()
        .profiles()
        .map(|(key, p)| (key, p.clone()))
        .collect();
    for (i, key) in keys.iter().copied().enumerate() {
        let mut kinds = vec![Kind::Noop];
        if i == 10 {
            kinds.resize(13, Kind::Noop);
            kinds[1] = Kind::RpcReferences;
            kinds[8..13].fill(Kind::Rpc);
        } else if i == 11 {
            kinds.resize(115, Kind::Noop);
            for index in [6, 11, 7, 8, 25, 21, 18, 19, 114, 95, 90, 89, 86, 87] {
                kinds[index] = Kind::RpcReferences;
            }
        }
        profiles.push((key, Profile::new(false, &kinds).unwrap()));
    }
    let profiles = Content::new(profiles).unwrap();
    let launchers = Some(nfs_world::launchers::Catalog::new(vec![], &profiles).unwrap());
    WorldContent {
        level: LoadLevel {
            level: roles.level.clone(),
            attributes: vec![],
            word: 0,
            text: vec![],
            flags: [false; 3],
            entries: vec![],
            final_word: 0,
        },
        roles: Some(roles),
        scene_profiles: Some(profiles),
        launchers,
        batches: vec![
            Batch::Names(SubLevelNames {
                word: 0,
                entries: names
                    .into_iter()
                    .enumerate()
                    .map(|(i, name)| (i as u16, name))
                    .collect(),
            }),
            Batch::Registrations {
                header: 0,
                definitions: (0..14)
                    .map(|_| RegistrationDefinition {
                        region: 0,
                        byte: 0,
                        flag: false,
                        enum3: 0,
                        word2: 0,
                        bit: false,
                        enum2: 0,
                        word3: 0,
                        text: vec![],
                    })
                    .collect(),
            },
        ],
    }
}

#[test]
fn explicit_scene_roles_bind_atomically_and_reject_incomplete_or_changed_content() {
    let content = content();
    content.validate_runtime().unwrap();
    assert_eq!(
        WorldContent::from_json(&content.to_json()).unwrap(),
        content
    );
    let mut listener = PlayerListener::new(101);
    let mut missing = content.clone();
    missing.roles = None;
    assert!(missing.validate_runtime().is_err());
    assert!(listener.initialize_world(&missing).is_err());
    assert!(listener.players.objects().is_empty());
    assert!(listener.roles.is_none());
    let mut invalid = content.clone();
    invalid.roles.as_mut().unwrap().gameplay = 12345;
    assert!(invalid.validate_runtime().is_err());
    assert!(listener.initialize_world(&invalid).is_err());
    assert!(listener.players.objects().is_empty());
    assert!(listener.pending.is_empty());
    listener.initialize_world(&content).unwrap();
    assert_eq!(listener.players.objects().len(), 15);
    assert_eq!(listener.roles, content.roles);
    let before = listener.players.objects().snapshot();
    let queued = listener.pending.len();
    assert!(listener.initialize_world(&content).is_err());
    assert_eq!(listener.players.objects().snapshot(), before);
    assert_eq!(listener.pending.len(), queued);
    let mut other = PlayerListener::new(202);
    other.initialize_world(&content).unwrap();
    assert_eq!(other.players.objects().snapshot(), before);
}

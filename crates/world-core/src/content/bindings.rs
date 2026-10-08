//! Atomic startup identity graphs with distinct name, level, parent and bundle IDs.
use super::{Error, MAX_BATCHES, MAX_ITEMS, MAX_STRING, Message};
use std::collections::{BTreeMap, BTreeSet};

/// Native ASCII case-insensitive DJB2-XOR content name key. Non-ASCII content
/// remains unsupported rather than assuming the client's character semantics.
pub fn name_key(name: &[u8]) -> Result<u32, Error> {
    if name.len() > MAX_STRING {
        return Err(Error::Bound);
    }
    if !name.is_ascii() || name.contains(&0) {
        return Err(Error::Unsupported);
    }
    Ok(if name.is_empty() {
        0
    } else {
        name.iter().fold(5381u32, |key, byte| {
            key.wrapping_mul(33) ^ u32::from(byte.to_ascii_lowercase())
        })
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Level {
    pub id: u16,
    pub parent: u16,
    pub content_key: u32,
    pub bundle_id: u16,
}

/// Validated identity graph for one complete startup. Construction is atomic;
/// callers cannot observe a partial name table or a cyclic/missing parent.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Bindings {
    levels: BTreeMap<u16, Level>,
}
impl Bindings {
    pub fn from_messages(messages: &[Message]) -> Result<Self, Error> {
        if messages.len() > MAX_BATCHES {
            return Err(Error::Bound);
        }
        let mut names = BTreeMap::new();
        let mut levels = BTreeMap::new();
        let mut bundles = BTreeSet::new();
        for message in messages {
            message.encode()?;
            match message {
                Message::Names(batch) => {
                    for (id, name) in &batch.entries {
                        if names.insert(*id, name_key(name)?).is_some() {
                            return Err(Error::Shape);
                        }
                    }
                }
                Message::Registrations(batch) => {
                    for entry in &batch.entries {
                        let level = Level {
                            id: entry.next,
                            parent: entry.region,
                            content_key: *names.get(&entry.handle).ok_or(Error::Shape)?,
                            bundle_id: u16::try_from(entry.word).map_err(|_| Error::Bound)?,
                        };
                        // 0 is the root; ffff is the explicit null level handle.
                        if [0, u16::MAX].contains(&level.id)
                            || level.bundle_id == u16::MAX
                            || !bundles.insert(level.bundle_id)
                            || levels.insert(level.id, level).is_some()
                        {
                            return Err(Error::Shape);
                        }
                    }
                }
                Message::LoadLevel(_) => {}
            }
            if names.len() > MAX_ITEMS || levels.len() > MAX_ITEMS {
                return Err(Error::Bound);
            }
        }
        for level in levels.values() {
            let mut current = level.id;
            let mut visited = BTreeSet::new();
            while current != 0 {
                if !visited.insert(current) {
                    return Err(Error::Shape);
                }
                current = levels.get(&current).ok_or(Error::Shape)?.parent;
            }
        }
        Ok(Self { levels })
    }

    pub fn level(&self, id: u16) -> Option<Level> {
        self.levels.get(&id).copied()
    }

    /// Stable current identities, parent before child. IDs need not encode
    /// ancestry. Construction already rejects missing parents and cycles.
    pub fn hierarchy(&self) -> Vec<Level> {
        let mut emitted = BTreeSet::from([0]);
        let mut result = Vec::with_capacity(self.levels.len());
        while result.len() < self.levels.len() {
            for level in self.levels.values() {
                if !emitted.contains(&level.id) && emitted.contains(&level.parent) {
                    emitted.insert(level.id);
                    result.push(*level);
                }
            }
        }
        result
    }

    /// Resolve a unique root child by static content identity. Multiple instances
    /// of the same content are legal in the graph but require a distinct role.
    pub fn root_child(&self, content_key: u32) -> Result<Level, Error> {
        let mut matches = self
            .levels
            .values()
            .filter(|v| v.parent == 0 && v.content_key == content_key);
        let result = *matches.next().ok_or(Error::Shape)?;
        if matches.next().is_some() {
            return Err(Error::Shape);
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::{Registration, Registrations, SubLevelNames};

    fn registration(name: u16, level: u16, parent: u16, bundle: u32) -> Registration {
        Registration {
            handle: name,
            next: level,
            region: parent,
            word: bundle,
            byte: 0,
            flag: false,
            enum3: 0,
            word2: 0,
            bit: false,
            enum2: 0,
            word3: 0,
            text: vec![],
        }
    }
    fn messages(entries: Vec<Registration>) -> Vec<Message> {
        vec![
            Message::Names(SubLevelNames {
                word: 0,
                entries: vec![(9, b"First".to_vec()), (3, b"Second".to_vec())],
            }),
            Message::Registrations(Registrations { header: 0, entries }),
        ]
    }
    #[test]
    fn names_levels_parents_and_bundles_have_separate_identities() {
        let source = messages(vec![
            registration(3, 29, 17, 93),
            registration(9, 17, 0, 81),
        ]);
        let graph = Bindings::from_messages(&source).unwrap();
        assert_eq!(
            graph.level(29),
            Some(Level {
                id: 29,
                parent: 17,
                bundle_id: 93,
                content_key: name_key(b"Second").unwrap(),
            })
        );
        assert_eq!(
            graph.root_child(name_key(b"First").unwrap()).unwrap().id,
            17
        );
        assert!(graph.root_child(name_key(b"Second").unwrap()).is_err());
        assert_eq!(Bindings::from_messages(&source).unwrap(), graph);
        assert!(Bindings::default().level(17).is_none());
    }
    #[test]
    fn invalid_or_ambiguous_identity_graphs_are_rejected() {
        for entries in [
            vec![registration(7, 17, 0, 81)],
            vec![registration(9, 17, 8, 81)],
            vec![registration(9, 17, 29, 81), registration(3, 29, 17, 93)],
            vec![registration(9, 17, 0, 81), registration(3, 17, 0, 93)],
            vec![registration(9, 17, 0, 81), registration(3, 29, 0, 81)],
            vec![registration(9, u16::MAX, 0, 81)],
            vec![registration(9, 17, 0, 65536)],
        ] {
            assert!(Bindings::from_messages(&messages(entries)).is_err());
        }
        let source = messages(vec![registration(9, 17, 0, 81), registration(9, 29, 0, 93)]);
        let graph = Bindings::from_messages(&source).unwrap();
        assert!(graph.root_child(name_key(b"First").unwrap()).is_err());
    }
    #[test]
    fn hierarchy_orders_parents_before_children_even_with_reverse_ids() {
        let graph = Bindings::from_messages(&messages(vec![
            registration(9, 7, 99, 81),
            registration(3, 99, 0, 93),
        ]))
        .unwrap();
        assert_eq!(
            graph.hierarchy().iter().map(|v| v.id).collect::<Vec<_>>(),
            [99, 7]
        );
        assert!(Bindings::default().hierarchy().is_empty());
    }
    #[test]
    fn name_hash_is_case_insensitive_and_checks_input_bounds() {
        assert_eq!(name_key(b"abc"), Ok(193409669));
        assert_eq!(name_key(b"ABC"), Ok(193409669));
        assert_eq!(name_key(b""), Ok(0));
        assert_eq!(name_key(b"name\0suffix"), Err(Error::Unsupported));
        assert_eq!(name_key(&[255]), Err(Error::Unsupported));
        assert_eq!(name_key(&vec![b'a'; MAX_STRING + 1]), Err(Error::Bound));
    }
}

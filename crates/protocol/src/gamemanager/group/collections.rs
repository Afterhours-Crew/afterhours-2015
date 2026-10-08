// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use super::*;
use crate::gamemanager::MatchmakingSetupContext;
use crate::users::IpPairAddress;
use nfs_heat2::Value;

macro_rules! integer_list {
    ($name:ident, $scalar:ty) => {
        #[derive(Default)]
        pub struct $name(pub Vec<$scalar>);
        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_tuple(stringify!($name))
                    .field(&self.0.len())
                    .finish()
            }
        }
        impl<'a> Wire<'a> for $name {
            const KIND: Kind = Kind::List;
            fn read(item: Item<'a>) -> Result<Self, Error> {
                check_list(item, Kind::Integer)?;
                Ok(Self(
                    item.elements()
                        .ok_or(Error::WrongType { tag: None })?
                        .map(|i| <$scalar>::read(i?))
                        .collect::<Result<_, _>>()?,
                ))
            }
            fn write(&self, tag: [u8; 3], w: &mut Encoder) -> Result<(), nfs_heat2::Error> {
                w.integer_list(tag, self.0.iter().map(|v| i64::from(*v)))
            }
            fn validate(&self, budget: &mut crate::Budget) -> Result<(), Error> {
                budget.take(1)?;
                budget.collection(self.0.len(), 1)
            }
        }
    };
}
integer_list!(PlayerIds, i64);
integer_list!(CapacityList, u16);

/// Native uint64 list preserves all identity bits through the signed wire integer.
#[derive(Default)]
pub struct GameIds(pub Vec<u64>);
impl fmt::Debug for GameIds {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("GameIdsLength").field(&self.0.len()).finish()
    }
}
impl<'a> Wire<'a> for GameIds {
    const KIND: Kind = Kind::List;
    fn read(item: Item<'a>) -> Result<Self, Error> {
        check_list(item, Kind::Integer)?;
        Ok(Self(
            item.elements()
                .ok_or(Error::WrongType { tag: None })?
                .map(|i| u64::read(i?))
                .collect::<Result<_, _>>()?,
        ))
    }
    fn write(&self, tag: [u8; 3], w: &mut Encoder) -> Result<(), nfs_heat2::Error> {
        w.integer_list(
            tag,
            self.0.iter().map(|v| i64::from_ne_bytes(v.to_ne_bytes())),
        )
    }
    fn validate(&self, budget: &mut crate::Budget) -> Result<(), Error> {
        budget.take(1)?;
        budget.collection(self.0.len(), 1)
    }
}

fn check_list(item: Item<'_>, kind: Kind) -> Result<(), Error> {
    match item.value() {
        Value::List { element, .. } if element == kind => Ok(()),
        _ => Err(Error::WrongType { tag: None }),
    }
}
macro_rules! struct_list {
    ($name:ident, $element:ident) => {
        #[derive(Default)]
        pub struct $name<'a>(pub Vec<$element<'a>>);
        impl fmt::Debug for $name<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_tuple(stringify!($name))
                    .field(&self.0.len())
                    .finish()
            }
        }
        impl<'a> Wire<'a> for $name<'a> {
            const KIND: Kind = Kind::List;
            fn read(item: Item<'a>) -> Result<Self, Error> {
                check_list(item, Kind::Struct)?;
                Ok(Self(
                    item.elements()
                        .ok_or(Error::WrongType { tag: None })?
                        .map(|i| $element::read(i?))
                        .collect::<Result<_, _>>()?,
                ))
            }
            fn write(&self, tag: [u8; 3], w: &mut Encoder) -> Result<(), nfs_heat2::Error> {
                w.struct_list(tag, &self.0, |w, value| value.write_fields(w))
            }
            fn validate(&self, budget: &mut crate::Budget) -> Result<(), Error> {
                budget.take(1)?;
                budget.collection(self.0.len(), 0)?;
                for value in &self.0 {
                    value.validate(budget)?;
                }
                Ok(())
            }
            fn unknown_count(&self) -> usize {
                self.0.iter().map(Wire::unknown_count).sum()
            }
        }
    };
}
struct_list!(PlayerJoinList, PerPlayerJoinData);
struct_list!(UserIdentificationList, UserIdentification);
struct_list!(PlayerRoster, ReplicatedGamePlayer);

#[derive(Default)]
pub struct RoleCriteriaMap<'a>(pub Vec<(&'a [u8], RoleCriteria<'a>)>);
impl fmt::Debug for RoleCriteriaMap<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("RoleCriteriaMapLength")
            .field(&self.0.len())
            .finish()
    }
}
impl<'a> Wire<'a> for RoleCriteriaMap<'a> {
    const KIND: Kind = Kind::Map;
    fn read(item: Item<'a>) -> Result<Self, Error> {
        if !matches!(
            item.value(),
            Value::Map {
                key: Kind::String,
                value: Kind::Struct,
                ..
            }
        ) {
            return Err(Error::WrongType { tag: None });
        }
        let mut elements = item.elements().ok_or(Error::WrongType { tag: None })?;
        let mut entries = Vec::new();
        while let Some(key) = elements.next() {
            entries.push((
                <&[u8]>::read(key?)?,
                RoleCriteria::read(elements.next().ok_or(Error::WrongType { tag: None })??)?,
            ));
        }
        crate::unique_keys(entries.iter().map(|(key, _)| *key))?;
        Ok(Self(entries))
    }
    fn write(&self, tag: [u8; 3], w: &mut Encoder) -> Result<(), nfs_heat2::Error> {
        w.string_struct_map(tag, &self.0, |w, value| value.write_fields(w))
    }
    fn validate(&self, budget: &mut crate::Budget) -> Result<(), Error> {
        budget.take(1)?;
        budget.collection(self.0.len(), 1)?;
        crate::unique_keys(self.0.iter().map(|(key, _)| *key))?;
        for (key, value) in &self.0 {
            crate::check_string(key, budget.limits)?;
            value.validate(budget)?;
        }
        Ok(())
    }
    fn unknown_count(&self) -> usize {
        self.0.iter().map(|(_, v)| v.unknown_field_count()).sum()
    }
}

#[derive(Default)]
pub struct NetworkAddressList<'a>(pub Vec<NetworkAddress<'a>>);
impl fmt::Debug for NetworkAddressList<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("NetworkAddressListLength")
            .field(&self.0.len())
            .finish()
    }
}
impl<'a> Wire<'a> for NetworkAddressList<'a> {
    const KIND: Kind = Kind::List;
    fn read(item: Item<'a>) -> Result<Self, Error> {
        check_list(item, Kind::Union)?;
        let mut values = Vec::new();
        for value in item.elements().ok_or(Error::WrongType { tag: None })? {
            let value = value?;
            values.push(match value.value() {
                Value::Union { discriminant: 127 } => NetworkAddress::Unset,
                Value::Union { discriminant: 2 } => NetworkAddress::IpPair(IpPairAddress::read(
                    value
                        .union_member()
                        .ok_or(Error::WrongType { tag: None })??,
                )?),
                _ => return Err(Error::WrongType { tag: None }),
            });
        }
        Ok(Self(values))
    }
    fn write(&self, tag: [u8; 3], w: &mut Encoder) -> Result<(), nfs_heat2::Error> {
        w.struct_union_list(
            tag,
            &self.0,
            |v| match v {
                NetworkAddress::Unset => None,
                NetworkAddress::IpPair(_) => Some(2),
            },
            |w, v| match v {
                NetworkAddress::Unset => Ok(()),
                NetworkAddress::IpPair(pair) => pair.write_fields(w),
            },
        )
    }
    fn validate(&self, budget: &mut crate::Budget) -> Result<(), Error> {
        budget.take(1)?;
        budget.collection(self.0.len(), 0)?;
        for value in &self.0 {
            value.validate(budget)?;
        }
        Ok(())
    }
    fn unknown_count(&self) -> usize {
        self.0.iter().map(Wire::unknown_count).sum()
    }
}

/// Create is dataless; matchmade uses selector 3.
/// Other native alternatives remain unsupported, never interpreted as dataless.
pub enum GameSetupReason<'a> {
    Unset,
    Dataless(DatalessSetupContext<'a>),
    Matchmaking(MatchmakingSetupContext<'a>),
}
impl fmt::Debug for GameSetupReason<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Unset => "GameSetupReason::Unset",
            Self::Dataless(_) => "GameSetupReason::Dataless(<redacted>)",
            Self::Matchmaking(_) => "GameSetupReason::Matchmaking(<redacted>)",
        })
    }
}
impl<'a> Wire<'a> for GameSetupReason<'a> {
    const KIND: Kind = Kind::Union;
    fn read(item: Item<'a>) -> Result<Self, Error> {
        match item.value() {
            Value::Union { discriminant: 127 } => Ok(Self::Unset),
            Value::Union { discriminant: 0 }
                if item.as_bytes().get(1..4) == Some(&[0x92, 0xcc, 0xe3]) =>
            {
                Ok(Self::Dataless(DatalessSetupContext::read(
                    item.union_member()
                        .ok_or(Error::WrongType { tag: None })??,
                )?))
            }
            Value::Union { discriminant: 3 }
                if item.as_bytes().get(1..4) == Some(&[0xb6, 0xdc, 0xe3]) =>
            {
                Ok(Self::Matchmaking(MatchmakingSetupContext::read(
                    item.union_member()
                        .ok_or(Error::WrongType { tag: None })??,
                )?))
            }
            _ => Err(Error::WrongType { tag: None }),
        }
    }
    fn write(&self, tag: [u8; 3], w: &mut Encoder) -> Result<(), nfs_heat2::Error> {
        match self {
            Self::Unset => w.unset_union(tag),
            Self::Dataless(context) => {
                w.struct_union(tag, 0, [0x92, 0xcc, 0xe3], |w| context.write_fields(w))
            }
            Self::Matchmaking(context) => {
                w.struct_union(tag, 3, [0xb6, 0xdc, 0xe3], |w| context.write_fields(w))
            }
        }
    }
    fn validate(&self, budget: &mut crate::Budget) -> Result<(), Error> {
        budget.take(1)?;
        match self {
            Self::Dataless(value) => value.validate(budget)?,
            Self::Matchmaking(value) => value.validate(budget)?,
            Self::Unset => {}
        }
        Ok(())
    }
    fn unknown_count(&self) -> usize {
        match self {
            Self::Unset => 0,
            Self::Dataless(v) => v.unknown_field_count(),
            Self::Matchmaking(v) => v.unknown_field_count(),
        }
    }
}

/// Raw binary32 bits, so NaNs and signed zero round-trip without arithmetic.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct PacketLossBits(pub u32);
impl fmt::Debug for PacketLossBits {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PacketLossBits(<redacted>)")
    }
}
impl<'a> Wire<'a> for PacketLossBits {
    const KIND: Kind = Kind::FloatBits;
    fn read(item: Item<'a>) -> Result<Self, Error> {
        match item.value() {
            Value::FloatBits(bits) => Ok(Self(bits)),
            _ => Err(Error::WrongType { tag: None }),
        }
    }
    fn write(&self, tag: [u8; 3], w: &mut Encoder) -> Result<(), nfs_heat2::Error> {
        w.float_bits(tag, self.0)
    }
}

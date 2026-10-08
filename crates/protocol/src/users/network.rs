//! Network address subset and UserSessions wrappers.
//! These are pure codecs, not update policy or success-response definitions.
use super::{AbsentClientData, ExtendedDataMap, LatencyList, NetworkQosData, ObjectIdList};
use crate::{Error, Wire, schema};
use nfs_heat2::{Encoder, Field, Fields, Item, Kind, Limits, Value};
use std::{collections::BTreeSet, fmt};

schema!(IpAddress {
    ip: u32 => [0xa7, 0x00, 0x00],
    machine_id: u64 => [0xb6, 0x18, 0xe9],
    port: u16 => [0xc2, 0xfc, 0xb4],
});

schema!(IpPairAddress {
    external_address: IpAddress<'a> => [0x97, 0x8a, 0x70],
    internal_address: IpAddress<'a> => [0xa6, 0xea, 0x70],
    machine_id: u64 => [0xb6, 0x18, 0xe9],
});

/// Zero-based member selection; ordinal 2 is IpPair.
/// Only this observed variant and explicit unset 127 are supported. Other
/// variants fail rather than being silently treated as an empty address.
pub enum NetworkAddress<'a> {
    Unset,
    IpPair(IpPairAddress<'a>),
}

impl fmt::Debug for NetworkAddress<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unset => f.write_str("NetworkAddress::Unset"),
            Self::IpPair(_) => f.write_str("NetworkAddress::IpPair(<redacted>)"),
        }
    }
}

impl<'a> Wire<'a> for NetworkAddress<'a> {
    const KIND: Kind = Kind::Union;
    fn read(item: Item<'a>) -> Result<Self, Error> {
        match item.value() {
            Value::Union { discriminant: 127 } => Ok(Self::Unset),
            Value::Union { discriminant: 2 }
                if item.as_bytes().get(1..4) == Some(&[0xda, 0x1b, 0x35]) =>
            {
                let member = item
                    .union_member()
                    .ok_or(Error::WrongType { tag: None })??;
                Ok(Self::IpPair(IpPairAddress::read(member)?))
            }
            _ => Err(Error::WrongType { tag: None }),
        }
    }
    fn write(&self, tag: [u8; 3], w: &mut Encoder) -> Result<(), nfs_heat2::Error> {
        match self {
            Self::Unset => w.unset_union(tag),
            Self::IpPair(pair) => {
                w.struct_union(tag, 2, [0xda, 0x1b, 0x35], |w| pair.write_fields(w))
            }
        }
    }
    fn validate(&self, budget: &mut crate::Budget) -> Result<(), Error> {
        budget.take(1)?;
        if let Self::IpPair(pair) = self {
            pair.validate(budget)?;
        }
        Ok(())
    }
    fn unknown_count(&self) -> usize {
        match self {
            Self::Unset => 0,
            Self::IpPair(pair) => pair.unknown_field_count(),
        }
    }
}

/// NLMP is a byte-string to signed int32 map; keep wire entry order.
#[derive(Default)]
pub struct PingSiteLatencyMap<'a>(pub Vec<(&'a [u8], i32)>);

impl fmt::Debug for PingSiteLatencyMap<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("PingSiteLatencyMapLength")
            .field(&self.0.len())
            .finish()
    }
}

impl<'a> Wire<'a> for PingSiteLatencyMap<'a> {
    const KIND: Kind = Kind::Map;
    fn read(item: Item<'a>) -> Result<Self, Error> {
        if !matches!(
            item.value(),
            Value::Map {
                key: Kind::String,
                value: Kind::Integer,
                ..
            }
        ) {
            return Err(Error::WrongType { tag: None });
        }
        let mut elements = item.elements().ok_or(Error::WrongType { tag: None })?;
        let mut entries = Vec::new();
        while let Some(key) = elements.next() {
            let key = <&[u8]>::read(key?)?;
            let value = i32::read(elements.next().ok_or(Error::WrongType { tag: None })??)?;
            entries.push((key, value));
        }
        crate::unique_keys(entries.iter().map(|(key, _)| *key))?;
        Ok(Self(entries))
    }
    fn write(&self, tag: [u8; 3], w: &mut Encoder) -> Result<(), nfs_heat2::Error> {
        w.string_integer_map(
            tag,
            self.0.iter().map(|(key, value)| (*key, i64::from(*value))),
        )
    }
    fn validate(&self, budget: &mut crate::Budget) -> Result<(), Error> {
        budget.take(1)?;
        budget.collection(self.0.len(), 2)?;
        for (key, _) in &self.0 {
            crate::check_string(key, budget.limits)?;
        }
        crate::unique_keys(self.0.iter().map(|(key, _)| *key))
    }
}

schema!(NetworkInfo {
    address: NetworkAddress<'a> => [0x86, 0x49, 0x32],
    ping_site_latency_by_alias: PingSiteLatencyMap<'a> => [0xba, 0xcb, 0x70],
    qos_data: NetworkQosData<'a> => [0xbb, 0x1b, 0xf3],
});

// native names and masks. Unknown option/hardware bits remain intact.
pub const NETWORK_ADDRESS_ONLY: u32 = 1;
pub const NAT_INFO_ONLY: u32 = 2;
pub const UPDATE_METRICS: u32 = 4;
pub const VOIP_HEADSET_STATUS: u32 = 1;

// The association between these body schemas and route constants remains inferred.
schema!(UpdateNetworkInfoRequest {
    network_info: NetworkInfo<'a> => [0xa6, 0xe9, 0xaf],
    opts: u32 => [0xbf, 0x0d, 0x33],
});

schema!(UpdateHardwareFlagsRequest {
    hardware_flags: u32 => [0xa3, 0x79, 0xa7],
});

// This retains twelve fields without changing the initial-only API.
// Present dynamic CVAR and address alternatives other than IpPair stay unsupported.
schema!(UserSessionExtendedDataNetwork {
    address: NetworkAddress<'a> => [0x86, 0x49, 0x32],
    best_ping_site_alias: &'a [u8] => [0x8b, 0x0c, 0xc0],
    country: &'a [u8] => [0x8f, 0x4e, 0x40],
    client_data: AbsentClientData => [0x8f, 0x68, 0x72],
    data_map: ExtendedDataMap => [0x92, 0xd8, 0x70],
    hardware_flags: u32 => [0xa3, 0x79, 0xa7],
    isp: &'a [u8] => [0xa7, 0x3c, 0x00],
    latency_list: LatencyList => [0xc3, 0x3b, 0x2d],
    qos_data: NetworkQosData<'a> => [0xc6, 0x48, 0x74],
    time_zone: &'a [u8] => [0xd3, 0xa0, 0x00],
    user_info_attribute: u64 => [0xd6, 0x1d, 0x34],
    blaze_object_id_list: ObjectIdList => [0xd6, 0xcc, 0xf4],
});

schema!(UserSessionExtendedDataUpdate {
    extended_data: UserSessionExtendedDataNetwork<'a> => [0x92, 0x1d, 0x21],
    subscribed: bool => [0xcf, 0x58, 0xb3],
    user_id: i64 => [0xd7, 0x3a, 0x64],
});

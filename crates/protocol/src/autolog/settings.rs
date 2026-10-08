//! native UserSettings payloads. Preferences are account state, not defaults.
//! Float values retain their raw IEEE-754 binary32 bits without normalization.
use crate::{Error, Wire, check_string, schema, unique_keys};
use nfs_heat2::{Encoder, Field, Fields, Item, Kind, Limits, Value};
use std::{collections::BTreeSet, fmt};

pub const GET_USER_SETTINGS: u16 = 72;
pub const SET_USER_SETTINGS: u16 = 73;

fn read_map<'a>(item: Item<'a>, kind: Kind) -> Result<Vec<(&'a [u8], u32)>, Error> {
    if !matches!(item.value(), Value::Map { key: Kind::String, value, .. } if value == kind) {
        return Err(Error::WrongType { tag: None });
    }
    let mut elements = item.elements().ok_or(Error::WrongType { tag: None })?;
    let mut entries = Vec::new();
    while let Some(key) = elements.next() {
        let key = <&[u8]>::read(key?)?;
        let item = elements.next().ok_or(Error::WrongType { tag: None })??;
        let value = if kind == Kind::FloatBits {
            match item.value() {
                Value::FloatBits(bits) => bits,
                _ => return Err(Error::WrongType { tag: None }),
            }
        } else {
            u32::read(item)?
        };
        entries.push((key, value));
    }
    unique_keys(entries.iter().map(|(k, _)| *k))?;
    Ok(entries)
}

macro_rules! settings_map {
    ($name:ident, $kind:ident, $writer:ident, $value:expr) => {
        #[derive(Default)]
        pub struct $name<'a>(pub Vec<(&'a [u8], u32)>);
        impl fmt::Debug for $name<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($name))
                    .field("entries", &self.0.len())
                    .finish()
            }
        }
        impl<'a> Wire<'a> for $name<'a> {
            const KIND: Kind = Kind::Map;
            fn read(item: Item<'a>) -> Result<Self, Error> {
                Ok(Self(read_map(item, Kind::$kind)?))
            }
            fn write(&self, tag: [u8; 3], w: &mut Encoder) -> Result<(), nfs_heat2::Error> {
                w.$writer(tag, self.0.iter().map(|(k, v)| (*k, ($value)(*v))))
            }
            fn validate(&self, budget: &mut crate::Budget) -> Result<(), Error> {
                budget.take(1)?;
                budget.collection(self.0.len(), 2)?;
                for (key, _) in &self.0 {
                    check_string(key, budget.limits)?;
                }
                unique_keys(self.0.iter().map(|(key, _)| *key))
            }
        }
    };
}
settings_map!(SettingsFloatBits, FloatBits, string_float_bits_map, |v| v);
settings_map!(SettingsIntegers, Integer, string_integer_map, i64::from);

schema!(UserSettingsRequest { blaze_id: i64 => [0x8a,0xca,0x64] });
schema!(UserSettingsResponse {
    blaze_id: i64 => [0x8a,0xca,0x64],
    settings_flt: SettingsFloatBits<'a> => [0xcf,0x48,0x66],
    settings_int: SettingsIntegers<'a> => [0xcf,0x48,0x69],
});
schema!(UserSettingsUpdateRequest {
    blaze_id: i64 => [0x8a,0xca,0x64],
    settings_flt: SettingsFloatBits<'a> => [0xcf,0x48,0x66],
    settings_int: SettingsIntegers<'a> => [0xcf,0x48,0x69],
});
schema!(UserSettingsUpdateResponse {
    blaze_id: i64 => [0x8a,0xca,0x64],
    success: bool => [0xcf,0x58,0xe3],
});

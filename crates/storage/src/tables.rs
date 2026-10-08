// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Persistent rows: player ownership is the repository account; each table
//! has u64 secondary keys and named-column hashes, with integer, float and
//! string values. This is a local storage codec,
//! never a game wire format. Schemas/defaults belong to the domain boundary.
use crate::{Error, Op};
use std::collections::BTreeMap;

pub const MAX_TABLES: usize = 64;
pub const MAX_ROWS: usize = 4096;
pub const MAX_COLUMNS: usize = 128;
pub const MAX_STRING_BYTES: usize = 4096;
pub const MAX_TABLE_BYTES: usize = 128 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Int(i32),
    /// IEEE single-precision bits. Finite values only; retain negative zero.
    Float(u32),
    String(String),
}
impl Value {
    fn size(&self) -> Result<usize, Error> {
        match self {
            Self::Int(_) => Ok(4),
            Self::Float(bits) if f32::from_bits(*bits).is_finite() => Ok(4),
            Self::Float(_) => Err(Error::Invalid),
            Self::String(s) if s.len() <= MAX_STRING_BYTES => Ok(2 + s.len()),
            Self::String(_) => Err(Error::Bounds),
        }
    }
}
pub type Row = BTreeMap<u32, Value>;

/// Present with no rows is a loaded empty table, distinct from an absent table.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Table {
    pub rows: BTreeMap<u64, Row>,
}
impl Table {
    pub fn validate(&self) -> Result<(), Error> {
        if self.rows.len() > MAX_ROWS {
            return Err(Error::Bounds);
        }
        let mut size = 8;
        for row in self.rows.values() {
            if row.len() > MAX_COLUMNS {
                return Err(Error::Bounds);
            }
            size += 10;
            for value in row.values() {
                size += 5 + value.size()?;
                if size > MAX_TABLE_BYTES {
                    return Err(Error::Bounds);
                }
            }
        }
        if size > MAX_TABLE_BYTES {
            return Err(Error::Bounds);
        }
        Ok(())
    }
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        self.validate()?;
        let mut out = b"TBL1".to_vec();
        out.extend_from_slice(&(self.rows.len() as u32).to_be_bytes());
        for (key, row) in &self.rows {
            out.extend_from_slice(&key.to_be_bytes());
            out.extend_from_slice(&(row.len() as u16).to_be_bytes());
            for (column, value) in row {
                out.extend_from_slice(&column.to_be_bytes());
                match value {
                    Value::Int(v) => {
                        out.push(1);
                        out.extend_from_slice(&v.to_be_bytes());
                    }
                    Value::Float(v) => {
                        out.push(3);
                        out.extend_from_slice(&v.to_be_bytes());
                    }
                    Value::String(v) => {
                        out.push(4);
                        out.extend_from_slice(&(v.len() as u16).to_be_bytes());
                        out.extend_from_slice(v.as_bytes());
                    }
                }
            }
        }
        Ok(out)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        struct Reader<'a>(&'a [u8]);
        impl<'a> Reader<'a> {
            fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
                let (head, tail) = self.0.split_at_checked(n).ok_or(Error::Config)?;
                self.0 = tail;
                Ok(head)
            }
            fn fixed<const N: usize>(&mut self) -> Result<[u8; N], Error> {
                self.take(N)?.try_into().map_err(|_| Error::Config)
            }
        }
        if bytes.len() > MAX_TABLE_BYTES {
            return Err(Error::Config);
        }
        let mut input = Reader(bytes);
        if input.take(4)? != b"TBL1" {
            return Err(Error::Config);
        }
        let count = u32::from_be_bytes(input.fixed()?) as usize;
        if count > MAX_ROWS {
            return Err(Error::Config);
        }
        let mut table = Self::default();
        for _ in 0..count {
            let key = u64::from_be_bytes(input.fixed()?);
            let fields = u16::from_be_bytes(input.fixed()?) as usize;
            if fields > MAX_COLUMNS || table.rows.last_key_value().is_some_and(|(k, _)| *k >= key) {
                return Err(Error::Config);
            }
            let mut row = Row::new();
            for _ in 0..fields {
                let column = u32::from_be_bytes(input.fixed()?);
                let value = match input.fixed::<1>()?[0] {
                    1 => Value::Int(i32::from_be_bytes(input.fixed()?)),
                    3 => Value::Float(u32::from_be_bytes(input.fixed()?)),
                    4 => {
                        let length = u16::from_be_bytes(input.fixed()?) as usize;
                        if length > MAX_STRING_BYTES {
                            return Err(Error::Config);
                        }
                        Value::String(
                            std::str::from_utf8(input.take(length)?)
                                .map_err(|_| Error::Config)?
                                .to_owned(),
                        )
                    }
                    _ => return Err(Error::Config),
                };
                if row.last_key_value().is_some_and(|(k, _)| *k >= column) {
                    return Err(Error::Config);
                }
                value.size().map_err(|_| Error::Config)?;
                row.insert(column, value);
            }
            table.rows.insert(key, row);
        }
        if !input.0.is_empty() {
            return Err(Error::Config);
        }
        table.validate().map_err(|_| Error::Config)?;
        Ok(table)
    }
}

pub fn transition(
    previous: &BTreeMap<u32, Table>,
    ops: &[Op],
) -> Result<BTreeMap<u32, Table>, Error> {
    let mut next = previous.clone();
    for op in ops {
        if let Op::SetTable(id, table) = op {
            table.validate()?;
            next.insert(*id, table.clone());
        }
    }
    if next.len() > MAX_TABLES {
        return Err(Error::Bounds);
    }
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn typed_rows_preserve_empty_rows_full_keys_and_float_bits() {
        let table = Table {
            rows: BTreeMap::from([
                (0, Row::new()),
                (
                    u64::MAX,
                    Row::from([
                        (0, Value::Int(i32::MIN)),
                        (7, Value::Float((-0.0f32).to_bits())),
                        (u32::MAX, Value::String("å雪".into())),
                    ]),
                ),
            ]),
        };
        let encoded = table.encode().unwrap();
        assert_eq!(Table::decode(&encoded).unwrap(), table);
        for len in 0..encoded.len() {
            assert_eq!(Table::decode(&encoded[..len]), Err(Error::Config));
        }
        let mut concatenated = encoded.clone();
        concatenated.extend_from_slice(&encoded);
        assert_eq!(Table::decode(&concatenated), Err(Error::Config));
        assert_ne!(
            Table::default(),
            Table {
                rows: BTreeMap::from([(0, Row::new())])
            }
        );
    }
    #[test]
    fn duplicate_keys_unsorted_columns_and_invalid_utf8_are_rejected() {
        let table = Table {
            rows: BTreeMap::from([(1, Row::new()), (2, Row::new())]),
        };
        let mut bytes = table.encode().unwrap();
        bytes[18..26].copy_from_slice(&1u64.to_be_bytes());
        assert_eq!(Table::decode(&bytes), Err(Error::Config));
        let table = Table {
            rows: BTreeMap::from([(0, Row::from([(1, Value::Int(0)), (2, Value::Int(0))]))]),
        };
        let mut bytes = table.encode().unwrap();
        bytes[27..31].copy_from_slice(&1u32.to_be_bytes());
        assert_eq!(Table::decode(&bytes), Err(Error::Config));
        let table = Table {
            rows: BTreeMap::from([(0, Row::from([(1, Value::String("a".into()))]))]),
        };
        let mut bytes = table.encode().unwrap();
        *bytes.last_mut().unwrap() = 0xff;
        assert_eq!(Table::decode(&bytes), Err(Error::Config));
    }
    #[test]
    fn stored_shape_and_resource_bounds_are_enforced() {
        let mut table = Table {
            rows: BTreeMap::from([(1, Row::from([(2, Value::Int(3))]))]),
        };
        let mut bytes = table.encode().unwrap();
        bytes[22] = 9;
        assert_eq!(Table::decode(&bytes), Err(Error::Config));
        table
            .rows
            .get_mut(&1)
            .unwrap()
            .insert(2, Value::Float(f32::NAN.to_bits()));
        assert_eq!(table.validate(), Err(Error::Invalid));
        table
            .rows
            .get_mut(&1)
            .unwrap()
            .insert(2, Value::String("x".repeat(MAX_STRING_BYTES + 1)));
        assert_eq!(table.validate(), Err(Error::Bounds));
        table.rows = (0..=MAX_ROWS as u64).map(|key| (key, Row::new())).collect();
        assert_eq!(table.validate(), Err(Error::Bounds));
        table.rows = BTreeMap::from([(
            0,
            (0..=MAX_COLUMNS as u32)
                .map(|k| (k, Value::Int(0)))
                .collect(),
        )]);
        assert_eq!(table.validate(), Err(Error::Bounds));
        table.rows = (0..64)
            .map(|key| {
                (
                    key,
                    Row::from([(0, Value::String("x".repeat(MAX_STRING_BYTES)))]),
                )
            })
            .collect();
        assert_eq!(table.validate(), Err(Error::Bounds));
    }
}

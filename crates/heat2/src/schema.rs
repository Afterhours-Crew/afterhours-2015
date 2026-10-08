// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Borrowed, indexed layouts. Collection byte 3 needs a layout.
use crate::{ErrorKind, Kind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TypeId(pub usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Member {
    pub tag: [u8; 3],
    pub ty: TypeId,
}

/// Struct members are sorted by tag; union members retain descriptor order.
/// Unsupported nodes allow recording a complete graph without claiming support.
#[derive(Clone, Copy, Debug)]
pub enum Type<'a> {
    Scalar(Kind),
    /// : tagged integer callback, but collection mapper returns byte 11.
    /// No unit or epoch is assigned here.
    TimeValue,
    Struct(&'a [Member]),
    List(TypeId),
    Map {
        key: TypeId,
        value: TypeId,
    },
    Union(&'a [Member]),
    Unsupported(u8),
}

impl Type<'_> {
    pub(crate) fn kind(self) -> Result<Kind, ErrorKind> {
        Ok(match self {
            Self::Scalar(kind) => kind,
            Self::TimeValue => Kind::Integer,
            Self::Struct(_) => Kind::Struct,
            Self::List(_) => Kind::List,
            Self::Map { .. } => Kind::Map,
            Self::Union(_) => Kind::Union,
            Self::Unsupported(wire) => return Err(ErrorKind::UnsupportedType(wire)),
        })
    }
    pub(crate) fn collection_wire(self) -> Result<u8, ErrorKind> {
        Ok(match self {
            Self::Struct(_) | Self::List(_) | Self::Map { .. } | Self::Union(_) => 3,
            Self::TimeValue => 11,
            _ => self.kind()? as u8,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchemaError {
    SizeLimit,
    InvalidReference,
    InvalidScalar,
    InvalidTag,
    UnsortedOrDuplicateTag,
    TooManyUnionMembers,
}
impl std::fmt::Display for SchemaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "heat2 schema {self:?}")
    }
}
impl std::error::Error for SchemaError {}

/// Validated immutable graph, at most 4096 types and 65536 members.
/// Cycles are allowed; the decoder's depth and value budgets bound traversal.
#[derive(Clone, Copy, Debug)]
pub struct Schema<'a> {
    types: &'a [Type<'a>],
}
impl<'a> Schema<'a> {
    pub fn new(types: &'a [Type<'a>]) -> Result<Self, SchemaError> {
        if types.len() > 4096 {
            return Err(SchemaError::SizeLimit);
        }
        let reference = |id: TypeId| {
            if id.0 < types.len() {
                Ok(())
            } else {
                Err(SchemaError::InvalidReference)
            }
        };
        let mut total = 0usize;
        for ty in types {
            match *ty {
                Type::Scalar(Kind::Struct | Kind::List | Kind::Map | Kind::Union) => {
                    return Err(SchemaError::InvalidScalar);
                }
                Type::Scalar(_) | Type::TimeValue | Type::Unsupported(_) => {}
                Type::List(id) => reference(id)?,
                Type::Map { key, value } => {
                    reference(key)?;
                    reference(value)?;
                }
                Type::Struct(members) | Type::Union(members) => {
                    total = total
                        .checked_add(members.len())
                        .ok_or(SchemaError::SizeLimit)?;
                    if total > 65536 {
                        return Err(SchemaError::SizeLimit);
                    }
                    let union = matches!(ty, Type::Union(_));
                    if union && members.len() > 127 {
                        return Err(SchemaError::TooManyUnionMembers);
                    }
                    for (index, member) in members.iter().enumerate() {
                        reference(member.ty)?;
                        if member.tag[0] == 0 {
                            return Err(SchemaError::InvalidTag);
                        }
                        if !union && index > 0 && members[index - 1].tag >= member.tag {
                            return Err(SchemaError::UnsortedOrDuplicateTag);
                        }
                    }
                }
            }
        }
        Ok(Self { types })
    }
    pub(crate) fn get(self, id: TypeId) -> Result<Type<'a>, ErrorKind> {
        self.types
            .get(id.0)
            .copied()
            .ok_or(ErrorKind::InvalidSchema)
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct Context<'a> {
    pub schema: Option<Schema<'a>>,
    pub ty: Option<TypeId>,
}
impl<'a> Context<'a> {
    pub fn child(self, ty: Option<TypeId>) -> Self {
        Self { ty, ..self }
    }
    pub fn layout(self) -> Result<Option<Type<'a>>, ErrorKind> {
        self.ty
            .map(|id| self.schema.ok_or(ErrorKind::InvalidSchema)?.get(id))
            .transpose()
    }
    pub fn member(self, tag: [u8; 3]) -> Result<Self, ErrorKind> {
        let id = match self.layout()? {
            Some(Type::Struct(members)) => members
                .binary_search_by_key(&tag, |m| m.tag)
                .ok()
                .map(|i| members[i].ty),
            None => None,
            _ => return Err(ErrorKind::InvalidSchema),
        };
        Ok(self.child(id))
    }
    pub fn elements(self) -> Result<[Self; 2], ErrorKind> {
        Ok(match self.layout()? {
            Some(Type::List(id)) => [self.child(Some(id)); 2],
            Some(Type::Map { key, value }) => [self.child(Some(key)), self.child(Some(value))],
            None => [self.child(None); 2],
            _ => return Err(ErrorKind::InvalidSchema),
        })
    }
    pub fn selected(self, selector: u8) -> Result<Option<Member>, ErrorKind> {
        match self.layout()? {
            Some(Type::Union(members)) => members
                .get(usize::from(selector))
                .copied()
                .map(Some)
                .ok_or(ErrorKind::InvalidSelector),
            None => Ok(None),
            _ => Err(ErrorKind::InvalidSchema),
        }
    }
}

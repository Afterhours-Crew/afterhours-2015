// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Borrowed navigation over already validated bytes. Each iteration scans one
//! subtree; repeated nested traversal costs at most depth times document work.
use crate::{Context, Error, ErrorKind, Event, Kind, Limits, Member, Parser, Stats, Value};
use std::fmt;

#[derive(Clone, Copy)]
pub struct Field<'a> {
    tag: [u8; 3],
    bytes: &'a [u8],
    item: Item<'a>,
}

impl<'a> Field<'a> {
    pub fn tag(self) -> [u8; 3] {
        self.tag
    }
    pub fn item(self) -> Item<'a> {
        self.item
    }
    /// Exact header and value, including nonminimal integers and unknown data.
    pub fn as_bytes(self) -> &'a [u8] {
        self.bytes
    }
    pub(crate) fn validate(self, limits: Limits) -> Result<Stats, Error> {
        if self.bytes.len() > limits.max_bytes {
            return Err(Error {
                offset: 0,
                kind: ErrorKind::InputLimit,
            });
        }
        let mut parser = parser(self.bytes, limits, |_| {});
        let member = self.item.context.ty.map(|ty| Member { tag: self.tag, ty });
        parser.field(0, self.item.context.child(None), member)?;
        Ok(parser.stats)
    }
}

impl fmt::Debug for Field<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Field")
            .field("tag", &self.tag)
            .field("item", &self.item)
            .finish()
    }
}

#[derive(Clone, Copy)]
pub struct Item<'a> {
    pub(crate) bytes: &'a [u8],
    value: Value<'a>,
    limits: Limits,
    context: Context<'a>,
    tagged: bool,
}

impl fmt::Debug for Item<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Item")
            .field("value", &self.value)
            .field("bytes", &self.bytes.len())
            .finish()
    }
}

impl<'a> Item<'a> {
    pub fn value(self) -> Value<'a> {
        self.value
    }
    /// Supplied layout identity, absent for unknown or schema-free values.
    pub fn schema_type(self) -> Option<crate::TypeId> {
        self.context.ty
    }
    pub fn fields(self) -> Option<Fields<'a>> {
        matches!(self.value, Value::Struct).then(|| {
            Fields::new(
                &self.bytes[..self.bytes.len() - 1],
                self.limits,
                self.context,
            )
        })
    }
    /// Exact value representation, without any field header.
    pub fn as_bytes(self) -> &'a [u8] {
        self.bytes
    }
    /// Selected union value; None for other kinds or an unset (0x7f) union.
    /// Tagged and bare union contexts are retained from the parent traversal.
    pub fn union_member(self) -> Option<Result<Item<'a>, Error>> {
        let Value::Union { discriminant } = self.value else {
            return None;
        };
        if discriminant == 0x7f {
            return None;
        }
        Some((|| {
            let selected = self
                .context
                .selected(discriminant)
                .map_err(|kind| Error { offset: 0, kind })?;
            let (context, bare) = if self.tagged {
                (self.context.child(None), None)
            } else {
                let member = selected.ok_or(Error {
                    offset: 0,
                    kind: ErrorKind::SchemaRequired,
                })?;
                let context = self.context.child(Some(member.ty));
                let kind = context
                    .layout()
                    .and_then(|layout| layout.ok_or(ErrorKind::InvalidSchema)?.kind())
                    .map_err(|kind| Error { offset: 0, kind })?;
                (context, Some(kind))
            };
            read(&self.bytes[1..], self.limits, bare, context, selected).map(|(_, item)| item)
        })())
    }
    /// Map elements alternate key/value; list elements have one common kind.
    pub fn elements(self) -> Option<Elements<'a>> {
        let (first, second, count, header) = match self.value {
            Value::List { element, count } => (element, element, count, 1),
            Value::Map { key, value, count } => (key, value, count * 2, 2),
            _ => return None,
        };
        // This header was already validated by decode. Use the same integer reader.
        let mut parser = parser(self.bytes, self.limits, |_| {});
        parser.take(header).ok()?;
        parser.length().ok()?;
        Some(Elements {
            bytes: &self.bytes[parser.position..],
            limits: self.limits,
            kinds: [first, second],
            contexts: self.context.elements().ok()?,
            remaining: count,
            index: 0,
        })
    }
}

pub struct Fields<'a> {
    bytes: &'a [u8],
    limits: Limits,
    context: Context<'a>,
}
impl<'a> Fields<'a> {
    pub(crate) fn new(bytes: &'a [u8], limits: Limits, context: Context<'a>) -> Self {
        Self {
            bytes,
            limits,
            context,
        }
    }
}
impl<'a> Iterator for Fields<'a> {
    type Item = Result<Field<'a>, Error>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.bytes.is_empty() {
            return None;
        }
        let result =
            read(self.bytes, self.limits, None, self.context, None).map(|(size, item)| Field {
                tag: [self.bytes[0], self.bytes[1], self.bytes[2]],
                bytes: &self.bytes[..size],
                item,
            });
        match &result {
            Ok(field) => self.bytes = &self.bytes[field.bytes.len()..],
            Err(_) => self.bytes = &[],
        }
        Some(result)
    }
}

pub struct Elements<'a> {
    bytes: &'a [u8],
    limits: Limits,
    kinds: [Kind; 2],
    contexts: [Context<'a>; 2],
    remaining: usize,
    index: usize,
}
impl<'a> Iterator for Elements<'a> {
    type Item = Result<Item<'a>, Error>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let result = read(
            self.bytes,
            self.limits,
            Some(self.kinds[self.index % 2]),
            self.contexts[self.index % 2],
            None,
        );
        self.index += 1;
        self.remaining -= 1;
        Some(match result {
            Ok((size, item)) => {
                self.bytes = &self.bytes[size..];
                Ok(item)
            }
            Err(error) => {
                self.remaining = 0;
                Err(error)
            }
        })
    }
}

fn parser<'a, F: FnMut(Event<'a>)>(bytes: &'a [u8], limits: Limits, visitor: F) -> Parser<'a, F> {
    Parser {
        bytes,
        position: 0,
        limits,
        stats: Stats::default(),
        visitor,
    }
}

fn read<'a>(
    bytes: &'a [u8],
    limits: Limits,
    bare: Option<Kind>,
    context: Context<'a>,
    selected: Option<Member>,
) -> Result<(usize, Item<'a>), Error> {
    let mut first = None;
    let mut parser = parser(bytes, limits, |event| {
        if let Event::Value {
            depth: 0, value, ..
        } = event
        {
            first = Some(value);
        }
    });
    if let Some(kind) = bare {
        parser.value(kind, 0, false, context)?;
    } else {
        parser.field(0, context, selected)?;
    }
    let end = parser.position;
    let context = if bare.is_some() {
        context
    } else if let Some(member) = selected {
        context.child(Some(member.ty))
    } else {
        context
            .member([bytes[0], bytes[1], bytes[2]])
            .map_err(|kind| Error { offset: 0, kind })?
    };
    Ok((
        end,
        Item {
            bytes: &bytes[if bare.is_some() { 0 } else { 4 }..end],
            value: first.ok_or(Error {
                offset: 0,
                kind: ErrorKind::Truncated,
            })?,
            limits,
            context,
            tagged: bare.is_none(),
        },
    ))
}

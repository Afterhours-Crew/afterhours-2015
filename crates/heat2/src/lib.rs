//! Allocation-free structural decoding of the evidenced heat2 subset.
//!
//! Field tags are preserved verbatim; [`tag_text`] exposes their display bytes.
//! Tagged types 11 and 12 are intentionally unsupported.
//! Type 7 is supported only as a tagged field, not a bare collection element.
//! : [`decode`] interprets collection byte 3 as Struct, but the client also
//! uses it for map/list/union elements. Use [`decode_with_schema`] to resolve
//! these layouts without guessing.
//! Validate a complete Fire2 body or metadata section with [`decode`] before
//! visiting values. No application operation or schema is implied by parsing.
use std::fmt;

mod schema;
mod view;
mod writer;
use schema::Context;
pub use schema::{Member, Schema, SchemaError, Type, TypeId};
pub use view::{Elements, Field, Fields, Item};
pub use writer::Encoder;

/// Translate the four six-bit tag groups to the client's display bytes.
/// A zero group becomes NUL; other groups become ASCII 0x21..=0x5f.
/// All four positions are retained, including interior and trailing NULs.
/// This is a display conversion, not a schema lookup or tag validity check.
pub fn tag_text(tag: [u8; 3]) -> [u8; 4] {
    let packed = u32::from_be_bytes([0, tag[0], tag[1], tag[2]]);
    std::array::from_fn(|index| {
        let group = ((packed >> (18 - 6 * index)) & 0x3f) as u8;
        if group == 0 { 0 } else { group + 0x20 }
    })
}

/// Tagged-field wire discriminants established from the encoder callbacks.
/// Collection headers use a separate mapping; byte 3 is ambiguous.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Kind {
    Integer = 0,
    String = 1,
    Blob = 2,
    Struct = 3,
    List = 4,
    Map = 5,
    Union = 6,
    Variable = 7,
    IntegerPair = 8,
    IntegerTriple = 9,
    FloatBits = 10,
}

impl TryFrom<u8> for Kind {
    type Error = ErrorKind;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => Self::Integer,
            1 => Self::String,
            2 => Self::Blob,
            3 => Self::Struct,
            4 => Self::List,
            5 => Self::Map,
            6 => Self::Union,
            7 => Self::Variable,
            8 => Self::IntegerPair,
            9 => Self::IntegerTriple,
            10 => Self::FloatBits,
            other => return Err(ErrorKind::UnsupportedType(other)),
        })
    }
}

/// Per-document work bounds. Depth counts container edges, starting at zero.
/// The implementation also enforces a hard depth ceiling of 32.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_bytes: usize,
    pub max_depth: usize,
    pub max_values: usize,
    pub max_collection: usize,
    pub max_byte_string: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_bytes: 512 * 1024,
            max_depth: 16,
            max_values: 16 * 1024,
            max_collection: 4096,
            max_byte_string: 256 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorKind {
    InputLimit,
    DepthLimit,
    ValueLimit,
    CollectionLimit,
    ByteStringLimit,
    Truncated,
    IntegerOverflow,
    NegativeLength,
    InvalidString,
    UnexpectedTerminator,
    UnsupportedType(u8),
    UnsupportedContext,
    InvalidVariable,
    InvalidTag,
    AllocationFailed,
    CollectionCountMismatch,
    InvalidSchema,
    SchemaRequired,
    SchemaMismatch,
    InvalidSelector,
}

/// Byte offsets and error categories are safe to log; input bytes are omitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Error {
    pub offset: usize,
    pub kind: ErrorKind,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "heat2 {:?} at byte {}", self.kind, self.offset)
    }
}
impl std::error::Error for Error {}

/// Values are exposed only through explicit matching. Debug output is redacted.
/// Strings exclude their required terminal NUL and are not assumed to be UTF-8.
#[derive(Clone, Copy, PartialEq)]
pub enum Value<'a> {
    Integer(i64),
    String(&'a [u8]),
    Blob(&'a [u8]),
    Struct,
    List {
        element: Kind,
        count: usize,
    },
    Map {
        key: Kind,
        value: Kind,
        count: usize,
    },
    Union {
        discriminant: u8,
    },
    IntegerPair([i64; 2]),
    Variable {
        type_id: Option<u32>,
    },
    IntegerTriple([i64; 3]),
    FloatBits(u32),
}

impl fmt::Debug for Value<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Integer(_) => f.write_str("Integer(<redacted>)"),
            Self::String(bytes) => f.debug_tuple("StringLength").field(&bytes.len()).finish(),
            Self::Blob(bytes) => f.debug_tuple("BlobLength").field(&bytes.len()).finish(),
            Self::Struct => f.write_str("Struct"),
            Self::List { element, count } => f
                .debug_struct("List")
                .field("element", element)
                .field("count", count)
                .finish(),
            Self::Map { key, value, count } => f
                .debug_struct("Map")
                .field("key", key)
                .field("value", value)
                .field("count", count)
                .finish(),
            Self::Union { discriminant } => f
                .debug_struct("Union")
                .field("discriminant", discriminant)
                .finish(),
            Self::IntegerPair(_) => f.write_str("IntegerPair(<redacted>)"),
            Self::Variable { type_id } => f
                .debug_struct("Variable")
                .field("present", &type_id.is_some())
                .finish(),
            Self::IntegerTriple(_) => f.write_str("IntegerTriple(<redacted>)"),
            Self::FloatBits(_) => f.write_str("FloatBits(<redacted>)"),
        }
    }
}

/// In wire order: Field, Value, any children, then End for each container.
/// List/map members have Value events without Field events. Struct members and
/// tagged union members carry their own Field events; bare union members do not.
/// Resolved list/map kinds may differ from their raw header byte. Offsets are relative
/// to the whole document. End.offset is the first byte after the container.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event<'a> {
    Field {
        offset: usize,
        depth: usize,
        tag: [u8; 3],
        kind: Kind,
    },
    Value {
        offset: usize,
        depth: usize,
        value: Value<'a>,
    },
    End {
        offset: usize,
        depth: usize,
        kind: Kind,
    },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Stats {
    pub fields: usize,
    pub values: usize,
    pub max_depth: usize,
}

/// A fully validated, borrowed message; no payload allocation.
/// Unknown tags and field order are retained in the original bytes and events.
pub struct Document<'a> {
    bytes: &'a [u8],
    limits: Limits,
    stats: Stats,
    context: Context<'a>,
}

impl fmt::Debug for Document<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Document")
            .field("bytes", &self.bytes.len())
            .field("stats", &self.stats)
            .finish()
    }
}

impl<'a> Document<'a> {
    /// Borrow immediate fields; nested traversal remains bounded and allocation-free.
    pub fn fields(&self) -> Fields<'a> {
        Fields::new(self.bytes, self.limits, self.context)
    }
    pub fn stats(&self) -> Stats {
        self.stats
    }
    /// Exact original representation, including bounded nonminimal integers.
    pub fn as_bytes(&self) -> &'a [u8] {
        self.bytes
    }
    /// Rewalk validated bytes with the same work bounds, without allocation.
    /// The callback can access private values; do not log them indiscriminately.
    pub fn visit(&self, visitor: impl FnMut(Event<'a>)) -> Result<Stats, Error> {
        scan(self.bytes, self.limits, self.context, visitor)
    }
}

/// Validate a complete root sequence (no root terminator). Nested structs end
/// with one zero byte. Empty root messages are valid. Unsupported types fail
/// explicitly since their size cannot safely be inferred.
pub fn decode(bytes: &[u8], limits: Limits) -> Result<Document<'_>, Error> {
    document(bytes, limits, Context::default())
}

/// Decode with an independently established struct root. Known tagged kinds and
/// collection headers must match. Unknown fields are retained if unambiguous;
/// unknown collection byte 3 fails with SchemaRequired, even for empty containers.
/// A successful parse validates this supplied layout, not the service binding.
pub fn decode_with_schema<'a>(
    bytes: &'a [u8],
    limits: Limits,
    schema: Schema<'a>,
    root: TypeId,
) -> Result<Document<'a>, Error> {
    if !matches!(schema.get(root), Ok(Type::Struct(_))) {
        return Err(Error {
            offset: 0,
            kind: ErrorKind::InvalidSchema,
        });
    }
    document(
        bytes,
        limits,
        Context {
            schema: Some(schema),
            ty: Some(root),
        },
    )
}

fn document<'a>(
    bytes: &'a [u8],
    limits: Limits,
    context: Context<'a>,
) -> Result<Document<'a>, Error> {
    let stats = scan(bytes, limits, context, |_| {})?;
    Ok(Document {
        bytes,
        limits,
        stats,
        context,
    })
}

fn scan<'a>(
    bytes: &'a [u8],
    limits: Limits,
    context: Context<'a>,
    visitor: impl FnMut(Event<'a>),
) -> Result<Stats, Error> {
    let mut parser = Parser {
        bytes,
        position: 0,
        limits,
        stats: Stats::default(),
        visitor,
    };
    if bytes.len() > limits.max_bytes {
        return Err(parser.error(ErrorKind::InputLimit));
    }
    while parser.position < bytes.len() {
        parser.field(0, context, None)?;
    }
    Ok(parser.stats)
}

struct Parser<'a, F> {
    bytes: &'a [u8],
    position: usize,
    limits: Limits,
    stats: Stats,
    visitor: F,
}

impl<'a, F: FnMut(Event<'a>)> Parser<'a, F> {
    fn error(&self, kind: ErrorKind) -> Error {
        Error {
            offset: self.position,
            kind,
        }
    }
    fn take(&mut self, count: usize) -> Result<&'a [u8], Error> {
        if count > self.bytes.len() - self.position {
            return Err(self.error(ErrorKind::Truncated));
        }
        let start = self.position;
        self.position += count;
        Ok(&self.bytes[start..self.position])
    }
    fn byte(&mut self) -> Result<u8, Error> {
        Ok(self.take(1)?[0])
    }
    fn kind(&mut self) -> Result<Kind, Error> {
        let offset = self.position;
        Kind::try_from(self.byte()?).map_err(|kind| Error { offset, kind })
    }
    fn integer(&mut self) -> Result<i64, Error> {
        let first = self.byte()?;
        let mut magnitude = u64::from(first & 0x3f);
        let mut continuation = first & 0x80 != 0;
        let mut shift = 6;
        while continuation {
            if shift > 62 {
                return Err(self.error(ErrorKind::IntegerOverflow));
            }
            let byte = self.byte()?;
            let part = u64::from(byte & 0x7f);
            if part > (i64::MAX as u64 >> shift) {
                return Err(self.error(ErrorKind::IntegerOverflow));
            }
            magnitude |= part << shift;
            continuation = byte & 0x80 != 0;
            shift += 7;
        }
        // : the client's reader explicitly maps sign + zero to INT64_MIN.
        Ok(if first & 0x40 == 0 {
            magnitude as i64
        } else if magnitude == 0 {
            i64::MIN
        } else {
            -(magnitude as i64)
        })
    }
    fn length(&mut self) -> Result<usize, Error> {
        let value = self.integer()?;
        if value < 0 {
            return Err(self.error(ErrorKind::NegativeLength));
        }
        usize::try_from(value).map_err(|_| self.error(ErrorKind::IntegerOverflow))
    }
    fn field(
        &mut self,
        depth: usize,
        parent: Context<'a>,
        selected: Option<Member>,
    ) -> Result<Kind, Error> {
        let offset = self.position;
        let first = self.byte()?;
        if first == 0 {
            return Err(self.error(ErrorKind::UnexpectedTerminator));
        }
        let tag = [first, self.byte()?, self.byte()?];
        let kind = self.kind()?;
        let context = if let Some(member) = selected {
            if member.tag != tag {
                return Err(self.error(ErrorKind::SchemaMismatch));
            }
            parent.child(Some(member.ty))
        } else {
            parent.member(tag).map_err(|kind| self.error(kind))?
        };
        if let Some(layout) = context.layout().map_err(|kind| self.error(kind))?
            && layout.kind().map_err(|kind| self.error(kind))? != kind
        {
            return Err(self.error(ErrorKind::SchemaMismatch));
        }
        self.stats.fields += 1;
        (self.visitor)(Event::Field {
            offset,
            depth,
            tag,
            kind,
        });
        self.value(kind, depth, true, context)?;
        Ok(kind)
    }
    fn collection(&self, count: usize, multiplier: usize) -> Result<(), Error> {
        if count > self.limits.max_collection {
            return Err(self.error(ErrorKind::CollectionLimit));
        }
        if count > (self.limits.max_values - self.stats.values) / multiplier {
            return Err(self.error(ErrorKind::ValueLimit));
        }
        Ok(())
    }
    fn emit(&mut self, offset: usize, depth: usize, value: Value<'a>) {
        (self.visitor)(Event::Value {
            offset,
            depth,
            value,
        });
    }
    fn collection_kind(&mut self, context: Context<'a>) -> Result<Kind, Error> {
        let offset = self.position;
        let wire = self.byte()?;
        if let Some(layout) = context.layout().map_err(|kind| self.error(kind))? {
            let kind = layout.kind().map_err(|kind| self.error(kind))?;
            let expected = layout.collection_wire().map_err(|kind| self.error(kind))?;
            if wire != expected {
                return Err(Error {
                    offset,
                    kind: ErrorKind::SchemaMismatch,
                });
            }
            Ok(kind)
        } else if context.schema.is_some() && wire == 3 {
            Err(Error {
                offset,
                kind: ErrorKind::SchemaRequired,
            })
        } else {
            let kind = Kind::try_from(wire).map_err(|kind| Error { offset, kind })?;
            if context.schema.is_some() && matches!(kind, Kind::List | Kind::Map | Kind::Union) {
                return Err(Error {
                    offset,
                    kind: ErrorKind::SchemaMismatch,
                });
            }
            Ok(kind)
        }
    }
    fn value(
        &mut self,
        kind: Kind,
        depth: usize,
        tagged: bool,
        context: Context<'a>,
    ) -> Result<(), Error> {
        if depth > self.limits.max_depth.min(32) {
            return Err(self.error(ErrorKind::DepthLimit));
        }
        if self.stats.values >= self.limits.max_values {
            return Err(self.error(ErrorKind::ValueLimit));
        }
        self.stats.values += 1;
        self.stats.max_depth = self.stats.max_depth.max(depth);
        let offset = self.position;
        match kind {
            Kind::Integer => {
                let value = self.integer()?;
                self.emit(offset, depth, Value::Integer(value));
            }
            Kind::String | Kind::Blob => {
                let length = self.length()?;
                if length > self.limits.max_byte_string {
                    return Err(self.error(ErrorKind::ByteStringLimit));
                }
                let bytes = self.take(length)?;
                let value = if kind == Kind::String {
                    if bytes.last() != Some(&0) {
                        return Err(self.error(ErrorKind::InvalidString));
                    }
                    Value::String(&bytes[..length - 1])
                } else {
                    Value::Blob(bytes)
                };
                self.emit(offset, depth, value);
            }
            Kind::Struct => {
                self.emit(offset, depth, Value::Struct);
                loop {
                    if self.bytes.get(self.position) == Some(&0) {
                        self.position += 1;
                        break;
                    }
                    self.field(depth + 1, context, None)?;
                }
            }
            Kind::List => {
                let children = context.elements().map_err(|kind| self.error(kind))?;
                let element = self.collection_kind(children[0])?;
                let count = self.length()?;
                self.collection(count, 1)?;
                self.emit(offset, depth, Value::List { element, count });
                for _ in 0..count {
                    self.value(element, depth + 1, false, children[0])?;
                }
            }
            Kind::Map => {
                let children = context.elements().map_err(|kind| self.error(kind))?;
                let key = self.collection_kind(children[0])?;
                let value = self.collection_kind(children[1])?;
                let count = self.length()?;
                self.collection(count, 2)?;
                self.emit(offset, depth, Value::Map { key, value, count });
                for _ in 0..count {
                    self.value(key, depth + 1, false, children[0])?;
                    self.value(value, depth + 1, false, children[1])?;
                }
            }
            Kind::Union => {
                let discriminant = self.byte()?;
                self.emit(offset, depth, Value::Union { discriminant });
                if discriminant != 0x7f {
                    let member = context
                        .selected(discriminant)
                        .map_err(|kind| self.error(kind))?;
                    if tagged {
                        self.field(depth + 1, context.child(None), member)?;
                    } else {
                        let member = member.ok_or_else(|| self.error(ErrorKind::SchemaRequired))?;
                        let child = context.child(Some(member.ty));
                        let kind = child
                            .layout()
                            .map_err(|kind| self.error(kind))?
                            .ok_or_else(|| self.error(ErrorKind::InvalidSchema))?
                            .kind()
                            .map_err(|kind| self.error(kind))?;
                        self.value(kind, depth + 1, false, child)?;
                    }
                }
            }
            Kind::IntegerPair => {
                let pair = [self.integer()?, self.integer()?];
                self.emit(offset, depth, Value::IntegerPair(pair));
            }
            Kind::Variable => {
                // The tagged callback emits presence, type ID, one tagged
                // struct and an additional terminator. Bare collection values
                // take a different header-suppression path, not yet validated.
                if !tagged {
                    return Err(self.error(ErrorKind::UnsupportedContext));
                }
                match self.byte()? {
                    0 => self.emit(offset, depth, Value::Variable { type_id: None }),
                    1 => {
                        let id = self.integer()?;
                        let id = u32::try_from(id)
                            .ok()
                            .filter(|id| *id != 0)
                            .ok_or_else(|| self.error(ErrorKind::InvalidVariable))?;
                        self.emit(offset, depth, Value::Variable { type_id: Some(id) });
                        if self.field(depth + 1, context.child(None), None)? != Kind::Struct
                            || self.byte()? != 0
                        {
                            return Err(self.error(ErrorKind::InvalidVariable));
                        }
                    }
                    _ => return Err(self.error(ErrorKind::InvalidVariable)),
                }
            }
            Kind::IntegerTriple => {
                let triple = [self.integer()?, self.integer()?, self.integer()?];
                self.emit(offset, depth, Value::IntegerTriple(triple));
            }
            Kind::FloatBits => {
                let bytes = self.take(4)?;
                let bits = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                self.emit(offset, depth, Value::FloatBits(bits));
            }
        }
        if matches!(
            kind,
            Kind::Struct | Kind::List | Kind::Map | Kind::Union | Kind::Variable
        ) {
            (self.visitor)(Event::End {
                offset: self.position,
                depth,
                kind,
            });
        }
        Ok(())
    }
}

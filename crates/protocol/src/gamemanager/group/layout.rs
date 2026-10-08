//! Only collection header 3 requires disambiguation beyond tagged wire kinds.
//! native metadata fixes these paths; other tags are still preserved.
use nfs_heat2::{Document, Encoder, Error, Limits, Member, Schema, Type, TypeId};
const fn member(tag: [u8; 3], ty: usize) -> Member {
    Member {
        tag,
        ty: TypeId(ty),
    }
}
const TYPES: &[Type<'static>] = &[
    Type::Unsupported(6), // Unimplemented NetworkAddress alternatives.
    Type::Struct(&[]),    // IpPairAddress: tagged children have unambiguous layouts.
    Type::Union(&[
        member([0xda, 0x1b, 0x35], 0),
        member([0xda, 0x1b, 0x35], 0),
        member([0xda, 0x1b, 0x35], 1),
        member([0xda, 0x1b, 0x35], 0),
        member([0xda, 0x1b, 0x35], 0),
    ]),
    Type::List(TypeId(2)),
    Type::Struct(&[
        // ReplicatedGameData
        member([0x92, 0xe9, 0x74], 3),
        member([0xa2, 0xe9, 0x74], 3),
        member([0xca, 0xe9, 0xaf], 8),
    ]),
    Type::Struct(&[
        // NotifyGameSetup
        member([0x9e, 0x1b, 0x65], 4),
        member([0xc3, 0x2b, 0xf3], 9),
        member([0xc7, 0x59, 0x75], 9),
    ]),
    Type::Scalar(nfs_heat2::Kind::String),
    Type::Map {
        key: TypeId(6),
        value: TypeId(1),
    }, // RoleCriteriaMap
    Type::Struct(&[member([0x8f, 0x2a, 0x74], 7)]), // RoleInformation
    Type::List(TypeId(1)),                          // ReplicatedGamePlayer: tagged children.
];
fn schema() -> Schema<'static> {
    Schema::new(TYPES).expect("constant group collection layout")
}
pub(super) fn decode_game(bytes: &[u8], limits: Limits) -> Result<Document<'_>, Error> {
    nfs_heat2::decode_with_schema(bytes, limits, schema(), TypeId(4))
}
pub(super) fn decode_setup(bytes: &[u8], limits: Limits) -> Result<Document<'_>, Error> {
    nfs_heat2::decode_with_schema(bytes, limits, schema(), TypeId(5))
}
pub(super) fn finish_game(writer: Encoder) -> Result<Vec<u8>, Error> {
    writer.finish_with_schema(schema(), TypeId(4))
}
pub(super) fn finish_setup(writer: Encoder) -> Result<Vec<u8>, Error> {
    writer.finish_with_schema(schema(), TypeId(5))
}

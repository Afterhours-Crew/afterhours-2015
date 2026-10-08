//! AssociationLists startup subset, independent of list policy.
//! The route-to-body association remains inferred; no service policy is supplied.
//! MEML's element schema is unresolved; it remains an opaque unknown field.
use crate::{Error, Wire, schema, users::ObjectId};
use nfs_heat2::{Encoder, Field, Fields, Item, Kind, Limits, Value};
use std::{collections::BTreeSet, fmt};

pub const COMPONENT: u16 = 25;
pub const GET_LISTS: u16 = 6;

/// Native ListStatusFlags masks. Other bits are retained in the u32 field.
pub const SUBSCRIBED: u32 = 1;
pub const ROLLOVER: u32 = 2;
pub const MUTUAL_ACTION: u32 = 4;
pub const PAIRED: u32 = 8;
pub const OFFLINE_UED: u32 = 16;

schema!(ListIdentification {
    list_name: &'a [u8] => [0xb2, 0xeb, 0x40],
    list_type: u16 => [0xd3, 0x9c, 0x25],
});

schema!(ListInfo {
    blaze_object_id: ObjectId => [0x8a, 0xfa, 0x64],
    status_flags: u32 => [0x9a, 0xc9, 0xf3],
    id: ListIdentification<'a> => [0xb2, 0x99, 0x00],
    max_size: u32 => [0xb2, 0xdc, 0xc0],
    pair_name: &'a [u8] => [0xc2, 0xe8, 0x6d],
    pair_id: u16 => [0xc3, 0x2a, 0x64],
    pair_max_size: u32 => [0xc3, 0x2b, 0x73],
});

// MEML is deliberately outside FIELD_TAGS. The captured replies omit it;
// preserving it as an unknown is not semantic validation of ListMemberInfo.
schema!(ListMembers {
    info: ListInfo<'a> => [0xa6, 0xe9, 0xaf],
    offset: u32 => [0xbe, 0x6c, 0xa3],
    total_count: u32 => [0xd2, 0xf8, 0xf4],
});

macro_rules! struct_vector {
    ($name:ident, $element:ident) => {
        #[derive(Default)]
        pub struct $name<'a>(pub Vec<$element<'a>>);

        impl fmt::Debug for $name<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_tuple(concat!(stringify!($name), "Length"))
                    .field(&self.0.len())
                    .finish()
            }
        }

        impl<'a> Wire<'a> for $name<'a> {
            const KIND: Kind = Kind::List;

            fn read(item: Item<'a>) -> Result<Self, Error> {
                if !matches!(
                    item.value(),
                    Value::List {
                        element: Kind::Struct,
                        ..
                    }
                ) {
                    return Err(Error::WrongType { tag: None });
                }
                let values = item
                    .elements()
                    .ok_or(Error::WrongType { tag: None })?
                    .map(|item| $element::read(item?))
                    .collect::<Result<_, _>>()?;
                Ok(Self(values))
            }

            fn write(&self, tag: [u8; 3], w: &mut Encoder) -> Result<(), nfs_heat2::Error> {
                w.struct_list(tag, &self.0, |w, value| value.write_fields(w))
            }

            fn validate(&self, budget: &mut crate::Budget) -> Result<(), Error> {
                budget.take(1)?;
                // Each element accounts for its own struct value and fields below.
                budget.collection(self.0.len(), 0)?;
                for value in &self.0 {
                    value.validate(budget)?;
                }
                Ok(())
            }

            fn unknown_count(&self) -> usize {
                self.0.iter().map($element::unknown_field_count).sum()
            }
        }
    };
}

struct_vector!(ListInfoVector, ListInfo);
struct_vector!(ListMembersVector, ListMembers);

schema!(GetListsRequest {
    lists: ListInfoVector<'a> => [0x86, 0xcc, 0xf4],
    max_result_count: u32 => [0xb7, 0x8c, 0xa3],
    offset: u32 => [0xbe, 0x6c, 0xa3],
});

schema!(Lists {
    lists: ListMembersVector<'a> => [0xb2, 0xd8, 0x70],
});

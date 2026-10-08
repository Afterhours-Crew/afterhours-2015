// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

#[cfg(test)]
mod tests {
    use nfs_protocol::world::admission::{
        Decoded, Message,
        type_zero::{Client5, Error, Host6},
    };
    fn c5() -> Client5 {
        Client5 {
            peer_echo: 0x12345678,
            client_fresh: 0xabcdef09,
            client_selector: 3,
            callback: vec![],
        }
    }
    fn decoded_c5() -> Decoded {
        Decoded::decode(&c5().new_body(7).unwrap()).unwrap()
    }
    #[test]
    fn independent_inner_vector_has_be_widths() {
        let decoded = Decoded::new(Message::Transformed {
            kind: 5,
            selector: 7,
            opaque: vec![0x12, 0x34, 0x56, 0x78, 0xab, 0xcd, 0xef, 0x09, 0, 3, 0],
        })
        .unwrap();
        assert_eq!(Client5::decode(&decoded).unwrap(), c5());
        assert_eq!(decoded.encode().unwrap().len(), 16);
    }
    #[test]
    fn source_qualified_empty_profile_and_binding() {
        let decoded = decoded_c5();
        let fields = Client5::decode(&decoded).unwrap();
        assert_eq!(fields.qualify_empty(&decoded, 7, 0x12345678), Ok(()));
        assert_eq!(
            fields.qualify_empty(&decoded, 6, 0x12345678),
            Err(Error::Binding)
        );
        assert_eq!(fields.qualify_empty(&decoded, 7, 0), Err(Error::Binding));
        let mut foreign_fields = fields.clone();
        foreign_fields.client_fresh ^= 1;
        assert_eq!(
            foreign_fields.qualify_empty(&decoded, 7, fields.peer_echo),
            Err(Error::Binding)
        );
    }
    #[test]
    fn directional_selectors_remain_separate() {
        let fields = c5();
        let body = Host6 {
            client_echo: fields.client_fresh,
            host_fresh: 0x13579bdf,
        }
        .new_body(fields.client_selector)
        .unwrap();
        let decoded = Decoded::decode(&body).unwrap();
        assert_eq!(body.len(), 13);
        assert!(matches!(
            decoded.message,
            Message::Transformed {
                kind: 6,
                selector: 3,
                ..
            }
        ));
        assert_eq!(
            Host6::decode(&decoded).unwrap().client_echo,
            fields.client_fresh
        );
    }
    #[test]
    fn preserved_callback_is_not_eligible_empty_profile() {
        let mut fields = c5();
        fields.callback = vec![1, 2, 3];
        let decoded = Decoded::decode(&fields.new_body(7).unwrap()).unwrap();
        assert_eq!(Client5::decode(&decoded).unwrap(), fields);
        assert_eq!(
            fields.qualify_empty(&decoded, 7, fields.peer_echo),
            Err(Error::Profile)
        );
    }
    #[test]
    fn full_width_client_selector_is_not_silently_normalized() {
        for selector in [0, 1 << 14, u16::MAX] {
            let mut fields = c5();
            fields.client_selector = selector;
            let decoded = Decoded::decode(&fields.new_body(7).unwrap()).unwrap();
            assert_eq!(Client5::decode(&decoded).unwrap().client_selector, selector);
            assert_eq!(
                fields.qualify_empty(&decoded, 7, fields.peer_echo),
                Err(Error::Profile)
            );
        }
    }
    #[test]
    fn every_inner_prefix_is_rejected_and_extra_data_is_rejected() {
        let inner = vec![0x12, 0x34, 0x56, 0x78, 0xab, 0xcd, 0xef, 0x09, 0, 3, 0];
        for len in 0..inner.len() {
            let d = Decoded::new(Message::Transformed {
                kind: 5,
                selector: 7,
                opaque: inner[..len].to_vec(),
            })
            .unwrap();
            assert_eq!(Client5::decode(&d), Err(Error::Length));
        }
        let mut extra = inner.clone();
        extra.push(0);
        let d = Decoded::new(Message::Transformed {
            kind: 5,
            selector: 7,
            opaque: extra,
        })
        .unwrap();
        assert_eq!(Client5::decode(&d), Err(Error::Length));
        for len in [0, 7, 9] {
            let d = Decoded::new(Message::Transformed {
                kind: 6,
                selector: 3,
                opaque: vec![0; len],
            })
            .unwrap();
            assert_eq!(Host6::decode(&d), Err(Error::Length));
        }
    }
    #[test]
    fn callback_capacity_follows_complete_outer_extent() {
        let mut fields = c5();
        fields.callback = vec![0x55; 244];
        assert!(fields.new_body(7).is_ok());
        fields.callback.push(0);
        assert_eq!(fields.new_body(7), Err(Error::Length));
    }
    #[test]
    fn kind_and_physical_padding_do_not_alias_fields() {
        let mut body = c5().new_body(7).unwrap();
        *body.last_mut().unwrap() |= 15;
        let decoded = Decoded::decode(&body).unwrap();
        assert_eq!(decoded.encode().unwrap(), body);
        assert_eq!(Client5::decode(&decoded).unwrap(), c5());
        assert_eq!(Host6::decode(&decoded), Err(Error::Kind));
    }
    #[test]
    fn debug_redacts_all_token_and_selector_values() {
        let c = format!("{:?}", c5());
        let h = format!(
            "{:?}",
            Host6 {
                client_echo: 0xabcdef09,
                host_fresh: 0x13579bdf
            }
        );
        for value in ["12345678", "abcdef09", "13579bdf", "2882400009"] {
            assert!(!c.contains(value));
            assert!(!h.contains(value));
        }
    }
}

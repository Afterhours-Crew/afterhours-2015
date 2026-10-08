//! Transport cryptography.
//!
//! The cipher is RC4 keyed with the 110-byte session key, with the first 3072
//! keystream bytes dropped. Datagram offsets advance in eight-byte units; the
//! wire cursor contains only their low 15 bits, so directional cipher state
//! continues across cursor rollover. The MAC uses 0x36/0x5c pads over a
//! 64-byte key using a MurmurHash3-x64-128-style digest, truncated to 12 bytes.
//! The 64-byte MAC key is the build's MAC template encrypted with the session cipher.

/// Keystream bytes discarded after key scheduling.
pub const RC4_DROP: usize = 3072;
/// Session key length: world UUID, '-', the two UUIDs sorted, joined by '-'.
pub const KEY_LEN: usize = 110;
/// Build-specific MAC template length.
pub const MAC_TEMPLATE_LEN: usize = 64;
/// Truncated MAC length carried in every datagram.
pub const MAC_LEN: usize = 12;

#[derive(Clone)]
pub struct Rc4 {
    s: [u8; 256],
    i: u8,
    j: u8,
}

impl Drop for Rc4 {
    fn drop(&mut self) {
        self.s.fill(0);
        self.i = 0;
        self.j = 0;
    }
}

impl Rc4 {
    /// Key-schedule `key` and discard `drop` keystream bytes.
    pub fn new(key: &[u8], drop: usize) -> Self {
        let mut out = Self {
            s: std::array::from_fn(|i| i as u8),
            i: 0,
            j: 0,
        };
        let mut j = 0u8;
        for i in 0..256 {
            j = j.wrapping_add(out.s[i]).wrapping_add(key[i % key.len()]);
            out.s.swap(i, usize::from(j));
        }
        out.skip(drop);
        out
    }

    fn next_byte(&mut self) -> u8 {
        self.i = self.i.wrapping_add(1);
        self.j = self.j.wrapping_add(self.s[usize::from(self.i)]);
        self.s.swap(usize::from(self.i), usize::from(self.j));
        self.s[usize::from(self.s[usize::from(self.i)].wrapping_add(self.s[usize::from(self.j)]))]
    }

    pub fn skip(&mut self, n: usize) {
        for _ in 0..n {
            self.next_byte();
        }
    }

    pub fn apply(&mut self, data: &[u8]) -> Vec<u8> {
        data.iter().map(|b| b ^ self.next_byte()).collect()
    }
}

fn fmix(mut v: u64) -> u64 {
    v ^= v >> 33;
    v = v.wrapping_mul(0xff51_afd7_ed55_8ccd);
    v ^= v >> 33;
    v = v.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    v ^ (v >> 33)
}

/// MurmurHash3-x64-128 variant with the game's seeds and final byte fold.
pub fn digest(data: &[u8]) -> [u8; 16] {
    let (mut h1, mut h2) = (0x6745_2301_efcd_ab89u64, 0x98ba_dcfe_1032_5476u64);
    let (c1, c2) = (0x87c3_7b91_1142_53d5u64, 0x4cf5_ad43_2745_937fu64);
    let stop = data.len() / 16 * 16;
    for block in data[..stop].as_chunks::<16>().0 {
        let k1 = u64::from_le_bytes(block[..8].try_into().expect("8 bytes"));
        let k2 = u64::from_le_bytes(block[8..].try_into().expect("8 bytes"));
        h1 ^= k1.wrapping_mul(c1).rotate_left(31).wrapping_mul(c2);
        h1 = h1
            .rotate_left(27)
            .wrapping_add(h2)
            .wrapping_mul(5)
            .wrapping_add(0x52dc_e729);
        h2 ^= k2.wrapping_mul(c2).rotate_left(33).wrapping_mul(c1);
        h2 = h2
            .rotate_left(31)
            .wrapping_add(h1)
            .wrapping_mul(5)
            .wrapping_add(0x3849_5ab5);
    }
    let tail = &data[stop..];
    if tail.len() > 8 {
        let mut word = [0; 8];
        word[..tail.len() - 8].copy_from_slice(&tail[8..]);
        h2 ^= u64::from_le_bytes(word)
            .wrapping_mul(c2)
            .rotate_left(33)
            .wrapping_mul(c1);
    }
    if !tail.is_empty() {
        let mut word = [0; 8];
        let n = tail.len().min(8);
        word[..n].copy_from_slice(&tail[..n]);
        h1 ^= u64::from_le_bytes(word)
            .wrapping_mul(c1)
            .rotate_left(31)
            .wrapping_mul(c2);
    }
    h1 ^= data.len() as u64;
    h2 ^= data.len() as u64;
    h1 = h1.wrapping_add(h2);
    h2 = h2.wrapping_add(h1);
    h1 = fmix(h1);
    h2 = fmix(h2);
    h1 = h1.wrapping_add(h2);
    h2 = h2.wrapping_add(h1);
    let mut out = [0; 16];
    out[..8].copy_from_slice(&h1.to_le_bytes());
    out[8..].copy_from_slice(&h2.to_le_bytes());
    out[7] = out[15];
    out
}

/// HMAC-style 12-byte tag over `data` with a 64-byte key.
pub fn mac(key: &[u8; MAC_TEMPLATE_LEN], data: &[u8]) -> [u8; MAC_LEN] {
    let mut inner = Vec::with_capacity(MAC_TEMPLATE_LEN + data.len());
    inner.extend(key.iter().map(|v| v ^ 0x36));
    inner.extend_from_slice(data);
    let mut outer = Vec::with_capacity(MAC_TEMPLATE_LEN + 16);
    outer.extend(key.iter().map(|v| v ^ 0x5c));
    outer.extend_from_slice(&digest(&inner));
    digest(&outer)[..MAC_LEN].try_into().expect("12 bytes")
}

/// The 110-byte session key: `world-<min(world,local)>-<max(world,local)>`.
/// Both inputs must be distinct lowercase UUID strings (36 bytes).
pub fn session_key(world_uuid: &str, local_uuid: &str) -> Option<[u8; KEY_LEN]> {
    fn uuid(text: &str) -> bool {
        text.len() == 36
            && text.bytes().enumerate().all(|(i, b)| {
                if [8, 13, 18, 23].contains(&i) {
                    b == b'-'
                } else {
                    b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
                }
            })
    }
    if !uuid(world_uuid) || !uuid(local_uuid) || world_uuid == local_uuid {
        return None;
    }
    let (first, second) = if world_uuid < local_uuid {
        (world_uuid, local_uuid)
    } else {
        (local_uuid, world_uuid)
    };
    let mut key = [0; KEY_LEN];
    key[..36].copy_from_slice(world_uuid.as_bytes());
    key[36] = b'-';
    key[37..73].copy_from_slice(first.as_bytes());
    key[73] = b'-';
    key[74..].copy_from_slice(second.as_bytes());
    Some(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rc4_matches_the_standard_vector() {
        // RFC 6229-style check with no drop: key "Key", plaintext "Plaintext".
        let mut c = Rc4::new(b"Key", 0);
        assert_eq!(
            c.apply(b"Plaintext"),
            [0xbb, 0xf3, 0x16, 0xe8, 0xd9, 0x40, 0xaf, 0x0a, 0xd3]
        );
    }

    #[test]
    fn rc4_is_symmetric_and_drop_shifts_the_stream() {
        let data = b"the same bytes";
        let encrypted = Rc4::new(b"k", RC4_DROP).apply(data);
        assert_eq!(Rc4::new(b"k", RC4_DROP).apply(&encrypted), data);
        let mut skipped = Rc4::new(b"k", 0);
        skipped.skip(RC4_DROP);
        assert_eq!(skipped.apply(data), encrypted);
    }

    #[test]
    fn digest_depends_on_every_byte_and_length() {
        let a = digest(b"0123456789abcdef0123");
        assert_ne!(a, digest(b"0123456789abcdef0124"));
        assert_ne!(a, digest(b"0123456789abcdef012"));
        assert_eq!(a[7], a[15], "byte 7 folds byte 15");
    }

    #[test]
    fn mac_changes_with_key_and_data() {
        let k = [7u8; 64];
        let t = mac(&k, b"message");
        assert_ne!(t, mac(&[8u8; 64], b"message"));
        assert_ne!(t, mac(&k, b"messagf"));
        assert_eq!(t, mac(&k, b"message"));
    }

    #[test]
    fn session_key_sorts_the_two_uuids_after_the_world_uuid() {
        let w = "bbbbbbbb-0000-0000-0000-000000000000";
        let l = "aaaaaaaa-0000-0000-0000-000000000000";
        let key = session_key(w, l).unwrap();
        assert_eq!(&key[..36], w.as_bytes());
        assert_eq!(&key[37..73], l.as_bytes());
        assert_eq!(&key[74..], w.as_bytes());
        assert_eq!(key[36], b'-');
        assert!(session_key(w, w).is_none());
        assert!(session_key("not-a-uuid", l).is_none());
        assert!(session_key(&w.to_uppercase(), l).is_none());
    }
}

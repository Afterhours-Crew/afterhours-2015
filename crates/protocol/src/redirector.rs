//! XML2 codecs, independent of HTTP and service policy.
//!
//! Only the `ServerAddress::ipAddress` alternative is supported. All response
//! fields are explicit; no endpoint, certificate, success policy or default
//! application state is supplied. This is fixture-tested, not game-validated.
//! Unlike the Heat2 adapters, XML text is UTF-8 with strict XML 1.0 characters.

use std::fmt;

pub mod request;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    pub max_output_bytes: usize,
    pub max_string_bytes: usize,
    pub max_blob_bytes: usize,
    pub max_list_items: usize,
    pub max_total_items: usize,
}

impl Default for Limits {
    fn default() -> Self {
        // Replacement resource policy, not observed client maxima.
        Self {
            max_output_bytes: 1024 * 1024,
            max_string_bytes: 64 * 1024,
            max_blob_bytes: 256 * 1024,
            max_list_items: 1024,
            max_total_items: 2048,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    OutputLimit,
    StringLimit,
    BlobLimit,
    CollectionLimit,
    InvalidXmlCharacter,
    AllocationFailed,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "redirector XML {self:?}")
    }
}
impl std::error::Error for Error {}

// Payloads deliberately do not implement Debug: they can contain hostnames,
// service names, certificate material and messages supplied by callers.
#[derive(Clone, Copy)]
pub struct IpAddress<'a> {
    pub hostname: &'a str,
    /// IPv4 numeric value, emitted as decimal (e.g. 127.0.0.1 = 2130706433).
    pub ip: u32,
    pub port: u16,
}

#[derive(Clone, Copy)]
pub struct AddressRemapEntry {
    pub dst_port: u16,
    pub net_mask: u32,
    pub service_id: u32,
    pub src_ip: u32,
    pub src_port: u16,
}

#[derive(Clone, Copy)]
pub struct NameRemapEntry<'a> {
    pub dst_port: u16,
    pub service_id: u32,
    pub hostname: &'a str,
    pub site_name: &'a str,
    pub src_port: u16,
}

#[derive(Clone, Copy)]
pub struct ServerInstanceInfo<'a> {
    pub address: IpAddress<'a>,
    pub address_remaps: &'a [AddressRemapEntry],
    /// Opaque bytes; this codec neither validates nor trusts certificates.
    pub certificate_list: &'a [&'a [u8]],
    pub messages: &'a [&'a str],
    pub name_remaps: &'a [NameRemapEntry<'a>],
    pub secure: bool,
    pub trial_service_name: &'a str,
    pub default_dns_address: u32,
}

impl ServerInstanceInfo<'_> {
    /// Encode the code-derived default XML2 shape, ending in one LF.
    ///
    /// Validate/count before allocating, then emit exactly that many bytes.
    /// No partial output escapes on error. See [`request`] for body decoding;
    /// stream framing and HTTP status remain outside scope.
    pub fn encode_xml(&self, limits: Limits) -> Result<Vec<u8>, Error> {
        let mut total_items = 0usize;
        for count in [
            self.address_remaps.len(),
            self.certificate_list.len(),
            self.messages.len(),
            self.name_remaps.len(),
        ] {
            total_items = total_items
                .checked_add(count)
                .ok_or(Error::CollectionLimit)?;
            if count > limits.max_list_items || total_items > limits.max_total_items {
                return Err(Error::CollectionLimit);
            }
        }
        let mut counter = Writer {
            output: None,
            length: 0,
            limits,
        };
        self.write(&mut counter)?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(counter.length)
            .map_err(|_| Error::AllocationFailed)?;
        let mut writer = Writer {
            output: Some(output),
            length: 0,
            limits,
        };
        self.write(&mut writer)?;
        // The second pass sees the same immutable input and limits.
        Ok(writer.output.unwrap_or_default())
    }

    fn write(&self, w: &mut Writer) -> Result<(), Error> {
        w.raw(b"<serverinstanceinfo><address member=\"0\"><valu>")?;
        w.text("hostname", self.address.hostname)?;
        w.number("ip", self.address.ip)?;
        w.number("port", u32::from(self.address.port))?;
        w.raw(b"</valu></address>")?;
        if !self.address_remaps.is_empty() {
            w.open("addressremaps")?;
            for entry in self.address_remaps {
                w.open("addressremapentry")?;
                w.number("dstport", u32::from(entry.dst_port))?;
                w.number("netmask", entry.net_mask)?;
                w.number("serviceid", entry.service_id)?;
                w.number("srcip", entry.src_ip)?;
                w.number("srcport", u32::from(entry.src_port))?;
                w.close("addressremapentry")?;
            }
            w.close("addressremaps")?;
        }
        if !self.certificate_list.is_empty() {
            w.open("certificatelist")?;
            for blob in self.certificate_list {
                w.blob(blob)?;
            }
            w.close("certificatelist")?;
        }
        if !self.messages.is_empty() {
            w.open("messages")?;
            for message in self.messages {
                w.text("messages", message)?;
            }
            w.close("messages")?;
        }
        if !self.name_remaps.is_empty() {
            w.open("nameremaps")?;
            for entry in self.name_remaps {
                w.open("nameremapentry")?;
                w.number("dstport", u32::from(entry.dst_port))?;
                w.number("serviceid", entry.service_id)?;
                w.text("hostname", entry.hostname)?;
                w.text("sitename", entry.site_name)?;
                w.number("srcport", u32::from(entry.src_port))?;
                w.close("nameremapentry")?;
            }
            w.close("nameremaps")?;
        }
        w.number("secure", u32::from(self.secure))?;
        w.text("trialservicename", self.trial_service_name)?;
        w.number("defaultdnsaddress", self.default_dns_address)?;
        w.raw(b"</serverinstanceinfo>\n")
    }
}

struct Writer {
    output: Option<Vec<u8>>,
    length: usize,
    limits: Limits,
}

impl Writer {
    fn raw(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.length = self
            .length
            .checked_add(bytes.len())
            .ok_or(Error::OutputLimit)?;
        if self.length > self.limits.max_output_bytes {
            return Err(Error::OutputLimit);
        }
        if let Some(output) = &mut self.output {
            output.extend_from_slice(bytes);
        }
        Ok(())
    }

    // Names are exclusively static schema names, never caller-controlled XML.
    fn open(&mut self, name: &'static str) -> Result<(), Error> {
        self.raw(b"<")?;
        self.raw(name.as_bytes())?;
        self.raw(b">")
    }

    fn close(&mut self, name: &'static str) -> Result<(), Error> {
        self.raw(b"</")?;
        self.raw(name.as_bytes())?;
        self.raw(b">")
    }

    fn number(&mut self, name: &'static str, value: u32) -> Result<(), Error> {
        self.open(name)?;
        self.raw(value.to_string().as_bytes())?;
        self.close(name)
    }

    fn text(&mut self, name: &'static str, value: &str) -> Result<(), Error> {
        if value.len() > self.limits.max_string_bytes {
            return Err(Error::StringLimit);
        }
        self.open(name)?;
        for c in value.chars() {
            match c {
                '&' => self.raw(b"&amp;")?,
                '<' => self.raw(b"&lt;")?,
                '>' => self.raw(b"&gt;")?,
                '\'' => self.raw(b"&apos;")?,
                '"' => self.raw(b"&quot;")?,
                '\t'
                | '\n'
                | '\r'
                | '\u{20}'..='\u{d7ff}'
                | '\u{e000}'..='\u{fffd}'
                | '\u{10000}'..='\u{10ffff}' => {
                    self.raw(c.encode_utf8(&mut [0; 4]).as_bytes())?;
                }
                _ => return Err(Error::InvalidXmlCharacter),
            }
        }
        self.close(name)
    }

    fn blob(&mut self, blob: &[u8]) -> Result<(), Error> {
        if blob.len() > self.limits.max_blob_bytes {
            return Err(Error::BlobLimit);
        }
        let count = blob
            .len()
            .div_ceil(3)
            .checked_mul(4)
            .ok_or(Error::OutputLimit)?;
        self.raw(b"<certificatelist count=\"")?;
        self.raw(count.to_string().as_bytes())?;
        self.raw(b"\" enc=\"base64\">")?;
        // Preflight the expanded content before traversing any blob bytes.
        if count > self.limits.max_output_bytes.saturating_sub(self.length) {
            return Err(Error::OutputLimit);
        }
        const ALPHABET: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        for chunk in blob.chunks(3) {
            let a = chunk[0];
            let b = chunk.get(1).copied().unwrap_or(0);
            let c = chunk.get(2).copied().unwrap_or(0);
            self.raw(&[
                ALPHABET[usize::from(a >> 2)],
                ALPHABET[usize::from(((a & 3) << 4) | (b >> 4))],
                if chunk.len() > 1 {
                    ALPHABET[usize::from(((b & 15) << 2) | (c >> 6))]
                } else {
                    b'='
                },
                if chunk.len() > 2 {
                    ALPHABET[usize::from(c & 63)]
                } else {
                    b'='
                },
            ])?;
        }
        self.close("certificatelist")
    }
}

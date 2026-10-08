// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Redirector request-body subset. Not a service validator.
//!
//! All schema fields are optional: absence is not replaced with client constructor
//! defaults. Unknown child elements retain their exact XML in input order. A
//! present FirstPartyId is explicitly unsupported, including an empty element.
//! Strict decimal/range, duplicate, root and XML-subset checks are replacement
//! policies, not claims about the original client's permissive failure behavior.

use quick_xml::{
    Reader,
    events::{BytesStart, Event},
};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    pub max_input_bytes: usize,
    pub max_depth: usize,
    pub max_elements: usize,
    pub max_events: usize,
    pub max_attributes_per_element: usize,
    pub max_name_bytes: usize,
    pub max_string_bytes: usize,
    pub max_unknown_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_input_bytes: 1024 * 1024,
            max_depth: 16,
            max_elements: 2048,
            max_events: 8192,
            max_attributes_per_element: 16,
            max_name_bytes: 128,
            max_string_bytes: 64 * 1024,
            max_unknown_bytes: 256 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InputLimit,
    DepthLimit,
    ElementLimit,
    EventLimit,
    AttributeLimit,
    NameLimit,
    StringLimit,
    UnknownLimit,
    AllocationFailed,
    InvalidXml,
    UnsupportedXml,
    WrongRoot,
    DuplicateField,
    InvalidValue,
    UnsupportedFirstPartyId,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "redirector request {self:?}")
    }
}
impl std::error::Error for Error {}

/// A recognized enum value. Numeric values outside the domain are rejected.
/// The static symbolic name is retained even when the XML used decimal text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EnumValue {
    pub value: i32,
    pub name: &'static str,
}

// No Debug on payloads: names, profile data and unknown XML may be private.
pub struct UnknownField<'a> {
    pub name: String,
    pub xml: &'a str,
}

#[derive(Default)]
pub struct ServerInstanceRequest<'a> {
    pub blaze_sdk_version: Option<String>,
    pub blaze_sdk_build_date: Option<String>,
    pub client_name: Option<String>,
    pub client_type: Option<EnumValue>,
    pub client_platform: Option<EnumValue>,
    pub client_sku_id: Option<String>,
    pub client_version: Option<String>,
    pub dirty_sdk_version: Option<String>,
    pub environment: Option<String>,
    pub client_locale: Option<u32>,
    pub name: Option<String>,
    pub platform: Option<String>,
    pub connection_profile: Option<String>,
    pub is_trial: Option<bool>,
    pub unknown: Vec<UnknownField<'a>>,
}

const FIELDS: [&str; 15] = [
    "blazesdkversion",
    "blazesdkbuilddate",
    "clientname",
    "clienttype",
    "clientplatform",
    "clientskuid",
    "clientversion",
    "dirtysdkversion",
    "environment",
    "firstpartyid",
    "clientlocale",
    "name",
    "platform",
    "connectionprofile",
    "istrial",
];
const CLIENT_TYPES: [&str; 6] = [
    "CLIENT_TYPE_GAMEPLAY_USER",
    "CLIENT_TYPE_HTTP_USER",
    "CLIENT_TYPE_DEDICATED_SERVER",
    "CLIENT_TYPE_TOOLS",
    "CLIENT_TYPE_LIMITED_GAMEPLAY_USER",
    "CLIENT_TYPE_INVALID",
];
const PLATFORMS: [&str; 21] = [
    "INVALID",
    "xbl2",
    "ps3",
    "wii",
    "pc",
    "android",
    "ios",
    "qnx",
    "common",
    "mobile",
    "legacyprofileid",
    "verizon",
    "facebook",
    "facebook_eacom",
    "bebo",
    "friendster",
    "twitter",
    "wiiu",
    "vita",
    "xone",
    "ps4",
];

struct Field {
    id: Option<usize>,
    start: usize,
    name: String,
    text: String,
}

impl<'a> ServerInstanceRequest<'a> {
    /// Decode one complete body, not a byte stream. Truncated or concatenated
    /// documents are errors. No network, entity resolver or global state is used.
    ///
    /// The supported subset uses ASCII non-namespaced element/attribute names,
    /// XML 1.0 characters and UTF-8. Comments, CDATA and the initial XML 1.0
    /// declaration with explicit UTF-8 encoding are accepted. Other declaration
    /// forms, processing instructions, DTDs and non-predefined entities are rejected.
    /// Root/known scalar attributes and nested content in scalars are rejected.
    pub fn decode_xml(input: &'a [u8], limits: Limits) -> Result<Self, Error> {
        if input.len() > limits.max_input_bytes {
            return Err(Error::InputLimit);
        }
        let input = std::str::from_utf8(input).map_err(|_| Error::InvalidXml)?;
        valid_chars(input)?;
        let mut reader = Reader::from_str(input);
        reader.config_mut().check_comments = true;
        let mut result = Self::default();
        let mut depth = 0usize;
        let mut elements = 0usize;
        let mut events = 0usize;
        let mut root_seen = false;
        let mut current: Option<Field> = None;
        let mut seen = [false; 15];
        let mut unknown_bytes = 0usize;

        loop {
            let start = usize::try_from(reader.buffer_position()).map_err(|_| Error::InputLimit)?;
            let event = reader.read_event().map_err(|_| Error::InvalidXml)?;
            if matches!(event, Event::Eof) {
                break;
            }
            events = events.checked_add(1).ok_or(Error::EventLimit)?;
            if events > limits.max_events {
                return Err(Error::EventLimit);
            }
            let empty = matches!(event, Event::Empty(_));
            match event {
                Event::Start(e) | Event::Empty(e) => {
                    if depth >= limits.max_depth {
                        return Err(Error::DepthLimit);
                    }
                    elements = elements.checked_add(1).ok_or(Error::ElementLimit)?;
                    if elements > limits.max_elements {
                        return Err(Error::ElementLimit);
                    }
                    let name = e.name();
                    valid_name(name.0, limits)?;
                    let attributes = validate_attributes(&e, limits)?;
                    match depth {
                        0 => {
                            if root_seen {
                                return Err(Error::InvalidXml);
                            }
                            if !name.0.eq_ignore_ascii_case("serverinstancerequest") {
                                return Err(Error::WrongRoot);
                            }
                            if attributes != 0 {
                                return Err(Error::UnsupportedXml);
                            }
                            root_seen = true;
                        }
                        1 => {
                            let id = FIELDS.iter().position(|n| name.0.eq_ignore_ascii_case(n));
                            if let Some(id) = id {
                                if id == 9 {
                                    return Err(Error::UnsupportedFirstPartyId);
                                }
                                if seen[id] {
                                    return Err(Error::DuplicateField);
                                }
                                seen[id] = true;
                                if attributes != 0 {
                                    return Err(Error::UnsupportedXml);
                                }
                            }
                            current = Some(Field {
                                id,
                                start,
                                name: copy_string(name.0)?,
                                text: String::new(),
                            });
                        }
                        _ if current.as_ref().is_some_and(|f| f.id.is_some()) => {
                            return Err(Error::InvalidValue);
                        }
                        _ => {}
                    }
                    depth += 1;
                    if empty {
                        if depth == 2 {
                            let end = usize::try_from(reader.buffer_position())
                                .map_err(|_| Error::InputLimit)?;
                            result.finish(
                                current.take().ok_or(Error::InvalidXml)?,
                                input,
                                end,
                                limits,
                                &mut unknown_bytes,
                            )?;
                        }
                        depth -= 1;
                    }
                }
                Event::End(_) => {
                    if depth == 0 {
                        return Err(Error::InvalidXml);
                    }
                    if depth == 2 {
                        let end = usize::try_from(reader.buffer_position())
                            .map_err(|_| Error::InputLimit)?;
                        result.finish(
                            current.take().ok_or(Error::InvalidXml)?,
                            input,
                            end,
                            limits,
                            &mut unknown_bytes,
                        )?;
                    }
                    depth -= 1;
                }
                Event::Text(e) => {
                    if e.contains("]]>") {
                        return Err(Error::InvalidXml);
                    }
                    let value = e.xml10_content();
                    append_text(&mut current, &value, depth, limits)?;
                }
                Event::CData(e) => {
                    if depth < 2 {
                        return Err(Error::InvalidXml);
                    }
                    append_text(&mut current, &e.xml10_content(), depth, limits)?;
                }
                Event::GeneralRef(e) => {
                    if depth < 2 {
                        return Err(Error::InvalidXml);
                    }
                    let c = match e.resolve_char_ref().map_err(|_| Error::InvalidXml)? {
                        Some(c) => c,
                        None => match e.as_ref() {
                            "amp" => '&',
                            "lt" => '<',
                            "gt" => '>',
                            "apos" => '\'',
                            "quot" => '"',
                            _ => return Err(Error::UnsupportedXml),
                        },
                    };
                    append_text(&mut current, c.encode_utf8(&mut [0; 4]), depth, limits)?;
                }
                Event::Comment(_) => {}
                Event::Decl(e) => {
                    // Require byte-zero placement and exactly one initial declaration.
                    // A declaration is not a processing instruction or field value.
                    if start != 0 || events != 1 || root_seen || !input.starts_with("<?xml") {
                        return Err(Error::UnsupportedXml);
                    }
                    let declaration = BytesStart::from_content(e.as_ref(), 3);
                    valid_name(declaration.name().0, limits)?;
                    if validate_attributes(&declaration, limits)? != 2 {
                        return Err(Error::UnsupportedXml);
                    }
                    for (attribute, (name, value)) in declaration
                        .attributes()
                        .zip([("version", "1.0"), ("encoding", "UTF-8")])
                    {
                        let attribute = attribute.map_err(|_| Error::InvalidXml)?;
                        if attribute.key.0 != name || !attribute.value.eq_ignore_ascii_case(value) {
                            return Err(Error::UnsupportedXml);
                        }
                    }
                }
                Event::PI(_) | Event::DocType(_) => {
                    return Err(Error::UnsupportedXml);
                }
                Event::Eof => unreachable!(),
            }
        }
        if !root_seen || depth != 0 {
            return Err(Error::InvalidXml);
        }
        Ok(result)
    }

    fn finish(
        &mut self,
        field: Field,
        input: &'a str,
        end: usize,
        limits: Limits,
        unknown_bytes: &mut usize,
    ) -> Result<(), Error> {
        let text = field.text;
        match field.id {
            Some(0) => self.blaze_sdk_version = Some(text),
            Some(1) => self.blaze_sdk_build_date = Some(text),
            Some(2) => self.client_name = Some(text),
            Some(3) => self.client_type = Some(parse_enum(&text, false)?),
            Some(4) => self.client_platform = Some(parse_enum(&text, true)?),
            Some(5) => self.client_sku_id = Some(text),
            Some(6) => self.client_version = Some(text),
            Some(7) => self.dirty_sdk_version = Some(text),
            Some(8) => self.environment = Some(text),
            Some(10) => self.client_locale = Some(decimal(&text)?),
            Some(11) => self.name = Some(text),
            Some(12) => self.platform = Some(text),
            Some(13) => self.connection_profile = Some(text),
            Some(14) => {
                self.is_trial = Some(match text.as_str() {
                    "1" | "true" => true,
                    "0" | "false" => false,
                    _ => return Err(Error::InvalidValue),
                })
            }
            Some(_) => return Err(Error::UnsupportedFirstPartyId),
            None => {
                let xml = input.get(field.start..end).ok_or(Error::InvalidXml)?;
                *unknown_bytes = unknown_bytes
                    .checked_add(xml.len())
                    .ok_or(Error::UnknownLimit)?;
                if *unknown_bytes > limits.max_unknown_bytes {
                    return Err(Error::UnknownLimit);
                }
                self.unknown
                    .try_reserve(1)
                    .map_err(|_| Error::AllocationFailed)?;
                self.unknown.push(UnknownField {
                    name: field.name,
                    xml,
                });
            }
        }
        Ok(())
    }
}

fn parse_enum(text: &str, platform: bool) -> Result<EnumValue, Error> {
    // Canonical symbolic spelling only; no unverified CRT case-fold behavior.
    let names: &[&str] = if platform { &PLATFORMS } else { &CLIENT_TYPES };
    if platform && text == "NATIVE" {
        return Ok(EnumValue {
            value: 65535,
            name: "NATIVE",
        });
    }
    if let Some(index) = names.iter().position(|n| *n == text) {
        return Ok(EnumValue {
            value: index as i32,
            name: names[index],
        });
    }
    let value = decimal(text)?;
    if platform && value == 65535 {
        return Ok(EnumValue {
            value: 65535,
            name: "NATIVE",
        });
    }
    let name = *names.get(value as usize).ok_or(Error::InvalidValue)?;
    Ok(EnumValue {
        value: value as i32,
        name,
    })
}

fn decimal(text: &str) -> Result<u32, Error> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Error::InvalidValue);
    }
    text.parse().map_err(|_| Error::InvalidValue)
}

fn valid_chars(text: &str) -> Result<(), Error> {
    if text.chars().all(|c| matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}')) {
        Ok(())
    } else { Err(Error::InvalidXml) }
}

fn valid_name(name: &str, limits: Limits) -> Result<(), Error> {
    if name.len() > limits.max_name_bytes {
        return Err(Error::NameLimit);
    }
    let mut bytes = name.bytes();
    if !bytes
        .next()
        .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
        || !bytes.all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
    {
        return Err(Error::UnsupportedXml);
    }
    Ok(())
}

fn validate_attributes(element: &BytesStart<'_>, limits: Limits) -> Result<usize, Error> {
    // quick-xml's attribute iterator accepts adjacent quoted attributes without
    // intervening whitespace. Require XML separators before using that iterator.
    let mut quote = None;
    let mut needs_separator = false;
    for byte in element.attributes_raw().bytes() {
        if let Some(end) = quote {
            if byte == end {
                quote = None;
                needs_separator = true;
            }
        } else if needs_separator {
            if !matches!(byte, b' ' | b'\t' | b'\r' | b'\n') {
                return Err(Error::InvalidXml);
            }
            needs_separator = false;
        } else if matches!(byte, b'\'' | b'"') {
            quote = Some(byte);
        }
    }
    let mut count = 0;
    for attribute in element.attributes() {
        count += 1;
        if count > limits.max_attributes_per_element {
            return Err(Error::AttributeLimit);
        }
        let attribute = attribute.map_err(|_| Error::InvalidXml)?;
        valid_name(attribute.key.0, limits)?;
        if attribute.key.0 == "xmlns" {
            return Err(Error::UnsupportedXml);
        }
        if attribute.value.len() > limits.max_string_bytes {
            return Err(Error::StringLimit);
        }
        if attribute.value.contains('<') {
            return Err(Error::InvalidXml);
        }
        let value = attribute
            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
            .map_err(|_| Error::InvalidXml)?;
        valid_chars(&value)?;
    }
    Ok(count)
}

fn copy_string(text: &str) -> Result<String, Error> {
    let mut value = String::new();
    value
        .try_reserve_exact(text.len())
        .map_err(|_| Error::AllocationFailed)?;
    value.push_str(text);
    Ok(value)
}

fn append_text(
    field: &mut Option<Field>,
    text: &str,
    depth: usize,
    limits: Limits,
) -> Result<(), Error> {
    valid_chars(text)?;
    if depth < 2 {
        return if text
            .bytes()
            .all(|b| matches!(b, b' ' | b'\t' | b'\r' | b'\n'))
        {
            Ok(())
        } else {
            Err(Error::InvalidXml)
        };
    }
    if let Some(field) = field.as_mut().filter(|f| f.id.is_some()) {
        let size = field
            .text
            .len()
            .checked_add(text.len())
            .ok_or(Error::StringLimit)?;
        if size > limits.max_string_bytes {
            return Err(Error::StringLimit);
        }
        field
            .text
            .try_reserve(text.len())
            .map_err(|_| Error::AllocationFailed)?;
        field.text.push_str(text);
    }
    Ok(())
}

//! A small, bounded XML reader and writer for the LSX envelopes: elements,
//! attributes, text and the standard five entities. No namespaces processing
//! (names are kept verbatim, so an `lsx:` prefix survives as part of the
//! name), no DTD, no processing instructions beyond skipping the prolog.
//!
//! The game's own reader selects children by exact name, attributes
//! by exact name and reads attribute values or node text; this reader keeps
//! the same distinctions.

use std::fmt;

pub const MAX_DEPTH: usize = 16;
pub const MAX_NODES: usize = 1024;
pub const MAX_INPUT: usize = 1 << 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Bound,
    /// Not well formed at this byte offset.
    Malformed(usize),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "lsx xml {self:?}")
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Element {
    pub name: String,
    pub attributes: Vec<(String, String)>,
    pub children: Vec<Element>,
    /// Character data directly inside this element (entities decoded,
    /// concatenated across child elements).
    pub text: String,
}

impl Element {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            ..Self::default()
        }
    }

    pub fn attribute(mut self, name: &str, value: &str) -> Self {
        self.attributes.push((name.to_owned(), value.to_owned()));
        self
    }

    pub fn child(mut self, child: Element) -> Self {
        self.children.push(child);
        self
    }

    pub fn text(mut self, text: &str) -> Self {
        self.text = text.to_owned();
        self
    }

    /// The value of the first attribute with this exact name.
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    /// The first child with this exact name.
    pub fn first(&self, name: &str) -> Option<&Element> {
        self.children.iter().find(|c| c.name == name)
    }

    /// Serialize as a document fragment (no prolog), minimal whitespace.
    pub fn to_xml(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }

    fn write(&self, out: &mut String) {
        out.push('<');
        out.push_str(&self.name);
        for (name, value) in &self.attributes {
            out.push(' ');
            out.push_str(name);
            out.push_str("=\"");
            escape(value, true, out);
            out.push('"');
        }
        if self.children.is_empty() && self.text.is_empty() {
            out.push_str("/>");
            return;
        }
        out.push('>');
        escape(&self.text, false, out);
        for child in &self.children {
            child.write(out);
        }
        out.push_str("</");
        out.push_str(&self.name);
        out.push('>');
    }
}

fn escape(value: &str, attribute: bool, out: &mut String) {
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if attribute => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
}

/// Parse one document: an optional `<?xml...?>` prolog, then exactly one
/// root element, then optional whitespace.
pub fn parse(input: &[u8]) -> Result<Element, Error> {
    if input.len() > MAX_INPUT {
        return Err(Error::Bound);
    }
    let text = std::str::from_utf8(input).map_err(|e| Error::Malformed(e.valid_up_to()))?;
    let mut p = Parser {
        s: text.as_bytes(),
        pos: 0,
        nodes: 0,
    };
    p.skip_space();
    while p.s[p.pos..].starts_with(b"<?") {
        let end = find(p.s, p.pos, b"?>").ok_or(Error::Malformed(p.pos))?;
        p.pos = end + 2;
        p.skip_space();
    }
    let root = p.element(0)?;
    p.skip_space();
    if p.pos != p.s.len() {
        return Err(Error::Malformed(p.pos));
    }
    Ok(root)
}

struct Parser<'a> {
    s: &'a [u8],
    pos: usize,
    nodes: usize,
}

fn find(s: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    (from..s.len().saturating_sub(needle.len() - 1)).find(|&i| s[i..].starts_with(needle))
}

fn is_name(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b':')
}

impl Parser<'_> {
    fn skip_space(&mut self) {
        while self.pos < self.s.len() && self.s[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn name(&mut self) -> Result<String, Error> {
        let start = self.pos;
        while self.pos < self.s.len() && is_name(self.s[self.pos]) {
            self.pos += 1;
        }
        if start == self.pos {
            return Err(Error::Malformed(self.pos));
        }
        Ok(String::from_utf8_lossy(&self.s[start..self.pos]).into_owned())
    }

    fn element(&mut self, depth: usize) -> Result<Element, Error> {
        if depth >= MAX_DEPTH || self.nodes >= MAX_NODES {
            return Err(Error::Bound);
        }
        self.nodes += 1;
        if self.s.get(self.pos) != Some(&b'<') {
            return Err(Error::Malformed(self.pos));
        }
        self.pos += 1;
        let mut element = Element::new(&self.name()?);
        loop {
            self.skip_space();
            match self.s.get(self.pos) {
                Some(b'/') => {
                    if self.s.get(self.pos + 1) != Some(&b'>') {
                        return Err(Error::Malformed(self.pos));
                    }
                    self.pos += 2;
                    return Ok(element);
                }
                Some(b'>') => {
                    self.pos += 1;
                    break;
                }
                Some(_) => {
                    let name = self.name()?;
                    self.skip_space();
                    if self.s.get(self.pos) != Some(&b'=') {
                        return Err(Error::Malformed(self.pos));
                    }
                    self.pos += 1;
                    self.skip_space();
                    let quote = *self.s.get(self.pos).ok_or(Error::Malformed(self.pos))?;
                    if quote != b'"' && quote != b'\'' {
                        return Err(Error::Malformed(self.pos));
                    }
                    self.pos += 1;
                    let end = self.s[self.pos..]
                        .iter()
                        .position(|b| *b == quote)
                        .map(|i| self.pos + i)
                        .ok_or(Error::Malformed(self.pos))?;
                    let value =
                        unescape(&self.s[self.pos..end]).ok_or(Error::Malformed(self.pos))?;
                    self.pos = end + 1;
                    element.attributes.push((name, value));
                }
                None => return Err(Error::Malformed(self.pos)),
            }
        }
        // Content until the matching end tag.
        loop {
            match self.s.get(self.pos) {
                None => return Err(Error::Malformed(self.pos)),
                Some(b'<') => {
                    if self.s[self.pos..].starts_with(b"</") {
                        self.pos += 2;
                        let name = self.name()?;
                        if name != element.name {
                            return Err(Error::Malformed(self.pos));
                        }
                        self.skip_space();
                        if self.s.get(self.pos) != Some(&b'>') {
                            return Err(Error::Malformed(self.pos));
                        }
                        self.pos += 1;
                        return Ok(element);
                    }
                    if self.s[self.pos..].starts_with(b"<!--") {
                        let end =
                            find(self.s, self.pos, b"-->").ok_or(Error::Malformed(self.pos))?;
                        self.pos = end + 3;
                        continue;
                    }
                    if self.s[self.pos..].starts_with(b"<![CDATA[") {
                        let end =
                            find(self.s, self.pos, b"]]>").ok_or(Error::Malformed(self.pos))?;
                        element
                            .text
                            .push_str(&String::from_utf8_lossy(&self.s[self.pos + 9..end]));
                        self.pos = end + 3;
                        continue;
                    }
                    let child = self.element(depth + 1)?;
                    element.children.push(child);
                }
                Some(_) => {
                    let end = self.s[self.pos..]
                        .iter()
                        .position(|b| *b == b'<')
                        .map(|i| self.pos + i)
                        .ok_or(Error::Malformed(self.pos))?;
                    let text =
                        unescape(&self.s[self.pos..end]).ok_or(Error::Malformed(self.pos))?;
                    element.text.push_str(&text);
                    self.pos = end;
                }
            }
        }
    }
}

fn unescape(raw: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(raw);
    if !text.contains('&') {
        return Some(text.into_owned());
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text.as_ref();
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let end = tail.find(';')?;
        let entity = &tail[1..end];
        match entity {
            "lt" => out.push('<'),
            "gt" => out.push('>'),
            "amp" => out.push('&'),
            "quot" => out.push('"'),
            "apos" => out.push('\''),
            _ => {
                let code = entity
                    .strip_prefix("#x")
                    .and_then(|h| u32::from_str_radix(h, 16).ok())
                    .or_else(|| entity.strip_prefix('#').and_then(|d| d.parse().ok()))?;
                out.push(char::from_u32(code)?);
            }
        }
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_envelopes_with_attributes_text_and_entities() {
        let doc = Element::new("LSX").child(
            Element::new("Response")
                .attribute("sender", "EALS")
                .attribute("id", "7")
                .child(Element::new("ChallengeAccepted").attribute("response", "ab<cd\"&'"))
                .child(Element::new("Version").text("9.10.1.7 <&>")),
        );
        let xml = doc.to_xml();
        assert_eq!(
            xml,
            "<LSX><Response sender=\"EALS\" id=\"7\"><ChallengeAccepted response=\"ab&lt;cd&quot;&amp;'\"/><Version>9.10.1.7 &lt;&amp;&gt;</Version></Response></LSX>"
        );
        let parsed = parse(xml.as_bytes()).unwrap();
        assert_eq!(parsed, doc);
        let response = parsed.first("Response").unwrap();
        assert_eq!(response.attr("id"), Some("7"));
        assert_eq!(response.attr("recipient"), None);
        assert_eq!(
            response
                .first("ChallengeAccepted")
                .unwrap()
                .attr("response"),
            Some("ab<cd\"&'")
        );
        assert_eq!(response.first("Version").unwrap().text, "9.10.1.7 <&>");
    }

    #[test]
    fn accepts_prolog_whitespace_comments_cdata_and_single_quotes() {
        let xml = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<LSX>\n <Request recipient='EALS' id = '1' >\n  <!-- c --><ChallengeResponse response=\"x\" key=\"y\"><ContentId><![CDATA[a<b]]></ContentId><Title>&#65;&#x42;</Title></ChallengeResponse>\n </Request>\n</LSX>\n";
        let doc = parse(xml).unwrap();
        assert_eq!(doc.name, "LSX");
        let request = doc.first("Request").unwrap();
        assert_eq!(request.attr("recipient"), Some("EALS"));
        assert_eq!(request.attr("id"), Some("1"));
        let cr = request.first("ChallengeResponse").unwrap();
        assert_eq!(cr.first("ContentId").unwrap().text, "a<b");
        assert_eq!(cr.first("Title").unwrap().text, "AB");
        assert_eq!(parse(b"<lsx:LSX/>").unwrap().name, "lsx:LSX");
    }

    #[test]
    fn rejects_malformed_and_oversized_input() {
        for bad in [
            &b"<LSX>"[..],
            b"<LSX></lsx>",
            b"<LSX a=b/>",
            b"<LSX a=\"x/>",
            b"<LSX/><LSX/>",
            b"text",
            b"<LSX>&bogus;</LSX>",
            b"<LSX>&#xZZ;</LSX>",
            b"<LSX><Request></LSX>",
            b"</LSX>",
        ] {
            assert!(matches!(parse(bad), Err(Error::Malformed(_))), "{bad:?}");
        }
        assert_eq!(parse(&vec![b'<'; MAX_INPUT + 1]), Err(Error::Bound));
        let deep = "<a>".repeat(MAX_DEPTH + 1) + &"</a>".repeat(MAX_DEPTH + 1);
        assert_eq!(parse(deep.as_bytes()), Err(Error::Bound));
        let wide = format!("<a>{}</a>", "<b/>".repeat(MAX_NODES));
        assert_eq!(parse(wide.as_bytes()), Err(Error::Bound));
    }
}

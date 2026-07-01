//! asn1-parser core — parse an ASN.1 / DER (or BER) hex string into a readable
//! tree of tags, lengths, OIDs, and decoded primitive values. Pure Rust; the
//! only dependency is serde_json (used for the optional JSON output shape).
//!
//! DER/BER TLV encoding:
//!   * Identifier octet: bits 8-7 = class, bit 6 = constructed flag, bits 5-1 =
//!     tag number (0x1F = high-tag-number form: base-128 continuation octets).
//!   * Length: short form (top bit clear = 0..127) or long form (top bit set =
//!     the low 7 bits give the count of following big-endian length octets).
//!     0x80 = indefinite length (BER only; we surface it as an error for DER).
//!   * Contents: exactly `length` octets, recursed into for constructed types.
//!
//! We best-effort recurse into BIT STRING / OCTET STRING contents when they
//! parse cleanly as a nested DER structure (as certificates nest a public key
//! or an extension value) — otherwise the raw bytes are shown as hex.

use serde_json::{json, Map, Value};

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Parse `hex` (an ASN.1/DER byte stream written as hex) and render it either as
/// an indented text tree (`format = "tree"`, the default) or as structured JSON
/// (`format = "json"`).
pub fn parse(hex: &str, format: &str) -> Result<String, String> {
    let bytes = decode_hex(hex)?;
    if bytes.is_empty() {
        return Err("input is empty — provide an ASN.1/DER structure as hex".into());
    }
    let mut p = Parser { data: &bytes, pos: 0, base: 0 };
    let mut nodes = Vec::new();
    // A DER file is usually a single top-level element, but tolerate a
    // concatenation of several (e.g. two certs back to back).
    while p.pos < bytes.len() {
        nodes.push(p.parse_tlv(0)?);
    }
    match format.trim().to_ascii_lowercase().as_str() {
        "json" => {
            let arr: Vec<Value> = nodes.iter().map(node_to_json).collect();
            // A single top-level node renders as an object; multiple as an array.
            let v = if arr.len() == 1 { arr.into_iter().next().unwrap() } else { Value::Array(arr) };
            serde_json::to_string_pretty(&v).map_err(|e| format!("JSON serialize failed: {e}"))
        }
        "tree" | "" => {
            let mut out = String::new();
            for n in &nodes {
                render_tree(n, 0, &mut out);
            }
            // Trim the trailing newline for tidy output.
            while out.ends_with('\n') {
                out.pop();
            }
            Ok(out)
        }
        other => Err(format!("unknown format '{other}' (use tree or json)")),
    }
}

// ---------------------------------------------------------------------------
// Parse tree
// ---------------------------------------------------------------------------

/// One decoded ASN.1 element.
#[derive(Debug, Clone)]
pub struct Node {
    pub offset: usize,      // offset of the identifier octet in the stream
    pub header_len: usize,  // identifier + length octets
    pub class: u8,          // 0=universal 1=application 2=context 3=private
    pub constructed: bool,
    pub tag: u64,           // tag number
    pub id_byte: u8,        // the raw first identifier octet
    pub length: usize,      // content length in bytes
    pub content: Vec<u8>,   // raw content octets
    pub children: Vec<Node>,
    /// Present when the content is a primitive we could decode to a string.
    pub value: Option<String>,
    /// True when `children` was recovered from a BIT/OCTET STRING wrapper.
    pub encapsulated: bool,
}

struct Parser<'a> {
    data: &'a [u8],
    pos: usize,
    /// Global stream offset of `data[0]`, so nested elements report absolute
    /// offsets even though each level parses over its own content sub-slice.
    base: usize,
}

impl<'a> Parser<'a> {
    fn parse_tlv(&mut self, depth: usize) -> Result<Node, String> {
        if depth > 64 {
            return Err("structure nested too deeply (>64 levels)".into());
        }
        let offset_local = self.pos;
        let offset = self.base + offset_local;
        let id_byte = *self
            .data
            .get(self.pos)
            .ok_or_else(|| format!("unexpected end of input at offset {offset} (expected a tag)"))?;
        self.pos += 1;
        let class = id_byte >> 6;
        let constructed = id_byte & 0x20 != 0;
        let mut tag = (id_byte & 0x1f) as u64;
        if tag == 0x1f {
            // High-tag-number form: base-128, most-significant octet first.
            tag = 0;
            loop {
                let b = *self.data.get(self.pos).ok_or_else(|| {
                    format!("truncated high-tag-number tag at offset {offset}")
                })?;
                self.pos += 1;
                tag = tag
                    .checked_mul(128)
                    .and_then(|t| t.checked_add((b & 0x7f) as u64))
                    .ok_or_else(|| "tag number overflow".to_string())?;
                if b & 0x80 == 0 {
                    break;
                }
            }
        }

        // Length.
        let len_first = *self
            .data
            .get(self.pos)
            .ok_or_else(|| format!("truncated length at offset {}", self.pos))?;
        self.pos += 1;
        let length = if len_first & 0x80 == 0 {
            (len_first & 0x7f) as usize
        } else {
            let n = (len_first & 0x7f) as usize;
            if n == 0 {
                return Err(format!(
                    "indefinite-length encoding at offset {offset} is not valid DER"
                ));
            }
            if n > 8 {
                return Err(format!("length field too large ({n} octets) at offset {offset}"));
            }
            let mut len: usize = 0;
            for _ in 0..n {
                let b = *self
                    .data
                    .get(self.pos)
                    .ok_or_else(|| format!("truncated long-form length at offset {}", self.pos))?;
                self.pos += 1;
                len = (len << 8) | b as usize;
            }
            len
        };

        let header_len = self.pos - offset_local;
        let content_base = self.base + self.pos; // global offset of content[0]
        let end = self
            .pos
            .checked_add(length)
            .ok_or_else(|| "content length overflow".to_string())?;
        if end > self.data.len() {
            return Err(format!(
                "element at offset {offset} claims {length} content bytes but only {} remain",
                self.data.len() - self.pos
            ));
        }
        let content = self.data[self.pos..end].to_vec();
        self.pos = end;

        let mut node = Node {
            offset,
            header_len,
            class,
            constructed,
            tag,
            id_byte,
            length,
            content: content.clone(),
            children: Vec::new(),
            value: None,
            encapsulated: false,
        };

        if constructed {
            // Recurse into the children using a sub-parser over the content span.
            let mut sub = Parser { data: &content, pos: 0, base: content_base };
            while sub.pos < content.len() {
                node.children.push(sub.parse_tlv(depth + 1)?);
            }
        } else {
            // Primitive — decode to a friendly value, and for BIT/OCTET STRING
            // best-effort recurse into an encapsulated DER structure.
            node.value = decode_primitive(class, tag, &content);
            if class == 0 && (tag == 3 || tag == 4) {
                if let Some(kids) = try_encapsulated(class, tag, &content, content_base, depth) {
                    node.children = kids;
                    node.encapsulated = true;
                }
            }
        }
        Ok(node)
    }
}

/// Try to parse the encapsulated content of a BIT STRING (tag 3, skipping the
/// leading unused-bits octet) or OCTET STRING (tag 4) as one-or-more complete
/// DER elements. Returns the children only if the whole span parses cleanly.
fn try_encapsulated(
    class: u8,
    tag: u64,
    content: &[u8],
    content_base: usize,
    depth: usize,
) -> Option<Vec<Node>> {
    let (inner, inner_base): (&[u8], usize) = if class == 0 && tag == 3 {
        // BIT STRING: first octet = unused bits; only recurse when it's 0 (whole
        // bytes) and something follows.
        if content.len() < 2 || content[0] != 0 {
            return None;
        }
        (&content[1..], content_base + 1)
    } else {
        if content.is_empty() {
            return None;
        }
        (content, content_base)
    };
    // The first inner byte should look like a plausible ASN.1 tag: constructed
    // SEQUENCE/SET is the common encapsulation. Guard to avoid mis-parsing text.
    let first = *inner.first()?;
    let is_seqset = first == 0x30 || first == 0x31;
    if !is_seqset {
        return None;
    }
    let mut sub = Parser { data: inner, pos: 0, base: inner_base };
    let mut kids = Vec::new();
    while sub.pos < inner.len() {
        match sub.parse_tlv(depth + 1) {
            Ok(n) => kids.push(n),
            Err(_) => return None,
        }
    }
    if sub.pos == inner.len() && !kids.is_empty() {
        Some(kids)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Primitive value decoding
// ---------------------------------------------------------------------------

/// Decode a primitive's content into a human-readable string, or `None` when the
/// best representation is just its hex (handled by the renderer).
fn decode_primitive(class: u8, tag: u64, content: &[u8]) -> Option<String> {
    if class != 0 {
        // Non-universal primitive: no semantic decode, show hex.
        return None;
    }
    match tag {
        1 => Some(decode_boolean(content)),         // BOOLEAN
        2 | 10 => Some(decode_integer(content)),    // INTEGER / ENUMERATED
        5 => Some(String::new()),                   // NULL
        6 => Some(decode_oid(content)),             // OBJECT IDENTIFIER
        13 => Some(decode_relative_oid(content)),   // RELATIVE-OID
        // Text strings.
        12 | 18 | 19 | 20 | 21 | 22 | 25 | 26 | 27 => Some(decode_text(content)),
        // Time strings (ASCII).
        23 | 24 => Some(decode_text(content)),
        30 => Some(decode_bmp_string(content)),     // BMPString (UTF-16BE)
        3 => Some(decode_bit_string(content)),      // BIT STRING (summary line)
        _ => None,                                  // e.g. OCTET STRING → hex
    }
}

fn decode_boolean(content: &[u8]) -> String {
    match content {
        [] => "(empty BOOLEAN)".to_string(),
        [0x00] => "FALSE".to_string(),
        [0xff] => "TRUE".to_string(),
        [b] => format!("TRUE (non-canonical 0x{b:02X})"),
        _ => format!("(invalid multi-byte BOOLEAN, {} bytes)", content.len()),
    }
}

fn decode_integer(content: &[u8]) -> String {
    if content.is_empty() {
        return "(empty INTEGER)".to_string();
    }
    // Two's-complement big-endian. Show as decimal when it fits an i128.
    if content.len() <= 16 {
        let negative = content[0] & 0x80 != 0;
        let mut v: i128 = if negative { -1 } else { 0 };
        for &b in content {
            v = (v << 8) | b as i128;
        }
        format!("{v} (0x{})", to_hex_upper(content))
    } else {
        format!("{}-bit integer, 0x{}", content.len() * 8, to_hex_upper(content))
    }
}

fn decode_bit_string(content: &[u8]) -> String {
    match content.split_first() {
        None => "(empty BIT STRING)".to_string(),
        Some((unused, rest)) => {
            let bits = rest.len() * 8 - *unused as usize;
            if rest.len() <= 32 {
                format!("{bits} bits, unused={unused}, 0x{}", to_hex_upper(rest))
            } else {
                format!("{bits} bits, unused={unused}, {} bytes", rest.len())
            }
        }
    }
}

fn decode_text(content: &[u8]) -> String {
    String::from_utf8_lossy(content).into_owned()
}

fn decode_bmp_string(content: &[u8]) -> String {
    if content.len() % 2 != 0 {
        return String::from_utf8_lossy(content).into_owned();
    }
    let units: Vec<u16> = content
        .chunks_exact(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16_lossy(&units)
}

/// Decode an OBJECT IDENTIFIER's content to dotted form plus a friendly name.
fn decode_oid(content: &[u8]) -> String {
    match oid_arcs(content) {
        Ok(dotted) => match oid_name(&dotted) {
            Some(name) => format!("{dotted} ({name})"),
            None => dotted,
        },
        Err(e) => e,
    }
}

fn decode_relative_oid(content: &[u8]) -> String {
    // RELATIVE-OID: all arcs are plain base-128, no 40*X+Y first-byte packing.
    let mut arcs = Vec::new();
    let mut acc: u128 = 0;
    let mut started = false;
    for &b in content {
        started = true;
        acc = match acc.checked_mul(128).and_then(|a| a.checked_add((b & 0x7f) as u128)) {
            Some(a) => a,
            None => return "(RELATIVE-OID arc overflow)".to_string(),
        };
        if b & 0x80 == 0 {
            arcs.push(acc.to_string());
            acc = 0;
        }
    }
    if !started {
        return "(empty RELATIVE-OID)".to_string();
    }
    arcs.join(".")
}

/// Turn OID content octets into the dotted-decimal string.
fn oid_arcs(content: &[u8]) -> Result<String, String> {
    if content.is_empty() {
        return Err("(empty OID)".to_string());
    }
    let mut arcs: Vec<String> = Vec::new();
    let mut acc: u128 = 0;
    let mut first = true;
    let mut pending = false;
    for &b in content {
        pending = true;
        acc = acc
            .checked_mul(128)
            .and_then(|a| a.checked_add((b & 0x7f) as u128))
            .ok_or_else(|| "(OID arc overflow)".to_string())?;
        if b & 0x80 == 0 {
            if first {
                // First two arcs are packed: value = 40*X + Y (X ∈ {0,1,2}).
                let (x, y) = if acc < 40 {
                    (0, acc)
                } else if acc < 80 {
                    (1, acc - 40)
                } else {
                    (2, acc - 80)
                };
                arcs.push(x.to_string());
                arcs.push(y.to_string());
                first = false;
            } else {
                arcs.push(acc.to_string());
            }
            acc = 0;
            pending = false;
        }
    }
    if pending {
        return Err("(truncated OID — final arc has continuation bit set)".to_string());
    }
    Ok(arcs.join("."))
}

// ---------------------------------------------------------------------------
// Rendering: text tree
// ---------------------------------------------------------------------------

fn render_tree(node: &Node, depth: usize, out: &mut String) {
    let indent = "  ".repeat(depth);
    let name = tag_label(node);
    out.push_str(&indent);
    out.push_str(&format!(
        "{name} (0x{:02X}) len={} @{}",
        node.id_byte, node.length, node.offset
    ));
    if node.encapsulated {
        out.push_str(" [encapsulated]");
    }
    if node.constructed || node.encapsulated {
        out.push('\n');
        for c in &node.children {
            render_tree(c, depth + 1, out);
        }
    } else {
        match &node.value {
            Some(v) if v.is_empty() => out.push('\n'),
            Some(v) => {
                // Keep single-line; collapse embedded newlines for tree tidiness.
                let one = v.replace('\n', "\\n");
                out.push_str(&format!(": {one}\n"));
            }
            None => {
                if node.content.is_empty() {
                    out.push('\n');
                } else if node.content.len() <= 48 {
                    out.push_str(&format!(": 0x{}\n", to_hex_upper(&node.content)));
                } else {
                    out.push_str(&format!(": {} bytes, 0x{}…\n", node.content.len(), to_hex_upper(&node.content[..16])));
                }
            }
        }
    }
}

/// A human label for the element's tag, accounting for class.
fn tag_label(node: &Node) -> String {
    match node.class {
        0 => universal_tag_name(node.tag)
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("UNIVERSAL [{}]", node.tag)),
        1 => format!("APPLICATION [{}]", node.tag),
        2 => format!("[{}]", node.tag), // context-specific
        _ => format!("PRIVATE [{}]", node.tag),
    }
}

fn universal_tag_name(tag: u64) -> Option<&'static str> {
    Some(match tag {
        0 => "END-OF-CONTENTS",
        1 => "BOOLEAN",
        2 => "INTEGER",
        3 => "BIT STRING",
        4 => "OCTET STRING",
        5 => "NULL",
        6 => "OBJECT IDENTIFIER",
        7 => "ObjectDescriptor",
        8 => "EXTERNAL",
        9 => "REAL",
        10 => "ENUMERATED",
        11 => "EMBEDDED PDV",
        12 => "UTF8String",
        13 => "RELATIVE-OID",
        16 => "SEQUENCE",
        17 => "SET",
        18 => "NumericString",
        19 => "PrintableString",
        20 => "TeletexString",
        21 => "VideotexString",
        22 => "IA5String",
        23 => "UTCTime",
        24 => "GeneralizedTime",
        25 => "GraphicString",
        26 => "VisibleString",
        27 => "GeneralString",
        28 => "UniversalString",
        29 => "CHARACTER STRING",
        30 => "BMPString",
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Rendering: JSON
// ---------------------------------------------------------------------------

fn node_to_json(node: &Node) -> Value {
    let mut m = Map::new();
    m.insert("offset".into(), json!(node.offset));
    m.insert("tag".into(), json!(tag_label(node)));
    m.insert("tagByte".into(), json!(format!("0x{:02X}", node.id_byte)));
    m.insert(
        "class".into(),
        json!(match node.class {
            0 => "universal",
            1 => "application",
            2 => "context-specific",
            _ => "private",
        }),
    );
    m.insert("constructed".into(), json!(node.constructed));
    m.insert("length".into(), json!(node.length));
    if node.encapsulated {
        m.insert("encapsulated".into(), json!(true));
    }
    if node.constructed || node.encapsulated {
        let kids: Vec<Value> = node.children.iter().map(node_to_json).collect();
        m.insert("children".into(), Value::Array(kids));
    } else {
        match &node.value {
            Some(v) if !v.is_empty() => {
                m.insert("value".into(), json!(v));
            }
            Some(_) => {} // NULL and the like: no value key
            None => {
                m.insert("hex".into(), json!(to_hex_upper(&node.content)));
            }
        }
    }
    Value::Object(m)
}

// ---------------------------------------------------------------------------
// OID friendly-name table (common PKI / X.509 OIDs)
// ---------------------------------------------------------------------------

fn oid_name(dotted: &str) -> Option<&'static str> {
    Some(match dotted {
        // X.500 attribute types (Distinguished Name components)
        "2.5.4.3" => "commonName (CN)",
        "2.5.4.4" => "surname (SN)",
        "2.5.4.5" => "serialNumber",
        "2.5.4.6" => "countryName (C)",
        "2.5.4.7" => "localityName (L)",
        "2.5.4.8" => "stateOrProvinceName (ST)",
        "2.5.4.9" => "streetAddress",
        "2.5.4.10" => "organizationName (O)",
        "2.5.4.11" => "organizationalUnitName (OU)",
        "2.5.4.12" => "title",
        "2.5.4.42" => "givenName (GN)",
        "2.5.4.97" => "organizationIdentifier",
        // X.509 certificate extensions
        "2.5.29.14" => "subjectKeyIdentifier",
        "2.5.29.15" => "keyUsage",
        "2.5.29.17" => "subjectAltName",
        "2.5.29.18" => "issuerAltName",
        "2.5.29.19" => "basicConstraints",
        "2.5.29.31" => "cRLDistributionPoints",
        "2.5.29.32" => "certificatePolicies",
        "2.5.29.35" => "authorityKeyIdentifier",
        "2.5.29.37" => "extKeyUsage",
        // PKCS#1 / signature algorithms
        "1.2.840.113549.1.1.1" => "rsaEncryption",
        "1.2.840.113549.1.1.5" => "sha1WithRSAEncryption",
        "1.2.840.113549.1.1.10" => "rsassaPss",
        "1.2.840.113549.1.1.11" => "sha256WithRSAEncryption",
        "1.2.840.113549.1.1.12" => "sha384WithRSAEncryption",
        "1.2.840.113549.1.1.13" => "sha512WithRSAEncryption",
        "1.2.840.113549.1.9.1" => "emailAddress",
        // PKCS#9 / friendly names
        "1.2.840.113549.1.9.2" => "unstructuredName",
        // Elliptic-curve
        "1.2.840.10045.2.1" => "ecPublicKey",
        "1.2.840.10045.3.1.7" => "prime256v1 (P-256)",
        "1.3.132.0.34" => "secp384r1 (P-384)",
        "1.3.132.0.35" => "secp521r1 (P-521)",
        "1.2.840.10045.4.3.2" => "ecdsa-with-SHA256",
        "1.2.840.10045.4.3.3" => "ecdsa-with-SHA384",
        "1.2.840.10045.4.3.4" => "ecdsa-with-SHA512",
        // EdDSA
        "1.3.101.112" => "Ed25519",
        "1.3.101.113" => "Ed448",
        // Hash algorithms
        "2.16.840.1.101.3.4.2.1" => "sha-256",
        "2.16.840.1.101.3.4.2.2" => "sha-384",
        "2.16.840.1.101.3.4.2.3" => "sha-512",
        "1.3.14.3.2.26" => "sha-1",
        // Extended key usage purposes
        "1.3.6.1.5.5.7.3.1" => "serverAuth",
        "1.3.6.1.5.5.7.3.2" => "clientAuth",
        "1.3.6.1.5.5.7.3.3" => "codeSigning",
        "1.3.6.1.5.5.7.3.4" => "emailProtection",
        // Authority info access / OCSP
        "1.3.6.1.5.5.7.1.1" => "authorityInfoAccess",
        "1.3.6.1.5.5.7.48.1" => "ocsp",
        "1.3.6.1.5.5.7.48.2" => "caIssuers",
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Hex helpers
// ---------------------------------------------------------------------------

/// Decode a hex string, tolerating whitespace, ':' separators and a leading
/// "0x" on the whole string.
fn decode_hex(s: &str) -> Result<Vec<u8>, String> {
    // Split on whitespace / common separators into tokens, strip an optional
    // "0x"/"0X" prefix from each, then concatenate the hex digits. Tokenizing
    // (rather than a flat char filter) is what lets a separated "0x02:01:2a"
    // work — otherwise the leading '0' of "0x" leaks in as a stray hex digit.
    let mut cleaned = String::with_capacity(s.len());
    for token in s.split(|c: char| matches!(c, ' ' | '\t' | '\n' | '\r' | ':' | '-' | '_' | ',')) {
        let token = token.strip_prefix("0x").or_else(|| token.strip_prefix("0X")).unwrap_or(token);
        for c in token.chars() {
            if c.is_ascii_hexdigit() {
                cleaned.push(c);
            } else {
                return Err(format!("invalid character '{c}' in hex input"));
            }
        }
    }
    if cleaned.is_empty() {
        return Err("no hex digits found in input".into());
    }
    if cleaned.len() % 2 != 0 {
        return Err(format!(
            "hex input has an odd number of digits ({})",
            cleaned.len()
        ));
    }
    let bytes = cleaned.as_bytes();
    let mut out = Vec::with_capacity(cleaned.len() / 2);
    let mut i = 0;
    while i < bytes.len() {
        let hi = nibble(bytes[i]);
        let lo = nibble(bytes[i + 1]);
        out.push((hi << 4) | lo);
        i += 2;
    }
    Ok(out)
}

fn nibble(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => 0, // unreachable: pre-filtered to hex digits
    }
}

fn to_hex_upper(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(char::from_digit((b >> 4) as u32, 16).unwrap().to_ascii_uppercase());
        s.push(char::from_digit((b & 0x0f) as u32, 16).unwrap().to_ascii_uppercase());
    }
    s
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_integer() {
        // INTEGER 0x02 len 1 value 0x2A = 42
        let out = parse("02012A", "tree").unwrap();
        assert_eq!(out, "INTEGER (0x02) len=1 @0: 42 (0x2A)");
    }

    #[test]
    fn parse_sequence_tree() {
        // SEQUENCE { INTEGER 1, BOOLEAN TRUE }
        // 30 06 02 01 01 01 01 FF
        let out = parse("30 06 02 01 01 01 01 FF", "tree").unwrap();
        let expected = "SEQUENCE (0x30) len=6 @0\n  INTEGER (0x02) len=1 @2: 1 (0x01)\n  BOOLEAN (0x01) len=1 @5: TRUE";
        assert_eq!(out, expected);
    }

    #[test]
    fn parse_null() {
        let out = parse("0500", "tree").unwrap();
        assert_eq!(out, "NULL (0x05) len=0 @0");
    }

    #[test]
    fn decode_oid_rsa() {
        // OID 1.2.840.113549.1.1.11 = sha256WithRSAEncryption
        // 06 09 2A 86 48 86 F7 0D 01 01 0B
        let out = parse("06 09 2A 86 48 86 F7 0D 01 01 0B", "tree").unwrap();
        assert_eq!(
            out,
            "OBJECT IDENTIFIER (0x06) len=9 @0: 1.2.840.113549.1.1.11 (sha256WithRSAEncryption)"
        );
    }

    #[test]
    fn decode_oid_first_arcs() {
        // 2.5.4.3 commonName: 06 03 55 04 03
        let out = parse("06 03 55 04 03", "tree").unwrap();
        assert_eq!(
            out,
            "OBJECT IDENTIFIER (0x06) len=3 @0: 2.5.4.3 (commonName (CN))"
        );
    }

    #[test]
    fn decode_printable_string() {
        // PrintableString "US": 13 02 55 53
        let out = parse("13 02 55 53", "tree").unwrap();
        assert_eq!(out, "PrintableString (0x13) len=2 @0: US");
    }

    #[test]
    fn decode_negative_integer() {
        // INTEGER -1: 02 01 FF
        let out = parse("0201FF", "tree").unwrap();
        assert_eq!(out, "INTEGER (0x02) len=1 @0: -1 (0xFF)");
    }

    #[test]
    fn context_specific_and_octet_string_hex() {
        // [0] { OCTET STRING 0xDEADBEEF }
        // A0 06 04 04 DE AD BE EF
        let out = parse("A0 06 04 04 DE AD BE EF", "tree").unwrap();
        let expected = "[0] (0xA0) len=6 @0\n  OCTET STRING (0x04) len=4 @2: 0xDEADBEEF";
        assert_eq!(out, expected);
    }

    #[test]
    fn bit_string_encapsulation() {
        // BIT STRING with 0 unused bits wrapping SEQUENCE { INTEGER 1 }
        // 03 06 00 30 03 02 01 01
        let out = parse("03 06 00 30 03 02 01 01", "tree").unwrap();
        assert!(out.contains("[encapsulated]"), "got: {out}");
        assert!(out.contains("SEQUENCE"), "got: {out}");
        assert!(out.contains("INTEGER (0x02) len=1 @5: 1 (0x01)"), "got: {out}");
    }

    #[test]
    fn json_output_shape() {
        let out = parse("30 06 02 01 01 01 01 FF", "json").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["tag"], "SEQUENCE");
        assert_eq!(v["constructed"], true);
        assert_eq!(v["children"][0]["tag"], "INTEGER");
        assert_eq!(v["children"][0]["value"], "1 (0x01)");
        assert_eq!(v["children"][1]["value"], "TRUE");
    }

    #[test]
    fn hex_tolerates_separators_and_prefix() {
        assert!(parse("0x02:01:2a", "tree").is_ok());
        assert!(parse("02\n01\n2a", "tree").is_ok());
    }

    #[test]
    fn errors_are_reported() {
        assert!(parse("", "tree").is_err()); // empty
        assert!(parse("0203FFFF", "tree").is_err()); // length overruns
        assert!(parse("30", "tree").is_err()); // truncated length
        assert!(parse("020", "tree").is_err()); // odd hex digits
        assert!(parse("02 01 2A", "xml").is_err()); // unknown format
        assert!(parse("ZZ", "tree").is_err()); // invalid hex char
    }

    #[test]
    fn boolean_false_and_noncanonical() {
        assert_eq!(parse("010100", "tree").unwrap(), "BOOLEAN (0x01) len=1 @0: FALSE");
        assert!(parse("010101", "tree").unwrap().contains("non-canonical"));
    }

    #[test]
    fn utf8_string_decoded() {
        // UTF8String "café" = 0x63 61 66 C3 A9
        let out = parse("0C 05 63 61 66 C3 A9", "tree").unwrap();
        assert_eq!(out, "UTF8String (0x0C) len=5 @0: café");
    }
}

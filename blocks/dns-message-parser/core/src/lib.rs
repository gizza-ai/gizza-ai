//! gizza-ai/dns-message-parser core — decode a DNS wire-format message (given as
//! a hex string or base64url, e.g. the GET ?dns= parameter of a DNS-over-HTTPS
//! query) into the 12-byte header (id + flags: QR, opcode, AA, TC, RD, RA, AD,
//! CD, RCODE) and the four sections: questions, answers, authority, additional.
//! Domain names are decompressed (the 0xC0 message-compression pointers are
//! followed), and the common record types are decoded to a human-readable RDATA
//! (A, AAAA, NS, CNAME, PTR, MX, TXT, SOA, SRV, CAA, OPT/EDNS0, …).
//! Pure-Rust, no wafer/wasm-bindgen deps.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Message {
    pub header: Header,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub questions: Vec<Question>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub answers: Vec<ResourceRecord>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub authority: Vec<ResourceRecord>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub additional: Vec<ResourceRecord>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Header {
    /// 16-bit transaction id.
    pub id: u16,
    /// "query" or "response".
    pub qr: String,
    /// Opcode number.
    pub opcode: u8,
    /// Opcode name (e.g. QUERY, IQUERY, STATUS, NOTIFY, UPDATE).
    pub opcode_name: String,
    /// Authoritative Answer.
    pub aa: bool,
    /// TrunCation.
    pub tc: bool,
    /// Recursion Desired.
    pub rd: bool,
    /// Recursion Available.
    pub ra: bool,
    /// Reserved Z bit (should be 0).
    pub z: bool,
    /// Authentic Data (DNSSEC).
    pub ad: bool,
    /// Checking Disabled (DNSSEC).
    pub cd: bool,
    /// Response code number.
    pub rcode: u8,
    /// Response code name (e.g. NOERROR, FORMERR, SERVFAIL, NXDOMAIN, REFUSED).
    pub rcode_name: String,
    /// Section counts as declared in the header.
    pub qdcount: u16,
    pub ancount: u16,
    pub nscount: u16,
    pub arcount: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Question {
    /// The queried domain name (decompressed), e.g. "example.com".
    pub name: String,
    /// QTYPE number.
    pub qtype: u16,
    /// QTYPE name (e.g. A, AAAA, MX, TXT, ANY).
    pub qtype_name: String,
    /// QCLASS number.
    pub qclass: u16,
    /// QCLASS name (e.g. IN).
    pub qclass_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResourceRecord {
    /// The owner domain name (decompressed).
    pub name: String,
    /// TYPE number.
    pub rtype: u16,
    /// TYPE name.
    pub rtype_name: String,
    /// CLASS number (omitted for OPT, which reuses the field as a UDP size).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class: Option<u16>,
    /// CLASS name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class_name: Option<String>,
    /// Time-to-live in seconds (omitted for OPT).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl: Option<u32>,
    /// Declared RDATA length.
    pub rdlength: u16,
    /// Decoded RDATA, human-readable (e.g. "93.184.216.34" for an A record).
    pub rdata: String,
    /// EDNS0 details when TYPE == OPT (41).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edns: Option<Edns>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Edns {
    /// Advertised UDP payload size (from the CLASS field).
    pub udp_payload_size: u16,
    /// Extended RCODE high bits (from the TTL field).
    pub extended_rcode: u8,
    /// EDNS version (usually 0).
    pub version: u8,
    /// DNSSEC OK flag.
    pub do_bit: bool,
}

// ---------------------------------------------------------------------------

fn type_name(t: u16) -> &'static str {
    match t {
        1 => "A",
        2 => "NS",
        5 => "CNAME",
        6 => "SOA",
        12 => "PTR",
        15 => "MX",
        16 => "TXT",
        28 => "AAAA",
        33 => "SRV",
        35 => "NAPTR",
        39 => "DNAME",
        41 => "OPT",
        43 => "DS",
        46 => "RRSIG",
        47 => "NSEC",
        48 => "DNSKEY",
        50 => "NSEC3",
        51 => "NSEC3PARAM",
        52 => "TLSA",
        59 => "CDS",
        60 => "CDNSKEY",
        64 => "SVCB",
        65 => "HTTPS",
        99 => "SPF",
        255 => "ANY",
        257 => "CAA",
        _ => "",
    }
}

fn class_name(c: u16) -> &'static str {
    match c {
        1 => "IN",
        2 => "CS",
        3 => "CH",
        4 => "HS",
        254 => "NONE",
        255 => "ANY",
        _ => "",
    }
}

fn opcode_name(o: u8) -> &'static str {
    match o {
        0 => "QUERY",
        1 => "IQUERY",
        2 => "STATUS",
        4 => "NOTIFY",
        5 => "UPDATE",
        6 => "DSO",
        _ => "UNKNOWN",
    }
}

fn rcode_name(r: u8) -> &'static str {
    match r {
        0 => "NOERROR",
        1 => "FORMERR",
        2 => "SERVFAIL",
        3 => "NXDOMAIN",
        4 => "NOTIMP",
        5 => "REFUSED",
        6 => "YXDOMAIN",
        7 => "YXRRSET",
        8 => "NXRRSET",
        9 => "NOTAUTH",
        10 => "NOTZONE",
        16 => "BADVERS",
        _ => "UNKNOWN",
    }
}

// ---------------------------------------------------------------------------

/// Decode a hex string (with optional separators/0x prefix) OR a base64url
/// string into raw bytes. DNS-over-HTTPS GET passes the message as base64url.
fn decode_input(input: &str) -> Result<Vec<u8>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("input is empty".into());
    }
    // Strip a leading 0x and common hex separators, then test if it's pure hex.
    let cleaned: String = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed)
        .chars()
        .filter(|c| !matches!(c, ' ' | ':' | '-' | '.' | ',' | '\n' | '\r' | '\t' | '_'))
        .collect();

    let looks_hex = !cleaned.is_empty()
        && cleaned.len() % 2 == 0
        && cleaned.chars().all(|c| c.is_ascii_hexdigit());
    if looks_hex {
        return hex_to_bytes(&cleaned);
    }

    // Otherwise treat the original (sans whitespace) as base64 / base64url.
    let b64: String = trimmed
        .chars()
        .filter(|c| !matches!(c, ' ' | '\n' | '\r' | '\t'))
        .collect();
    base64_decode(&b64)
        .map_err(|e| format!("input is neither valid hex nor base64/base64url: {e}"))
}

fn hex_to_bytes(s: &str) -> Result<Vec<u8>, String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 2);
    let mut i = 0;
    while i < bytes.len() {
        let hi = hex_val(bytes[i])?;
        let lo = hex_val(bytes[i + 1])?;
        out.push((hi << 4) | lo);
        i += 2;
    }
    Ok(out)
}

fn hex_val(b: u8) -> Result<u8, String> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(format!("invalid hex digit '{}'", b as char)),
    }
}

/// Minimal RFC 4648 base64 / base64url decoder (accepts both alphabets, optional padding).
fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
    fn val(c: u8) -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' | b'-' => Some(62),
            b'/' | b'_' => Some(63),
            _ => None,
        }
    }
    let mut buf = 0u32;
    let mut bits = 0u32;
    let mut out = Vec::new();
    for &c in s.as_bytes() {
        if c == b'=' {
            break;
        }
        let v = val(c).ok_or_else(|| format!("invalid base64 character '{}'", c as char))?;
        buf = (buf << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    if out.is_empty() {
        return Err("decoded to zero bytes".into());
    }
    Ok(out)
}

// ---------------------------------------------------------------------------

/// Read a (possibly compressed) domain name starting at `pos`. Returns the
/// dotted name and the position immediately after the name *in the original
/// stream* (pointers do not advance the outer cursor past the pointer bytes).
fn read_name(msg: &[u8], start: usize) -> Result<(String, usize), String> {
    let mut labels: Vec<String> = Vec::new();
    let mut pos = start;
    let mut jumped = false;
    let mut next_after = start;
    let mut guard = 0usize;

    loop {
        guard += 1;
        if guard > 255 {
            return Err("domain name has too many labels (possible compression loop)".into());
        }
        if pos >= msg.len() {
            return Err("domain name runs past end of message".into());
        }
        let len = msg[pos];
        match len & 0xC0 {
            0x00 => {
                if len == 0 {
                    pos += 1;
                    if !jumped {
                        next_after = pos;
                    }
                    break;
                }
                let lstart = pos + 1;
                let lend = lstart + len as usize;
                if lend > msg.len() {
                    return Err("label runs past end of message".into());
                }
                let label = String::from_utf8_lossy(&msg[lstart..lend]).to_string();
                labels.push(label);
                pos = lend;
            }
            0xC0 => {
                // Compression pointer: 14-bit offset.
                if pos + 1 >= msg.len() {
                    return Err("compression pointer runs past end of message".into());
                }
                let ptr = (((len as usize) & 0x3F) << 8) | msg[pos + 1] as usize;
                if !jumped {
                    next_after = pos + 2;
                    jumped = true;
                }
                if ptr >= msg.len() {
                    return Err("compression pointer targets past end of message".into());
                }
                pos = ptr;
            }
            _ => return Err("reserved label length bits (0x40/0x80) are not supported".into()),
        }
    }

    let name = if labels.is_empty() {
        ".".to_string()
    } else {
        labels.join(".")
    };
    Ok((name, next_after))
}

fn read_u16(msg: &[u8], pos: usize) -> Result<u16, String> {
    if pos + 2 > msg.len() {
        return Err("unexpected end of message reading u16".into());
    }
    Ok(((msg[pos] as u16) << 8) | msg[pos + 1] as u16)
}

fn read_u32(msg: &[u8], pos: usize) -> Result<u32, String> {
    if pos + 4 > msg.len() {
        return Err("unexpected end of message reading u32".into());
    }
    Ok(((msg[pos] as u32) << 24)
        | ((msg[pos + 1] as u32) << 16)
        | ((msg[pos + 2] as u32) << 8)
        | msg[pos + 3] as u32)
}

fn fmt_ipv4(b: &[u8]) -> String {
    format!("{}.{}.{}.{}", b[0], b[1], b[2], b[3])
}

fn fmt_ipv6(b: &[u8]) -> String {
    // Render the 8 hextets, then compress the longest run of zero groups to "::".
    let groups: Vec<u16> = (0..8)
        .map(|i| ((b[i * 2] as u16) << 8) | b[i * 2 + 1] as u16)
        .collect();
    // Find longest zero run (length >= 2 to compress).
    let (mut best_start, mut best_len) = (0usize, 0usize);
    let (mut cur_start, mut cur_len) = (0usize, 0usize);
    for (i, &g) in groups.iter().enumerate() {
        if g == 0 {
            if cur_len == 0 {
                cur_start = i;
            }
            cur_len += 1;
            if cur_len > best_len {
                best_len = cur_len;
                best_start = cur_start;
            }
        } else {
            cur_len = 0;
        }
    }
    if best_len < 2 {
        return groups
            .iter()
            .map(|g| format!("{:x}", g))
            .collect::<Vec<_>>()
            .join(":");
    }
    let mut out = String::new();
    let mut i = 0;
    while i < 8 {
        if i == best_start {
            out.push_str("::");
            i += best_len;
            continue;
        }
        if !out.is_empty() && !out.ends_with(':') {
            out.push(':');
        }
        out.push_str(&format!("{:x}", groups[i]));
        i += 1;
    }
    out
}

/// Decode RDATA for the well-known types; falls back to a hex dump.
/// `rstart` is the offset of the RDATA in the full message; `rlen` its length.
fn decode_rdata(
    msg: &[u8],
    rtype: u16,
    rstart: usize,
    rlen: usize,
    notes: &mut Vec<String>,
) -> String {
    let rd = &msg[rstart..rstart + rlen];
    match rtype {
        1 => {
            if rlen == 4 {
                fmt_ipv4(rd)
            } else {
                hex_dump(rd)
            }
        }
        28 => {
            if rlen == 16 {
                fmt_ipv6(rd)
            } else {
                hex_dump(rd)
            }
        }
        2 | 5 | 12 | 39 => {
            // NS / CNAME / PTR / DNAME — a single domain name.
            match read_name(msg, rstart) {
                Ok((n, _)) => n,
                Err(e) => {
                    notes.push(format!("name in RDATA: {e}"));
                    hex_dump(rd)
                }
            }
        }
        15 => {
            // MX: preference (u16) + exchange name.
            if rlen < 3 {
                return hex_dump(rd);
            }
            let pref = ((rd[0] as u16) << 8) | rd[1] as u16;
            match read_name(msg, rstart + 2) {
                Ok((n, _)) => format!("{} {}", pref, n),
                Err(e) => {
                    notes.push(format!("MX exchange: {e}"));
                    hex_dump(rd)
                }
            }
        }
        16 | 99 => {
            // TXT / SPF: one or more <len><string> chunks.
            let mut parts: Vec<String> = Vec::new();
            let mut i = 0;
            while i < rlen {
                let l = rd[i] as usize;
                i += 1;
                if i + l > rlen {
                    notes.push("TXT chunk runs past RDATA".into());
                    break;
                }
                parts.push(format!(
                    "\"{}\"",
                    String::from_utf8_lossy(&rd[i..i + l]).replace('"', "\\\"")
                ));
                i += l;
            }
            parts.join(" ")
        }
        6 => {
            // SOA: mname, rname, serial, refresh, retry, expire, minimum.
            match read_name(msg, rstart) {
                Ok((mname, p1)) => match read_name(msg, p1) {
                    Ok((rname, p2)) => {
                        if p2 + 20 <= rstart + rlen {
                            let serial = read_u32(msg, p2).unwrap_or(0);
                            let refresh = read_u32(msg, p2 + 4).unwrap_or(0);
                            let retry = read_u32(msg, p2 + 8).unwrap_or(0);
                            let expire = read_u32(msg, p2 + 12).unwrap_or(0);
                            let minimum = read_u32(msg, p2 + 16).unwrap_or(0);
                            format!(
                                "{} {} {} {} {} {} {}",
                                mname, rname, serial, refresh, retry, expire, minimum
                            )
                        } else {
                            format!("{} {} (truncated)", mname, rname)
                        }
                    }
                    Err(e) => {
                        notes.push(format!("SOA rname: {e}"));
                        hex_dump(rd)
                    }
                },
                Err(e) => {
                    notes.push(format!("SOA mname: {e}"));
                    hex_dump(rd)
                }
            }
        }
        33 => {
            // SRV: priority, weight, port, target.
            if rlen < 7 {
                return hex_dump(rd);
            }
            let prio = ((rd[0] as u16) << 8) | rd[1] as u16;
            let weight = ((rd[2] as u16) << 8) | rd[3] as u16;
            let port = ((rd[4] as u16) << 8) | rd[5] as u16;
            match read_name(msg, rstart + 6) {
                Ok((n, _)) => format!("{} {} {} {}", prio, weight, port, n),
                Err(e) => {
                    notes.push(format!("SRV target: {e}"));
                    hex_dump(rd)
                }
            }
        }
        257 => {
            // CAA: flags (u8), tag-length (u8), tag, value.
            if rlen < 2 {
                return hex_dump(rd);
            }
            let flags = rd[0];
            let taglen = rd[1] as usize;
            if 2 + taglen > rlen {
                return hex_dump(rd);
            }
            let tag = String::from_utf8_lossy(&rd[2..2 + taglen]);
            let value = String::from_utf8_lossy(&rd[2 + taglen..]);
            format!("{} {} \"{}\"", flags, tag, value)
        }
        _ => hex_dump(rd),
    }
}

fn hex_dump(b: &[u8]) -> String {
    if b.is_empty() {
        return String::new();
    }
    b.iter()
        .map(|x| format!("{:02x}", x))
        .collect::<Vec<_>>()
        .join("")
}

// ---------------------------------------------------------------------------

fn name_or_num(name: &str, num: u16) -> String {
    if name.is_empty() {
        format!("TYPE{}", num)
    } else {
        name.to_string()
    }
}

fn parse_question(msg: &[u8], pos: usize) -> Result<(Question, usize), String> {
    let (name, p) = read_name(msg, pos)?;
    let qtype = read_u16(msg, p)?;
    let qclass = read_u16(msg, p + 2)?;
    let q = Question {
        name,
        qtype,
        qtype_name: name_or_num(type_name(qtype), qtype),
        qclass,
        qclass_name: name_or_num(class_name(qclass), qclass),
    };
    Ok((q, p + 4))
}

fn parse_rr(
    msg: &[u8],
    pos: usize,
    notes: &mut Vec<String>,
) -> Result<(ResourceRecord, usize), String> {
    let (name, p) = read_name(msg, pos)?;
    let rtype = read_u16(msg, p)?;
    let class_or_size = read_u16(msg, p + 2)?;
    let ttl = read_u32(msg, p + 4)?;
    let rdlength = read_u16(msg, p + 8)?;
    let rstart = p + 10;
    if rstart + rdlength as usize > msg.len() {
        return Err("RDATA runs past end of message".into());
    }
    let rtype_name = name_or_num(type_name(rtype), rtype);

    if rtype == 41 {
        // OPT / EDNS0: the CLASS field is the UDP payload size; the TTL field
        // carries the extended RCODE (high byte), version, and flags (DO bit).
        let extended_rcode = ((ttl >> 24) & 0xFF) as u8;
        let version = ((ttl >> 16) & 0xFF) as u8;
        let do_bit = (ttl & 0x8000) != 0;
        let rr = ResourceRecord {
            name,
            rtype,
            rtype_name,
            class: None,
            class_name: None,
            ttl: None,
            rdlength,
            rdata: format!(
                "EDNS0 udp={} version={} do={}",
                class_or_size, version, do_bit
            ),
            edns: Some(Edns {
                udp_payload_size: class_or_size,
                extended_rcode,
                version,
                do_bit,
            }),
        };
        return Ok((rr, rstart + rdlength as usize));
    }

    let rdata = decode_rdata(msg, rtype, rstart, rdlength as usize, notes);
    let rr = ResourceRecord {
        name,
        rtype,
        rtype_name,
        class: Some(class_or_size),
        class_name: Some(name_or_num(class_name(class_or_size), class_or_size)),
        ttl: Some(ttl),
        rdlength,
        rdata,
        edns: None,
    };
    Ok((rr, rstart + rdlength as usize))
}

/// Parse a DNS message from a hex or base64url string.
pub fn parse(input: &str) -> Result<Message, String> {
    let msg = decode_input(input)?;
    if msg.len() < 12 {
        return Err(format!(
            "message is only {} bytes; a DNS message needs at least a 12-byte header",
            msg.len()
        ));
    }

    let id = read_u16(&msg, 0)?;
    let flags = read_u16(&msg, 2)?;
    let qdcount = read_u16(&msg, 4)?;
    let ancount = read_u16(&msg, 6)?;
    let nscount = read_u16(&msg, 8)?;
    let arcount = read_u16(&msg, 10)?;

    let qr = if (flags & 0x8000) != 0 {
        "response"
    } else {
        "query"
    };
    let opcode = ((flags >> 11) & 0x0F) as u8;
    let aa = (flags & 0x0400) != 0;
    let tc = (flags & 0x0200) != 0;
    let rd = (flags & 0x0100) != 0;
    let ra = (flags & 0x0080) != 0;
    let z = (flags & 0x0040) != 0;
    let ad = (flags & 0x0020) != 0;
    let cd = (flags & 0x0010) != 0;
    let rcode = (flags & 0x000F) as u8;

    let header = Header {
        id,
        qr: qr.to_string(),
        opcode,
        opcode_name: opcode_name(opcode).to_string(),
        aa,
        tc,
        rd,
        ra,
        z,
        ad,
        cd,
        rcode,
        rcode_name: rcode_name(rcode).to_string(),
        qdcount,
        ancount,
        nscount,
        arcount,
    };

    let mut notes: Vec<String> = Vec::new();
    let mut pos = 12;

    let mut questions = Vec::new();
    for _ in 0..qdcount {
        match parse_question(&msg, pos) {
            Ok((q, p)) => {
                questions.push(q);
                pos = p;
            }
            Err(e) => {
                notes.push(format!("stopped parsing questions: {e}"));
                break;
            }
        }
    }

    let parse_section = |count: u16, label: &str, pos: &mut usize, notes: &mut Vec<String>| {
        let mut recs = Vec::new();
        for _ in 0..count {
            match parse_rr(&msg, *pos, notes) {
                Ok((rr, p)) => {
                    recs.push(rr);
                    *pos = p;
                }
                Err(e) => {
                    notes.push(format!("stopped parsing {label}: {e}"));
                    break;
                }
            }
        }
        recs
    };

    let answers = parse_section(ancount, "answers", &mut pos, &mut notes);
    let authority = parse_section(nscount, "authority", &mut pos, &mut notes);
    let additional = parse_section(arcount, "additional", &mut pos, &mut notes);

    if pos < msg.len() {
        notes.push(format!(
            "{} trailing byte(s) after the last record were not consumed",
            msg.len() - pos
        ));
    }

    Ok(Message {
        header,
        questions,
        answers,
        authority,
        additional,
        notes,
    })
}

/// Parse and return as pretty JSON (chat / programmatic surface).
pub fn run(input: &str) -> Result<String, String> {
    let m = parse(input)?;
    serde_json::to_string_pretty(&m).map_err(|e| e.to_string())
}

/// Human-readable rendering (used by the page).
pub fn render(input: &str) -> Result<String, String> {
    let m = parse(input)?;
    let h = &m.header;
    let mut out = String::new();
    out.push_str(&format!("Header:\n  ID:       0x{:04x} ({})\n", h.id, h.id));
    out.push_str(&format!("  QR:       {}\n", h.qr));
    out.push_str(&format!("  Opcode:   {} ({})\n", h.opcode, h.opcode_name));
    let mut flags = Vec::new();
    if h.aa {
        flags.push("AA");
    }
    if h.tc {
        flags.push("TC");
    }
    if h.rd {
        flags.push("RD");
    }
    if h.ra {
        flags.push("RA");
    }
    if h.ad {
        flags.push("AD");
    }
    if h.cd {
        flags.push("CD");
    }
    out.push_str(&format!(
        "  Flags:    {}\n",
        if flags.is_empty() {
            "(none)".to_string()
        } else {
            flags.join(" ")
        }
    ));
    out.push_str(&format!("  RCODE:    {} ({})\n", h.rcode, h.rcode_name));
    out.push_str(&format!(
        "  Counts:   QD={} AN={} NS={} AR={}\n",
        h.qdcount, h.ancount, h.nscount, h.arcount
    ));

    if !m.questions.is_empty() {
        out.push_str("\nQuestion:\n");
        for q in &m.questions {
            out.push_str(&format!("  {} {} {}\n", q.name, q.qclass_name, q.qtype_name));
        }
    }

    let mut render_section = |label: &str, recs: &[ResourceRecord]| {
        if recs.is_empty() {
            return;
        }
        out.push_str(&format!("\n{}:\n", label));
        for r in recs {
            match (r.ttl, &r.class_name) {
                (Some(ttl), Some(cn)) => out.push_str(&format!(
                    "  {} {} {} {}  {}\n",
                    r.name, ttl, cn, r.rtype_name, r.rdata
                )),
                _ => out.push_str(&format!("  {} {}  {}\n", r.name, r.rtype_name, r.rdata)),
            }
        }
    };
    render_section("Answer", &m.answers);
    render_section("Authority", &m.authority);
    render_section("Additional", &m.additional);

    if !m.notes.is_empty() {
        out.push_str("\nNotes:\n");
        for n in &m.notes {
            out.push_str(&format!("  - {}\n", n));
        }
    }

    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // A real-ish DNS response for example.com A query, hand-built:
    // header id=0x1234, flags=0x8180 (response, RD, RA, NOERROR), qd=1 an=1
    // Q: example.com A IN
    // A: name -> compression pointer 0xC00C, A IN ttl=60 rdata 93.184.216.34
    const RESP_HEX: &str = concat!(
        "1234",     // id
        "8180",     // flags: response, RD, RA, NOERROR
        "0001",     // qdcount
        "0001",     // ancount
        "0000",     // nscount
        "0000",     // arcount
        "076578616d706c6503636f6d00", // 7 example 3 com 0
        "0001",     // QTYPE A
        "0001",     // QCLASS IN
        "c00c",     // name = pointer 0x0c
        "0001",     // type A
        "0001",     // class IN
        "0000003c", // ttl 60
        "0004",     // rdlength 4
        "5db8d822"  // 93.184.216.34
    );

    #[test]
    fn parses_a_response_with_compression() {
        let m = parse(RESP_HEX).unwrap();
        assert_eq!(m.header.id, 0x1234);
        assert_eq!(m.header.qr, "response");
        assert!(m.header.rd);
        assert!(m.header.ra);
        assert_eq!(m.header.rcode_name, "NOERROR");
        assert_eq!(m.questions.len(), 1);
        assert_eq!(m.questions[0].name, "example.com");
        assert_eq!(m.questions[0].qtype_name, "A");
        assert_eq!(m.answers.len(), 1);
        // compression pointer resolved back to example.com
        assert_eq!(m.answers[0].name, "example.com");
        assert_eq!(m.answers[0].rtype_name, "A");
        assert_eq!(m.answers[0].ttl, Some(60));
        assert_eq!(m.answers[0].rdata, "93.184.216.34");
        assert!(m.notes.is_empty());
    }

    #[test]
    fn render_is_human_readable() {
        let s = render(RESP_HEX).unwrap();
        assert!(s.contains("QR:       response"));
        assert!(s.contains("example.com"));
        assert!(s.contains("93.184.216.34"));
        assert!(s.contains("NOERROR"));
    }

    #[test]
    fn accepts_base64url() {
        // build raw bytes from RESP_HEX
        let bytes = hex_to_bytes(
            &RESP_HEX
                .chars()
                .filter(|c| c.is_ascii_hexdigit())
                .collect::<String>(),
        )
        .unwrap();
        // encode base64url (no padding)
        const ALPHA: &[u8] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        let mut b64 = String::new();
        for chunk in bytes.chunks(3) {
            let b0 = chunk[0] as u32;
            let b1 = *chunk.get(1).unwrap_or(&0) as u32;
            let b2 = *chunk.get(2).unwrap_or(&0) as u32;
            let n = (b0 << 16) | (b1 << 8) | b2;
            b64.push(ALPHA[((n >> 18) & 63) as usize] as char);
            b64.push(ALPHA[((n >> 12) & 63) as usize] as char);
            if chunk.len() > 1 {
                b64.push(ALPHA[((n >> 6) & 63) as usize] as char);
            }
            if chunk.len() > 2 {
                b64.push(ALPHA[(n & 63) as usize] as char);
            }
        }
        let m = parse(&b64).unwrap();
        assert_eq!(m.questions[0].name, "example.com");
        assert_eq!(m.answers[0].rdata, "93.184.216.34");
    }

    #[test]
    fn decodes_mx_and_txt() {
        // Question: a.test MX IN ; answers MX + TXT
        let hex = concat!(
            "0000", "8180", "0001", "0002", "0000", "0000",
            "0161047465737400", "000f", "0001",
            "c00c", "000f", "0001", "0000012c", "000a",
            "000a", "016d047465737400",
            "c00c", "0010", "0001", "0000012c", "0006",
            "0568656c6c6f"
        );
        let m = parse(hex).unwrap();
        assert_eq!(m.questions[0].name, "a.test");
        assert_eq!(m.questions[0].qtype_name, "MX");
        assert_eq!(m.answers.len(), 2);
        assert_eq!(m.answers[0].rtype_name, "MX");
        assert_eq!(m.answers[0].rdata, "10 m.test");
        assert_eq!(m.answers[1].rtype_name, "TXT");
        assert_eq!(m.answers[1].rdata, "\"hello\"");
    }

    #[test]
    fn decodes_aaaa_and_opt() {
        // Query + AAAA answer + OPT (EDNS0) in additional.
        let hex = concat!(
            "0000", "8180", "0001", "0001", "0000", "0001",
            "04686f737400", "001c", "0001",
            "c00c", "001c", "0001", "0000003c", "0010",
            "00000000000000000000000000000001",
            "00", "0029", "1000", "00000000", "0000"
        );
        let m = parse(hex).unwrap();
        assert_eq!(m.answers[0].rtype_name, "AAAA");
        assert_eq!(m.answers[0].rdata, "::1");
        assert_eq!(m.additional.len(), 1);
        let opt = &m.additional[0];
        assert_eq!(opt.rtype_name, "OPT");
        let edns = opt.edns.as_ref().unwrap();
        assert_eq!(edns.udp_payload_size, 4096);
        assert!(opt.class.is_none());
        assert!(opt.ttl.is_none());
    }

    #[test]
    fn rejects_short_message() {
        let e = parse("0011223344").unwrap_err();
        assert!(e.contains("at least a 12-byte header"), "got: {e}");
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse("not valid hex or base64 !!!@@@###").is_err());
    }

    #[test]
    fn ipv6_compression_full_zero() {
        let hex = concat!(
            "0000", "8180", "0001", "0001", "0000", "0000",
            "04686f737400", "001c", "0001",
            "c00c", "001c", "0001", "0000003c", "0010",
            "00000000000000000000000000000000"
        );
        let m = parse(hex).unwrap();
        assert_eq!(m.answers[0].rdata, "::");
    }
}

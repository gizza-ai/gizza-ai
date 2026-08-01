//! pcap-grep core — pure, ngrep-style payload search over a libpcap (`.pcap`) or
//! pcapng (`.pcapng`) capture. Walks the container, extracts each packet's
//! application payload (bytes after the TCP/UDP header; after the IP header for
//! other IP protocols; the L2 payload for non-IP), and matches a regular (or
//! hexadecimal) expression against it, returning the matching packets with their
//! metadata.
//!
//! The only dependency is `regex` (pure Rust, wasm-safe); the container parsing
//! is hand-rolled from the byte layout, so the whole thing instantiates on every
//! backend including the chat Service Worker.

use regex::bytes::{Regex, RegexBuilder};
use std::fmt::Write as _;

/// Maximum payload bytes rendered into `payload_ascii` per match.
const MAX_ASCII_RENDER: usize = 2048;
/// Maximum payload bytes included in the optional hex dump per match.
const MAX_HEX_RENDER: usize = 512;
/// Maximum matched-substring bytes rendered into `matched_text`.
const MAX_MATCH_RENDER: usize = 512;

/// Options controlling the search.
#[derive(Debug, Clone)]
pub struct GrepOptions {
    /// Case-insensitive regex match (ngrep `-i`). Ignored in hex mode.
    pub ignore_case: bool,
    /// Treat `pattern` as a hexadecimal byte string (ngrep `-X`), e.g. `47455420`.
    pub hex: bool,
    /// Invert the match: return packets whose payload does NOT match (ngrep `-v`).
    pub invert: bool,
    /// When set, only search packets whose source OR destination port equals this.
    pub port: Option<u16>,
    /// Include a canonical hex + ASCII dump of the payload per match (ngrep `-x`).
    pub show_hex: bool,
    /// Maximum number of matching packets to return (the total count is still reported).
    pub limit: usize,
}

impl Default for GrepOptions {
    fn default() -> Self {
        GrepOptions {
            ignore_case: false,
            hex: false,
            invert: false,
            port: None,
            show_hex: false,
            limit: 100,
        }
    }
}

/// One matching packet.
#[derive(Debug, Clone, PartialEq)]
pub struct Match {
    /// 1-based index in capture order.
    pub index: usize,
    /// Capture timestamp in seconds since the Unix epoch (fractional).
    pub timestamp: f64,
    /// Highest decoded protocol name (e.g. `TCP`, `UDP`, `ICMP`).
    pub protocol: String,
    /// Source address (IP or MAC).
    pub source: String,
    /// Destination address.
    pub destination: String,
    /// Source transport port, when TCP/UDP.
    pub source_port: Option<u16>,
    /// Destination transport port, when TCP/UDP.
    pub destination_port: Option<u16>,
    /// TCP flag names (e.g. `SYN, ACK`), when TCP.
    pub flags: Option<String>,
    /// Length of the searched payload in bytes.
    pub payload_len: usize,
    /// Byte offset of the match within the payload (None for inverted matches).
    pub match_offset: Option<usize>,
    /// Length of the matched span in bytes (None for inverted matches).
    pub match_length: Option<usize>,
    /// Printable rendering of the matched bytes (None for inverted matches).
    pub matched_text: Option<String>,
    /// Printable rendering of the payload (non-printable bytes shown as `.`).
    pub payload_ascii: String,
    /// Canonical hex + ASCII dump of the payload (only when `show_hex`).
    pub payload_hex: Option<String>,
}

/// The result of a search.
#[derive(Debug, Clone, PartialEq)]
pub struct GrepResult {
    /// Container format: `pcap` or `pcapng`.
    pub format: &'static str,
    /// Link-layer type name of the first interface (e.g. `Ethernet`).
    pub link_type: String,
    /// Total packets present in the file.
    pub total_packets: usize,
    /// Packets that had a non-empty payload and passed the port filter (i.e. were searched).
    pub scanned_packets: usize,
    /// Total packets matching the expression (before the `limit` cap).
    pub matched_packets: usize,
    /// True when `matched_packets` exceeded `limit` and the list was truncated.
    pub truncated: bool,
    /// The matching packets (capped by `limit`).
    pub matches: Vec<Match>,
}

/// Compile the user pattern (regex or hex) into a byte regex.
fn compile(pattern: &str, opts: &GrepOptions) -> Result<Regex, String> {
    if pattern.is_empty() {
        return Err("pattern is empty; provide a regular expression to search for".into());
    }
    let effective = if opts.hex {
        let cleaned: String = pattern
            .chars()
            .filter(|c| !c.is_whitespace() && *c != ':' && *c != '-')
            .collect();
        if cleaned.is_empty() {
            return Err("hex pattern is empty after removing separators".into());
        }
        if cleaned.len() % 2 != 0 {
            return Err(format!(
                "hex pattern must have an even number of hex digits, got {} (e.g. 47455420 for \"GET \")",
                cleaned.len()
            ));
        }
        let mut esc = String::with_capacity(cleaned.len() * 2);
        let bytes = cleaned.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            let hi = (bytes[i] as char)
                .to_digit(16)
                .ok_or_else(|| format!("invalid hex digit '{}' in hex pattern", bytes[i] as char))?;
            let lo = (bytes[i + 1] as char)
                .to_digit(16)
                .ok_or_else(|| format!("invalid hex digit '{}' in hex pattern", bytes[i + 1] as char))?;
            let _ = write!(esc, "\\x{:02x}", (hi * 16 + lo) as u8);
            i += 2;
        }
        esc
    } else {
        pattern.to_string()
    };

    RegexBuilder::new(&effective)
        .unicode(false)
        .case_insensitive(opts.ignore_case && !opts.hex)
        .build()
        .map_err(|e| format!("invalid regular expression: {e}"))
}

/// Search a capture for `pattern`, returning the matching packets.
pub fn grep(bytes: &[u8], pattern: &str, opts: &GrepOptions) -> Result<GrepResult, String> {
    let re = compile(pattern, opts)?;
    let limit = opts.limit;

    let mut matches: Vec<Match> = Vec::new();
    let mut total_packets = 0usize;
    let mut scanned_packets = 0usize;
    let mut matched_packets = 0usize;

    let (format, link_type) = walk(bytes, &mut |index, ts, data, link_type| {
        total_packets += 1;
        let dec = decode(data, link_type);
        // Port filter (ngrep-style narrowing): keep only packets touching `port`.
        if let Some(want) = opts.port {
            let hit =
                dec.source_port == Some(want) || dec.destination_port == Some(want);
            if !hit {
                return;
            }
        }
        let payload = dec.payload;
        if payload.is_empty() {
            return;
        }
        scanned_packets += 1;

        let found = re.find(payload);
        let is_match = found.is_some() ^ opts.invert;
        if !is_match {
            return;
        }
        matched_packets += 1;
        if matches.len() >= limit {
            return;
        }

        let (match_offset, match_length, matched_text) = match (found, opts.invert) {
            (Some(m), false) => (
                Some(m.start()),
                Some(m.end() - m.start()),
                Some(render_ascii(&payload[m.start()..m.end()], MAX_MATCH_RENDER)),
            ),
            _ => (None, None, None),
        };

        matches.push(Match {
            index,
            timestamp: ts,
            protocol: dec.protocol,
            source: dec.source,
            destination: dec.destination,
            source_port: dec.source_port,
            destination_port: dec.destination_port,
            flags: dec.flags,
            payload_len: payload.len(),
            match_offset,
            match_length,
            matched_text,
            payload_ascii: render_ascii(payload, MAX_ASCII_RENDER),
            payload_hex: if opts.show_hex {
                Some(hex_dump(payload, MAX_HEX_RENDER))
            } else {
                None
            },
        });
    })?;

    Ok(GrepResult {
        format,
        link_type,
        total_packets,
        scanned_packets,
        matched_packets,
        truncated: matched_packets > matches.len(),
        matches,
    })
}

/// Render bytes ngrep-style: printable ASCII kept, everything else shown as `.`.
fn render_ascii(data: &[u8], cap: usize) -> String {
    let take = data.len().min(cap);
    let mut s = String::with_capacity(take + 8);
    for &b in &data[..take] {
        if (0x20..=0x7e).contains(&b) {
            s.push(b as char);
        } else {
            s.push('.');
        }
    }
    if data.len() > cap {
        let _ = write!(s, "… (+{} more bytes)", data.len() - cap);
    }
    s
}

/// Canonical 16-byte-per-row hex + ASCII dump (offset  hex  ascii).
fn hex_dump(data: &[u8], cap: usize) -> String {
    let take = data.len().min(cap);
    let mut out = String::new();
    let mut off = 0usize;
    while off < take {
        let row = &data[off..(off + 16).min(take)];
        let _ = write!(out, "{off:04x}  ");
        for (i, b) in row.iter().enumerate() {
            let _ = write!(out, "{b:02x} ");
            if i == 7 {
                out.push(' ');
            }
        }
        for i in row.len()..16 {
            out.push_str("   ");
            if i == 7 {
                out.push(' ');
            }
        }
        out.push(' ');
        for &b in row {
            out.push(if (0x20..=0x7e).contains(&b) { b as char } else { '.' });
        }
        out.push('\n');
        off += 16;
    }
    if data.len() > cap {
        let _ = write!(out, "… (+{} more bytes)\n", data.len() - cap);
    }
    out
}

// ---------------------------------------------------------------------------
// Container walking (classic pcap + pcapng) — yields (index, ts, data, linktype)
// ---------------------------------------------------------------------------

const LINKTYPE_ETHERNET: u32 = 1;
const LINKTYPE_RAW: u32 = 101;
const LINKTYPE_RAW_ALT1: u32 = 12;
const LINKTYPE_LINUX_SLL: u32 = 113;
const LINKTYPE_NULL: u32 = 0;

fn link_type_name(lt: u32) -> String {
    match lt {
        LINKTYPE_NULL => "Null/Loopback".into(),
        LINKTYPE_ETHERNET => "Ethernet".into(),
        LINKTYPE_RAW_ALT1 | LINKTYPE_RAW => "Raw IP".into(),
        LINKTYPE_LINUX_SLL => "Linux cooked".into(),
        other => format!("LINKTYPE_{other}"),
    }
}

fn rd_u16_le(b: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*b.get(o)?, *b.get(o + 1)?]))
}
fn rd_u32_le(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes([
        *b.get(o)?,
        *b.get(o + 1)?,
        *b.get(o + 2)?,
        *b.get(o + 3)?,
    ]))
}
fn rd_u16_be(b: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*b.get(o)?, *b.get(o + 1)?]))
}
fn rd_u32_be(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_be_bytes([
        *b.get(o)?,
        *b.get(o + 1)?,
        *b.get(o + 2)?,
        *b.get(o + 3)?,
    ]))
}

/// Walk the capture, invoking `on_pkt(index, ts, data, linktype)` per packet.
/// Returns `(format, first_link_type_name)`.
fn walk(
    bytes: &[u8],
    on_pkt: &mut dyn FnMut(usize, f64, &[u8], u32),
) -> Result<(&'static str, String), String> {
    if bytes.len() < 4 {
        return Err("file is too small to be a pcap/pcapng capture".into());
    }
    let magic = [bytes[0], bytes[1], bytes[2], bytes[3]];
    match magic {
        [0xa1, 0xb2, 0xc3, 0xd4] | [0xa1, 0xb2, 0x3c, 0x4d] => walk_classic(bytes, false, on_pkt),
        [0xd4, 0xc3, 0xb2, 0xa1] | [0x4d, 0x3c, 0xb2, 0xa1] => walk_classic(bytes, true, on_pkt),
        [0x0a, 0x0d, 0x0d, 0x0a] => walk_pcapng(bytes, on_pkt),
        _ => Err("not a pcap or pcapng capture (unrecognised magic bytes); expected a libpcap \
                  (.pcap) or pcapng (.pcapng) file"
            .into()),
    }
}

fn walk_classic(
    b: &[u8],
    swapped: bool,
    on_pkt: &mut dyn FnMut(usize, f64, &[u8], u32),
) -> Result<(&'static str, String), String> {
    if b.len() < 24 {
        return Err("truncated pcap global header".into());
    }
    let ns_magic = matches!(b[0..4], [0xa1, 0xb2, 0x3c, 0x4d] | [0x4d, 0x3c, 0xb2, 0xa1]);
    let frac_div = if ns_magic { 1_000_000_000.0 } else { 1_000_000.0 };
    let rd32 = |o: usize| if swapped { rd_u32_le(b, o) } else { rd_u32_be(b, o) };
    let link_type = rd32(20).ok_or("truncated pcap header")? & 0xffff;

    let mut off = 24usize;
    let mut index = 0usize;
    while off + 16 <= b.len() {
        let ts_sec = rd32(off).unwrap();
        let ts_frac = rd32(off + 4).unwrap();
        let cap_len = rd32(off + 8).unwrap();
        let data_off = off + 16;
        let cl = cap_len as usize;
        if data_off + cl > b.len() {
            break;
        }
        index += 1;
        let ts = ts_sec as f64 + ts_frac as f64 / frac_div;
        on_pkt(index, ts, &b[data_off..data_off + cl], link_type);
        off = data_off + cl;
    }
    Ok(("pcap", link_type_name(link_type)))
}

fn walk_pcapng(
    b: &[u8],
    on_pkt: &mut dyn FnMut(usize, f64, &[u8], u32),
) -> Result<(&'static str, String), String> {
    let bom = rd_u32_le(b, 8).ok_or("truncated pcapng section header")?;
    let le = match bom {
        0x1a2b_3c4d => true,
        0x4d3c_2b1a => false,
        _ => return Err("invalid pcapng byte-order magic".into()),
    };
    let r32 = |o: usize| if le { rd_u32_le(b, o) } else { rd_u32_be(b, o) };
    let r16 = |o: usize| if le { rd_u16_le(b, o) } else { rd_u16_be(b, o) };

    let mut if_linktypes: Vec<u32> = Vec::new();
    let mut if_tsresol: Vec<f64> = Vec::new();
    let mut off = 0usize;
    let mut index = 0usize;

    while off + 12 <= b.len() {
        let btype = r32(off).ok_or("truncated block")?;
        let blen = r32(off + 4).ok_or("truncated block")? as usize;
        if blen < 12 || off + blen > b.len() {
            break;
        }
        match btype {
            0x0a0d_0d0a => {}
            1 => {
                let body = &b[off + 8..off + blen - 4];
                let lt = if le { rd_u16_le(body, 0) } else { rd_u16_be(body, 0) }.unwrap_or(0) as u32;
                if_linktypes.push(lt);
                if_tsresol.push(pcapng_tsresol(body, 8, le).unwrap_or(1_000_000.0));
            }
            6 => {
                let if_id = r32(off + 8).unwrap_or(0) as usize;
                let ts_high = r32(off + 12).unwrap_or(0) as u64;
                let ts_low = r32(off + 16).unwrap_or(0) as u64;
                let cap_len = r32(off + 20).unwrap_or(0);
                let data_start = off + 28;
                let cl = cap_len as usize;
                if data_start + cl <= off + blen - 4 {
                    index += 1;
                    let div = if_tsresol.get(if_id).copied().unwrap_or(1_000_000.0);
                    let ts = ((ts_high << 32) | ts_low) as f64 / div;
                    let lt = if_linktypes.get(if_id).copied().unwrap_or(LINKTYPE_ETHERNET);
                    on_pkt(index, ts, &b[data_start..data_start + cl], lt);
                }
            }
            3 => {
                let orig_len = r32(off + 8).unwrap_or(0);
                let data_start = off + 12;
                let avail = (off + blen - 4).saturating_sub(data_start);
                let cl = (orig_len as usize).min(avail);
                if data_start + cl <= b.len() {
                    index += 1;
                    let lt = if_linktypes.first().copied().unwrap_or(LINKTYPE_ETHERNET);
                    on_pkt(index, 0.0, &b[data_start..data_start + cl], lt);
                }
            }
            2 => {
                let if_id = r16(off + 8).unwrap_or(0) as usize;
                let ts_high = r32(off + 12).unwrap_or(0) as u64;
                let ts_low = r32(off + 16).unwrap_or(0) as u64;
                let cap_len = r32(off + 20).unwrap_or(0);
                let data_start = off + 28;
                let cl = cap_len as usize;
                if data_start + cl <= off + blen - 4 {
                    index += 1;
                    let div = if_tsresol.get(if_id).copied().unwrap_or(1_000_000.0);
                    let ts = ((ts_high << 32) | ts_low) as f64 / div;
                    let lt = if_linktypes.get(if_id).copied().unwrap_or(LINKTYPE_ETHERNET);
                    on_pkt(index, ts, &b[data_start..data_start + cl], lt);
                }
            }
            _ => {}
        }
        off += blen;
    }
    let link_type = if_linktypes.first().copied().unwrap_or(LINKTYPE_ETHERNET);
    Ok(("pcapng", link_type_name(link_type)))
}

fn pcapng_tsresol(body: &[u8], start: usize, le: bool) -> Option<f64> {
    let mut o = start;
    while o + 4 <= body.len() {
        let code = if le { rd_u16_le(body, o) } else { rd_u16_be(body, o) }?;
        let len = if le { rd_u16_le(body, o + 2) } else { rd_u16_be(body, o + 2) }? as usize;
        if code == 0 {
            break;
        }
        if code == 9 && len >= 1 {
            let raw = *body.get(o + 4)?;
            let div = if raw & 0x80 != 0 {
                2f64.powi((raw & 0x7f) as i32)
            } else {
                10f64.powi(raw as i32)
            };
            return Some(div);
        }
        o += 4 + len.div_ceil(4) * 4;
    }
    None
}

// ---------------------------------------------------------------------------
// Per-packet decode: metadata + the application payload slice.
// ---------------------------------------------------------------------------

struct Decoded<'a> {
    protocol: String,
    source: String,
    destination: String,
    source_port: Option<u16>,
    destination_port: Option<u16>,
    flags: Option<String>,
    payload: &'a [u8],
}

fn fmt_mac(b: &[u8]) -> String {
    let mut s = String::new();
    for (i, x) in b.iter().take(6).enumerate() {
        if i > 0 {
            s.push(':');
        }
        let _ = write!(s, "{x:02x}");
    }
    s
}

fn fmt_ipv4(b: &[u8]) -> String {
    format!("{}.{}.{}.{}", b[0], b[1], b[2], b[3])
}

fn fmt_ipv6(b: &[u8]) -> String {
    let groups: Vec<u16> = (0..8).map(|i| u16::from_be_bytes([b[i * 2], b[i * 2 + 1]])).collect();
    let (mut best_start, mut best_len) = (usize::MAX, 0usize);
    let (mut i, mut cur_start, mut cur_len) = (0usize, 0usize, 0usize);
    while i < 8 {
        if groups[i] == 0 {
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
        i += 1;
    }
    if best_len < 2 {
        return groups.iter().map(|g| format!("{g:x}")).collect::<Vec<_>>().join(":");
    }
    let mut out = String::new();
    let mut idx = 0;
    while idx < 8 {
        if idx == best_start {
            out.push_str("::");
            idx += best_len;
            continue;
        }
        if !out.is_empty() && !out.ends_with(':') {
            out.push(':');
        }
        let _ = write!(out, "{:x}", groups[idx]);
        idx += 1;
    }
    out
}

const IPPROTO_ICMP: u8 = 1;
const IPPROTO_TCP: u8 = 6;
const IPPROTO_UDP: u8 = 17;
const IPPROTO_ICMPV6: u8 = 58;

fn proto_name(p: u8) -> &'static str {
    match p {
        IPPROTO_ICMP => "ICMP",
        2 => "IGMP",
        IPPROTO_TCP => "TCP",
        IPPROTO_UDP => "UDP",
        47 => "GRE",
        50 => "ESP",
        51 => "AH",
        IPPROTO_ICMPV6 => "ICMPv6",
        89 => "OSPF",
        132 => "SCTP",
        _ => "IP",
    }
}

fn tcp_flags(f: u8) -> String {
    let names = [
        (0x01, "FIN"),
        (0x02, "SYN"),
        (0x04, "RST"),
        (0x08, "PSH"),
        (0x10, "ACK"),
        (0x20, "URG"),
        (0x40, "ECE"),
        (0x80, "CWR"),
    ];
    let set: Vec<&str> = names.iter().filter(|(bit, _)| f & bit != 0).map(|(_, n)| *n).collect();
    if set.is_empty() {
        "none".into()
    } else {
        set.join(", ")
    }
}

#[derive(Clone, Copy)]
enum EtherType {
    Ipv4,
    Ipv6,
    Arp,
    Other(u16),
}

fn ethertype_kind(t: u16) -> EtherType {
    match t {
        0x0800 => EtherType::Ipv4,
        0x86dd => EtherType::Ipv6,
        0x0806 => EtherType::Arp,
        other => EtherType::Other(other),
    }
}

/// Decode link/network/transport layers, returning metadata and the payload slice.
fn decode(data: &[u8], link_type: u32) -> Decoded<'_> {
    let mut d = Decoded {
        protocol: "Unknown".into(),
        source: String::new(),
        destination: String::new(),
        source_port: None,
        destination_port: None,
        flags: None,
        payload: &[],
    };

    let l3: Option<(&[u8], EtherType)> = match link_type {
        LINKTYPE_ETHERNET => decode_ethernet(data, &mut d),
        LINKTYPE_RAW | LINKTYPE_RAW_ALT1 => data.first().map(|v| {
            if v >> 4 == 6 {
                (data, EtherType::Ipv6)
            } else {
                (data, EtherType::Ipv4)
            }
        }),
        LINKTYPE_NULL => {
            if data.len() >= 4 {
                let af = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let et = if af == 2 { EtherType::Ipv4 } else { EtherType::Ipv6 };
                Some((&data[4..], et))
            } else {
                None
            }
        }
        LINKTYPE_LINUX_SLL => {
            if data.len() >= 16 {
                let et = rd_u16_be(data, 14).unwrap_or(0);
                Some((&data[16..], ethertype_kind(et)))
            } else {
                None
            }
        }
        _ => None,
    };

    if let Some((l3data, et)) = l3 {
        match et {
            EtherType::Ipv4 => decode_ipv4(l3data, &mut d),
            EtherType::Ipv6 => decode_ipv6(l3data, &mut d),
            EtherType::Arp => {
                d.protocol = "ARP".into();
                d.payload = l3data;
            }
            EtherType::Other(t) => {
                d.protocol = format!("0x{t:04x}");
                d.payload = l3data;
            }
        }
    } else {
        // No decodable link layer: search the whole frame.
        d.payload = data;
    }
    d
}

fn decode_ethernet<'a>(data: &'a [u8], d: &mut Decoded) -> Option<(&'a [u8], EtherType)> {
    if data.len() < 14 {
        return None;
    }
    d.destination = fmt_mac(&data[0..6]);
    d.source = fmt_mac(&data[6..12]);
    let mut etype = rd_u16_be(data, 12)?;
    let mut payload_off = 14;
    while etype == 0x8100 || etype == 0x88a8 {
        if data.len() < payload_off + 4 {
            return None;
        }
        etype = rd_u16_be(data, payload_off + 2)?;
        payload_off += 4;
    }
    d.protocol = "Ethernet".into();
    Some((&data[payload_off..], ethertype_kind(etype)))
}

fn decode_ipv4<'a>(data: &'a [u8], d: &mut Decoded<'a>) {
    if data.len() < 20 {
        d.protocol = "IPv4".into();
        return;
    }
    let ihl = (data[0] & 0x0f) as usize * 4;
    let proto = data[9];
    d.source = fmt_ipv4(&data[12..16]);
    d.destination = fmt_ipv4(&data[16..20]);
    d.protocol = proto_name(proto).into();
    let l4 = if data.len() >= ihl { &data[ihl..] } else { &[] };
    decode_l4(proto, l4, d);
}

fn decode_ipv6<'a>(data: &'a [u8], d: &mut Decoded<'a>) {
    if data.len() < 40 {
        d.protocol = "IPv6".into();
        return;
    }
    let next_header = data[6];
    d.source = fmt_ipv6(&data[8..24]);
    d.destination = fmt_ipv6(&data[24..40]);
    d.protocol = proto_name(next_header).into();
    decode_l4(next_header, &data[40..], d);
}

fn decode_l4<'a>(proto: u8, l4: &'a [u8], d: &mut Decoded<'a>) {
    match proto {
        IPPROTO_TCP if l4.len() >= 20 => {
            d.source_port = rd_u16_be(l4, 0);
            d.destination_port = rd_u16_be(l4, 2);
            d.flags = Some(tcp_flags(l4[13]));
            d.protocol = "TCP".into();
            let data_off = ((l4[12] >> 4) as usize) * 4;
            d.payload = if l4.len() >= data_off { &l4[data_off..] } else { &[] };
        }
        IPPROTO_UDP if l4.len() >= 8 => {
            d.source_port = rd_u16_be(l4, 0);
            d.destination_port = rd_u16_be(l4, 2);
            d.protocol = "UDP".into();
            d.payload = &l4[8..];
        }
        IPPROTO_ICMP => {
            d.protocol = "ICMP".into();
            d.payload = l4;
        }
        IPPROTO_ICMPV6 => {
            d.protocol = "ICMPv6".into();
            d.payload = l4;
        }
        _ => {
            // Unknown transport: the L4 bytes ARE the payload to search.
            d.payload = l4;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a classic little-endian pcap with one Ethernet/IPv4/TCP packet whose
    /// TCP payload is `body`.
    fn build_pcap_tcp(body: &[u8], sport: u16, dport: u16) -> Vec<u8> {
        let mut f = Vec::new();
        f.extend_from_slice(&[0xd4, 0xc3, 0xb2, 0xa1]);
        f.extend_from_slice(&2u16.to_le_bytes());
        f.extend_from_slice(&4u16.to_le_bytes());
        f.extend_from_slice(&0u32.to_le_bytes());
        f.extend_from_slice(&0u32.to_le_bytes());
        f.extend_from_slice(&65535u32.to_le_bytes());
        f.extend_from_slice(&1u32.to_le_bytes()); // Ethernet

        let mut pkt = Vec::new();
        pkt.extend_from_slice(&[0x11, 0x22, 0x33, 0x44, 0x55, 0x66]);
        pkt.extend_from_slice(&[0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);
        pkt.extend_from_slice(&0x0800u16.to_be_bytes());
        let total_ip = 20 + 20 + body.len();
        let mut ip = vec![0x45, 0x00];
        ip.extend_from_slice(&(total_ip as u16).to_be_bytes());
        ip.extend_from_slice(&[0, 0, 0, 0]);
        ip.push(64);
        ip.push(6);
        ip.extend_from_slice(&0u16.to_be_bytes());
        ip.extend_from_slice(&[192, 168, 0, 1]);
        ip.extend_from_slice(&[192, 168, 0, 2]);
        pkt.extend_from_slice(&ip);
        let mut tcp = Vec::new();
        tcp.extend_from_slice(&sport.to_be_bytes());
        tcp.extend_from_slice(&dport.to_be_bytes());
        tcp.extend_from_slice(&0u32.to_be_bytes());
        tcp.extend_from_slice(&0u32.to_be_bytes());
        tcp.push(0x50); // data offset 5 words
        tcp.push(0x18); // PSH, ACK
        tcp.extend_from_slice(&8192u16.to_be_bytes());
        tcp.extend_from_slice(&0u16.to_be_bytes());
        tcp.extend_from_slice(&0u16.to_be_bytes());
        pkt.extend_from_slice(&tcp);
        pkt.extend_from_slice(body);

        f.extend_from_slice(&1_700_000_000u32.to_le_bytes());
        f.extend_from_slice(&0u32.to_le_bytes());
        f.extend_from_slice(&(pkt.len() as u32).to_le_bytes());
        f.extend_from_slice(&(pkt.len() as u32).to_le_bytes());
        f.extend_from_slice(&pkt);
        f
    }

    #[test]
    fn matches_http_get() {
        let f = build_pcap_tcp(b"GET /index.html HTTP/1.1\r\nHost: example.com\r\n\r\n", 4321, 80);
        let r = grep(&f, "GET /\\S+", &GrepOptions::default()).unwrap();
        assert_eq!(r.format, "pcap");
        assert_eq!(r.total_packets, 1);
        assert_eq!(r.scanned_packets, 1);
        assert_eq!(r.matched_packets, 1);
        let m = &r.matches[0];
        assert_eq!(m.protocol, "TCP");
        assert_eq!(m.destination_port, Some(80));
        assert_eq!(m.match_offset, Some(0));
        assert_eq!(m.matched_text.as_deref(), Some("GET /index.html"));
        assert_eq!(m.flags.as_deref(), Some("PSH, ACK"));
        assert!(m.payload_ascii.starts_with("GET /index.html"));
    }

    #[test]
    fn case_insensitive() {
        let f = build_pcap_tcp(b"User-Agent: curl/8.0", 5000, 443);
        let sensitive = grep(&f, "user-agent", &GrepOptions::default()).unwrap();
        assert_eq!(sensitive.matched_packets, 0);
        let opts = GrepOptions { ignore_case: true, ..Default::default() };
        let insensitive = grep(&f, "user-agent", &opts).unwrap();
        assert_eq!(insensitive.matched_packets, 1);
    }

    #[test]
    fn hex_pattern() {
        // "GET " == 47 45 54 20
        let f = build_pcap_tcp(b"GET / HTTP/1.0\r\n\r\n", 1111, 80);
        let opts = GrepOptions { hex: true, ..Default::default() };
        let r = grep(&f, "47 45 54 20", &opts).unwrap();
        assert_eq!(r.matched_packets, 1);
        assert_eq!(r.matches[0].match_offset, Some(0));
    }

    #[test]
    fn invert_match() {
        let f = build_pcap_tcp(b"POST /login HTTP/1.1\r\n", 2000, 80);
        let opts = GrepOptions { invert: true, ..Default::default() };
        let r = grep(&f, "GET ", &opts).unwrap();
        assert_eq!(r.matched_packets, 1);
        assert_eq!(r.matches[0].match_offset, None);
    }

    #[test]
    fn port_filter() {
        let f = build_pcap_tcp(b"hello world", 12345, 8080);
        let opts = GrepOptions { port: Some(53), ..Default::default() };
        let r = grep(&f, "hello", &opts).unwrap();
        assert_eq!(r.scanned_packets, 0);
        assert_eq!(r.matched_packets, 0);
        let opts2 = GrepOptions { port: Some(8080), ..Default::default() };
        let r2 = grep(&f, "hello", &opts2).unwrap();
        assert_eq!(r2.matched_packets, 1);
    }

    #[test]
    fn limit_truncates() {
        let one = build_pcap_tcp(b"needle here", 1000, 80);
        let (hdr, rec) = one.split_at(24);
        let mut f = hdr.to_vec();
        for _ in 0..3 {
            f.extend_from_slice(rec);
        }
        let opts = GrepOptions { limit: 2, ..Default::default() };
        let r = grep(&f, "needle", &opts).unwrap();
        assert_eq!(r.matched_packets, 3);
        assert_eq!(r.matches.len(), 2);
        assert!(r.truncated);
    }

    #[test]
    fn hex_dump_present() {
        let f = build_pcap_tcp(b"AB", 1000, 80);
        let opts = GrepOptions { show_hex: true, ..Default::default() };
        let r = grep(&f, "AB", &opts).unwrap();
        let dump = r.matches[0].payload_hex.as_deref().unwrap();
        assert!(dump.contains("41 42"));
        assert!(dump.contains("AB"));
    }

    #[test]
    fn rejects_non_pcap() {
        let err = grep(b"not a capture file at all", "x", &GrepOptions::default()).unwrap_err();
        assert!(err.contains("not a pcap"));
    }

    #[test]
    fn rejects_empty_pattern() {
        let f = build_pcap_tcp(b"data", 1, 2);
        let err = grep(&f, "", &GrepOptions::default()).unwrap_err();
        assert!(err.contains("empty"));
    }

    #[test]
    fn rejects_bad_regex() {
        let f = build_pcap_tcp(b"data", 1, 2);
        let err = grep(&f, "(unclosed", &GrepOptions::default()).unwrap_err();
        assert!(err.contains("invalid regular expression"));
    }

    #[test]
    fn rejects_odd_hex() {
        let f = build_pcap_tcp(b"data", 1, 2);
        let opts = GrepOptions { hex: true, ..Default::default() };
        let err = grep(&f, "abc", &opts).unwrap_err();
        assert!(err.contains("even number"));
    }
}

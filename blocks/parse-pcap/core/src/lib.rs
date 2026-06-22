//! parse-pcap core — pure, dependency-free parser for libpcap (`.pcap`) and
//! pcapng (`.pcapng`) capture files. Decodes the link layer (Ethernet) and the
//! common network/transport layers (IPv4, IPv6, TCP, UDP, ICMP/ICMPv6, ARP) into
//! a per-packet summary with a capture timestamp.
//!
//! No I/O, no external deps → instantiates on every backend including the chat
//! Service Worker. The classic-pcap and pcapng container formats are parsed
//! directly from the byte layout (a small, well-specified binary format).

use std::fmt::Write as _;

/// One decoded packet record.
#[derive(Debug, Clone, PartialEq)]
pub struct Packet {
    /// 1-based index in capture order.
    pub index: usize,
    /// Capture timestamp in seconds since the Unix epoch (fractional).
    pub timestamp: f64,
    /// Wire length of the original packet (bytes), if known.
    pub orig_len: u32,
    /// Number of captured bytes available in the file (may be < orig_len).
    pub cap_len: u32,
    /// Source address (IP, MAC, or "" when not decodable).
    pub src: String,
    /// Destination address.
    pub dst: String,
    /// Source transport port, when TCP/UDP.
    pub src_port: Option<u16>,
    /// Destination transport port, when TCP/UDP.
    pub dst_port: Option<u16>,
    /// Highest decoded protocol name (e.g. `TCP`, `UDP`, `ICMP`, `ARP`, `IPv6`).
    pub protocol: String,
    /// Human-readable one-line description of the decoded layers.
    pub info: String,
}

/// Result of parsing a whole capture file.
#[derive(Debug, Clone, PartialEq)]
pub struct Capture {
    /// Container format: `pcap` or `pcapng`.
    pub format: &'static str,
    /// Link-layer type name of the first interface (e.g. `Ethernet`).
    pub link_type: String,
    /// Decoded packets (capped at `max` by the caller).
    pub packets: Vec<Packet>,
    /// Total packets present in the file (may exceed `packets.len()` if capped).
    pub total_packets: usize,
    /// True when the packet list was truncated to a maximum.
    pub truncated: bool,
}

const LINKTYPE_ETHERNET: u32 = 1;
const LINKTYPE_RAW: u32 = 101;
const LINKTYPE_RAW_ALT1: u32 = 12; // older "RAW" value
const LINKTYPE_LINUX_SLL: u32 = 113;
const LINKTYPE_NULL: u32 = 0; // BSD loopback

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

/// Parse a capture file, decoding up to `max` packets.
pub fn parse(bytes: &[u8], max: usize) -> Result<Capture, String> {
    if bytes.len() < 4 {
        return Err("file is too small to be a pcap/pcapng capture".into());
    }
    let magic = [bytes[0], bytes[1], bytes[2], bytes[3]];
    match magic {
        // classic pcap, big- or little-endian (us or ns resolution variants)
        [0xa1, 0xb2, 0xc3, 0xd4] | [0xa1, 0xb2, 0x3c, 0x4d] => parse_classic(bytes, max, false),
        [0xd4, 0xc3, 0xb2, 0xa1] | [0x4d, 0x3c, 0xb2, 0xa1] => parse_classic(bytes, max, true),
        // pcapng Section Header Block starts with block type 0x0a0d0d0a
        [0x0a, 0x0d, 0x0d, 0x0a] => parse_pcapng(bytes, max),
        _ => Err(
            "not a pcap or pcapng capture (unrecognised magic bytes); expected a libpcap \
             (.pcap) or pcapng (.pcapng) file"
                .into(),
        ),
    }
}

/// Classic global header is 24 bytes; `swapped` true => little-endian fields.
fn parse_classic(b: &[u8], max: usize, swapped: bool) -> Result<Capture, String> {
    if b.len() < 24 {
        return Err("truncated pcap global header".into());
    }
    // magic a1b2c3d4 => seconds+microseconds; a1b23c4d => seconds+nanoseconds.
    let ns_magic = matches!(b[0..4], [0xa1, 0xb2, 0x3c, 0x4d] | [0x4d, 0x3c, 0xb2, 0xa1]);
    let frac_div = if ns_magic { 1_000_000_000.0 } else { 1_000_000.0 };

    let rd32 = |o: usize| if swapped { rd_u32_le(b, o) } else { rd_u32_be(b, o) };

    let link_type = rd32(20).ok_or("truncated pcap header")? & 0xffff;
    let mut packets = Vec::new();
    let mut total = 0usize;
    let mut off = 24usize;
    let mut truncated = false;
    while off + 16 <= b.len() {
        let ts_sec = rd32(off).unwrap();
        let ts_frac = rd32(off + 4).unwrap();
        let cap_len = rd32(off + 8).unwrap();
        let orig_len = rd32(off + 12).unwrap();
        let data_off = off + 16;
        let cl = cap_len as usize;
        if data_off + cl > b.len() {
            break; // truncated final record
        }
        total += 1;
        if packets.len() < max {
            let ts = ts_sec as f64 + ts_frac as f64 / frac_div;
            let pkt = decode_packet(
                packets.len() + 1,
                ts,
                orig_len,
                cap_len,
                link_type,
                &b[data_off..data_off + cl],
            );
            packets.push(pkt);
        } else {
            truncated = true;
        }
        off = data_off + cl;
    }
    Ok(Capture {
        format: "pcap",
        link_type: link_type_name(link_type),
        packets,
        total_packets: total,
        truncated,
    })
}

/// Minimal pcapng walk: read SHB byte-order, then iterate blocks. We track the
/// link type from Interface Description Blocks (IDB) and decode Enhanced/Simple
/// Packet Blocks (EPB=6, SPB=3) and the legacy Packet Block (=2).
fn parse_pcapng(b: &[u8], max: usize) -> Result<Capture, String> {
    // Section Header Block: type(4) len(4) byteorder-magic(4)...
    let bom = rd_u32_le(b, 8).ok_or("truncated pcapng section header")?;
    let le = match bom {
        0x1a2b_3c4d => true,
        0x4d3c_2b1a => false,
        _ => return Err("invalid pcapng byte-order magic".into()),
    };
    let r32 = |o: usize| if le { rd_u32_le(b, o) } else { rd_u32_be(b, o) };
    let r16 = |o: usize| if le { rd_u16_le(b, o) } else { rd_u16_be(b, o) };

    // Per-interface link type + timestamp resolution (default 1e6 = microseconds).
    let mut if_linktypes: Vec<u32> = Vec::new();
    let mut if_tsresol: Vec<f64> = Vec::new();
    let mut packets = Vec::new();
    let mut total = 0usize;
    let mut truncated = false;

    let mut off = 0usize;
    while off + 12 <= b.len() {
        let btype = r32(off).ok_or("truncated block")?;
        let blen = r32(off + 4).ok_or("truncated block")? as usize;
        if blen < 12 || off + blen > b.len() {
            break;
        }
        match btype {
            0x0a0d_0d0a => { /* SHB — new section; keep going */ }
            1 => {
                // IDB body begins at off+8: linktype(2) reserved(2) snaplen(4) options...
                let body = &b[off + 8..off + blen - 4];
                let lt = if le {
                    rd_u16_le(body, 0)
                } else {
                    rd_u16_be(body, 0)
                }
                .unwrap_or(0) as u32;
                if_linktypes.push(lt);
                let div = pcapng_tsresol(body, 8, le).unwrap_or(1_000_000.0);
                if_tsresol.push(div);
            }
            6 => {
                // EPB: interface_id(4) ts_high(4) ts_low(4) caplen(4) origlen(4) data...
                let if_id = r32(off + 8).unwrap_or(0) as usize;
                let ts_high = r32(off + 12).unwrap_or(0) as u64;
                let ts_low = r32(off + 16).unwrap_or(0) as u64;
                let cap_len = r32(off + 20).unwrap_or(0);
                let orig_len = r32(off + 24).unwrap_or(0);
                let data_start = off + 28;
                let cl = cap_len as usize;
                if data_start + cl <= off + blen - 4 {
                    total += 1;
                    if packets.len() < max {
                        let div = if_tsresol.get(if_id).copied().unwrap_or(1_000_000.0);
                        let ts = ((ts_high << 32) | ts_low) as f64 / div;
                        let lt = if_linktypes.get(if_id).copied().unwrap_or(LINKTYPE_ETHERNET);
                        packets.push(decode_packet(
                            packets.len() + 1,
                            ts,
                            orig_len,
                            cap_len,
                            lt,
                            &b[data_start..data_start + cl],
                        ));
                    } else {
                        truncated = true;
                    }
                }
            }
            3 => {
                // SPB: origlen(4) data... (caplen = available; no timestamp)
                let orig_len = r32(off + 8).unwrap_or(0);
                let data_start = off + 12;
                let avail = (off + blen - 4).saturating_sub(data_start);
                let cl = (orig_len as usize).min(avail);
                if data_start + cl <= b.len() {
                    total += 1;
                    if packets.len() < max {
                        let lt = if_linktypes.first().copied().unwrap_or(LINKTYPE_ETHERNET);
                        packets.push(decode_packet(
                            packets.len() + 1,
                            0.0,
                            orig_len,
                            cl as u32,
                            lt,
                            &b[data_start..data_start + cl],
                        ));
                    } else {
                        truncated = true;
                    }
                }
            }
            2 => {
                // legacy Packet Block: if_id(2) drops(2) ts_high(4) ts_low(4) caplen(4) origlen(4) data
                let if_id = r16(off + 8).unwrap_or(0) as usize;
                let ts_high = r32(off + 12).unwrap_or(0) as u64;
                let ts_low = r32(off + 16).unwrap_or(0) as u64;
                let cap_len = r32(off + 20).unwrap_or(0);
                let orig_len = r32(off + 24).unwrap_or(0);
                let data_start = off + 28;
                let cl = cap_len as usize;
                if data_start + cl <= off + blen - 4 {
                    total += 1;
                    if packets.len() < max {
                        let div = if_tsresol.get(if_id).copied().unwrap_or(1_000_000.0);
                        let ts = ((ts_high << 32) | ts_low) as f64 / div;
                        let lt = if_linktypes.get(if_id).copied().unwrap_or(LINKTYPE_ETHERNET);
                        packets.push(decode_packet(
                            packets.len() + 1,
                            ts,
                            orig_len,
                            cap_len,
                            lt,
                            &b[data_start..data_start + cl],
                        ));
                    } else {
                        truncated = true;
                    }
                }
            }
            _ => { /* skip unknown blocks */ }
        }
        off += blen;
    }

    let link_type = if_linktypes.first().copied().unwrap_or(LINKTYPE_ETHERNET);
    Ok(Capture {
        format: "pcapng",
        link_type: link_type_name(link_type),
        packets,
        total_packets: total,
        truncated,
    })
}

/// Scan pcapng options starting at `start` for if_tsresol (code 9). Returns the
/// timestamp divisor (e.g. 1e6 for microseconds, 1e9 for nanoseconds).
fn pcapng_tsresol(body: &[u8], start: usize, le: bool) -> Option<f64> {
    let mut o = start;
    while o + 4 <= body.len() {
        let code = if le {
            rd_u16_le(body, o)
        } else {
            rd_u16_be(body, o)
        }?;
        let len = if le {
            rd_u16_le(body, o + 2)
        } else {
            rd_u16_be(body, o + 2)
        }? as usize;
        if code == 0 {
            break; // opt_endofopt
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
        // options are padded to a 4-byte boundary
        o += 4 + len.div_ceil(4) * 4;
    }
    None
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
    // RFC 5952-ish: join 8 groups, then collapse the longest run of zero groups.
    let groups: Vec<u16> = (0..8)
        .map(|i| u16::from_be_bytes([b[i * 2], b[i * 2 + 1]]))
        .collect();
    // find longest run of zeros (len >= 2)
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
        return groups
            .iter()
            .map(|g| format!("{g:x}"))
            .collect::<Vec<_>>()
            .join(":");
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

/// Decode a single packet body according to the link type.
fn decode_packet(
    index: usize,
    ts: f64,
    orig_len: u32,
    cap_len: u32,
    link_type: u32,
    data: &[u8],
) -> Packet {
    let mut pkt = Packet {
        index,
        timestamp: ts,
        orig_len,
        cap_len,
        src: String::new(),
        dst: String::new(),
        src_port: None,
        dst_port: None,
        protocol: "Unknown".into(),
        info: String::new(),
    };

    let l3: Option<(&[u8], EtherType)> = match link_type {
        LINKTYPE_ETHERNET => decode_ethernet(data, &mut pkt),
        LINKTYPE_RAW | LINKTYPE_RAW_ALT1 => {
            // Raw IP: peek IP version.
            data.first().map(|v| {
                if v >> 4 == 6 {
                    (data, EtherType::Ipv6)
                } else {
                    (data, EtherType::Ipv4)
                }
            })
        }
        LINKTYPE_NULL => {
            // 4-byte BSD address family header (host byte order; 2 => AF_INET).
            if data.len() >= 4 {
                let af = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let et = if af == 2 {
                    EtherType::Ipv4
                } else {
                    EtherType::Ipv6
                };
                Some((&data[4..], et))
            } else {
                None
            }
        }
        LINKTYPE_LINUX_SLL => {
            // 16-byte SLL header; ethertype at offset 14.
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
            EtherType::Ipv4 => decode_ipv4(l3data, &mut pkt),
            EtherType::Ipv6 => decode_ipv6(l3data, &mut pkt),
            EtherType::Arp => decode_arp(l3data, &mut pkt),
            EtherType::Other(t) => {
                pkt.protocol = format!("0x{t:04x}");
                if pkt.info.is_empty() {
                    pkt.info = format!("Ethernet, EtherType 0x{t:04x}");
                }
            }
        }
    }

    if pkt.info.is_empty() {
        pkt.info = format!("{} bytes captured", cap_len);
    }
    pkt
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

/// Decode an Ethernet II frame, filling MAC addresses, returning the L3 payload.
fn decode_ethernet<'a>(data: &'a [u8], pkt: &mut Packet) -> Option<(&'a [u8], EtherType)> {
    if data.len() < 14 {
        return None;
    }
    pkt.dst = fmt_mac(&data[0..6]);
    pkt.src = fmt_mac(&data[6..12]);
    let mut etype = rd_u16_be(data, 12)?;
    let mut payload_off = 14;
    // 802.1Q / 802.1ad VLAN tag(s)
    while etype == 0x8100 || etype == 0x88a8 {
        if data.len() < payload_off + 4 {
            return None;
        }
        etype = rd_u16_be(data, payload_off + 2)?;
        payload_off += 4;
    }
    pkt.protocol = "Ethernet".into();
    Some((&data[payload_off..], ethertype_kind(etype)))
}

fn decode_ipv4(data: &[u8], pkt: &mut Packet) {
    if data.len() < 20 {
        pkt.protocol = "IPv4".into();
        pkt.info = "truncated IPv4 header".into();
        return;
    }
    let ihl = (data[0] & 0x0f) as usize * 4;
    let proto = data[9];
    pkt.src = fmt_ipv4(&data[12..16]);
    pkt.dst = fmt_ipv4(&data[16..20]);
    pkt.protocol = proto_name(proto).into();
    let l4 = if data.len() >= ihl { &data[ihl..] } else { &[] };
    decode_l4(proto, l4, pkt, false);
}

fn decode_ipv6(data: &[u8], pkt: &mut Packet) {
    if data.len() < 40 {
        pkt.protocol = "IPv6".into();
        pkt.info = "truncated IPv6 header".into();
        return;
    }
    let next_header = data[6];
    pkt.src = fmt_ipv6(&data[8..24]);
    pkt.dst = fmt_ipv6(&data[24..40]);
    pkt.protocol = proto_name(next_header).into();
    decode_l4(next_header, &data[40..], pkt, true);
}

fn decode_l4(proto: u8, l4: &[u8], pkt: &mut Packet, v6: bool) {
    match proto {
        IPPROTO_TCP if l4.len() >= 20 => {
            let sp = rd_u16_be(l4, 0).unwrap();
            let dp = rd_u16_be(l4, 2).unwrap();
            pkt.src_port = Some(sp);
            pkt.dst_port = Some(dp);
            let f = tcp_flags(l4[13]);
            pkt.protocol = "TCP".into();
            pkt.info = format!("TCP {sp} -> {dp} [{f}]");
        }
        IPPROTO_UDP if l4.len() >= 8 => {
            let sp = rd_u16_be(l4, 0).unwrap();
            let dp = rd_u16_be(l4, 2).unwrap();
            pkt.src_port = Some(sp);
            pkt.dst_port = Some(dp);
            pkt.protocol = "UDP".into();
            pkt.info = format!("UDP {sp} -> {dp}");
        }
        IPPROTO_ICMP if !v6 => {
            pkt.protocol = "ICMP".into();
            let t = l4.first().copied().unwrap_or(255);
            pkt.info = format!("ICMP type {t}");
        }
        IPPROTO_ICMPV6 if v6 => {
            pkt.protocol = "ICMPv6".into();
            let t = l4.first().copied().unwrap_or(255);
            pkt.info = format!("ICMPv6 type {t}");
        }
        _ => {
            if pkt.info.is_empty() {
                pkt.info = format!("{} {} -> {}", pkt.protocol, pkt.src, pkt.dst);
            }
        }
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
    let set: Vec<&str> = names
        .iter()
        .filter(|(bit, _)| f & bit != 0)
        .map(|(_, n)| *n)
        .collect();
    if set.is_empty() {
        "none".into()
    } else {
        set.join(", ")
    }
}

fn decode_arp(data: &[u8], pkt: &mut Packet) {
    pkt.protocol = "ARP".into();
    if data.len() < 28 {
        pkt.info = "ARP (truncated)".into();
        return;
    }
    let op = rd_u16_be(data, 6).unwrap_or(0);
    let sender_mac = fmt_mac(&data[8..14]);
    let sender_ip = fmt_ipv4(&data[14..18]);
    let target_ip = fmt_ipv4(&data[24..28]);
    pkt.src = sender_mac.clone();
    pkt.dst = fmt_mac(&data[18..24]);
    pkt.info = match op {
        1 => format!("ARP request: who has {target_ip}? tell {sender_ip}"),
        2 => format!("ARP reply: {sender_ip} is at {sender_mac}"),
        _ => format!("ARP opcode {op}"),
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a tiny classic little-endian pcap with one Ethernet/IPv4/TCP packet.
    fn build_pcap_one_tcp() -> Vec<u8> {
        let mut f = Vec::new();
        // global header (little-endian magic d4c3b2a1, microseconds)
        f.extend_from_slice(&[0xd4, 0xc3, 0xb2, 0xa1]);
        f.extend_from_slice(&2u16.to_le_bytes()); // major
        f.extend_from_slice(&4u16.to_le_bytes()); // minor
        f.extend_from_slice(&0u32.to_le_bytes()); // thiszone
        f.extend_from_slice(&0u32.to_le_bytes()); // sigfigs
        f.extend_from_slice(&65535u32.to_le_bytes()); // snaplen
        f.extend_from_slice(&1u32.to_le_bytes()); // linktype Ethernet

        // build packet: Ethernet + IPv4 + TCP
        let mut pkt = Vec::new();
        pkt.extend_from_slice(&[0x11, 0x22, 0x33, 0x44, 0x55, 0x66]); // dst mac
        pkt.extend_from_slice(&[0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]); // src mac
        pkt.extend_from_slice(&0x0800u16.to_be_bytes()); // ethertype IPv4
        let mut ip = vec![0x45, 0x00];
        ip.extend_from_slice(&40u16.to_be_bytes());
        ip.extend_from_slice(&[0, 0, 0, 0]);
        ip.push(64);
        ip.push(6); // TCP
        ip.extend_from_slice(&0u16.to_be_bytes());
        ip.extend_from_slice(&[192, 168, 0, 1]);
        ip.extend_from_slice(&[192, 168, 0, 2]);
        pkt.extend_from_slice(&ip);
        let mut tcp = Vec::new();
        tcp.extend_from_slice(&1234u16.to_be_bytes());
        tcp.extend_from_slice(&80u16.to_be_bytes());
        tcp.extend_from_slice(&0u32.to_be_bytes());
        tcp.extend_from_slice(&0u32.to_be_bytes());
        tcp.push(0x50);
        tcp.push(0x02); // SYN
        tcp.extend_from_slice(&8192u16.to_be_bytes());
        tcp.extend_from_slice(&0u16.to_be_bytes());
        tcp.extend_from_slice(&0u16.to_be_bytes());
        pkt.extend_from_slice(&tcp);

        f.extend_from_slice(&1_700_000_000u32.to_le_bytes()); // ts_sec
        f.extend_from_slice(&500_000u32.to_le_bytes()); // ts_usec
        f.extend_from_slice(&(pkt.len() as u32).to_le_bytes()); // caplen
        f.extend_from_slice(&(pkt.len() as u32).to_le_bytes()); // origlen
        f.extend_from_slice(&pkt);
        f
    }

    #[test]
    fn parses_classic_tcp() {
        let f = build_pcap_one_tcp();
        let cap = parse(&f, 100).unwrap();
        assert_eq!(cap.format, "pcap");
        assert_eq!(cap.link_type, "Ethernet");
        assert_eq!(cap.total_packets, 1);
        assert_eq!(cap.packets.len(), 1);
        let p = &cap.packets[0];
        assert_eq!(p.src, "192.168.0.1");
        assert_eq!(p.dst, "192.168.0.2");
        assert_eq!(p.protocol, "TCP");
        assert_eq!(p.src_port, Some(1234));
        assert_eq!(p.dst_port, Some(80));
        assert!(p.info.contains("SYN"));
        assert!((p.timestamp - 1_700_000_000.5).abs() < 1e-6);
    }

    #[test]
    fn truncation_caps_packets() {
        let one = build_pcap_one_tcp();
        let (hdr, rec) = one.split_at(24);
        let mut f = hdr.to_vec();
        for _ in 0..3 {
            f.extend_from_slice(rec);
        }
        let cap = parse(&f, 2).unwrap();
        assert_eq!(cap.total_packets, 3);
        assert_eq!(cap.packets.len(), 2);
        assert!(cap.truncated);
    }

    #[test]
    fn rejects_non_pcap() {
        let err = parse(b"not a capture file at all", 10).unwrap_err();
        assert!(err.contains("not a pcap"));
    }

    #[test]
    fn ipv6_formatting() {
        let b = [0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];
        assert_eq!(fmt_ipv6(&b), "2001:db8::1");
        let loop6 = [0u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];
        assert_eq!(fmt_ipv6(&loop6), "::1");
    }

    #[test]
    fn tcp_flag_names() {
        assert_eq!(tcp_flags(0x12), "SYN, ACK");
        assert_eq!(tcp_flags(0x10), "ACK");
        assert_eq!(tcp_flags(0x00), "none");
    }

    /// Minimal pcapng with SHB + IDB + EPB (one UDP packet).
    fn build_pcapng_udp() -> Vec<u8> {
        fn pad4(v: &mut Vec<u8>) {
            while v.len() % 4 != 0 {
                v.push(0);
            }
        }
        let mut f = Vec::new();
        // SHB
        let mut shb = Vec::new();
        shb.extend_from_slice(&0x0a0d0d0au32.to_le_bytes());
        let shb_len_pos = shb.len();
        shb.extend_from_slice(&0u32.to_le_bytes()); // placeholder
        shb.extend_from_slice(&0x1a2b3c4du32.to_le_bytes()); // BOM
        shb.extend_from_slice(&1u16.to_le_bytes()); // major
        shb.extend_from_slice(&0u16.to_le_bytes()); // minor
        shb.extend_from_slice(&(-1i64).to_le_bytes()); // section length unknown
        let total = (shb.len() + 4) as u32;
        shb[shb_len_pos..shb_len_pos + 4].copy_from_slice(&total.to_le_bytes());
        shb.extend_from_slice(&total.to_le_bytes());
        f.extend_from_slice(&shb);

        // IDB
        let mut idb = Vec::new();
        idb.extend_from_slice(&1u32.to_le_bytes());
        let idb_len_pos = idb.len();
        idb.extend_from_slice(&0u32.to_le_bytes()); // placeholder
        idb.extend_from_slice(&1u16.to_le_bytes()); // linktype Ethernet
        idb.extend_from_slice(&0u16.to_le_bytes()); // reserved
        idb.extend_from_slice(&65535u32.to_le_bytes()); // snaplen
        let total = (idb.len() + 4) as u32;
        idb[idb_len_pos..idb_len_pos + 4].copy_from_slice(&total.to_le_bytes());
        idb.extend_from_slice(&total.to_le_bytes());
        f.extend_from_slice(&idb);

        // packet: Ethernet + IPv4 + UDP
        let mut pkt = Vec::new();
        pkt.extend_from_slice(&[1, 2, 3, 4, 5, 6]);
        pkt.extend_from_slice(&[7, 8, 9, 10, 11, 12]);
        pkt.extend_from_slice(&0x0800u16.to_be_bytes());
        let mut ip = vec![0x45, 0x00];
        ip.extend_from_slice(&28u16.to_be_bytes());
        ip.extend_from_slice(&[0, 0, 0, 0]);
        ip.push(64);
        ip.push(17); // UDP
        ip.extend_from_slice(&0u16.to_be_bytes());
        ip.extend_from_slice(&[10, 0, 0, 1]);
        ip.extend_from_slice(&[10, 0, 0, 2]);
        pkt.extend_from_slice(&ip);
        pkt.extend_from_slice(&53u16.to_be_bytes());
        pkt.extend_from_slice(&53u16.to_be_bytes());
        pkt.extend_from_slice(&8u16.to_be_bytes());
        pkt.extend_from_slice(&0u16.to_be_bytes());

        // EPB
        let mut epb = Vec::new();
        epb.extend_from_slice(&6u32.to_le_bytes());
        let epb_len_pos = epb.len();
        epb.extend_from_slice(&0u32.to_le_bytes()); // placeholder
        epb.extend_from_slice(&0u32.to_le_bytes()); // interface id
        epb.extend_from_slice(&0u32.to_le_bytes()); // ts high
        epb.extend_from_slice(&0u32.to_le_bytes()); // ts low
        epb.extend_from_slice(&(pkt.len() as u32).to_le_bytes()); // caplen
        epb.extend_from_slice(&(pkt.len() as u32).to_le_bytes()); // origlen
        epb.extend_from_slice(&pkt);
        pad4(&mut epb);
        let total = (epb.len() + 4) as u32;
        epb[epb_len_pos..epb_len_pos + 4].copy_from_slice(&total.to_le_bytes());
        epb.extend_from_slice(&total.to_le_bytes());
        f.extend_from_slice(&epb);
        f
    }

    #[test]
    fn parses_pcapng_udp() {
        let f = build_pcapng_udp();
        let cap = parse(&f, 100).unwrap();
        assert_eq!(cap.format, "pcapng");
        assert_eq!(cap.link_type, "Ethernet");
        assert_eq!(cap.total_packets, 1);
        let p = &cap.packets[0];
        assert_eq!(p.protocol, "UDP");
        assert_eq!(p.src, "10.0.0.1");
        assert_eq!(p.dst, "10.0.0.2");
        assert_eq!(p.src_port, Some(53));
        assert_eq!(p.dst_port, Some(53));
    }
}

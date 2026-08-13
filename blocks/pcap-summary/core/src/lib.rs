//! pcap-summary core — pure, dependency-free capture-statistics aggregator for
//! libpcap (`.pcap`) and pcapng (`.pcapng`) captures.
//!
//! Answers "what does this capture consist of, and who is doing the talking":
//!   * `overview`      — capture properties: format, link type, snaplen, packet
//!                       and byte totals, first/last timestamp, duration,
//!                       average packet size, packet rate, bit rate, truncation,
//!   * `protocols`     — per-protocol packet/byte counts with percentages,
//!   * `hierarchy`     — colon-joined layer paths (`eth:ipv4:tcp:https`), the flat
//!                       equivalent of a protocol-hierarchy tree,
//!   * `talkers`       — per-IP endpoints with sent/received split,
//!   * `mac_talkers`   — the same at the Ethernet layer (Ethernet captures only),
//!   * `conversations` — endpoint pairs with per-direction counts, relative start
//!                       and duration,
//!   * `ports`         — busiest service ports with an optional service name.
//!
//! The container is walked straight from its byte layout (no external deps → runs
//! on every backend, including the chat Service Worker). Decoded link layers are
//! Ethernet (incl. stacked VLAN tags), raw IP, Linux cooked and null/loopback;
//! other link types still count toward the overview totals. Encrypted payloads
//! are never decrypted — they are named by port only. Only the first IP fragment
//! carries transport headers, so later fragments count toward the IP and talker
//! totals but not toward the port or conversation tables.
//!
//! Sibling blocks deliberately not duplicated here: `parse-pcap` (per-packet
//! dump), `pcap-grep` (payload search), `pcap-file-extractor` (object carving),
//! `pcap-network-forensics` (DNS/HTTP/credential artefacts).

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

/// Which column the ranked tables are ordered by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortBy {
    /// Rank by total bytes (volume) — the default.
    Bytes,
    /// Rank by total packets.
    Packets,
}

/// Aggregation knobs.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Maximum rows returned per ranked list.
    pub top: usize,
    /// Ranking column.
    pub sort_by: SortBy,
    /// Name well-known ports (`443` → `https`) in `ports` and `hierarchy`.
    pub resolve_ports: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options { top: 10, sort_by: SortBy::Bytes, resolve_ports: true }
    }
}

/// Capture-level properties (the `capinfos`-style header block).
#[derive(Debug, Clone, PartialEq)]
pub struct Overview {
    /// Container format: `pcap` or `pcapng`.
    pub format: &'static str,
    /// Link-layer type of the first interface (e.g. `Ethernet`).
    pub link_type: String,
    /// Per-packet capture limit recorded in the file header (0 when unknown).
    pub snaplen: u32,
    /// Packets present in the capture.
    pub packets: u64,
    /// Sum of the original on-wire packet lengths.
    pub bytes: u64,
    /// Sum of the bytes actually stored in the file.
    pub captured_bytes: u64,
    /// Packets whose stored bytes were fewer than their on-wire length.
    pub truncated_packets: u64,
    /// Packets whose network layer (IPv4/IPv6) was decoded.
    pub decoded_packets: u64,
    /// Timestamp of the first packet, seconds since the Unix epoch (0 if absent).
    pub first_timestamp: f64,
    /// Timestamp of the last packet, seconds since the Unix epoch (0 if absent).
    pub last_timestamp: f64,
    /// Capture span in seconds (0 when the capture carries no timestamps).
    pub duration_seconds: f64,
    /// Mean on-wire packet length in bytes.
    pub average_packet_size_bytes: f64,
    /// Mean packet rate over the capture span (0 when the duration is 0).
    pub packets_per_second: f64,
    /// Mean bit rate over the capture span (0 when the duration is 0).
    pub bits_per_second: f64,
}

/// One protocol's share of the capture.
#[derive(Debug, Clone, PartialEq)]
pub struct ProtocolStat {
    /// Lower-case layer name (`eth`, `ipv4`, `tcp`, `https`, `vlan`, `fragment`).
    pub protocol: String,
    pub packets: u64,
    pub bytes: u64,
    /// Share of all packets, percent (2 decimals).
    pub packet_percent: f64,
    /// Share of all bytes, percent (2 decimals).
    pub byte_percent: f64,
}

/// One full layer path, the flat equivalent of a protocol-hierarchy tree node.
#[derive(Debug, Clone, PartialEq)]
pub struct HierarchyStat {
    /// Colon-joined layer path, e.g. `eth:ipv4:tcp:https`.
    pub path: String,
    pub packets: u64,
    pub bytes: u64,
}

/// An endpoint (IP or MAC) with its directional split.
#[derive(Debug, Clone, PartialEq)]
pub struct Talker {
    /// IPv4/IPv6 address, or MAC address for `mac_talkers`.
    pub address: String,
    /// `packets_sent + packets_received`.
    pub packets: u64,
    /// `bytes_sent + bytes_received`.
    pub bytes: u64,
    pub packets_sent: u64,
    pub bytes_sent: u64,
    pub packets_received: u64,
    pub bytes_received: u64,
}

/// A conversation between two endpoints, with both directions kept apart.
#[derive(Debug, Clone, PartialEq)]
pub struct Conversation {
    /// `TCP`, `UDP`, or the IP protocol name (e.g. `ICMP`) when there are no ports.
    pub protocol: String,
    /// Lower-sorting endpoint (`ip:port`, or bare `ip` without ports).
    pub endpoint_a: String,
    /// Higher-sorting endpoint.
    pub endpoint_b: String,
    pub packets: u64,
    pub bytes: u64,
    pub packets_a_to_b: u64,
    pub bytes_a_to_b: u64,
    pub packets_b_to_a: u64,
    pub bytes_b_to_a: u64,
    /// Seconds from the first packet of the capture to the first of this flow.
    pub start_seconds: f64,
    /// Seconds between this flow's first and last packet.
    pub duration_seconds: f64,
}

/// A busy service port.
#[derive(Debug, Clone, PartialEq)]
pub struct PortStat {
    /// Transport protocol: `TCP` or `UDP`.
    pub protocol: &'static str,
    /// The service-side port number.
    pub port: u16,
    /// Well-known service name, when `resolve_ports` is on and the port is known.
    pub service: Option<String>,
    pub packets: u64,
    pub bytes: u64,
    /// Distinct `ip:port` endpoints observed on the service side of this port.
    pub endpoints: usize,
}

/// The complete summary. Each `*_total` is the row count before `top` truncated
/// the corresponding list.
#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    pub overview: Overview,
    /// True when at least one Ethernet frame was seen (gates `mac_talkers`).
    pub ethernet: bool,
    pub protocols_total: usize,
    pub protocols: Vec<ProtocolStat>,
    pub hierarchy_total: usize,
    pub hierarchy: Vec<HierarchyStat>,
    pub talkers_total: usize,
    pub talkers: Vec<Talker>,
    pub mac_talkers_total: usize,
    pub mac_talkers: Vec<Talker>,
    pub conversations_total: usize,
    pub conversations: Vec<Conversation>,
    pub ports_total: usize,
    pub ports: Vec<PortStat>,
}

const LINKTYPE_NULL: u32 = 0;
const LINKTYPE_ETHERNET: u32 = 1;
const LINKTYPE_RAW_ALT1: u32 = 12;
const LINKTYPE_RAW: u32 = 101;
const LINKTYPE_LINUX_SLL: u32 = 113;

const IPPROTO_TCP: u8 = 6;
const IPPROTO_UDP: u8 = 17;

fn link_type_name(lt: u32) -> String {
    match lt {
        LINKTYPE_NULL => "Null/Loopback".into(),
        LINKTYPE_ETHERNET => "Ethernet".into(),
        LINKTYPE_RAW_ALT1 | LINKTYPE_RAW => "Raw IP".into(),
        LINKTYPE_LINUX_SLL => "Linux cooked".into(),
        other => format!("LINKTYPE_{other}"),
    }
}

/// Short lower-case layer label used inside the hierarchy paths.
fn link_layer_label(lt: u32) -> String {
    match lt {
        LINKTYPE_NULL => "loop".into(),
        LINKTYPE_ETHERNET => "eth".into(),
        LINKTYPE_RAW_ALT1 | LINKTYPE_RAW => "rawip".into(),
        LINKTYPE_LINUX_SLL => "sll".into(),
        other => format!("linktype_{other}"),
    }
}

fn rd_u16_le(b: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*b.get(o)?, *b.get(o + 1)?]))
}
fn rd_u32_le(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes([*b.get(o)?, *b.get(o + 1)?, *b.get(o + 2)?, *b.get(o + 3)?]))
}
fn rd_u16_be(b: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*b.get(o)?, *b.get(o + 1)?]))
}
fn rd_u32_be(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_be_bytes([*b.get(o)?, *b.get(o + 1)?, *b.get(o + 2)?, *b.get(o + 3)?]))
}

fn round(v: f64, places: i32) -> f64 {
    let f = 10f64.powi(places);
    let r = (v * f).round() / f;
    // `-0.0` would serialize as `-0.0`; normalise it away.
    if r == 0.0 {
        0.0
    } else {
        r
    }
}

fn percent(n: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        round(n as f64 * 100.0 / total as f64, 2)
    }
}

/// Aggregate a whole capture into ranked summary tables.
pub fn analyze(bytes: &[u8], opts: &Options) -> Result<Summary, String> {
    if bytes.len() < 4 {
        return Err("file is too small to be a pcap/pcapng capture".into());
    }
    let mut agg = Aggregator { resolve_ports: opts.resolve_ports, ..Aggregator::default() };
    match [bytes[0], bytes[1], bytes[2], bytes[3]] {
        [0xa1, 0xb2, 0xc3, 0xd4] | [0xa1, 0xb2, 0x3c, 0x4d] => walk_classic(bytes, false, &mut agg)?,
        [0xd4, 0xc3, 0xb2, 0xa1] | [0x4d, 0x3c, 0xb2, 0xa1] => walk_classic(bytes, true, &mut agg)?,
        [0x0a, 0x0d, 0x0d, 0x0a] => walk_pcapng(bytes, &mut agg)?,
        _ => {
            return Err("not a pcap or pcapng capture (unrecognised magic bytes); expected a \
                        libpcap (.pcap) or pcapng (.pcapng) file"
                .into())
        }
    }
    Ok(agg.finish(opts))
}

/// Classic pcap: 24-byte global header, then 16-byte record headers + data.
fn walk_classic(b: &[u8], swapped: bool, agg: &mut Aggregator) -> Result<(), String> {
    if b.len() < 24 {
        return Err("truncated pcap global header".into());
    }
    let ns_magic = matches!(b[0..4], [0xa1, 0xb2, 0x3c, 0x4d] | [0x4d, 0x3c, 0xb2, 0xa1]);
    let frac_div = if ns_magic { 1_000_000_000.0 } else { 1_000_000.0 };
    let rd32 = |o: usize| if swapped { rd_u32_le(b, o) } else { rd_u32_be(b, o) };
    let link_type = rd32(20).ok_or("truncated pcap header")? & 0xffff;
    agg.format = "pcap";
    agg.link_type = link_type_name(link_type);
    agg.snaplen = rd32(16).unwrap_or(0);

    let mut off = 24usize;
    while off + 16 <= b.len() {
        let ts_sec = rd32(off).unwrap();
        let ts_frac = rd32(off + 4).unwrap();
        let cap_len = rd32(off + 8).unwrap() as usize;
        let orig_len = rd32(off + 12).unwrap();
        let data_off = off + 16;
        if data_off + cap_len > b.len() {
            break;
        }
        let ts = ts_sec as f64 + ts_frac as f64 / frac_div;
        agg.add_packet(ts, orig_len, cap_len as u32, link_type, &b[data_off..data_off + cap_len]);
        off = data_off + cap_len;
    }
    Ok(())
}

/// pcapng: iterate blocks, tracking per-interface link type + timestamp scale.
fn walk_pcapng(b: &[u8], agg: &mut Aggregator) -> Result<(), String> {
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
    agg.format = "pcapng";

    let mut off = 0usize;
    while off + 12 <= b.len() {
        let btype = r32(off).ok_or("truncated block")?;
        let blen = r32(off + 4).ok_or("truncated block")? as usize;
        if blen < 12 || off + blen > b.len() {
            break;
        }
        match btype {
            0x0a0d_0d0a => {}
            // Interface Description Block: link type, snaplen, ts resolution.
            1 => {
                let body = &b[off + 8..off + blen - 4];
                let lt = if le { rd_u16_le(body, 0) } else { rd_u16_be(body, 0) }.unwrap_or(0) as u32;
                if if_linktypes.is_empty() {
                    agg.link_type = link_type_name(lt);
                    agg.snaplen =
                        if le { rd_u32_le(body, 4) } else { rd_u32_be(body, 4) }.unwrap_or(0);
                }
                if_linktypes.push(lt);
                if_tsresol.push(pcapng_tsresol(body, 8, le).unwrap_or(1_000_000.0));
            }
            // Enhanced Packet Block / obsolete Packet Block — same field order
            // after the interface id (u32 vs u16 + u16 drop count).
            6 | 2 => {
                let if_id = if btype == 6 {
                    r32(off + 8).unwrap_or(0) as usize
                } else {
                    r16(off + 8).unwrap_or(0) as usize
                };
                let ts_high = r32(off + 12).unwrap_or(0) as u64;
                let ts_low = r32(off + 16).unwrap_or(0) as u64;
                let cap_len = r32(off + 20).unwrap_or(0) as usize;
                let orig_len = r32(off + 24).unwrap_or(0);
                let data_start = off + 28;
                if data_start + cap_len <= off + blen - 4 {
                    let div = if_tsresol.get(if_id).copied().unwrap_or(1_000_000.0);
                    let ts = ((ts_high << 32) | ts_low) as f64 / div;
                    let lt = if_linktypes.get(if_id).copied().unwrap_or(LINKTYPE_ETHERNET);
                    agg.add_packet(
                        ts,
                        orig_len,
                        cap_len as u32,
                        lt,
                        &b[data_start..data_start + cap_len],
                    );
                }
            }
            // Simple Packet Block: original length only, no timestamp.
            3 => {
                let orig_len = r32(off + 8).unwrap_or(0);
                let data_start = off + 12;
                let avail = (off + blen - 4).saturating_sub(data_start);
                let cl = (orig_len as usize).min(avail);
                if data_start + cl <= b.len() {
                    let lt = if_linktypes.first().copied().unwrap_or(LINKTYPE_ETHERNET);
                    agg.add_packet(0.0, orig_len, cl as u32, lt, &b[data_start..data_start + cl]);
                }
            }
            _ => {}
        }
        off += blen;
    }
    if agg.link_type.is_empty() {
        agg.link_type = link_type_name(LINKTYPE_ETHERNET);
    }
    Ok(())
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
            let div =
                if raw & 0x80 != 0 { 2f64.powi((raw & 0x7f) as i32) } else { 10f64.powi(raw as i32) };
            return Some(div);
        }
        o += 4 + len.div_ceil(4) * 4;
    }
    None
}

// ---------------------------------------------------------------------------
// Aggregation
// ---------------------------------------------------------------------------

/// Directional packet/byte counters for one endpoint.
#[derive(Default, Clone, Copy)]
struct Dir {
    packets_sent: u64,
    bytes_sent: u64,
    packets_received: u64,
    bytes_received: u64,
}

#[derive(Default)]
struct ConvAcc {
    protocol: String,
    endpoint_a: String,
    endpoint_b: String,
    packets_a_to_b: u64,
    bytes_a_to_b: u64,
    packets_b_to_a: u64,
    bytes_b_to_a: u64,
    first_ts: Option<f64>,
    last_ts: Option<f64>,
}

#[derive(Default)]
struct PortAcc {
    packets: u64,
    bytes: u64,
    endpoints: HashSet<String>,
}

#[derive(Default)]
struct Aggregator {
    resolve_ports: bool,
    format: &'static str,
    link_type: String,
    snaplen: u32,
    packets: u64,
    bytes: u64,
    captured_bytes: u64,
    truncated_packets: u64,
    decoded_packets: u64,
    first_ts: Option<f64>,
    last_ts: Option<f64>,
    ethernet: bool,
    protocols: HashMap<String, (u64, u64)>,
    hierarchy: HashMap<String, (u64, u64)>,
    talkers: HashMap<String, Dir>,
    mac_talkers: HashMap<String, Dir>,
    convs: HashMap<String, ConvAcc>,
    ports: HashMap<(&'static str, u16), PortAcc>,
}

/// The network layer carried by a link-layer frame.
enum L3 {
    Ipv4,
    Ipv6,
    Arp,
    Other(u16),
}

impl Aggregator {
    fn add_packet(&mut self, ts: f64, orig_len: u32, cap_len: u32, link_type: u32, data: &[u8]) {
        self.packets += 1;
        self.bytes += orig_len as u64;
        self.captured_bytes += cap_len as u64;
        if cap_len < orig_len {
            self.truncated_packets += 1;
        }
        if ts > 0.0 {
            self.first_ts = Some(self.first_ts.map_or(ts, |v| v.min(ts)));
            self.last_ts = Some(self.last_ts.map_or(ts, |v| v.max(ts)));
        }

        let weight = orig_len as u64;
        let mut path: Vec<String> = vec![link_layer_label(link_type)];
        let l3 = self.strip_link(data, link_type, weight, &mut path);
        match l3 {
            Some((L3::Ipv4, payload)) => {
                path.push("ipv4".into());
                self.decoded_packets += 1;
                self.ipv4(payload, weight, ts, &mut path);
            }
            Some((L3::Ipv6, payload)) => {
                path.push("ipv6".into());
                self.decoded_packets += 1;
                self.ipv6(payload, weight, ts, &mut path);
            }
            Some((L3::Arp, _)) => path.push("arp".into()),
            Some((L3::Other(t), _)) => path.push(format!("ethertype_0x{t:04x}")),
            None => {}
        }

        // Every distinct layer on the path gets one packet/byte credit, and the
        // whole path gets one hierarchy row (tshark `io,phs` flattened).
        let mut credited: Vec<&str> = Vec::with_capacity(path.len());
        for layer in &path {
            if credited.contains(&layer.as_str()) {
                continue;
            }
            credited.push(layer.as_str());
            let e = self.protocols.entry(layer.clone()).or_insert((0, 0));
            e.0 += 1;
            e.1 += weight;
        }
        let e = self.hierarchy.entry(path.join(":")).or_insert((0, 0));
        e.0 += 1;
        e.1 += weight;
    }

    /// Strip the link layer, recording MAC talkers and VLAN tags on the way.
    fn strip_link<'a>(
        &mut self,
        data: &'a [u8],
        link_type: u32,
        weight: u64,
        path: &mut Vec<String>,
    ) -> Option<(L3, &'a [u8])> {
        match link_type {
            LINKTYPE_ETHERNET => {
                self.ethernet = true;
                if data.len() < 14 {
                    return None;
                }
                let dst = fmt_mac(&data[0..6]);
                let src = fmt_mac(&data[6..12]);
                let e = self.mac_talkers.entry(src).or_default();
                e.packets_sent += 1;
                e.bytes_sent += weight;
                let e = self.mac_talkers.entry(dst).or_default();
                e.packets_received += 1;
                e.bytes_received += weight;

                let mut etype = rd_u16_be(data, 12)?;
                let mut off = 14usize;
                // Stacked VLAN tags (802.1Q / 802.1ad).
                while matches!(etype, 0x8100 | 0x88a8 | 0x9100) {
                    if data.len() < off + 4 {
                        return None;
                    }
                    path.push("vlan".into());
                    etype = rd_u16_be(data, off + 2)?;
                    off += 4;
                }
                Some((ethertype_kind(etype), data.get(off..)?))
            }
            LINKTYPE_RAW | LINKTYPE_RAW_ALT1 => data
                .first()
                .map(|v| if v >> 4 == 6 { (L3::Ipv6, data) } else { (L3::Ipv4, data) }),
            LINKTYPE_NULL => {
                if data.len() < 4 {
                    return None;
                }
                let af = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                let l3 = if af == 2 { L3::Ipv4 } else { L3::Ipv6 };
                Some((l3, &data[4..]))
            }
            LINKTYPE_LINUX_SLL => {
                if data.len() < 16 {
                    return None;
                }
                Some((ethertype_kind(rd_u16_be(data, 14)?), &data[16..]))
            }
            _ => None,
        }
    }

    fn ipv4(&mut self, d: &[u8], weight: u64, ts: f64, path: &mut Vec<String>) {
        if d.len() < 20 {
            return;
        }
        let ihl = (d[0] & 0x0f) as usize * 4;
        let proto = d[9];
        let frag_offset = u16::from_be_bytes([d[6], d[7]]) & 0x1fff;
        let src = fmt_ipv4(&d[12..16]);
        let dst = fmt_ipv4(&d[16..20]);
        self.talk(&src, &dst, weight);
        // Only the first fragment carries the transport header.
        if frag_offset > 0 {
            path.push("fragment".into());
            return;
        }
        if ihl < 20 || d.len() < ihl {
            return;
        }
        self.transport(proto, &d[ihl..], src, dst, weight, ts, path);
    }

    fn ipv6(&mut self, d: &[u8], weight: u64, ts: f64, path: &mut Vec<String>) {
        if d.len() < 40 {
            return;
        }
        let src = fmt_ipv6(&d[8..24]);
        let dst = fmt_ipv6(&d[24..40]);
        self.talk(&src, &dst, weight);

        // Walk the extension-header chain to the transport header.
        let mut next = d[6];
        let mut rest = &d[40..];
        for _ in 0..8 {
            match next {
                // Hop-by-hop, routing, destination options, mobility: TLV shape.
                0 | 43 | 60 | 135 => {
                    if rest.len() < 8 {
                        return;
                    }
                    let len = (rest[1] as usize + 1) * 8;
                    if rest.len() < len {
                        return;
                    }
                    next = rest[0];
                    rest = &rest[len..];
                }
                // Fragment header: fixed 8 bytes; a non-zero offset means no
                // transport header is present in this packet.
                44 => {
                    if rest.len() < 8 {
                        return;
                    }
                    if u16::from_be_bytes([rest[2], rest[3]]) & 0xfff8 != 0 {
                        path.push("fragment".into());
                        return;
                    }
                    next = rest[0];
                    rest = &rest[8..];
                }
                _ => break,
            }
        }
        self.transport(next, rest, src, dst, weight, ts, path);
    }

    fn transport(
        &mut self,
        proto: u8,
        l4: &[u8],
        src: String,
        dst: String,
        weight: u64,
        ts: f64,
        path: &mut Vec<String>,
    ) {
        match proto {
            IPPROTO_TCP if l4.len() >= 20 => {
                let sp = rd_u16_be(l4, 0).unwrap();
                let dp = rd_u16_be(l4, 2).unwrap();
                path.push("tcp".into());
                self.flow("TCP", format!("{src}:{sp}"), format!("{dst}:{dp}"), weight, ts);
                self.port("TCP", &src, sp, &dst, dp, weight, path);
            }
            IPPROTO_UDP if l4.len() >= 8 => {
                let sp = rd_u16_be(l4, 0).unwrap();
                let dp = rd_u16_be(l4, 2).unwrap();
                path.push("udp".into());
                self.flow("UDP", format!("{src}:{sp}"), format!("{dst}:{dp}"), weight, ts);
                self.port("UDP", &src, sp, &dst, dp, weight, path);
            }
            other => {
                let name = ip_proto_name(other);
                path.push(name.to_ascii_lowercase());
                self.flow(&name, src, dst, weight, ts);
            }
        }
    }

    fn talk(&mut self, src: &str, dst: &str, weight: u64) {
        let e = self.talkers.entry(src.to_string()).or_default();
        e.packets_sent += 1;
        e.bytes_sent += weight;
        let e = self.talkers.entry(dst.to_string()).or_default();
        e.packets_received += 1;
        e.bytes_received += weight;
    }

    /// Record one packet on a conversation, keeping the two directions apart.
    /// Endpoint `a` is the lower-sorting of the pair, so direction is stable.
    fn flow(&mut self, protocol: &str, ep_src: String, ep_dst: String, weight: u64, ts: f64) {
        let src_is_a = ep_src <= ep_dst;
        let (a, b) = if src_is_a { (ep_src, ep_dst) } else { (ep_dst, ep_src) };
        let c = self.convs.entry(format!("{protocol}|{a}|{b}")).or_insert_with(|| ConvAcc {
            protocol: protocol.to_string(),
            endpoint_a: a,
            endpoint_b: b,
            ..ConvAcc::default()
        });
        if src_is_a {
            c.packets_a_to_b += 1;
            c.bytes_a_to_b += weight;
        } else {
            c.packets_b_to_a += 1;
            c.bytes_b_to_a += weight;
        }
        if ts > 0.0 {
            c.first_ts = Some(c.first_ts.map_or(ts, |v| v.min(ts)));
            c.last_ts = Some(c.last_ts.map_or(ts, |v| v.max(ts)));
        }
    }

    /// Credit the packet to its service-side port — the numerically lower of the
    /// two, which is the server port in ordinary client→server traffic.
    fn port(
        &mut self,
        protocol: &'static str,
        src: &str,
        sp: u16,
        dst: &str,
        dp: u16,
        weight: u64,
        path: &mut Vec<String>,
    ) {
        let (port, owner) = if sp <= dp { (sp, src) } else { (dp, dst) };
        let e = self.ports.entry((protocol, port)).or_default();
        e.packets += 1;
        e.bytes += weight;
        e.endpoints.insert(format!("{owner}:{port}"));
        if self.resolve_ports {
            if let Some(name) = service_name(port) {
                path.push(name.to_string());
            }
        }
    }

    fn finish(self, opts: &Options) -> Summary {
        let top = opts.top;
        let (total_packets, total_bytes) = (self.packets, self.bytes);
        let duration = match (self.first_ts, self.last_ts) {
            (Some(a), Some(b)) => (b - a).max(0.0),
            _ => 0.0,
        };
        let overview = Overview {
            format: self.format,
            link_type: self.link_type,
            snaplen: self.snaplen,
            packets: total_packets,
            bytes: total_bytes,
            captured_bytes: self.captured_bytes,
            truncated_packets: self.truncated_packets,
            decoded_packets: self.decoded_packets,
            first_timestamp: round(self.first_ts.unwrap_or(0.0), 6),
            last_timestamp: round(self.last_ts.unwrap_or(0.0), 6),
            duration_seconds: round(duration, 6),
            average_packet_size_bytes: if total_packets == 0 {
                0.0
            } else {
                round(total_bytes as f64 / total_packets as f64, 2)
            },
            packets_per_second: if duration > 0.0 {
                round(total_packets as f64 / duration, 2)
            } else {
                0.0
            },
            bits_per_second: if duration > 0.0 {
                round(total_bytes as f64 * 8.0 / duration, 2)
            } else {
                0.0
            },
        };

        let rank = |a: (u64, u64), b: (u64, u64)| match opts.sort_by {
            // (packets, bytes) tuples; both orderings are descending.
            SortBy::Bytes => b.1.cmp(&a.1).then(b.0.cmp(&a.0)),
            SortBy::Packets => b.0.cmp(&a.0).then(b.1.cmp(&a.1)),
        };

        let mut protocols: Vec<ProtocolStat> = self
            .protocols
            .into_iter()
            .map(|(protocol, (packets, bytes))| ProtocolStat {
                protocol,
                packets,
                bytes,
                packet_percent: percent(packets, total_packets),
                byte_percent: percent(bytes, total_bytes),
            })
            .collect();
        protocols.sort_by(|a, b| {
            rank((a.packets, a.bytes), (b.packets, b.bytes)).then(a.protocol.cmp(&b.protocol))
        });
        let protocols_total = protocols.len();
        protocols.truncate(top);

        let mut hierarchy: Vec<HierarchyStat> = self
            .hierarchy
            .into_iter()
            .map(|(path, (packets, bytes))| HierarchyStat { path, packets, bytes })
            .collect();
        hierarchy.sort_by(|a, b| {
            rank((a.packets, a.bytes), (b.packets, b.bytes)).then(a.path.cmp(&b.path))
        });
        let hierarchy_total = hierarchy.len();
        hierarchy.truncate(top);

        let to_talkers = |m: HashMap<String, Dir>| {
            let mut v: Vec<Talker> = m
                .into_iter()
                .map(|(address, d)| Talker {
                    address,
                    packets: d.packets_sent + d.packets_received,
                    bytes: d.bytes_sent + d.bytes_received,
                    packets_sent: d.packets_sent,
                    bytes_sent: d.bytes_sent,
                    packets_received: d.packets_received,
                    bytes_received: d.bytes_received,
                })
                .collect();
            v.sort_by(|a, b| {
                rank((a.packets, a.bytes), (b.packets, b.bytes)).then(a.address.cmp(&b.address))
            });
            let total = v.len();
            v.truncate(top);
            (total, v)
        };
        let (talkers_total, talkers) = to_talkers(self.talkers);
        let (mac_talkers_total, mac_talkers) = to_talkers(self.mac_talkers);

        let capture_start = self.first_ts.unwrap_or(0.0);
        let mut conversations: Vec<Conversation> = self
            .convs
            .into_values()
            .map(|c| Conversation {
                protocol: c.protocol,
                endpoint_a: c.endpoint_a,
                endpoint_b: c.endpoint_b,
                packets: c.packets_a_to_b + c.packets_b_to_a,
                bytes: c.bytes_a_to_b + c.bytes_b_to_a,
                packets_a_to_b: c.packets_a_to_b,
                bytes_a_to_b: c.bytes_a_to_b,
                packets_b_to_a: c.packets_b_to_a,
                bytes_b_to_a: c.bytes_b_to_a,
                start_seconds: round(c.first_ts.map_or(0.0, |t| (t - capture_start).max(0.0)), 6),
                duration_seconds: round(
                    match (c.first_ts, c.last_ts) {
                        (Some(a), Some(b)) => (b - a).max(0.0),
                        _ => 0.0,
                    },
                    6,
                ),
            })
            .collect();
        conversations.sort_by(|a, b| {
            rank((a.packets, a.bytes), (b.packets, b.bytes))
                .then(a.protocol.cmp(&b.protocol))
                .then(a.endpoint_a.cmp(&b.endpoint_a))
                .then(a.endpoint_b.cmp(&b.endpoint_b))
        });
        let conversations_total = conversations.len();
        conversations.truncate(top);

        let resolve_ports = self.resolve_ports;
        let mut ports: Vec<PortStat> = self
            .ports
            .into_iter()
            .map(|((protocol, port), acc)| PortStat {
                protocol,
                port,
                service: if resolve_ports {
                    service_name(port).map(|s| s.to_string())
                } else {
                    None
                },
                packets: acc.packets,
                bytes: acc.bytes,
                endpoints: acc.endpoints.len(),
            })
            .collect();
        ports.sort_by(|a, b| {
            rank((a.packets, a.bytes), (b.packets, b.bytes))
                .then(a.port.cmp(&b.port))
                .then(a.protocol.cmp(b.protocol))
        });
        let ports_total = ports.len();
        ports.truncate(top);

        Summary {
            overview,
            ethernet: self.ethernet,
            protocols_total,
            protocols,
            hierarchy_total,
            hierarchy,
            talkers_total,
            talkers,
            mac_talkers_total,
            mac_talkers,
            conversations_total,
            conversations,
            ports_total,
            ports,
        }
    }
}

// ---------------------------------------------------------------------------
// Naming helpers
// ---------------------------------------------------------------------------

fn ethertype_kind(t: u16) -> L3 {
    match t {
        0x0800 => L3::Ipv4,
        0x86dd => L3::Ipv6,
        0x0806 => L3::Arp,
        other => L3::Other(other),
    }
}

fn ip_proto_name(p: u8) -> String {
    match p {
        1 => "ICMP",
        2 => "IGMP",
        4 => "IPIP",
        6 => "TCP",
        17 => "UDP",
        41 => "IPV6",
        47 => "GRE",
        50 => "ESP",
        51 => "AH",
        58 => "ICMPV6",
        89 => "OSPF",
        112 => "VRRP",
        132 => "SCTP",
        other => return format!("IPPROTO_{other}"),
    }
    .to_string()
}

/// Well-known service names for the ports a capture summary actually surfaces.
/// Deliberately a fixed table — no network lookups, no bundled IANA dump.
fn service_name(port: u16) -> Option<&'static str> {
    Some(match port {
        20 => "ftp-data",
        21 => "ftp",
        22 => "ssh",
        23 => "telnet",
        25 => "smtp",
        53 => "dns",
        67 | 68 => "dhcp",
        69 => "tftp",
        80 => "http",
        88 => "kerberos",
        110 => "pop3",
        111 => "rpcbind",
        119 => "nntp",
        123 => "ntp",
        135 => "msrpc",
        137 => "netbios-ns",
        138 => "netbios-dgm",
        139 => "netbios-ssn",
        143 => "imap",
        161 => "snmp",
        162 => "snmptrap",
        179 => "bgp",
        389 => "ldap",
        443 => "https",
        445 => "smb",
        465 => "smtps",
        500 => "isakmp",
        514 => "syslog",
        520 => "rip",
        546 | 547 => "dhcpv6",
        587 => "submission",
        631 => "ipp",
        636 => "ldaps",
        873 => "rsync",
        993 => "imaps",
        995 => "pop3s",
        1080 => "socks",
        1194 => "openvpn",
        1433 => "mssql",
        1521 => "oracle",
        1701 => "l2tp",
        1723 => "pptp",
        1812 => "radius",
        1813 => "radius-acct",
        1883 => "mqtt",
        1900 => "ssdp",
        2049 => "nfs",
        2181 => "zookeeper",
        3128 => "http-proxy",
        3306 => "mysql",
        3389 => "rdp",
        3478 => "stun",
        4500 => "ipsec-nat-t",
        5060 => "sip",
        5061 => "sips",
        5222 => "xmpp",
        5353 => "mdns",
        5432 => "postgresql",
        5672 => "amqp",
        5900 => "vnc",
        5985 => "winrm",
        5986 => "winrm-ssl",
        6379 => "redis",
        6667 => "irc",
        8080 => "http-alt",
        8443 => "https-alt",
        8883 => "mqtt-tls",
        9092 => "kafka",
        9200 => "elasticsearch",
        11211 => "memcached",
        27017 => "mongodb",
        _ => return None,
    })
}

fn fmt_mac(b: &[u8]) -> String {
    let mut out = String::with_capacity(17);
    for (i, byte) in b.iter().enumerate() {
        if i > 0 {
            out.push(':');
        }
        let _ = write!(out, "{byte:02x}");
    }
    out
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Assemble a classic little-endian pcap (Ethernet) from a list of frames.
    /// Timestamps advance one second per frame.
    fn pcap(frames: &[Vec<u8>]) -> Vec<u8> {
        let mut f = Vec::new();
        f.extend_from_slice(&[0xd4, 0xc3, 0xb2, 0xa1]); // LE magic, microseconds
        f.extend_from_slice(&2u16.to_le_bytes());
        f.extend_from_slice(&4u16.to_le_bytes());
        f.extend_from_slice(&0u32.to_le_bytes());
        f.extend_from_slice(&0u32.to_le_bytes());
        f.extend_from_slice(&65535u32.to_le_bytes()); // snaplen
        f.extend_from_slice(&1u32.to_le_bytes()); // Ethernet
        for (i, p) in frames.iter().enumerate() {
            f.extend_from_slice(&(1_700_000_000u32 + i as u32).to_le_bytes());
            f.extend_from_slice(&0u32.to_le_bytes());
            f.extend_from_slice(&(p.len() as u32).to_le_bytes());
            f.extend_from_slice(&(p.len() as u32).to_le_bytes());
            f.extend_from_slice(p);
        }
        f
    }

    /// Ethernet + IPv4 + TCP/UDP frame carrying `payload`.
    fn frame(proto: u8, src: [u8; 4], dst: [u8; 4], sp: u16, dp: u16, payload: &[u8]) -> Vec<u8> {
        let mut pkt = Vec::new();
        pkt.extend_from_slice(&[0x11, 0x22, 0x33, 0x44, 0x55, 0x66]); // dst mac
        pkt.extend_from_slice(&[0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]); // src mac
        pkt.extend_from_slice(&0x0800u16.to_be_bytes()); // IPv4
        let l4 = if proto == IPPROTO_TCP {
            let mut t = Vec::new();
            t.extend_from_slice(&sp.to_be_bytes());
            t.extend_from_slice(&dp.to_be_bytes());
            t.extend_from_slice(&0u32.to_be_bytes());
            t.extend_from_slice(&0u32.to_be_bytes());
            t.push(0x50); // data offset 5 words
            t.push(0x18); // PSH, ACK
            t.extend_from_slice(&8192u16.to_be_bytes());
            t.extend_from_slice(&0u16.to_be_bytes());
            t.extend_from_slice(&0u16.to_be_bytes());
            t.extend_from_slice(payload);
            t
        } else {
            let mut u = Vec::new();
            u.extend_from_slice(&sp.to_be_bytes());
            u.extend_from_slice(&dp.to_be_bytes());
            u.extend_from_slice(&((8 + payload.len()) as u16).to_be_bytes());
            u.extend_from_slice(&0u16.to_be_bytes());
            u.extend_from_slice(payload);
            u
        };
        let total = 20 + l4.len();
        let mut ip = vec![0x45, 0x00];
        ip.extend_from_slice(&(total as u16).to_be_bytes());
        ip.extend_from_slice(&[0, 0, 0, 0]);
        ip.push(64);
        ip.push(proto);
        ip.extend_from_slice(&0u16.to_be_bytes());
        ip.extend_from_slice(&src);
        ip.extend_from_slice(&dst);
        pkt.extend_from_slice(&ip);
        pkt.extend_from_slice(&l4);
        pkt
    }

    const A: [u8; 4] = [192, 168, 0, 10];
    const B: [u8; 4] = [192, 168, 0, 20];

    fn find_protocol<'a>(s: &'a Summary, name: &str) -> &'a ProtocolStat {
        s.protocols.iter().find(|p| p.protocol == name).expect("protocol row")
    }

    #[test]
    fn rejects_non_pcap() {
        let err = analyze(b"not a capture file at all", &Options::default()).unwrap_err();
        assert!(err.contains("not a pcap"), "{err}");
    }

    #[test]
    fn rejects_tiny_file() {
        assert!(analyze(b"ab", &Options::default()).unwrap_err().contains("too small"));
    }

    #[test]
    fn overview_reports_capture_properties() {
        let cap = pcap(&[
            frame(IPPROTO_TCP, A, B, 40000, 443, b"hello"),
            frame(IPPROTO_TCP, B, A, 443, 40000, b"hi"),
        ]);
        let s = analyze(&cap, &Options::default()).unwrap();
        assert_eq!(s.overview.format, "pcap");
        assert_eq!(s.overview.link_type, "Ethernet");
        assert_eq!(s.overview.snaplen, 65535);
        assert_eq!(s.overview.packets, 2);
        assert_eq!(s.overview.decoded_packets, 2);
        assert_eq!(s.overview.truncated_packets, 0);
        assert_eq!(s.overview.bytes, s.overview.captured_bytes);
        assert!((s.overview.duration_seconds - 1.0).abs() < 1e-9);
        assert!((s.overview.packets_per_second - 2.0).abs() < 1e-9);
        assert!((s.overview.bits_per_second - s.overview.bytes as f64 * 8.0).abs() < 1e-6);
        let expected_avg = s.overview.bytes as f64 / 2.0;
        assert!((s.overview.average_packet_size_bytes - expected_avg).abs() < 0.01);
        assert!(s.ethernet);
    }

    #[test]
    fn protocol_breakdown_counts_every_layer_once() {
        let cap = pcap(&[
            frame(IPPROTO_TCP, A, B, 40000, 443, b"x"),
            frame(IPPROTO_UDP, A, B, 5000, 53, b"y"),
        ]);
        let s = analyze(&cap, &Options { top: 50, ..Options::default() }).unwrap();
        assert_eq!(find_protocol(&s, "eth").packets, 2);
        assert_eq!(find_protocol(&s, "ipv4").packets, 2);
        assert_eq!(find_protocol(&s, "tcp").packets, 1);
        assert_eq!(find_protocol(&s, "udp").packets, 1);
        // Well-known ports name the application layer.
        assert_eq!(find_protocol(&s, "https").packets, 1);
        assert_eq!(find_protocol(&s, "dns").packets, 1);
        assert!((find_protocol(&s, "eth").packet_percent - 100.0).abs() < 1e-9);
        assert!((find_protocol(&s, "tcp").packet_percent - 50.0).abs() < 1e-9);
    }

    #[test]
    fn hierarchy_paths_are_layer_joined() {
        let cap = pcap(&[frame(IPPROTO_TCP, A, B, 40000, 443, b"x")]);
        let s = analyze(&cap, &Options::default()).unwrap();
        assert_eq!(s.hierarchy_total, 1);
        assert_eq!(s.hierarchy[0].path, "eth:ipv4:tcp:https");
        assert_eq!(s.hierarchy[0].packets, 1);
    }

    #[test]
    fn resolve_ports_off_drops_service_names() {
        let cap = pcap(&[frame(IPPROTO_TCP, A, B, 40000, 443, b"x")]);
        let s = analyze(&cap, &Options { resolve_ports: false, ..Options::default() }).unwrap();
        assert_eq!(s.hierarchy[0].path, "eth:ipv4:tcp");
        assert_eq!(s.ports[0].service, None);
        assert_eq!(s.ports[0].port, 443);
    }

    #[test]
    fn talkers_split_sent_and_received() {
        // A sends two packets, B sends one.
        let cap = pcap(&[
            frame(IPPROTO_TCP, A, B, 40000, 443, b"aaaa"),
            frame(IPPROTO_TCP, A, B, 40000, 443, b"bbbb"),
            frame(IPPROTO_TCP, B, A, 443, 40000, b"c"),
        ]);
        let s = analyze(&cap, &Options::default()).unwrap();
        assert_eq!(s.talkers_total, 2);
        let a = s.talkers.iter().find(|t| t.address == "192.168.0.10").unwrap();
        assert_eq!(a.packets_sent, 2);
        assert_eq!(a.packets_received, 1);
        assert_eq!(a.packets, 3);
        assert_eq!(a.bytes, a.bytes_sent + a.bytes_received);
        let b = s.talkers.iter().find(|t| t.address == "192.168.0.20").unwrap();
        assert_eq!(b.packets_sent, 1);
        assert_eq!(b.packets_received, 2);
    }

    #[test]
    fn mac_talkers_present_for_ethernet() {
        let cap = pcap(&[frame(IPPROTO_TCP, A, B, 40000, 443, b"x")]);
        let s = analyze(&cap, &Options::default()).unwrap();
        assert_eq!(s.mac_talkers_total, 2);
        let sender = s.mac_talkers.iter().find(|t| t.address == "aa:bb:cc:dd:ee:ff").unwrap();
        assert_eq!(sender.packets_sent, 1);
        assert_eq!(sender.packets_received, 0);
    }

    #[test]
    fn conversations_keep_directions_apart() {
        let cap = pcap(&[
            frame(IPPROTO_TCP, A, B, 40000, 443, b"aa"),
            frame(IPPROTO_TCP, B, A, 443, 40000, b"b"),
            frame(IPPROTO_TCP, A, B, 40000, 443, b"cc"),
        ]);
        let s = analyze(&cap, &Options::default()).unwrap();
        assert_eq!(s.conversations_total, 1);
        let c = &s.conversations[0];
        assert_eq!(c.protocol, "TCP");
        assert_eq!(c.endpoint_a, "192.168.0.10:40000");
        assert_eq!(c.endpoint_b, "192.168.0.20:443");
        assert_eq!(c.packets, 3);
        assert_eq!(c.packets_a_to_b, 2);
        assert_eq!(c.packets_b_to_a, 1);
        assert_eq!(c.bytes, c.bytes_a_to_b + c.bytes_b_to_a);
        assert!((c.start_seconds - 0.0).abs() < 1e-9);
        assert!((c.duration_seconds - 2.0).abs() < 1e-9);
    }

    #[test]
    fn busiest_ports_credit_the_service_side() {
        let cap = pcap(&[
            frame(IPPROTO_TCP, A, B, 40000, 443, b"aaaaaaaa"),
            frame(IPPROTO_TCP, B, A, 443, 40000, b"bbbbbbbb"),
            frame(IPPROTO_UDP, A, B, 5000, 53, b"c"),
        ]);
        let s = analyze(&cap, &Options::default()).unwrap();
        assert_eq!(s.ports_total, 2);
        let https = s.ports.iter().find(|p| p.port == 443).unwrap();
        assert_eq!(https.protocol, "TCP");
        assert_eq!(https.service.as_deref(), Some("https"));
        assert_eq!(https.packets, 2);
        assert_eq!(https.endpoints, 1); // one server endpoint 192.168.0.20:443
        let dns = s.ports.iter().find(|p| p.port == 53).unwrap();
        assert_eq!(dns.protocol, "UDP");
        assert_eq!(dns.service.as_deref(), Some("dns"));
        assert_eq!(dns.packets, 1);
    }

    #[test]
    fn sort_by_packets_outranks_a_single_large_flow() {
        // 192.168.0.30 sends one big packet; 192.168.0.10 sends three small ones.
        const C: [u8; 4] = [192, 168, 0, 30];
        let big = vec![0u8; 800];
        let mut frames = vec![frame(IPPROTO_UDP, C, B, 4000, 9999, &big)];
        for _ in 0..3 {
            frames.push(frame(IPPROTO_UDP, A, B, 4001, 9999, b"s"));
        }
        let cap = pcap(&frames);

        let by_bytes = analyze(&cap, &Options::default()).unwrap();
        assert_eq!(by_bytes.talkers[0].address, "192.168.0.20"); // B is in every packet
        let c_bytes = by_bytes.talkers.iter().position(|t| t.address == "192.168.0.30").unwrap();
        let a_bytes = by_bytes.talkers.iter().position(|t| t.address == "192.168.0.10").unwrap();
        assert!(c_bytes < a_bytes, "the bulk sender ranks higher by bytes");

        let by_packets =
            analyze(&cap, &Options { sort_by: SortBy::Packets, ..Options::default() }).unwrap();
        let c_pkts = by_packets.talkers.iter().position(|t| t.address == "192.168.0.30").unwrap();
        let a_pkts = by_packets.talkers.iter().position(|t| t.address == "192.168.0.10").unwrap();
        assert!(a_pkts < c_pkts, "the chatty sender ranks higher by packets");
    }

    #[test]
    fn top_truncates_but_totals_survive() {
        let frames: Vec<Vec<u8>> = (0..5)
            .map(|i| frame(IPPROTO_UDP, A, [10, 0, 0, i as u8], 5000, 9000 + i as u16, b"x"))
            .collect();
        let cap = pcap(&frames);
        let s = analyze(&cap, &Options { top: 2, ..Options::default() }).unwrap();
        assert_eq!(s.talkers_total, 6); // A plus five destinations
        assert_eq!(s.talkers.len(), 2);
        assert_eq!(s.conversations_total, 5);
        assert_eq!(s.conversations.len(), 2);
    }

    #[test]
    fn vlan_tags_and_arp_appear_in_the_hierarchy() {
        // Ethernet + 802.1Q + ARP.
        let mut pkt = Vec::new();
        pkt.extend_from_slice(&[0xff; 6]);
        pkt.extend_from_slice(&[0x11, 0x22, 0x33, 0x44, 0x55, 0x66]);
        pkt.extend_from_slice(&0x8100u16.to_be_bytes());
        pkt.extend_from_slice(&0x0064u16.to_be_bytes()); // VLAN 100
        pkt.extend_from_slice(&0x0806u16.to_be_bytes()); // ARP
        pkt.extend_from_slice(&[0u8; 28]);
        let s = analyze(&pcap(&[pkt]), &Options::default()).unwrap();
        assert_eq!(s.hierarchy[0].path, "eth:vlan:arp");
        assert_eq!(s.overview.decoded_packets, 0);
        assert_eq!(s.talkers_total, 0);
    }

    #[test]
    fn ipv6_udp_is_decoded() {
        let payload = b"hello";
        let mut pkt = Vec::new();
        pkt.extend_from_slice(&[0x11, 0x22, 0x33, 0x44, 0x55, 0x66]);
        pkt.extend_from_slice(&[0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);
        pkt.extend_from_slice(&0x86ddu16.to_be_bytes());
        let mut ip6 = vec![0x60, 0, 0, 0];
        ip6.extend_from_slice(&((8 + payload.len()) as u16).to_be_bytes());
        ip6.push(17); // UDP
        ip6.push(64);
        let mut src = [0u8; 16];
        src[0] = 0x20;
        src[1] = 0x01;
        src[15] = 1;
        let mut dst = [0u8; 16];
        dst[0] = 0x20;
        dst[1] = 0x01;
        dst[15] = 2;
        ip6.extend_from_slice(&src);
        ip6.extend_from_slice(&dst);
        ip6.extend_from_slice(&5000u16.to_be_bytes());
        ip6.extend_from_slice(&53u16.to_be_bytes());
        ip6.extend_from_slice(&((8 + payload.len()) as u16).to_be_bytes());
        ip6.extend_from_slice(&0u16.to_be_bytes());
        ip6.extend_from_slice(payload);
        pkt.extend_from_slice(&ip6);

        let s = analyze(&pcap(&[pkt]), &Options::default()).unwrap();
        assert_eq!(s.hierarchy[0].path, "eth:ipv6:udp:dns");
        assert_eq!(s.talkers_total, 2);
        assert_eq!(s.talkers[0].address, "2001::1");
        assert_eq!(s.conversations[0].endpoint_a, "2001::1:5000");
    }

    #[test]
    fn icmp_conversations_use_bare_ip_endpoints() {
        let cap = pcap(&[frame(1, A, B, 0, 0, b"")]);
        let s = analyze(&cap, &Options::default()).unwrap();
        assert_eq!(s.conversations_total, 1);
        assert_eq!(s.conversations[0].protocol, "ICMP");
        assert_eq!(s.conversations[0].endpoint_a, "192.168.0.10");
        assert_eq!(s.ports_total, 0);
    }

    #[test]
    fn later_ip_fragments_skip_ports_and_conversations() {
        // frame() writes flags/frag-offset as zero; patch in a non-zero offset.
        let mut f = frame(IPPROTO_TCP, A, B, 40000, 443, b"tail");
        f[14 + 6] = 0x00;
        f[14 + 7] = 0x20; // fragment offset 32 (8-byte units)
        let s = analyze(&pcap(&[f]), &Options::default()).unwrap();
        assert_eq!(s.talkers_total, 2, "fragments still count toward the IP totals");
        assert_eq!(s.conversations_total, 0);
        assert_eq!(s.ports_total, 0);
        assert_eq!(s.hierarchy[0].path, "eth:ipv4:fragment");
    }

    #[test]
    fn truncated_packets_are_flagged() {
        // Hand-build a record whose stored length is shorter than the on-wire one.
        let f = frame(IPPROTO_TCP, A, B, 40000, 443, b"payload");
        let mut cap = Vec::new();
        cap.extend_from_slice(&[0xd4, 0xc3, 0xb2, 0xa1]);
        cap.extend_from_slice(&2u16.to_le_bytes());
        cap.extend_from_slice(&4u16.to_le_bytes());
        cap.extend_from_slice(&0u32.to_le_bytes());
        cap.extend_from_slice(&0u32.to_le_bytes());
        cap.extend_from_slice(&64u32.to_le_bytes()); // snaplen 64
        cap.extend_from_slice(&1u32.to_le_bytes());
        cap.extend_from_slice(&1_700_000_000u32.to_le_bytes());
        cap.extend_from_slice(&0u32.to_le_bytes());
        cap.extend_from_slice(&(f.len() as u32).to_le_bytes()); // captured
        cap.extend_from_slice(&((f.len() + 500) as u32).to_le_bytes()); // on-wire
        cap.extend_from_slice(&f);
        let s = analyze(&cap, &Options::default()).unwrap();
        assert_eq!(s.overview.snaplen, 64);
        assert_eq!(s.overview.truncated_packets, 1);
        assert_eq!(s.overview.bytes, f.len() as u64 + 500);
        assert_eq!(s.overview.captured_bytes, f.len() as u64);
    }

    #[test]
    fn pcapng_captures_are_walked() {
        // SHB + IDB (Ethernet, tsresol 6) + one EPB.
        let payload = frame(IPPROTO_UDP, A, B, 5000, 53, b"q");
        let mut ng = Vec::new();
        ng.extend_from_slice(&[0x0a, 0x0d, 0x0d, 0x0a]);
        ng.extend_from_slice(&28u32.to_le_bytes());
        ng.extend_from_slice(&0x1a2b3c4du32.to_le_bytes());
        ng.extend_from_slice(&1u16.to_le_bytes());
        ng.extend_from_slice(&0u16.to_le_bytes());
        ng.extend_from_slice(&(-1i64).to_le_bytes());
        ng.extend_from_slice(&28u32.to_le_bytes());
        // IDB
        ng.extend_from_slice(&1u32.to_le_bytes());
        ng.extend_from_slice(&20u32.to_le_bytes());
        ng.extend_from_slice(&1u16.to_le_bytes()); // Ethernet
        ng.extend_from_slice(&0u16.to_le_bytes());
        ng.extend_from_slice(&262144u32.to_le_bytes()); // snaplen
        ng.extend_from_slice(&20u32.to_le_bytes());
        // EPB
        let pad = (4 - payload.len() % 4) % 4;
        let epb_len = 32 + payload.len() + pad;
        ng.extend_from_slice(&6u32.to_le_bytes());
        ng.extend_from_slice(&(epb_len as u32).to_le_bytes());
        ng.extend_from_slice(&0u32.to_le_bytes()); // interface id
        let ts = 1_700_000_000_000_000u64;
        ng.extend_from_slice(&((ts >> 32) as u32).to_le_bytes());
        ng.extend_from_slice(&(ts as u32).to_le_bytes());
        ng.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        ng.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        ng.extend_from_slice(&payload);
        ng.extend_from_slice(&vec![0u8; pad]);
        ng.extend_from_slice(&(epb_len as u32).to_le_bytes());

        let s = analyze(&ng, &Options::default()).unwrap();
        assert_eq!(s.overview.format, "pcapng");
        assert_eq!(s.overview.link_type, "Ethernet");
        assert_eq!(s.overview.snaplen, 262144);
        assert_eq!(s.overview.packets, 1);
        assert_eq!(s.hierarchy[0].path, "eth:ipv4:udp:dns");
        assert!((s.overview.first_timestamp - 1_700_000_000.0).abs() < 1e-3);
    }

    #[test]
    fn empty_capture_yields_zeroed_rates() {
        let s = analyze(&pcap(&[]), &Options::default()).unwrap();
        assert_eq!(s.overview.packets, 0);
        assert_eq!(s.overview.average_packet_size_bytes, 0.0);
        assert_eq!(s.overview.packets_per_second, 0.0);
        assert_eq!(s.overview.bits_per_second, 0.0);
        assert_eq!(s.protocols_total, 0);
    }
}

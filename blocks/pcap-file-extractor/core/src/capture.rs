//! Container parsing (libpcap + pcapng), link/IP/TCP decoding, and TCP stream
//! reassembly.
//!
//! Segments are stored as `(offset, len)` slices into the ORIGINAL capture
//! buffer — no per-packet copies — so only the finally reassembled streams cost
//! memory, which matters inside the 64 MiB sandbox.

use std::collections::BTreeMap;

/// Total bytes of reassembled TCP payload we are willing to materialise.
pub const STREAM_BUDGET: usize = 12 * 1024 * 1024;
/// Hard cap on segments, so a pathological capture can't blow up sorting.
const MAX_SEGMENTS: usize = 2_000_000;

const LINKTYPE_NULL: u32 = 0;
const LINKTYPE_ETHERNET: u32 = 1;
const LINKTYPE_RAW_ALT: u32 = 12;
const LINKTYPE_RAW: u32 = 101;
const LINKTYPE_LINUX_SLL: u32 = 113;
const LINKTYPE_LINUX_SLL2: u32 = 276;

const IPPROTO_TCP: u8 = 6;

/// An IPv4 or IPv6 address, kept as bytes so flow keys stay cheap to compare.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum Ip {
    V4([u8; 4]),
    V6([u8; 16]),
}

impl Ip {
    pub fn text(&self) -> String {
        match self {
            Ip::V4(b) => format!("{}.{}.{}.{}", b[0], b[1], b[2], b[3]),
            Ip::V6(b) => {
                let groups: Vec<String> = (0..8)
                    .map(|i| format!("{:x}", u16::from_be_bytes([b[i * 2], b[i * 2 + 1]])))
                    .collect();
                groups.join(":")
            }
        }
    }
}

/// `ip:port`, or `[ip]:port` for IPv6 so the port stays readable.
pub fn endpoint_text(ip: &Ip, port: u16) -> String {
    match ip {
        Ip::V4(_) => format!("{}:{}", ip.text(), port),
        Ip::V6(_) => format!("[{}]:{}", ip.text(), port),
    }
}

pub fn link_type_name(lt: u32) -> String {
    match lt {
        LINKTYPE_NULL => "null/loopback".into(),
        LINKTYPE_ETHERNET => "ethernet".into(),
        LINKTYPE_RAW_ALT | LINKTYPE_RAW => "raw-ip".into(),
        LINKTYPE_LINUX_SLL => "linux-sll".into(),
        LINKTYPE_LINUX_SLL2 => "linux-sll2".into(),
        other => format!("linktype-{other}"),
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct FlowKey {
    pub src: Ip,
    pub sport: u16,
    pub dst: Ip,
    pub dport: u16,
}

impl FlowKey {
    fn reversed(&self) -> FlowKey {
        FlowKey { src: self.dst, sport: self.dport, dst: self.src, dport: self.sport }
    }
}

#[derive(Clone, Copy)]
struct Segment {
    seq: u32,
    off: usize,
    len: usize,
    pkt: usize,
    ts: f64,
}

#[derive(Default)]
struct Flow {
    segments: Vec<Segment>,
    /// Sequence number of a SYN (without ACK is not required — the ISN is the
    /// same either way), used to anchor the byte stream at offset 0.
    syn_seq: Option<u32>,
    saw_syn_only: bool,
    first_pkt: usize,
    first_ts: f64,
}

/// One direction of a TCP conversation, reassembled into a contiguous buffer.
#[derive(Default)]
pub struct Stream {
    pub data: Vec<u8>,
    /// `(stream offset, packet number, timestamp)` for each contributing
    /// segment, ascending — lets an object at offset N name its packet.
    pub marks: Vec<(usize, usize, f64)>,
    /// Bytes that were never captured (holes between segments). They are
    /// zero-filled in `data` so downstream offsets stay correct.
    pub missing: usize,
    /// True when the stream hit the reassembly budget and was cut short.
    pub truncated: bool,
}

impl Stream {
    /// Packet number + timestamp of the segment covering `offset`.
    pub fn locate(&self, offset: usize) -> (usize, f64) {
        match self.marks.binary_search_by(|m| m.0.cmp(&offset)) {
            Ok(i) => (self.marks[i].1, self.marks[i].2),
            Err(0) => self.marks.first().map(|m| (m.1, m.2)).unwrap_or((0, 0.0)),
            Err(i) => (self.marks[i - 1].1, self.marks[i - 1].2),
        }
    }

    pub fn completeness(&self) -> f64 {
        let total = self.data.len();
        if total == 0 {
            return 100.0;
        }
        let present = total.saturating_sub(self.missing) as f64;
        (present / total as f64) * 100.0
    }
}

/// A bidirectional TCP conversation.
pub struct Conn {
    pub client_ip: Ip,
    pub client_port: u16,
    pub server_ip: Ip,
    pub server_port: u16,
    /// Client → server bytes.
    pub c2s: Stream,
    /// Server → client bytes.
    pub s2c: Stream,
    pub first_pkt: usize,
    pub first_ts: f64,
}

impl Conn {
    pub fn client(&self) -> String {
        endpoint_text(&self.client_ip, self.client_port)
    }
    pub fn server(&self) -> String {
        endpoint_text(&self.server_ip, self.server_port)
    }
    pub fn involves_port(&self, port: u16) -> bool {
        self.client_port == port || self.server_port == port
    }
}

pub struct Capture {
    pub format: &'static str,
    pub link_type: String,
    pub total_packets: usize,
    pub conns: Vec<Conn>,
    /// True when at least one stream hit the reassembly budget.
    pub stream_budget_hit: bool,
    pub ip_fragments_skipped: usize,
}

fn u16le(b: &[u8], o: usize) -> Option<u16> {
    b.get(o..o + 2).map(|s| u16::from_le_bytes([s[0], s[1]]))
}
fn u32le(b: &[u8], o: usize) -> Option<u32> {
    b.get(o..o + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}
fn u16be(b: &[u8], o: usize) -> Option<u16> {
    b.get(o..o + 2).map(|s| u16::from_be_bytes([s[0], s[1]]))
}
fn u32be(b: &[u8], o: usize) -> Option<u32> {
    b.get(o..o + 4).map(|s| u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
}

#[derive(Default)]
struct Collector {
    flows: BTreeMap<FlowKey, Flow>,
    packets: usize,
    segments: usize,
    fragments_skipped: usize,
}

/// Parse a capture and reassemble every TCP conversation it contains.
pub fn parse(bytes: &[u8]) -> Result<Capture, String> {
    if bytes.len() < 24 {
        return Err(format!(
            "not a capture file: expected at least 24 bytes of pcap/pcapng header, got {} bytes",
            bytes.len()
        ));
    }
    let magic = u32be(bytes, 0).unwrap_or(0);
    let mut col = Collector::default();
    let (format, link_type) = match magic {
        0x0a0d0d0a => ("pcapng", walk_pcapng(bytes, &mut col)?),
        0xa1b2c3d4 | 0xa1b23c4d => ("pcap", walk_classic(bytes, false, magic == 0xa1b23c4d, &mut col)?),
        0xd4c3b2a1 | 0x4d3cb2a1 => ("pcap", walk_classic(bytes, true, magic == 0x4d3cb2a1, &mut col)?),
        _ => {
            return Err(format!(
                "unrecognised capture format: file magic 0x{magic:08x} is neither libpcap \
                 (0xa1b2c3d4 / 0xd4c3b2a1) nor pcapng (0x0a0d0d0a) — export the capture as .pcap \
                 or .pcapng"
            ))
        }
    };

    let (conns, budget_hit) = build_conns(bytes, col.flows);
    Ok(Capture {
        format,
        link_type,
        total_packets: col.packets,
        conns,
        stream_budget_hit: budget_hit,
        ip_fragments_skipped: col.fragments_skipped,
    })
}

fn walk_classic(
    b: &[u8],
    swapped: bool,
    nanos: bool,
    col: &mut Collector,
) -> Result<String, String> {
    let rd32 = |o: usize| if swapped { u32le(b, o) } else { u32be(b, o) };
    let link = rd32(20).ok_or("truncated pcap file header")?;
    let mut pos = 24usize;
    while pos + 16 <= b.len() {
        let ts_sec = rd32(pos).unwrap_or(0) as f64;
        let ts_frac = rd32(pos + 4).unwrap_or(0) as f64;
        let incl = rd32(pos + 8).unwrap_or(0) as usize;
        pos += 16;
        if incl > b.len().saturating_sub(pos) {
            break;
        }
        col.packets += 1;
        let ts = ts_sec + ts_frac / if nanos { 1e9 } else { 1e6 };
        handle_packet(b, pos, incl, link, ts, col);
        pos += incl;
    }
    Ok(link_type_name(link))
}

fn walk_pcapng(b: &[u8], col: &mut Collector) -> Result<String, String> {
    let mut le = true;
    let mut pos = 0usize;
    // interface id -> (linktype, timestamp resolution divisor)
    let mut ifaces: Vec<(u32, f64)> = Vec::new();
    let mut first_link: Option<u32> = None;
    while pos + 12 <= b.len() {
        let btype = if le { u32le(b, pos) } else { u32be(b, pos) }.unwrap_or(0);
        if btype == 0x0a0d0d0a {
            // Section header: the byte-order magic tells us the endianness.
            le = u32le(b, pos + 8) == Some(0x1a2b_3c4d);
            ifaces.clear();
        }
        let blen = if le { u32le(b, pos + 4) } else { u32be(b, pos + 4) }.unwrap_or(0) as usize;
        if blen < 12 || pos + blen > b.len() {
            break;
        }
        let body = &b[pos + 8..pos + blen - 4];
        match btype {
            0x0000_0001 => {
                // Interface description block.
                let lt = if le { u16le(body, 0) } else { u16be(body, 0) }.unwrap_or(1) as u32;
                let div = tsresol(body, 8, le).unwrap_or(1e6);
                if first_link.is_none() {
                    first_link = Some(lt);
                }
                ifaces.push((lt, div));
            }
            0x0000_0006 => {
                // Enhanced packet block.
                let iface = if le { u32le(body, 0) } else { u32be(body, 0) }.unwrap_or(0) as usize;
                let hi = if le { u32le(body, 4) } else { u32be(body, 4) }.unwrap_or(0) as u64;
                let lo = if le { u32le(body, 8) } else { u32be(body, 8) }.unwrap_or(0) as u64;
                let cap = if le { u32le(body, 12) } else { u32be(body, 12) }.unwrap_or(0) as usize;
                let (lt, div) = ifaces.get(iface).copied().unwrap_or((1, 1e6));
                let ts = (((hi << 32) | lo) as f64) / div;
                let data_start = pos + 8 + 20;
                if cap <= b.len().saturating_sub(data_start) {
                    col.packets += 1;
                    handle_packet(b, data_start, cap, lt, ts, col);
                }
            }
            0x0000_0003 => {
                // Simple packet block: no timestamp, no interface id.
                let orig = if le { u32le(body, 0) } else { u32be(body, 0) }.unwrap_or(0) as usize;
                let (lt, _) = ifaces.first().copied().unwrap_or((1, 1e6));
                let data_start = pos + 8 + 4;
                let cap =
                    orig.min(b.len().saturating_sub(data_start)).min(blen.saturating_sub(16));
                col.packets += 1;
                handle_packet(b, data_start, cap, lt, 0.0, col);
            }
            _ => {}
        }
        pos += blen;
    }
    Ok(link_type_name(first_link.unwrap_or(1)))
}

/// `if_tsresol` (option code 9): one byte, either a power of 10 or, with the
/// high bit set, a power of 2.
fn tsresol(body: &[u8], start: usize, le: bool) -> Option<f64> {
    let mut o = start;
    while o + 4 <= body.len() {
        let code = if le { u16le(body, o) } else { u16be(body, o) }?;
        let len = if le { u16le(body, o + 2) } else { u16be(body, o + 2) }? as usize;
        if code == 0 {
            return None;
        }
        if code == 9 && len >= 1 {
            let raw = *body.get(o + 4)?;
            return Some(if raw & 0x80 != 0 {
                2f64.powi((raw & 0x7f) as i32)
            } else {
                10f64.powi(raw as i32)
            });
        }
        o += 4 + ((len + 3) & !3);
    }
    None
}

fn handle_packet(b: &[u8], off: usize, len: usize, link: u32, ts: f64, col: &mut Collector) {
    let frame = &b[off..off + len];
    let Some((ip_off, is_v6)) = link_layer(frame, link) else { return };
    if ip_off >= frame.len() {
        return;
    }
    decode_ip(b, off + ip_off, frame.len() - ip_off, is_v6, ts, col);
}

/// Returns `(offset of the IP header inside the frame, is_ipv6)`.
fn link_layer(frame: &[u8], link: u32) -> Option<(usize, bool)> {
    match link {
        LINKTYPE_ETHERNET => {
            let mut o = 12usize;
            let mut et = u16be(frame, o)?;
            // Walk any number of VLAN / QinQ tags.
            for _ in 0..4 {
                if et == 0x8100 || et == 0x88a8 || et == 0x9100 {
                    o += 4;
                    et = u16be(frame, o)?;
                } else {
                    break;
                }
            }
            match et {
                0x0800 => Some((o + 2, false)),
                0x86dd => Some((o + 2, true)),
                _ => None,
            }
        }
        LINKTYPE_RAW | LINKTYPE_RAW_ALT => match frame.first()? >> 4 {
            4 => Some((0, false)),
            6 => Some((0, true)),
            _ => None,
        },
        LINKTYPE_LINUX_SLL => match u16be(frame, 14)? {
            0x0800 => Some((16, false)),
            0x86dd => Some((16, true)),
            _ => None,
        },
        LINKTYPE_LINUX_SLL2 => match u16be(frame, 0)? {
            0x0800 => Some((20, false)),
            0x86dd => Some((20, true)),
            _ => None,
        },
        LINKTYPE_NULL => {
            let fam = u32le(frame, 0)?;
            let fam = if fam > 0x00ff_ffff { u32be(frame, 0)? } else { fam };
            match fam {
                2 => Some((4, false)),
                24 | 28 | 30 => Some((4, true)),
                _ => None,
            }
        }
        _ => None,
    }
}

fn decode_ip(b: &[u8], off: usize, avail: usize, is_v6: bool, ts: f64, col: &mut Collector) {
    let hdr = match b.get(off..off + avail) {
        Some(s) => s,
        None => return,
    };
    let (src, dst, proto, l4_off, l4_len) = if is_v6 {
        if hdr.len() < 40 {
            return;
        }
        let mut src = [0u8; 16];
        let mut dst = [0u8; 16];
        src.copy_from_slice(&hdr[8..24]);
        dst.copy_from_slice(&hdr[24..40]);
        let payload_len = u16be(hdr, 4).unwrap_or(0) as usize;
        // Walk the extension-header chain to the transport header.
        let mut next = hdr[6];
        let mut o = 40usize;
        for _ in 0..8 {
            match next {
                0 | 43 | 60 | 51 => {
                    let ext_len = *hdr.get(o + 1).unwrap_or(&0) as usize;
                    next = *hdr.get(o).unwrap_or(&59);
                    o += (ext_len + 1) * 8;
                }
                44 => {
                    // Fragment header: only the first fragment is usable.
                    let frag_off = u16be(hdr, o + 2).unwrap_or(0) & 0xfff8;
                    if frag_off != 0 {
                        col.fragments_skipped += 1;
                        return;
                    }
                    next = *hdr.get(o).unwrap_or(&59);
                    o += 8;
                }
                _ => break,
            }
        }
        let total = (payload_len + 40).min(hdr.len());
        (Ip::V6(src), Ip::V6(dst), next, o, total.saturating_sub(o))
    } else {
        if hdr.len() < 20 {
            return;
        }
        let ihl = ((hdr[0] & 0x0f) as usize) * 4;
        if ihl < 20 || hdr.len() < ihl {
            return;
        }
        let total_len = (u16be(hdr, 2).unwrap_or(0) as usize).min(hdr.len());
        let frag = u16be(hdr, 6).unwrap_or(0);
        if frag & 0x1fff != 0 {
            col.fragments_skipped += 1;
            return;
        }
        let mut src = [0u8; 4];
        let mut dst = [0u8; 4];
        src.copy_from_slice(&hdr[12..16]);
        dst.copy_from_slice(&hdr[16..20]);
        (Ip::V4(src), Ip::V4(dst), hdr[9], ihl, total_len.saturating_sub(ihl))
    };

    if proto != IPPROTO_TCP || l4_len < 20 {
        return;
    }
    let tcp = match b.get(off + l4_off..off + l4_off + l4_len) {
        Some(s) => s,
        None => return,
    };
    let sport = u16be(tcp, 0).unwrap_or(0);
    let dport = u16be(tcp, 2).unwrap_or(0);
    let seq = u32be(tcp, 4).unwrap_or(0);
    let doff = ((tcp[12] >> 4) as usize) * 4;
    if doff < 20 || doff > tcp.len() {
        return;
    }
    let flags = tcp[13];
    let syn = flags & 0x02 != 0;
    let ack = flags & 0x10 != 0;
    let payload_len = tcp.len() - doff;
    let key = FlowKey { src, sport, dst, dport };
    let entry = col.flows.entry(key).or_insert_with(|| Flow {
        first_pkt: col.packets,
        first_ts: ts,
        ..Flow::default()
    });
    if syn {
        entry.syn_seq.get_or_insert(seq);
        if !ack {
            entry.saw_syn_only = true;
        }
    }
    if payload_len > 0 && col.segments < MAX_SEGMENTS {
        col.segments += 1;
        entry.segments.push(Segment {
            seq,
            off: off + l4_off + doff,
            len: payload_len,
            pkt: col.packets,
            ts,
        });
    }
}

/// Reassemble one direction: sort by sequence, drop retransmissions, zero-fill
/// holes, and record where each segment landed.
fn reassemble(bytes: &[u8], flow: &Flow, budget: &mut usize) -> Stream {
    let mut out = Stream::default();
    if flow.segments.is_empty() {
        return out;
    }
    // Anchor at the SYN's ISN+1 when we saw the handshake, otherwise at the
    // lowest sequence we observed (a mid-conversation capture).
    let base = match flow.syn_seq {
        Some(s) => s.wrapping_add(1),
        None => flow.segments.iter().map(|s| s.seq).min().unwrap_or(0),
    };
    let mut rel: Vec<(usize, &Segment)> = Vec::with_capacity(flow.segments.len());
    for seg in &flow.segments {
        let delta = seg.seq.wrapping_sub(base);
        // A delta in the top half of the u32 space means "before the anchor" —
        // a keep-alive or a pre-capture retransmission. Skip it.
        if delta >= 0x8000_0000 {
            continue;
        }
        rel.push((delta as usize, seg));
    }
    if rel.is_empty() {
        return out;
    }
    rel.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.len.cmp(&a.1.len)));
    let max_end = rel.iter().map(|(o, s)| o + s.len).max().unwrap_or(0);
    let cap = max_end.min(*budget);
    if cap < max_end {
        out.truncated = true;
    }
    if cap == 0 {
        return out;
    }
    *budget -= cap;
    out.data = vec![0u8; cap];
    let mut covered = 0usize;
    for (start, seg) in rel {
        let end = (start + seg.len).min(cap);
        if end <= covered {
            continue; // pure retransmission
        }
        if start > covered {
            out.missing += start - covered;
        }
        let copy_from = covered.max(start);
        let src_skip = copy_from - start;
        if src_skip < seg.len {
            let src = &bytes[seg.off + src_skip..seg.off + seg.len.min(src_skip + (end - copy_from))];
            out.data[copy_from..copy_from + src.len()].copy_from_slice(src);
            out.marks.push((copy_from, seg.pkt, seg.ts));
        }
        covered = end;
        if covered >= cap {
            break;
        }
    }
    out
}

fn build_conns(bytes: &[u8], flows: BTreeMap<FlowKey, Flow>) -> (Vec<Conn>, bool) {
    let mut budget = STREAM_BUDGET;
    let mut budget_hit = false;
    let mut done: BTreeMap<FlowKey, ()> = BTreeMap::new();
    let mut conns = Vec::new();
    for (key, flow) in &flows {
        if done.contains_key(key) {
            continue;
        }
        let rev = key.reversed();
        done.insert(*key, ());
        done.insert(rev, ());
        let back = flows.get(&rev);
        // The client is whoever sent a bare SYN; failing that, whoever spoke
        // first in the capture.
        let forward_is_client = match (flow.saw_syn_only, back.map(|f| f.saw_syn_only)) {
            (true, _) => true,
            (false, Some(true)) => false,
            _ => back.map(|f| flow.first_pkt <= f.first_pkt).unwrap_or(true),
        };
        let (ckey, cflow, sflow) = if forward_is_client {
            (*key, flow, back)
        } else {
            (rev, back.unwrap(), Some(flow))
        };
        let c2s = reassemble(bytes, cflow, &mut budget);
        let s2c = sflow.map(|f| reassemble(bytes, f, &mut budget)).unwrap_or_default();
        if c2s.truncated || s2c.truncated {
            budget_hit = true;
        }
        if c2s.data.is_empty() && s2c.data.is_empty() {
            continue;
        }
        let first_pkt = sflow
            .map(|f| cflow.first_pkt.min(f.first_pkt))
            .unwrap_or(cflow.first_pkt);
        let first_ts = sflow
            .map(|f| if f.first_pkt < cflow.first_pkt { f.first_ts } else { cflow.first_ts })
            .unwrap_or(cflow.first_ts);
        conns.push(Conn {
            client_ip: ckey.src,
            client_port: ckey.sport,
            server_ip: ckey.dst,
            server_port: ckey.dport,
            c2s,
            s2c,
            first_pkt,
            first_ts,
        });
    }
    conns.sort_by_key(|c| c.first_pkt);
    (conns, budget_hit)
}

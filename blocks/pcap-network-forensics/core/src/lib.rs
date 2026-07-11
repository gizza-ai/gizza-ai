//! pcap-network-forensics core — pure, dependency-free forensic aggregator for
//! libpcap (`.pcap`) and pcapng (`.pcapng`) captures.
//!
//! Walks the container directly from its byte layout (no external deps → runs on
//! every backend, incl. the chat Service Worker), decodes the Ethernet / raw-IP /
//! Linux-cooked / loopback link layers plus IPv4/IPv6 and TCP/UDP, and aggregates
//! the traffic into a forensic report:
//!   * `hosts`         — per-IP inventory (packets + bytes),
//!   * `conversations` — bidirectional TCP/UDP flows (endpoints + packets + bytes),
//!   * `dns_queries`   — decoded DNS questions (+ any A/AAAA answers),
//!   * `http_requests` — decoded HTTP request lines (method / host / path / UA),
//!   * `credentials`   — cleartext logins observed on unencrypted protocols
//!                       (HTTP Basic, HTTP form POST, FTP, POP3).
//!
//! This is a higher-level lens than the sibling `parse-pcap` per-packet decoder;
//! it does not reproduce the packet dump.

use std::collections::HashMap;
use std::fmt::Write as _;

/// Per-IP endpoint record.
#[derive(Debug, Clone, PartialEq)]
pub struct Host {
    /// IP address (IPv4 dotted or IPv6 compressed).
    pub address: String,
    /// Packets in which this host was the source or destination.
    pub packets: u64,
    /// Total on-wire bytes of packets involving this host.
    pub bytes: u64,
}

/// A bidirectional TCP/UDP conversation between two endpoints.
#[derive(Debug, Clone, PartialEq)]
pub struct Conversation {
    /// Transport protocol: `TCP` or `UDP`.
    pub protocol: &'static str,
    /// First endpoint seen, as `ip:port`.
    pub endpoint_a: String,
    /// Second endpoint, as `ip:port`.
    pub endpoint_b: String,
    /// Packets exchanged in either direction.
    pub packets: u64,
    /// Total on-wire bytes exchanged in either direction.
    pub bytes: u64,
}

/// A decoded DNS question (with any A/AAAA answers seen for the same name/type).
#[derive(Debug, Clone, PartialEq)]
pub struct DnsQuery {
    /// Queried domain name.
    pub name: String,
    /// Record type (e.g. `A`, `AAAA`, `CNAME`, `MX`).
    pub qtype: String,
    /// Resolved A/AAAA addresses observed in responses, if any.
    pub answers: Vec<String>,
}

/// A decoded cleartext HTTP request.
#[derive(Debug, Clone, PartialEq)]
pub struct HttpRequest {
    /// HTTP method (`GET`, `POST`, …).
    pub method: String,
    /// `Host` header value (empty if absent).
    pub host: String,
    /// Request target / path.
    pub path: String,
    /// `User-Agent` header value (empty if absent).
    pub user_agent: String,
}

/// A cleartext credential observed on an unencrypted protocol.
#[derive(Debug, Clone, PartialEq)]
pub struct Credential {
    /// How it was found: `HTTP Basic Auth`, `HTTP form login`, `FTP`, `POP3`.
    pub kind: String,
    /// Application protocol: `HTTP`, `FTP`, `POP3`.
    pub protocol: String,
    /// Client endpoint (`ip:port`).
    pub source: String,
    /// Server endpoint (`ip:port`).
    pub destination: String,
    /// Recovered username (may be empty).
    pub username: String,
    /// Recovered password (may be empty).
    pub password: String,
}

/// The complete forensic report. Each `*_total` field is the count before the
/// per-section `limit` truncated the corresponding list.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    /// Container format: `pcap` or `pcapng`.
    pub format: &'static str,
    /// Link-layer type of the first interface (e.g. `Ethernet`).
    pub link_type: String,
    /// Total packets present in the capture.
    pub total_packets: usize,
    /// Capture span in seconds (last − first timestamp; 0 when timestamps absent).
    pub duration_seconds: f64,
    pub hosts_total: usize,
    pub hosts: Vec<Host>,
    pub conversations_total: usize,
    pub conversations: Vec<Conversation>,
    pub dns_queries_total: usize,
    pub dns_queries: Vec<DnsQuery>,
    pub http_requests_total: usize,
    pub http_requests: Vec<HttpRequest>,
    pub credentials_total: usize,
    pub credentials: Vec<Credential>,
}

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

/// Analyze a whole capture, capping each output section to `limit` entries.
pub fn analyze(bytes: &[u8], limit: usize) -> Result<Report, String> {
    if bytes.len() < 4 {
        return Err("file is too small to be a pcap/pcapng capture".into());
    }
    let mut agg = Aggregator::default();
    let magic = [bytes[0], bytes[1], bytes[2], bytes[3]];
    match magic {
        [0xa1, 0xb2, 0xc3, 0xd4] | [0xa1, 0xb2, 0x3c, 0x4d] => walk_classic(bytes, false, &mut agg)?,
        [0xd4, 0xc3, 0xb2, 0xa1] | [0x4d, 0x3c, 0xb2, 0xa1] => walk_classic(bytes, true, &mut agg)?,
        [0x0a, 0x0d, 0x0d, 0x0a] => walk_pcapng(bytes, &mut agg)?,
        _ => {
            return Err("not a pcap or pcapng capture (unrecognised magic bytes); expected a \
                        libpcap (.pcap) or pcapng (.pcapng) file"
                .into())
        }
    }
    Ok(agg.finish(limit))
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
        agg.add_packet(ts, orig_len, link_type, &b[data_off..data_off + cap_len]);
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
            1 => {
                let body = &b[off + 8..off + blen - 4];
                let lt = if le { rd_u16_le(body, 0) } else { rd_u16_be(body, 0) }.unwrap_or(0) as u32;
                if if_linktypes.is_empty() {
                    agg.link_type = link_type_name(lt);
                }
                if_linktypes.push(lt);
                if_tsresol.push(pcapng_tsresol(body, 8, le).unwrap_or(1_000_000.0));
            }
            6 => {
                let if_id = r32(off + 8).unwrap_or(0) as usize;
                let ts_high = r32(off + 12).unwrap_or(0) as u64;
                let ts_low = r32(off + 16).unwrap_or(0) as u64;
                let cap_len = r32(off + 20).unwrap_or(0) as usize;
                let orig_len = r32(off + 24).unwrap_or(0);
                let data_start = off + 28;
                if data_start + cap_len <= off + blen - 4 {
                    let div = if_tsresol.get(if_id).copied().unwrap_or(1_000_000.0);
                    let ts = ((ts_high << 32) | ts_low) as f64 / div;
                    let lt = if_linktypes.get(if_id).copied().unwrap_or(LINKTYPE_ETHERNET);
                    agg.add_packet(ts, orig_len, lt, &b[data_start..data_start + cap_len]);
                }
            }
            3 => {
                let orig_len = r32(off + 8).unwrap_or(0);
                let data_start = off + 12;
                let avail = (off + blen - 4).saturating_sub(data_start);
                let cl = (orig_len as usize).min(avail);
                if data_start + cl <= b.len() {
                    let lt = if_linktypes.first().copied().unwrap_or(LINKTYPE_ETHERNET);
                    agg.add_packet(0.0, orig_len, lt, &b[data_start..data_start + cl]);
                }
            }
            2 => {
                let if_id = r16(off + 8).unwrap_or(0) as usize;
                let ts_high = r32(off + 12).unwrap_or(0) as u64;
                let ts_low = r32(off + 16).unwrap_or(0) as u64;
                let cap_len = r32(off + 20).unwrap_or(0) as usize;
                let orig_len = r32(off + 24).unwrap_or(0);
                let data_start = off + 28;
                if data_start + cap_len <= off + blen - 4 {
                    let div = if_tsresol.get(if_id).copied().unwrap_or(1_000_000.0);
                    let ts = ((ts_high << 32) | ts_low) as f64 / div;
                    let lt = if_linktypes.get(if_id).copied().unwrap_or(LINKTYPE_ETHERNET);
                    agg.add_packet(ts, orig_len, lt, &b[data_start..data_start + cap_len]);
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
// Aggregation
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Aggregator {
    format: &'static str,
    link_type: String,
    total_packets: usize,
    first_ts: Option<f64>,
    last_ts: Option<f64>,
    hosts: HashMap<String, (u64, u64)>,
    convs: HashMap<String, ConvAcc>,
    dns: Vec<DnsQuery>,
    dns_index: HashMap<(String, String), usize>,
    http: Vec<HttpRequest>,
    creds: Vec<Credential>,
    /// Pending `USER` values for FTP/POP3, keyed by `proto client->server` flow.
    pending_user: HashMap<String, String>,
}

struct ConvAcc {
    protocol: &'static str,
    endpoint_a: String,
    endpoint_b: String,
    packets: u64,
    bytes: u64,
}

const IPPROTO_TCP: u8 = 6;
const IPPROTO_UDP: u8 = 17;

impl Aggregator {
    fn add_packet(&mut self, ts: f64, orig_len: u32, link_type: u32, data: &[u8]) {
        self.total_packets += 1;
        if ts > 0.0 {
            self.first_ts = Some(self.first_ts.map_or(ts, |v| v.min(ts)));
            self.last_ts = Some(self.last_ts.map_or(ts, |v| v.max(ts)));
        }
        if let Some((l3, et)) = link_layer(data, link_type) {
            match et {
                EtherType::Ipv4 => self.ipv4(l3, orig_len),
                EtherType::Ipv6 => self.ipv6(l3, orig_len),
                EtherType::Other => {}
            }
        }
    }

    fn ipv4(&mut self, data: &[u8], orig_len: u32) {
        if data.len() < 20 {
            return;
        }
        let ihl = (data[0] & 0x0f) as usize * 4;
        let proto = data[9];
        let src = fmt_ipv4(&data[12..16]);
        let dst = fmt_ipv4(&data[16..20]);
        let l4 = if data.len() >= ihl { &data[ihl..] } else { &[] };
        self.l3_common(src, dst, proto, l4, orig_len);
    }

    fn ipv6(&mut self, data: &[u8], orig_len: u32) {
        if data.len() < 40 {
            return;
        }
        let next = data[6];
        let src = fmt_ipv6(&data[8..24]);
        let dst = fmt_ipv6(&data[24..40]);
        self.l3_common(src, dst, next, &data[40..], orig_len);
    }

    fn l3_common(&mut self, src: String, dst: String, proto: u8, l4: &[u8], orig_len: u32) {
        // Host inventory (per IP).
        for ip in [&src, &dst] {
            let e = self.hosts.entry(ip.clone()).or_insert((0, 0));
            e.0 += 1;
            e.1 += orig_len as u64;
        }
        let (protocol, sp, dp, payload): (&'static str, u16, u16, &[u8]) = match proto {
            IPPROTO_TCP if l4.len() >= 20 => {
                let sp = rd_u16_be(l4, 0).unwrap();
                let dp = rd_u16_be(l4, 2).unwrap();
                let doff = ((l4[12] >> 4) as usize) * 4;
                let pl = if l4.len() >= doff { &l4[doff..] } else { &[] };
                ("TCP", sp, dp, pl)
            }
            IPPROTO_UDP if l4.len() >= 8 => {
                let sp = rd_u16_be(l4, 0).unwrap();
                let dp = rd_u16_be(l4, 2).unwrap();
                ("UDP", sp, dp, &l4[8..])
            }
            _ => return,
        };

        // Conversation (bidirectional).
        let ep_src = format!("{src}:{sp}");
        let ep_dst = format!("{dst}:{dp}");
        let key = conv_key(protocol, &ep_src, &ep_dst);
        let c = self.convs.entry(key).or_insert_with(|| ConvAcc {
            protocol,
            endpoint_a: ep_src.clone(),
            endpoint_b: ep_dst.clone(),
            packets: 0,
            bytes: 0,
        });
        c.packets += 1;
        c.bytes += orig_len as u64;

        // Application-layer extraction.
        if protocol == "UDP" && (sp == 53 || dp == 53) {
            self.dns(payload);
        }
        if protocol == "TCP" {
            if (sp == 53 || dp == 53) && payload.len() > 2 {
                // DNS over TCP carries a 2-byte length prefix.
                self.dns(&payload[2..]);
            }
            if !payload.is_empty() {
                self.http(payload, &ep_src, &ep_dst);
                if sp == 21 || dp == 21 {
                    self.line_creds("FTP", payload, sp == 21, &ep_src, &ep_dst);
                }
                if sp == 110 || dp == 110 {
                    self.line_creds("POP3", payload, sp == 110, &ep_src, &ep_dst);
                }
            }
        }
    }

    fn dns(&mut self, msg: &[u8]) {
        if msg.len() < 12 {
            return;
        }
        let qr = msg[2] & 0x80 != 0;
        let qd = u16::from_be_bytes([msg[4], msg[5]]) as usize;
        let an = u16::from_be_bytes([msg[6], msg[7]]) as usize;
        let mut off = 12usize;
        let mut first_key: Option<(String, String)> = None;
        for i in 0..qd {
            let (name, next) = match read_name(msg, off) {
                Some(v) => v,
                None => return,
            };
            if next + 4 > msg.len() {
                return;
            }
            let qtype = u16::from_be_bytes([msg[next], msg[next + 1]]);
            off = next + 4;
            let tname = dns_type_name(qtype);
            if i == 0 {
                first_key = Some((name.clone(), tname.clone()));
            }
            self.record_dns(name, tname, Vec::new());
        }
        // Parse A/AAAA answers and attach them to the first question.
        if qr && an > 0 {
            if let Some((qname, qtype)) = first_key {
                let mut answers = Vec::new();
                for _ in 0..an {
                    let (_n, next) = match read_name(msg, off) {
                        Some(v) => v,
                        None => break,
                    };
                    if next + 10 > msg.len() {
                        break;
                    }
                    let atype = u16::from_be_bytes([msg[next], msg[next + 1]]);
                    let rdlen = u16::from_be_bytes([msg[next + 8], msg[next + 9]]) as usize;
                    let rdata = next + 10;
                    if rdata + rdlen > msg.len() {
                        break;
                    }
                    if atype == 1 && rdlen == 4 {
                        answers.push(fmt_ipv4(&msg[rdata..rdata + 4]));
                    } else if atype == 28 && rdlen == 16 {
                        answers.push(fmt_ipv6(&msg[rdata..rdata + 16]));
                    }
                    off = rdata + rdlen;
                }
                if !answers.is_empty() {
                    self.record_dns(qname, qtype, answers);
                }
            }
        }
    }

    fn record_dns(&mut self, name: String, qtype: String, answers: Vec<String>) {
        let key = (name.clone(), qtype.clone());
        if let Some(&idx) = self.dns_index.get(&key) {
            for a in answers {
                if !self.dns[idx].answers.contains(&a) {
                    self.dns[idx].answers.push(a);
                }
            }
        } else {
            self.dns_index.insert(key, self.dns.len());
            self.dns.push(DnsQuery { name, qtype, answers });
        }
    }

    fn http(&mut self, payload: &[u8], client: &str, server: &str) {
        let Some(req) = parse_http_request(payload) else {
            return;
        };
        if let Some(b) = &req.basic_auth {
            let (u, p) = split_userpass(b);
            self.creds.push(Credential {
                kind: "HTTP Basic Auth".into(),
                protocol: "HTTP".into(),
                source: client.into(),
                destination: server.into(),
                username: u,
                password: p,
            });
        }
        if let Some((u, p)) = &req.form_login {
            self.creds.push(Credential {
                kind: "HTTP form login".into(),
                protocol: "HTTP".into(),
                source: client.into(),
                destination: server.into(),
                username: u.clone(),
                password: p.clone(),
            });
        }
        self.http.push(HttpRequest {
            method: req.method,
            host: req.host,
            path: req.path,
            user_agent: req.user_agent,
        });
    }

    /// FTP/POP3 share the `USER <name>` / `PASS <secret>` command shape.
    fn line_creds(&mut self, proto: &str, payload: &[u8], server_is_src: bool, a: &str, b: &str) {
        // Commands flow client->server; skip server responses.
        if server_is_src {
            return;
        }
        let text = String::from_utf8_lossy(payload);
        let flow = format!("{proto} {a}->{b}");
        for line in text.split(['\r', '\n']) {
            let line = line.trim();
            if line.len() < 5 {
                continue;
            }
            let (cmd, rest) = line.split_at(4);
            let rest = rest.trim();
            if cmd.eq_ignore_ascii_case("USER") {
                self.pending_user.insert(flow.clone(), rest.to_string());
            } else if cmd.eq_ignore_ascii_case("PASS") {
                let username = self.pending_user.remove(&flow).unwrap_or_default();
                self.creds.push(Credential {
                    kind: proto.into(),
                    protocol: proto.into(),
                    source: a.into(),
                    destination: b.into(),
                    username,
                    password: rest.to_string(),
                });
            }
        }
    }

    fn finish(self, limit: usize) -> Report {
        let duration = match (self.first_ts, self.last_ts) {
            (Some(a), Some(b)) => (b - a).max(0.0),
            _ => 0.0,
        };

        let mut hosts: Vec<Host> = self
            .hosts
            .into_iter()
            .map(|(address, (packets, bytes))| Host { address, packets, bytes })
            .collect();
        hosts.sort_by(|a, b| b.packets.cmp(&a.packets).then(a.address.cmp(&b.address)));
        let hosts_total = hosts.len();
        hosts.truncate(limit);

        let mut conversations: Vec<Conversation> = self
            .convs
            .into_values()
            .map(|c| Conversation {
                protocol: c.protocol,
                endpoint_a: c.endpoint_a,
                endpoint_b: c.endpoint_b,
                packets: c.packets,
                bytes: c.bytes,
            })
            .collect();
        conversations.sort_by(|a, b| {
            b.packets
                .cmp(&a.packets)
                .then(a.endpoint_a.cmp(&b.endpoint_a))
                .then(a.endpoint_b.cmp(&b.endpoint_b))
        });
        let conversations_total = conversations.len();
        conversations.truncate(limit);

        let dns_queries_total = self.dns.len();
        let mut dns_queries = self.dns;
        dns_queries.truncate(limit);

        let http_requests_total = self.http.len();
        let mut http_requests = self.http;
        http_requests.truncate(limit);

        let credentials_total = self.creds.len();
        let mut credentials = self.creds;
        credentials.truncate(limit);

        Report {
            format: self.format,
            link_type: self.link_type,
            total_packets: self.total_packets,
            duration_seconds: duration,
            hosts_total,
            hosts,
            conversations_total,
            conversations,
            dns_queries_total,
            dns_queries,
            http_requests_total,
            http_requests,
            credentials_total,
            credentials,
        }
    }
}

/// Canonical (direction-insensitive) conversation key.
fn conv_key(proto: &str, a: &str, b: &str) -> String {
    if a <= b {
        format!("{proto}|{a}|{b}")
    } else {
        format!("{proto}|{b}|{a}")
    }
}

// ---------------------------------------------------------------------------
// Link / network layer decoding
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum EtherType {
    Ipv4,
    Ipv6,
    Other,
}

fn ethertype_kind(t: u16) -> EtherType {
    match t {
        0x0800 => EtherType::Ipv4,
        0x86dd => EtherType::Ipv6,
        _ => EtherType::Other,
    }
}

/// Strip the link layer, returning the L3 payload + its EtherType.
fn link_layer(data: &[u8], link_type: u32) -> Option<(&[u8], EtherType)> {
    match link_type {
        LINKTYPE_ETHERNET => {
            if data.len() < 14 {
                return None;
            }
            let mut etype = rd_u16_be(data, 12)?;
            let mut off = 14;
            while etype == 0x8100 || etype == 0x88a8 {
                if data.len() < off + 4 {
                    return None;
                }
                etype = rd_u16_be(data, off + 2)?;
                off += 4;
            }
            Some((&data[off..], ethertype_kind(etype)))
        }
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
                Some((&data[16..], ethertype_kind(rd_u16_be(data, 14).unwrap_or(0))))
            } else {
                None
            }
        }
        _ => None,
    }
}

fn fmt_ipv4(b: &[u8]) -> String {
    format!("{}.{}.{}.{}", b[0], b[1], b[2], b[3])
}

fn fmt_ipv6(b: &[u8]) -> String {
    let groups: Vec<u16> = (0..8)
        .map(|i| u16::from_be_bytes([b[i * 2], b[i * 2 + 1]]))
        .collect();
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

// ---------------------------------------------------------------------------
// DNS name decoding
// ---------------------------------------------------------------------------

/// Read a (possibly compressed) DNS name; returns the decoded name and the
/// offset just past the name in the *record* (following any compression pointer).
fn read_name(msg: &[u8], start: usize) -> Option<(String, usize)> {
    let mut labels: Vec<String> = Vec::new();
    let mut off = start;
    let mut jumped = false;
    let mut next_after = start;
    let mut guard = 0;
    loop {
        guard += 1;
        if guard > 128 {
            return None;
        }
        let len = *msg.get(off)? as usize;
        if len == 0 {
            if !jumped {
                next_after = off + 1;
            }
            break;
        }
        if len & 0xc0 == 0xc0 {
            let ptr = ((len & 0x3f) << 8) | *msg.get(off + 1)? as usize;
            if !jumped {
                next_after = off + 2;
            }
            jumped = true;
            off = ptr;
            continue;
        }
        let s = off + 1;
        let e = s + len;
        if e > msg.len() {
            return None;
        }
        labels.push(String::from_utf8_lossy(&msg[s..e]).into_owned());
        off = e;
    }
    let name = if labels.is_empty() { ".".to_string() } else { labels.join(".") };
    Some((name, next_after))
}

fn dns_type_name(t: u16) -> String {
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
        65 => "HTTPS",
        255 => "ANY",
        _ => return format!("TYPE{t}"),
    }
    .to_string()
}

// ---------------------------------------------------------------------------
// HTTP request parsing
// ---------------------------------------------------------------------------

struct ParsedHttp {
    method: String,
    path: String,
    host: String,
    user_agent: String,
    basic_auth: Option<String>,
    form_login: Option<(String, String)>,
}

const HTTP_METHODS: [&str; 9] = [
    "GET", "POST", "PUT", "HEAD", "DELETE", "OPTIONS", "PATCH", "TRACE", "CONNECT",
];

fn parse_http_request(payload: &[u8]) -> Option<ParsedHttp> {
    let text = String::from_utf8_lossy(payload);
    // Split into lines tolerating CRLF or bare LF.
    let lines: Vec<&str> = text.split('\n').map(|l| l.trim_end_matches('\r')).collect();
    let head = lines.first().copied().unwrap_or("");

    let mut parts = head.split(' ');
    let method = parts.next()?.to_string();
    if !HTTP_METHODS.contains(&method.as_str()) {
        return None;
    }
    let path = parts.next()?.to_string();
    let version = parts.next().unwrap_or("");
    if !version.starts_with("HTTP/") {
        return None;
    }

    let mut host = String::new();
    let mut user_agent = String::new();
    let mut basic_auth = None;
    let mut content_type = String::new();
    let mut body_start = None;

    for (i, line) in lines.iter().enumerate() {
        if i == 0 {
            continue;
        }
        if line.is_empty() {
            body_start = Some(i + 1);
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            let value = value.trim();
            if name.eq_ignore_ascii_case("host") {
                host = value.to_string();
            } else if name.eq_ignore_ascii_case("user-agent") {
                user_agent = value.to_string();
            } else if name.eq_ignore_ascii_case("content-type") {
                content_type = value.to_ascii_lowercase();
            } else if name.eq_ignore_ascii_case("authorization") {
                let lower = value.to_ascii_lowercase();
                if let Some(rest) = lower.strip_prefix("basic ") {
                    // Recover the original-case token at the same offset.
                    let tok = &value[value.len() - rest.len()..];
                    if let Some(decoded) = b64_decode(tok.trim()) {
                        basic_auth = Some(String::from_utf8_lossy(&decoded).into_owned());
                    }
                }
            }
        }
    }

    let mut form_login = None;
    if method == "POST" && content_type.contains("application/x-www-form-urlencoded") {
        if let Some(bs) = body_start {
            let body = lines[bs..].join("\n");
            form_login = parse_form_login(&body);
        }
    }

    Some(ParsedHttp {
        method,
        path,
        host,
        user_agent,
        basic_auth,
        form_login,
    })
}

fn parse_form_login(body: &str) -> Option<(String, String)> {
    const USER_KEYS: [&str; 8] =
        ["username", "user", "login", "email", "userid", "user_name", "usr", "uname"];
    const PASS_KEYS: [&str; 5] = ["password", "passwd", "pass", "pwd", "pw"];
    let mut username = String::new();
    let mut password = String::new();
    for pair in body.trim().split('&') {
        let Some((k, v)) = pair.split_once('=') else {
            continue;
        };
        let key = k.trim().to_ascii_lowercase();
        if username.is_empty() && USER_KEYS.contains(&key.as_str()) {
            username = url_decode(v);
        } else if password.is_empty() && PASS_KEYS.contains(&key.as_str()) {
            password = url_decode(v);
        }
    }
    if password.is_empty() {
        None
    } else {
        Some((username, password))
    }
}

fn split_userpass(s: &str) -> (String, String) {
    match s.split_once(':') {
        Some((u, p)) => (u.to_string(), p.to_string()),
        None => (s.to_string(), String::new()),
    }
}

fn b64_decode(s: &str) -> Option<Vec<u8>> {
    fn val(c: u8) -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let mut bits = 0u32;
    let mut nbits = 0u32;
    let mut out = Vec::new();
    for &c in s.as_bytes() {
        if c == b'=' || c.is_ascii_whitespace() {
            continue;
        }
        let v = val(c)?;
        bits = (bits << 6) | v as u32;
        nbits += 6;
        if nbits >= 8 {
            nbits -= 8;
            out.push((bits >> nbits) as u8);
        }
    }
    Some(out)
}

fn url_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < b.len() => match (hexval(b[i + 1]), hexval(b[i + 2])) {
                (Some(h), Some(l)) => {
                    out.push((h << 4) | l);
                    i += 3;
                }
                _ => {
                    out.push(b'%');
                    i += 1;
                }
            },
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hexval(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Assemble a classic little-endian pcap from a list of Ethernet frames.
    fn pcap(frames: &[Vec<u8>]) -> Vec<u8> {
        let mut f = Vec::new();
        f.extend_from_slice(&[0xd4, 0xc3, 0xb2, 0xa1]); // le magic, microseconds
        f.extend_from_slice(&2u16.to_le_bytes());
        f.extend_from_slice(&4u16.to_le_bytes());
        f.extend_from_slice(&0u32.to_le_bytes());
        f.extend_from_slice(&0u32.to_le_bytes());
        f.extend_from_slice(&65535u32.to_le_bytes());
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
            t.push(0x50); // data offset 5 words (20 bytes)
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

    #[test]
    fn rejects_non_pcap() {
        let err = analyze(b"not a capture file at all", 100).unwrap_err();
        assert!(err.contains("not a pcap"));
    }

    #[test]
    fn hosts_and_conversations_aggregate_bidirectionally() {
        // Two packets each way on the same TCP flow → one conversation, 4 packets.
        let cap = pcap(&[
            frame(IPPROTO_TCP, A, B, 1234, 80, b"a"),
            frame(IPPROTO_TCP, B, A, 80, 1234, b"b"),
            frame(IPPROTO_TCP, A, B, 1234, 80, b"c"),
            frame(IPPROTO_TCP, B, A, 80, 1234, b"d"),
        ]);
        let r = analyze(&cap, 100).unwrap();
        assert_eq!(r.format, "pcap");
        assert_eq!(r.link_type, "Ethernet");
        assert_eq!(r.total_packets, 4);
        assert_eq!(r.hosts_total, 2);
        assert_eq!(r.conversations_total, 1);
        assert_eq!(r.conversations[0].packets, 4);
        assert_eq!(r.hosts[0].packets, 4); // each host is in all 4 packets
    }

    /// Build a DNS query for `name` (type A). `qr`+answer optional.
    fn dns_query_msg(name: &str) -> Vec<u8> {
        let mut m = Vec::new();
        m.extend_from_slice(&0x1234u16.to_be_bytes()); // id
        m.extend_from_slice(&0x0100u16.to_be_bytes()); // flags: RD
        m.extend_from_slice(&1u16.to_be_bytes()); // qd
        m.extend_from_slice(&0u16.to_be_bytes()); // an
        m.extend_from_slice(&0u16.to_be_bytes()); // ns
        m.extend_from_slice(&0u16.to_be_bytes()); // ar
        for label in name.split('.') {
            m.push(label.len() as u8);
            m.extend_from_slice(label.as_bytes());
        }
        m.push(0);
        m.extend_from_slice(&1u16.to_be_bytes()); // type A
        m.extend_from_slice(&1u16.to_be_bytes()); // class IN
        m
    }

    #[test]
    fn extracts_dns_query() {
        let cap = pcap(&[frame(IPPROTO_UDP, A, B, 5000, 53, &dns_query_msg("example.com"))]);
        let r = analyze(&cap, 100).unwrap();
        assert_eq!(r.dns_queries_total, 1);
        assert_eq!(r.dns_queries[0].name, "example.com");
        assert_eq!(r.dns_queries[0].qtype, "A");
    }

    #[test]
    fn extracts_dns_answer_with_compression() {
        // Response: 1 question (example.com A) + 1 answer (pointer to name, A 93.184.216.34).
        let mut m = Vec::new();
        m.extend_from_slice(&0x1234u16.to_be_bytes());
        m.extend_from_slice(&0x8180u16.to_be_bytes()); // QR + RD + RA
        m.extend_from_slice(&1u16.to_be_bytes());
        m.extend_from_slice(&1u16.to_be_bytes());
        m.extend_from_slice(&0u16.to_be_bytes());
        m.extend_from_slice(&0u16.to_be_bytes());
        let name_off = m.len();
        for label in "example.com".split('.') {
            m.push(label.len() as u8);
            m.extend_from_slice(label.as_bytes());
        }
        m.push(0);
        m.extend_from_slice(&1u16.to_be_bytes());
        m.extend_from_slice(&1u16.to_be_bytes());
        // answer
        m.push(0xc0);
        m.push(name_off as u8); // pointer to the question name
        m.extend_from_slice(&1u16.to_be_bytes()); // type A
        m.extend_from_slice(&1u16.to_be_bytes()); // class
        m.extend_from_slice(&300u32.to_be_bytes()); // ttl
        m.extend_from_slice(&4u16.to_be_bytes()); // rdlength
        m.extend_from_slice(&[93, 184, 216, 34]);
        let cap = pcap(&[frame(IPPROTO_UDP, B, A, 53, 5000, &m)]);
        let r = analyze(&cap, 100).unwrap();
        assert_eq!(r.dns_queries_total, 1);
        assert_eq!(r.dns_queries[0].answers, vec!["93.184.216.34".to_string()]);
    }

    #[test]
    fn extracts_http_request_and_basic_auth() {
        // "user:pass" base64 = dXNlcjpwYXNz
        let req = b"GET /admin HTTP/1.1\r\nHost: victim.example\r\nUser-Agent: curl/8.0\r\n\
                    Authorization: Basic dXNlcjpwYXNz\r\n\r\n";
        let cap = pcap(&[frame(IPPROTO_TCP, A, B, 40000, 80, req)]);
        let r = analyze(&cap, 100).unwrap();
        assert_eq!(r.http_requests_total, 1);
        let h = &r.http_requests[0];
        assert_eq!(h.method, "GET");
        assert_eq!(h.host, "victim.example");
        assert_eq!(h.path, "/admin");
        assert_eq!(h.user_agent, "curl/8.0");
        assert_eq!(r.credentials_total, 1);
        assert_eq!(r.credentials[0].kind, "HTTP Basic Auth");
        assert_eq!(r.credentials[0].username, "user");
        assert_eq!(r.credentials[0].password, "pass");
    }

    #[test]
    fn extracts_http_form_login() {
        let body = "username=alice&password=s3cr3t%21";
        let req = format!(
            "POST /login HTTP/1.1\r\nHost: app.example\r\n\
             Content-Type: application/x-www-form-urlencoded\r\n\
             Content-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        let cap = pcap(&[frame(IPPROTO_TCP, A, B, 40001, 80, req.as_bytes())]);
        let r = analyze(&cap, 100).unwrap();
        assert_eq!(r.credentials_total, 1);
        assert_eq!(r.credentials[0].kind, "HTTP form login");
        assert_eq!(r.credentials[0].username, "alice");
        assert_eq!(r.credentials[0].password, "s3cr3t!");
    }

    #[test]
    fn pairs_ftp_user_and_pass() {
        let cap = pcap(&[
            frame(IPPROTO_TCP, A, B, 50000, 21, b"USER bob\r\n"),
            frame(IPPROTO_TCP, A, B, 50000, 21, b"PASS hunter2\r\n"),
        ]);
        let r = analyze(&cap, 100).unwrap();
        assert_eq!(r.credentials_total, 1);
        let c = &r.credentials[0];
        assert_eq!(c.protocol, "FTP");
        assert_eq!(c.username, "bob");
        assert_eq!(c.password, "hunter2");
        assert_eq!(c.source, "192.168.0.10:50000");
    }

    #[test]
    fn limit_truncates_but_totals_survive() {
        let frames: Vec<Vec<u8>> = (0..5)
            .map(|i| frame(IPPROTO_UDP, A, B, 5000 + i as u16, 53, &dns_query_msg(&format!("h{i}.example"))))
            .collect();
        let cap = pcap(&frames);
        let r = analyze(&cap, 2).unwrap();
        assert_eq!(r.dns_queries_total, 5);
        assert_eq!(r.dns_queries.len(), 2);
    }

    #[test]
    fn duration_spans_first_to_last() {
        let cap = pcap(&[
            frame(IPPROTO_TCP, A, B, 1, 80, b"x"),
            frame(IPPROTO_TCP, A, B, 1, 80, b"y"),
        ]);
        let r = analyze(&cap, 100).unwrap();
        assert!((r.duration_seconds - 1.0).abs() < 1e-6);
    }
}

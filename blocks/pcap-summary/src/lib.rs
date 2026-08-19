//! gizza-ai/pcap-summary — aggregate a pcap/pcapng into capture statistics.
//!
//! Pipeline: resolve the uploaded capture (URL or attachment ref) → pure core
//! aggregator → JSON summary with capture overview, protocol breakdown + layer
//! hierarchy, top talkers (IP and MAC), conversations with per-direction counts,
//! and the busiest service ports.
//!
//! Chat + CLI only; like `parse-pcap` and `pcap-network-forensics`, this is a
//! binary-file→JSON report shape with no standalone page.
//!
//! Stated limits (also in the skill description so an LLM can relay them):
//!   * captures are capped at 32 MiB;
//!   * encrypted payloads are summarised by port, never decrypted;
//!   * only the first IP fragment carries transport headers, so later fragments
//!     count toward the IP/talker totals but not toward ports or conversations;
//!   * link layers decoded are Ethernet (incl. stacked VLAN tags), raw IP, Linux
//!     cooked and null/loopback — other link types still count in the overview;
//!   * timestamps come from the capture: one written without them reports a zero
//!     duration and zero rates rather than a fabricated one.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_pcap_summary_core::{Options, SortBy};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_BYTES: usize = 32 * 1024 * 1024;
const DEFAULT_TOP: u64 = 10;
const MAX_TOP: u64 = 1000;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    /// Maximum rows per ranked list.
    #[serde(default)]
    top: Option<u64>,
    /// Which section to return (`all`, `overview`, `protocols`, `talkers`,
    /// `conversations`, `ports`).
    #[serde(default = "default_section")]
    section: String,
    /// Ranking column (`bytes` or `packets`).
    #[serde(default = "default_sort_by")]
    sort_by: String,
    /// Name well-known ports (`443` → `https`).
    #[serde(default = "default_true")]
    resolve_ports: bool,
}

fn default_section() -> String {
    "all".to_string()
}
fn default_sort_by() -> String {
    "bytes".to_string()
}
fn default_true() -> bool {
    true
}

#[derive(Serialize)]
struct Resp {
    format: &'static str,
    link_type: String,
    total_packets: u64,
    section: String,
    sort_by: String,
    top: usize,
    resolve_ports: bool,
    protocols_total: usize,
    hierarchy_total: usize,
    talkers_total: usize,
    mac_talkers_total: usize,
    conversations_total: usize,
    ports_total: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    overview: Option<OverviewOut>,
    #[serde(skip_serializing_if = "Option::is_none")]
    protocols: Option<Vec<ProtocolOut>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hierarchy: Option<Vec<HierarchyOut>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    talkers: Option<Vec<TalkerOut>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mac_talkers: Option<Vec<TalkerOut>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    conversations: Option<Vec<ConversationOut>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ports: Option<Vec<PortOut>>,
}

#[derive(Serialize)]
struct OverviewOut {
    snaplen: u32,
    packets: u64,
    bytes: u64,
    captured_bytes: u64,
    truncated_packets: u64,
    decoded_packets: u64,
    first_timestamp: f64,
    last_timestamp: f64,
    duration_seconds: f64,
    average_packet_size_bytes: f64,
    packets_per_second: f64,
    bits_per_second: f64,
}

#[derive(Serialize)]
struct ProtocolOut {
    protocol: String,
    packets: u64,
    bytes: u64,
    packet_percent: f64,
    byte_percent: f64,
}

#[derive(Serialize)]
struct HierarchyOut {
    path: String,
    packets: u64,
    bytes: u64,
}

#[derive(Serialize)]
struct TalkerOut {
    address: String,
    packets: u64,
    bytes: u64,
    packets_sent: u64,
    bytes_sent: u64,
    packets_received: u64,
    bytes_received: u64,
}

#[derive(Serialize)]
struct ConversationOut {
    protocol: String,
    endpoint_a: String,
    endpoint_b: String,
    packets: u64,
    bytes: u64,
    packets_a_to_b: u64,
    bytes_a_to_b: u64,
    packets_b_to_a: u64,
    bytes_b_to_a: u64,
    start_seconds: f64,
    duration_seconds: f64,
}

#[derive(Serialize)]
struct PortOut {
    protocol: &'static str,
    port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    service: Option<String>,
    packets: u64,
    bytes: u64,
    endpoints: usize,
}

/// `Input::File` emits the `url`⊕`ref` `oneOf`; `section` picks one table,
/// `top`/`sort_by` shape every ranked list, `resolve_ports` names known ports.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
        .param(
            Param::enumv(
                "section",
                ["all", "overview", "protocols", "talkers", "conversations", "ports"],
            )
            .default("all")
            .describe(
                "Which part of the summary to return: all (default), overview (capture \
                 properties and rates), protocols (per-protocol breakdown plus the layer \
                 hierarchy), talkers (top IP endpoints plus MAC endpoints on Ethernet \
                 captures), conversations, or ports. Row totals for every section are \
                 always included.",
            ),
        )
        .param(
            Param::integer("top")
                .min(1.0)
                .max(MAX_TOP as f64)
                .default(DEFAULT_TOP as i64)
                .describe(
                    "How many rows to return per ranked list (1-1000, default 10). Each list \
                     is accompanied by its untruncated *_total count, so a cap is never \
                     mistaken for the whole picture.",
                ),
        )
        .param(
            Param::enumv("sort_by", ["bytes", "packets"])
                .default("bytes")
                .describe(
                    "Column the ranked lists are ordered by: bytes (default, ranks by traffic \
                     volume) or packets (ranks by how chatty an endpoint or port is). Both \
                     counts are reported on every row either way.",
                ),
        )
        .param(
            Param::boolean("resolve_ports")
                .default(true)
                .describe(
                    "Name well-known ports from a built-in table (443 -> https, 53 -> dns) in \
                     the ports list and in the protocol hierarchy paths. Default true; set \
                     false for raw port numbers only. No DNS or network lookups are performed.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct PcapSummary;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pcap-summary",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Summarize a pcap: protocol breakdown, top talkers, conversations, and busiest ports.",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Summarize an uploaded libpcap (.pcap) or pcapng (.pcapng) capture into capture statistics. Reports an overview (format, link type, snaplen, packet and byte totals, first/last timestamp, duration, average packet size, packets/s, bits/s, truncated packets), a protocol breakdown with packet and byte percentages plus colon-joined layer-hierarchy paths (eth:ipv4:tcp:https), top talkers with sent/received split (IP, plus MAC endpoints on Ethernet captures), conversations with per-direction counts and relative start/duration, and the busiest service ports. Offline single-file analysis: no live capture, no TLS decryption (encrypted flows are counted and named by port), no reverse DNS or GeoIP. Captures up to 32 MiB. Later IP fragments count toward IP totals but not toward the port or conversation tables. Provide the capture as either url (HTTP/HTTPS) or ref (id from a prior file upload/tool call). Use section to focus on one table, top and sort_by to shape the rankings.",
        parameters = schema_json()
    ),
)]
impl PcapSummary {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("pcap-summary")?;
    let (bytes, _mime, _filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;
    let resp = build(&bytes, args.top, &args.section, &args.sort_by, args.resolve_ports)?;
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize pcap-summary response: {e}")))
}

fn build(
    bytes: &[u8],
    top: Option<u64>,
    section: &str,
    sort_by: &str,
    resolve_ports: bool,
) -> Result<Resp, SkillError> {
    let section = match section.trim().to_ascii_lowercase().as_str() {
        "" | "all" => "all",
        "overview" => "overview",
        "protocols" => "protocols",
        "talkers" => "talkers",
        "conversations" => "conversations",
        "ports" => "ports",
        other => {
            return Err(SkillError::InvalidArgs(format!(
                "invalid section '{other}' (use all, overview, protocols, talkers, \
                 conversations, or ports)"
            )))
        }
    };
    let sort_by_name = match sort_by.trim().to_ascii_lowercase().as_str() {
        "" | "bytes" => "bytes",
        "packets" => "packets",
        other => {
            return Err(SkillError::InvalidArgs(format!(
                "invalid sort_by '{other}' (use bytes or packets)"
            )))
        }
    };
    let top = top.unwrap_or(DEFAULT_TOP).clamp(1, MAX_TOP) as usize;
    let opts = Options {
        top,
        sort_by: if sort_by_name == "packets" { SortBy::Packets } else { SortBy::Bytes },
        resolve_ports,
    };
    let s = gizza_ai_pcap_summary_core::analyze(bytes, &opts).map_err(SkillError::InvalidArgs)?;

    let include = |name: &str| section == "all" || section == name;
    Ok(Resp {
        format: s.overview.format,
        link_type: s.overview.link_type.clone(),
        total_packets: s.overview.packets,
        section: section.to_string(),
        sort_by: sort_by_name.to_string(),
        top,
        resolve_ports,
        protocols_total: s.protocols_total,
        hierarchy_total: s.hierarchy_total,
        talkers_total: s.talkers_total,
        mac_talkers_total: s.mac_talkers_total,
        conversations_total: s.conversations_total,
        ports_total: s.ports_total,
        overview: include("overview").then(|| OverviewOut {
            snaplen: s.overview.snaplen,
            packets: s.overview.packets,
            bytes: s.overview.bytes,
            captured_bytes: s.overview.captured_bytes,
            truncated_packets: s.overview.truncated_packets,
            decoded_packets: s.overview.decoded_packets,
            first_timestamp: s.overview.first_timestamp,
            last_timestamp: s.overview.last_timestamp,
            duration_seconds: s.overview.duration_seconds,
            average_packet_size_bytes: s.overview.average_packet_size_bytes,
            packets_per_second: s.overview.packets_per_second,
            bits_per_second: s.overview.bits_per_second,
        }),
        protocols: include("protocols").then(|| {
            s.protocols
                .into_iter()
                .map(|p| ProtocolOut {
                    protocol: p.protocol,
                    packets: p.packets,
                    bytes: p.bytes,
                    packet_percent: p.packet_percent,
                    byte_percent: p.byte_percent,
                })
                .collect()
        }),
        hierarchy: include("protocols").then(|| {
            s.hierarchy
                .into_iter()
                .map(|h| HierarchyOut { path: h.path, packets: h.packets, bytes: h.bytes })
                .collect()
        }),
        talkers: include("talkers").then(|| s.talkers.into_iter().map(talker_out).collect()),
        // MAC-level endpoints only exist for Ethernet captures.
        mac_talkers: (include("talkers") && s.ethernet)
            .then(|| s.mac_talkers.into_iter().map(talker_out).collect()),
        conversations: include("conversations").then(|| {
            s.conversations
                .into_iter()
                .map(|c| ConversationOut {
                    protocol: c.protocol,
                    endpoint_a: c.endpoint_a,
                    endpoint_b: c.endpoint_b,
                    packets: c.packets,
                    bytes: c.bytes,
                    packets_a_to_b: c.packets_a_to_b,
                    bytes_a_to_b: c.bytes_a_to_b,
                    packets_b_to_a: c.packets_b_to_a,
                    bytes_b_to_a: c.bytes_b_to_a,
                    start_seconds: c.start_seconds,
                    duration_seconds: c.duration_seconds,
                })
                .collect()
        }),
        ports: include("ports").then(|| {
            s.ports
                .into_iter()
                .map(|p| PortOut {
                    protocol: p.protocol,
                    port: p.port,
                    service: p.service,
                    packets: p.packets,
                    bytes: p.bytes,
                    endpoints: p.endpoints,
                })
                .collect()
        }),
    })
}

fn talker_out(t: gizza_ai_pcap_summary_core::Talker) -> TalkerOut {
    TalkerOut {
        address: t.address,
        packets: t.packets,
        bytes: t.bytes,
        packets_sent: t.packets_sent,
        bytes_sent: t.bytes_sent,
        packets_received: t.packets_received,
        bytes_received: t.bytes_received,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "File URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "section": {
                        "type": "string",
                        "enum": ["all", "overview", "protocols", "talkers", "conversations", "ports"],
                        "default": "all",
                        "description": "Which part of the summary to return: all (default), overview (capture properties and rates), protocols (per-protocol breakdown plus the layer hierarchy), talkers (top IP endpoints plus MAC endpoints on Ethernet captures), conversations, or ports. Row totals for every section are always included."
                    },
                    "top": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 1000,
                        "default": 10,
                        "description": "How many rows to return per ranked list (1-1000, default 10). Each list is accompanied by its untruncated *_total count, so a cap is never mistaken for the whole picture."
                    },
                    "sort_by": {
                        "type": "string",
                        "enum": ["bytes", "packets"],
                        "default": "bytes",
                        "description": "Column the ranked lists are ordered by: bytes (default, ranks by traffic volume) or packets (ranks by how chatty an endpoint or port is). Both counts are reported on every row either way."
                    },
                    "resolve_ports": {
                        "type": "boolean",
                        "default": true,
                        "description": "Name well-known ports from a built-in table (443 -> https, 53 -> dns) in the ports list and in the protocol hierarchy paths. Default true; set false for raw port numbers only. No DNS or network lookups are performed."
                    }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    /// Ethernet + IPv4 + TCP frame, one packet each way on an HTTPS flow.
    fn sample_pcap() -> Vec<u8> {
        fn frame(src: [u8; 4], dst: [u8; 4], sp: u16, dp: u16, payload: &[u8]) -> Vec<u8> {
            let mut pkt = Vec::new();
            pkt.extend_from_slice(&[0x11, 0x22, 0x33, 0x44, 0x55, 0x66]);
            pkt.extend_from_slice(&[0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);
            pkt.extend_from_slice(&0x0800u16.to_be_bytes());
            let mut tcp = Vec::new();
            tcp.extend_from_slice(&sp.to_be_bytes());
            tcp.extend_from_slice(&dp.to_be_bytes());
            tcp.extend_from_slice(&0u32.to_be_bytes());
            tcp.extend_from_slice(&0u32.to_be_bytes());
            tcp.push(0x50);
            tcp.push(0x18);
            tcp.extend_from_slice(&8192u16.to_be_bytes());
            tcp.extend_from_slice(&0u16.to_be_bytes());
            tcp.extend_from_slice(&0u16.to_be_bytes());
            tcp.extend_from_slice(payload);
            let total = 20 + tcp.len();
            let mut ip = vec![0x45, 0x00];
            ip.extend_from_slice(&(total as u16).to_be_bytes());
            ip.extend_from_slice(&[0, 0, 0, 0]);
            ip.push(64);
            ip.push(6);
            ip.extend_from_slice(&0u16.to_be_bytes());
            ip.extend_from_slice(&src);
            ip.extend_from_slice(&dst);
            pkt.extend_from_slice(&ip);
            pkt.extend_from_slice(&tcp);
            pkt
        }
        let frames = [
            frame([192, 168, 0, 10], [192, 168, 0, 20], 40000, 443, b"hello"),
            frame([192, 168, 0, 20], [192, 168, 0, 10], 443, 40000, b"hi"),
        ];
        let mut cap = Vec::new();
        cap.extend_from_slice(&[0xd4, 0xc3, 0xb2, 0xa1]);
        cap.extend_from_slice(&2u16.to_le_bytes());
        cap.extend_from_slice(&4u16.to_le_bytes());
        cap.extend_from_slice(&0u32.to_le_bytes());
        cap.extend_from_slice(&0u32.to_le_bytes());
        cap.extend_from_slice(&65535u32.to_le_bytes());
        cap.extend_from_slice(&1u32.to_le_bytes());
        for (i, f) in frames.iter().enumerate() {
            cap.extend_from_slice(&(1_700_000_000u32 + i as u32).to_le_bytes());
            cap.extend_from_slice(&0u32.to_le_bytes());
            cap.extend_from_slice(&(f.len() as u32).to_le_bytes());
            cap.extend_from_slice(&(f.len() as u32).to_le_bytes());
            cap.extend_from_slice(f);
        }
        cap
    }

    #[test]
    fn build_all_sections() {
        let r = build(&sample_pcap(), None, "all", "bytes", true).unwrap();
        assert_eq!(r.format, "pcap");
        assert_eq!(r.link_type, "Ethernet");
        assert_eq!(r.total_packets, 2);
        assert_eq!(r.top, 10);
        assert_eq!(r.sort_by, "bytes");
        assert!(r.resolve_ports);
        assert_eq!(r.talkers_total, 2);
        assert_eq!(r.conversations_total, 1);
        assert_eq!(r.ports_total, 1);
        assert!(r.overview.is_some());
        assert!(r.protocols.is_some());
        assert!(r.hierarchy.is_some());
        assert!(r.talkers.is_some());
        assert!(r.mac_talkers.is_some());
        assert!(r.conversations.is_some());
        let ports = r.ports.as_ref().unwrap();
        assert_eq!(ports[0].port, 443);
        assert_eq!(ports[0].service.as_deref(), Some("https"));
    }

    #[test]
    fn section_filters_lists_but_keeps_totals() {
        let r = build(&sample_pcap(), None, "ports", "bytes", true).unwrap();
        assert_eq!(r.talkers_total, 2);
        assert_eq!(r.conversations_total, 1);
        assert!(r.overview.is_none());
        assert!(r.protocols.is_none());
        assert!(r.talkers.is_none());
        assert!(r.mac_talkers.is_none());
        assert!(r.conversations.is_none());
        assert!(r.ports.is_some());
    }

    #[test]
    fn resolve_ports_false_omits_service_names() {
        let r = build(&sample_pcap(), None, "ports", "bytes", false).unwrap();
        assert!(!r.resolve_ports);
        assert_eq!(r.ports.as_ref().unwrap()[0].service, None);
    }

    #[test]
    fn top_is_clamped_into_range() {
        let r = build(&sample_pcap(), Some(0), "all", "bytes", true).unwrap();
        assert_eq!(r.top, 1);
        assert_eq!(r.talkers.as_ref().unwrap().len(), 1);
        assert_eq!(r.talkers_total, 2);
        let r = build(&sample_pcap(), Some(99_999), "all", "bytes", true).unwrap();
        assert_eq!(r.top, MAX_TOP as usize);
    }

    #[test]
    fn invalid_section_errors() {
        let Err(err) = build(&sample_pcap(), None, "credentials", "bytes", true) else {
            panic!("an unknown section must be rejected");
        };
        assert!(format!("{err:?}").contains("invalid section"), "{err:?}");
    }

    #[test]
    fn invalid_sort_by_errors() {
        let Err(err) = build(&sample_pcap(), None, "all", "duration", true) else {
            panic!("an unknown sort_by must be rejected");
        };
        assert!(format!("{err:?}").contains("invalid sort_by"), "{err:?}");
    }

    #[test]
    fn non_capture_input_errors() {
        assert!(build(b"definitely not a capture", None, "all", "bytes", true).is_err());
    }
}

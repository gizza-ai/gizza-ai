//! gizza-ai/parse-pcap — parse a libpcap (`.pcap`) or pcapng (`.pcapng`)
//! capture into a per-packet summary with decoded Ethernet / IPv4 / IPv6 / TCP /
//! UDP / ICMP / ARP layers and a capture timestamp.
//!
//! Pipeline: resolve the source file (URL fetch or attachment ref, any bytes) →
//! `core::parse` (pure, dependency-free binary parsing) → flat JSON the LLM reads
//! directly (format, link type, total packet count, and a list of decoded
//! packets capped by `limit`).
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (a file→JSON report fits neither the pure-text
//! page nor the ffmpeg file→media page shape — the "no-page file-input" pattern,
//! like detect-file-type / web-fetch / pdf-extract-text).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_BYTES: usize = 32 * 1024 * 1024;
const DEFAULT_LIMIT: u64 = 200;
const MAX_LIMIT: u64 = 5000;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    /// Maximum number of packets to decode and return.
    #[serde(default)]
    limit: Option<u64>,
}

#[derive(Serialize)]
struct PacketOut {
    index: usize,
    /// Capture timestamp, seconds since the Unix epoch (fractional).
    timestamp: f64,
    /// Original on-wire length in bytes.
    length: u32,
    /// Captured bytes in the file (<= length when the capture was snapped).
    captured: u32,
    source: String,
    destination: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    destination_port: Option<u16>,
    protocol: String,
    info: String,
}

#[derive(Serialize)]
struct Resp {
    /// Container format: `pcap` or `pcapng`.
    format: &'static str,
    /// Link-layer type of the first interface (e.g. `Ethernet`).
    link_type: String,
    /// Total packets present in the file.
    total_packets: usize,
    /// Number of packets decoded into `packets` (<= total when capped).
    returned_packets: usize,
    /// True when `total_packets` exceeded the limit and the list was truncated.
    truncated: bool,
    packets: Vec<PacketOut>,
}

/// `Input::File` emits the `url`⊕`ref` `oneOf`; a `limit` caps how many packets
/// are decoded (the file can be huge, the LLM only needs a sample).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File).param(
        Param::integer("limit")
            .min(1.0)
            .max(MAX_LIMIT as f64)
            .default(DEFAULT_LIMIT as i64)
            .describe(
                "Maximum number of packets to decode and return (1-5000, default 200). The total \
                 packet count is always reported even when the list is truncated.",
            ),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ParsePcap;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/parse-pcap",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse a pcap/pcapng capture into a per-packet summary",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Parse an uploaded libpcap (.pcap) or pcapng (.pcapng) network capture into a per-packet summary. Decodes the link layer (Ethernet, with VLAN tags, plus raw-IP / Linux-cooked / loopback) and the network/transport layers: IPv4, IPv6, TCP (with flag names), UDP, ICMP/ICMPv6, and ARP. Each packet reports its capture timestamp, on-wire and captured length, source/destination addresses, source/destination ports (TCP/UDP), the highest decoded protocol, and a one-line description. Returns the container format, link type, total packet count, and the decoded packets (capped by limit). Provide the capture as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl ParsePcap {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("parse-pcap")?;
    let limit = args.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT) as usize;
    let (bytes, _mime, _filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;
    let resp = build(&bytes, limit)?;
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize parse-pcap response: {e}")))
}

/// Pure assembly of the response from raw bytes (shared by run + tests).
fn build(bytes: &[u8], limit: usize) -> Result<Resp, SkillError> {
    let cap = gizza_ai_parse_pcap_core::parse(bytes, limit).map_err(SkillError::InvalidArgs)?;
    let packets = cap
        .packets
        .iter()
        .map(|p| PacketOut {
            index: p.index,
            timestamp: p.timestamp,
            length: p.orig_len,
            captured: p.cap_len,
            source: p.src.clone(),
            destination: p.dst.clone(),
            source_port: p.src_port,
            destination_port: p.dst_port,
            protocol: p.protocol.clone(),
            info: p.info.clone(),
        })
        .collect::<Vec<_>>();
    Ok(Resp {
        format: cap.format,
        link_type: cap.link_type,
        total_packets: cap.total_packets,
        returned_packets: packets.len(),
        truncated: cap.truncated,
        packets,
    })
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
                    "limit": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 5000,
                        "default": 200,
                        "description": "Maximum number of packets to decode and return (1-5000, default 200). The total packet count is always reported even when the list is truncated."
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

    /// Build a tiny classic little-endian pcap with one Ethernet/IPv4/TCP packet
    /// (mirrors the core test fixture) to exercise `build`.
    fn one_tcp_pcap() -> Vec<u8> {
        let mut f = Vec::new();
        f.extend_from_slice(&[0xd4, 0xc3, 0xb2, 0xa1]);
        f.extend_from_slice(&2u16.to_le_bytes());
        f.extend_from_slice(&4u16.to_le_bytes());
        f.extend_from_slice(&0u32.to_le_bytes());
        f.extend_from_slice(&0u32.to_le_bytes());
        f.extend_from_slice(&65535u32.to_le_bytes());
        f.extend_from_slice(&1u32.to_le_bytes());
        let mut pkt = Vec::new();
        pkt.extend_from_slice(&[0x11, 0x22, 0x33, 0x44, 0x55, 0x66]);
        pkt.extend_from_slice(&[0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);
        pkt.extend_from_slice(&0x0800u16.to_be_bytes());
        let mut ip = vec![0x45, 0x00];
        ip.extend_from_slice(&40u16.to_be_bytes());
        ip.extend_from_slice(&[0, 0, 0, 0]);
        ip.push(64);
        ip.push(6);
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
        tcp.push(0x02);
        tcp.extend_from_slice(&8192u16.to_be_bytes());
        tcp.extend_from_slice(&0u16.to_be_bytes());
        tcp.extend_from_slice(&0u16.to_be_bytes());
        pkt.extend_from_slice(&tcp);
        f.extend_from_slice(&1_700_000_000u32.to_le_bytes());
        f.extend_from_slice(&500_000u32.to_le_bytes());
        f.extend_from_slice(&(pkt.len() as u32).to_le_bytes());
        f.extend_from_slice(&(pkt.len() as u32).to_le_bytes());
        f.extend_from_slice(&pkt);
        f
    }

    #[test]
    fn build_summary() {
        let resp = build(&one_tcp_pcap(), 200).unwrap();
        assert_eq!(resp.format, "pcap");
        assert_eq!(resp.total_packets, 1);
        assert_eq!(resp.returned_packets, 1);
        assert!(!resp.truncated);
        let p = &resp.packets[0];
        assert_eq!(p.protocol, "TCP");
        assert_eq!(p.source, "192.168.0.1");
        assert_eq!(p.destination_port, Some(80));
    }
}

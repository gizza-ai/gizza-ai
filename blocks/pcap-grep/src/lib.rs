//! gizza-ai/pcap-grep — ngrep-style payload search over an uploaded libpcap
//! (`.pcap`) or pcapng (`.pcapng`) capture. Matches a regular (or hexadecimal)
//! expression against each packet's application payload and returns the matching
//! packets with their metadata (timestamp, protocol, addresses, ports, TCP flags)
//! and an ASCII / hex rendering of the payload.
//!
//! Pipeline: resolve the source file (URL fetch or attachment ref, any bytes) →
//! `core::grep` (pure, `regex`-only) → flat JSON the LLM reads directly.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (a file→text-report tool fits neither the
//! pure-text page nor the ffmpeg file→media page shape — the "no-page file-input"
//! pattern, like parse-pcap / pcap-network-forensics / detect-file-type).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_pcap_grep_core::GrepOptions;
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_BYTES: usize = 32 * 1024 * 1024;
const DEFAULT_LIMIT: u64 = 100;
const MAX_LIMIT: u64 = 1000;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    /// The regular expression (or hex string) to match against packet payloads.
    pattern: String,
    #[serde(default)]
    ignore_case: bool,
    #[serde(default)]
    hex: bool,
    #[serde(default)]
    invert: bool,
    #[serde(default)]
    port: Option<u16>,
    #[serde(default)]
    show_hex: bool,
    #[serde(default)]
    limit: Option<u64>,
}

#[derive(Serialize, Debug)]
struct MatchOut {
    index: usize,
    /// Capture timestamp, seconds since the Unix epoch (fractional).
    timestamp: f64,
    protocol: String,
    source: String,
    destination: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    destination_port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    flags: Option<String>,
    payload_len: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    match_offset: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    match_length: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    matched_text: Option<String>,
    payload_ascii: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    payload_hex: Option<String>,
}

#[derive(Serialize, Debug)]
struct Resp {
    /// Container format: `pcap` or `pcapng`.
    format: &'static str,
    /// Link-layer type of the first interface (e.g. `Ethernet`).
    link_type: String,
    /// The pattern that was searched (echoed back).
    pattern: String,
    /// Total packets present in the file.
    total_packets: usize,
    /// Packets that had a non-empty payload and passed the port filter (searched).
    scanned_packets: usize,
    /// Total packets whose payload matched (before the limit cap).
    matched_packets: usize,
    /// Number of matches returned in `matches` (<= matched_packets when capped).
    returned_matches: usize,
    /// True when `matched_packets` exceeded the limit and the list was truncated.
    truncated: bool,
    matches: Vec<MatchOut>,
}

/// `Input::File` emits the `url`⊕`ref` `oneOf`; `pattern` is required, the rest
/// tune the search (all default off / unset).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
        .param(
            Param::string("pattern")
                .required()
                .describe(
                    "The expression to search packet payloads for. By default an extended regular \
                     expression (e.g. 'GET /\\S+', 'pass(word)?=', '(?:User-Agent|Host):'). With \
                     hex=true it is instead a hexadecimal byte string like '47455420' (spaces and \
                     colons are ignored). Matched against the application payload (bytes after the \
                     TCP/UDP header; after the IP header for other protocols).",
                ),
        )
        .param(
            Param::boolean("ignore_case")
                .default(false)
                .describe(
                    "Case-insensitive matching for the regular expression (like ngrep -i). Has no \
                     effect in hex mode. Default false.",
                ),
        )
        .param(
            Param::boolean("hex")
                .default(false)
                .describe(
                    "Treat 'pattern' as a hexadecimal byte string instead of a regex (like ngrep \
                     -X), e.g. '47 45 54 20' to find the bytes 'GET '. Default false.",
                ),
        )
        .param(
            Param::boolean("invert")
                .default(false)
                .describe(
                    "Invert the match: return packets whose payload does NOT match (like ngrep \
                     -v). Default false.",
                ),
        )
        .param(
            Param::integer("port")
                .min(0.0)
                .max(65535.0)
                .describe(
                    "Optional TCP/UDP port narrowing: only search packets whose source OR \
                     destination port equals this (0-65535). Omit to search every packet.",
                ),
        )
        .param(
            Param::boolean("show_hex")
                .default(false)
                .describe(
                    "Include a canonical hex + ASCII dump of each matching packet's payload (like \
                     ngrep -x), capped to the first 512 bytes. Default false.",
                ),
        )
        .param(
            Param::integer("limit")
                .min(1.0)
                .max(MAX_LIMIT as f64)
                .default(DEFAULT_LIMIT as i64)
                .describe(
                    "Maximum number of matching packets to return (1-1000, default 100). The total \
                     match count is always reported even when the list is truncated.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct PcapGrep;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pcap-grep",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Search .pcap/.pcapng packet payloads with a regex, ngrep-style",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Search the payloads of an uploaded libpcap (.pcap) or pcapng (.pcapng) network capture with a regular expression (or a hexadecimal byte string), ngrep-style, and return the matching packets. Each match reports the packet index, capture timestamp, protocol, source/destination addresses and ports, TCP flags, the payload length, the byte offset and text of the match, an ASCII rendering of the payload (non-printable bytes shown as '.'), and optionally a hex dump. Options mirror ngrep: ignore_case (-i), hex (-X, treat the pattern as hex), invert (-v, show non-matching packets), an optional port narrowing, show_hex (-x), and a limit on returned matches. Searches the application payload (bytes after the TCP/UDP header). Provide the capture as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl PcapGrep {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("pcap-grep")?;
    let (bytes, _mime, _filename) =
        resolve_source(args.source.clone().into_inner(), AssetKind::Any, MAX_BYTES)?;
    let resp = build(&bytes, &args)?;
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize pcap-grep response: {e}")))
}

/// Pure assembly of the response from raw bytes + args (shared by run + tests).
fn build(bytes: &[u8], args: &Args) -> Result<Resp, SkillError> {
    let opts = GrepOptions {
        ignore_case: args.ignore_case,
        hex: args.hex,
        invert: args.invert,
        port: args.port,
        show_hex: args.show_hex,
        limit: args.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT) as usize,
    };
    let res =
        gizza_ai_pcap_grep_core::grep(bytes, &args.pattern, &opts).map_err(SkillError::InvalidArgs)?;
    let matches = res
        .matches
        .into_iter()
        .map(|m| MatchOut {
            index: m.index,
            timestamp: m.timestamp,
            protocol: m.protocol,
            source: m.source,
            destination: m.destination,
            source_port: m.source_port,
            destination_port: m.destination_port,
            flags: m.flags,
            payload_len: m.payload_len,
            match_offset: m.match_offset,
            match_length: m.match_length,
            matched_text: m.matched_text,
            payload_ascii: m.payload_ascii,
            payload_hex: m.payload_hex,
        })
        .collect::<Vec<_>>();
    Ok(Resp {
        format: res.format,
        link_type: res.link_type,
        pattern: args.pattern.clone(),
        total_packets: res.total_packets,
        scanned_packets: res.scanned_packets,
        matched_packets: res.matched_packets,
        returned_matches: matches.len(),
        truncated: res.truncated,
        matches,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_args(pattern: &str) -> Args {
        Args {
            source: serde_json::from_str(r#"{"url":"http://x"}"#).unwrap(),
            pattern: pattern.to_string(),
            ignore_case: false,
            hex: false,
            invert: false,
            port: None,
            show_hex: false,
            limit: None,
        }
    }

    /// Classic little-endian pcap with one Ethernet/IPv4/TCP HTTP-GET packet.
    fn http_get_pcap() -> Vec<u8> {
        let body = b"GET /index.html HTTP/1.1\r\nHost: example.com\r\n\r\n";
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
        tcp.extend_from_slice(&4321u16.to_be_bytes());
        tcp.extend_from_slice(&80u16.to_be_bytes());
        tcp.extend_from_slice(&0u32.to_be_bytes());
        tcp.extend_from_slice(&0u32.to_be_bytes());
        tcp.push(0x50);
        tcp.push(0x18);
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
    fn build_matches() {
        let resp = build(&http_get_pcap(), &default_args("GET /\\S+")).unwrap();
        assert_eq!(resp.format, "pcap");
        assert_eq!(resp.total_packets, 1);
        assert_eq!(resp.matched_packets, 1);
        assert_eq!(resp.returned_matches, 1);
        assert!(!resp.truncated);
        let m = &resp.matches[0];
        assert_eq!(m.protocol, "TCP");
        assert_eq!(m.destination_port, Some(80));
        assert_eq!(m.matched_text.as_deref(), Some("GET /index.html"));
    }

    #[test]
    fn build_no_match() {
        let resp = build(&http_get_pcap(), &default_args("NOPE-NOT-HERE")).unwrap();
        assert_eq!(resp.matched_packets, 0);
        assert_eq!(resp.returned_matches, 0);
        assert!(resp.matches.is_empty());
    }

    #[test]
    fn build_bad_regex_errors() {
        let err = build(&http_get_pcap(), &default_args("(oops")).unwrap_err();
        match err {
            SkillError::InvalidArgs(m) => assert!(m.contains("invalid regular expression")),
            other => panic!("expected InvalidArgs, got {other:?}"),
        }
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "File URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "pattern": {
                        "type": "string",
                        "description": "The expression to search packet payloads for. By default an extended regular expression (e.g. 'GET /\\S+', 'pass(word)?=', '(?:User-Agent|Host):'). With hex=true it is instead a hexadecimal byte string like '47455420' (spaces and colons are ignored). Matched against the application payload (bytes after the TCP/UDP header; after the IP header for other protocols)."
                    },
                    "ignore_case": {
                        "type": "boolean",
                        "default": false,
                        "description": "Case-insensitive matching for the regular expression (like ngrep -i). Has no effect in hex mode. Default false."
                    },
                    "hex": {
                        "type": "boolean",
                        "default": false,
                        "description": "Treat 'pattern' as a hexadecimal byte string instead of a regex (like ngrep -X), e.g. '47 45 54 20' to find the bytes 'GET '. Default false."
                    },
                    "invert": {
                        "type": "boolean",
                        "default": false,
                        "description": "Invert the match: return packets whose payload does NOT match (like ngrep -v). Default false."
                    },
                    "port": {
                        "type": "integer",
                        "minimum": 0,
                        "maximum": 65535,
                        "description": "Optional TCP/UDP port narrowing: only search packets whose source OR destination port equals this (0-65535). Omit to search every packet."
                    },
                    "show_hex": {
                        "type": "boolean",
                        "default": false,
                        "description": "Include a canonical hex + ASCII dump of each matching packet's payload (like ngrep -x), capped to the first 512 bytes. Default false."
                    },
                    "limit": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 1000,
                        "default": 100,
                        "description": "Maximum number of matching packets to return (1-1000, default 100). The total match count is always reported even when the list is truncated."
                    }
                },
                "required": ["pattern"],
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}

//! gizza-ai/pcap-network-forensics — aggregate a pcap/pcapng into a forensic report.
//!
//! Pipeline: resolve the uploaded capture (URL or attachment ref) → pure core parser
//! → JSON report with host inventory, conversations, DNS queries, HTTP requests,
//! and cleartext credential findings. Chat + CLI only; like parse-pcap, this is a
//! file→JSON report shape with no standalone page.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_BYTES: usize = 64 * 1024 * 1024;
const DEFAULT_LIMIT: u64 = 100;
const MAX_LIMIT: u64 = 5000;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    /// Maximum number of entries returned per report section.
    #[serde(default)]
    limit: Option<u64>,
    /// Which section to return (`all`, `hosts`, `conversations`, `dns`, `http`, `credentials`).
    #[serde(default = "default_section")]
    section: String,
}

fn default_section() -> String {
    "all".to_string()
}

#[derive(Serialize)]
struct Resp {
    format: &'static str,
    link_type: String,
    total_packets: usize,
    duration_seconds: f64,
    limit: usize,
    section: String,
    hosts_total: usize,
    conversations_total: usize,
    dns_queries_total: usize,
    http_requests_total: usize,
    credentials_total: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    hosts: Option<Vec<HostOut>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    conversations: Option<Vec<ConversationOut>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dns_queries: Option<Vec<DnsQueryOut>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    http_requests: Option<Vec<HttpRequestOut>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    credentials: Option<Vec<CredentialOut>>,
}

#[derive(Serialize)]
struct HostOut {
    address: String,
    packets: u64,
    bytes: u64,
}

#[derive(Serialize)]
struct ConversationOut {
    protocol: &'static str,
    endpoint_a: String,
    endpoint_b: String,
    packets: u64,
    bytes: u64,
}

#[derive(Serialize)]
struct DnsQueryOut {
    name: String,
    qtype: String,
    answers: Vec<String>,
}

#[derive(Serialize)]
struct HttpRequestOut {
    method: String,
    host: String,
    path: String,
    user_agent: String,
}

#[derive(Serialize)]
struct CredentialOut {
    kind: String,
    protocol: String,
    source: String,
    destination: String,
    username: String,
    password: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
        .param(
            Param::integer("limit")
                .min(1.0)
                .max(MAX_LIMIT as f64)
                .default(DEFAULT_LIMIT as i64)
                .describe(
                    "Maximum number of entries returned per section (1-5000, default 100). Totals \
                     are always reported even when lists are capped.",
                ),
        )
        .param(
            Param::enumv(
                "section",
                ["all", "hosts", "conversations", "dns", "http", "credentials"],
            )
            .default("all")
            .describe(
                "Which report section to include: all, hosts, conversations, dns, http, or \
                 credentials. Totals for every section are always included.",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct PcapNetworkForensics;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pcap-network-forensics",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Summarize hosts, conversations, DNS, HTTP, and cleartext credentials from a pcap.",
    requires = ["wafer-run/network"],
    skill(
        description = "Parse an uploaded libpcap (.pcap) or pcapng (.pcapng) capture into a forensic summary. Reports host inventory, bidirectional TCP/UDP conversations, DNS questions with A/AAAA answers, cleartext HTTP request lines, and cleartext credential findings from HTTP Basic auth, HTTP form logins, FTP USER/PASS, and POP3 USER/PASS. This is offline single-file analysis: no live capture, no GeoIP enrichment, no TLS decryption, and no file carving. Provide the capture as either url (HTTP/HTTPS) or ref (id from a prior file upload/tool call). Use limit to cap each list and section to focus on one tab-like view.",
        parameters = schema_json()
    ),
)]
impl PcapNetworkForensics {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("pcap-network-forensics")?;
    let limit = args.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT) as usize;
    let (bytes, _mime, _filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;
    let resp = build(&bytes, limit, &args.section)?;
    serde_json::to_vec(&resp).map_err(|e| {
        SkillError::Serialize(format!("serialize pcap-network-forensics response: {e}"))
    })
}

fn build(bytes: &[u8], limit: usize, section: &str) -> Result<Resp, SkillError> {
    let normalized = section.trim().to_ascii_lowercase();
    let section = match normalized.as_str() {
        "" => "all",
        "all" | "hosts" | "conversations" | "dns" | "http" | "credentials" => normalized.as_str(),
        other => {
            return Err(SkillError::InvalidArgs(format!(
                "invalid section '{other}' (use all, hosts, conversations, dns, http, or credentials)"
            )))
        }
    };
    let report = gizza_ai_pcap_network_forensics_core::analyze(bytes, limit)
        .map_err(SkillError::InvalidArgs)?;

    let include = |name: &str| section == "all" || section == name;
    Ok(Resp {
        format: report.format,
        link_type: report.link_type,
        total_packets: report.total_packets,
        duration_seconds: report.duration_seconds,
        limit,
        section: section.to_string(),
        hosts_total: report.hosts_total,
        conversations_total: report.conversations_total,
        dns_queries_total: report.dns_queries_total,
        http_requests_total: report.http_requests_total,
        credentials_total: report.credentials_total,
        hosts: include("hosts").then(|| {
            report
                .hosts
                .into_iter()
                .map(|h| HostOut { address: h.address, packets: h.packets, bytes: h.bytes })
                .collect()
        }),
        conversations: include("conversations").then(|| {
            report
                .conversations
                .into_iter()
                .map(|c| ConversationOut {
                    protocol: c.protocol,
                    endpoint_a: c.endpoint_a,
                    endpoint_b: c.endpoint_b,
                    packets: c.packets,
                    bytes: c.bytes,
                })
                .collect()
        }),
        dns_queries: include("dns").then(|| {
            report
                .dns_queries
                .into_iter()
                .map(|d| DnsQueryOut { name: d.name, qtype: d.qtype, answers: d.answers })
                .collect()
        }),
        http_requests: include("http").then(|| {
            report
                .http_requests
                .into_iter()
                .map(|h| HttpRequestOut {
                    method: h.method,
                    host: h.host,
                    path: h.path,
                    user_agent: h.user_agent,
                })
                .collect()
        }),
        credentials: include("credentials").then(|| {
            report
                .credentials
                .into_iter()
                .map(|c| CredentialOut {
                    kind: c.kind,
                    protocol: c.protocol,
                    source: c.source,
                    destination: c.destination,
                    username: c.username,
                    password: c.password,
                })
                .collect()
        }),
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
                        "default": 100,
                        "description": "Maximum number of entries returned per section (1-5000, default 100). Totals are always reported even when lists are capped."
                    },
                    "section": {
                        "type": "string",
                        "enum": ["all", "hosts", "conversations", "dns", "http", "credentials"],
                        "default": "all",
                        "description": "Which report section to include: all, hosts, conversations, dns, http, or credentials. Totals for every section are always included."
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

    fn pcap_with_http() -> Vec<u8> {
        let payload = b"GET /admin HTTP/1.1\r\nHost: victim.example\r\nUser-Agent: curl/8.0\r\n\r\n";
        let mut frame = Vec::new();
        frame.extend_from_slice(&[0x11, 0x22, 0x33, 0x44, 0x55, 0x66]);
        frame.extend_from_slice(&[0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);
        frame.extend_from_slice(&0x0800u16.to_be_bytes());
        let mut tcp = Vec::new();
        tcp.extend_from_slice(&40000u16.to_be_bytes());
        tcp.extend_from_slice(&80u16.to_be_bytes());
        tcp.extend_from_slice(&0u32.to_be_bytes());
        tcp.extend_from_slice(&0u32.to_be_bytes());
        tcp.push(0x50);
        tcp.push(0x18);
        tcp.extend_from_slice(&8192u16.to_be_bytes());
        tcp.extend_from_slice(&0u16.to_be_bytes());
        tcp.extend_from_slice(&0u16.to_be_bytes());
        tcp.extend_from_slice(payload);
        let total_len = 20 + tcp.len();
        let mut ip = vec![0x45, 0x00];
        ip.extend_from_slice(&(total_len as u16).to_be_bytes());
        ip.extend_from_slice(&[0, 0, 0, 0]);
        ip.push(64);
        ip.push(6);
        ip.extend_from_slice(&0u16.to_be_bytes());
        ip.extend_from_slice(&[192, 168, 0, 10]);
        ip.extend_from_slice(&[192, 168, 0, 20]);
        frame.extend_from_slice(&ip);
        frame.extend_from_slice(&tcp);

        let mut pcap = Vec::new();
        pcap.extend_from_slice(&[0xd4, 0xc3, 0xb2, 0xa1]);
        pcap.extend_from_slice(&2u16.to_le_bytes());
        pcap.extend_from_slice(&4u16.to_le_bytes());
        pcap.extend_from_slice(&0u32.to_le_bytes());
        pcap.extend_from_slice(&0u32.to_le_bytes());
        pcap.extend_from_slice(&65535u32.to_le_bytes());
        pcap.extend_from_slice(&1u32.to_le_bytes());
        pcap.extend_from_slice(&1_700_000_000u32.to_le_bytes());
        pcap.extend_from_slice(&0u32.to_le_bytes());
        pcap.extend_from_slice(&(frame.len() as u32).to_le_bytes());
        pcap.extend_from_slice(&(frame.len() as u32).to_le_bytes());
        pcap.extend_from_slice(&frame);
        pcap
    }

    #[test]
    fn build_all_sections() {
        let resp = build(&pcap_with_http(), 100, "all").unwrap();
        assert_eq!(resp.format, "pcap");
        assert_eq!(resp.total_packets, 1);
        assert_eq!(resp.hosts_total, 2);
        assert_eq!(resp.conversations_total, 1);
        assert_eq!(resp.http_requests_total, 1);
        assert!(resp.hosts.is_some());
        assert!(resp.conversations.is_some());
        assert!(resp.http_requests.is_some());
    }

    #[test]
    fn section_filters_lists_but_keeps_totals() {
        let resp = build(&pcap_with_http(), 100, "http").unwrap();
        assert_eq!(resp.hosts_total, 2);
        assert_eq!(resp.http_requests_total, 1);
        assert!(resp.hosts.is_none());
        assert!(resp.conversations.is_none());
        assert!(resp.http_requests.is_some());
    }

    #[test]
    fn invalid_section_errors() {
        assert!(build(&pcap_with_http(), 100, "files").is_err());
    }
}

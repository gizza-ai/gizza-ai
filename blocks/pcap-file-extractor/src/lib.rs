//! gizza-ai/pcap-file-extractor — recover the files carried inside a capture.
//!
//! Pipeline: resolve the uploaded capture (URL or attachment ref) → pure core
//! (pcap/pcapng parse → TCP reassembly → HTTP / FTP / SMB2 object extraction) →
//! flat JSON with each recovered file's metadata, hashes, and bytes inline as
//! base64 within a budget.
//!
//! Pure Rust → runs on ALL backends including the chat sandbox. Surfaces: chat +
//! CLI. No standalone page (binary file → JSON, the no-page file-input pattern
//! shared with parse-pcap / pcap-grep / pcap-network-forensics / carve-files).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_pcap_file_extractor_core::{
    extract, ExtractResult, Options, DEFAULT_CONTENT_BUDGET, DEFAULT_LIMIT, MAX_CAPTURE_BYTES,
    MAX_CONTENT_BUDGET, MAX_LIMIT,
};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    /// `all` or a comma-separated subset of `http`, `ftp`, `smb`.
    #[serde(default)]
    protocols: Option<String>,
    /// Case-insensitive substring over filename / path / host / content type.
    #[serde(default)]
    filter: Option<String>,
    /// Drop objects smaller than this many bytes.
    #[serde(default)]
    min_size: Option<u64>,
    #[serde(default)]
    include_incomplete: Option<bool>,
    #[serde(default)]
    include_content: Option<bool>,
    #[serde(default)]
    max_content_bytes: Option<u64>,
    #[serde(default)]
    limit: Option<u64>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File)
        .param(
            Param::string("protocols")
                .default("all")
                .describe(
                    "Which protocols to carve: 'all' (default) or a comma-separated subset of \
                     http, ftp, smb — for example 'http,smb'. HTTP covers response bodies and \
                     POST/PUT/PATCH uploads; FTP uses the control channel (RETR/STOR/LIST plus \
                     PASV/EPSV/PORT/EPRT) to name each data connection; SMB covers SMB2/3 \
                     CREATE+READ/WRITE (SMB1/CIFS is detected and reported, not carved).",
                ),
        )
        .param(
            Param::string("filter")
                .default("")
                .describe(
                    "Case-insensitive substring that an object must match on its filename, \
                     request path, host, or declared content type — the equivalent of a packet \
                     analyser's object-list text filter. Example: 'exe' or 'invoice'. Empty \
                     (default) keeps everything.",
                ),
        )
        .param(
            Param::integer("min_size")
                .default(0)
                .min(0.0)
                .max(1_073_741_824.0)
                .describe(
                    "Drop recovered objects smaller than this many bytes (default 0 = keep all). \
                     Useful for skipping 1-pixel trackers and tiny redirect bodies.",
                ),
        )
        .param(
            Param::boolean("include_incomplete")
                .default(true)
                .describe(
                    "Include objects whose bytes were only partly captured (default true). Each \
                     object reports complete plus completeness_percent, so a partial carve is \
                     never mistaken for a whole file. Set false to keep only 100%-complete files.",
                ),
        )
        .param(
            Param::boolean("include_content")
                .default(true)
                .describe(
                    "Return each object's bytes inline as base64 (default true). Set false for a \
                     fast inventory — filenames, sizes, types, MD5 and SHA-256 are still \
                     reported without the payload.",
                ),
        )
        .param(
            Param::integer("max_content_bytes")
                .default(DEFAULT_CONTENT_BUDGET as i64)
                .min(0.0)
                .max(MAX_CONTENT_BUDGET as f64)
                .describe(
                    "Total budget in bytes for inline base64 content across all returned objects \
                     (default 4194304 = 4 MiB, maximum 16777216 = 16 MiB). Objects are inlined in \
                     packet order until the budget runs out; the rest are still listed with their \
                     hashes, and a note says so.",
                ),
        )
        .param(
            Param::integer("limit")
                .default(DEFAULT_LIMIT as i64)
                .min(1.0)
                .max(MAX_LIMIT as f64)
                .describe(
                    "Maximum number of recovered objects to return (1-5000, default 100). \
                     files_total always reports how many matched before this cap.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct PcapFileExtractor;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pcap-file-extractor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Carve files transferred over HTTP, FTP, and SMB out of a pcap capture.",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Reassemble the TCP streams inside an uploaded libpcap (.pcap) or pcapng (.pcapng) capture and carve out the files that were transferred over HTTP, FTP, and SMB. HTTP objects cover response bodies (downloads) and POST/PUT/PATCH bodies (uploads), de-chunked and gzip/deflate-inflated, named from Content-Disposition or the request URI. FTP objects take their real filename and direction from the control channel (RETR/STOR/LIST with PASV/EPSV/PORT/EPRT). SMB objects are SMB2/3 CREATE+READ/WRITE, assembled at their true file offsets. Every object reports protocol, filename, path, host, direction, declared content type, the type sniffed from the recovered bytes (with type_mismatch flagging an executable served as text), size, source and destination endpoints, packet number, timestamp, MD5, SHA-256, a completeness percentage, and its bytes inline as base64 within max_content_bytes. Limits, stated up front: encrypted transports (HTTPS/TLS, FTPS, SMB3 encryption) yield nothing without key material; SMB1/CIFS is detected but not carved; email protocols (SMTP/POP3/IMAP) and UDP-borne transfers (TFTP, QUIC) are out of scope; IP fragments after the first are skipped; captures are capped at 32 MiB. Provide the capture as either url (HTTP/HTTPS) or ref (id from a prior file upload/tool call). Runs locally — the capture never leaves the device.",
        parameters = schema_json()
    ),
)]
impl PcapFileExtractor {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("pcap-file-extractor")?;
    let opts = options(&args)?;
    let (bytes, _mime, _filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_CAPTURE_BYTES)?;
    let report = build(&bytes, &opts)?;
    serde_json::to_vec(&report)
        .map_err(|e| SkillError::Serialize(format!("serialize pcap-file-extractor response: {e}")))
}

fn options(args: &Args) -> Result<Options, SkillError> {
    let opts = Options {
        filter: args.filter.clone().unwrap_or_default(),
        min_size: args.min_size.unwrap_or(0).min(usize::MAX as u64) as usize,
        include_incomplete: args.include_incomplete.unwrap_or(true),
        include_content: args.include_content.unwrap_or(true),
        content_budget: args
            .max_content_bytes
            .unwrap_or(DEFAULT_CONTENT_BUDGET)
            .min(MAX_CONTENT_BUDGET),
        limit: args.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT) as usize,
        ..Options::default()
    };
    opts.with_protocols(args.protocols.as_deref().unwrap_or("all"))
        .map_err(SkillError::InvalidArgs)
}

fn build(bytes: &[u8], opts: &Options) -> Result<ExtractResult, SkillError> {
    if bytes.is_empty() {
        return Err(SkillError::InvalidArgs(
            "the capture is empty — upload a .pcap or .pcapng file with packets in it".into(),
        ));
    }
    extract(bytes, opts).map_err(SkillError::InvalidArgs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(json: &str) -> Args {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "File URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "protocols": {
                        "type": "string",
                        "default": "all",
                        "description": "Which protocols to carve: 'all' (default) or a comma-separated subset of http, ftp, smb — for example 'http,smb'. HTTP covers response bodies and POST/PUT/PATCH uploads; FTP uses the control channel (RETR/STOR/LIST plus PASV/EPSV/PORT/EPRT) to name each data connection; SMB covers SMB2/3 CREATE+READ/WRITE (SMB1/CIFS is detected and reported, not carved)."
                    },
                    "filter": {
                        "type": "string",
                        "default": "",
                        "description": "Case-insensitive substring that an object must match on its filename, request path, host, or declared content type — the equivalent of a packet analyser's object-list text filter. Example: 'exe' or 'invoice'. Empty (default) keeps everything."
                    },
                    "min_size": {
                        "type": "integer",
                        "default": 0,
                        "minimum": 0,
                        "maximum": 1073741824,
                        "description": "Drop recovered objects smaller than this many bytes (default 0 = keep all). Useful for skipping 1-pixel trackers and tiny redirect bodies."
                    },
                    "include_incomplete": {
                        "type": "boolean",
                        "default": true,
                        "description": "Include objects whose bytes were only partly captured (default true). Each object reports complete plus completeness_percent, so a partial carve is never mistaken for a whole file. Set false to keep only 100%-complete files."
                    },
                    "include_content": {
                        "type": "boolean",
                        "default": true,
                        "description": "Return each object's bytes inline as base64 (default true). Set false for a fast inventory — filenames, sizes, types, MD5 and SHA-256 are still reported without the payload."
                    },
                    "max_content_bytes": {
                        "type": "integer",
                        "default": 4194304,
                        "minimum": 0,
                        "maximum": 16777216,
                        "description": "Total budget in bytes for inline base64 content across all returned objects (default 4194304 = 4 MiB, maximum 16777216 = 16 MiB). Objects are inlined in packet order until the budget runs out; the rest are still listed with their hashes, and a note says so."
                    },
                    "limit": {
                        "type": "integer",
                        "default": 100,
                        "minimum": 1,
                        "maximum": 5000,
                        "description": "Maximum number of recovered objects to return (1-5000, default 100). files_total always reports how many matched before this cap."
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

    #[test]
    fn defaults_are_applied_when_only_a_source_is_given() {
        let opts = options(&args(r#"{"url":"https://example.com/a.pcap"}"#)).unwrap();
        assert!(opts.http && opts.ftp && opts.smb);
        assert_eq!(opts.limit, DEFAULT_LIMIT as usize);
        assert_eq!(opts.content_budget, DEFAULT_CONTENT_BUDGET);
        assert!(opts.include_content);
        assert!(opts.include_incomplete);
    }

    #[test]
    fn parameters_are_parsed_and_clamped() {
        let opts = options(&args(
            r#"{"url":"https://example.com/a.pcap","protocols":"smb","filter":"Plan","min_size":128,
                "include_content":false,"include_incomplete":false,"max_content_bytes":99999999,"limit":99999}"#,
        ))
        .unwrap();
        assert!(opts.smb && !opts.http && !opts.ftp);
        assert_eq!(opts.filter, "Plan");
        assert_eq!(opts.min_size, 128);
        assert!(!opts.include_content);
        assert!(!opts.include_incomplete);
        assert_eq!(opts.content_budget, MAX_CONTENT_BUDGET);
        assert_eq!(opts.limit, MAX_LIMIT as usize);
    }

    #[test]
    fn an_unknown_protocol_is_rejected() {
        let err = options(&args(r#"{"url":"https://e.example/a.pcap","protocols":"smtp"}"#))
            .unwrap_err();
        assert!(format!("{err:?}").contains("unknown protocol"), "{err:?}");
    }

    #[test]
    fn an_empty_capture_is_rejected() {
        let err = build(&[], &Options::default()).unwrap_err();
        assert!(format!("{err:?}").contains("capture is empty"), "{err:?}");
    }

    #[test]
    fn a_non_capture_file_is_rejected() {
        let err = build(b"not a capture, just some prose here", &Options::default()).unwrap_err();
        assert!(format!("{err:?}").contains("unrecognised capture format"), "{err:?}");
    }
}

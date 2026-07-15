//! gizza-ai/pdf-signature-inspector — extract embedded digital signatures from
//! a PDF and report, per signature, the signer (certificate subject/issuer DN),
//! the signing time (signature dictionary `/M` + the CMS `signingTime`
//! attribute), the signature handler/sub-filter, and whether the `/ByteRange`
//! digest window is structurally intact.
//!
//! Pipeline: parse `{url|ref}` → fetch the PDF bytes via `block-utils`
//! `resolve_source` (URL fetch through `wafer-run/network`, or an uploaded
//! attachment ref) → delegate to the pure `core::inspect` (lopdf + RustCrypto
//! `cms`/`x509-cert`) → return the structured findings as a flat JSON response
//! the LLM reads directly.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI). Like pdf-extract-text, the success shape is a flat JSON
//! object (not the `{ "result": … }` wrapper `run_skill` produces), so the
//! handler stays thin rather than going through `run_skill`.
//!
//! No page surface: a PDF is a binary file input and the output is structured
//! JSON, which fits neither the pure-text nor the ffmpeg file→media page shapes
//! — this is a chat + CLI block (the "no-page file-input" pattern, like
//! pdf-extract-text / web-fetch).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{AssetKind, Input, SkillError, SourceFields, ToolDescriptor};
use gizza_ai_pdf_signature_inspector_core::{inspect, SignatureInfo};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024; // 16 MiB

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
}

#[derive(Serialize)]
struct SigOut {
    #[serde(skip_serializing_if = "Option::is_none")]
    field_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    filter: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sub_filter: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    signing_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    signing_time_pdf_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    signer_subject: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    signer_issuer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    byte_range: Option<[i64; 4]>,
    byte_range_intact: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
}

impl From<SignatureInfo> for SigOut {
    fn from(s: SignatureInfo) -> Self {
        SigOut {
            field_name: s.field_name,
            filter: s.filter,
            sub_filter: s.sub_filter,
            name: s.name,
            reason: s.reason,
            location: s.location,
            // Prefer the cryptographically-signed CMS time; fall back to the
            // dictionary /M date string.
            signing_time: s.signing_time_cms,
            signing_time_pdf_date: s.signing_time_dict,
            signer_subject: s.signer_subject,
            signer_issuer: s.signer_issuer,
            byte_range: s.byte_range,
            byte_range_intact: s.byte_range_intact,
            note: s.note,
        }
    }
}

#[derive(Serialize)]
struct Resp {
    /// True when the document carries at least one signature.
    signed: bool,
    /// Number of embedded signatures found.
    signature_count: usize,
    signatures: Vec<SigOut>,
    /// Explains the scope of the integrity check so callers don't over-trust it.
    integrity_check: &'static str,
}

/// Single-source param descriptor → chat schema (and CLI). `Input::Document`
/// emits the `url`⊕`ref` `oneOf` (a PDF arrives via URL fetch or an attachment
/// ref). No other params — the tool inspects whatever signatures the PDF holds.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Document)
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct PdfSignatureInspector;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pdf-signature-inspector",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Inspect embedded digital signatures in a PDF",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Extract the embedded digital signatures from a PDF and report, for each one: the signer (certificate subject and issuer Distinguished Names), the signing time, the signature handler/sub-filter, and whether the /ByteRange digest window is structurally intact (i.e. no bytes were appended or changed outside the signed region). Provide url (HTTP/HTTPS) or ref from a prior tool call. This is a structural inspection — it reads the PKCS#7/CMS signature metadata and checks the byte ranges; it does NOT cryptographically verify the digest against a trusted certificate chain.",
        parameters = schema_json()
    ),
)]
impl PdfSignatureInspector {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).map_err(|e| {
        SkillError::InvalidArgs(format!("invalid arguments for pdf-signature-inspector: {e}"))
    })?;

    let (input_bytes, _mime, _filename) =
        resolve_source(args.source.into_inner(), AssetKind::Document, MAX_INPUT_BYTES)?;

    let inspection = inspect(&input_bytes).map_err(SkillError::InvalidArgs)?;
    let signatures: Vec<SigOut> = inspection.signatures.into_iter().map(SigOut::from).collect();

    let resp = Resp {
        signed: !signatures.is_empty(),
        signature_count: signatures.len(),
        signatures,
        integrity_check:
            "structural: /ByteRange coverage + PKCS#7/CMS metadata only; not a cryptographic digest/chain verification",
    };
    serde_json::to_vec(&resp).map_err(|e| {
        SkillError::Serialize(format!("serialize pdf-signature-inspector response: {e}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use gizza_ai_block_utils::Source;

    /// Migration safety: the descriptor-derived chat schema must match the
    /// authored schema (drift guard). `Input::Document` with no params emits
    /// just the `url`⊕`ref` `oneOf`.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Document URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." }
                },
                "additionalProperties": false,
                "oneOf": [
                    { "required": ["url"] },
                    { "required": ["ref"] }
                ]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn args_parse_url() {
        let a: Args = serde_json::from_str(r#"{"url":"https://x/y.pdf"}"#).unwrap();
        assert!(matches!(a.source.into_inner(), Source::Url(u) if u == "https://x/y.pdf"));
    }

    #[test]
    fn args_parse_ref() {
        let a: Args = serde_json::from_str(r#"{"ref":"call_7"}"#).unwrap();
        assert!(matches!(a.source.into_inner(), Source::Ref(r) if r == "call_7"));
    }

    #[test]
    fn args_reject_both_url_and_ref() {
        let err = serde_json::from_str::<Args>(r#"{"url":"u","ref":"r"}"#).unwrap_err();
        assert!(err.to_string().contains("exactly one"));
    }

    #[test]
    fn sigout_prefers_cms_time() {
        let info = SignatureInfo {
            signing_time_cms: Some("2024-01-15T12:00:00Z".into()),
            signing_time_dict: Some("D:20240115120000Z".into()),
            ..Default::default()
        };
        let out = SigOut::from(info);
        assert_eq!(out.signing_time.as_deref(), Some("2024-01-15T12:00:00Z"));
        assert_eq!(out.signing_time_pdf_date.as_deref(), Some("D:20240115120000Z"));
    }
}

//! gizza-ai/fuzzy-hash — compute an SSDEEP context-triggered piecewise hash
//! (CTPH, "fuzzy hash") of a file.
//!
//! Pipeline: resolve the source file (URL fetch or attachment ref, any bytes) →
//! `core::fuzzy_hash` (pure Rust ssdeep) → flat JSON the LLM reads directly.
//! Unlike a cryptographic hash, a fuzzy hash changes only slightly when the
//! input changes slightly, so two fingerprints can be compared for similarity —
//! useful for malware triage, near-duplicate detection, and spam clustering.
//! Pass `compare` (an existing ssdeep hash) to also get a 0..100 similarity
//! score against the file's hash.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (a file→fingerprint report fits neither the
//! pure-text nor the ffmpeg media page shape — the no-page file-input pattern,
//! like file-hash / detect-file-type).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::Serialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 64 * 1024 * 1024; // 64 MiB

#[derive(serde::Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    /// Optional existing ssdeep hash to compare the file's hash against.
    #[serde(default)]
    compare: Option<String>,
}

#[derive(Serialize)]
struct Resp {
    /// The ssdeep fuzzy hash, `blocksize:sig1:sig2`.
    ssdeep: String,
    /// Size of the hashed file in bytes.
    bytes: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    filename: Option<String>,
    /// Similarity score 0..100 vs the `compare` hash (only when `compare` set).
    #[serde(skip_serializing_if = "Option::is_none")]
    similarity: Option<u32>,
    /// Echo of the `compare` hash that `similarity` was computed against.
    #[serde(skip_serializing_if = "Option::is_none")]
    compared_to: Option<String>,
}

/// `Input::File` emits the `url`⊕`ref` `oneOf` — a file arrives via URL fetch or
/// an attachment ref. Optional `compare` adds a similarity score.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::File).param(
        Param::string("compare")
            .describe("Optional: an existing ssdeep fuzzy hash (blocksize:sig1:sig2). When set, the response includes a 0-100 similarity score between this hash and the file's hash."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct FuzzyHash;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/fuzzy-hash",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compute an SSDEEP fuzzy hash (CTPH) of a file",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Compute an SSDEEP context-triggered piecewise hash (CTPH, a 'fuzzy hash') of a file, as blocksize:sig1:sig2. Unlike MD5/SHA, a fuzzy hash changes only slightly when the input changes slightly, so two fingerprints can be compared for similarity — useful for malware triage, near-duplicate detection, and spam clustering. Optionally pass compare (an existing ssdeep hash) to also get a 0-100 similarity score against the file's hash. Provide the file as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl FuzzyHash {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("fuzzy-hash")?;
    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_BYTES)?;

    let ssdeep = gizza_ai_fuzzy_hash_core::fuzzy_hash(&bytes);
    let (similarity, compared_to) = match args.compare.as_deref() {
        Some(other) if !other.trim().is_empty() => (
            Some(gizza_ai_fuzzy_hash_core::compare(&ssdeep, other.trim())),
            Some(other.trim().to_string()),
        ),
        _ => (None, None),
    };

    let resp = Resp {
        ssdeep,
        bytes: bytes.len(),
        filename: (!filename.is_empty()).then_some(filename),
        similarity,
        compared_to,
    };
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize fuzzy-hash response: {e}")))
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
                    "compare": { "type": "string", "description": "Optional: an existing ssdeep fuzzy hash (blocksize:sig1:sig2). When set, the response includes a 0-100 similarity score between this hash and the file's hash." }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}

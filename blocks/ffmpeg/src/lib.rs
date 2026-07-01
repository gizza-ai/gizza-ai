//! gizza-ai/ffmpeg — fetch a URL, run `ffmpeg -i input` on the bytes,
//! return the log.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI). The handler stays thin (parse `Args`, fetch the bytes,
//! run ffmpeg-runtime, emit the flat `ToolResp`) rather than going through
//! `run_skill`, because ffmpeg's success shape is the flat `ToolResp` JSON
//! (`{ url, info }`), not the `{ "result": … }` wrapper `run_skill` produces.

// The #[wafer_block] macro emits wasm-only registration; supporting imports
// and the Args type are only used inside that impl. See image-resize for
// the full rationale.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use std::collections::HashMap;

use gizza_ai_block_utils::{
    dispatch_ffmpeg_runtime, FfmpegReq, FfmpegResp, Input, Param, SkillError, SkillResultExt,
    ToolDescriptor,
};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_BYTES: usize = 16 * 1024 * 1024; // 16 MiB

#[derive(Deserialize)]
struct Args {
    url: String,
}

#[derive(Serialize)]
struct ToolResp {
    url: String,
    info: String,
}

/// Single-source param descriptor → chat schema (and CLI). See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
/// ffmpeg is `Input::None` — `url` is a normal required string param (the
/// chat/CLI surface takes only a `url`, with no `ref`), so there is no
/// `url`⊕`ref` `oneOf`.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::string("url")
            .required()
            .describe("HTTP/HTTPS URL of the media file to inspect."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// argv for an ffprobe-style inspection — read `input` from ffmpeg's
/// virtual FS, emit format/stream metadata on stderr, no output file.
fn build_inspect_argv() -> Vec<String> {
    vec!["-i".into(), "input".into()]
}

#[cfg(target_arch = "wasm32")]
struct FfmpegSkill;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/ffmpeg",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Inspect a media file's codec/duration/dimensions via ffmpeg.",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Run ffprobe on a media URL and return format/stream metadata.",
        parameters = schema_json()
    ),
)]
impl FfmpegSkill {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // ffmpeg returns the flat ToolResp JSON directly (no `{ "result": … }`
        // wrapper), so it keeps a thin handle rather than using run_skill.
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("ffmpeg")?;

    let net = wafer_sdk::clients::network::do_request("GET", &args.url, &HashMap::new(), None)?;
    if net.status_code >= 400 {
        return Err(SkillError::HttpStatus {
            status: net.status_code,
            url: args.url,
        });
    }
    if net.body.len() > MAX_BYTES {
        return Err(SkillError::TooLarge {
            kind: "media file",
            bytes: net.body.len(),
            cap: MAX_BYTES,
        });
    }

    // Hand bytes to gizza-ai/ffmpeg-runtime (consumer-controlled JSON
    // protocol, NOT a wafer-run service — must encode via serde_json).
    let ffreq = serde_json::to_vec(&FfmpegReq {
        args: build_inspect_argv(),
        inputs: vec![("input".into(), net.body)],
        output: String::new(),
    })
    .map_err(|e| SkillError::Serialize(format!("serialize ffmpeg request: {e}")))?;
    let ff_resp_bytes = dispatch_ffmpeg_runtime(&ffreq)?;
    let ff: FfmpegResp = serde_json::from_slice(&ff_resp_bytes)
        .map_err(|e| SkillError::Serialize(format!("malformed ffmpeg response: {e}")))?;
    // ffmpeg's exit code is expected to be non-zero when no output file is
    // produced; the log is what we want regardless.

    let tool = ToolResp {
        url: args.url,
        info: ff.log,
    };
    serde_json::to_vec(&tool)
        .map_err(|e| SkillError::Serialize(format!("serialize tool response: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Migration safety: the descriptor-derived chat schema must match the
    /// pre-retrofit authored schema, so the LLM sees no drift. (to_schema_json
    /// emits `additionalProperties: false`, which ffmpeg's authored schema
    /// already had — so this is an exact match.)
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "HTTP/HTTPS URL of the media file to inspect." }
                },
                "required": ["url"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn argv_uses_input_filename_with_dash_i() {
        let argv = build_inspect_argv();
        assert_eq!(argv, vec!["-i", "input"]);
    }

    #[test]
    fn tool_resp_serializes_url_and_info() {
        let tool = ToolResp {
            url: "https://x.test/a.mp4".to_string(),
            info: "Stream #0: Video".to_string(),
        };
        let json: serde_json::Value =
            serde_json::from_slice(&serde_json::to_vec(&tool).unwrap()).unwrap();
        assert_eq!(json["url"], "https://x.test/a.mp4");
        assert_eq!(json["info"], "Stream #0: Video");
    }
}

//! gizza-ai/pdf-javascript-extractor — locate every piece of JavaScript a PDF
//! can execute, recover its source, and unwind the obfuscation layers around it.
//!
//! Pipeline: parse `{pdf_base64, deobfuscate, beautify, include_raw, detail,
//! max_script_chars}` → decode the PDF bytes → delegate to the pure
//! `core::extract` (lopdf + the shared js-beautify core) → return the structured
//! report as a flat JSON response the LLM reads directly.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI). Like the sibling pdf-object-analyzer / pdf-extract-text
//! blocks, the success shape is a flat JSON object (not the `{ "result": … }`
//! wrapper `run_skill` produces), so the handler stays thin.
//!
//! Static analysis only: the extracted JavaScript is never executed, emulated,
//! or fetched, and the report is not a malware verdict.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::Engine;
use gizza_ai_block_utils::{Input, Param, SkillError, ToolDescriptor};
use gizza_ai_pdf_javascript_extractor_core::{extract, Options};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024; // 16 MiB
const MIN_SCRIPT_CHARS: u64 = 500;
const MAX_SCRIPT_CHARS: u64 = 200_000;

#[derive(Deserialize, Debug)]
struct Args {
    /// Base64-encoded PDF bytes. A `data:application/pdf;base64,...` prefix is
    /// accepted for copy/paste convenience.
    pdf_base64: String,
    /// Unwind the common obfuscation layers (default true).
    #[serde(default)]
    deobfuscate: Option<bool>,
    /// Re-indent the recovered source (default true).
    #[serde(default)]
    beautify: Option<bool>,
    /// Also return the untouched extracted source (default false).
    #[serde(default)]
    include_raw: Option<bool>,
    /// `full` (default) includes the script source; `summary` returns metadata only.
    #[serde(default)]
    detail: Option<String>,
    /// Per-script character cap (default 20000).
    #[serde(default)]
    max_script_chars: Option<u64>,
}

/// Single-source param descriptor → chat schema (and CLI). The PDF arrives as
/// base64 so the extractor stays pure/no-network and runs in every wafer
/// runtime; the remaining params control decoding and report depth.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("pdf_base64")
                .required()
                .describe(
                    "Base64-encoded PDF bytes. A data:application/pdf;base64,... prefix is accepted.",
                ),
        )
        .param(
            Param::boolean("deobfuscate")
                .default(true)
                .describe(
                    "Unwind common JavaScript obfuscation before reporting the source (default \
                     true): String.fromCharCode(decimal and 0x hex), unescape/decodeURIComponent \
                     over %XX and %uXXXX, atob base64, \\xNN / \\uNNNN / octal escapes inside \
                     string literals, and \"a\" + \"b\" literal concatenation. The passes are \
                     re-applied up to 4 times so nested layers unwrap; each script reports which \
                     passes fired (decodings) and how many rounds changed it (rounds). Set false \
                     to see exactly what the PDF stores.",
                ),
        )
        .param(
            Param::boolean("beautify")
                .default(true)
                .describe(
                    "Re-indent the recovered source with 2-space indentation (default true). \
                     Obfuscated PDF JavaScript is usually one very long line. Whitespace-only \
                     change: no code is reordered or dropped.",
                ),
        )
        .param(
            Param::boolean("include_raw")
                .default(false)
                .describe(
                    "Also return each script's untouched extracted source as `raw`, alongside the \
                     decoded `source`, so the two can be compared (default false). Roughly \
                     doubles the response size.",
                ),
        )
        .param(
            Param::enumv("detail", ["full", "summary"])
                .default("full")
                .describe(
                    "Report depth. 'full' (default) includes each script's source; 'summary' \
                     returns metadata only — object id, trigger, location, length, which \
                     decodings fired, indicators and URLs — which is the cheap way to answer \
                     'does this PDF contain JavaScript, and where?'.",
                ),
        )
        .param(
            Param::integer("max_script_chars")
                .default(20000)
                .min(MIN_SCRIPT_CHARS as f64)
                .max(MAX_SCRIPT_CHARS as f64)
                .describe(
                    "Per-script character cap on the returned source (default 20000, range \
                     500-200000). Real-world PDF droppers are a few KB, so the default rarely \
                     bites; when it does, the script's `truncated` flag and a top-level note say \
                     so. At most 64 scripts are reported.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct PdfJavascriptExtractor;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pdf-javascript-extractor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract and de-obfuscate every JavaScript a PDF can run, with its trigger and location",
    skill(
        description = "Locate every piece of JavaScript embedded in a PDF, recover its source, and unwind the obfuscation layers around it. Walks the document-level /Names -> /JavaScript name tree (including nested /Kids), /OpenAction, /AA additional-action dictionaries on the catalog, pages, annotations and form fields, annotation and form-field /A actions with their /Next chains, and finishes with a catch-all sweep for any other object carrying /JS. Script bodies stored as PDF strings (literal, hex, PDFDocEncoding or UTF-16BE) and as streams (inflated through the declared filters, e.g. FlateDecode) are both handled. Each script is reported with its object id, what makes it run (trigger: document-open, document-level, additional-action, annotation-action, form-field-action, object-scan), where it sits in the object graph (location), the name-tree entry name where applicable, whether it came from a string or a stream, its length, and the source. De-obfuscation unwinds String.fromCharCode, unescape/decodeURIComponent (%XX and %uXXXX), atob base64, backslash \\xNN / \\uNNNN / octal escapes, and literal string concatenation, iterating up to 4 rounds so nested layers unwrap; the passes that fired are listed per script. The report also flags suspicious Acrobat/JavaScript API names (eval, app.launchURL, app.setTimeOut, util.printf, Collab.collectEmailInfo, media.newPlayer, exportDataObject, ActiveXObject, WScript.Shell, ...) and pulls out URLs found in the decoded source. Provide pdf_base64 containing the PDF bytes (a data:application/pdf;base64,... prefix is accepted). This is static analysis: the JavaScript is never executed, emulated, or fetched, no shellcode emulation or reputation lookup is performed, encrypted PDFs are not decrypted, and the result is NOT a malware verdict.",
        parameters = schema_json()
    )
)]
impl PdfJavascriptExtractor {
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
        SkillError::InvalidArgs(format!("invalid arguments for pdf-javascript-extractor: {e}"))
    })?;
    let (opts, summary) = options_from(&args)?;

    let encoded = args
        .pdf_base64
        .strip_prefix("data:application/pdf;base64,")
        .unwrap_or(&args.pdf_base64);
    let input_bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded.trim())
        .map_err(|e| SkillError::InvalidArgs(format!("pdf_base64 is not valid base64: {e}")))?;
    if input_bytes.len() > MAX_INPUT_BYTES {
        return Err(SkillError::InvalidArgs(format!(
            "PDF is too large: {} bytes (limit {MAX_INPUT_BYTES})",
            input_bytes.len()
        )));
    }

    let mut report = extract(&input_bytes, &opts).map_err(SkillError::InvalidArgs)?;
    if summary {
        report = report.summarized();
    }

    serde_json::to_vec(&report).map_err(|e| {
        SkillError::Serialize(format!("serialize pdf-javascript-extractor response: {e}"))
    })
}

/// Validate the knobs and build the core `Options`. Returns the options plus
/// whether `detail=summary` was requested. Errors name the expected values.
fn options_from(args: &Args) -> Result<(Options, bool), SkillError> {
    let summary = match args.detail.as_deref() {
        None | Some("full") => false,
        Some("summary") => true,
        Some(other) => {
            return Err(SkillError::InvalidArgs(format!(
                "detail must be 'full' or 'summary', got '{other}'"
            )))
        }
    };
    let max_script_chars = match args.max_script_chars {
        None => 20_000,
        Some(n) if (MIN_SCRIPT_CHARS..=MAX_SCRIPT_CHARS).contains(&n) => n as usize,
        Some(n) => {
            return Err(SkillError::InvalidArgs(format!(
                "max_script_chars must be between {MIN_SCRIPT_CHARS} and {MAX_SCRIPT_CHARS}, got {n}"
            )))
        }
    };
    Ok((
        Options {
            deobfuscate: args.deobfuscate.unwrap_or(true),
            beautify: args.beautify.unwrap_or(true),
            include_raw: args.include_raw.unwrap_or(false),
            max_script_chars,
        },
        summary,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Migration safety: the descriptor-derived chat schema must match the
    /// authored schema (drift guard). Regenerate this literal whenever
    /// `descriptor()` changes — never the other way around.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "pdf_base64": {
                        "type": "string",
                        "description": "Base64-encoded PDF bytes. A data:application/pdf;base64,... prefix is accepted."
                    },
                    "deobfuscate": {
                        "type": "boolean",
                        "default": true,
                        "description": "Unwind common JavaScript obfuscation before reporting the source (default true): String.fromCharCode(decimal and 0x hex), unescape/decodeURIComponent over %XX and %uXXXX, atob base64, \\xNN / \\uNNNN / octal escapes inside string literals, and \"a\" + \"b\" literal concatenation. The passes are re-applied up to 4 times so nested layers unwrap; each script reports which passes fired (decodings) and how many rounds changed it (rounds). Set false to see exactly what the PDF stores."
                    },
                    "beautify": {
                        "type": "boolean",
                        "default": true,
                        "description": "Re-indent the recovered source with 2-space indentation (default true). Obfuscated PDF JavaScript is usually one very long line. Whitespace-only change: no code is reordered or dropped."
                    },
                    "include_raw": {
                        "type": "boolean",
                        "default": false,
                        "description": "Also return each script's untouched extracted source as `raw`, alongside the decoded `source`, so the two can be compared (default false). Roughly doubles the response size."
                    },
                    "detail": {
                        "type": "string",
                        "enum": ["full", "summary"],
                        "default": "full",
                        "description": "Report depth. 'full' (default) includes each script's source; 'summary' returns metadata only — object id, trigger, location, length, which decodings fired, indicators and URLs — which is the cheap way to answer 'does this PDF contain JavaScript, and where?'."
                    },
                    "max_script_chars": {
                        "type": "integer",
                        "minimum": 500,
                        "maximum": 200000,
                        "default": 20000,
                        "description": "Per-script character cap on the returned source (default 20000, range 500-200000). Real-world PDF droppers are a few KB, so the default rarely bites; when it does, the script's `truncated` flag and a top-level note say so. At most 64 scripts are reported."
                    }
                },
                "additionalProperties": false,
                "required": ["pdf_base64"]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn args_parse_base64_with_defaults() {
        let a: Args = serde_json::from_str(r#"{"pdf_base64":"JVBERi0xLjQ="}"#).unwrap();
        assert_eq!(a.pdf_base64, "JVBERi0xLjQ=");
        let (opts, summary) = options_from(&a).unwrap();
        assert!(!summary);
        assert_eq!(opts, Options::default());
    }

    #[test]
    fn args_parse_all_knobs() {
        let a: Args = serde_json::from_str(
            r#"{"pdf_base64":"data:application/pdf;base64,JVBERi0xLjQ=","deobfuscate":false,
                "beautify":false,"include_raw":true,"detail":"summary","max_script_chars":500}"#,
        )
        .unwrap();
        assert!(a.pdf_base64.starts_with("data:application/pdf;base64,"));
        let (opts, summary) = options_from(&a).unwrap();
        assert!(summary);
        assert!(!opts.deobfuscate && !opts.beautify && opts.include_raw);
        assert_eq!(opts.max_script_chars, 500);
    }

    #[test]
    fn args_require_base64() {
        let err = serde_json::from_str::<Args>(r#"{"detail":"summary"}"#).unwrap_err();
        assert!(err.to_string().contains("missing field `pdf_base64`"));
    }

    #[test]
    fn bad_detail_and_out_of_range_cap_are_rejected_with_the_expected_values() {
        let a: Args =
            serde_json::from_str(r#"{"pdf_base64":"JVBERi0xLjQ=","detail":"brief"}"#).unwrap();
        let e = options_from(&a).unwrap_err().to_string();
        assert!(e.contains("detail must be 'full' or 'summary', got 'brief'"), "got: {e}");

        let a: Args =
            serde_json::from_str(r#"{"pdf_base64":"JVBERi0xLjQ=","max_script_chars":10}"#).unwrap();
        let e = options_from(&a).unwrap_err().to_string();
        assert!(
            e.contains("max_script_chars must be between 500 and 200000, got 10"),
            "got: {e}"
        );
    }
}

//! gizza-ai/xlsx-sheet-diff — cell-by-cell diff of two worksheets in a workbook.
//!
//! Pipeline: parse `{url|ref}` + sheet selectors + options → fetch the workbook
//! bytes via `block-utils` `resolve_source` (URL fetch through
//! `wafer-run/network`, or an uploaded attachment ref) → delegate to the pure
//! `core::diff` (calamine) → return the diff report as a flat JSON response the
//! LLM reads directly.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI). The handler stays thin (parse `Args`, run the diff, emit
//! the flat `Resp` JSON) rather than going through `run_skill`, because the
//! success shape is the flat `Resp` JSON, not the `{ "result": … }` wrapper.
//!
//! No page surface: a spreadsheet is a binary file input and the output is a
//! plain-text diff, which fits neither the pure-text nor the ffmpeg file→media
//! page shapes — this is a chat + CLI block (the no-page file-input pattern,
//! like xlsx-to-csv / pdf-extract-text).

// The #[wafer_block] macro emits the impl gated to wasm32. The supporting
// imports + the Args type are only used inside that impl, so they look "unused"
// when running native unit tests. `descriptor()`/`schema_json()` stay
// native-compilable so the drift-guard + arg-parse tests can exercise them.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_xlsx_sheet_diff_core::{diff, Options};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

/// Cap on the workbook we accept (matches xlsx-to-csv; calamine holds the whole
/// workbook in memory).
const MAX_INPUT_BYTES: usize = 4 * 1024 * 1024; // 4 MiB

/// Cap on the diff report fed back to the LLM. Larger reports are head-clipped
/// with a note.
const MAX_OUTPUT_CHARS: usize = 200_000;

#[derive(Debug, Deserialize)]
struct Args {
    /// Exactly one of `url` / `ref` (validated at deserialize time).
    #[serde(flatten)]
    source: SourceFields,
    /// First worksheet: a sheet name, or a 0-based index as a string. Defaults
    /// to the first sheet.
    #[serde(default)]
    sheet1: Option<String>,
    /// Second worksheet: a sheet name, or a 0-based index as a string. Defaults
    /// to the second sheet.
    #[serde(default)]
    sheet2: Option<String>,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_true")]
    compare_formulas: bool,
    #[serde(default)]
    ignore_case: bool,
    #[serde(default)]
    ignore_whitespace: bool,
}

fn default_format() -> String {
    "table".to_string()
}
fn default_true() -> bool {
    true
}

#[derive(Serialize)]
struct Resp {
    /// The rendered diff report (table / json / csv per `format`).
    report: String,
    /// The format the report was rendered in.
    format: String,
    /// True when `report` was clipped to the output cap.
    truncated: bool,
}

/// Single-source param descriptor → chat schema (and CLI). `Input::Document`
/// emits the `url`⊕`ref` `oneOf` (a workbook arrives via URL fetch or an
/// attachment ref).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Document)
        .param(Param::string("sheet1").describe(
            "First worksheet to compare: a sheet name, or a 0-based index as a string (e.g. \"0\"). Defaults to the first sheet.",
        ))
        .param(Param::string("sheet2").describe(
            "Second worksheet to compare: a sheet name, or a 0-based index as a string (e.g. \"1\"). Defaults to the second sheet.",
        ))
        .param(
            Param::enumv("format", ["table", "json", "csv"])
                .default("table")
                .describe("Output: table (readable report), json (structured report), or csv (flat change-log, one row per changed cell)."),
        )
        .param(
            Param::boolean("compare_formulas")
                .default(true)
                .describe("Also compare stored formula strings, so a rewritten formula is caught even when its cached result is unchanged."),
        )
        .param(
            Param::boolean("ignore_case")
                .default(false)
                .describe("Compare cell and formula text case-insensitively (original text is still shown)."),
        )
        .param(
            Param::boolean("ignore_whitespace")
                .default(false)
                .describe("Collapse runs of whitespace before comparing (original text is still shown)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Clip `text` to at most `max_chars` unicode characters. Returns
/// `(clipped, was_truncated)`.
fn clip_chars(text: &str, max_chars: usize) -> (String, bool) {
    if text.chars().count() > max_chars {
        (text.chars().take(max_chars).collect(), true)
    } else {
        (text.to_string(), false)
    }
}

#[cfg(target_arch = "wasm32")]
struct XlsxSheetDiff;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/xlsx-sheet-diff",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compare two spreadsheet sheets cell-by-cell (value, formula, structural diffs)",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Compare two worksheets of a spreadsheet (.xlsx, .xlsm, .xls, or .ods) cell-by-cell and report value changes, formula changes, and structural (added/removed row and column) differences. Provide the workbook via url (a public http/https link) or ref (an uploaded attachment id), and pick the two sheets by name or 0-based index (defaults to the first two). Output as a readable table, structured JSON, or a flat CSV change-log.",
        parameters = schema_json()
    ),
)]
impl XlsxSheetDiff {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // Returns the flat Resp JSON directly (no `{ "result": … }` wrapper),
        // so it keeps a thin handle rather than using run_skill.
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("xlsx-sheet-diff")?;

    let (bytes, _mime, _filename) =
        resolve_source(args.source.into_inner(), AssetKind::Document, MAX_INPUT_BYTES)?;

    let opts = Options {
        ignore_case: args.ignore_case,
        ignore_whitespace: args.ignore_whitespace,
        compare_formulas: args.compare_formulas,
    };
    let sheet1 = args.sheet1.as_deref().filter(|s| !s.trim().is_empty());
    let sheet2 = args.sheet2.as_deref().filter(|s| !s.trim().is_empty());

    let report =
        diff(&bytes, sheet1, sheet2, opts, &args.format).map_err(SkillError::InvalidArgs)?;
    let (report, truncated) = clip_chars(&report, MAX_OUTPUT_CHARS);

    let resp = Resp {
        report,
        format: args.format,
        truncated,
    };
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize xlsx-sheet-diff response: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gizza_ai_block_utils::Source;

    /// Migration safety: the descriptor-derived chat schema must match the
    /// authored schema, so the LLM sees no drift. The `url`/`ref` descriptions
    /// are fixed by `Input::Document` (single source); the enum/bool params keep
    /// their expected shapes.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":    { "type": "string", "description": "Document URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":    { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "sheet1": { "type": "string", "description": "First worksheet to compare: a sheet name, or a 0-based index as a string (e.g. \"0\"). Defaults to the first sheet." },
                    "sheet2": { "type": "string", "description": "Second worksheet to compare: a sheet name, or a 0-based index as a string (e.g. \"1\"). Defaults to the second sheet." },
                    "format": { "type": "string", "enum": ["table", "json", "csv"], "default": "table", "description": "Output: table (readable report), json (structured report), or csv (flat change-log, one row per changed cell)." },
                    "compare_formulas": { "type": "boolean", "default": true, "description": "Also compare stored formula strings, so a rewritten formula is caught even when its cached result is unchanged." },
                    "ignore_case": { "type": "boolean", "default": false, "description": "Compare cell and formula text case-insensitively (original text is still shown)." },
                    "ignore_whitespace": { "type": "boolean", "default": false, "description": "Collapse runs of whitespace before comparing (original text is still shown)." }
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
    fn args_parse_url_and_sheets() {
        let a: Args = serde_json::from_str(
            r#"{"url":"https://x/y.xlsx","sheet1":"Q1","sheet2":"Q2","format":"json"}"#,
        )
        .unwrap();
        assert!(matches!(a.source.into_inner(), Source::Url(u) if u == "https://x/y.xlsx"));
        assert_eq!(a.sheet1.as_deref(), Some("Q1"));
        assert_eq!(a.sheet2.as_deref(), Some("Q2"));
        assert_eq!(a.format, "json");
    }

    #[test]
    fn args_defaults() {
        let a: Args = serde_json::from_str(r#"{"ref":"call_1"}"#).unwrap();
        assert!(matches!(a.source.into_inner(), Source::Ref(r) if r == "call_1"));
        assert_eq!(a.sheet1, None);
        assert_eq!(a.sheet2, None);
        assert_eq!(a.format, "table");
        assert!(a.compare_formulas);
        assert!(!a.ignore_case);
        assert!(!a.ignore_whitespace);
    }

    #[test]
    fn args_reject_both_url_and_ref() {
        let err = serde_json::from_str::<Args>(r#"{"url":"u","ref":"r"}"#).unwrap_err();
        assert!(err.to_string().contains("exactly one"));
    }

    #[test]
    fn args_reject_neither_url_nor_ref() {
        let err = serde_json::from_str::<Args>(r#"{"format":"csv"}"#).unwrap_err();
        assert!(err.to_string().contains("required"));
    }

    #[test]
    fn clip_chars_truncates() {
        let (out, trunc) = clip_chars("abcdef", 3);
        assert_eq!(out, "abc");
        assert!(trunc);
    }
}

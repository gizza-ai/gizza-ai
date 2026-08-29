//! gizza-ai/convert-quotes — converts quote delimiters in text or code between
//! single, double and curly styles, respecting backslash escapes.
//!
//! Thin chat-skill wrapper around `gizza-ai-convert-quotes-core`. The chat schema
//! is derived from `descriptor()` (single source — shared shape across chat +
//! CLI); the handler delegates to `block_utils::run_skill`. No host calls — the
//! conversion is pure string work, so it runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    direction: String,
    #[serde(default)]
    escape_style: String,
    /// Treat word-internal quotes as apostrophes (default true).
    #[serde(default = "default_true")]
    preserve_apostrophes: bool,
    #[serde(default)]
    on_unbalanced: String,
    /// Return a JSON report alongside the text (default false).
    #[serde(default)]
    include_report: bool,
}

/// Single-source param descriptor → chat schema (and CLI). See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The text or code whose quote delimiters should be converted, e.g. print('hello') or He said \u{201C}hi\u{201D}. Everything outside a quoted run is passed through byte for byte. Up to 1,000,000 bytes (1000 KB) per run."),
        )
        .param(
            Param::enumv(
                "direction",
                [
                    "single-to-double",
                    "double-to-single",
                    "smart-to-double",
                    "smart-to-single",
                    "auto-to-double",
                    "auto-to-single",
                    "swap",
                ],
            )
            .default("single-to-double")
            .describe("Which delimiters to read and which to write. 'single-to-double' (default) turns 'x' into \"x\"; 'double-to-single' does the reverse; 'smart-to-double'/'smart-to-single' read curly \u{201C}x\u{201D} and \u{2018}x\u{2019} runs and emit straight quotes; 'auto-to-double'/'auto-to-single' read every style at once and normalize a mixed file to one delimiter; 'swap' exchanges ' and \" in a single pass."),
        )
        .param(
            Param::enumv("escape_style", ["backslash", "doubled", "bare"])
                .default("backslash")
                .describe("How to protect a quote that ends up inside the new delimiter. 'backslash' (default) writes \\\" or \\' — the C, JavaScript, Python, JSON, Rust and Go convention; 'doubled' writes \"\" or '' — the SQL, CSV and Pascal convention; 'bare' leaves the inner quote alone, which is right for prose but produces a broken literal in code."),
        )
        .param(
            Param::boolean("preserve_apostrophes")
                .default(true)
                .describe("When true (default), a ' or \u{2019} sitting between two word characters is an apostrophe, not a delimiter, so don't, it\u{2019}s and O'Hara survive and 'don't stop' still converts as one run. Set false to treat every single quote as a delimiter."),
        )
        .param(
            Param::enumv("on_unbalanced", ["keep", "error"])
                .default("keep")
                .describe("What to do with an opening quote that has no closing partner. 'keep' (default) leaves that one character exactly as it was and converts the rest; 'error' fails and names the character position, so a malformed file is not half-converted."),
        )
        .param(
            Param::boolean("include_report")
                .default(false)
                .describe("When true, return JSON with the converted text plus counts: 'converted' quoted runs, inner quotes 'escaped', and 'unbalanced' lone quotes left as-is. Default false returns just the converted text."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ConvertQuotes;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/convert-quotes",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert quote delimiters in text or code between single, double and curly styles, respecting escapes.",
    skill(
        description = "Convert the quote delimiters around quoted runs in text or code between styles: single 'x', double \"x\", curly \u{201C}x\u{201D} and \u{2018}x\u{2019}. Unlike a find-and-replace it parses the text: backslash escapes are respected so \\\" is never mistaken for a delimiter, a quote that no longer needs escaping is unescaped (\"a \\\" b\" -> 'a \" b'), a quote that now collides with the new delimiter is escaped (backslash, SQL-style doubling, or left bare), and word-internal apostrophes (don't, it\u{2019}s, O'Hara) are never treated as quotes. direction=single-to-double|double-to-single|smart-to-double|smart-to-single|auto-to-double|auto-to-single|swap; auto-* normalizes a file that mixes all four styles. An opening quote with no partner is left untouched by default, or reported with on_unbalanced=error. include_report=true adds counts of runs converted, quotes escaped and lone quotes found. Up to 1,000,000 bytes per run.",
        parameters = schema_json()
    )
)]
impl ConvertQuotes {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "convert-quotes", |a: Args| {
            gizza_ai_convert_quotes_core::run(
                &a.input,
                &a.direction,
                &a.escape_style,
                a.preserve_apostrophes,
                &a.on_unbalanced,
                a.include_report,
            )
            .map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed. Authored 2026-08-29 with the tool.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "The text or code whose quote delimiters should be converted, e.g. print('hello') or He said “hi”. Everything outside a quoted run is passed through byte for byte. Up to 1,000,000 bytes (1000 KB) per run." },
                    "direction": { "type": "string", "enum": ["single-to-double", "double-to-single", "smart-to-double", "smart-to-single", "auto-to-double", "auto-to-single", "swap"], "default": "single-to-double", "description": "Which delimiters to read and which to write. 'single-to-double' (default) turns 'x' into \"x\"; 'double-to-single' does the reverse; 'smart-to-double'/'smart-to-single' read curly “x” and ‘x’ runs and emit straight quotes; 'auto-to-double'/'auto-to-single' read every style at once and normalize a mixed file to one delimiter; 'swap' exchanges ' and \" in a single pass." },
                    "escape_style": { "type": "string", "enum": ["backslash", "doubled", "bare"], "default": "backslash", "description": "How to protect a quote that ends up inside the new delimiter. 'backslash' (default) writes \\\" or \\' — the C, JavaScript, Python, JSON, Rust and Go convention; 'doubled' writes \"\" or '' — the SQL, CSV and Pascal convention; 'bare' leaves the inner quote alone, which is right for prose but produces a broken literal in code." },
                    "preserve_apostrophes": { "type": "boolean", "default": true, "description": "When true (default), a ' or ’ sitting between two word characters is an apostrophe, not a delimiter, so don't, it’s and O'Hara survive and 'don't stop' still converts as one run. Set false to treat every single quote as a delimiter." },
                    "on_unbalanced": { "type": "string", "enum": ["keep", "error"], "default": "keep", "description": "What to do with an opening quote that has no closing partner. 'keep' (default) leaves that one character exactly as it was and converts the rest; 'error' fails and names the character position, so a malformed file is not half-converted." },
                    "include_report": { "type": "boolean", "default": false, "description": "When true, return JSON with the converted text plus counts: 'converted' quoted runs, inner quotes 'escaped', and 'unbalanced' lone quotes left as-is. Default false returns just the converted text." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}

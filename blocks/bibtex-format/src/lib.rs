//! gizza-ai/bibtex-format — validates, sorts, and pretty-prints BibTeX entries.
//!
//! Thin chat-skill wrapper around `gizza-ai-bibtex-format-core`. The chat schema
//! is derived from `descriptor()` (single source — shared shape across chat +
//! CLI); the handler delegates to `block_utils::run_skill`. No host calls — runs
//! entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    /// The BibTeX source to clean up.
    bibtex: String,
    #[serde(default)]
    sort: String,
    /// Sort the fields within each entry alphabetically (default false).
    #[serde(default)]
    sort_fields: bool,
    /// Spaces to indent fields, 0–16 (the core clamps). Default 2.
    #[serde(default = "default_indent")]
    indent: u32,
    /// Pad field names so all `=` line up within an entry (default false).
    #[serde(default)]
    align_values: bool,
    /// Lowercase the entry type / keyword (default true). Field names are
    /// always lowercased.
    #[serde(default = "default_true")]
    lowercase_type: bool,
    /// Error on a duplicate cite key (default true).
    #[serde(default = "default_true")]
    check_duplicates: bool,
}

fn default_indent() -> u32 {
    2
}
fn default_true() -> bool {
    true
}

/// Single-source param descriptor → chat schema (and CLI). See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("bibtex")
                .required()
                .describe("The BibTeX bibliography source to validate and pretty-print."),
        )
        .param(
            Param::enumv("sort", ["none", "key", "type-key"])
                .default("none")
                .describe("Reorder entries: 'none' (default) keeps source order; 'key' sorts case-insensitively by cite key; 'type-key' sorts by entry type then key. @string/@preamble/@comment items stay at the front."),
        )
        .param(
            Param::boolean("sort_fields")
                .default(false)
                .describe("When true, sort the fields within each entry alphabetically by name. Default false (keep source order)."),
        )
        .param(
            Param::integer("indent")
                .default(2)
                .min(0.0)
                .max(gizza_ai_bibtex_format_core::MAX_INDENT as f64)
                .describe("Spaces to indent each field line, 0-16. Default 2."),
        )
        .param(
            Param::boolean("align_values")
                .default(false)
                .describe("When true, pad field names so all '=' signs in an entry line up. Default false (a single space before '=')."),
        )
        .param(
            Param::boolean("lowercase_type")
                .default(true)
                .describe("When true (default), lowercase the entry type and the @string/@preamble/@comment keyword. Field names are always lowercased."),
        )
        .param(
            Param::boolean("check_duplicates")
                .default(true)
                .describe("When true (default), error if two entries share a cite key (case-insensitive) — BibTeX silently keeps only the first, so a duplicate key is a real bug. Set false to format anyway."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct BibtexFormat;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/bibtex-format",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "BibTeX formatter",
    skill(
        description = "Validate, sort, and pretty-print BibTeX bibliography entries. Pass the raw .bib source in 'bibtex'; the tool parses @article/@book/etc. entries plus @string/@preamble/@comment, reports any syntax error (unbalanced braces, missing '='), then re-emits them cleanly with one field per indented line. Set sort='key' or 'type-key' to reorder entries, sort_fields=true to alphabetize fields within each entry, indent to control field indentation (0-16, default 2), align_values=true to line up the '=' signs, lowercase_type=false to keep the original @TYPE casing (field names are always lowercased), and check_duplicates=false to allow duplicate cite keys (by default a repeated key is an error). Field values keep their original {braces}/\"quotes\"/macro # concatenation.",
        parameters = schema_json()
    ),
)]
impl BibtexFormat {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "bibtex-format", |a: Args| {
            gizza_ai_bibtex_format_core::format(
                &a.bibtex,
                &a.sort,
                a.sort_fields,
                a.indent,
                a.align_values,
                a.lowercase_type,
                a.check_duplicates,
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
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "bibtex": { "type": "string", "description": "The BibTeX bibliography source to validate and pretty-print." },
                    "sort": { "type": "string", "enum": ["none", "key", "type-key"], "default": "none", "description": "Reorder entries: 'none' (default) keeps source order; 'key' sorts case-insensitively by cite key; 'type-key' sorts by entry type then key. @string/@preamble/@comment items stay at the front." },
                    "sort_fields": { "type": "boolean", "default": false, "description": "When true, sort the fields within each entry alphabetically by name. Default false (keep source order)." },
                    "indent": { "type": "integer", "minimum": 0, "maximum": 16, "default": 2, "description": "Spaces to indent each field line, 0-16. Default 2." },
                    "align_values": { "type": "boolean", "default": false, "description": "When true, pad field names so all '=' signs in an entry line up. Default false (a single space before '=')." },
                    "lowercase_type": { "type": "boolean", "default": true, "description": "When true (default), lowercase the entry type and the @string/@preamble/@comment keyword. Field names are always lowercased." },
                    "check_duplicates": { "type": "boolean", "default": true, "description": "When true (default), error if two entries share a cite key (case-insensitive) — BibTeX silently keeps only the first, so a duplicate key is a real bug. Set false to format anyway." }
                },
                "required": ["bibtex"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}

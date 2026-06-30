//! gizza-ai/latex-table — convert CSV or TSV data into a LaTeX table. Thin
//! wrapper; chat schema single-sourced from descriptor(); handler delegates to
//! run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_latex_table_core::{to_latex, Options, Style};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    delimiter: String,
    #[serde(default = "default_style")]
    style: String,
    #[serde(default = "default_alignment")]
    alignment: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default = "default_true")]
    escape: bool,
    #[serde(default)]
    bold_header: bool,
    #[serde(default)]
    caption: String,
    #[serde(default)]
    label: String,
}
fn default_style() -> String {
    "booktabs".to_string()
}
fn default_alignment() -> String {
    "l".to_string()
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The CSV or TSV text to convert."))
        .param(
            Param::string("delimiter")
                .default("auto")
                .describe("Field separator: 'auto' (detect CSV vs TSV), a single char, or comma/tab/semicolon/pipe. Default 'auto'."),
        )
        .param(
            Param::enumv("style", ["booktabs", "grid", "plain"]).default("booktabs").describe(
                "Rule style: booktabs (\\toprule/\\midrule/\\bottomrule, needs \\usepackage{booktabs}), grid (vertical bars + \\hline), or plain (no rules).",
            ),
        )
        .param(
            Param::string("alignment")
                .default("l")
                .describe("Column alignment: a single l/c/r applied to all columns, or a per-column string like 'lcr'."),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("Treat the first row as the header (default true); a \\midrule/\\hline separates it from the body."),
        )
        .param(
            Param::boolean("escape")
                .default(true)
                .describe("Escape LaTeX special characters (& % $ # _ { } ~ ^ \\) in cells (default true). Turn off to keep raw LaTeX."),
        )
        .param(
            Param::boolean("bold_header")
                .default(false)
                .describe("Wrap header cells in \\textbf{} (default false)."),
        )
        .param(
            Param::string("caption")
                .default("")
                .describe("Optional caption; if set (or label is), the tabular is wrapped in a centered table float."),
        )
        .param(
            Param::string("label")
                .default("")
                .describe("Optional \\label for cross-references; if set (or caption is), the tabular is wrapped in a table float."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct LatexTable;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/latex-table",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert CSV or TSV data into a LaTeX tabular",
    skill(
        description = "Convert CSV or TSV data into a LaTeX tabular environment. style=booktabs (default) uses \\toprule/\\midrule/\\bottomrule (needs \\usepackage{booktabs}); style=grid uses vertical bars and \\hline; style=plain has no rules. alignment is a single l/c/r for all columns or a per-column string like 'lcr'. header=true (default) separates the first row with a rule; escape=true (default) escapes LaTeX special characters; bold_header wraps header cells in \\textbf{}. Set caption and/or label to wrap the tabular in a centered table float. delimiter is 'auto' (detect), a single char, or comma/tab/semicolon/pipe. Runs locally.",
        parameters = schema_json()
    ),
)]
impl LatexTable {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "latex-table", |a: Args| {
            let style = Style::parse(&a.style).map_err(SkillError::InvalidArgs)?;
            let opts = Options {
                style,
                has_header: a.header,
                alignment: &a.alignment,
                escape: a.escape,
                bold_header: a.bold_header,
                caption: &a.caption,
                label: &a.label,
            };
            to_latex(&a.data, &a.delimiter, &opts).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
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
                    "data": { "type": "string", "description": "The CSV or TSV text to convert." },
                    "delimiter": { "type": "string", "default": "auto", "description": "Field separator: 'auto' (detect CSV vs TSV), a single char, or comma/tab/semicolon/pipe. Default 'auto'." },
                    "style": { "type": "string", "enum": ["booktabs", "grid", "plain"], "default": "booktabs", "description": "Rule style: booktabs (\\toprule/\\midrule/\\bottomrule, needs \\usepackage{booktabs}), grid (vertical bars + \\hline), or plain (no rules)." },
                    "alignment": { "type": "string", "default": "l", "description": "Column alignment: a single l/c/r applied to all columns, or a per-column string like 'lcr'." },
                    "header": { "type": "boolean", "default": true, "description": "Treat the first row as the header (default true); a \\midrule/\\hline separates it from the body." },
                    "escape": { "type": "boolean", "default": true, "description": "Escape LaTeX special characters (& % $ # _ { } ~ ^ \\) in cells (default true). Turn off to keep raw LaTeX." },
                    "bold_header": { "type": "boolean", "default": false, "description": "Wrap header cells in \\textbf{} (default false)." },
                    "caption": { "type": "string", "default": "", "description": "Optional caption; if set (or label is), the tabular is wrapped in a centered table float." },
                    "label": { "type": "string", "default": "", "description": "Optional \\label for cross-references; if set (or caption is), the tabular is wrapped in a table float." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}

//! gizza-ai/mail-merge — chat skill block on the shared tool abstraction.
//! Fills a text/markdown template once per CSV row (classic mail merge). The
//! chat schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill. Pure — runs entirely inside the
//! WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    template: String,
    csv: String,
    #[serde(default)]
    syntax: String,
    #[serde(default)]
    delimiter: String,
    #[serde(default)]
    on_missing: String,
    /// Match placeholder names to headers case-insensitively (default true).
    #[serde(default = "default_true")]
    case_insensitive: bool,
    #[serde(default)]
    separator: String,
}

fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("template")
                .required()
                .describe("The template text, with {{Column}} placeholders that name CSV header columns, e.g. \"Hi {{name}}, your invoice for ${{amount}} is due {{due}}.\"."),
        )
        .param(
            Param::string("csv")
                .required()
                .describe("CSV data. The first row is the header (column names); every following row renders one output. Quoted fields may contain the delimiter and newlines, e.g. \"name,amount\\nAlice,10\\nBob,20\"."),
        )
        .param(
            Param::enumv("syntax", ["double_curly", "single_curly", "double_angle"])
                .default("double_curly")
                .describe("Placeholder style: 'double_curly' {{col}} (default), 'single_curly' {col}, or 'double_angle' <<col>>."),
        )
        .param(
            Param::enumv("delimiter", ["comma", "semicolon", "tab"])
                .default("comma")
                .describe("CSV field delimiter: 'comma' (default), 'semicolon' (common in European exports), or 'tab' (TSV)."),
        )
        .param(
            Param::enumv("on_missing", ["empty", "keep", "error"])
                .default("empty")
                .describe("What to do when a placeholder names a column not in the header: 'empty' (default) replaces it with nothing, 'keep' leaves the {{placeholder}} text, 'error' fails the merge. A column that exists but is blank for a row always renders empty."),
        )
        .param(
            Param::boolean("case_insensitive")
                .default(true)
                .describe("When true (default), {{First Name}} matches a header column named 'first name'. When false, names must match exactly."),
        )
        .param(
            Param::enumv("separator", ["divider", "blank_line", "newline", "form_feed", "none"])
                .default("divider")
                .describe("Text inserted between each rendered row: 'divider' a --- rule (default), 'blank_line', 'newline', 'form_feed' (page break), or 'none' (concatenate)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/mail-merge",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Fill a text template once per CSV row (mail merge)",
    skill(
        description = "Mail merge: fill a text or markdown template once per CSV row and return the combined output. Each {{Column}} placeholder is replaced by that row's value for the matching CSV header column; the per-row results are joined by a separator. Set syntax to choose the placeholder style ({{col}}, {col}, <<col>>), delimiter for the CSV field separator (comma/semicolon/tab), on_missing for how to handle a placeholder whose column is absent (empty/keep/error), case_insensitive to match names loosely, and separator for what goes between documents. Plain named substitution only — no loops or conditionals (use render-template for those).",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "mail-merge", |a: Args| {
            gizza_ai_mail_merge_core::merge(
                &a.template,
                &a.csv,
                &a.syntax,
                &a.delimiter,
                &a.on_missing,
                a.case_insensitive,
                &a.separator,
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
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "template": { "type": "string", "description": "The template text, with {{Column}} placeholders that name CSV header columns, e.g. \"Hi {{name}}, your invoice for ${{amount}} is due {{due}}.\"." },
                    "csv": { "type": "string", "description": "CSV data. The first row is the header (column names); every following row renders one output. Quoted fields may contain the delimiter and newlines, e.g. \"name,amount\\nAlice,10\\nBob,20\"." },
                    "syntax": { "type": "string", "enum": ["double_curly", "single_curly", "double_angle"], "default": "double_curly", "description": "Placeholder style: 'double_curly' {{col}} (default), 'single_curly' {col}, or 'double_angle' <<col>>." },
                    "delimiter": { "type": "string", "enum": ["comma", "semicolon", "tab"], "default": "comma", "description": "CSV field delimiter: 'comma' (default), 'semicolon' (common in European exports), or 'tab' (TSV)." },
                    "on_missing": { "type": "string", "enum": ["empty", "keep", "error"], "default": "empty", "description": "What to do when a placeholder names a column not in the header: 'empty' (default) replaces it with nothing, 'keep' leaves the {{placeholder}} text, 'error' fails the merge. A column that exists but is blank for a row always renders empty." },
                    "case_insensitive": { "type": "boolean", "default": true, "description": "When true (default), {{First Name}} matches a header column named 'first name'. When false, names must match exactly." },
                    "separator": { "type": "string", "enum": ["divider", "blank_line", "newline", "form_feed", "none"], "default": "divider", "description": "Text inserted between each rendered row: 'divider' a --- rule (default), 'blank_line', 'newline', 'form_feed' (page break), or 'none' (concatenate)." }
                },
                "required": ["template", "csv"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}

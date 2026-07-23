//! gizza-ai/log-merger — chat skill block on the shared tool abstraction.
//! Interleaves multiple pasted log sources into one timeline, sorted by each
//! line's parsed timestamp and tagged with its `[source]`. The chat schema is
//! single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    logs: String,
    #[serde(default = "default_source_mode")]
    source_mode: String,
    #[serde(default = "default_order")]
    order: String,
    #[serde(default)]
    dedupe: bool,
    #[serde(default = "default_align")]
    align: bool,
}

fn default_source_mode() -> String {
    "header".to_string()
}
fn default_order() -> String {
    "asc".to_string()
}
fn default_align() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("logs")
                .required()
                .describe("The concatenated log sources to interleave. Separate sources with header lines (`--- app.log ---`, `=== name ===`, GNU tail `==> name <==`, or markdown `# name`), or use source_mode=blank to split on blank lines."),
        )
        .param(
            Param::enumv("source_mode", ["header", "blank"])
                .default("header")
                .describe("How sources are split: header (delimit by header lines like `--- app.log ---`) or blank (split on blank lines into source1, source2, …)."),
        )
        .param(
            Param::enumv("order", ["asc", "desc"])
                .default("asc")
                .describe("Sort direction of the merged timeline: asc (oldest first) or desc (newest first)."),
        )
        .param(
            Param::boolean("dedupe")
                .default(false)
                .describe("When true, drop later lines that repeat an already-emitted (timestamp, text) pair — useful for overlapping captures."),
        )
        .param(
            Param::boolean("align")
                .default(true)
                .describe("When true, pad each [source] tag to a common width so messages line up in a column."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/log-merger",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Merge multiple logs into one timeline sorted by timestamp",
    skill(
        description = "Interleave multiple pasted log sources into one unified timeline, sorted by each line's parsed timestamp, with every line prefixed by a [source] tag. Sources inside the paste are delimited by header lines (`--- app.log ---`, `=== name ===`, GNU tail `==> name <==`, or markdown `# name`); set source_mode=blank to split on blank lines into source1, source2, … instead. Timestamps are auto-detected anywhere in a line (ISO 8601/RFC 3339, `YYYY-MM-DD HH:MM:SS`, syslog `Mon DD HH:MM:SS`, Apache `10/Oct/2000:13:55:36 -0700`, unix epoch seconds/ms); untimestamped lines inherit the previous line's timestamp so stack traces stay attached. order is asc (oldest first, default) or desc. dedupe drops repeated (timestamp, text) lines; align pads the [source] tags to a common width (default on). Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "log-merger", |a: Args| {
            gizza_ai_log_merger_core::merge(&a.logs, &a.source_mode, &a.order, a.dedupe, a.align)
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
                    "logs": { "type": "string", "description": "The concatenated log sources to interleave. Separate sources with header lines (`--- app.log ---`, `=== name ===`, GNU tail `==> name <==`, or markdown `# name`), or use source_mode=blank to split on blank lines." },
                    "source_mode": { "type": "string", "enum": ["header", "blank"], "default": "header", "description": "How sources are split: header (delimit by header lines like `--- app.log ---`) or blank (split on blank lines into source1, source2, …)." },
                    "order": { "type": "string", "enum": ["asc", "desc"], "default": "asc", "description": "Sort direction of the merged timeline: asc (oldest first) or desc (newest first)." },
                    "dedupe": { "type": "boolean", "default": false, "description": "When true, drop later lines that repeat an already-emitted (timestamp, text) pair — useful for overlapping captures." },
                    "align": { "type": "boolean", "default": true, "description": "When true, pad each [source] tag to a common width so messages line up in a column." }
                },
                "required": ["logs"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}

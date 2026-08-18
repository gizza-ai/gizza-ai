//! gizza-ai/ssh-config-formatter — chat skill block on the shared tool abstraction.
//! Parses, lints and pretty-prints an OpenSSH client config (`~/.ssh/config`).
//! Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default)]
    output: String,
    #[serde(default = "default_indent")]
    indent: i64,
    #[serde(default)]
    keyword_case: String,
    #[serde(default)]
    align_values: bool,
    #[serde(default)]
    sort_keywords: bool,
    #[serde(default)]
    dedupe: bool,
    #[serde(default = "default_true")]
    include_notes: bool,
    #[serde(default)]
    min_severity: String,
}

fn default_indent() -> i64 {
    2
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The OpenSSH client configuration to format, e.g. the contents of ~/.ssh/config or /etc/ssh/ssh_config. Host and Match blocks, comments, and `Keyword=Value` lines are all accepted. Maximum 10000 lines."),
        )
        .param(
            Param::enumv("output", ["formatted", "report", "json", "hosts"])
                .default("formatted")
                .describe("Output shape: formatted (normalized config text, the default), report (readable lint findings plus counts), json (hosts, blocks, issues, stats and the formatted text), or hosts (one Host pattern per line)."),
        )
        .param(
            Param::integer("indent")
                .default(2)
                .min(0.0)
                .max(8.0)
                .describe("Spaces used to indent directives under a Host or Match header, 0-8. Default 2. Directives before the first Host block are never indented."),
        )
        .param(
            Param::enumv("keyword_case", ["canonical", "lower", "preserve"])
                .default("canonical")
                .describe("Keyword spelling in the formatted output: canonical (OpenSSH manual spelling such as HostName and IdentityFile, the default), lower (all lowercase), or preserve (leave each keyword exactly as written). SSH matches keywords case-insensitively either way."),
        )
        .param(
            Param::boolean("align_values")
                .default(false)
                .describe("Pad each keyword so the values line up in a column within its block. Default false."),
        )
        .param(
            Param::boolean("sort_keywords")
                .default(false)
                .describe("Sort the directives inside each block alphabetically by keyword. Comments stay attached to the directive below them. Default false, which preserves the written order."),
        )
        .param(
            Param::boolean("dedupe")
                .default(false)
                .describe("Delete repeated keywords inside a block that SSH already ignores (it keeps the first value). Keywords that may legitimately repeat, such as IdentityFile, LocalForward and SendEnv, are never removed. Default false."),
        )
        .param(
            Param::boolean("include_notes")
                .default(true)
                .describe("Append the lint findings as `#` comment lines under the formatted config. Default true; set false for a clean copy-paste config. Only affects the formatted output."),
        )
        .param(
            Param::enumv("min_severity", ["info", "warning", "error"])
                .default("info")
                .describe("Lowest severity to report: info (everything, the default), warning (skip advisory notes), or error (only findings SSH itself rejects, such as bad values or a missing value)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/ssh-config-formatter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Format, normalize and lint an OpenSSH ~/.ssh/config file.",
    skill(
        description = "Parse an OpenSSH client configuration (~/.ssh/config), normalize it (canonical keyword spelling, consistent indent, optional value alignment, alphabetical keyword order, removal of ignored duplicate keywords) and lint it. Flags duplicate Host patterns, blocks shadowed by an earlier pattern, a wildcard Host block that is not last, unknown, deprecated and sshd_config-only keywords, missing values, out-of-range ports and invalid yes/no or fixed-choice values. Output as the formatted config, a readable report, structured JSON, or a plain list of host aliases.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "ssh-config-formatter", |a: Args| {
            gizza_ai_ssh_config_formatter_core::run(
                &a.text,
                &a.output,
                a.indent,
                &a.keyword_case,
                a.align_values,
                a.sort_keywords,
                a.dedupe,
                a.include_notes,
                &a.min_severity,
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

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "text":          { "type": "string", "description": "The OpenSSH client configuration to format, e.g. the contents of ~/.ssh/config or /etc/ssh/ssh_config. Host and Match blocks, comments, and `Keyword=Value` lines are all accepted. Maximum 10000 lines." },
                    "output":        { "type": "string", "enum": ["formatted", "report", "json", "hosts"], "default": "formatted", "description": "Output shape: formatted (normalized config text, the default), report (readable lint findings plus counts), json (hosts, blocks, issues, stats and the formatted text), or hosts (one Host pattern per line)." },
                    "indent":        { "type": "integer", "default": 2, "minimum": 0, "maximum": 8, "description": "Spaces used to indent directives under a Host or Match header, 0-8. Default 2. Directives before the first Host block are never indented." },
                    "keyword_case":  { "type": "string", "enum": ["canonical", "lower", "preserve"], "default": "canonical", "description": "Keyword spelling in the formatted output: canonical (OpenSSH manual spelling such as HostName and IdentityFile, the default), lower (all lowercase), or preserve (leave each keyword exactly as written). SSH matches keywords case-insensitively either way." },
                    "align_values":  { "type": "boolean", "default": false, "description": "Pad each keyword so the values line up in a column within its block. Default false." },
                    "sort_keywords": { "type": "boolean", "default": false, "description": "Sort the directives inside each block alphabetically by keyword. Comments stay attached to the directive below them. Default false, which preserves the written order." },
                    "dedupe":        { "type": "boolean", "default": false, "description": "Delete repeated keywords inside a block that SSH already ignores (it keeps the first value). Keywords that may legitimately repeat, such as IdentityFile, LocalForward and SendEnv, are never removed. Default false." },
                    "include_notes": { "type": "boolean", "default": true, "description": "Append the lint findings as `#` comment lines under the formatted config. Default true; set false for a clean copy-paste config. Only affects the formatted output." },
                    "min_severity":  { "type": "string", "enum": ["info", "warning", "error"], "default": "info", "description": "Lowest severity to report: info (everything, the default), warning (skip advisory notes), or error (only findings SSH itself rejects, such as bad values or a missing value)." }
                },
                "required": ["text"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}

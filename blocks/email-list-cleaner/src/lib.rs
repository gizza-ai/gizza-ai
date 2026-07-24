//! gizza-ai/email-list-cleaner — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    emails: String,
    #[serde(default)]
    canonicalize: bool,
    #[serde(default = "default_sort")]
    sort: String,
    #[serde(default = "default_format")]
    format: String,
}

fn default_sort() -> String {
    "input".to_string()
}
fn default_format() -> String {
    "report".to_string()
}

/// Single source for the chat schema (and CLI). Param names also drive the
/// generated page manifest and query-string contract.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("emails").required().multiline().describe("Email address list to clean. Paste one address per line, or separate entries with commas or semicolons. Display-name wrappers and mailto: prefixes are accepted."))
        .param(Param::boolean("canonicalize").default(false).describe("When true, apply provider canonicalization before de-duplicating (for example, Gmail dot removal and +tag folding). Leave false to only trim and lowercase."))
        .param(Param::enumv("sort", ["input", "alpha"]).default("input").describe("Output order: 'input' preserves first-seen order, while 'alpha' sorts the cleaned unique addresses alphabetically."))
        .param(Param::enumv("format", ["report", "clean", "comma"]).default("report").describe("Output format: 'report' includes counts, invalid rows, and typo suggestions; 'clean' returns one address per line; 'comma' returns a comma-separated list."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/email-list-cleaner",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Clean, validate, and de-duplicate pasted email address lists",
    skill(
        description = "Clean a pasted email address list: split multiline/comma/semicolon entries, trim and lowercase addresses, validate syntax, remove duplicates, optionally fold provider aliases, and report invalid rows plus likely typo suggestions.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "email-list-cleaner", |a: Args| {
            let sort_alpha = match a.sort.trim().to_ascii_lowercase().as_str() {
                "" | "input" => false,
                "alpha" => true,
                other => {
                    return Err(SkillError::InvalidArgs(format!(
                        "invalid sort {other:?}: expected 'input' or 'alpha'"
                    )))
                }
            };
            gizza_ai_email_list_cleaner_core::report(
                &a.emails,
                a.canonicalize,
                sort_alpha,
                &a.format,
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
    use serde_json::json;

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let actual: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let expected = json!({
            "type": "object",
            "required": ["emails"],
            "properties": {
                "canonicalize": {
                    "default": false,
                    "description": "When true, apply provider canonicalization before de-duplicating (for example, Gmail dot removal and +tag folding). Leave false to only trim and lowercase.",
                    "type": "boolean"
                },
                "emails": {
                    "description": "Email address list to clean. Paste one address per line, or separate entries with commas or semicolons. Display-name wrappers and mailto: prefixes are accepted.",
                    "type": "string"
                },
                "format": {
                    "default": "report",
                    "description": "Output format: 'report' includes counts, invalid rows, and typo suggestions; 'clean' returns one address per line; 'comma' returns a comma-separated list.",
                    "enum": ["report", "clean", "comma"],
                    "type": "string"
                },
                "sort": {
                    "default": "input",
                    "description": "Output order: 'input' preserves first-seen order, while 'alpha' sorts the cleaned unique addresses alphabetically.",
                    "enum": ["input", "alpha"],
                    "type": "string"
                }
            },
            "additionalProperties": false
        });
        assert_eq!(actual, expected);
    }
}

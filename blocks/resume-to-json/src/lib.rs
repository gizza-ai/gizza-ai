//! gizza-ai/resume-to-json — extract a pasted plain-text resume into the
//! standard JSON Resume schema (jsonresume.org v1.0.0), or validate a
//! resume.json document against it. Thin wrapper; chat schema single-sourced
//! from descriptor(); handler delegates to run_skill. Pure → all backends
//! (chat, CLI, web page).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_resume_to_json_core::{run, Mode};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_auto")]
    mode: String,
    #[serde(default)]
    schema_ref: bool,
    #[serde(default = "default_true")]
    pretty: bool,
}
fn default_auto() -> String {
    "auto".into()
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The resume as plain text (name and contact lines first, then sections like Experience, Education, Skills) — or an existing resume.json document to validate. Max 1 MiB."),
        )
        .param(
            Param::enumv("mode", ["auto", "extract", "validate"])
                .default("auto")
                .describe("'extract' parses plain resume text into a JSON Resume (v1.0.0) document; 'validate' checks a pasted resume.json against the schema and returns {valid, errors, warnings, summary}; 'auto' (default) picks validate when the input is a JSON object, extract otherwise."),
        )
        .param(
            Param::boolean("schema_ref")
                .default(false)
                .describe("Add the official $schema URL and a meta.version field to the extracted document (like the published JSON Resume samples). Only affects extraction; validation reports are unchanged. Default false."),
        )
        .param(
            Param::boolean("pretty")
                .default(true)
                .describe("Pretty-print (indent) the JSON output instead of emitting it on one line. Default true."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ResumeToJson;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/resume-to-json",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a pasted resume to JSON Resume format and validate it",
    skill(
        description = "Extract a pasted plain-text resume into the standard JSON Resume schema (jsonresume.org v1.0.0) — canonical sections (basics, work, education, skills, projects, languages, …), ISO-8601 partial dates (YYYY, YYYY-MM, YYYY-MM-DD), LinkedIn/GitHub profile detection — or validate an existing resume.json document against the schema. `mode` is 'extract', 'validate', or 'auto' (default: JSON object input → validate, anything else → extract). Validation returns {valid, errors, warnings, summary}: type mismatches and bad date patterns are errors; malformed email/URL formats and unknown keys are warnings. `schema_ref` adds the $schema URL + meta.version to extracted output; `pretty` (default true) indents the JSON. Heuristic English-heading parser; runs locally.",
        parameters = schema_json()
    ),
)]
impl ResumeToJson {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "resume-to-json", |a: Args| {
            let mode = Mode::parse(&a.mode).map_err(SkillError::InvalidArgs)?;
            run(&a.data, mode, a.schema_ref, a.pretty).map_err(SkillError::InvalidArgs)
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
                    "data": { "type": "string", "description": "The resume as plain text (name and contact lines first, then sections like Experience, Education, Skills) — or an existing resume.json document to validate. Max 1 MiB." },
                    "mode": { "type": "string", "enum": ["auto", "extract", "validate"], "default": "auto", "description": "'extract' parses plain resume text into a JSON Resume (v1.0.0) document; 'validate' checks a pasted resume.json against the schema and returns {valid, errors, warnings, summary}; 'auto' (default) picks validate when the input is a JSON object, extract otherwise." },
                    "schema_ref": { "type": "boolean", "default": false, "description": "Add the official $schema URL and a meta.version field to the extracted document (like the published JSON Resume samples). Only affects extraction; validation reports are unchanged. Default false." },
                    "pretty": { "type": "boolean", "default": true, "description": "Pretty-print (indent) the JSON output instead of emitting it on one line. Default true." }
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

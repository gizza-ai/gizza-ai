//! gizza-ai/job-posting-parser — extract structured fields from a pasted job ad.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    posting: String,
    #[serde(default)]
    output: String,
    #[serde(default = "default_true")]
    include_evidence: bool,
}

fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("posting").required().describe("Raw job posting text to parse. Paste the title/header, company line, location, compensation, requirements, and skills sections when available."))
        .param(Param::enumv("output", ["json", "markdown"]).default("json").describe("Output format. JSON is best for downstream scripts; Markdown is easier to paste into notes."))
        .param(Param::boolean("include_evidence").default(true).describe("Include short evidence lines showing which source text produced key fields (default true)."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct JobPostingParser;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/job-posting-parser",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract title, company, salary, location, skills, and work mode from a pasted job ad.",
    skill(
        description = "Parse a pasted job posting into deterministic structured fields: title, company, location, salary or compensation range, employment type, remote/hybrid/onsite work mode, experience level, skills, warnings, and optional evidence snippets. This is a pure Rust heuristic parser for triage and spreadsheet cleanup: it does not call an LLM, scrape websites, infer hidden facts, or normalize against a private taxonomy. Provide the full job posting text in posting; choose json or markdown output; keep include_evidence enabled when you want to audit why a field was selected.",
        parameters = schema_json()
    ),
)]
impl JobPostingParser {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "job-posting-parser", |a: Args| {
            gizza_ai_job_posting_parser_core::parse_job_posting(
                &a.posting,
                &a.output,
                a.include_evidence,
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
        let authored: serde_json::Value = serde_json::from_str(r#"{
            "type":"object",
            "properties":{
                "posting":{"type":"string","description":"Raw job posting text to parse. Paste the title/header, company line, location, compensation, requirements, and skills sections when available."},
                "output":{"type":"string","enum":["json","markdown"],"default":"json","description":"Output format. JSON is best for downstream scripts; Markdown is easier to paste into notes."},
                "include_evidence":{"type":"boolean","default":true,"description":"Include short evidence lines showing which source text produced key fields (default true)."}
            },
            "required":["posting"],
            "additionalProperties":false
        }"#).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}

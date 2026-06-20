//! gizza-ai/resume-builder — structured details → ATS-friendly Markdown resume.
//!
//! Thin chat-skill wrapper around `gizza-ai-resume-builder-core`. Chat schema
//! single-sourced from `descriptor()`; handler delegates to `run_skill`. Pure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_resume_builder_core::build;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::string("data").required().describe(
            "A JSON object of resume fields. Recognized: name (required), title, email, phone, location, links[], summary, experience[{role,company,location,dates,bullets[]}], education[{degree,school,location,dates,details}], skills[], and sections[{heading,items[]}] for extras like Projects/Certifications.",
        ),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ResumeBuilder;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/resume-builder",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Build an ATS-friendly Markdown resume from JSON",
    skill(
        description = "Turn structured resume details (a JSON object) into a clean, ATS-friendly Markdown resume — plain headings and bullets, no tables/columns/graphics that resume parsers choke on. `data` is a JSON object: name (required), title, email, phone, location, links[], summary, experience[{role,company,location,dates,bullets[]}], education[{degree,school,location,dates,details}], skills[], and optional sections[{heading,items[]}].",
        parameters = schema_json()
    )
)]
impl ResumeBuilder {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "resume-builder", |a: Args| {
            build(&a.data).map_err(SkillError::InvalidArgs)
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
                    "data": { "type": "string", "description": "A JSON object of resume fields. Recognized: name (required), title, email, phone, location, links[], summary, experience[{role,company,location,dates,bullets[]}], education[{degree,school,location,dates,details}], skills[], and sections[{heading,items[]}] for extras like Projects/Certifications." }
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

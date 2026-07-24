//! gizza-ai/hl7-v2-parse — parse a pipe-delimited HL7 v2.x message into named
//! segments, fields, components and subcomponents (MSH, PID, OBX, …) and render
//! as structured JSON or a flat CSV leaf table. Thin wrapper around the core; the
//! chat schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill. Pure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_hl7_v2_parse_core::run;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    output: String,
    #[serde(default = "default_true")]
    include_descriptions: bool,
    #[serde(default = "default_true")]
    unescape: bool,
}
fn default_true() -> bool { true }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The raw HL7 v2.x message (pipe-delimited segments — MSH, PID, OBX, …). Segments may be separated by carriage returns or newlines; the field/component/repetition/subcomponent delimiters are read from the MSH segment (default '|', '^', '~', '&')."))
        .param(Param::enumv("output", ["json", "csv"]).default("json").describe("Output format: 'json' for the full nested hierarchy (segments → fields → components → subcomponents, with repetitions) or 'csv' for a flat 'Segment, Location, Value' leaf table. Default 'json'."))
        .param(Param::boolean("include_descriptions").default(true).describe("When true, attach human-readable names to each segment (e.g. PID → Patient Identification) and to fields of the common segments (MSH, PID, OBX, PV1, OBR, …). Default true."))
        .param(Param::boolean("unescape").default(true).describe("When true, decode HL7 escape sequences (\\F\\ → field sep, \\S\\ → component, \\T\\ → subcomponent, \\R\\ → repetition, \\E\\ → escape, \\Xhh\\ → hex bytes, \\.br\\ → line break) into their literal characters. When false, leaf values are kept verbatim. Default true."))
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct Hl7V2Parse;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/hl7-v2-parse",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse a pipe-delimited HL7 v2.x message into named segments, fields and components as JSON or CSV",
    skill(
        description = "Parse a pipe-delimited HL7 v2.x message (MSH, PID, OBX, PV1, OBR, EVN, …) into a structured hierarchy of segments, fields, components, subcomponents and repetitions. Reads the field/component/repetition/subcomponent delimiters from the MSH segment (handling the MSH-1 field-separator / MSH-2 encoding-characters offset), decodes HL7 escape sequences (\\F\\ \\S\\ \\T\\ \\R\\ \\E\\ \\Xhh\\ \\.br\\), and attaches human-readable segment/field names for common segments. Output full nested JSON (default) or a flat 'Segment, Location, Value' CSV leaf table. Runs fully locally — no PHI leaves the browser.",
        parameters = schema_json()
    ),
)]
impl Hl7V2Parse {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "hl7-v2-parse", |a: Args| {
            run(&a.data, &a.output, a.include_descriptions, a.unescape)
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
                    "data":                 { "type": "string", "description": "The raw HL7 v2.x message (pipe-delimited segments — MSH, PID, OBX, …). Segments may be separated by carriage returns or newlines; the field/component/repetition/subcomponent delimiters are read from the MSH segment (default '|', '^', '~', '&')." },
                    "output":               { "type": "string", "enum": ["json", "csv"], "default": "json", "description": "Output format: 'json' for the full nested hierarchy (segments → fields → components → subcomponents, with repetitions) or 'csv' for a flat 'Segment, Location, Value' leaf table. Default 'json'." },
                    "include_descriptions": { "type": "boolean", "default": true, "description": "When true, attach human-readable names to each segment (e.g. PID → Patient Identification) and to fields of the common segments (MSH, PID, OBX, PV1, OBR, …). Default true." },
                    "unescape":             { "type": "boolean", "default": true, "description": "When true, decode HL7 escape sequences (\\F\\ → field sep, \\S\\ → component, \\T\\ → subcomponent, \\R\\ → repetition, \\E\\ → escape, \\Xhh\\ → hex bytes, \\.br\\ → line break) into their literal characters. When false, leaf values are kept verbatim. Default true." }
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

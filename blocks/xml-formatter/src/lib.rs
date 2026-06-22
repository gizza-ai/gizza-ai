//! gizza-ai/xml-formatter — pretty-print or minify XML with a well-formedness
//! check. Thin wrapper; chat schema single-sourced from descriptor(); handler
//! delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_xml_formatter_core::{format, Mode};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    xml: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "default_indent")]
    indent: u32,
}
fn default_mode() -> String {
    "pretty".to_string()
}
fn default_indent() -> u32 {
    2
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("xml").required().describe("The XML to format."))
        .param(
            Param::enumv("mode", ["pretty", "minify"]).default("pretty").describe(
                "pretty (default) indents the XML; minify strips insignificant whitespace.",
            ),
        )
        .param(
            Param::integer("indent")
                .min(0.0)
                .max(16.0)
                .describe("Spaces per indent level in pretty mode (default 2)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct XmlFormatter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/xml-formatter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Pretty-print or minify XML",
    skill(
        description = "Pretty-print or minify XML, checking it is well-formed. mode=pretty (default) re-indents the XML with `indent` spaces per level (default 2); mode=minify strips insignificant whitespace into one line. Returns a clear error with the byte position if the XML is malformed. Runs locally — the XML never leaves the device.",
        parameters = schema_json()
    ),
)]
impl XmlFormatter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "xml-formatter", |a: Args| {
            let mode = Mode::parse(&a.mode).map_err(SkillError::InvalidArgs)?;
            format(&a.xml, mode, a.indent as usize).map_err(SkillError::InvalidArgs)
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
                    "xml": { "type": "string", "description": "The XML to format." },
                    "mode": { "type": "string", "enum": ["pretty", "minify"], "default": "pretty", "description": "pretty (default) indents the XML; minify strips insignificant whitespace." },
                    "indent": { "type": "integer", "minimum": 0, "maximum": 16, "description": "Spaces per indent level in pretty mode (default 2)." }
                },
                "required": ["xml"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}

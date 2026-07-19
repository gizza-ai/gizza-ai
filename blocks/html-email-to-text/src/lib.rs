//! gizza-ai/html-email-to-text — chat skill block on the shared tool abstraction.
//! Converts an HTML email body to clean plain text. The chat schema is
//! single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to block_utils::run_skill. Pure — runs entirely in the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_html_email_to_text_core::convert;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    html: String,
    #[serde(default)]
    links: String,
    #[serde(default)]
    wrap: f64,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("html")
                .required()
                .describe("The HTML email body to convert to plain text. Paste the full <html> document or just the <body> fragment."),
        )
        .param(
            Param::enumv("links", ["text", "inline", "footnote"])
                .default("inline")
                .describe(
                    "How to render hyperlinks: 'text' keeps only the link text and drops the URL; \
'inline' (default) writes the text followed by the URL in parentheses, e.g. 'click here \
(https://example.com)'; 'footnote' numbers each link like 'click here[1]' and lists the URLs in a \
'[1] https://example.com' reference block at the bottom.",
                ),
        )
        .param(
            Param::integer("wrap")
                .default(0)
                .min(0.0)
                .max(200.0)
                .describe(
                    "Hard-wrap output lines to at most this many columns on word boundaries; 0 \
(default) disables wrapping. Use 72 for the classic plain-text-email width. Long words such as \
URLs are never split.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/html-email-to-text",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert an HTML email body to clean plain text.",
    skill(
        description = "Convert an HTML email body into clean, readable plain text — tags stripped, entities decoded, paragraphs and lists preserved. Pass the HTML as `html`. Control hyperlinks with `links`: 'text' (link text only), 'inline' (text + URL in parentheses, the default), or 'footnote' (numbered links with a reference list at the bottom). Set `wrap` to a column count (e.g. 72) to hard-wrap lines for a plain-text email, or 0 to leave lines unwrapped.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "html-email-to-text", |a: Args| {
            let wrap = if a.wrap.is_finite() && a.wrap > 0.0 {
                a.wrap as u32
            } else {
                0
            };
            convert(&a.html, &a.links, wrap).map_err(SkillError::InvalidArgs)
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
                    "html": {
                        "type": "string",
                        "description": "The HTML email body to convert to plain text. Paste the full <html> document or just the <body> fragment."
                    },
                    "links": {
                        "type": "string",
                        "enum": ["text", "inline", "footnote"],
                        "default": "inline",
                        "description": "How to render hyperlinks: 'text' keeps only the link text and drops the URL; 'inline' (default) writes the text followed by the URL in parentheses, e.g. 'click here (https://example.com)'; 'footnote' numbers each link like 'click here[1]' and lists the URLs in a '[1] https://example.com' reference block at the bottom."
                    },
                    "wrap": {
                        "type": "integer",
                        "default": 0,
                        "minimum": 0,
                        "maximum": 200,
                        "description": "Hard-wrap output lines to at most this many columns on word boundaries; 0 (default) disables wrapping. Use 72 for the classic plain-text-email width. Long words such as URLs are never split."
                    }
                },
                "required": ["html"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn block_info_serializes_for_wafer_validation() {
        let mut info = wafer_block::BlockInfo::new(
            "gizza-ai/html-email-to-text",
            "0.1.0",
            "handler@v1",
            "Convert an HTML email body to clean plain text.",
        );
        info = info.tool(wafer_block::types::SkillTool {
            description: "Convert an HTML email body into clean, readable plain text — tags stripped, entities decoded, paragraphs and lists preserved. Pass the HTML as `html`. Control hyperlinks with `links`: 'text' (link text only), 'inline' (text + URL in parentheses, the default), or 'footnote' (numbered links with a reference list at the bottom). Set `wrap` to a column count (e.g. 72) to hard-wrap lines for a plain-text email, or 0 to leave lines unwrapped.".to_string(),
            parameters: serde_json::from_str(&schema_json()).unwrap(),
        });
        serde_json::to_vec(&info).expect("wafer __wafer_info JSON serialization");
    }
}

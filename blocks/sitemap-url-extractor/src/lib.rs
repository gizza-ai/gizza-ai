//! gizza-ai/sitemap-url-extractor — parse an XML sitemap or sitemap index and
//! extract every URL (with its optional lastmod date). Chat schema single-sourced
//! from descriptor() (which also drives the CLI); handle() delegates to run_skill.
//! Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_sitemap_url_extractor_core::extract;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    xml: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(
        Param::string("xml")
            .required()
            .describe("The XML content of a sitemap (<urlset>) or sitemap index (<sitemapindex>)."),
    )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/sitemap-url-extractor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract every URL from an XML sitemap or sitemap index",
    skill(
        description = "Parse an XML sitemap (<urlset>) or sitemap index (<sitemapindex>) and extract every URL from its <loc> elements, each paired with the <lastmod> date when present. Detects which document kind it was given and returns the kind, the URL count, and the entries (loc + optional lastmod). Namespace prefixes are handled. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "sitemap-url-extractor", |a: Args| {
            extract(&a.xml).map_err(SkillError::InvalidArgs)
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
                    "xml": { "type": "string", "description": "The XML content of a sitemap (<urlset>) or sitemap index (<sitemapindex>)." }
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

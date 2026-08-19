//! gizza-ai/email-tracker-pixel-detector — chat skill block on the shared tool abstraction.
//! Detects remote email images, tracking pixels, known tracker hosts, and optional click trackers.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default)]
    format: String,
    #[serde(default)]
    report: String,
    #[serde(default)]
    include_links: bool,
    #[serde(default)]
    vendors: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("text").required().describe("Raw email source, HTML email markup, or a pasted message body to inspect for remote images and open-tracking beacons."))
        .param(Param::enumv("format", ["auto", "html", "raw"]).default("auto").describe("Input interpretation: auto (default), html for markup fragments, or raw for exported .eml/source text. Raw scanning is local and does not fetch remote content."))
        .param(Param::enumv("report", ["summary", "json", "hosts"]).default("summary").describe("Output shape: summary (human-readable verdict and findings), json (structured assets/signals), or hosts (one remote host per line for a blocklist)."))
        .param(Param::boolean("include_links").default(false).describe("Also inspect remote links for click-tracking redirect domains and tracking paths. Default false so the primary verdict focuses on open-tracking images."))
        .param(Param::string("vendors").describe("Optional extra tracker domains or hosts to flag, separated by commas, spaces, or new lines. Useful for private ESP/CDN domains."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/email-tracker-pixel-detector",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Detect tracking pixels and remote email images in pasted email source.",
    skill(
        description = "Inspect raw email source or HTML email markup for remote images, tiny or hidden open pixels, known email-tracking vendor domains, tracking/open/beacon URL patterns, CSS background-image beacons, prefetch/preload assets, and optional click-tracking links. The tool performs no network requests; it reports what would be contacted when the message is opened.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "email-tracker-pixel-detector", |a: Args| {
            gizza_ai_email_tracker_pixel_detector_core::run(
                &a.text,
                &a.format,
                &a.report,
                a.include_links,
                &a.vendors,
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
                    "text":          { "type": "string", "description": "Raw email source, HTML email markup, or a pasted message body to inspect for remote images and open-tracking beacons." },
                    "format":        { "type": "string", "enum": ["auto", "html", "raw"], "default": "auto", "description": "Input interpretation: auto (default), html for markup fragments, or raw for exported .eml/source text. Raw scanning is local and does not fetch remote content." },
                    "report":        { "type": "string", "enum": ["summary", "json", "hosts"], "default": "summary", "description": "Output shape: summary (human-readable verdict and findings), json (structured assets/signals), or hosts (one remote host per line for a blocklist)." },
                    "include_links": { "type": "boolean", "default": false, "description": "Also inspect remote links for click-tracking redirect domains and tracking paths. Default false so the primary verdict focuses on open-tracking images." },
                    "vendors":       { "type": "string", "description": "Optional extra tracker domains or hosts to flag, separated by commas, spaces, or new lines. Useful for private ESP/CDN domains." }
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

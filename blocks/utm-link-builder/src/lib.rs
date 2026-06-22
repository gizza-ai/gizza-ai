//! gizza-ai/utm-link-builder — append UTM campaign parameters to a URL.
//! Chat schema single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_utm_link_builder_core::{build, Utm};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    url: String,
    source: String,
    medium: String,
    campaign: String,
    #[serde(default)]
    term: String,
    #[serde(default)]
    content: String,
    #[serde(default)]
    id: String,
    #[serde(default)]
    source_platform: String,
    #[serde(default)]
    creative_format: String,
    #[serde(default)]
    marketing_tactic: String,
    #[serde(default)]
    lowercase: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("url")
                .required()
                .describe("The destination URL to tag (e.g. https://example.com/landing). If no scheme is given, https:// is assumed. Existing query parameters and the fragment are preserved; any existing utm_* parameters are replaced."),
        )
        .param(
            Param::string("source")
                .required()
                .describe("utm_source — where the traffic comes from (e.g. google, newsletter, twitter)."),
        )
        .param(
            Param::string("medium")
                .required()
                .describe("utm_medium — the marketing medium (e.g. cpc, email, social, banner)."),
        )
        .param(
            Param::string("campaign")
                .required()
                .describe("utm_campaign — the campaign name (e.g. spring_sale, product_launch)."),
        )
        .param(
            Param::string("term")
                .describe("utm_term — optional paid-search keyword for the campaign. Omitted when blank."),
        )
        .param(
            Param::string("content")
                .describe("utm_content — optional identifier to A/B test or differentiate ads/links (e.g. logolink, textlink). Omitted when blank."),
        )
        .param(
            Param::string("id")
                .describe("utm_id — optional GA4 Campaign ID used to join cost/ads data to a campaign. Omitted when blank."),
        )
        .param(
            Param::string("source_platform")
                .describe("utm_source_platform — optional GA4 field naming the platform that directed traffic (e.g. 'Google Ads', 'Manual'). Omitted when blank."),
        )
        .param(
            Param::string("creative_format")
                .describe("utm_creative_format — optional GA4 field for the creative type (e.g. display, video, search). Omitted when blank."),
        )
        .param(
            Param::string("marketing_tactic")
                .describe("utm_marketing_tactic — optional GA4 field for the targeting criteria (e.g. remarketing, prospecting). Omitted when blank."),
        )
        .param(
            Param::boolean("lowercase")
                .default(false)
                .describe("When true, lowercase the parameter values so 'Email' and 'email' don't split a campaign in analytics. Default false."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/utm-link-builder",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Append UTM campaign parameters to a URL",
    skill(
        description = "Build a campaign-tracking URL by appending UTM query parameters. Required: url, source (utm_source), medium (utm_medium), campaign (utm_campaign). Optional: term (utm_term), content (utm_content), and the GA4 fields id (utm_id Campaign ID), source_platform (utm_source_platform), creative_format (utm_creative_format) and marketing_tactic (utm_marketing_tactic) — each omitted when blank. Values are application/x-www-form-urlencoded (spaces become '+', reserved chars percent-encoded). A url with no scheme gets https://; existing non-UTM query parameters and the fragment are preserved, and any existing utm_* parameters are replaced (so re-tagging is idempotent). Set lowercase=true to normalize the parameter values. Returns the full tagged url, the applied query string, and the list of params. Runs locally; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "utm-link-builder", |a: Args| {
            build(
                &Utm {
                    url: &a.url,
                    source: &a.source,
                    medium: &a.medium,
                    campaign: &a.campaign,
                    term: &a.term,
                    content: &a.content,
                    id: &a.id,
                    source_platform: &a.source_platform,
                    creative_format: &a.creative_format,
                    marketing_tactic: &a.marketing_tactic,
                },
                a.lowercase,
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
                    "url": { "type": "string", "description": "The destination URL to tag (e.g. https://example.com/landing). If no scheme is given, https:// is assumed. Existing query parameters and the fragment are preserved; any existing utm_* parameters are replaced." },
                    "source": { "type": "string", "description": "utm_source — where the traffic comes from (e.g. google, newsletter, twitter)." },
                    "medium": { "type": "string", "description": "utm_medium — the marketing medium (e.g. cpc, email, social, banner)." },
                    "campaign": { "type": "string", "description": "utm_campaign — the campaign name (e.g. spring_sale, product_launch)." },
                    "term": { "type": "string", "description": "utm_term — optional paid-search keyword for the campaign. Omitted when blank." },
                    "content": { "type": "string", "description": "utm_content — optional identifier to A/B test or differentiate ads/links (e.g. logolink, textlink). Omitted when blank." },
                    "id": { "type": "string", "description": "utm_id — optional GA4 Campaign ID used to join cost/ads data to a campaign. Omitted when blank." },
                    "source_platform": { "type": "string", "description": "utm_source_platform — optional GA4 field naming the platform that directed traffic (e.g. 'Google Ads', 'Manual'). Omitted when blank." },
                    "creative_format": { "type": "string", "description": "utm_creative_format — optional GA4 field for the creative type (e.g. display, video, search). Omitted when blank." },
                    "marketing_tactic": { "type": "string", "description": "utm_marketing_tactic — optional GA4 field for the targeting criteria (e.g. remarketing, prospecting). Omitted when blank." },
                    "lowercase": { "type": "boolean", "default": false, "description": "When true, lowercase the parameter values so 'Email' and 'email' don't split a campaign in analytics. Default false." }
                },
                "required": ["url", "source", "medium", "campaign"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}

//! gizza-ai/json-ld-generator — chat skill block on the shared tool abstraction.
//! Generates schema.org JSON-LD structured-data markup from simple field inputs.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    /// schema.org @type to generate. Case-insensitive; blank → Article.
    #[serde(default)]
    schema_type: String,
    /// Newline-separated `name = value` (or `name: value`) property pairs.
    #[serde(default)]
    fields: String,
    /// For FAQPage: newline-separated `Question? | Answer` pairs.
    #[serde(default)]
    faq: String,
    /// Wrap the JSON in a <script type="application/ld+json"> tag.
    #[serde(default)]
    wrap_script: bool,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::enumv(
                "schema_type",
                [
                    "Article",
                    "Product",
                    "FAQPage",
                    "Organization",
                    "LocalBusiness",
                    "Person",
                    "Event",
                    "Recipe",
                    "WebSite",
                    "BreadcrumbList",
                    "Review",
                    "VideoObject",
                    "HowTo",
                    "JobPosting",
                    "Course",
                    "SoftwareApplication",
                ],
            )
            .default("Article")
            .describe("The schema.org @type to generate. Case-insensitive; defaults to Article."),
        )
        .param(
            Param::string("fields")
                .describe(
                    "Newline-separated 'name = value' (or 'name: value') property pairs, e.g. 'headline = My Title'. Dotted names build nested objects with an inferred @type (author.name -> Person, address.streetAddress -> PostalAddress, offers.price -> Offer, publisher.name -> Organization). List fields (keywords, sameAs, ingredients) split on commas into arrays; numeric fields (price, ratingValue, reviewCount, latitude, longitude, calories) become JSON numbers. Blank lines and lines starting with # are ignored.",
                ),
        )
        .param(
            Param::string("faq")
                .describe(
                    "For schema_type=FAQPage only: newline-separated 'Question? | Answer' pairs (or use '::' as the separator). Each pair becomes a Question/acceptedAnswer entry in mainEntity. Required when schema_type is FAQPage; ignored otherwise.",
                ),
        )
        .param(
            Param::boolean("wrap_script")
                .default(false)
                .describe(
                    "When true, wrap the JSON-LD in a <script type=\"application/ld+json\">...</script> tag ready to paste into a page's <head>. Default false (raw JSON only).",
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
    name = "gizza-ai/json-ld-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate schema.org JSON-LD structured data from simple field inputs.",
    skill(
        description = "Generate schema.org JSON-LD structured-data markup from simple field inputs. Pick a schema_type (Article, Product, FAQPage, Organization, LocalBusiness, Person, Event, Recipe, WebSite, BreadcrumbList, Review, VideoObject, HowTo, JobPosting, Course, SoftwareApplication) and pass 'fields' as newline-separated 'name = value' pairs. Dotted names build nested objects with an inferred @type (author.name, address.streetAddress, offers.price); list fields split on commas; numeric fields become JSON numbers. For FAQPage, pass 'faq' as 'Question? | Answer' pairs. Set wrap_script=true to get a ready-to-paste <script type=\"application/ld+json\"> tag. Output validates against Google's Rich Results requirements for the chosen type.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "json-ld-generator", |a: Args| {
            gizza_ai_json_ld_generator_core::generate(
                &a.schema_type,
                &a.fields,
                &a.faq,
                a.wrap_script,
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

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "schema_type": { "type": "string", "enum": ["Article","Product","FAQPage","Organization","LocalBusiness","Person","Event","Recipe","WebSite","BreadcrumbList","Review","VideoObject","HowTo","JobPosting","Course","SoftwareApplication"], "default": "Article", "description": "The schema.org @type to generate. Case-insensitive; defaults to Article." },
                    "fields": { "type": "string", "description": "Newline-separated 'name = value' (or 'name: value') property pairs, e.g. 'headline = My Title'. Dotted names build nested objects with an inferred @type (author.name -> Person, address.streetAddress -> PostalAddress, offers.price -> Offer, publisher.name -> Organization). List fields (keywords, sameAs, ingredients) split on commas into arrays; numeric fields (price, ratingValue, reviewCount, latitude, longitude, calories) become JSON numbers. Blank lines and lines starting with # are ignored." },
                    "faq": { "type": "string", "description": "For schema_type=FAQPage only: newline-separated 'Question? | Answer' pairs (or use '::' as the separator). Each pair becomes a Question/acceptedAnswer entry in mainEntity. Required when schema_type is FAQPage; ignored otherwise." },
                    "wrap_script": { "type": "boolean", "default": false, "description": "When true, wrap the JSON-LD in a <script type=\"application/ld+json\">...</script> tag ready to paste into a page's <head>. Default false (raw JSON only)." }
                },
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}

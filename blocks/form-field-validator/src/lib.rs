//! gizza-ai/form-field-validator — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. The new-tool skill edits
//! descriptor()'s params + core::run to the tool's real inputs/logic.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    fields: String,
    #[serde(default = "default_country")]
    country: String,
    #[serde(default)]
    required: String,
    #[serde(default)]
    rules: String,
    #[serde(default = "default_normalize")]
    normalize: bool,
    #[serde(default = "default_mask_sensitive")]
    mask_sensitive: bool,
    #[serde(default = "default_output")]
    output: String,
}

fn default_country() -> String {
    "any".to_string()
}
fn default_normalize() -> bool {
    true
}
fn default_mask_sensitive() -> bool {
    true
}
fn default_output() -> String {
    "text".to_string()
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("fields")
                .required()
                .describe("Form fields to validate. Use one `name: value` pair per line (for example `email: user@example.com`) or a JSON object of name/value pairs. Field names are used to infer email, phone, URL, postal-code, credit-card, or text validation unless overridden by `rules`. Maximum 200 fields."),
        )
        .param(
            Param::enumv("country", gizza_ai_form_field_validator_core::COUNTRY_CODES)
                .default("any")
                .describe("Country / locale for phone and postal-code checks. Use `any` for generic E.164-style phone length and loose postal-code shape, or an ISO 3166-1 alpha-2 code such as US, GB, CA, DE, FR, AU, IN, JP, BR, or ZA for country-specific examples and formats."),
        )
        .param(
            Param::string("required")
                .default("")
                .describe("Required fields. Leave blank for no required fields, use `*` to require every supplied field, or list names separated by commas or new lines (for example `email, phone, zip`). Missing or blank required fields fail with an explicit error."),
        )
        .param(
            Param::string("rules")
                .default("")
                .describe("Optional type overrides, one per line as `field: type` or `field=type`. Types: email, phone, url, postal-code, credit-card, text. Common aliases like zip, postcode, tel, website, card, and cc are accepted. Blank lines and `#` comments are ignored."),
        )
        .param(
            Param::boolean("normalize")
                .default(true)
                .describe("When true (default), passing values are shown in their normalized form where possible: lower-cased email domains, E.164-style phone numbers, canonical postal-code spacing/case, and digits-only credit cards."),
        )
        .param(
            Param::boolean("mask_sensitive")
                .default(true)
                .describe("When true (default), credit-card values are masked in text and JSON output so only the final four digits remain visible. The Luhn and brand checks still run against the full value."),
        )
        .param(
            Param::enumv("output", ["text", "json"])
                .default("text")
                .describe("Output format: `text` (default) is a human-readable per-field report; `json` returns a machine-readable object with valid/checked/passed/failed/skipped counts plus per-field status, value, normalized value, errors, and expected-format hints."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/form-field-validator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Validate a whole form submission: email, phone, URL, postal code, and credit card fields with locale-aware errors",
    skill(
        description = "Validate a whole form submission field by field. `fields` accepts one `name: value` line per form field, or a JSON object of name/value pairs. Field names infer email, phone, URL, postal-code, credit-card, or text validation; `rules` can override types with `field: type`. `country` is `any` or an ISO alpha-2 locale such as US, GB, CA, DE, FR, AU, IN, JP, BR, or ZA, and controls phone and postal-code format checks. `required` can be blank, `*`, or a comma/newline list of field names. `normalize` shows canonical passing values; `mask_sensitive` hides credit-card digits except the last four; `output` is text or json. This is deterministic offline format validation only: no DNS/MX, carrier, address-existence, or BIN lookups.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "form-field-validator", |a: Args| {
            gizza_ai_form_field_validator_core::run(
                &a.fields,
                &a.country,
                &a.required,
                &a.rules,
                a.normalize,
                a.mask_sensitive,
                &a.output,
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
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["required"], serde_json::json!(["fields"]));
        assert_eq!(schema["additionalProperties"], false);
        let props = schema["properties"].as_object().unwrap();
        assert_eq!(props.len(), 7, "no LLM-facing chat-schema drift");
        assert!(props["fields"]["description"]
            .as_str()
            .unwrap()
            .contains("Maximum 200 fields"));
        assert_eq!(
            props["country"]["enum"].as_array().unwrap().len(),
            gizza_ai_form_field_validator_core::COUNTRY_CODES.len()
        );
        assert_eq!(props["country"]["default"], "any");
        assert_eq!(props["normalize"]["type"], "boolean");
        assert_eq!(props["normalize"]["default"], true);
        assert_eq!(props["mask_sensitive"]["type"], "boolean");
        assert_eq!(props["mask_sensitive"]["default"], true);
        assert_eq!(props["output"]["enum"], serde_json::json!(["text", "json"]));
    }
}

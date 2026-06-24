//! gizza-ai/html-form-generator — chat skill block on the shared tool abstraction.
//! Generates accessible HTML `<form>` markup from a plain-text field description.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. No host calls.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    fields: String,
    #[serde(default)]
    method: String,
    #[serde(default)]
    action: String,
    #[serde(default)]
    submit_label: String,
    /// Emit a `<style>` block + the `gizza-form` class (default true).
    #[serde(default = "default_true")]
    styled: bool,
}

fn default_true() -> bool {
    true
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("fields")
                .required()
                .describe(
                    "The form fields, one per line. Each line is pipe-separated: \
                     'Label | type | options'. End a label with '*' to mark it required \
                     (e.g. 'Email*'). The type (default 'text') is one of: text, email, \
                     password, number, tel, url, date, time, textarea, select, radio, \
                     checkbox, checkboxes. For select/radio/checkboxes the third part is a \
                     comma-separated option list (e.g. 'Color | select | Red, Green, Blue'); \
                     for the other types it is used as the input's placeholder. Lines starting \
                     with '#' are comments.",
                ),
        )
        .param(
            Param::enumv("method", ["post", "get"])
                .default("post")
                .describe("The form's HTTP method attribute: 'post' (default) or 'get'."),
        )
        .param(
            Param::string("action")
                .default("")
                .describe("The form's 'action' URL where it submits. Blank (default) omits the attribute."),
        )
        .param(
            Param::string("submit_label")
                .default("Submit")
                .describe("Text on the submit button. Default 'Submit'."),
        )
        .param(
            Param::boolean("styled")
                .default(true)
                .describe("When true (default), prepend a small <style> block and add the 'gizza-form' class so the markup looks good standalone. Set false for unstyled, class-free markup."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/html-form-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate accessible HTML form markup",
    skill(
        description = "Generate accessible, ready-to-paste HTML <form> markup from a plain-text description of fields. Describe one field per line as 'Label | type | options': end a label with '*' to make it required; type (default text) is one of text, email, password, number, tel, url, date, time, textarea, select, radio, checkbox, checkboxes; for select/radio/checkboxes give a comma-separated option list, otherwise the third part is a placeholder. Each field gets a slugified name/id, a properly bound <label for>, validation attributes (required, type=email/url/number/...), and unique names; output can include a small <style> block. Set method (post/get), action URL, and the submit button label.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "html-form-generator", |a: Args| {
            gizza_ai_html_form_generator_core::generate(
                &a.fields,
                &a.method,
                &a.action,
                &a.submit_label,
                a.styled,
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
                    "fields": { "type": "string", "description": "The form fields, one per line. Each line is pipe-separated: 'Label | type | options'. End a label with '*' to mark it required (e.g. 'Email*'). The type (default 'text') is one of: text, email, password, number, tel, url, date, time, textarea, select, radio, checkbox, checkboxes. For select/radio/checkboxes the third part is a comma-separated option list (e.g. 'Color | select | Red, Green, Blue'); for the other types it is used as the input's placeholder. Lines starting with '#' are comments." },
                    "method": { "type": "string", "enum": ["post", "get"], "default": "post", "description": "The form's HTTP method attribute: 'post' (default) or 'get'." },
                    "action": { "type": "string", "default": "", "description": "The form's 'action' URL where it submits. Blank (default) omits the attribute." },
                    "submit_label": { "type": "string", "default": "Submit", "description": "Text on the submit button. Default 'Submit'." },
                    "styled": { "type": "boolean", "default": true, "description": "When true (default), prepend a small <style> block and add the 'gizza-form' class so the markup looks good standalone. Set false for unstyled, class-free markup." }
                },
                "required": ["fields"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}

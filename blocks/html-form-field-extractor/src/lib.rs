//! gizza-ai/html-form-field-extractor — enumerate every `<form>` in pasted HTML
//! and list every form control with its name, id, type, label, required flag,
//! default value, and validation attributes. Thin wrapper around the core; chat
//! schema single-sourced from descriptor(); handler delegates to run_skill.
//! Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_html_form_field_extractor_core::{extract, parse_format, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    html: String,
    #[serde(default)]
    format: String,
    #[serde(default = "default_form_index")]
    form_index: i64,
    #[serde(default)]
    include_buttons: bool,
    #[serde(default = "default_true")]
    include_hidden: bool,
    #[serde(default = "default_true")]
    include_labels: bool,
}
fn default_form_index() -> i64 {
    -1
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("html").required().describe("The raw HTML containing the form(s), e.g. a page's source or a <form> snippet. Parsed with a real HTML parser, so unquoted attributes and unclosed tags are fine."))
        .param(Param::enumv("format", ["json", "markdown", "csv"]).default("json").describe("Output shape: json (default, forms each with a nested fields array), markdown (one table per form, plus option lists), or csv (one flat row per field, spreadsheet-ready)."))
        .param(Param::integer("form_index").default(-1).min(-1.0).describe("Which form to report, 0-based in document order. -1 (default) reports every form. Controls that sit outside any <form> are grouped into a final extra form."))
        .param(Param::boolean("include_buttons").default(false).describe("Include submit, reset, button, and image controls (and every <button>). Default false — they are actions, not data fields."))
        .param(Param::boolean("include_hidden").default(true).describe("Include <input type=\"hidden\"> fields such as CSRF tokens and tracking ids. Default true."))
        .param(Param::boolean("include_labels").default(true).describe("Resolve each control's visible label text from <label for>, a wrapping <label>, aria-label, or title. Default true; set false to drop the label column."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct HtmlFormFieldExtractor;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/html-form-field-extractor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "List every field of every HTML form with its type, required flag, default, and validation rules",
    skill(
        description = "Enumerate every <form> in pasted HTML and list every control it contains. Each field reports its tag, effective type, name, id, resolved label, required flag, default value, placeholder, state flags (disabled/readonly/checked/multiple), validation attributes (pattern, min, max, step, minlength, maxlength, accept, autocomplete), and — for <select> — every option with its value, label, optgroup, and selected state; each form reports its action, method, id, name, and enctype. format='json' (default), 'markdown', or 'csv'. form_index picks one form (0-based; -1 = all). include_buttons (default false) adds submit/reset/button controls; include_hidden (default true) keeps hidden inputs; include_labels (default true) resolves label text. Controls outside any <form> are grouped into a final extra form. Caps at 2000 fields.",
        parameters = schema_json()
    )
)]
impl HtmlFormFieldExtractor {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "html-form-field-extractor", |a: Args| {
            let fmt = parse_format(&a.format).map_err(SkillError::InvalidArgs)?;
            extract(
                &a.html,
                fmt,
                &Options {
                    form_index: a.form_index,
                    include_buttons: a.include_buttons,
                    include_hidden: a.include_hidden,
                    include_labels: a.include_labels,
                },
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
                    "html":            { "type": "string", "description": "The raw HTML containing the form(s), e.g. a page's source or a <form> snippet. Parsed with a real HTML parser, so unquoted attributes and unclosed tags are fine." },
                    "format":          { "type": "string", "enum": ["json", "markdown", "csv"], "default": "json", "description": "Output shape: json (default, forms each with a nested fields array), markdown (one table per form, plus option lists), or csv (one flat row per field, spreadsheet-ready)." },
                    "form_index":      { "type": "integer", "minimum": -1, "default": -1, "description": "Which form to report, 0-based in document order. -1 (default) reports every form. Controls that sit outside any <form> are grouped into a final extra form." },
                    "include_buttons": { "type": "boolean", "default": false, "description": "Include submit, reset, button, and image controls (and every <button>). Default false — they are actions, not data fields." },
                    "include_hidden":  { "type": "boolean", "default": true, "description": "Include <input type=\"hidden\"> fields such as CSRF tokens and tracking ids. Default true." },
                    "include_labels":  { "type": "boolean", "default": true, "description": "Resolve each control's visible label text from <label for>, a wrapping <label>, aria-label, or title. Default true; set false to drop the label column." }
                },
                "required": ["html"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}

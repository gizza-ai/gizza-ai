//! gizza-ai/bulk-file-renamer — compute old→new filename mappings from batch rename rules.
//! Chat/CLI schema is single-sourced from descriptor(); the handler delegates to the pure core.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_bulk_file_renamer_core::run_named;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    filenames: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default)]
    find: String,
    #[serde(default)]
    replace: String,
    #[serde(default = "default_case")]
    case_type: String,
    #[serde(default = "default_pattern")]
    pattern: String,
    #[serde(default = "default_start")]
    start: i64,
    #[serde(default = "default_padding")]
    padding: i64,
    #[serde(default)]
    prefix: String,
    #[serde(default)]
    suffix: String,
    #[serde(default = "default_true")]
    preserve_extension: bool,
}

fn default_mode() -> String {
    "find_replace".into()
}
fn default_case() -> String {
    "lower".into()
}
fn default_pattern() -> String {
    "file-{n}".into()
}
fn default_start() -> i64 {
    1
}
fn default_padding() -> i64 {
    1
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("filenames").required().describe("One filename per line. The tool computes a safe old -> new rename mapping only; it does not touch files or unzip archives."))
        .param(Param::enumv("mode", ["find_replace", "regex", "sequential", "case"]).default("find_replace").describe("Rename rule to apply: find_replace, regex, sequential numbering, or case conversion."))
        .param(Param::string("find").default("").describe("Text or regular expression to find. Required for regex mode; optional for find_replace."))
        .param(Param::string("replace").default("").describe("Replacement text. Regex mode supports capture references like $1 and $name."))
        .param(Param::enumv("case_type", ["lower", "upper", "title", "snake", "kebab", "camel", "pascal"]).default("lower").describe("Case conversion used when mode=case."))
        .param(Param::string("pattern").default("file-{n}").describe("Sequential numbering pattern for mode=sequential. Supports {n}, {name}, and {ext}."))
        .param(Param::integer("start").min(-999999.0).max(999999.0).default(1).describe("Starting number for sequential mode."))
        .param(Param::integer("padding").min(1.0).max(20.0).default(1).describe("Minimum digit width for {n} in sequential mode."))
        .param(Param::string("prefix").default("").describe("Optional text to prepend to every generated filename."))
        .param(Param::string("suffix").default("").describe("Optional text to append before the preserved extension."))
        .param(Param::boolean("preserve_extension").default(true).describe("When true, transform only the filename stem and keep the original extension."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/bulk-file-renamer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Preview bulk filename renames with find/replace, regex, numbering, and case rules",
    skill(
        description = "Compute a deterministic old -> new rename mapping for a newline-separated list of filenames. Supports find/replace, regex replacements with capture groups, sequential numbering patterns ({n}, {name}, {ext}), case conversions (lower, upper, title, snake, kebab, camel, pascal), optional prefix/suffix, extension preservation, and collision warnings. It is a safe preview engine only: it does not mutate files, upload archives, or create ZIP output.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "bulk-file-renamer", |a: Args| {
            run_named(
                &a.filenames,
                &a.mode,
                &a.find,
                &a.replace,
                &a.case_type,
                &a.pattern,
                a.start,
                a.padding,
                &a.prefix,
                &a.suffix,
                a.preserve_extension,
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
    fn schema_json_matches_authored_parameters() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = schema["properties"].as_object().unwrap();
        assert!(schema["required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("filenames")));
        assert_eq!(
            props["mode"]["enum"],
            serde_json::json!(["find_replace", "regex", "sequential", "case"])
        );
        assert_eq!(
            props["case_type"]["enum"],
            serde_json::json!(["lower", "upper", "title", "snake", "kebab", "camel", "pascal"])
        );
        assert_eq!(props["padding"]["maximum"], serde_json::json!(20));
        assert_eq!(props["preserve_extension"]["default"], serde_json::json!(true));
    }
}

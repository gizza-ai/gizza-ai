//! gizza-ai/xliff-to-json — extract XLIFF translation units into JSON. Thin
//! wrapper; chat schema single-sourced from descriptor(); handler delegates to
//! run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    xliff: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_key")]
    key: String,
    #[serde(default = "default_inline_tags")]
    inline_tags: String,
    #[serde(default = "default_true")]
    include_empty_targets: bool,
    #[serde(default)]
    fallback_to_source: bool,
    #[serde(default)]
    nested: bool,
    #[serde(default = "default_separator")]
    separator: String,
    #[serde(default)]
    include_metadata: bool,
}
fn default_true() -> bool {
    true
}
fn default_output() -> String {
    "pairs".to_string()
}
fn default_key() -> String {
    "id".to_string()
}
fn default_inline_tags() -> String {
    "placeholder".to_string()
}
fn default_separator() -> String {
    ".".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("xliff")
                .required()
                .describe("The XLIFF document to convert. Both XLIFF 1.2 (<trans-unit>) and XLIFF 2.x (<unit>/<segment>) are accepted; the version is detected from the document."),
        )
        .param(
            Param::enumv("output", ["pairs", "target", "source", "array"])
                .default("pairs")
                .describe("Shape of the emitted JSON: 'pairs' = { id: { source, target } } (lossless, default), 'target' = { id: target } for a drop-in translation bundle, 'source' = { id: source }, 'array' = [ { id, source, target } ] which keeps document order and duplicate ids."),
        )
        .param(
            Param::enumv("key", ["id", "resname", "source"])
                .default("id")
                .describe("Which value becomes the object key: the unit's 'id' (default), its 'resname' (1.2) / 'name' (2.x) attribute, or its 'source' text. resname and source fall back to the id when the unit has none. Ignored when output=array."),
        )
        .param(
            Param::enumv("inline_tags", ["placeholder", "strip", "keep"])
                .default("placeholder")
                .describe("How inline markup inside <source>/<target> is rendered: 'placeholder' (default) replaces each code element with its equiv-text, else {id}, so interpolations like {{name}} survive; 'strip' drops the markup and keeps only translatable text; 'keep' preserves the element's inner XML verbatim."),
        )
        .param(
            Param::boolean("include_empty_targets")
                .default(true)
                .describe("Include units whose <target> is missing or empty. Set false to emit only translated strings. Default true."),
        )
        .param(
            Param::boolean("fallback_to_source")
                .default(false)
                .describe("When a unit has no translation, use its source text as the target instead of an empty string. Default false, so untranslated strings stay visibly empty rather than silently shipping the source language."),
        )
        .param(
            Param::boolean("nested")
                .default(false)
                .describe("Split each key on the separator and emit a nested object tree (i18next style), so 'home.title' becomes { \"home\": { \"title\": … } }. Default false (flat keys)."),
        )
        .param(
            Param::string("separator")
                .default(".")
                .describe("Separator used to split keys when nested=true. Default '.'."),
        )
        .param(
            Param::boolean("include_metadata")
                .default(false)
                .describe("Add each unit's resname, note, translation state, approved flag, originating file and enclosing group path to its record. Only applies to the 'pairs' and 'array' shapes. Default false."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct XliffToJson;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/xliff-to-json",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract XLIFF translation units into JSON",
    skill(
        description = "Convert an XLIFF localization file into JSON. Handles XLIFF 1.2 (<trans-unit id resname> with <source>/<target>) and XLIFF 2.x (<unit id name> › <segment> › <source>/<target>) in one pass, so the version never has to be declared; <group> nesting is walked rather than rejected, and a 2.x unit's multiple <segment> children are concatenated in document order. XML entities and CDATA are decoded, xml:space=\"preserve\" is honoured, and <alt-trans>/<seg-source>/translation-memory copies are ignored. Inline markup (<x/>, <ph>, <bpt>/<ept>, <sc>/<ec>, <pc>) is rendered per inline_tags: by default each code element becomes its equiv-text, else {id}, so interpolation placeholders survive instead of silently vanishing. Choose the output shape (source/target pairs by id, a flat target-only or source-only bundle, or an array of records that keeps duplicates and order), the key (id, resname/name, or source text), flat or nested keys, whether untranslated units are included or filled from the source, and whether per-unit metadata (note, state, approved, file, group) is attached. Output is deterministic pretty-printed JSON in document order. Fully local and deterministic — it extracts what the file already contains and does not translate anything.",
        parameters = schema_json()
    ),
)]
impl XliffToJson {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "xliff-to-json", |a: Args| {
            gizza_ai_xliff_to_json_core::run(
                &a.xliff,
                &a.output,
                &a.key,
                a.nested,
                &a.separator,
                a.include_empty_targets,
                a.fallback_to_source,
                &a.inline_tags,
                a.include_metadata,
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
            r##"{
                "type": "object",
                "properties": {
                    "xliff": { "type": "string", "description": "The XLIFF document to convert. Both XLIFF 1.2 (<trans-unit>) and XLIFF 2.x (<unit>/<segment>) are accepted; the version is detected from the document." },
                    "output": { "type": "string", "enum": ["pairs", "target", "source", "array"], "default": "pairs", "description": "Shape of the emitted JSON: 'pairs' = { id: { source, target } } (lossless, default), 'target' = { id: target } for a drop-in translation bundle, 'source' = { id: source }, 'array' = [ { id, source, target } ] which keeps document order and duplicate ids." },
                    "key": { "type": "string", "enum": ["id", "resname", "source"], "default": "id", "description": "Which value becomes the object key: the unit's 'id' (default), its 'resname' (1.2) / 'name' (2.x) attribute, or its 'source' text. resname and source fall back to the id when the unit has none. Ignored when output=array." },
                    "inline_tags": { "type": "string", "enum": ["placeholder", "strip", "keep"], "default": "placeholder", "description": "How inline markup inside <source>/<target> is rendered: 'placeholder' (default) replaces each code element with its equiv-text, else {id}, so interpolations like {{name}} survive; 'strip' drops the markup and keeps only translatable text; 'keep' preserves the element's inner XML verbatim." },
                    "include_empty_targets": { "type": "boolean", "default": true, "description": "Include units whose <target> is missing or empty. Set false to emit only translated strings. Default true." },
                    "fallback_to_source": { "type": "boolean", "default": false, "description": "When a unit has no translation, use its source text as the target instead of an empty string. Default false, so untranslated strings stay visibly empty rather than silently shipping the source language." },
                    "nested": { "type": "boolean", "default": false, "description": "Split each key on the separator and emit a nested object tree (i18next style), so 'home.title' becomes { \"home\": { \"title\": … } }. Default false (flat keys)." },
                    "separator": { "type": "string", "default": ".", "description": "Separator used to split keys when nested=true. Default '.'." },
                    "include_metadata": { "type": "boolean", "default": false, "description": "Add each unit's resname, note, translation state, approved flag, originating file and enclosing group path to its record. Only applies to the 'pairs' and 'array' shapes. Default false." }
                },
                "required": ["xliff"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}

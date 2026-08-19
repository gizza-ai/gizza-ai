//! gizza-ai/json-mask — chat skill block on the shared tool abstraction.
//! Prunes a JSON document to a subtree using the Google Partial-Response /
//! json-mask field syntax (`a,b(c,d)`). The chat schema is single-sourced from
//! descriptor() (which also drives the CLI); handle() delegates to
//! block_utils::run_skill. No host calls — runs entirely in the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    json: String,
    mask: String,
    #[serde(default)]
    mode: String,
    #[serde(default)]
    format: String,
    #[serde(default)]
    on_missing: String,
    #[serde(default)]
    empty: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("json")
                .required()
                .describe("The JSON document to mask — an object or an array at the top level, e.g. {\"id\":1,\"user\":{\"name\":\"Ada\",\"email\":\"a@x.io\"}}. Maximum 5000000 bytes."),
        )
        .param(
            Param::string("mask")
                .required()
                .describe("The field mask, in Google Partial-Response / json-mask syntax. 'a' keeps field a and its whole subtree; 'a,b,c' keeps several siblings; 'a/b/c' follows a path; 'a(b,c)' keeps only b and c inside a; '*' is a wildcard matching every key of an object; a backslash escapes a literal , / ( ) * or \\ in a key name. These combine and nest freely: 'kind,items(title,stats/length)'. Arrays are transparent — a selection is applied to every element and the array's length and order are preserved. Maximum 4000 bytes, nested at most 64 levels deep."),
        )
        .param(
            Param::enumv("mode", ["keep", "remove"])
                .default("keep")
                .describe("What the mask selects. 'keep' (default) returns ONLY the masked fields, pruning everything else while preserving the document's shape. 'remove' inverts it: the masked fields are deleted and everything else is returned — use it to strip tokens or PII, e.g. mask='users(token)' with mode='remove'."),
        )
        .param(
            Param::enumv("format", ["pretty", "compact"])
                .default("pretty")
                .describe("How the result is rendered: 'pretty' (default) is 2-space indented for reading; 'compact' is a single line with no whitespace, for piping into another tool."),
        )
        .param(
            Param::enumv("on_missing", ["omit", "null", "error"])
                .default("omit")
                .describe("What to do when the mask names a field the document doesn't have. 'omit' (default) leaves it out, matching every other json-mask implementation. 'null' emits the key with a JSON null so every record has the same shape — handy when masking a list of records for a table. 'error' fails with the field name and its path, which catches a typo'd mask instead of silently returning an empty result. In mode='remove' only 'error' has an effect (there is nothing to omit or null out)."),
        )
        .param(
            Param::enumv("empty", ["keep", "drop"])
                .default("keep")
                .describe("What to do with objects and array elements left empty by masking. 'keep' (default) keeps the {} husk, matching other implementations. 'drop' removes it, recursively — a container left empty by dropping is dropped too. Use 'drop' when masking a large array where most elements don't have the field."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct JsonMask;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/json-mask",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Select a subtree of JSON with a field mask like a,b(c,d).",
    skill(
        description = "Prune a JSON document down to just the fields you name, using the Google Partial-Response / json-mask field syntax — the same 'fields=' language Google and GitHub-style APIs use for partial responses. Unlike JSONPath or jq (which return a flat list of matched nodes), a mask preserves the document's SHAPE: the output is the same object with the unwanted branches removed. Syntax: 'a' keeps a and everything under it; 'a,b,c' keeps siblings; 'a/b/c' follows a path; 'a(b,c)' keeps only b and c inside a; '*' matches every key; '\\' escapes a special character in a key name. Arrays are transparent — 'items(title)' keeps just the title of every element, preserving array order and length. Set mode='remove' to invert the mask and strip the named fields instead (e.g. dropping tokens or PII). on_missing controls absent fields (omit, null, or a loud error), empty='drop' discards elements left empty, and format='compact' emits a single line. Example: mask 'kind,items(title,stats/length)'.",
        parameters = schema_json()
    ),
)]
impl JsonMask {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "json-mask", |a: Args| {
            gizza_ai_json_mask_core::run(
                &a.json,
                &a.mask,
                &a.mode,
                &a.format,
                &a.on_missing,
                &a.empty,
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
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed. Authored 2026-08-07 with the initial build.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "json": { "type": "string", "description": "The JSON document to mask — an object or an array at the top level, e.g. {\"id\":1,\"user\":{\"name\":\"Ada\",\"email\":\"a@x.io\"}}. Maximum 5000000 bytes." },
                    "mask": { "type": "string", "description": "The field mask, in Google Partial-Response / json-mask syntax. 'a' keeps field a and its whole subtree; 'a,b,c' keeps several siblings; 'a/b/c' follows a path; 'a(b,c)' keeps only b and c inside a; '*' is a wildcard matching every key of an object; a backslash escapes a literal , / ( ) * or \\ in a key name. These combine and nest freely: 'kind,items(title,stats/length)'. Arrays are transparent — a selection is applied to every element and the array's length and order are preserved. Maximum 4000 bytes, nested at most 64 levels deep." },
                    "mode": { "type": "string", "enum": ["keep", "remove"], "default": "keep", "description": "What the mask selects. 'keep' (default) returns ONLY the masked fields, pruning everything else while preserving the document's shape. 'remove' inverts it: the masked fields are deleted and everything else is returned — use it to strip tokens or PII, e.g. mask='users(token)' with mode='remove'." },
                    "format": { "type": "string", "enum": ["pretty", "compact"], "default": "pretty", "description": "How the result is rendered: 'pretty' (default) is 2-space indented for reading; 'compact' is a single line with no whitespace, for piping into another tool." },
                    "on_missing": { "type": "string", "enum": ["omit", "null", "error"], "default": "omit", "description": "What to do when the mask names a field the document doesn't have. 'omit' (default) leaves it out, matching every other json-mask implementation. 'null' emits the key with a JSON null so every record has the same shape — handy when masking a list of records for a table. 'error' fails with the field name and its path, which catches a typo'd mask instead of silently returning an empty result. In mode='remove' only 'error' has an effect (there is nothing to omit or null out)." },
                    "empty": { "type": "string", "enum": ["keep", "drop"], "default": "keep", "description": "What to do with objects and array elements left empty by masking. 'keep' (default) keeps the {} husk, matching other implementations. 'drop' removes it, recursively — a container left empty by dropping is dropped too. Use 'drop' when masking a large array where most elements don't have the field." }
                },
                "required": ["json", "mask"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}

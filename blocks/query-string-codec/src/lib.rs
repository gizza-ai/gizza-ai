//! gizza-ai/query-string-codec — chat skill block on the shared tool abstraction.
//! A bidirectional URL query-string codec: `parse` a query string into JSON, or
//! `build` a query string from a JSON object. The chat schema is single-sourced
//! from descriptor() (which also drives the CLI); handle() delegates to
//! block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default = "default_direction")]
    direction: String,
    input: String,
    #[serde(default = "default_array_style")]
    array_style: String,
    #[serde(default = "default_true")]
    space_as_plus: bool,
    #[serde(default)]
    sort_keys: bool,
    #[serde(default)]
    prefix_question_mark: bool,
}

fn default_direction() -> String {
    "parse".to_string()
}
fn default_array_style() -> String {
    "brackets".to_string()
}
fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::enumv("direction", ["parse", "build"])
                .default("parse")
                .describe("Which way to convert. `parse` (default): a query string → structured JSON (repeated keys become arrays; `a[]`/`a[0]`/`a[key]` bracket notation becomes nested arrays/objects). `build`: a JSON object → an encoded query string."),
        )
        .param(
            Param::string("input")
                .required()
                .describe("The data to convert. For `parse`: a query string, e.g. `name=John+Doe&color=red&color=blue&user[age]=30` (a leading `?` or a whole URL's `?...` part is accepted). For `build`: a JSON object, e.g. `{\"name\":\"John Doe\",\"tags\":[\"a\",\"b\"],\"user\":{\"age\":30}}` (the top level must be an object)."),
        )
        .param(
            Param::enumv("array_style", ["brackets", "indices", "repeat", "comma"])
                .default("brackets")
                .describe("`build` only — how JSON arrays serialize. `brackets`: `tags[]=a&tags[]=b`. `indices`: `tags[0]=a&tags[1]=b`. `repeat`: `tags=a&tags=b`. `comma`: `tags=a,b` (one compact pair; scalar values only, and not auto-split when parsing back). brackets/indices/repeat round-trip through `parse`."),
        )
        .param(
            Param::boolean("space_as_plus")
                .default(true)
                .describe("Encode/decode spaces as `+` (application/x-www-form-urlencoded). When true (default): `build` writes a space as `+` and `parse` decodes `+` back to a space. When false: `build` writes `%20` and `parse` keeps `+` literal (strict RFC 3986). A literal `+` in a value is always escaped to `%2B` on build."),
        )
        .param(
            Param::boolean("sort_keys")
                .default(false)
                .describe("`build` only — sort object keys alphabetically (recursively) instead of keeping the input JSON's key order. Default false (preserve order)."),
        )
        .param(
            Param::boolean("prefix_question_mark")
                .default(false)
                .describe("`build` only — prepend a leading `?` to the result so it can be appended straight onto a URL. Default false (bare query string)."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/query-string-codec",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert between a URL query string and JSON, both directions",
    skill(
        description = "A bidirectional URL query-string codec. Set `direction` to `parse` (query string → JSON) or `build` (JSON object → query string). Parse splits on `&`/`;`, percent- and `+`-decodes every key and value, collapses repeated keys into arrays, and expands PHP/Rails bracket notation — `a[]=1&a[]=2` → an array, `a[0]=x&a[2]=z` → an indexed (null-padded) array, `user[name]=Ann&user[age]=30` → a nested object, including arrays of objects (`items[][id]=1`) — returning pretty structured JSON. Build walks a JSON object and serializes it back to an encoded query string: choose the array style (`brackets` `tags[]=a`, `indices` `tags[0]=a`, `repeat` `tags=a&tags=a`, or `comma` `tags=a,b`), whether a space is `+` or `%20` (`space_as_plus`), whether to sort keys, and whether to prepend `?`. brackets/indices/repeat round-trip back through parse. Runs locally; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "query-string-codec", |a: Args| {
            gizza_ai_query_string_codec_core::run(
                &a.direction,
                &a.input,
                &a.array_style,
                a.space_as_plus,
                a.sort_keys,
                a.prefix_question_mark,
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
                    "direction": { "type": "string", "enum": ["parse", "build"], "default": "parse", "description": "Which way to convert. `parse` (default): a query string → structured JSON (repeated keys become arrays; `a[]`/`a[0]`/`a[key]` bracket notation becomes nested arrays/objects). `build`: a JSON object → an encoded query string." },
                    "input": { "type": "string", "description": "The data to convert. For `parse`: a query string, e.g. `name=John+Doe&color=red&color=blue&user[age]=30` (a leading `?` or a whole URL's `?...` part is accepted). For `build`: a JSON object, e.g. `{\"name\":\"John Doe\",\"tags\":[\"a\",\"b\"],\"user\":{\"age\":30}}` (the top level must be an object)." },
                    "array_style": { "type": "string", "enum": ["brackets", "indices", "repeat", "comma"], "default": "brackets", "description": "`build` only — how JSON arrays serialize. `brackets`: `tags[]=a&tags[]=b`. `indices`: `tags[0]=a&tags[1]=b`. `repeat`: `tags=a&tags=b`. `comma`: `tags=a,b` (one compact pair; scalar values only, and not auto-split when parsing back). brackets/indices/repeat round-trip through `parse`." },
                    "space_as_plus": { "type": "boolean", "default": true, "description": "Encode/decode spaces as `+` (application/x-www-form-urlencoded). When true (default): `build` writes a space as `+` and `parse` decodes `+` back to a space. When false: `build` writes `%20` and `parse` keeps `+` literal (strict RFC 3986). A literal `+` in a value is always escaped to `%2B` on build." },
                    "sort_keys": { "type": "boolean", "default": false, "description": "`build` only — sort object keys alphabetically (recursively) instead of keeping the input JSON's key order. Default false (preserve order)." },
                    "prefix_question_mark": { "type": "boolean", "default": false, "description": "`build` only — prepend a leading `?` to the result so it can be appended straight onto a URL. Default false (bare query string)." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}

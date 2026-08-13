//! gizza-ai/graphql-formatter — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. The new-tool skill edits
//! descriptor()'s params + core::run to the tool's real inputs/logic.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_indent")]
    indent: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default)]
    sort_fields: bool,
    #[serde(default)]
    remove_comments: bool,
}

fn default_indent() -> String {
    "2".to_string()
}
fn default_mode() -> String {
    "format".to_string()
}

/// Single source for the chat schema (and CLI). Edit the params to match the
/// tool's real inputs — e.g. `.param(Param::enumv("mode", ["a","b"]).default("a"))`,
/// `.param(Param::integer("n").min(1.0))`. Use Input::Image/Video/Document/File
/// for tools that take a url/ref media input (see image-resize / web-fetch).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe("GraphQL query, mutation, subscription, fragment, or SDL schema text to validate and reformat."))
        .param(Param::enumv("indent", ["2", "4", "8", "tab"]).default("2").describe("Indentation unit for formatted output: 2, 4, or 8 spaces, or tab."))
        .param(Param::enumv("mode", ["format", "minify"]).default("format").describe("Format for readable output, or minify to remove unnecessary whitespace and comments."))
        .param(Param::boolean("sort_fields").default(false).describe("Sort selection fields and SDL object/input fields alphabetically for stable diffs."))
        .param(Param::boolean("remove_comments").default(false).describe("Drop GraphQL # comments when formatting. Minify mode always removes comments."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/graphql-formatter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Format, minify, and validate GraphQL queries and schemas",
    skill(
        description = "Format, minify, and validate GraphQL queries, mutations, fragments, and SDL schemas locally. Choose 2/4/8-space or tab indentation, format or minify mode, optionally sort fields for stable diffs, and optionally remove # comments. Syntax errors include line and column information.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }. For a media
        // tool, use resolve_source + dispatch_ffmpeg + build_media_envelope
        // instead (see blocks/image-resize/src/lib.rs).
        match run_skill(&body, "graphql-formatter", |a: Args| {
            gizza_ai_graphql_formatter_core::run(
                &a.input,
                &a.indent,
                &a.mode,
                a.sort_fields,
                a.remove_comments,
            )
            .map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

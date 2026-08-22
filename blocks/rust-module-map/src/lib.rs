//! gizza-ai/rust-module-map — render pasted Rust source as a module/item map.
//! Chat schema is single-sourced from descriptor(); handler delegates to the
//! pure core so CLI, chat and the browser page stay in sync.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_rust_module_map_core::{module_map, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    source: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default)]
    max_depth: u32,
    #[serde(default)]
    focus_on: String,
    #[serde(default = "default_sort")]
    sort_by: String,
    #[serde(default = "default_true")]
    show_types: bool,
    #[serde(default = "default_true")]
    show_traits: bool,
    #[serde(default = "default_true")]
    show_fns: bool,
    #[serde(default = "default_true")]
    show_impls: bool,
    #[serde(default)]
    show_consts: bool,
    #[serde(default)]
    include_tests: bool,
    #[serde(default = "default_true")]
    show_visibility: bool,
    #[serde(default)]
    crate_name: String,
}

fn default_format() -> String {
    "tree".into()
}
fn default_sort() -> String {
    "source".into()
}
fn default_true() -> bool {
    true
}

impl From<Args> for Options {
    fn from(a: Args) -> Options {
        Options {
            format: a.format,
            max_depth: a.max_depth,
            focus_on: a.focus_on,
            sort_by: a.sort_by,
            show_types: a.show_types,
            show_traits: a.show_traits,
            show_fns: a.show_fns,
            show_impls: a.show_impls,
            show_consts: a.show_consts,
            include_tests: a.include_tests,
            show_visibility: a.show_visibility,
            crate_name: a.crate_name,
        }
    }
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("source")
                .required()
                .describe("Rust source to map. Paste a lib.rs/main.rs/module file, or paste multiple files separated by header lines like '=== src/lib.rs ===' and '=== src/foo.rs ==='. Max 512 KiB."),
        )
        .param(
            Param::enumv("format", ["tree", "mermaid", "json", "paths"])
                .default("tree")
                .describe("Output shape: 'tree' for an indented module/item tree, 'mermaid' for a Mermaid flowchart, 'json' for a structured tree with counts, or 'paths' for a flat crate::module::Item list."),
        )
        .param(
            Param::integer("max_depth")
                .default(0)
                .min(0.0)
                .max(64.0)
                .describe("Maximum depth below the crate root to render. 0 (default) means unlimited; 1 shows only top-level items; max 64."),
        )
        .param(
            Param::string("focus_on")
                .default("")
                .describe("Optional module path to render instead of the whole crate, e.g. 'crate::config' or 'config::loader'. Empty (default) renders the whole pasted source."),
        )
        .param(
            Param::enumv("sort_by", ["source", "name", "kind", "visibility"])
                .default("source")
                .describe("Sibling ordering. 'source' preserves declaration order; 'name' sorts alphabetically; 'kind' groups modules/types/functions; 'visibility' puts public items first."),
        )
        .param(
            Param::boolean("show_types")
                .default(true)
                .describe("Include type declarations: structs, enums, unions and type aliases. Default true."),
        )
        .param(
            Param::boolean("show_traits")
                .default(true)
                .describe("Include trait and trait-alias declarations. Default true."),
        )
        .param(
            Param::boolean("show_fns")
                .default(true)
                .describe("Include free functions and methods inside impl blocks. Default true."),
        )
        .param(
            Param::boolean("show_impls")
                .default(true)
                .describe("Include impl blocks and their associated items. Default true."),
        )
        .param(
            Param::boolean("show_consts")
                .default(false)
                .describe("Include const and static items. Default false to keep large modules focused on structure."),
        )
        .param(
            Param::boolean("include_tests")
                .default(false)
                .describe("Include #[cfg(test)] modules and #[test]/#[bench] functions. Default false, matching normal cargo-modules output."),
        )
        .param(
            Param::boolean("show_visibility")
                .default(true)
                .describe("Annotate each item with visibility such as pub, pub(crate), pub(super), pub(in path), or pub(self). Default true."),
        )
        .param(
            Param::string("crate_name")
                .default("")
                .describe("Optional label for the crate root. Empty (default) renders the root as 'crate'; set this to your package name for prettier paths/graphs."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct RustModuleMap;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/rust-module-map",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Map Rust modules, types, traits, impls and functions from pasted source",
    skill(
        description = "Parse pasted Rust source with syn and render the module/item hierarchy as an indented tree, Mermaid flowchart, JSON tree, or flat crate::path list. Supports nested inline modules, 'mod foo;' declarations resolved from multi-file pastes with '=== src/foo.rs ===' headers, visibility annotations, impl methods, optional test items, kind filters, sorting, max-depth truncation, and focusing on a module path. It does not run cargo metadata or resolve cfg/features from disk; it maps exactly the Rust text you paste.",
        parameters = schema_json()
    ),
)]
impl RustModuleMap {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "rust-module-map", |a: Args| {
            let source = a.source.clone();
            let opts: Options = a.into();
            module_map(&source, &opts).map_err(SkillError::InvalidArgs)
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
                    "source": { "type": "string", "description": "Rust source to map. Paste a lib.rs/main.rs/module file, or paste multiple files separated by header lines like '=== src/lib.rs ===' and '=== src/foo.rs ==='. Max 512 KiB." },
                    "format": { "type": "string", "enum": ["tree", "mermaid", "json", "paths"], "default": "tree", "description": "Output shape: 'tree' for an indented module/item tree, 'mermaid' for a Mermaid flowchart, 'json' for a structured tree with counts, or 'paths' for a flat crate::module::Item list." },
                    "max_depth": { "type": "integer", "default": 0, "minimum": 0, "maximum": 64, "description": "Maximum depth below the crate root to render. 0 (default) means unlimited; 1 shows only top-level items; max 64." },
                    "focus_on": { "type": "string", "default": "", "description": "Optional module path to render instead of the whole crate, e.g. 'crate::config' or 'config::loader'. Empty (default) renders the whole pasted source." },
                    "sort_by": { "type": "string", "enum": ["source", "name", "kind", "visibility"], "default": "source", "description": "Sibling ordering. 'source' preserves declaration order; 'name' sorts alphabetically; 'kind' groups modules/types/functions; 'visibility' puts public items first." },
                    "show_types": { "type": "boolean", "default": true, "description": "Include type declarations: structs, enums, unions and type aliases. Default true." },
                    "show_traits": { "type": "boolean", "default": true, "description": "Include trait and trait-alias declarations. Default true." },
                    "show_fns": { "type": "boolean", "default": true, "description": "Include free functions and methods inside impl blocks. Default true." },
                    "show_impls": { "type": "boolean", "default": true, "description": "Include impl blocks and their associated items. Default true." },
                    "show_consts": { "type": "boolean", "default": false, "description": "Include const and static items. Default false to keep large modules focused on structure." },
                    "include_tests": { "type": "boolean", "default": false, "description": "Include #[cfg(test)] modules and #[test]/#[bench] functions. Default false, matching normal cargo-modules output." },
                    "show_visibility": { "type": "boolean", "default": true, "description": "Annotate each item with visibility such as pub, pub(crate), pub(super), pub(in path), or pub(self). Default true." },
                    "crate_name": { "type": "string", "default": "", "description": "Optional label for the crate root. Empty (default) renders the root as 'crate'; set this to your package name for prettier paths/graphs." }
                },
                "required": ["source"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}

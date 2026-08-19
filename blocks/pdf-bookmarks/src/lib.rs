//! gizza-ai/pdf-bookmarks — list, add, or remove PDF outline bookmarks.
//!
//! The tool takes a PDF document (`url` or `ref`) plus a `mode`. `list` returns
//! a readable outline, `apply` writes the supplied outline, `per-page` creates
//! one flat bookmark per page, and `remove` strips the existing outline. PDF
//! outputs are returned as a base64 `application/pdf` envelope. Chat + CLI only:
//! binary document input and PDF output do not have a standalone page surface.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    replace_extension, Envelope, ForUi, Input, Param, SkillError, SkillResultExt, SourceFields,
    ToolDescriptor,
};
use gizza_ai_pdf_bookmarks_core::{self as core, Bookmark, Options, Zoom};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "d_mode")]
    mode: String,
    #[serde(default)]
    bookmarks: String,
    #[serde(default = "d_replace")]
    replace: bool,
    #[serde(default = "d_expanded")]
    expanded: bool,
    #[serde(default = "d_show_pane")]
    show_pane: bool,
    #[serde(default = "d_zoom")]
    zoom: String,
    #[serde(default = "d_per_page_label")]
    per_page_label: String,
}

fn d_mode() -> String {
    "list".to_string()
}
fn d_replace() -> bool {
    true
}
fn d_expanded() -> bool {
    true
}
fn d_show_pane() -> bool {
    true
}
fn d_zoom() -> String {
    "fit".to_string()
}
fn d_per_page_label() -> String {
    "Page {n}".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Document)
        .param(
            Param::enumv("mode", ["list", "apply", "per-page", "remove"])
                .default("list")
                .describe("Operation to perform: list existing bookmarks, apply the supplied outline, create one bookmark per page, or remove all bookmarks. Default list."),
        )
        .param(Param::string("bookmarks").default("").describe(
            "Outline to write when mode=apply. Use one 'Title | page' entry per line, indent children with spaces or tabs, and optionally add attributes such as 'bold', 'italic', or '#3366cc'. A JSON array of {title,page,children} entries is also accepted.",
        ))
        .param(Param::boolean("replace").default(true).describe(
            "When mode=apply, replace the existing outline if true; append after existing bookmarks if false. Default true.",
        ))
        .param(Param::boolean("expanded").default(true).describe(
            "When writing bookmarks, store nested sections expanded/open in supporting PDF viewers. Default true.",
        ))
        .param(Param::boolean("show_pane").default(true).describe(
            "When writing bookmarks, request that PDF viewers open the bookmarks/outline pane. Default true.",
        ))
        .param(
            Param::enumv("zoom", ["fit", "fit-width", "keep"])
                .default("fit")
                .describe("Bookmark destination zoom: fit whole page, fit-width, or keep the reader's current zoom. Default fit."),
        )
        .param(Param::string("per_page_label").default("Page {n}").describe(
            "Label template for mode=per-page. Use {n} for the page number and {total} for page count, e.g. 'Sheet {n} of {total}'. Default 'Page {n}'.",
        ))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn options(args: &Args) -> Result<Options, SkillError> {
    Ok(Options {
        replace: args.replace,
        expanded: args.expanded,
        show_pane: args.show_pane,
        zoom: Zoom::parse(&args.zoom).map_err(SkillError::InvalidArgs)?,
    })
}

#[cfg(target_arch = "wasm32")]
struct PdfBookmarks;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pdf-bookmarks",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Manage PDF bookmarks and outlines",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "List, add, generate, or remove the bookmark/outline tree of a PDF. Provide the PDF as either url (HTTP/HTTPS) or ref. Use mode=list to inspect bookmarks, mode=apply with `bookmarks` to write an indented 'Title | page' outline (or JSON), mode=per-page to add one bookmark per page, and mode=remove to strip the outline. Writing options include replace, expanded, show_pane, zoom, and per_page_label.",
        parameters = schema_json()
    ),
)]
impl PdfBookmarks {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    use gizza_ai_block_utils::AssetKind;

    let args: Args = serde_json::from_slice(&body).invalid_args("pdf-bookmarks")?;
    let (bytes, _mime, filename) = resolve_source(
        args.source.clone().into_inner(),
        AssetKind::Document,
        MAX_BYTES,
    )?;

    match args.mode.trim().to_ascii_lowercase().as_str() {
        "list" => list_response(&bytes, &filename),
        "apply" => {
            let bookmarks = core::parse_spec(&args.bookmarks).map_err(SkillError::InvalidArgs)?;
            let result = core::apply(&bytes, bookmarks, &options(&args)?)
                .map_err(SkillError::InvalidArgs)?;
            pdf_response(result, &filename, "bookmarked.pdf", "wrote")
        }
        "per-page" => {
            let outline = core::list(&bytes).map_err(SkillError::InvalidArgs)?;
            let bookmarks = core::per_page(outline.page_count, &args.per_page_label);
            let result = core::apply(&bytes, bookmarks, &options(&args)?)
                .map_err(SkillError::InvalidArgs)?;
            pdf_response(result, &filename, "bookmarked.pdf", "created")
        }
        "remove" => {
            let result = core::remove(&bytes).map_err(SkillError::InvalidArgs)?;
            pdf_response(result, &filename, "no-bookmarks.pdf", "removed")
        }
        other => Err(SkillError::InvalidArgs(format!(
            "invalid mode '{other}' (expected list, apply, per-page, or remove)"
        ))),
    }
}

#[cfg(target_arch = "wasm32")]
fn list_response(bytes: &[u8], filename: &str) -> Result<Vec<u8>, SkillError> {
    let outline = core::list(bytes).map_err(SkillError::InvalidArgs)?;
    let result = if outline.bookmarks.is_empty() {
        format!(
            "{filename}: 0 bookmarks across {} page(s)",
            outline.page_count
        )
    } else {
        let mut out = format!(
            "{filename}: {} bookmark(s) across {} page(s)\n",
            outline.total, outline.page_count
        );
        render_bookmarks(&outline.bookmarks, 0, &mut out);
        out.trim_end().to_string()
    };
    serde_json::to_vec(&serde_json::json!({ "result": result }))
        .map_err(|e| SkillError::Serialize(format!("serialize result: {e}")))
}

#[cfg(target_arch = "wasm32")]
fn pdf_response(
    result: core::WriteResult,
    input_name: &str,
    suffix: &str,
    verb: &str,
) -> Result<Vec<u8>, SkillError> {
    let filename = replace_extension(input_name, suffix);
    let warnings = if result.warnings.is_empty() {
        String::new()
    } else {
        format!(" Warnings: {}", result.warnings.join("; "))
    };
    let for_llm = format!(
        "{verb} {} bookmark(s) in {input_name} ({} page(s), removed {}, top-level {}) -> {filename}.{warnings}",
        result.total, result.page_count, result.removed, result.top_level
    );
    let data_url = format!("data:application/pdf;base64,{}", B64.encode(&result.bytes));
    let env = Envelope {
        for_llm,
        for_ui: ForUi {
            data_url,
            mime: "application/pdf".to_string(),
            filename,
        },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

fn render_bookmarks(nodes: &[Bookmark], depth: usize, out: &mut String) {
    for node in nodes {
        out.push_str(&"  ".repeat(depth));
        out.push_str("- ");
        out.push_str(&node.title);
        if let Some(page) = node.page {
            out.push_str(&format!(" | page {page}"));
        } else {
            out.push_str(" | unresolved page");
        }
        if node.bold || node.italic || node.color.is_some() {
            let mut attrs = Vec::new();
            if node.bold {
                attrs.push("bold".to_string());
            }
            if node.italic {
                attrs.push("italic".to_string());
            }
            if let Some([r, g, b]) = node.color {
                attrs.push(format!(
                    "#{:02x}{:02x}{:02x}",
                    (r * 255.0).round() as u8,
                    (g * 255.0).round() as u8,
                    (b * 255.0).round() as u8
                ));
            }
            out.push_str(&format!(" ({})", attrs.join(", ")));
        }
        out.push('\n');
        render_bookmarks(&node.children, depth + 1, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_bookmark_tree_for_list_output() {
        let mut root = Bookmark::new("Intro", 1);
        root.children.push(Bookmark::new("Details", 2));
        root.bold = true;
        root.color = Some([1.0, 0.0, 0.0]);
        let mut out = String::new();
        render_bookmarks(&[root], 0, &mut out);
        assert!(out.contains("- Intro | page 1 (bold, #ff0000)"));
        assert!(out.contains("  - Details | page 2"));
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Document URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "mode": { "type": "string", "enum": ["list", "apply", "per-page", "remove"], "default": "list", "description": "Operation to perform: list existing bookmarks, apply the supplied outline, create one bookmark per page, or remove all bookmarks. Default list." },
                    "bookmarks": { "type": "string", "default": "", "description": "Outline to write when mode=apply. Use one 'Title | page' entry per line, indent children with spaces or tabs, and optionally add attributes such as 'bold', 'italic', or '#3366cc'. A JSON array of {title,page,children} entries is also accepted." },
                    "replace": { "type": "boolean", "default": true, "description": "When mode=apply, replace the existing outline if true; append after existing bookmarks if false. Default true." },
                    "expanded": { "type": "boolean", "default": true, "description": "When writing bookmarks, store nested sections expanded/open in supporting PDF viewers. Default true." },
                    "show_pane": { "type": "boolean", "default": true, "description": "When writing bookmarks, request that PDF viewers open the bookmarks/outline pane. Default true." },
                    "zoom": { "type": "string", "enum": ["fit", "fit-width", "keep"], "default": "fit", "description": "Bookmark destination zoom: fit whole page, fit-width, or keep the reader's current zoom. Default fit." },
                    "per_page_label": { "type": "string", "default": "Page {n}", "description": "Label template for mode=per-page. Use {n} for the page number and {total} for page count, e.g. 'Sheet {n} of {total}'. Default 'Page {n}'." }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}

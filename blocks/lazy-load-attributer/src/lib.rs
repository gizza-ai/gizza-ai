//! gizza-ai/lazy-load-attributer — add `loading="lazy"` and `decoding="async"`
//! to `<img>` / `<iframe>` tags that lack them, leaving the rest of the document
//! byte-for-byte intact.
//!
//! Thin chat-skill wrapper around `gizza-ai-lazy-load-attributer-core`. The chat
//! schema is single-sourced from `descriptor()` (which also drives the CLI + the
//! page query-params); `handle()` delegates to `block_utils::run_skill`. Pure —
//! no network, no host calls, so it runs on every backend.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_lazy_load_attributer_core::{run, Decoding, Options, Output, Targets, MAX_SKIP_FIRST};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    html: String,
    #[serde(default)]
    targets: String,
    #[serde(default)]
    decoding: String,
    #[serde(default)]
    skip_first: u64,
    #[serde(default)]
    eager_first: bool,
    #[serde(default)]
    fetchpriority_first: bool,
    #[serde(default = "default_true")]
    respect_skip_markers: bool,
    #[serde(default)]
    output: String,
}

fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI + page query-params).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("html")
                .required()
                .describe("The HTML to rewrite — a full document or a fragment, e.g. '<img src=\"photo.jpg\"><iframe src=\"https://example.com/embed\"></iframe>'. Everything except the matched start tags is copied through unchanged."),
        )
        .param(
            Param::enumv("targets", ["both", "images", "iframes"])
                .default("both")
                .describe("Which elements to rewrite: 'both' (default) does <img> and <iframe>, 'images' does <img> only, 'iframes' does <iframe> only."),
        )
        .param(
            Param::enumv("decoding", ["async", "sync", "auto", "none"])
                .default("async")
                .describe("Value written for the decoding attribute on <img> tags that lack one: 'async' (default, decode off the main thread), 'sync', 'auto', or 'none' to add no decoding attribute at all. Iframes have no decode step, so they never get this attribute."),
        )
        .param(
            Param::integer("skip_first")
                .min(0.0)
                .max(MAX_SKIP_FIRST as f64)
                .default(0)
                .describe("Leave the first N images in document order untouched so the above-the-fold / LCP image is never deferred (0-50, default 0). Counts <img> tags only; iframes are never treated as LCP candidates."),
        )
        .param(
            Param::boolean("eager_first")
                .default(false)
                .describe("When true, write loading=\"eager\" on the first skip_first images instead of leaving their loading attribute absent. Default false. Has no effect when skip_first is 0."),
        )
        .param(
            Param::boolean("fetchpriority_first")
                .default(false)
                .describe("When true, add fetchpriority=\"high\" to the very first image, hinting that it is the LCP element. Default false. Usually paired with skip_first=1."),
        )
        .param(
            Param::boolean("respect_skip_markers")
                .default(true)
                .describe("When true (default), leave tags carrying an opt-out marker alone: a 'skip-lazy' or 'no-lazy' class, or a data-skip-lazy / data-no-lazy attribute. Set false to rewrite them anyway."),
        )
        .param(
            Param::enumv("output", ["html", "report"])
                .default("html")
                .describe("What to return: 'html' (default) is the rewritten markup; 'report' is a human-readable count of what was added and what was left unchanged and why."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn build(a: &Args) -> Result<(Options, Output), String> {
    Ok((
        Options {
            targets: Targets::parse(&a.targets)?,
            decoding: Decoding::parse(&a.decoding)?,
            skip_first: a.skip_first as usize,
            eager_first: a.eager_first,
            fetchpriority_first: a.fetchpriority_first,
            respect_skip_markers: a.respect_skip_markers,
        },
        Output::parse(&a.output)?,
    ))
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/lazy-load-attributer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Add loading=\"lazy\" and decoding=\"async\" to images and iframes that lack them",
    skill(
        description = "Rewrite pasted HTML so every <img> and <iframe> that lacks them gets loading=\"lazy\" and (images only) decoding=\"async\". Pass the markup as 'html'. An attribute that is already present is never overwritten, so re-running is a no-op, and every other byte of the document — text, comments, doctype, script/style bodies, attribute quoting — is copied through unchanged. Tags with no src or srcset are skipped (there is nothing to defer), as are tags carrying a 'skip-lazy'/'no-lazy' class or a data-skip-lazy/data-no-lazy attribute unless respect_skip_markers=false. Use skip_first=N to leave the first N images eager so the above-the-fold LCP image is not deferred, eager_first=true to mark those explicitly with loading=\"eager\", and fetchpriority_first=true to add fetchpriority=\"high\" to the first image. targets narrows to images or iframes; decoding picks the decoding value (async/sync/auto) or 'none' to skip it; output='report' returns a count of what changed instead of the markup. Pure and local: no network, no image files are read, so widths/heights and srcset are never inferred.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "lazy-load-attributer", |a: Args| {
            let (opts, output) = build(&a).map_err(SkillError::InvalidArgs)?;
            run(&a.html, &opts, output).map_err(SkillError::InvalidArgs)
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
                    "html": { "type": "string", "description": "The HTML to rewrite — a full document or a fragment, e.g. '<img src=\"photo.jpg\"><iframe src=\"https://example.com/embed\"></iframe>'. Everything except the matched start tags is copied through unchanged." },
                    "targets": { "type": "string", "enum": ["both", "images", "iframes"], "default": "both", "description": "Which elements to rewrite: 'both' (default) does <img> and <iframe>, 'images' does <img> only, 'iframes' does <iframe> only." },
                    "decoding": { "type": "string", "enum": ["async", "sync", "auto", "none"], "default": "async", "description": "Value written for the decoding attribute on <img> tags that lack one: 'async' (default, decode off the main thread), 'sync', 'auto', or 'none' to add no decoding attribute at all. Iframes have no decode step, so they never get this attribute." },
                    "skip_first": { "type": "integer", "minimum": 0, "maximum": 50, "default": 0, "description": "Leave the first N images in document order untouched so the above-the-fold / LCP image is never deferred (0-50, default 0). Counts <img> tags only; iframes are never treated as LCP candidates." },
                    "eager_first": { "type": "boolean", "default": false, "description": "When true, write loading=\"eager\" on the first skip_first images instead of leaving their loading attribute absent. Default false. Has no effect when skip_first is 0." },
                    "fetchpriority_first": { "type": "boolean", "default": false, "description": "When true, add fetchpriority=\"high\" to the very first image, hinting that it is the LCP element. Default false. Usually paired with skip_first=1." },
                    "respect_skip_markers": { "type": "boolean", "default": true, "description": "When true (default), leave tags carrying an opt-out marker alone: a 'skip-lazy' or 'no-lazy' class, or a data-skip-lazy / data-no-lazy attribute. Set false to rewrite them anyway." },
                    "output": { "type": "string", "enum": ["html", "report"], "default": "html", "description": "What to return: 'html' (default) is the rewritten markup; 'report' is a human-readable count of what was added and what was left unchanged and why." }
                },
                "required": ["html"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn defaults_match_the_core_defaults() {
        let a = Args {
            html: String::new(),
            targets: String::new(),
            decoding: String::new(),
            skip_first: 0,
            eager_first: false,
            fetchpriority_first: false,
            respect_skip_markers: true,
            output: String::new(),
        };
        let (opts, output) = build(&a).unwrap();
        let d = Options::default();
        assert_eq!(opts.targets, d.targets);
        assert_eq!(opts.decoding, d.decoding);
        assert_eq!(opts.respect_skip_markers, d.respect_skip_markers);
        assert_eq!(output, Output::Html);
    }

    #[test]
    fn unknown_enum_values_are_rejected() {
        let a = Args {
            html: "<img src=a>".into(),
            targets: "videos".into(),
            decoding: String::new(),
            skip_first: 0,
            eager_first: false,
            fetchpriority_first: false,
            respect_skip_markers: true,
            output: String::new(),
        };
        assert!(build(&a).is_err());
    }
}

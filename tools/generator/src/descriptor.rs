//! Per-tool machine-readable descriptor (`tool.json`) — slug, name, category,
//! URLs (page / markdown twin / descriptor / deep-link example), the same
//! copy-paste CLI invocation as the page, and the block manifest's `tool`
//! schema (description + parameters JSON Schema, verbatim) — so agents can
//! discover and call a tool without scraping HTML.

use crate::categories;
use crate::control::ParamSchema;
use crate::markdown::{cli_example, example_deeplink};
use crate::meta::ToolMeta;
use crate::site::SiteConfig;
use serde::Serialize;
use serde_json::Value;
use std::path::Path;

/// The `tool.json` document. Field order here is the emitted key order
/// (serde serializes struct fields in declaration order).
#[derive(Serialize)]
struct Descriptor<'a> {
    slug: &'a str,
    name: String,
    /// Block version from `manifest.json`; omitted when the manifest is missing.
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    /// `meta.title` (bare) + `cfg.title_suffix` — see `SiteConfig::title`.
    title: String,
    description: &'a str,
    tags: &'a [String],
    /// Primary category slug (first taxonomy match — see `categories.rs`).
    category: &'static str,
    urls: Urls,
    /// Copy-paste-runnable CLI invocation — identical to the page/markdown example.
    cli: String,
    /// The manifest's `tool` block (description + parameters JSON Schema,
    /// passed through verbatim). Omitted — with a stderr warning — when the
    /// block has no manifest or no `tool` object.
    #[serde(skip_serializing_if = "Option::is_none")]
    tool: Option<ToolSpec>,
}

#[derive(Serialize)]
struct Urls {
    page: String,
    markdown: String,
    descriptor: String,
    /// Pre-filled auto-run URL — identical to the page/markdown example.
    /// Omitted for auto-only tools (nothing to deep-link, same rule as the
    /// markdown twin's "Query parameters" section).
    #[serde(skip_serializing_if = "Option::is_none")]
    deep_link_example: Option<String>,
}

#[derive(Serialize)]
struct ToolSpec {
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parameters: Option<Value>,
}

/// Read `<tool_dir>/manifest.json` as raw JSON. `None` when missing or
/// unparseable — `tool_descriptor` warns and degrades instead of aborting the
/// build over one stale manifest.
pub fn load_manifest(tool_dir: &Path) -> Option<Value> {
    let text = std::fs::read_to_string(tool_dir.join("manifest.json")).ok()?;
    serde_json::from_str(&text).ok()
}

/// Build the pretty-printed `tool.json` for one tool. `manifest` is the raw
/// `blocks/<slug>/manifest.json` (from [`load_manifest`]); when it — or its
/// `tool` object — is missing, the descriptor is still written without the
/// `tool` key so the discovery surface never disappears with a stale manifest.
pub fn tool_descriptor(
    cfg: &SiteConfig,
    meta: &ToolMeta,
    schema: &ParamSchema,
    manifest: Option<&Value>,
) -> String {
    let base = cfg.url_or_rel(&format!("/tools/{}/", meta.slug));
    let tool = match manifest {
        None => {
            eprintln!(
                "warning: no readable manifest.json for {} — tool.json omits the `tool` schema",
                meta.slug
            );
            None
        }
        Some(m) => match m.get("tool").filter(|t| t.is_object()) {
            None => {
                eprintln!(
                    "warning: manifest for {} has no `tool` object — tool.json omits the `tool` schema",
                    meta.slug
                );
                None
            }
            Some(t) => Some(ToolSpec {
                description: t.get("description").cloned(),
                parameters: t.get("parameters").cloned(),
            }),
        },
    };
    let descriptor = Descriptor {
        slug: &meta.slug,
        name: format!("gizza-ai/{}", meta.slug),
        version: manifest
            .and_then(|m| m.get("version"))
            .and_then(|v| v.as_str())
            .map(String::from),
        title: cfg.title(&meta.title),
        description: &meta.description,
        tags: &meta.tags,
        category: categories::primary_category(meta).slug,
        urls: Urls {
            page: base.clone(),
            markdown: format!("{base}index.md"),
            descriptor: format!("{base}tool.json"),
            deep_link_example: meta
                .inputs
                .iter()
                .any(|i| i.source == "field" || i.source == "file")
                .then(|| example_deeplink(cfg, meta, schema)),
        },
        cli: cli_example(meta, schema),
        tool,
    };
    serde_json::to_string_pretty(&descriptor).expect("serialize tool descriptor")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn branded() -> SiteConfig {
        SiteConfig {
            base_url: "https://gizza.ai".into(),
            brand_name: "gizza.ai".into(),
            title_suffix: " — gizza.ai".into(),
            ..Default::default()
        }
    }

    fn audio_tool() -> ToolMeta {
        ToolMeta::from_toml(
            r#"
slug          = "audio-convert"
title         = "Convert Audio"
description   = "Convert audio between formats in your browser."
tags          = ["audio"]
h1            = "Convert Audio"
hero_subtitle = "Upload an audio file."
wasm          = "gizza_ai_audio_convert_web"
export        = "build_argv"
output_label  = "Audio"
format        = "audio"
runtime       = "ffmpeg"

[[input]]
name   = "audio"
label  = "Audio"
source = "file"
accept = "audio/*"

[[input]]
name   = "format"
label  = "Format"
source = "field"
"#,
        )
        .unwrap()
    }

    fn manifest() -> Value {
        json!({
            "name": "gizza-ai/audio-convert",
            "version": "0.1.0",
            "interface": "handler@v1",
            "summary": "Convert audio between formats",
            "role": "skill",
            "tool": {
                "description": "Convert an audio file to mp3, wav, ogg, flac or m4a.",
                "parameters": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "url": { "type": "string", "description": "Audio URL." },
                        "format": { "type": "string", "enum": ["mp3", "wav"], "default": "mp3" }
                    },
                    "required": ["format"]
                }
            }
        })
    }

    fn schema_of(manifest: &Value) -> ParamSchema {
        ParamSchema::from_props_for_tests(manifest["tool"]["parameters"]["properties"].clone())
    }

    #[test]
    fn happy_path_has_identity_urls_cli_and_schema() {
        let m = manifest();
        let json = tool_descriptor(&branded(), &audio_tool(), &schema_of(&m), Some(&m));
        let v: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["slug"], "audio-convert");
        assert_eq!(v["name"], "gizza-ai/audio-convert");
        assert_eq!(v["version"], "0.1.0");
        assert_eq!(v["title"], "Convert Audio — gizza.ai");
        assert_eq!(v["description"], "Convert audio between formats in your browser.");
        assert_eq!(v["tags"][0], "audio");
        assert_eq!(v["category"], "audio", "primary category slug");
        assert_eq!(v["urls"]["page"], "https://gizza.ai/tools/audio-convert/");
        assert_eq!(v["urls"]["markdown"], "https://gizza.ai/tools/audio-convert/index.md");
        assert_eq!(v["urls"]["descriptor"], "https://gizza.ai/tools/audio-convert/tool.json");
        assert_eq!(
            v["urls"]["deep_link_example"],
            "https://gizza.ai/tools/audio-convert/?url=https://example.com/input&format=mp3",
            "deep-link example matches example_deeplink"
        );
        assert_eq!(
            v["cli"],
            "gizza tool audio-convert 'url=https://example.com/input' 'format=mp3'",
            "CLI example matches cli_example"
        );
        assert_eq!(
            v["tool"]["description"],
            "Convert an audio file to mp3, wav, ogg, flac or m4a."
        );
        // Pretty-printed (multi-line) output.
        assert!(json.contains('\n'), "pretty-printed");
    }

    #[test]
    fn missing_manifest_is_tolerated_without_tool_or_version() {
        let json = tool_descriptor(&branded(), &audio_tool(), &ParamSchema::empty(), None);
        let v: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["slug"], "audio-convert");
        assert_eq!(v["name"], "gizza-ai/audio-convert");
        assert!(v.get("tool").is_none(), "tool key omitted");
        assert!(v.get("version").is_none(), "version omitted");
        assert_eq!(v["urls"]["descriptor"], "https://gizza.ai/tools/audio-convert/tool.json");
        assert!(v["cli"].as_str().unwrap().starts_with("gizza tool audio-convert"));
    }

    #[test]
    fn manifest_without_tool_object_keeps_version_but_omits_tool() {
        let m = json!({ "name": "gizza-ai/audio-convert", "version": "0.2.0" });
        let v: Value =
            serde_json::from_str(&tool_descriptor(&branded(), &audio_tool(), &ParamSchema::empty(), Some(&m)))
                .unwrap();
        assert_eq!(v["version"], "0.2.0");
        assert!(v.get("tool").is_none(), "tool key omitted");
    }

    #[test]
    fn parameters_schema_is_passed_through_verbatim() {
        let m = manifest();
        let v: Value =
            serde_json::from_str(&tool_descriptor(&branded(), &audio_tool(), &schema_of(&m), Some(&m)))
                .unwrap();
        assert_eq!(
            v["tool"]["parameters"], m["tool"]["parameters"],
            "full JSON Schema — required, enum, defaults, additionalProperties — intact"
        );
    }

    #[test]
    fn auto_only_tool_has_no_deep_link_example() {
        let clock = ToolMeta::from_toml(
            r#"
slug          = "clock"
title         = "Clock"
description   = "The current time, live."
h1            = "Clock"
hero_subtitle = "Live time."
wasm          = "gizza_ai_clock_web"
export        = "now"
live          = true
output_label  = "Time"
format        = "text"

[[input]]
name   = "now"
label  = "Now"
source = "clock"
"#,
        )
        .unwrap();
        let v: Value =
            serde_json::from_str(&tool_descriptor(&branded(), &clock, &ParamSchema::empty(), None)).unwrap();
        assert!(
            v["urls"].get("deep_link_example").is_none(),
            "nothing to deep-link on an auto-only tool"
        );
        assert_eq!(v["cli"], "gizza tool clock");
    }

    #[test]
    fn default_config_uses_relative_urls() {
        let json = tool_descriptor(&SiteConfig::default(), &audio_tool(), &ParamSchema::empty(), None);
        let v: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["urls"]["page"], "/tools/audio-convert/");
        assert_eq!(v["urls"]["markdown"], "/tools/audio-convert/index.md");
        assert_eq!(v["urls"]["descriptor"], "/tools/audio-convert/tool.json");
    }
}

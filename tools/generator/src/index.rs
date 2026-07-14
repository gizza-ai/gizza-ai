//! Build-time JSON index of all tool pages, consumed by the in-app tools modal.

use crate::categories::Hub;
use crate::meta::ToolMeta;
use crate::site::SiteConfig;
use serde::Serialize;

#[derive(Serialize)]
struct IndexEntry<'a> {
    slug: &'a str,
    /// `meta.title` (bare) + `cfg.title_suffix` — see `SiteConfig::title`.
    title: String,
    description: &'a str,
    tags: &'a [String],
    /// Site-relative path of the tool's machine-readable descriptor
    /// (`tool.json` — identity, URLs, CLI example, parameters JSON Schema).
    descriptor: String,
}

/// Serialize `[{slug,title,description,tags,descriptor}]` for every tool, in
/// the given order.
pub fn tools_index_json(cfg: &SiteConfig, metas: &[ToolMeta]) -> String {
    let entries: Vec<IndexEntry> = metas
        .iter()
        .map(|m| IndexEntry {
            slug: &m.slug,
            title: cfg.title(&m.title),
            description: &m.description,
            tags: &m.tags,
            descriptor: format!("/tools/{}/tool.json", m.slug),
        })
        .collect();
    serde_json::to_string(&entries).expect("serialize tools index")
}

#[derive(Serialize)]
struct HubIndexEntry<'a> {
    slug: &'a str,
    title: &'a str,
    description: &'a str,
    count: usize,
}

/// Serialize `[{slug,title,description,count}]` for every category hub —
/// written to `tools/_hubs.json` and consumed by `scripts/gen-seo.sh` to add
/// the hub URLs to the sitemap.
pub fn hubs_json(hubs: &[Hub]) -> String {
    let entries: Vec<HubIndexEntry> = hubs
        .iter()
        .map(|h| HubIndexEntry {
            slug: h.category.slug,
            title: h.category.title,
            description: h.category.blurb,
            count: h.members.len(),
        })
        .collect();
    serde_json::to_string(&entries).expect("serialize hubs index")
}

/// Markdown catalog of every tool — the `text/markdown` twin of the `/tools/`
/// landing page, for LLMs and AI agents. Built from the same `metas` as the
/// HTML landing + `_index.json` (one source of truth, no drift).
pub fn tools_catalog_md(cfg: &SiteConfig, metas: &[ToolMeta]) -> String {
    let catalog_name = if cfg.brand_name.is_empty() {
        "Tools".to_string()
    } else {
        format!("{} tools", cfg.brand_name)
    };
    let brand_prefix = if cfg.brand_name.is_empty() {
        String::new()
    } else {
        format!("{} ", cfg.brand_name)
    };
    let mut s = format!(
        "# {catalog_name}\n\n> Every {brand_prefix}tool — free, private, browser-local utilities. \
         Nothing leaves your device, no sign-up, works offline.\n\n",
    );
    for m in metas {
        s.push_str(&format!(
            "- [{}]({}): {}\n",
            m.h1,
            cfg.url_or_rel(&format!("/tools/{}/", m.slug)),
            m.description
        ));
    }
    s.push_str(&format!(
        "\nMachine-readable catalog: <{}>. \
         Run any tool headlessly with `gizza tool <slug>` (see the CLI README).\n",
        cfg.url_or_rel("/tools/_index.json")
    ));
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn calc() -> ToolMeta {
        ToolMeta::from_toml(
            r#"
slug          = "calculator"
title         = "Free Online Calculator"
description   = "Evaluate expressions instantly."
tags          = ["math", "arithmetic"]
h1            = "Free Online Calculator"
hero_subtitle = "Type a math expression."
wasm          = "gizza_ai_calculator_web"
export        = "evaluate"
live          = false
output_label  = "Result"
format        = "number"

[[input]]
name        = "expr"
label       = "Expression"
placeholder = "2 + 2 * 3"
source      = "field"
"#,
        )
        .unwrap()
    }

    #[test]
    fn index_has_slug_title_description() {
        // Bare `meta.title` + `cfg.title_suffix` (Task 3) round-trips to the
        // historical suffixed title.
        let json = tools_index_json(&branded(), &[calc()]);
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(v.is_array());
        assert_eq!(v[0]["slug"], "calculator");
        assert_eq!(v[0]["title"], "Free Online Calculator — gizza.ai");
        assert_eq!(v[0]["description"], "Evaluate expressions instantly.");
        assert_eq!(v[0]["tags"][0], "math");
        assert_eq!(v[0]["tags"][1], "arithmetic");
        assert_eq!(
            v[0]["descriptor"], "/tools/calculator/tool.json",
            "entry links the machine-readable descriptor"
        );
    }

    #[test]
    fn default_config_renders_bare_unsuffixed_title() {
        let json = tools_index_json(&SiteConfig::default(), &[calc()]);
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v[0]["title"], "Free Online Calculator", "unsuffixed generic title");
    }

    #[test]
    fn tags_default_to_empty_array_when_absent() {
        // A tool meta without a `tags` line still serializes `tags: []`.
        let m = ToolMeta::from_toml(
            "slug=\"x\"\ntitle=\"X\"\ndescription=\"d\"\nh1=\"h\"\nhero_subtitle=\"s\"\nwasm=\"w\"\nexport=\"e\"\noutput_label=\"o\"\nformat=\"text\"\n",
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&tools_index_json(&SiteConfig::default(), &[m])).unwrap();
        assert!(v[0]["tags"].is_array());
        assert_eq!(v[0]["tags"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn empty_metas_is_empty_array() {
        assert_eq!(tools_index_json(&SiteConfig::default(), &[]), "[]");
    }

    #[test]
    fn hubs_json_has_slug_title_description_count() {
        let metas = vec![calc()];
        let hubs = crate::categories::build_hubs(&metas);
        let v: serde_json::Value = serde_json::from_str(&hubs_json(&hubs)).unwrap();
        assert!(v.is_array());
        // calc() is tagged "math" → exactly one hub, the math category
        assert_eq!(v.as_array().unwrap().len(), 1);
        assert_eq!(v[0]["slug"], "math");
        assert_eq!(v[0]["title"], "Math & statistics tools");
        assert!(v[0]["description"].as_str().unwrap().contains("Calculators"));
        assert_eq!(v[0]["count"], 1);
    }

    fn branded() -> SiteConfig {
        SiteConfig {
            base_url: "https://gizza.ai".into(),
            brand_name: "gizza.ai".into(),
            title_suffix: " — gizza.ai".into(),
            ..Default::default()
        }
    }

    #[test]
    fn catalog_md_lists_tools_from_metas() {
        let md = tools_catalog_md(&branded(), &[calc()]);
        assert!(md.starts_with("# gizza.ai tools"));
        assert!(md.contains("[Free Online Calculator](https://gizza.ai/tools/calculator/)"));
        assert!(md.contains("Evaluate expressions instantly."));
        assert!(md.contains("https://gizza.ai/tools/_index.json"));
    }

    #[test]
    fn catalog_md_default_config_is_generic() {
        let md = tools_catalog_md(&SiteConfig::default(), &[calc()]);
        assert!(md.starts_with("# Tools"));
        assert!(md.contains("[Free Online Calculator](/tools/calculator/)"));
        assert!(md.contains("/tools/_index.json"));
        assert!(!md.contains("gizza.ai"), "no brand leaks into a generic render");
    }
}

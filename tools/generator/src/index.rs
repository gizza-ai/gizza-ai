//! Build-time JSON index of all tool pages, consumed by the in-app tools modal.

use crate::meta::ToolMeta;
use serde::Serialize;

#[derive(Serialize)]
struct IndexEntry<'a> {
    slug: &'a str,
    title: &'a str,
    description: &'a str,
    tags: &'a [String],
}

/// Serialize `[{slug,title,description,tags}]` for every tool, in the given order.
pub fn tools_index_json(metas: &[ToolMeta]) -> String {
    let entries: Vec<IndexEntry> = metas
        .iter()
        .map(|m| IndexEntry {
            slug: &m.slug,
            title: &m.title,
            description: &m.description,
            tags: &m.tags,
        })
        .collect();
    serde_json::to_string(&entries).expect("serialize tools index")
}

/// Markdown catalog of every tool — the `text/markdown` twin of the `/tools/`
/// landing page, for LLMs and AI agents. Built from the same `metas` as the
/// HTML landing + `_index.json` (one source of truth, no drift).
pub fn tools_catalog_md(metas: &[ToolMeta]) -> String {
    let mut s = String::from(
        "# gizza.ai tools\n\n> Every gizza.ai tool — free, private, browser-local utilities. \
         Nothing leaves your device, no sign-up, works offline.\n\n",
    );
    for m in metas {
        s.push_str(&format!(
            "- [{}](https://gizza.ai/tools/{}/): {}\n",
            m.h1, m.slug, m.description
        ));
    }
    s.push_str(
        "\nMachine-readable catalog: <https://gizza.ai/tools/_index.json>. \
         Run any tool headlessly with `gizza tool <slug>` (see the CLI README).\n",
    );
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn calc() -> ToolMeta {
        ToolMeta::from_toml(
            r#"
slug          = "calculator"
title         = "Free Online Calculator — gizza.ai"
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
        let json = tools_index_json(&[calc()]);
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(v.is_array());
        assert_eq!(v[0]["slug"], "calculator");
        assert_eq!(v[0]["title"], "Free Online Calculator — gizza.ai");
        assert_eq!(v[0]["description"], "Evaluate expressions instantly.");
        assert_eq!(v[0]["tags"][0], "math");
        assert_eq!(v[0]["tags"][1], "arithmetic");
    }

    #[test]
    fn tags_default_to_empty_array_when_absent() {
        // A tool meta without a `tags` line still serializes `tags: []`.
        let m = ToolMeta::from_toml(
            "slug=\"x\"\ntitle=\"X\"\ndescription=\"d\"\nh1=\"h\"\nhero_subtitle=\"s\"\nwasm=\"w\"\nexport=\"e\"\noutput_label=\"o\"\nformat=\"text\"\n",
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&tools_index_json(&[m])).unwrap();
        assert!(v[0]["tags"].is_array());
        assert_eq!(v[0]["tags"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn empty_metas_is_empty_array() {
        assert_eq!(tools_index_json(&[]), "[]");
    }

    #[test]
    fn catalog_md_lists_tools_from_metas() {
        let md = tools_catalog_md(&[calc()]);
        assert!(md.starts_with("# gizza.ai tools"));
        assert!(md.contains("[Free Online Calculator](https://gizza.ai/tools/calculator/)"));
        assert!(md.contains("Evaluate expressions instantly."));
        assert!(md.contains("/tools/_index.json"));
    }
}

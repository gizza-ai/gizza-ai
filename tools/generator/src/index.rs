//! Build-time JSON index of all tool pages, consumed by the in-app tools modal.

use crate::meta::ToolMeta;
use serde::Serialize;

#[derive(Serialize)]
struct IndexEntry<'a> {
    slug: &'a str,
    title: &'a str,
    description: &'a str,
}

/// Serialize `[{slug,title,description}]` for every tool, in the given order.
pub fn tools_index_json(metas: &[ToolMeta]) -> String {
    let entries: Vec<IndexEntry> = metas
        .iter()
        .map(|m| IndexEntry {
            slug: &m.slug,
            title: &m.title,
            description: &m.description,
        })
        .collect();
    serde_json::to_string(&entries).expect("serialize tools index")
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
    }

    #[test]
    fn empty_metas_is_empty_array() {
        assert_eq!(tools_index_json(&[]), "[]");
    }
}

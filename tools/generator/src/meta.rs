//! Per-tool page metadata, parsed from `blocks/<tool>/page/meta.toml`.

use serde::Deserialize;

/// One input field for a tool page.
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct Input {
    pub name: String,
    /// "field" = a visible text input; "clock" = current unix seconds, supplied by JS.
    pub source: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub placeholder: String,
}

/// Full metadata for a single tool page.
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct ToolMeta {
    pub subdomain: String,
    pub title: String,
    pub description: String,
    pub h1: String,
    pub hero_subtitle: String,
    /// wasm-pack output basename (without extension), e.g. "gizza_ai_calculator_web".
    pub wasm: String,
    /// Exported function name to call.
    pub export: String,
    #[serde(default)]
    pub live: bool,
    #[serde(default)]
    pub interval_ms: Option<u64>,
    pub output_label: String,
    /// "number" or "text" — how to render the result.
    pub format: String,
    #[serde(default, rename = "input")]
    pub inputs: Vec<Input>,
}

impl ToolMeta {
    /// Parse from TOML text.
    pub fn from_toml(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|e| format!("meta.toml parse error: {e}"))
    }

    /// Build the `window.GIZZA_TOOL` config object consumed by `tool.js`.
    pub fn client_config(&self) -> serde_json::Value {
        let inputs: Vec<serde_json::Value> = self
            .inputs
            .iter()
            .map(|i| {
                serde_json::json!({
                    "name": i.name,
                    "source": i.source,
                    "elementId": format!("in-{}", i.name),
                })
            })
            .collect();
        serde_json::json!({
            "module": format!("./{}.js", self.wasm),
            "export": self.export,
            "live": self.live,
            "intervalMs": self.interval_ms,
            "inputs": inputs,
            "output": { "label": self.output_label, "elementId": "tool-output" },
            "format": self.format,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_calculator_meta() {
        let text = r#"
subdomain     = "calculator"
title         = "Free Online Calculator — gizza.ai"
description   = "desc"
h1            = "Free Online Calculator"
hero_subtitle = "sub"
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
"#;
        let m = ToolMeta::from_toml(text).unwrap();
        assert_eq!(m.subdomain, "calculator");
        assert_eq!(m.export, "evaluate");
        assert_eq!(m.inputs.len(), 1);
        assert_eq!(m.inputs[0].source, "field");
        assert!(!m.live);
    }

    #[test]
    fn parses_live_tool_without_inputs_fields() {
        let text = r#"
subdomain     = "clock"
title         = "t"
description   = "d"
h1            = "h"
hero_subtitle = "s"
wasm          = "gizza_ai_clock_web"
export        = "format_time"
live          = true
interval_ms   = 1000
output_label  = "Current time (UTC)"
format        = "text"

[[input]]
name   = "unix"
source = "clock"
"#;
        let m = ToolMeta::from_toml(text).unwrap();
        assert!(m.live);
        assert_eq!(m.interval_ms, Some(1000));
        assert_eq!(m.inputs[0].source, "clock");
    }

    #[test]
    fn builds_client_config_json() {
        let text = r#"
subdomain     = "calculator"
title         = "t"
description   = "d"
h1            = "h"
hero_subtitle = "s"
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
"#;
        let m = ToolMeta::from_toml(text).unwrap();
        let cfg = m.client_config();
        assert_eq!(cfg["module"], "./gizza_ai_calculator_web.js");
        assert_eq!(cfg["export"], "evaluate");
        assert_eq!(cfg["live"], false);
        assert_eq!(cfg["inputs"][0]["source"], "field");
        assert_eq!(cfg["output"]["label"], "Result");
        assert_eq!(cfg["format"], "number");
    }
}

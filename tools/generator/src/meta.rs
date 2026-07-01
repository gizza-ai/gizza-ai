//! Per-tool page metadata, parsed from `blocks/<tool>/page/meta.toml`.

use serde::Deserialize;

/// One input field for a tool page.
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct Input {
    pub name: String,
    /// "field" = a visible text input; "clock" = current unix seconds, supplied by JS;
    /// "file" = a file picker, for ffmpeg tools.
    pub source: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub placeholder: String,
    /// For source="file": the `accept` attribute (e.g. "image/*", "video/*").
    #[serde(default)]
    pub accept: String,
    /// For source="field": render a multi-line `<textarea>` instead of a
    /// single-line `<input>` (e.g. for batch/per-line input). Default false.
    #[serde(default)]
    pub multiline: bool,
    /// For source="field": force a native picker control — "date", "time", or
    /// "datetime-local". Overrides the schema-derived control (the descriptor
    /// only knows "string"). Empty = derive from the schema.
    #[serde(default)]
    pub kind: String,
    /// For source="field": the NAME of an option vocabulary (see `vocab.rs`,
    /// e.g. "timezones") rendered as a `<datalist>` for searchable autocomplete.
    #[serde(default)]
    pub options: String,
    /// For source="field": client-side default applied on load when the field is
    /// empty and not pre-filled from the URL. "today" → local YYYY-MM-DD, "now" →
    /// local YYYY-MM-DDTHH:MM, "local-timezone" → the user's IANA zone; anything
    /// else is used literally. Also what the Reset button restores.
    #[serde(default)]
    pub default: String,
}

/// A one-click example: a chip under the inputs that pre-fills `params` and runs.
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct Example {
    pub label: String,
    /// param name → value, applied via the same path as URL query pre-fill.
    #[serde(default)]
    pub params: std::collections::BTreeMap<String, String>,
}

/// Full metadata for a single tool page.
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct ToolMeta {
    pub slug: String,
    pub title: String,
    pub description: String,
    /// Extra search keywords for the tools modal (not displayed). Optional.
    #[serde(default)]
    pub tags: Vec<String>,
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
    /// "wasm" (pure compute, default) or "ffmpeg" (file in → media out via browser ffmpeg).
    #[serde(default = "default_runtime")]
    pub runtime: String,
    #[serde(default, rename = "input")]
    pub inputs: Vec<Input>,
    /// One-click example chips rendered under the inputs.
    #[serde(default, rename = "example")]
    pub examples: Vec<Example>,
    /// Widen the widget (multi-column result layouts). Renders as the
    /// `tool-widget--wide` class instead of a per-tool JS width hack.
    #[serde(default)]
    pub wide: bool,
}

fn default_runtime() -> String {
    "wasm".to_string()
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
                    "accept": i.accept,
                    "default": i.default,
                })
            })
            .collect();
        let examples: Vec<serde_json::Value> = self
            .examples
            .iter()
            .map(|e| serde_json::json!({ "label": e.label, "params": e.params }))
            .collect();
        serde_json::json!({
            "slug": self.slug,
            "module": format!("./{}.js", self.wasm),
            "export": self.export,
            "live": self.live,
            "intervalMs": self.interval_ms,
            "inputs": inputs,
            "examples": examples,
            "output": { "label": self.output_label, "elementId": "tool-output" },
            "format": self.format,
            "runtime": self.runtime,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_calculator_meta() {
        let text = r#"
slug          = "calculator"
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
        assert_eq!(m.slug, "calculator");
        assert_eq!(m.export, "evaluate");
        assert_eq!(m.inputs.len(), 1);
        assert_eq!(m.inputs[0].source, "field");
        assert!(!m.live);
    }

    #[test]
    fn parses_live_tool_without_inputs_fields() {
        let text = r#"
slug          = "clock"
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
slug          = "calculator"
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
        assert_eq!(cfg["slug"], "calculator");
        assert_eq!(cfg["module"], "./gizza_ai_calculator_web.js");
        assert_eq!(cfg["export"], "evaluate");
        assert_eq!(cfg["live"], false);
        assert_eq!(cfg["inputs"][0]["source"], "field");
        assert_eq!(cfg["output"]["label"], "Result");
        assert_eq!(cfg["format"], "number");
    }

    #[test]
    fn parses_ffmpeg_meta_with_file_input() {
        let text = r#"
slug          = "image-resize"
title         = "t"
description   = "d"
h1            = "h"
hero_subtitle = "s"
wasm          = "gizza_ai_image_resize_web"
export        = "build_argv"
runtime       = "ffmpeg"
output_label  = "Resized image"
format        = "image"

[[input]]
name   = "image"
source = "file"
accept = "image/*"
label  = "Image"

[[input]]
name   = "width"
source = "field"
label  = "Width (px)"
"#;
        let m = ToolMeta::from_toml(text).unwrap();
        assert_eq!(m.runtime, "ffmpeg");
        assert_eq!(m.inputs[0].source, "file");
        assert_eq!(m.inputs[0].accept, "image/*");
        let cfg = m.client_config();
        assert_eq!(cfg["runtime"], "ffmpeg");
        assert_eq!(cfg["inputs"][0]["accept"], "image/*");
        assert_eq!(cfg["format"], "image");
    }

    #[test]
    fn runtime_defaults_to_wasm() {
        let text = r#"
slug = "calculator"
title = "t"
description = "d"
h1 = "h"
hero_subtitle = "s"
wasm = "w"
export = "evaluate"
output_label = "Result"
format = "number"
"#;
        let m = ToolMeta::from_toml(text).unwrap();
        assert_eq!(m.runtime, "wasm");
    }
}

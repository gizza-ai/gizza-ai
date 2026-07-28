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
    /// For kind="slider": the slider's drag granularity (e.g. "1", "0.5").
    /// Empty = 1. The paired number box always accepts finer typed values.
    #[serde(default)]
    pub step: String,
    /// For source="field": client-side default applied on load when the field is
    /// empty and not pre-filled from the URL. "today" → local YYYY-MM-DD, "now" →
    /// local YYYY-MM-DDTHH:MM, "local-timezone" → the user's IANA zone; anything
    /// else is used literally. Also what the Reset button restores.
    #[serde(default)]
    pub default: String,
    /// For enum params (a `<select>`): optional display label per option VALUE
    /// (e.g. `"9:16" = "9:16 — Reels / Shorts (1080×1920)"`). The option VALUE
    /// stays the canonical schema string, so deep-links, chips, reset and the
    /// CLI are untouched; only the visible text is enriched. Unlisted options
    /// display their value.
    #[serde(default)]
    pub labels: std::collections::BTreeMap<String, String>,
}

/// A one-click example: a chip under the inputs that pre-fills `params` and runs.
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct Example {
    pub label: String,
    /// param name → value, applied via the same path as URL query pre-fill.
    #[serde(default)]
    pub params: std::collections::BTreeMap<String, String>,
}

/// Optional shared-waveform behavior. `waveform = false` disables the audio
/// waveform player on this page; `[waveform] start/end` names the two field
/// inputs the selection is two-way bound to (seconds). Absent = player
/// enabled, selection audition-only.
#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(untagged)]
pub enum WaveformSpec {
    Enabled(bool),
    Binding { start: String, end: String },
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
    /// How to render the result: "number", "text", "svg" (rendered as an
    /// <img> data URI, source still copyable), or the media formats
    /// "image"/"video"/"audio". Any other value renders as plain text.
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
    /// See [`WaveformSpec`]. Only audio-input ffmpeg tools consult this.
    #[serde(default)]
    pub waveform: Option<WaveformSpec>,
}

fn default_runtime() -> String {
    "wasm".to_string()
}

impl ToolMeta {
    /// Parse from TOML text.
    pub fn from_toml(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|e| format!("meta.toml parse error: {e}"))
    }

    /// Build the `window.GIZZA_TOOL` config object consumed by `tool.js`. The
    /// tool's declared param `schema` is threaded in so each `source="field"`
    /// input carries its JSON-schema numeric-ness (`"numeric": true` for
    /// `integer`/`number` params) — `tool.js` coerces field values to `Number`
    /// BY TYPE instead of sniffing whether a string looks numeric, so a
    /// digits-only string param (a bare hex color, a numeric-looking label)
    /// stays a string.
    pub fn client_config(&self, schema: &crate::control::ParamSchema) -> serde_json::Value {
        let inputs: Vec<serde_json::Value> = self
            .inputs
            .iter()
            .map(|i| {
                let mut obj = serde_json::json!({
                    "name": i.name,
                    "source": i.source,
                    "elementId": format!("in-{}", i.name),
                    "accept": i.accept,
                    "default": i.default,
                });
                if i.source == "field" && schema.is_numeric(&i.name) {
                    obj["numeric"] = serde_json::json!(true);
                }
                obj
            })
            .collect();
        let examples: Vec<serde_json::Value> = self
            .examples
            .iter()
            .map(|e| serde_json::json!({ "label": e.label, "params": e.params }))
            .collect();
        let waveform = match &self.waveform {
            None => serde_json::Value::Null,
            Some(WaveformSpec::Enabled(b)) => serde_json::json!(b),
            Some(WaveformSpec::Binding { start, end }) => {
                serde_json::json!({ "start": start, "end": end })
            }
        };
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
            "waveform": waveform,
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
title         = "Free Online Calculator"
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
        let cfg = m.client_config(&crate::control::ParamSchema::empty());
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
        let cfg = m.client_config(&crate::control::ParamSchema::empty());
        assert_eq!(cfg["runtime"], "ffmpeg");
        assert_eq!(cfg["inputs"][0]["accept"], "image/*");
        assert_eq!(cfg["format"], "image");
    }

    #[test]
    fn client_config_bakes_numeric_flag_by_schema_type() {
        // A number/integer field must carry "numeric":true; a string field and a
        // color-kind field (still a schema "string") must not — so tool.js
        // coerces by declared type, not by sniffing digit-looking strings.
        let text = r##"
slug          = "waveform-image"
title         = "t"
description   = "d"
h1            = "h"
hero_subtitle = "s"
wasm          = "w"
export        = "build_argv"
runtime       = "ffmpeg"
output_label  = "o"
format        = "image"

[[input]]
name   = "audio"
source = "file"
accept = "audio/*"

[[input]]
name        = "height"
source      = "field"
label       = "Height"

[[input]]
name        = "label"
source      = "field"
label       = "Label"

[[input]]
name        = "background"
source      = "field"
label       = "Background"
kind        = "color"
"##;
        let m = ToolMeta::from_toml(text).unwrap();
        let schema = crate::control::ParamSchema::from_props_for_tests(serde_json::json!({
            "height": { "type": "integer", "minimum": 1 },
            "label": { "type": "string" },
            "background": { "type": "string" }
        }));
        let cfg = m.client_config(&schema);
        let inputs = cfg["inputs"].as_array().unwrap();
        let by_name = |n: &str| inputs.iter().find(|i| i["name"] == n).unwrap();
        // file input never numeric
        assert!(!by_name("audio")["numeric"].as_bool().unwrap_or(false));
        // integer field → numeric:true
        assert_eq!(by_name("height")["numeric"], serde_json::json!(true));
        // string field → flag absent (falsy)
        assert!(by_name("label").get("numeric").is_none());
        assert!(!by_name("label")["numeric"].as_bool().unwrap_or(false));
        // color-kind field is a schema string → not numeric (stays a string,
        // so a bare hex like "112233" survives coercion)
        assert!(by_name("background").get("numeric").is_none());
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

    #[test]
    fn parses_waveform_binding_table() {
        let text = r#"
slug = "trim-audio"
title = "t"
description = "d"
h1 = "h"
hero_subtitle = "s"
wasm = "w"
export = "build_argv"
output_label = "o"
format = "audio"
runtime = "ffmpeg"

[waveform]
start = "start"
end = "end"
"#;
        let m = ToolMeta::from_toml(text).unwrap();
        assert_eq!(
            m.waveform,
            Some(WaveformSpec::Binding { start: "start".into(), end: "end".into() })
        );
        let cfg = m.client_config(&crate::control::ParamSchema::empty());
        assert_eq!(cfg["waveform"]["start"], "start");
        assert_eq!(cfg["waveform"]["end"], "end");
    }

    #[test]
    fn parses_waveform_opt_out_and_default() {
        let base = r#"
slug = "x"
title = "t"
description = "d"
h1 = "h"
hero_subtitle = "s"
wasm = "w"
export = "run"
output_label = "o"
format = "audio"
"#;
        let off = format!("{base}waveform = false\n");
        let m = ToolMeta::from_toml(&off).unwrap();
        assert_eq!(m.waveform, Some(WaveformSpec::Enabled(false)));
        assert_eq!(m.client_config(&crate::control::ParamSchema::empty())["waveform"], serde_json::json!(false));

        let m = ToolMeta::from_toml(base).unwrap();
        assert_eq!(m.waveform, None);
        assert_eq!(m.client_config(&crate::control::ParamSchema::empty())["waveform"], serde_json::Value::Null);
    }

    #[test]
    fn parses_waveform_true_and_partial_table_errors() {
        let base = r#"
slug = "x"
title = "t"
description = "d"
h1 = "h"
hero_subtitle = "s"
wasm = "w"
export = "run"
output_label = "o"
format = "audio"
"#;
        let on = format!("{base}waveform = true\n");
        let m = ToolMeta::from_toml(&on).unwrap();
        assert_eq!(m.waveform, Some(WaveformSpec::Enabled(true)));
        assert_eq!(m.client_config(&crate::control::ParamSchema::empty())["waveform"], serde_json::json!(true));

        // A partial [waveform] table (start without end) matches neither
        // untagged variant and must fail at parse time, not silently bind.
        let partial = format!("{base}\n[waveform]\nstart = \"start\"\n");
        assert!(
            ToolMeta::from_toml(&partial).is_err(),
            "partial [waveform] table must be a parse error"
        );
    }
}

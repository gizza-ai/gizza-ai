//! Page input control types, derived from the tool's declared schema
//! (`blocks/<tool>/manifest.json` → `tool.parameters`) so the page form never
//! duplicates the enum/bool/number facts that already live in the descriptor.
//!
//! A `source="field"` input renders as: `<select>` (param has an `enum`),
//! checkbox (`boolean`), number input (`integer`/`number`), or text/textarea
//! (`string` or — defensively — any param missing from the schema). The meta can
//! override a string param with a native picker (`kind = "date" | "time" |
//! "datetime-local"`) or a named `<datalist>` vocabulary (`options = "timezones"`)
//! — the descriptor only knows "string", so these page-level refinements live in
//! `page/meta.toml`.

use crate::meta::Input;
use crate::vocab;
use std::collections::HashMap;
use std::path::Path;

/// Render a numeric bound/default as an HTML/text value — whole numbers as
/// integers (`16`, not `16.0`).
pub fn fmt_num(x: f64) -> String {
    if x.fract() == 0.0 && x.is_finite() {
        format!("{}", x as i64)
    } else {
        format!("{x}")
    }
}

/// How one `source="field"` input renders on the page.
#[derive(Debug, Clone, PartialEq)]
pub enum Control {
    Text,
    Textarea,
    Number {
        min: Option<f64>,
        max: Option<f64>,
        default: Option<f64>,
        /// True for JSON-schema `number` (fractional) params → `step="any"`,
        /// so decimals like 0.5 don't trip the browser's step-mismatch
        /// validation. Integer params keep the default whole-number stepper.
        step_any: bool,
    },
    /// A range slider paired with the regular number box (meta `kind =
    /// "slider"`), for bounded numeric params where dragging beats typing.
    /// The number input keeps the canonical `in-<name>` id (deep-links, CLI
    /// parity, reset all unchanged); the slider mirrors it via the runtime's
    /// `tool.js`.
    Slider {
        min: f64,
        max: f64,
        default: Option<f64>,
        /// Slider drag granularity (meta `step`, default 1). The paired
        /// number box always accepts finer values (`step="any"`).
        step: String,
    },
    Select {
        options: Vec<String>,
        default: Option<String>,
    },
    Checkbox {
        default: bool,
    },
    /// A native picker: `<input type="date|time|datetime-local">` (meta `kind`).
    Picker {
        input_type: String,
    },
    /// A hex-color text input paired with a native `<input type="color">`
    /// swatch (meta `kind = "color"`). The TEXT input keeps the canonical
    /// `in-<name>` id — it stays the source of truth so empty ("transparent"/
    /// "use the default"), alpha hex (`#RRGGBBAA`) and multi-color lists stay
    /// expressible, which a bare native picker can't do. The swatch mirrors it
    /// two-way via the runtime's `tool.js`. `default` is the schema default hex, shown
    /// on the swatch at rest.
    Color {
        default: Option<String>,
    },
    /// A text input backed by a `<datalist>` of a named vocabulary (meta `options`).
    Datalist {
        options: Vec<String>,
    },
    /// A multi-value pill list (meta `kind = "tag-list"`): the real field is a
    /// hidden comma-joined input; pills + a search box (optionally backed by a
    /// vocabulary datalist) are wired generically by the runtime's `tool.js`.
    TagList {
        options: Vec<String>,
    },
}

/// Validate a `#RGB`/`#RGBA`/`#RRGGBB`/`#RRGGBBAA` hex color and expand it to
/// the 6-digit `#rrggbb` form a native `<input type="color">` accepts (alpha
/// digits are dropped — the swatch can't show alpha). `None` when `s` isn't a
/// single hex color (empty, named color, multi-color list, …).
pub fn expand_hex(s: &str) -> Option<String> {
    let v = s.trim();
    let digits = v.strip_prefix('#')?;
    if !matches!(digits.len(), 3 | 4 | 6 | 8) || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let full: String = if digits.len() <= 4 {
        digits.chars().flat_map(|c| [c, c]).collect()
    } else {
        digits.to_string()
    };
    Some(format!("#{}", full[..6].to_ascii_lowercase()))
}

/// One tool's param schema: param name → its JSON-schema property object,
/// plus the schema's `required` param names (in declared order — the CLI maps
/// bare positionals against exactly this list).
pub struct ParamSchema {
    props: HashMap<String, serde_json::Value>,
    required: Vec<String>,
}

impl ParamSchema {
    /// An empty schema — every field falls back to text/textarea. Used by tests
    /// and as the graceful default when a manifest is missing/unreadable.
    pub fn empty() -> Self {
        ParamSchema { props: HashMap::new(), required: Vec::new() }
    }

    /// Build a schema from a JSON `properties` object — test-only constructor
    /// for sibling modules (the real path goes through `load`).
    #[cfg(test)]
    pub(crate) fn from_props_for_tests(props: serde_json::Value) -> Self {
        ParamSchema {
            props: props.as_object().unwrap().clone().into_iter().collect(),
            required: Vec::new(),
        }
    }

    /// Mark params as required — test-only builder companion to
    /// `from_props_for_tests`.
    #[cfg(test)]
    pub(crate) fn with_required_for_tests(mut self, names: &[&str]) -> Self {
        self.required = names.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Read `<tool_dir>/manifest.json` and extract `tool.parameters`
    /// (`properties` + `required`). A missing or unparseable manifest yields an
    /// empty schema (no panic, no abort) so a tool with a stale manifest just
    /// keeps plain text inputs.
    pub fn load(tool_dir: &Path) -> Self {
        let params = std::fs::read_to_string(tool_dir.join("manifest.json"))
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            .and_then(|v| v.get("tool")?.get("parameters").cloned());
        let props = params
            .as_ref()
            .and_then(|p| p.get("properties")?.as_object().cloned())
            .map(|m| m.into_iter().collect())
            .unwrap_or_default();
        let required = params
            .as_ref()
            .and_then(|p| p.get("required")?.as_array().cloned())
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();
        ParamSchema { props, required }
    }

    /// Whether `name` is a required param. False for the empty/stale-manifest
    /// fallback schema (which doesn't know).
    pub fn is_required(&self, name: &str) -> bool {
        self.required.iter().any(|r| r == name)
    }

    /// Whether the schema knows any params at all (false → stale/missing
    /// manifest fallback, where `is_required` answers are meaningless).
    pub fn knows_params(&self) -> bool {
        !self.props.is_empty()
    }

    /// Whether `name`'s declared JSON-schema type is numeric (`integer` or
    /// `number`) — the page must marshal its field value as a JS `Number` for
    /// the wasm `f64` param. False for `string`/`boolean` params, for `enum`
    /// params (a `<select>` yields a canonical string value, even a
    /// digits-looking one), and for the empty/stale fallback schema (unknown →
    /// left a string). This is what lets `tool.js` coerce BY TYPE instead of
    /// sniffing whether a string looks numeric.
    pub fn is_numeric(&self, name: &str) -> bool {
        let Some(p) = self.props.get(name) else {
            return false;
        };
        // An enum is a string-valued <select> regardless of its JSON "type";
        // its value must never be coerced to a Number.
        if p.get("enum").is_some() {
            return false;
        }
        matches!(
            p.get("type").and_then(|t| t.as_str()),
            Some("integer" | "number")
        )
    }

    /// Resolve the control for a `source="field"` input: meta-level overrides
    /// (`kind`, `options`) win over the schema-derived control, because the
    /// schema only knows JSON types ("string") while the meta knows the page
    /// affordance (a date picker, a timezone autocomplete).
    pub fn control_for_input(&self, input: &Input) -> Control {
        match input.kind.as_str() {
            "" => {}
            k @ ("date" | "time" | "datetime-local") => {
                return Control::Picker { input_type: k.to_string() };
            }
            "color" => {
                // Text stays canonical (empty/alpha/lists work); the swatch
                // is a mirror. The schema default (if any) seeds the swatch.
                let default = self
                    .props
                    .get(&input.name)
                    .and_then(|p| p.get("default"))
                    .and_then(|d| d.as_str())
                    .map(String::from);
                return Control::Color { default };
            }
            "slider" => {
                // A slider needs real bounds; only a numeric param with both
                // min and max in the schema qualifies. Anything else falls back
                // to the schema control (with a warning) rather than rendering
                // an unbounded range input.
                if let Control::Number { min: Some(min), max: Some(max), default, .. } =
                    self.control_for(&input.name, false)
                {
                    let step = if input.step.is_empty() { "1".to_string() } else { input.step.clone() };
                    return Control::Slider { min, max, default, step };
                }
                eprintln!(
                    "warning: [[input]] \"{}\" kind=\"slider\" needs a numeric schema param with min+max — falling back to the schema control",
                    input.name
                );
                return self.control_for(&input.name, input.multiline);
            }
            "tag-list" => {
                let options: Vec<String> = vocab::options(&input.options)
                    .map(|opts| opts.iter().map(|s| s.to_string()).collect())
                    .unwrap_or_default();
                if !input.options.is_empty() && options.is_empty() {
                    eprintln!(
                        "warning: [[input]] \"{}\" tag-list names unknown options vocabulary \"{}\" — tags will be free-text",
                        input.name, input.options
                    );
                }
                return Control::TagList { options };
            }
            other => {
                eprintln!(
                    "warning: [[input]] \"{}\" has unknown kind \"{other}\" (want date|time|datetime-local|color|tag-list|slider) — falling back to the schema control",
                    input.name
                );
            }
        }
        if !input.options.is_empty() {
            match vocab::options(&input.options) {
                Some(opts) => {
                    return Control::Datalist {
                        options: opts.iter().map(|s| s.to_string()).collect(),
                    };
                }
                None => eprintln!(
                    "warning: [[input]] \"{}\" names unknown options vocabulary \"{}\" (see tools/generator/src/vocab.rs) — falling back to the schema control",
                    input.name, input.options
                ),
            }
        }
        self.control_for(&input.name, input.multiline)
    }

    /// Resolve the schema-derived control for field `name`. `multiline` only
    /// matters for a string/unknown param (textarea vs single-line text).
    pub fn control_for(&self, name: &str, multiline: bool) -> Control {
        let text_or_area = || {
            if multiline {
                Control::Textarea
            } else {
                Control::Text
            }
        };
        let Some(p) = self.props.get(name) else {
            return text_or_area();
        };
        // An enum is a <select> regardless of its JSON "type" ("string").
        if let Some(variants) = p.get("enum").and_then(|e| e.as_array()) {
            return Control::Select {
                options: variants
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect(),
                default: p
                    .get("default")
                    .and_then(|d| d.as_str())
                    .map(String::from),
            };
        }
        match p.get("type").and_then(|t| t.as_str()) {
            Some("boolean") => Control::Checkbox {
                default: p.get("default").and_then(|d| d.as_bool()).unwrap_or(false),
            },
            Some(t @ ("integer" | "number")) => Control::Number {
                min: p.get("minimum").and_then(|m| m.as_f64()),
                max: p.get("maximum").and_then(|m| m.as_f64()),
                default: p.get("default").and_then(|d| d.as_f64()),
                step_any: t == "number",
            },
            _ => text_or_area(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn schema(props: serde_json::Value) -> ParamSchema {
        ParamSchema::from_props_for_tests(props)
    }

    #[test]
    fn enum_param_is_a_select_with_options_and_default() {
        let s = schema(json!({
            "target": { "type": "string", "enum": ["component", "uri", "form"], "default": "component" }
        }));
        assert_eq!(
            s.control_for("target", false),
            Control::Select {
                options: vec!["component".into(), "uri".into(), "form".into()],
                default: Some("component".into()),
            }
        );
    }

    #[test]
    fn boolean_param_is_a_checkbox() {
        let s = schema(json!({ "per_line": { "type": "boolean", "default": false } }));
        assert_eq!(
            s.control_for("per_line", false),
            Control::Checkbox { default: false }
        );
    }

    #[test]
    fn integer_param_is_a_number_with_bounds() {
        let s = schema(json!({
            "repeat": { "type": "integer", "minimum": 1, "maximum": 16, "default": 1 }
        }));
        assert_eq!(
            s.control_for("repeat", false),
            Control::Number {
                min: Some(1.0),
                max: Some(16.0),
                default: Some(1.0),
                step_any: false,
            }
        );
    }

    #[test]
    fn fractional_number_param_gets_step_any() {
        let s = schema(json!({
            "semitones": { "type": "number", "minimum": -24, "maximum": 24 }
        }));
        assert_eq!(
            s.control_for("semitones", false),
            Control::Number {
                min: Some(-24.0),
                max: Some(24.0),
                default: None,
                step_any: true,
            }
        );
    }

    #[test]
    fn slider_kind_pairs_range_with_number_when_bounded() {
        let s = schema(json!({
            "semitones": { "type": "number", "minimum": -24, "maximum": 24 }
        }));
        let mut f = field("semitones", "slider", "");
        assert_eq!(
            s.control_for_input(&f),
            Control::Slider {
                min: -24.0,
                max: 24.0,
                default: None,
                step: "1".into(),
            }
        );
        f.step = "0.5".into();
        assert_eq!(
            s.control_for_input(&f),
            Control::Slider {
                min: -24.0,
                max: 24.0,
                default: None,
                step: "0.5".into(),
            }
        );
    }

    #[test]
    fn slider_kind_without_bounds_falls_back_to_schema_control() {
        // No min/max in the schema → an unbounded range input would be
        // meaningless; fall back to the plain number box.
        let s = schema(json!({ "gain": { "type": "number" } }));
        assert_eq!(
            s.control_for_input(&field("gain", "slider", "")),
            Control::Number { min: None, max: None, default: None, step_any: true }
        );
        // Non-numeric param → schema control (text).
        let s = schema(json!({ "mode": { "type": "string" } }));
        assert_eq!(s.control_for_input(&field("mode", "slider", "")), Control::Text);
    }

    #[test]
    fn string_param_is_text_or_textarea_by_multiline() {
        let s = schema(json!({ "text": { "type": "string" } }));
        assert_eq!(s.control_for("text", false), Control::Text);
        assert_eq!(s.control_for("text", true), Control::Textarea);
    }

    #[test]
    fn unknown_param_falls_back_gracefully() {
        let s = ParamSchema::empty();
        assert_eq!(s.control_for("whatever", false), Control::Text);
        assert_eq!(s.control_for("whatever", true), Control::Textarea);
    }

    fn field(name: &str, kind: &str, options: &str) -> Input {
        Input {
            name: name.into(),
            source: "field".into(),
            label: String::new(),
            placeholder: String::new(),
            accept: String::new(),
            multiline: false,
            kind: kind.into(),
            options: options.into(),
            step: String::new(),
            default: String::new(),
            labels: Default::default(),
        }
    }

    #[test]
    fn meta_kind_overrides_schema_with_a_native_picker() {
        let s = schema(json!({ "birthdate": { "type": "string" } }));
        assert_eq!(
            s.control_for_input(&field("birthdate", "date", "")),
            Control::Picker { input_type: "date".into() }
        );
        assert_eq!(
            s.control_for_input(&field("at", "datetime-local", "")),
            Control::Picker { input_type: "datetime-local".into() }
        );
    }

    #[test]
    fn named_options_render_a_datalist() {
        let s = schema(json!({ "from": { "type": "string" } }));
        match s.control_for_input(&field("from", "", "timezones")) {
            Control::Datalist { options } => {
                assert!(options.iter().any(|o| o == "UTC"));
                assert!(options.iter().any(|o| o == "Europe/Amsterdam"));
            }
            other => panic!("expected Datalist, got {other:?}"),
        }
    }

    #[test]
    fn tag_list_kind_renders_a_tag_list_with_vocab_options() {
        let s = schema(json!({ "to": { "type": "string" } }));
        match s.control_for_input(&{
            let mut f = field("to", "tag-list", "timezones");
            f.multiline = false;
            f
        }) {
            Control::TagList { options } => assert!(options.iter().any(|o| o == "UTC")),
            other => panic!("expected TagList, got {other:?}"),
        }
        // free-text tag list (no vocabulary)
        assert_eq!(
            s.control_for_input(&field("to", "tag-list", "")),
            Control::TagList { options: vec![] }
        );
    }

    #[test]
    fn unknown_kind_and_vocab_fall_back_to_schema_control() {
        let s = schema(json!({ "x": { "type": "string" } }));
        assert_eq!(s.control_for_input(&field("x", "range", "")), Control::Text);
        assert_eq!(s.control_for_input(&field("x", "", "nope")), Control::Text);
    }

    #[test]
    fn color_kind_pairs_swatch_with_text_and_takes_schema_default() {
        let s = schema(json!({ "color": { "type": "string", "default": "#4f46e5" } }));
        assert_eq!(
            s.control_for_input(&field("color", "color", "")),
            Control::Color { default: Some("#4f46e5".into()) }
        );
        // No schema default (e.g. "empty = transparent") → swatch has no rest value.
        let s = schema(json!({ "background": { "type": "string" } }));
        assert_eq!(
            s.control_for_input(&field("background", "color", "")),
            Control::Color { default: None }
        );
    }

    #[test]
    fn expand_hex_normalizes_short_and_alpha_forms_for_the_swatch() {
        assert_eq!(expand_hex("#f00").as_deref(), Some("#ff0000"));
        assert_eq!(expand_hex(" #ABC ").as_deref(), Some("#aabbcc"));
        assert_eq!(expand_hex("#f008").as_deref(), Some("#ff0000")); // alpha dropped
        assert_eq!(expand_hex("#4f46e5").as_deref(), Some("#4f46e5"));
        assert_eq!(expand_hex("#00000080").as_deref(), Some("#000000"));
        for bad in ["", "red", "#12345", "#ff0000,#0000ff", "4f46e5"] {
            assert_eq!(expand_hex(bad), None, "{bad}");
        }
    }

    #[test]
    fn is_numeric_is_true_only_for_integer_and_number_types() {
        let s = schema(json!({
            "width":    { "type": "integer" },
            "semitones":{ "type": "number" },
            "color":    { "type": "string" },
            "flag":     { "type": "boolean" },
            "mode":     { "type": "string", "enum": ["a", "b"] }
        }));
        assert!(s.is_numeric("width"), "integer param is numeric");
        assert!(s.is_numeric("semitones"), "number param is numeric");
        assert!(!s.is_numeric("color"), "string param is not numeric");
        assert!(!s.is_numeric("flag"), "boolean param is not numeric");
        assert!(!s.is_numeric("mode"), "enum <select> value stays a string");
        // Unknown / stale-manifest fallback: never coerce.
        assert!(!s.is_numeric("absent"), "unknown param is not numeric");
        assert!(!ParamSchema::empty().is_numeric("width"), "empty schema is not numeric");
    }

    #[test]
    fn without_overrides_meta_input_uses_schema_control() {
        let s = schema(json!({ "mode": { "type": "string", "enum": ["a", "b"] } }));
        match s.control_for_input(&field("mode", "", "")) {
            Control::Select { options, .. } => assert_eq!(options, vec!["a", "b"]),
            other => panic!("expected Select, got {other:?}"),
        }
    }
}

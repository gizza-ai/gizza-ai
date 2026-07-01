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
    /// A text input backed by a `<datalist>` of a named vocabulary (meta `options`).
    Datalist {
        options: Vec<String>,
    },
    /// A multi-value pill list (meta `kind = "tag-list"`): the real field is a
    /// hidden comma-joined input; pills + a search box (optionally backed by a
    /// vocabulary datalist) are wired generically by `site/tool.js`.
    TagList {
        options: Vec<String>,
    },
}

/// One tool's param schema: param name → its JSON-schema property object.
pub struct ParamSchema(HashMap<String, serde_json::Value>);

impl ParamSchema {
    /// An empty schema — every field falls back to text/textarea. Used by tests
    /// and as the graceful default when a manifest is missing/unreadable.
    pub fn empty() -> Self {
        ParamSchema(HashMap::new())
    }

    /// Read `<tool_dir>/manifest.json` and extract `tool.parameters.properties`.
    /// A missing or unparseable manifest yields an empty schema (no panic, no
    /// abort) so a tool with a stale manifest just keeps plain text inputs.
    pub fn load(tool_dir: &Path) -> Self {
        let props = std::fs::read_to_string(tool_dir.join("manifest.json"))
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            .and_then(|v| {
                v.get("tool")?
                    .get("parameters")?
                    .get("properties")?
                    .as_object()
                    .cloned()
            })
            .map(|m| m.into_iter().collect())
            .unwrap_or_default();
        ParamSchema(props)
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
                    "warning: [[input]] \"{}\" has unknown kind \"{other}\" (want date|time|datetime-local|tag-list) — falling back to the schema control",
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
        let Some(p) = self.0.get(name) else {
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
            Some("integer") | Some("number") => Control::Number {
                min: p.get("minimum").and_then(|m| m.as_f64()),
                max: p.get("maximum").and_then(|m| m.as_f64()),
                default: p.get("default").and_then(|d| d.as_f64()),
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
        ParamSchema(props.as_object().unwrap().clone().into_iter().collect())
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
            }
        );
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
            default: String::new(),
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
        assert_eq!(s.control_for_input(&field("x", "color", "")), Control::Text);
        assert_eq!(s.control_for_input(&field("x", "", "nope")), Control::Text);
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

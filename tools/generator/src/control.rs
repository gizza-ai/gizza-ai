//! Page input control types, derived from the tool's declared schema
//! (`blocks/<tool>/manifest.json` → `tool.parameters`) so the page form never
//! duplicates the enum/bool/number facts that already live in the descriptor.
//!
//! A `source="field"` input renders as: `<select>` (param has an `enum`),
//! checkbox (`boolean`), number input (`integer`/`number`), or text/textarea
//! (`string` or — defensively — any param missing from the schema).

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

    /// Resolve the control for field `name`. `multiline` only matters for a
    /// string/unknown param (textarea vs single-line text).
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
}

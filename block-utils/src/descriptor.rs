//! Single-source tool descriptor. One declaration per tool (in its `core`
//! crate) from which the chat schema, page form, `build_argv` keying, and the
//! URL query-param contract are all derived — see
//! docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.

use serde::{Deserialize, Serialize};

/// The binary/remote input a tool consumes. Varies by surface: chat/CLI take
/// `url`⊕`ref`, the page takes a file upload or `?url=`. Plain text is a
/// `String` [`Param`], not an `Input`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Input {
    None,
    Image,
    Video,
    Document,
    File,
}

/// A logical parameter's type. Numeric bounds live on [`Param::minimum`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParamKind {
    String,
    Integer,
    Number,
    Enum(Vec<String>),
    Bool,
    /// A JSON object whose values are strings — renders as
    /// `{ "type": "object", "additionalProperties": { "type": "string" } }`
    /// (e.g. an HTTP `headers` / `query` name→value map).
    StringMap,
}

/// One logical parameter. `name` is the chat-schema property name, the page
/// field name, AND the URL query-param name (single source, no drift).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Param {
    pub name: String,
    pub kind: ParamKind,
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum: Option<f64>,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    #[serde(default)]
    pub multiline: bool,
}

impl Param {
    fn new(name: &str, kind: ParamKind) -> Self {
        Param {
            name: name.to_string(),
            kind,
            required: false,
            default: None,
            minimum: None,
            maximum: None,
            description: String::new(),
            label: None,
            placeholder: None,
            multiline: false,
        }
    }
    pub fn string(name: &str) -> Self {
        Self::new(name, ParamKind::String)
    }
    pub fn integer(name: &str) -> Self {
        Self::new(name, ParamKind::Integer)
    }
    pub fn number(name: &str) -> Self {
        Self::new(name, ParamKind::Number)
    }
    pub fn boolean(name: &str) -> Self {
        Self::new(name, ParamKind::Bool)
    }
    /// A `{ name: value }` object with string values (e.g. HTTP headers/query).
    pub fn string_map(name: &str) -> Self {
        Self::new(name, ParamKind::StringMap)
    }
    pub fn enumv<const N: usize>(name: &str, variants: [&str; N]) -> Self {
        Self::new(
            name,
            ParamKind::Enum(variants.iter().map(|s| s.to_string()).collect()),
        )
    }
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }
    pub fn default(mut self, v: impl Into<serde_json::Value>) -> Self {
        self.default = Some(v.into());
        self
    }
    pub fn min(mut self, n: f64) -> Self {
        self.minimum = Some(n);
        self
    }
    pub fn max(mut self, n: f64) -> Self {
        self.maximum = Some(n);
        self
    }
    pub fn describe(mut self, s: &str) -> Self {
        self.description = s.to_string();
        self
    }
    pub fn label(mut self, s: &str) -> Self {
        self.label = Some(s.to_string());
        self
    }
    pub fn placeholder(mut self, s: &str) -> Self {
        self.placeholder = Some(s.to_string());
        self
    }
    pub fn multiline(mut self) -> Self {
        self.multiline = true;
        self
    }
}

/// One declaration per tool. See module docs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolDescriptor {
    pub input: Input,
    pub params: Vec<Param>,
}

impl ToolDescriptor {
    pub fn new(input: Input) -> Self {
        ToolDescriptor {
            input,
            params: Vec::new(),
        }
    }
    pub fn param(mut self, p: Param) -> Self {
        self.params.push(p);
        self
    }

    /// Render the chat-schema JSON for `#[wafer_block(skill(parameters = …))]`.
    /// Single source: properties/enums/defaults/required and the media
    /// `url`⊕`ref` `oneOf` are all derived from this descriptor.
    pub fn to_schema_json(&self) -> String {
        use serde_json::{json, Map, Value};

        let mut properties = Map::new();

        // Media/file/document tools take exactly one of `url` / `ref`.
        let media_label = match self.input {
            Input::Image => Some("Image"),
            Input::Video => Some("Video"),
            Input::Document => Some("Document"),
            Input::File => Some("File"),
            Input::None => None,
        };
        if let Some(label) = media_label {
            properties.insert(
                "url".into(),
                json!({ "type": "string",
                        "description": format!("{label} URL (HTTP/HTTPS). Use either url or ref.") }),
            );
            properties.insert(
                "ref".into(),
                json!({ "type": "string",
                        "description": "Reference id from a prior tool call. Use either url or ref." }),
            );
        }

        let mut required: Vec<Value> = Vec::new();
        for p in &self.params {
            let mut prop = Map::new();
            match &p.kind {
                ParamKind::String => {
                    prop.insert("type".into(), json!("string"));
                }
                ParamKind::Integer => {
                    prop.insert("type".into(), json!("integer"));
                }
                ParamKind::Number => {
                    prop.insert("type".into(), json!("number"));
                }
                ParamKind::Bool => {
                    prop.insert("type".into(), json!("boolean"));
                }
                ParamKind::Enum(variants) => {
                    prop.insert("type".into(), json!("string"));
                    prop.insert("enum".into(), json!(variants));
                }
                ParamKind::StringMap => {
                    prop.insert("type".into(), json!("object"));
                    prop.insert("additionalProperties".into(), json!({ "type": "string" }));
                }
            }
            if let Some(m) = p.minimum {
                // Render whole-number bounds as JSON integers (`1`, not `1.0`)
                // so an integer param's `minimum` matches what hand-authored
                // schemas wrote and what `ParamKind::Integer` implies. `Param`
                // stores `minimum` as f64; without this, every integer min
                // would serialize as a float and drift from the authored schema.
                if m.fract() == 0.0 && m.is_finite() {
                    prop.insert("minimum".into(), json!(m as i64));
                } else {
                    prop.insert("minimum".into(), json!(m));
                }
            }
            if let Some(m) = p.maximum {
                // Same whole-number-as-integer rendering as `minimum`.
                if m.fract() == 0.0 && m.is_finite() {
                    prop.insert("maximum".into(), json!(m as i64));
                } else {
                    prop.insert("maximum".into(), json!(m));
                }
            }
            if let Some(d) = &p.default {
                prop.insert("default".into(), d.clone());
            }
            if !p.description.is_empty() {
                prop.insert("description".into(), json!(p.description));
            }
            properties.insert(p.name.clone(), Value::Object(prop));
            if p.required {
                required.push(json!(p.name));
            }
        }

        let mut schema = Map::new();
        schema.insert("type".into(), json!("object"));
        schema.insert("properties".into(), Value::Object(properties));
        // Tool schemas reject unknown params uniformly (hardening for LLM tool
        // calls). Some hand-written schemas had this, some didn't — the
        // descriptor makes it consistent.
        schema.insert("additionalProperties".into(), json!(false));
        if !required.is_empty() {
            schema.insert("required".into(), Value::Array(required));
        }
        if media_label.is_some() {
            schema.insert(
                "oneOf".into(),
                json!([{ "required": ["url"] }, { "required": ["ref"] }]),
            );
        }
        Value::Object(schema).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_descriptor_with_typed_params() {
        let d = ToolDescriptor::new(Input::Image)
            .param(Param::integer("width").min(1.0).label("Width (px)"))
            .param(
                Param::enumv("fit", ["contain", "cover", "stretch"])
                    .default("contain")
                    .describe("How to fit."),
            );
        assert_eq!(d.input, Input::Image);
        assert_eq!(d.params.len(), 2);
        assert_eq!(d.params[0].name, "width");
        assert_eq!(d.params[0].kind, ParamKind::Integer);
        assert_eq!(d.params[0].minimum, Some(1.0));
        assert_eq!(d.params[1].default, Some(serde_json::json!("contain")));
        assert_eq!(
            d.params[1].kind,
            ParamKind::Enum(vec!["contain".into(), "cover".into(), "stretch".into()])
        );
    }

    #[test]
    fn descriptor_round_trips_through_json() {
        // The generator (Plan 3) reads an emitted descriptor.json — serde must
        // round-trip losslessly.
        let d = ToolDescriptor::new(Input::None)
            .param(Param::string("expression").required().placeholder("2 + 2"));
        let json = serde_json::to_string(&d).expect("serialize");
        let back: ToolDescriptor = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(d, back);
    }

    #[test]
    fn schema_for_pure_text_tool() {
        // calculator: Input::None + one required string param.
        let d = ToolDescriptor::new(Input::None).param(
            Param::string("expression")
                .required()
                .describe("The expression."),
        );
        let v: serde_json::Value =
            serde_json::from_str(&d.to_schema_json()).expect("valid JSON schema");
        assert_eq!(v["type"], "object");
        assert_eq!(v["properties"]["expression"]["type"], "string");
        assert_eq!(
            v["properties"]["expression"]["description"],
            "The expression."
        );
        assert_eq!(v["required"], serde_json::json!(["expression"]));
        assert_eq!(
            v["additionalProperties"], false,
            "schemas reject unknown params"
        );
        assert!(v.get("oneOf").is_none(), "no url/ref oneOf for Input::None");
    }

    #[test]
    fn schema_for_media_tool_has_url_ref_oneof_and_typed_params() {
        // image-resize: Input::Image + optional integer(min) + enum(default).
        let d = ToolDescriptor::new(Input::Image)
            .param(
                Param::integer("width")
                    .min(1.0)
                    .describe("Target width in pixels."),
            )
            .param(
                Param::integer("height")
                    .min(1.0)
                    .describe("Target height in pixels."),
            )
            .param(
                Param::enumv("fit", ["contain", "cover", "stretch"])
                    .default("contain")
                    .describe("Resize mode."),
            );
        let v: serde_json::Value =
            serde_json::from_str(&d.to_schema_json()).expect("valid JSON schema");
        // url/ref properties + exclusive oneOf.
        assert_eq!(v["properties"]["url"]["type"], "string");
        assert_eq!(v["properties"]["ref"]["type"], "string");
        assert_eq!(
            v["oneOf"],
            serde_json::json!([{ "required": ["url"] }, { "required": ["ref"] }])
        );
        // typed params.
        assert_eq!(v["properties"]["width"]["type"], "integer");
        // Whole-number bounds render as JSON integers, not floats.
        assert_eq!(v["properties"]["width"]["minimum"], 1);
        assert_eq!(
            v["properties"]["fit"]["enum"],
            serde_json::json!(["contain", "cover", "stretch"])
        );
        assert_eq!(v["properties"]["fit"]["default"], "contain");
        // optional params => no top-level required.
        assert!(v.get("required").is_none());
    }

    #[test]
    fn schema_renders_maximum_and_string_map() {
        let d = ToolDescriptor::new(Input::None)
            .param(Param::integer("quality").min(1.0).max(100.0).describe("Q"))
            .param(Param::string_map("headers").describe("H"));
        let v: serde_json::Value =
            serde_json::from_str(&d.to_schema_json()).expect("valid JSON schema");
        assert_eq!(v["properties"]["quality"]["minimum"], 1);
        assert_eq!(v["properties"]["quality"]["maximum"], 100);
        assert_eq!(v["properties"]["headers"]["type"], "object");
        assert_eq!(
            v["properties"]["headers"]["additionalProperties"]["type"],
            "string"
        );
    }
}

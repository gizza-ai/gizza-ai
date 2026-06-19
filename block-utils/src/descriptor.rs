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
}

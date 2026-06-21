//! gizza-ai/json-yaml-convert core — convert config data between JSON, YAML, and
//! TOML in any direction. Pure-Rust: parse the source into a universal
//! `serde_json::Value`, then serialize to the target format.

use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Json,
    Yaml,
    Toml,
}

impl Format {
    pub fn parse(s: &str) -> Result<Format, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "json" => Ok(Format::Json),
            "yaml" | "yml" => Ok(Format::Yaml),
            "toml" => Ok(Format::Toml),
            other => Err(format!("unknown format '{other}' (use json, yaml, or toml)")),
        }
    }
    pub fn name(&self) -> &'static str {
        match self {
            Format::Json => "json",
            Format::Yaml => "yaml",
            Format::Toml => "toml",
        }
    }
}

/// Convert `input` from `from` to `to`. `pretty` indents JSON output.
pub fn convert(input: &str, from: Format, to: Format, pretty: bool) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err("no input".into());
    }
    // Parse source → universal Value.
    let value: Value = match from {
        Format::Json => serde_json::from_str(input).map_err(|e| format!("invalid JSON: {e}"))?,
        Format::Yaml => serde_yml::from_str(input).map_err(|e| format!("invalid YAML: {e}"))?,
        Format::Toml => toml::from_str(input).map_err(|e| format!("invalid TOML: {e}"))?,
    };

    // Serialize to target.
    match to {
        Format::Json => {
            if pretty {
                serde_json::to_string_pretty(&value).map_err(|e| format!("JSON encode: {e}"))
            } else {
                serde_json::to_string(&value).map_err(|e| format!("JSON encode: {e}"))
            }
        }
        Format::Yaml => serde_yml::to_string(&value).map_err(|e| format!("YAML encode: {e}")),
        Format::Toml => {
            // TOML requires a table at the top level and has no null.
            if !value.is_object() {
                return Err("TOML output requires a top-level object/table".into());
            }
            if pretty {
                toml::to_string_pretty(&value).map_err(|e| format!("TOML encode: {e} (note: TOML has no null and needs a table root)"))
            } else {
                toml::to_string(&value).map_err(|e| format!("TOML encode: {e} (note: TOML has no null and needs a table root)"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_to_yaml() {
        let out = convert(r#"{"name":"gizza","tags":["a","b"]}"#, Format::Json, Format::Yaml, false).unwrap();
        assert!(out.contains("name: gizza"));
        assert!(out.contains("- a"));
    }

    #[test]
    fn yaml_to_json() {
        let out = convert("name: gizza\ncount: 3\n", Format::Yaml, Format::Json, false).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["name"], "gizza");
        assert_eq!(v["count"], 3);
    }

    #[test]
    fn json_to_toml() {
        let out = convert(r#"{"title":"x","owner":{"name":"me"}}"#, Format::Json, Format::Toml, false).unwrap();
        assert!(out.contains("title = \"x\""));
        assert!(out.contains("[owner]"));
        assert!(out.contains("name = \"me\""));
    }

    #[test]
    fn toml_to_json() {
        let out = convert("title = \"x\"\n[server]\nport = 8080\n", Format::Toml, Format::Json, false).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["title"], "x");
        assert_eq!(v["server"]["port"], 8080);
    }

    #[test]
    fn toml_to_yaml_roundtrip_values() {
        let out = convert("a = 1\nb = true\n", Format::Toml, Format::Yaml, false).unwrap();
        assert!(out.contains("a: 1"));
        assert!(out.contains("b: true"));
    }

    #[test]
    fn toml_output_requires_table() {
        // a top-level array can't be TOML
        assert!(convert("[1,2,3]", Format::Json, Format::Toml, false).is_err());
    }

    #[test]
    fn errors() {
        assert!(convert("", Format::Json, Format::Yaml, false).is_err());
        assert!(convert("{bad}", Format::Json, Format::Yaml, false).is_err());
        assert!(Format::parse("xml").is_err());
    }
}

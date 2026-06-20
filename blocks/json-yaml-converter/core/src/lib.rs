//! gizza-ai/json-yaml-converter core — convert between JSON and YAML in either
//! direction. No wafer/wasm-bindgen deps. Uses serde_json::Value as the common
//! data model: serde_yml deserializes/serializes it for YAML, serde_json for JSON.

use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    JsonToYaml,
    YamlToJson,
}

/// Resolve `auto`/explicit direction against the input. `auto` treats text whose
/// first non-space char is `{` or `[` as JSON (→ YAML); otherwise YAML (→ JSON).
pub fn resolve_direction(spec: &str, input: &str) -> Result<Direction, String> {
    match spec.trim().to_ascii_lowercase().as_str() {
        "json-to-yaml" | "json2yaml" | "j2y" => Ok(Direction::JsonToYaml),
        "yaml-to-json" | "yaml2json" | "y2j" => Ok(Direction::YamlToJson),
        "" | "auto" => {
            let t = input.trim_start();
            if t.starts_with('{') || t.starts_with('[') {
                Ok(Direction::JsonToYaml)
            } else {
                Ok(Direction::YamlToJson)
            }
        }
        other => Err(format!(
            "direction {other:?} not supported (auto|json-to-yaml|yaml-to-json)"
        )),
    }
}

/// Convert `input` per `direction`. For YAML→JSON, `pretty` indents the JSON.
pub fn convert(input: &str, direction: Direction, pretty: bool) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err("input is empty".into());
    }
    match direction {
        Direction::JsonToYaml => {
            let value: Value =
                serde_json::from_str(input).map_err(|e| format!("invalid JSON: {e}"))?;
            serde_yml::to_string(&value).map_err(|e| format!("YAML serialization failed: {e}"))
        }
        Direction::YamlToJson => {
            let value: Value =
                serde_yml::from_str(input).map_err(|e| format!("invalid YAML: {e}"))?;
            if pretty {
                serde_json::to_string_pretty(&value)
            } else {
                serde_json::to_string(&value)
            }
            .map_err(|e| format!("JSON serialization failed: {e}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_to_yaml() {
        let y = convert(r#"{"name":"Ada","tags":["a","b"]}"#, Direction::JsonToYaml, false).unwrap();
        assert!(y.contains("name: Ada"), "got:\n{y}");
        assert!(y.contains("- a"));
        assert!(y.contains("- b"));
    }

    #[test]
    fn yaml_to_json_compact() {
        let j = convert("name: Ada\nage: 30\n", Direction::YamlToJson, false).unwrap();
        let v: Value = serde_json::from_str(&j).unwrap();
        assert_eq!(v["name"], "Ada");
        assert_eq!(v["age"], 30);
    }

    #[test]
    fn yaml_to_json_pretty_has_newlines() {
        let j = convert("a: 1\nb: 2\n", Direction::YamlToJson, true).unwrap();
        assert!(j.contains('\n'), "pretty JSON should be multi-line: {j}");
    }

    #[test]
    fn round_trip_json_yaml_json() {
        let original = r#"{"n":1,"list":[true,null,"x"],"obj":{"k":"v"}}"#;
        let yaml = convert(original, Direction::JsonToYaml, false).unwrap();
        let back = convert(&yaml, Direction::YamlToJson, false).unwrap();
        let a: Value = serde_json::from_str(original).unwrap();
        let b: Value = serde_json::from_str(&back).unwrap();
        assert_eq!(a, b, "round-trip must preserve data");
    }

    #[test]
    fn auto_detects_direction() {
        assert_eq!(resolve_direction("auto", "{\"a\":1}").unwrap(), Direction::JsonToYaml);
        assert_eq!(resolve_direction("auto", "[1,2]").unwrap(), Direction::JsonToYaml);
        assert_eq!(resolve_direction("auto", "a: 1").unwrap(), Direction::YamlToJson);
        assert_eq!(resolve_direction("", "key: value").unwrap(), Direction::YamlToJson);
    }

    #[test]
    fn errors() {
        assert!(convert("", Direction::JsonToYaml, false).is_err());
        assert!(convert("{not json}", Direction::JsonToYaml, false).is_err());
        assert!(convert("key: : : bad", Direction::YamlToJson, false).is_err());
        assert!(resolve_direction("sideways", "x").is_err());
    }
}

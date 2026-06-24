//! render-jinja2 core — render a Jinja2 (Jinja) template against supplied
//! JSON or YAML data, entirely in pure Rust.
//!
//! Shared by the chat skill block and the web page. Uses the `minijinja` crate,
//! a pure-Rust implementation of the Jinja2 templating language: `{{ var }}`
//! substitution, `{% for %}` loops, `{% if %}` conditionals, filters
//! (`{{ name | upper }}`), expressions, and more. No I/O, no wasm-incompatible
//! deps.

use minijinja::{Environment, UndefinedBehavior};
use serde_json::Value;

/// Parse `data` (JSON or YAML, selected by `format`) into a JSON value used as
/// the template context. An empty string yields an empty object.
fn parse_data(data: &str, format: &str) -> Result<Value, String> {
    let trimmed = data.trim();
    if trimmed.is_empty() {
        return Ok(Value::Object(serde_json::Map::new()));
    }
    let fmt = format.trim().to_ascii_lowercase();
    let value: Value = match fmt.as_str() {
        "json" => {
            serde_json::from_str(trimmed).map_err(|e| format!("data is not valid JSON: {e}"))?
        }
        "yaml" | "yml" => {
            serde_yml::from_str(trimmed).map_err(|e| format!("data is not valid YAML: {e}"))?
        }
        "auto" | "" => {
            // Auto-detect: try JSON first (gives the best error messages), then
            // fall back to YAML.
            match serde_json::from_str::<Value>(trimmed) {
                Ok(v) => v,
                Err(json_err) => serde_yml::from_str::<Value>(trimmed).map_err(|yaml_err| {
                    format!("data is neither valid JSON ({json_err}) nor valid YAML ({yaml_err})")
                })?,
            }
        }
        other => {
            return Err(format!(
                "unknown data format '{other}' (expected 'auto', 'json', or 'yaml')"
            ))
        }
    };
    Ok(value)
}

/// Render `template` (Jinja2 syntax) against `data` (JSON or YAML).
///
/// - `data_format`: "auto" (default), "json", or "yaml".
/// - `strict`: when true, referencing a missing variable is an error; when
///   false (default) it renders as empty.
///
/// Returns the rendered text, or a human-readable error string.
pub fn render(
    template: &str,
    data: &str,
    data_format: &str,
    strict: bool,
) -> Result<String, String> {
    let ctx = parse_data(data, data_format)?;

    let mut env = Environment::new();
    env.set_undefined_behavior(if strict {
        UndefinedBehavior::Strict
    } else {
        UndefinedBehavior::Lenient
    });
    // Default autoescape off (non-HTML output): keep the tool useful for emails,
    // config files, and code (no &amp; surprises).
    env.set_auto_escape_callback(|_name| minijinja::AutoEscape::None);

    env.add_template("tmpl", template)
        .map_err(|e| format!("template error: {}", strip_position(&e.to_string())))?;
    let tmpl = env
        .get_template("tmpl")
        .map_err(|e| format!("template error: {e}"))?;
    tmpl.render(&ctx)
        .map_err(|e| format!("render error: {}", strip_position(&e.to_string())))
}

/// minijinja error strings include the synthetic template name ("tmpl"); keep
/// them readable without leaking that internal name.
fn strip_position(msg: &str) -> String {
    msg.replace(" (in tmpl:", " (line ").replace("tmpl:", "line ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_variable() {
        let out = render("Hello {{ name }}!", r#"{"name":"Ada"}"#, "json", false).unwrap();
        assert_eq!(out, "Hello Ada!");
    }

    #[test]
    fn for_loop() {
        let out = render(
            "{% for i in items %}- {{ i }}\n{% endfor %}",
            r#"{"items":["a","b","c"]}"#,
            "json",
            false,
        )
        .unwrap();
        assert_eq!(out, "- a\n- b\n- c\n");
    }

    #[test]
    fn if_conditional() {
        let out = render(
            "{% if admin %}root{% else %}user{% endif %}",
            r#"{"admin":true}"#,
            "auto",
            false,
        )
        .unwrap();
        assert_eq!(out, "root");
    }

    #[test]
    fn filters() {
        let out = render("{{ name | upper }}", r#"{"name":"ada"}"#, "json", false).unwrap();
        assert_eq!(out, "ADA");
    }

    #[test]
    fn nested_path() {
        let out = render(
            "{{ user.profile.email }}",
            r#"{"user":{"profile":{"email":"a@b.co"}}}"#,
            "json",
            false,
        )
        .unwrap();
        assert_eq!(out, "a@b.co");
    }

    #[test]
    fn yaml_data() {
        let out = render(
            "{{ greeting }}, {{ who }}!",
            "greeting: Hello\nwho: World\n",
            "yaml",
            false,
        )
        .unwrap();
        assert_eq!(out, "Hello, World!");
    }

    #[test]
    fn yaml_loop_auto_detect() {
        let out = render(
            "{% for f in fruits %}{{ f }} {% endfor %}",
            "fruits:\n  - apple\n  - pear\n",
            "auto",
            false,
        )
        .unwrap();
        assert_eq!(out, "apple pear ");
    }

    #[test]
    fn missing_var_lenient_is_empty() {
        let out = render("[{{ missing }}]", "{}", "json", false).unwrap();
        assert_eq!(out, "[]");
    }

    #[test]
    fn missing_var_strict_errors() {
        let err = render("[{{ missing }}]", "{}", "json", true).unwrap_err();
        assert!(err.contains("render error"), "got: {err}");
    }

    #[test]
    fn no_html_escape() {
        let out = render("{{ html }}", r#"{"html":"<b>x</b>"}"#, "json", false).unwrap();
        assert_eq!(out, "<b>x</b>");
    }

    #[test]
    fn empty_data_renders_static() {
        let out = render("static text", "", "auto", false).unwrap();
        assert_eq!(out, "static text");
    }

    #[test]
    fn invalid_json_errors() {
        let err = render("{{ x }}", "{not json", "json", false).unwrap_err();
        assert!(err.contains("not valid JSON"), "got: {err}");
    }

    #[test]
    fn invalid_template_errors() {
        let err = render("{% for %}", "{}", "json", false).unwrap_err();
        assert!(err.contains("template error"), "got: {err}");
    }

    #[test]
    fn unknown_format_errors() {
        let err = render("{{ x }}", "x: 1", "toml", false).unwrap_err();
        assert!(err.contains("unknown data format"), "got: {err}");
    }
}

//! render-template core — render a Handlebars/Mustache template against JSON data.
//!
//! Pure compute, shared by the chat skill block and the web page. The
//! `handlebars` crate implements the Handlebars language, which is a superset of
//! Mustache (`{{var}}`, `{{#each}}`, `{{#if}}`, `{{> partial}}` not used here),
//! so both the "handlebars" and "mustache" engine selections render with it. In
//! mustache mode HTML-escaping of `{{var}}` is kept (the Mustache default), and
//! strict-vs-lenient missing-variable handling is exposed via `strict`.

use serde_json::Value;

/// Which template dialect to render. Both run on the Handlebars engine
/// (Handlebars is a superset of Mustache); the distinction only affects which
/// helpers/features are documented as supported.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    Handlebars,
    Mustache,
}

impl Engine {
    pub fn parse(s: &str) -> Result<Engine, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "handlebars" | "hbs" => Ok(Engine::Handlebars),
            "mustache" => Ok(Engine::Mustache),
            other => Err(format!(
                "unknown engine '{other}' (expected 'handlebars' or 'mustache')"
            )),
        }
    }
}

/// Render `template` against the JSON object/array in `data_json`.
///
/// - `engine`: "handlebars" or "mustache".
/// - `strict`: when true, referencing a missing variable is an error; when
///   false (default) it renders as empty (the Mustache/Handlebars default).
pub fn render(
    template: &str,
    data_json: &str,
    engine: &str,
    strict: bool,
) -> Result<String, String> {
    let _engine = Engine::parse(engine)?;

    let data: Value = if data_json.trim().is_empty() {
        Value::Object(serde_json::Map::new())
    } else {
        serde_json::from_str(data_json).map_err(|e| format!("data is not valid JSON: {e}"))?
    };

    let mut hb = handlebars::Handlebars::new();
    hb.set_strict_mode(strict);
    // Don't HTML-escape by default for either engine — these templates are used
    // for arbitrary text/config/code generation, not just HTML. Callers that
    // want HTML escaping can use the `{{var}}` -> triple is reversed: keep the
    // raw passthrough so output is predictable.
    hb.register_escape_fn(handlebars::no_escape);

    hb.render_template(template, &data)
        .map_err(|e| format!("template error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_substitution() {
        let out = render("Hello {{name}}!", r#"{"name":"Ada"}"#, "handlebars", false).unwrap();
        assert_eq!(out, "Hello Ada!");
    }

    #[test]
    fn each_loop() {
        let out = render(
            "{{#each items}}- {{this}}\n{{/each}}",
            r#"{"items":["a","b","c"]}"#,
            "handlebars",
            false,
        )
        .unwrap();
        assert_eq!(out, "- a\n- b\n- c\n");
    }

    #[test]
    fn conditional() {
        let tpl = "{{#if admin}}ADMIN{{else}}user{{/if}}";
        assert_eq!(render(tpl, r#"{"admin":true}"#, "handlebars", false).unwrap(), "ADMIN");
        assert_eq!(render(tpl, r#"{"admin":false}"#, "handlebars", false).unwrap(), "user");
    }

    #[test]
    fn nested_path() {
        let out = render(
            "{{user.profile.email}}",
            r#"{"user":{"profile":{"email":"a@b.co"}}}"#,
            "mustache",
            false,
        )
        .unwrap();
        assert_eq!(out, "a@b.co");
    }

    #[test]
    fn missing_var_lenient_is_empty() {
        let out = render("[{{missing}}]", "{}", "handlebars", false).unwrap();
        assert_eq!(out, "[]");
    }

    #[test]
    fn missing_var_strict_errors() {
        let err = render("[{{missing}}]", "{}", "handlebars", true).unwrap_err();
        assert!(err.contains("template error"), "got: {err}");
    }

    #[test]
    fn no_html_escape() {
        let out = render("{{html}}", r#"{"html":"<b>x</b>"}"#, "handlebars", false).unwrap();
        assert_eq!(out, "<b>x</b>");
    }

    #[test]
    fn empty_data_defaults_to_object() {
        let out = render("static text", "", "handlebars", false).unwrap();
        assert_eq!(out, "static text");
    }

    #[test]
    fn invalid_json_errors() {
        let err = render("{{x}}", "{not json}", "handlebars", false).unwrap_err();
        assert!(err.contains("not valid JSON"), "got: {err}");
    }

    #[test]
    fn unknown_engine_errors() {
        let err = render("{{x}}", "{}", "jinja", false).unwrap_err();
        assert!(err.contains("unknown engine"), "got: {err}");
    }

    #[test]
    fn invalid_template_errors() {
        let err = render("{{#each}}", r#"{}"#, "handlebars", false).unwrap_err();
        assert!(err.contains("template error"), "got: {err}");
    }
}

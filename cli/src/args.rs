//! Map CLI args to a JSON body using the tool's JSON schema.
use anyhow::{bail, Result};
use serde_json::{Map, Value};

/// Map positional/key=value CLI arguments (and an optional `--json` escape
/// hatch) into a JSON object suitable for passing to `run_tool`.
///
/// Rules:
/// - `json` and `positional` are mutually exclusive.
/// - Bare positionals (no `=`) are matched left-to-right against the schema's
///   `required` scalar fields.
/// - `key=value` positionals are inserted directly, with type coercion from the
///   schema's `properties`.
/// - All `required` fields must be present after construction or an error is
///   returned.
pub fn map_args(schema: &Value, positional: &[String], json: Option<&str>) -> Result<Value> {
    if json.is_some() && !positional.is_empty() {
        bail!("pass either positional/key=value args or --json, not both");
    }
    if let Some(s) = json {
        let v: Value =
            serde_json::from_str(s).map_err(|e| anyhow::anyhow!("invalid --json: {e}"))?;
        validate_required(schema, &v)?;
        return Ok(v);
    }
    let props = schema.get("properties").and_then(|p| p.as_object());
    let required: Vec<&str> = schema
        .get("required")
        .and_then(|r| r.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();
    let mut body = Map::new();
    let mut bare: Vec<&String> = Vec::new();
    for arg in positional.iter() {
        // Treat an arg as `key=value` ONLY when the part before the first `=`
        // names a real schema property. Otherwise it's a bare positional whose
        // VALUE merely contains `=` — e.g. files-to-prompt's `=== path` header
        // input, or url-encoding a query string like `a=b`. (With an unknown
        // schema, `props` is None → keep the legacy "any `=` is key=value".)
        match arg.split_once('=') {
            Some((k, v)) if props.is_none_or(|p| p.contains_key(k)) => {
                body.insert(k.to_string(), coerce(props, k, v));
            }
            _ => bare.push(arg),
        }
    }
    let scalar_required: Vec<&str> = required
        .iter()
        .copied()
        .filter(|k| is_scalar(props, k))
        .collect();
    if !bare.is_empty() {
        if bare.len() > scalar_required.len() {
            bail!(
                "too many positional args; expected at most {} ({:?}). Use key=value.",
                scalar_required.len(),
                scalar_required
            );
        }
        for (k, val) in scalar_required.iter().zip(bare.iter()) {
            body.insert((*k).to_string(), coerce(props, k, val));
        }
    }
    let v = Value::Object(body);
    validate_required(schema, &v)?;
    Ok(v)
}

fn is_scalar(props: Option<&Map<String, Value>>, key: &str) -> bool {
    matches!(
        props
            .and_then(|p| p.get(key))
            .and_then(|s| s.get("type"))
            .and_then(|t| t.as_str()),
        Some("string") | Some("integer") | Some("number") | Some("boolean")
    )
}

fn coerce(props: Option<&Map<String, Value>>, key: &str, raw: &str) -> Value {
    match props
        .and_then(|p| p.get(key))
        .and_then(|s| s.get("type"))
        .and_then(|t| t.as_str())
    {
        Some("integer") => raw
            .parse::<i64>()
            .map(Value::from)
            .unwrap_or_else(|_| Value::from(raw)),
        Some("number") => raw
            .parse::<f64>()
            .map(Value::from)
            .unwrap_or_else(|_| Value::from(raw)),
        Some("boolean") => match raw {
            "true" => Value::Bool(true),
            "false" => Value::Bool(false),
            _ => Value::from(raw),
        },
        _ => Value::from(raw),
    }
}

fn validate_required(schema: &Value, body: &Value) -> Result<()> {
    if let Some(req) = schema.get("required").and_then(|r| r.as_array()) {
        for k in req.iter().filter_map(|v| v.as_str()) {
            if body.get(k).is_none() {
                bail!("missing required arg `{k}`");
            }
        }
    }
    Ok(())
}

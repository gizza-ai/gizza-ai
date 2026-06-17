//! Turn a skill's response body into terminal output + an exit code.
use serde_json::Value;

/// The rendered output of a skill response.
pub struct Rendered {
    /// Text to print to stdout.
    pub stdout: String,
    /// Process exit code.
    pub exit_code: i32,
}

/// Render a skill response body into human-friendly or raw-JSON output.
pub fn render(body: &[u8], json_mode: bool) -> Rendered {
    let text = String::from_utf8_lossy(body);
    let parsed: Option<Value> = serde_json::from_str(&text).ok();
    if let Some(Value::Object(map)) = &parsed {
        if let Some(err) = map.get("error") {
            let msg = map
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or_else(|| err.as_str().unwrap_or("tool error"));
            return Rendered {
                stdout: msg.to_string(),
                exit_code: 1,
            };
        }
    }
    if json_mode {
        return Rendered {
            stdout: text.into_owned(),
            exit_code: 0,
        };
    }
    if let Some(Value::Object(map)) = &parsed {
        if let Some(s) = map.get("_for_llm").and_then(|v| v.as_str()) {
            return Rendered {
                stdout: s.to_string(),
                exit_code: 0,
            };
        }
        if let Some(r) = map.get("result") {
            return Rendered {
                stdout: trim_number(r),
                exit_code: 0,
            };
        }
    }
    Rendered {
        stdout: text.into_owned(),
        exit_code: 0,
    }
}

fn trim_number(v: &Value) -> String {
    if let Some(f) = v.as_f64() {
        if f.fract() == 0.0 {
            return format!("{}", f as i64);
        }
        return format!("{f}");
    }
    v.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_number_human() {
        let r = render(br#"{"result":4.0}"#, false);
        assert_eq!(r.stdout, "4");
        assert_eq!(r.exit_code, 0);
    }

    #[test]
    fn error_nonzero() {
        let r = render(br#"{"error":"eval failed: non-finite"}"#, false);
        assert_eq!(r.exit_code, 1);
    }

    #[test]
    fn envelope_for_llm() {
        let r = render(br#"{"_for_llm":"resized cat to 64x64","_for_ui":{}}"#, false);
        assert_eq!(r.stdout, "resized cat to 64x64");
    }
}

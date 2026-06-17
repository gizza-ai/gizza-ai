//! Turn a skill's response body into terminal output + an exit code.
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use serde_json::Value;

/// A binary payload extracted from a `_for_ui.data_url` response envelope.
pub struct BinaryOut {
    /// Suggested filename (from `_for_ui.filename` or `"output.bin"`).
    pub filename: String,
    /// Decoded bytes.
    pub bytes: Vec<u8>,
}

/// Extract a binary payload from a `_for_ui.data_url` response envelope.
///
/// Returns `None` if the body is not JSON, has no `_for_ui`, or has no
/// `data_url` with a base64 segment.
pub fn extract_binary(body: &[u8]) -> Option<BinaryOut> {
    let v: Value = serde_json::from_slice(body).ok()?;
    let ui = v.get("_for_ui")?;
    let data_url = ui.get("data_url")?.as_str()?;
    let b64 = data_url.split(";base64,").nth(1)?;
    let bytes = B64.decode(b64).ok()?;
    let filename = ui
        .get("filename")
        .and_then(|f| f.as_str())
        .unwrap_or("output.bin")
        .to_string();
    Some(BinaryOut { filename, bytes })
}

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
    fn data_url_extracts_bytes_and_filename() {
        let body = br#"{"_for_llm":"made png","_for_ui":{"data_url":"data:image/png;base64,AAAA","filename":"out.png","mime":"image/png"}}"#;
        let bin = super::extract_binary(body).expect("some");
        assert_eq!(bin.filename, "out.png");
        assert_eq!(bin.bytes, vec![0, 0, 0]);
    }

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

//! har-validator core — validate a HAR (HTTP Archive) 1.x file against the spec.
//! Pure-Rust (serde_json only). Checks the `log`/`entries` shape, the required
//! request/response/timings fields, and (optionally) that each entry's `time`
//! equals the sum of its timing phases. Reports EVERY problem it finds, not just
//! the first, and returns a human-readable report for both valid and invalid HARs
//! (only non-JSON / empty input is an error).

use serde_json::Value;

/// Timing phases whose non-negative values must add up to `entry.time`
/// (per the HAR 1.2 spec; `ssl` is counted inside `connect`, so it's excluded).
const TIMING_PHASES: [&str; 6] = ["blocked", "dns", "connect", "send", "wait", "receive"];

/// Rounding tolerance (ms) for the `time` == Σ(phases) check. HAR writers round
/// phase values independently, so tiny drift is expected and not flagged.
const TIMING_TOLERANCE_MS: f64 = 1.0;

struct Findings {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl Findings {
    fn new() -> Self {
        Findings { errors: Vec::new(), warnings: Vec::new() }
    }
    fn err(&mut self, msg: impl Into<String>) {
        self.errors.push(msg.into());
    }
    fn warn(&mut self, msg: impl Into<String>) {
        self.warnings.push(msg.into());
    }
}

/// Validate a HAR document. `check_timings` toggles the per-entry
/// `time == Σ(phases)` consistency check (reported as warnings). Returns the
/// formatted report, or `Err` only when the input isn't parseable JSON.
pub fn validate(har: &str, check_timings: bool) -> Result<String, String> {
    if har.trim().is_empty() {
        return Err("no HAR input".into());
    }
    let root: Value = serde_json::from_str(har).map_err(|e| format!("invalid JSON: {e}"))?;

    let mut f = Findings::new();
    let mut version: Option<String> = None;
    let mut creator: Option<String> = None;
    let mut page_count = 0usize;
    let mut entry_count = 0usize;

    match root.get("log") {
        None => f.err("log: missing required field \"log\" (a HAR file is a single { \"log\": … } object)"),
        Some(Value::Object(log)) => {
            // version (required string)
            match log.get("version") {
                Some(Value::String(s)) => version = Some(s.clone()),
                Some(_) => f.err("log.version: must be a string"),
                None => f.err("log: missing required field \"version\""),
            }
            // creator (required object with name + version strings)
            match log.get("creator") {
                Some(Value::Object(c)) => {
                    let name = string_field(&mut f, c, "log.creator", "name");
                    let ver = string_field(&mut f, c, "log.creator", "version");
                    if let (Some(n), Some(v)) = (name, ver) {
                        creator = Some(if v.is_empty() { n } else { format!("{n} {v}") });
                    }
                }
                Some(_) => f.err("log.creator: must be an object"),
                None => f.err("log: missing required field \"creator\""),
            }
            // browser (optional object; if present needs name + version)
            if let Some(b) = log.get("browser") {
                match b {
                    Value::Object(b) => {
                        string_field(&mut f, b, "log.browser", "name");
                        string_field(&mut f, b, "log.browser", "version");
                    }
                    _ => f.err("log.browser: must be an object when present"),
                }
            }
            // pages (optional array)
            match log.get("pages") {
                None => {}
                Some(Value::Array(pages)) => {
                    page_count = pages.len();
                    for (i, p) in pages.iter().enumerate() {
                        validate_page(&mut f, p, i);
                    }
                }
                Some(_) => f.err("log.pages: must be an array when present"),
            }
            // entries (required array)
            match log.get("entries") {
                Some(Value::Array(entries)) => {
                    entry_count = entries.len();
                    for (i, e) in entries.iter().enumerate() {
                        validate_entry(&mut f, e, i, check_timings);
                    }
                }
                Some(_) => f.err("log.entries: must be an array"),
                None => f.err("log: missing required field \"entries\""),
            }
        }
        Some(_) => f.err("log: must be an object"),
    }

    Ok(render_report(&f, version.as_deref(), creator.as_deref(), page_count, entry_count))
}

/// Back-compat entry point used by the block/web wrappers before `check_timings`
/// existed; defaults timing checks on.
pub fn run(har: &str) -> Result<String, String> {
    validate(har, true)
}

fn validate_page(f: &mut Findings, page: &Value, i: usize) {
    let path = format!("log.pages[{i}]");
    match page {
        Value::Object(p) => {
            string_field(f, p, &path, "startedDateTime");
            string_field(f, p, &path, "id");
            string_field(f, p, &path, "title");
            match p.get("pageTimings") {
                Some(Value::Object(_)) => {}
                Some(_) => f.err(format!("{path}.pageTimings: must be an object")),
                None => f.err(format!("{path}: missing required field \"pageTimings\"")),
            }
        }
        _ => f.err(format!("{path}: must be an object")),
    }
}

fn validate_entry(f: &mut Findings, entry: &Value, i: usize, check_timings: bool) {
    let path = format!("log.entries[{i}]");
    let obj = match entry {
        Value::Object(o) => o,
        _ => {
            f.err(format!("{path}: must be an object"));
            return;
        }
    };

    string_field(f, obj, &path, "startedDateTime");
    let time = number_field(f, obj, &path, "time");

    // request (required object)
    match obj.get("request") {
        Some(Value::Object(r)) => validate_request(f, r, &format!("{path}.request")),
        Some(_) => f.err(format!("{path}.request: must be an object")),
        None => f.err(format!("{path}: missing required field \"request\"")),
    }
    // response (required object)
    match obj.get("response") {
        Some(Value::Object(r)) => validate_response(f, r, &format!("{path}.response")),
        Some(_) => f.err(format!("{path}.response: must be an object")),
        None => f.err(format!("{path}: missing required field \"response\"")),
    }
    // cache (required object)
    match obj.get("cache") {
        Some(Value::Object(_)) => {}
        Some(_) => f.err(format!("{path}.cache: must be an object")),
        None => f.err(format!("{path}: missing required field \"cache\"")),
    }
    // timings (required object)
    let timings = match obj.get("timings") {
        Some(Value::Object(t)) => Some(t),
        Some(_) => {
            f.err(format!("{path}.timings: must be an object"));
            None
        }
        None => {
            f.err(format!("{path}: missing required field \"timings\""));
            None
        }
    };
    if let Some(t) = timings {
        let tpath = format!("{path}.timings");
        // send / wait / receive are required; blocked / dns / connect / ssl optional.
        number_field(f, t, &tpath, "send");
        number_field(f, t, &tpath, "wait");
        number_field(f, t, &tpath, "receive");

        if check_timings {
            if let Some(total) = time {
                check_timing_total(f, t, total, &path);
            }
        }
    }
}

fn validate_request(f: &mut Findings, r: &serde_json::Map<String, Value>, path: &str) {
    string_field(f, r, path, "method");
    string_field(f, r, path, "url");
    string_field(f, r, path, "httpVersion");
    array_field(f, r, path, "cookies");
    array_field(f, r, path, "headers");
    array_field(f, r, path, "queryString");
    number_field(f, r, path, "headersSize");
    number_field(f, r, path, "bodySize");
}

fn validate_response(f: &mut Findings, r: &serde_json::Map<String, Value>, path: &str) {
    number_field(f, r, path, "status");
    string_field(f, r, path, "statusText");
    string_field(f, r, path, "httpVersion");
    array_field(f, r, path, "cookies");
    array_field(f, r, path, "headers");
    match r.get("content") {
        Some(Value::Object(c)) => {
            let cpath = format!("{path}.content");
            number_field(f, c, &cpath, "size");
            string_field(f, c, &cpath, "mimeType");
        }
        Some(_) => f.err(format!("{path}.content: must be an object")),
        None => f.err(format!("{path}: missing required field \"content\"")),
    }
    string_field(f, r, path, "redirectURL");
    number_field(f, r, path, "headersSize");
    number_field(f, r, path, "bodySize");
}

/// Warn if `entry.time` doesn't equal the sum of the non-negative timing phases
/// (a value of -1 means "not applicable/available" and is excluded from the sum).
fn check_timing_total(
    f: &mut Findings,
    t: &serde_json::Map<String, Value>,
    total: f64,
    entry_path: &str,
) {
    // A -1 anywhere means the total is intentionally not computable — skip.
    let mut sum = 0.0;
    for phase in TIMING_PHASES {
        if let Some(v) = t.get(phase).and_then(Value::as_f64) {
            if v >= 0.0 {
                sum += v;
            }
        }
    }
    if total < 0.0 {
        return; // time = -1 → not tracked
    }
    let diff = (total - sum).abs();
    if diff > TIMING_TOLERANCE_MS {
        f.warn(format!(
            "{entry_path}.timings: entry time ({}) ≠ sum of phases ({}); difference {} ms",
            trim_num(total),
            trim_num(sum),
            trim_num(diff)
        ));
    }
}

// ---- typed-field helpers: record a specific error, return the value on success ----

fn string_field(
    f: &mut Findings,
    obj: &serde_json::Map<String, Value>,
    path: &str,
    key: &str,
) -> Option<String> {
    match obj.get(key) {
        Some(Value::String(s)) => Some(s.clone()),
        Some(_) => {
            f.err(format!("{path}.{key}: must be a string"));
            None
        }
        None => {
            f.err(format!("{path}: missing required field \"{key}\""));
            None
        }
    }
}

fn number_field(
    f: &mut Findings,
    obj: &serde_json::Map<String, Value>,
    path: &str,
    key: &str,
) -> Option<f64> {
    match obj.get(key) {
        Some(v) if v.is_number() => v.as_f64(),
        Some(_) => {
            f.err(format!("{path}.{key}: must be a number"));
            None
        }
        None => {
            f.err(format!("{path}: missing required field \"{key}\""));
            None
        }
    }
}

fn array_field(f: &mut Findings, obj: &serde_json::Map<String, Value>, path: &str, key: &str) {
    match obj.get(key) {
        Some(Value::Array(_)) => {}
        Some(_) => f.err(format!("{path}.{key}: must be an array")),
        None => f.err(format!("{path}: missing required field \"{key}\"")),
    }
}

/// Render a number without a trailing `.0` (so `100.0` prints as `100`).
fn trim_num(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        // strip insignificant trailing zeros
        let s = format!("{n:.3}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn render_report(
    f: &Findings,
    version: Option<&str>,
    creator: Option<&str>,
    pages: usize,
    entries: usize,
) -> String {
    let mut out = String::new();
    let valid = f.errors.is_empty();
    let spec = version.map(|v| format!("HTTP Archive {v}")).unwrap_or_else(|| "HTTP Archive".into());
    if valid {
        out.push_str(&format!("✓ Valid HAR ({spec})\n\n"));
    } else {
        out.push_str(&format!("✗ Invalid HAR ({spec})\n\n"));
    }

    out.push_str("Summary\n");
    out.push_str(&format!("  version:  {}\n", version.unwrap_or("(missing)")));
    out.push_str(&format!("  creator:  {}\n", creator.unwrap_or("(missing)")));
    out.push_str(&format!("  pages:    {pages}\n"));
    out.push_str(&format!("  entries:  {entries}\n"));

    if f.errors.is_empty() {
        out.push_str("\nNo structural errors found.\n");
    } else {
        out.push_str(&format!("\nErrors ({})\n", f.errors.len()));
        for e in &f.errors {
            out.push_str(&format!("  • {e}\n"));
        }
    }

    if !f.warnings.is_empty() {
        out.push_str(&format!("\nWarnings ({})\n", f.warnings.len()));
        for w in &f.warnings {
            out.push_str(&format!("  • {w}\n"));
        }
    }

    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_valid() -> &'static str {
        r#"{
          "log": {
            "version": "1.2",
            "creator": { "name": "WebInspector", "version": "537.36" },
            "entries": [
              {
                "startedDateTime": "2024-01-01T00:00:00.000Z",
                "time": 100,
                "request": {
                  "method": "GET", "url": "https://example.com/", "httpVersion": "HTTP/1.1",
                  "cookies": [], "headers": [], "queryString": [], "headersSize": -1, "bodySize": 0
                },
                "response": {
                  "status": 200, "statusText": "OK", "httpVersion": "HTTP/1.1",
                  "cookies": [], "headers": [],
                  "content": { "size": 12, "mimeType": "text/html" },
                  "redirectURL": "", "headersSize": -1, "bodySize": 12
                },
                "cache": {},
                "timings": { "send": 10, "wait": 80, "receive": 10 }
              }
            ]
          }
        }"#
    }

    #[test]
    fn valid_har_passes() {
        let out = validate(minimal_valid(), true).unwrap();
        assert!(out.starts_with("✓ Valid HAR (HTTP Archive 1.2)"), "{out}");
        assert!(out.contains("creator:  WebInspector 537.36"));
        assert!(out.contains("entries:  1"));
        assert!(out.contains("No structural errors found."));
    }

    #[test]
    fn missing_log_is_invalid() {
        let out = validate(r#"{"foo": 1}"#, true).unwrap();
        assert!(out.starts_with("✗ Invalid HAR"));
        assert!(out.contains("missing required field \"log\""));
    }

    #[test]
    fn missing_request_field_reported_with_path() {
        // drop request.method
        let har = minimal_valid().replace("\"method\": \"GET\", ", "");
        let out = validate(&har, true).unwrap();
        assert!(out.contains("log.entries[0].request: missing required field \"method\""), "{out}");
        assert!(out.starts_with("✗ Invalid HAR"));
    }

    #[test]
    fn wrong_type_reported() {
        let har = minimal_valid().replace("\"status\": 200", "\"status\": \"200\"");
        let out = validate(&har, true).unwrap();
        assert!(out.contains("log.entries[0].response.status: must be a number"), "{out}");
    }

    #[test]
    fn collects_multiple_errors() {
        let har = minimal_valid()
            .replace("\"send\": 10, ", "")
            .replace("\"redirectURL\": \"\", ", "");
        let out = validate(&har, true).unwrap();
        assert!(out.contains("missing required field \"send\""), "{out}");
        assert!(out.contains("missing required field \"redirectURL\""), "{out}");
    }

    #[test]
    fn timing_total_mismatch_warns_but_still_valid() {
        // phases sum to 100 but time says 250
        let har = minimal_valid().replace("\"time\": 100", "\"time\": 250");
        let out = validate(&har, true).unwrap();
        assert!(out.starts_with("✓ Valid HAR"), "structural ok → still valid: {out}");
        assert!(out.contains("Warnings (1)"), "{out}");
        assert!(out.contains("entry time (250) ≠ sum of phases (100); difference 150 ms"), "{out}");
    }

    #[test]
    fn timing_check_can_be_disabled() {
        let har = minimal_valid().replace("\"time\": 100", "\"time\": 250");
        let out = validate(&har, false).unwrap();
        assert!(!out.contains("Warnings"), "no timing warnings when disabled: {out}");
    }

    #[test]
    fn negative_one_phase_excluded_from_sum() {
        // blocked/dns/connect default -1; send+wait+receive = 100, time = 100 → consistent
        let har = minimal_valid().replace(
            "\"timings\": { \"send\": 10, \"wait\": 80, \"receive\": 10 }",
            "\"timings\": { \"blocked\": -1, \"dns\": -1, \"connect\": -1, \"send\": 10, \"wait\": 80, \"receive\": 10, \"ssl\": -1 }",
        );
        let out = validate(&har, true).unwrap();
        assert!(!out.contains("Warnings"), "{out}");
    }

    #[test]
    fn tolerates_sub_millisecond_rounding() {
        let har = minimal_valid().replace("\"time\": 100", "\"time\": 100.4");
        let out = validate(&har, true).unwrap();
        assert!(!out.contains("Warnings"), "0.4ms drift within tolerance: {out}");
    }

    #[test]
    fn non_json_errors() {
        assert!(validate("not json {", true).is_err());
        assert!(validate("", true).is_err());
        assert!(validate("   ", true).is_err());
    }

    #[test]
    fn version_shows_in_spec_label() {
        let har = minimal_valid().replace("\"version\": \"1.2\"", "\"version\": \"1.1\"");
        let out = validate(&har, true).unwrap();
        assert!(out.contains("HTTP Archive 1.1"), "{out}");
    }

    #[test]
    fn run_defaults_timing_on() {
        let har = minimal_valid().replace("\"time\": 100", "\"time\": 250");
        let out = run(&har).unwrap();
        assert!(out.contains("Warnings (1)"), "{out}");
    }
}

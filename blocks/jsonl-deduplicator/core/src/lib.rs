//! gizza-ai/jsonl-deduplicator core — remove duplicate records from an
//! NDJSON/JSONL stream (one JSON value per line) by whole-line equality or a
//! chosen set of key fields, keeping the first or last occurrence. Pure-Rust,
//! shared by the chat skill block and the web page. Depends only on serde/serde_json.

use std::collections::HashSet;

use serde::Serialize;
use serde_json::Value;

/// Which occurrence of a duplicated record to keep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keep {
    /// Keep the first occurrence, drop later duplicates (default).
    First,
    /// Keep the last occurrence, drop earlier duplicates.
    Last,
}

impl Keep {
    pub fn parse(s: &str) -> Result<Keep, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "first" | "" => Ok(Keep::First),
            "last" => Ok(Keep::Last),
            other => Err(format!("keep must be \"first\" or \"last\", got \"{other}\"")),
        }
    }
}

/// What to do with a line that is not valid JSON (only relevant in key-field
/// mode, where each line must be parsed to read its fields).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnInvalid {
    /// Stop and return an error naming the offending line (default).
    Error,
    /// Drop the invalid line from the output.
    Skip,
    /// Keep the invalid line as-is, de-duplicated by its raw text.
    Keep,
}

impl OnInvalid {
    pub fn parse(s: &str) -> Result<OnInvalid, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "error" | "" => Ok(OnInvalid::Error),
            "skip" => Ok(OnInvalid::Skip),
            "keep" => Ok(OnInvalid::Keep),
            other => Err(format!(
                "on_invalid must be \"error\", \"skip\", or \"keep\", got \"{other}\""
            )),
        }
    }
}

/// Options controlling how duplicate records are detected and removed.
#[derive(Debug, Clone)]
pub struct Options {
    /// Key field paths to de-duplicate on (dot-notation for nested fields, e.g.
    /// `user.id`; numeric segments index into arrays). Empty = whole-line mode:
    /// records are compared by the exact text of the line.
    pub keys: Vec<String>,
    /// Which occurrence of a duplicated record to keep.
    pub keep: Keep,
    /// Compare case-insensitively (lowercases the comparison key).
    pub ignore_case: bool,
    /// What to do with a line that is not valid JSON (key-field mode only).
    pub on_invalid: OnInvalid,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            keys: Vec::new(),
            keep: Keep::First,
            ignore_case: false,
            on_invalid: OnInvalid::Error,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Output {
    /// The de-duplicated NDJSON/JSONL text.
    pub text: String,
    /// Number of records (non-blank lines) in the input.
    pub total_records: usize,
    /// Number of records kept in the output.
    pub kept_records: usize,
    /// Number of records removed (duplicates + any dropped invalid lines).
    pub removed_records: usize,
}

/// Parse a comma-separated key list into trimmed, non-empty field paths.
pub fn parse_keys(s: &str) -> Vec<String> {
    s.split(',')
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
        .collect()
}

/// Navigate a JSON value by a dot-notation path (object fields; numeric
/// segments index arrays). Returns `None` if any segment is absent.
fn extract<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = value;
    for seg in path.split('.') {
        match cur {
            Value::Object(map) => cur = map.get(seg)?,
            Value::Array(arr) => {
                let idx: usize = seg.parse().ok()?;
                cur = arr.get(idx)?;
            }
            _ => return None,
        }
    }
    Some(cur)
}

/// The de-duplication key for a single record.
enum RecKey {
    /// Drop this record entirely (invalid line under `on_invalid = skip`).
    Drop,
    /// Compare records by this string key.
    Key(String),
}

fn maybe_lower(s: String, ignore_case: bool) -> String {
    if ignore_case {
        s.to_lowercase()
    } else {
        s
    }
}

/// Compute the dedup key for `line` (1-based `lineno` used in error messages).
fn record_key(line: &str, lineno: usize, opts: &Options) -> Result<RecKey, String> {
    // Whole-line mode: compare by the exact (trimmed) line text — no JSON parse.
    if opts.keys.is_empty() {
        return Ok(RecKey::Key(maybe_lower(line.trim().to_string(), opts.ignore_case)));
    }

    // Key-field mode: parse the line and read the selected fields.
    let value: Value = match serde_json::from_str(line.trim()) {
        Ok(v) => v,
        Err(e) => {
            return match opts.on_invalid {
                OnInvalid::Error => Err(format!("line {lineno}: invalid JSON: {e}")),
                OnInvalid::Skip => Ok(RecKey::Drop),
                // Fall back to whole-line equality for the unparseable line.
                OnInvalid::Keep => {
                    Ok(RecKey::Key(maybe_lower(line.trim().to_string(), opts.ignore_case)))
                }
            };
        }
    };

    // Build a composite key from each field's canonical JSON. serde_json::Value
    // sorts object keys (no preserve_order feature), so `{"a":1,"b":2}` and
    // `{"b":2,"a":1}` produce the same field serialization. A missing field uses
    // a sentinel distinct from an explicit JSON null.
    let mut parts: Vec<String> = Vec::with_capacity(opts.keys.len());
    for path in &opts.keys {
        match extract(&value, path) {
            Some(v) => parts.push(serde_json::to_string(v).unwrap_or_default()),
            None => parts.push("\u{0}\u{0}absent".to_string()),
        }
    }
    // \u{1} can't appear in a JSON string token, so it's a safe part separator.
    Ok(RecKey::Key(maybe_lower(parts.join("\u{1}"), opts.ignore_case)))
}

/// Remove duplicate records from `data` according to `opts`.
///
/// Blank / whitespace-only lines are ignored (never counted or emitted). A
/// trailing newline is preserved iff the input ended with one and output is
/// non-empty.
pub fn dedupe(data: &str, opts: &Options) -> Result<Output, String> {
    let ends_with_newline = data.ends_with('\n');

    // Collect non-blank records with their dedup keys (1-based line numbers).
    let mut recs: Vec<(&str, RecKey)> = Vec::new();
    for (i, line) in data.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let key = record_key(line, i + 1, opts)?;
        recs.push((line, key));
    }
    let total_records = recs.len();

    let kept: Vec<&str> = if opts.keep == Keep::Last {
        // Keep the last occurrence: scan from the end keeping first-seen keys,
        // then restore order. Output position is the last occurrence.
        let mut seen: HashSet<&str> = HashSet::new();
        let mut out: Vec<&str> = Vec::new();
        for (line, key) in recs.iter().rev() {
            match key {
                RecKey::Drop => continue,
                RecKey::Key(k) => {
                    if seen.insert(k.as_str()) {
                        out.push(*line);
                    }
                }
            }
        }
        out.reverse();
        out
    } else {
        // Keep the first occurrence (order-preserving).
        let mut seen: HashSet<&str> = HashSet::new();
        let mut out: Vec<&str> = Vec::new();
        for (line, key) in recs.iter() {
            match key {
                RecKey::Drop => continue,
                RecKey::Key(k) => {
                    if seen.insert(k.as_str()) {
                        out.push(*line);
                    }
                }
            }
        }
        out
    };

    let kept_records = kept.len();
    let mut text = kept.join("\n");
    if ends_with_newline && !text.is_empty() {
        text.push('\n');
    }

    let removed_records = total_records.saturating_sub(kept_records);
    Ok(Output { text, total_records, kept_records, removed_records })
}

/// Page-facing rendering: just the de-duplicated JSONL text (paste-ready).
pub fn render(data: &str, opts: &Options) -> Result<String, String> {
    Ok(dedupe(data, opts)?.text)
}

/// Web/page entry: build [`Options`] from the raw string fields (field order
/// matches the descriptor / meta.toml) and return the de-duplicated JSONL text.
pub fn run(
    data: &str,
    keys: &str,
    keep: &str,
    ignore_case: bool,
    on_invalid: &str,
) -> Result<String, String> {
    let opts = Options {
        keys: parse_keys(keys),
        keep: Keep::parse(keep)?,
        ignore_case,
        on_invalid: OnInvalid::parse(on_invalid)?,
    };
    render(data, &opts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keyed(keys: &[&str]) -> Options {
        Options { keys: keys.iter().map(|s| s.to_string()).collect(), ..Options::default() }
    }

    #[test]
    fn whole_line_keep_first() {
        let o = dedupe("{\"a\":1}\n{\"a\":2}\n{\"a\":1}", &Options::default()).unwrap();
        assert_eq!(o.text, "{\"a\":1}\n{\"a\":2}");
        assert_eq!(o.total_records, 3);
        assert_eq!(o.kept_records, 2);
        assert_eq!(o.removed_records, 1);
    }

    #[test]
    fn whole_line_is_literal_not_semantic() {
        // Different key order → different text → NOT collapsed in whole-line mode.
        let o = dedupe("{\"a\":1,\"b\":2}\n{\"b\":2,\"a\":1}", &Options::default()).unwrap();
        assert_eq!(o.kept_records, 2);
    }

    #[test]
    fn key_field_dedup() {
        let data = "{\"id\":1,\"v\":\"a\"}\n{\"id\":2,\"v\":\"b\"}\n{\"id\":1,\"v\":\"c\"}";
        let o = dedupe(data, &keyed(&["id"])).unwrap();
        // second id=1 dropped; first occurrence (v=a) kept
        assert_eq!(o.text, "{\"id\":1,\"v\":\"a\"}\n{\"id\":2,\"v\":\"b\"}");
        assert_eq!(o.kept_records, 2);
    }

    #[test]
    fn key_field_ignores_field_order() {
        // Same id, whole line differs by key order — key mode collapses them.
        let data = "{\"id\":1,\"x\":2}\n{\"x\":9,\"id\":1}";
        let o = dedupe(data, &keyed(&["id"])).unwrap();
        assert_eq!(o.kept_records, 1);
        assert_eq!(o.text, "{\"id\":1,\"x\":2}");
    }

    #[test]
    fn key_field_keep_last() {
        let data = "{\"id\":1,\"v\":\"a\"}\n{\"id\":2,\"v\":\"b\"}\n{\"id\":1,\"v\":\"c\"}";
        let mut opts = keyed(&["id"]);
        opts.keep = Keep::Last;
        let o = dedupe(data, &opts).unwrap();
        // last id=1 (v=c) kept, in its final position
        assert_eq!(o.text, "{\"id\":2,\"v\":\"b\"}\n{\"id\":1,\"v\":\"c\"}");
    }

    #[test]
    fn multi_key_composite() {
        let data = "{\"a\":1,\"b\":1}\n{\"a\":1,\"b\":2}\n{\"a\":1,\"b\":1}";
        let o = dedupe(data, &keyed(&["a", "b"])).unwrap();
        assert_eq!(o.kept_records, 2);
    }

    #[test]
    fn nested_dot_path() {
        let data = "{\"user\":{\"id\":7}}\n{\"user\":{\"id\":7},\"t\":1}\n{\"user\":{\"id\":8}}";
        let o = dedupe(data, &keyed(&["user.id"])).unwrap();
        assert_eq!(o.kept_records, 2);
        assert_eq!(o.text, "{\"user\":{\"id\":7}}\n{\"user\":{\"id\":8}}");
    }

    #[test]
    fn array_index_path() {
        let data = "{\"t\":[9,1]}\n{\"t\":[9,2]}\n{\"t\":[9,5]}";
        let o = dedupe(data, &keyed(&["t.0"])).unwrap();
        // all share t[0]=9 → collapse to first
        assert_eq!(o.kept_records, 1);
    }

    #[test]
    fn missing_field_groups_together() {
        // Records lacking the key field collapse together (shared "absent" key),
        // distinct from an explicit null.
        let data = "{\"x\":1}\n{\"id\":null}\n{\"y\":2}";
        let o = dedupe(data, &keyed(&["id"])).unwrap();
        // {"x":1} and {"y":2} are both absent → one kept; {"id":null} is distinct
        assert_eq!(o.kept_records, 2);
        assert_eq!(o.text, "{\"x\":1}\n{\"id\":null}");
    }

    #[test]
    fn ignore_case_key() {
        let data = "{\"email\":\"A@X.COM\"}\n{\"email\":\"a@x.com\"}";
        let mut opts = keyed(&["email"]);
        opts.ignore_case = true;
        let o = dedupe(data, &opts).unwrap();
        assert_eq!(o.kept_records, 1);
    }

    #[test]
    fn invalid_json_error_by_default() {
        let data = "{\"id\":1}\nnot json\n{\"id\":2}";
        let err = dedupe(data, &keyed(&["id"])).unwrap_err();
        assert!(err.contains("line 2"), "error names the line: {err}");
    }

    #[test]
    fn invalid_json_skip() {
        let data = "{\"id\":1}\nnot json\n{\"id\":2}";
        let mut opts = keyed(&["id"]);
        opts.on_invalid = OnInvalid::Skip;
        let o = dedupe(data, &opts).unwrap();
        assert_eq!(o.text, "{\"id\":1}\n{\"id\":2}");
        assert_eq!(o.total_records, 3);
        assert_eq!(o.kept_records, 2);
        assert_eq!(o.removed_records, 1);
    }

    #[test]
    fn invalid_json_keep() {
        let data = "{\"id\":1}\nnot json\nnot json\n{\"id\":2}";
        let mut opts = keyed(&["id"]);
        opts.on_invalid = OnInvalid::Keep;
        let o = dedupe(data, &opts).unwrap();
        // the two identical "not json" lines collapse; both records kept
        assert_eq!(o.text, "{\"id\":1}\nnot json\n{\"id\":2}");
    }

    #[test]
    fn whole_line_mode_ignores_invalid_setting() {
        // Whole-line mode never parses JSON, so garbage lines are just text.
        let o = dedupe("hi\nhi\nbye", &Options::default()).unwrap();
        assert_eq!(o.text, "hi\nbye");
    }

    #[test]
    fn blank_lines_dropped() {
        let o = dedupe("{\"a\":1}\n\n{\"a\":1}\n  \n{\"a\":2}\n", &Options::default()).unwrap();
        assert_eq!(o.total_records, 3);
        assert_eq!(o.text, "{\"a\":1}\n{\"a\":2}\n");
    }

    #[test]
    fn preserves_trailing_newline() {
        let o = dedupe("{\"a\":1}\n{\"a\":1}\n", &Options::default()).unwrap();
        assert_eq!(o.text, "{\"a\":1}\n");
    }

    #[test]
    fn empty_input() {
        let o = dedupe("", &Options::default()).unwrap();
        assert_eq!(o.text, "");
        assert_eq!(o.total_records, 0);
        assert_eq!(o.removed_records, 0);
    }

    #[test]
    fn parse_keys_splits_and_trims() {
        assert_eq!(parse_keys(" id , user.id ,, name "), vec!["id", "user.id", "name"]);
        assert!(parse_keys("").is_empty());
        assert!(parse_keys(" , ").is_empty());
    }

    #[test]
    fn keep_parse() {
        assert_eq!(Keep::parse("first").unwrap(), Keep::First);
        assert_eq!(Keep::parse("LAST").unwrap(), Keep::Last);
        assert_eq!(Keep::parse("").unwrap(), Keep::First);
        assert!(Keep::parse("middle").is_err());
    }

    #[test]
    fn on_invalid_parse() {
        assert_eq!(OnInvalid::parse("error").unwrap(), OnInvalid::Error);
        assert_eq!(OnInvalid::parse("SKIP").unwrap(), OnInvalid::Skip);
        assert_eq!(OnInvalid::parse("keep").unwrap(), OnInvalid::Keep);
        assert_eq!(OnInvalid::parse("").unwrap(), OnInvalid::Error);
        assert!(OnInvalid::parse("bogus").is_err());
    }
}

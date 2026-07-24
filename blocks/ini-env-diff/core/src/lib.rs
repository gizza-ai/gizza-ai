//! ini-env-diff core — pure compute, shared by the chat skill block and the web
//! page. No wafer/wasm-bindgen deps (only serde_json for the JSON output).
//!
//! Diffs two `.env` or `.ini` config files key-by-key. Each file is parsed into
//! a flat key→value map (handling `KEY=VALUE`/`key: value` pairs, `#`/`;`
//! comments, blank lines, quotes, inline comments, an `export ` prefix, and
//! `[section]` headers → dotted `section.key`), then the two maps are compared:
//! keys present only in the right file are **added**, only in the left are
//! **removed**, present in both with different values are **changed**, and
//! identical keys are **unchanged**. Direction is left (old) → right (new).

use std::collections::{BTreeMap, BTreeSet};

/// Substrings (matched case-insensitively against the KEY name) that mark a
/// value as sensitive and therefore masked when `mask_secrets` is on. Mirrors
/// the dotenv-manager list so masking is consistent across the .env tools.
const SENSITIVE_MARKERS: &[&str] = &[
    "SECRET",
    "TOKEN",
    "PASSWORD",
    "PASSWD",
    "PWD",
    "KEY",
    "AUTH",
    "CRED",
    "PRIVATE",
    "CERT",
    "SIGNATURE",
    "ACCESS",
    "SESSION",
    "DSN",
];

/// Which parser to use for the two files.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Flat `KEY=VALUE` dotenv files (no `[section]` headers).
    Env,
    /// INI/conf files with `[section]` headers → dotted `section.key`.
    Ini,
}

impl Mode {
    fn label(self) -> &'static str {
        match self {
            Mode::Env => "env",
            Mode::Ini => "ini",
        }
    }
}

/// True if the text has at least one `[section]` header line.
fn has_section(text: &str) -> bool {
    text.lines().any(|l| {
        let t = l.trim();
        t.len() >= 3 && t.starts_with('[') && t.ends_with(']')
    })
}

/// Resolve the effective parse mode from the `format` param and the file content.
fn resolve_mode(format: &str, left: &str, right: &str) -> Result<Mode, String> {
    match format.trim() {
        "" | "auto" => Ok(if has_section(left) || has_section(right) {
            Mode::Ini
        } else {
            Mode::Env
        }),
        "env" => Ok(Mode::Env),
        "ini" => Ok(Mode::Ini),
        other => Err(format!(
            "invalid format {other:?}: expected \"auto\", \"env\" or \"ini\""
        )),
    }
}

/// Clean a raw value: unquote a `"..."`/`'...'` value (keeping only the quoted
/// span), else strip an unquoted inline `#`/`;` comment. Returns the bare value.
fn clean_value(raw: &str) -> String {
    let v = raw.trim();
    if let Some(q) = v.chars().next() {
        if q == '"' || q == '\'' {
            if let Some(end) = v[1..].find(q) {
                return v[1..1 + end].to_string();
            }
        }
    }
    // Unquoted: an inline comment begins at the first " #"/" ;" (or a tab before
    // the comment char). Full-line comments were already skipped by the caller.
    let cut = [" #", " ;", "\t#", "\t;"]
        .iter()
        .filter_map(|p| v.find(p))
        .min();
    match cut {
        Some(i) => v[..i].trim().to_string(),
        None => v.to_string(),
    }
}

/// Parse one config document into `(display_key, value)` pairs in file order
/// (duplicates kept — the caller's map keeps the last value, dotenv semantics).
fn parse(text: &str, mode: Mode) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut section = String::new();
    for raw in text.lines() {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }
        if mode == Mode::Ini
            && trimmed.len() >= 3
            && trimmed.starts_with('[')
            && trimmed.ends_with(']')
        {
            section = trimmed[1..trimmed.len() - 1].trim().to_string();
            continue;
        }
        // Strip an optional `export ` prefix (`export KEY=value`).
        let body = trimmed
            .strip_prefix("export ")
            .map(str::trim_start)
            .unwrap_or(trimmed);
        // Split on the first '='; in INI mode fall back to ':' when there is no
        // '=' (so `key: value` works while `url = postgres://…` still splits on '=').
        let sep = match body.find('=') {
            Some(i) => Some(i),
            None if mode == Mode::Ini => body.find(':'),
            None => None,
        };
        let Some(sep) = sep else {
            // Bareword with no separator — nothing to diff, skip it.
            continue;
        };
        let key = body[..sep].trim();
        if key.is_empty() {
            continue;
        }
        let value = clean_value(&body[sep + 1..]);
        let display = if mode == Mode::Ini && !section.is_empty() {
            format!("{section}.{key}")
        } else {
            key.to_string()
        };
        out.push((display, value));
    }
    out
}

/// A parsed file as a canonical-key → (display_key, value) map (last value wins).
/// The `BTreeMap` gives deterministic, alphabetically-sorted diff output.
struct Parsed {
    map: BTreeMap<String, (String, String)>,
}

fn build(entries: Vec<(String, String)>, ignore_case: bool) -> Parsed {
    let mut map = BTreeMap::new();
    for (key, value) in entries {
        let canon = if ignore_case {
            key.to_ascii_lowercase()
        } else {
            key.clone()
        };
        map.insert(canon, (key, value));
    }
    Parsed { map }
}

/// Whether a key name looks sensitive (for value masking).
fn is_sensitive(key: &str) -> bool {
    let upper = key.to_ascii_uppercase();
    SENSITIVE_MARKERS.iter().any(|m| upper.contains(m))
}

/// Mask a value: `ab****yz` for longer values, `****` for short/empty-safe ones.
fn mask(value: &str) -> String {
    let n = value.chars().count();
    if n == 0 {
        return String::new();
    }
    if n <= 4 {
        return "****".to_string();
    }
    let chars: Vec<char> = value.chars().collect();
    let first: String = chars[..2].iter().collect();
    let last: String = chars[n - 2..].iter().collect();
    format!("{first}****{last}")
}

/// A single value, masked when the key is sensitive and masking is on.
fn show(key: &str, value: &str, mask_secrets: bool) -> String {
    if mask_secrets && is_sensitive(key) {
        mask(value)
    } else {
        value.to_string()
    }
}

/// Diff two `.env`/`.ini` config files key-by-key.
///
/// - `left`: the first (old/base) config file contents.
/// - `right`: the second (new/compared) config file contents.
/// - `format`: `"auto"` (default — INI if either file has a `[section]` header,
///   else env), `"env"`, or `"ini"`.
/// - `ignore_case`: compare key names case-insensitively (`DB_HOST` == `db_host`).
/// - `mask_secrets`: mask values of sensitive-looking keys in the output.
/// - `output`: `"report"` (default) a grouped human summary, or `"json"` a
///   structured object.
///
/// Returns `Err` only for an unknown `format` or `output` mode.
pub fn diff(
    left: &str,
    right: &str,
    format: &str,
    ignore_case: bool,
    mask_secrets: bool,
    output: &str,
) -> Result<String, String> {
    let out_mode = if output.trim().is_empty() {
        "report"
    } else {
        output.trim()
    };
    if !matches!(out_mode, "report" | "json") {
        return Err(format!(
            "invalid output {out_mode:?}: expected \"report\" or \"json\""
        ));
    }
    let mode = resolve_mode(format, left, right)?;
    let l = build(parse(left, mode), ignore_case);
    let r = build(parse(right, mode), ignore_case);

    let keys: BTreeSet<&String> = l.map.keys().chain(r.map.keys()).collect();

    // Grouped buckets, each holding (display_key, ...) tuples in sorted order.
    let mut added: Vec<(String, String)> = Vec::new();
    let mut removed: Vec<(String, String)> = Vec::new();
    let mut changed: Vec<(String, String, String)> = Vec::new(); // key, old, new
    let mut unchanged: Vec<String> = Vec::new();

    for canon in keys {
        match (l.map.get(canon), r.map.get(canon)) {
            (None, Some((disp, val))) => added.push((disp.clone(), val.clone())),
            (Some((disp, val)), None) => removed.push((disp.clone(), val.clone())),
            (Some((disp, lv)), Some((_, rv))) => {
                if lv == rv {
                    unchanged.push(disp.clone());
                } else {
                    changed.push((disp.clone(), lv.clone(), rv.clone()));
                }
            }
            (None, None) => unreachable!("key came from one of the maps"),
        }
    }

    if out_mode == "json" {
        Ok(render_json(
            mode, &added, &removed, &changed, &unchanged, mask_secrets,
        ))
    } else {
        Ok(render_report(
            mode, &added, &removed, &changed, &unchanged, mask_secrets,
        ))
    }
}

fn render_report(
    mode: Mode,
    added: &[(String, String)],
    removed: &[(String, String)],
    changed: &[(String, String, String)],
    unchanged: &[String],
    mask_secrets: bool,
) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "Config diff ({}) — {} added, {} removed, {} changed, {} unchanged\n\n",
        mode.label(),
        added.len(),
        removed.len(),
        changed.len(),
        unchanged.len(),
    ));

    s.push_str(&format!("Added ({}):\n", added.len()));
    if added.is_empty() {
        s.push_str("  (none)\n");
    } else {
        for (k, v) in added {
            s.push_str(&format!("  + {k} = {}\n", show(k, v, mask_secrets)));
        }
    }
    s.push('\n');

    s.push_str(&format!("Removed ({}):\n", removed.len()));
    if removed.is_empty() {
        s.push_str("  (none)\n");
    } else {
        for (k, v) in removed {
            s.push_str(&format!("  - {k} = {}\n", show(k, v, mask_secrets)));
        }
    }
    s.push('\n');

    s.push_str(&format!("Changed ({}):\n", changed.len()));
    if changed.is_empty() {
        s.push_str("  (none)\n");
    } else {
        for (k, old, new) in changed {
            s.push_str(&format!(
                "  ~ {k}: {} -> {}\n",
                show(k, old, mask_secrets),
                show(k, new, mask_secrets)
            ));
        }
    }
    s.push('\n');

    s.push_str(&format!("Unchanged ({}):\n", unchanged.len()));
    if unchanged.is_empty() {
        s.push_str("  (none)");
    } else {
        s.push_str(&format!("  {}", unchanged.join(", ")));
    }
    s
}

fn render_json(
    mode: Mode,
    added: &[(String, String)],
    removed: &[(String, String)],
    changed: &[(String, String, String)],
    unchanged: &[String],
    mask_secrets: bool,
) -> String {
    use serde_json::{Map, Value};

    let mut added_obj = Map::new();
    for (k, v) in added {
        added_obj.insert(k.clone(), Value::String(show(k, v, mask_secrets)));
    }
    let mut removed_obj = Map::new();
    for (k, v) in removed {
        removed_obj.insert(k.clone(), Value::String(show(k, v, mask_secrets)));
    }
    let mut changed_obj = Map::new();
    for (k, old, new) in changed {
        let mut pair = Map::new();
        pair.insert("old".into(), Value::String(show(k, old, mask_secrets)));
        pair.insert("new".into(), Value::String(show(k, new, mask_secrets)));
        changed_obj.insert(k.clone(), Value::Object(pair));
    }
    let unchanged_arr: Vec<Value> = unchanged.iter().map(|k| Value::String(k.clone())).collect();

    let mut summary = Map::new();
    summary.insert("added".into(), Value::from(added.len()));
    summary.insert("removed".into(), Value::from(removed.len()));
    summary.insert("changed".into(), Value::from(changed.len()));
    summary.insert("unchanged".into(), Value::from(unchanged.len()));

    let mut root = Map::new();
    root.insert("format".into(), Value::String(mode.label().to_string()));
    root.insert("summary".into(), Value::Object(summary));
    root.insert("added".into(), Value::Object(added_obj));
    root.insert("removed".into(), Value::Object(removed_obj));
    root.insert("changed".into(), Value::Object(changed_obj));
    root.insert("unchanged".into(), Value::Array(unchanged_arr));

    serde_json::to_string_pretty(&Value::Object(root)).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_env_diff_report() {
        let left = "# base\nDB_HOST=localhost\nDB_PORT=5432\nDEBUG=true\n";
        let right = "DB_HOST=prod.internal\nDB_PORT=5432\nNEW_FLAG=on\n";
        let out = diff(left, right, "auto", false, false, "report").unwrap();
        assert_eq!(
            out,
            "Config diff (env) — 1 added, 1 removed, 1 changed, 1 unchanged\n\n\
             Added (1):\n  + NEW_FLAG = on\n\n\
             Removed (1):\n  - DEBUG = true\n\n\
             Changed (1):\n  ~ DB_HOST: localhost -> prod.internal\n\n\
             Unchanged (1):\n  DB_PORT"
        );
    }

    #[test]
    fn quotes_inline_comments_and_export_are_parsed() {
        let left = "export API_URL=\"http://old\"  # dev\n";
        let right = "API_URL='http://new' ; prod\n";
        let out = diff(left, right, "env", false, false, "report").unwrap();
        assert!(out.contains("~ API_URL: http://old -> http://new"), "{out}");
    }

    #[test]
    fn ini_sections_become_dotted_keys() {
        let left = "[db]\nhost = localhost\nport = 5432\n";
        let right = "[db]\nhost = 10.0.0.1\nport = 5432\n";
        let out = diff(left, right, "auto", false, false, "report").unwrap();
        assert!(out.starts_with("Config diff (ini) —"), "{out}");
        assert!(out.contains("~ db.host: localhost -> 10.0.0.1"), "{out}");
        assert!(out.contains("Unchanged (1):\n  db.port"), "{out}");
    }

    #[test]
    fn ignore_case_matches_keys() {
        let left = "DB_HOST=a\n";
        let right = "db_host=b\n";
        let sensitive = diff(left, right, "env", false, false, "report").unwrap();
        assert!(
            sensitive.contains("1 added, 1 removed, 0 changed"),
            "{sensitive}"
        );
        let folded = diff(left, right, "env", true, false, "report").unwrap();
        assert!(folded.contains("0 added, 0 removed, 1 changed"), "{folded}");
    }

    #[test]
    fn mask_secrets_hides_values() {
        let left = "API_TOKEN=devtoken123\n";
        let right = "API_TOKEN=prodtoken456\n";
        let masked = diff(left, right, "env", false, true, "report").unwrap();
        assert!(
            masked.contains("~ API_TOKEN: de****23 -> pr****56"),
            "{masked}"
        );
        assert!(!masked.contains("devtoken123"), "{masked}");
    }

    #[test]
    fn json_output_shape() {
        let left = "A=1\nB=2\n";
        let right = "A=9\nC=3\n";
        let out = diff(left, right, "env", false, false, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["summary"]["added"], 1);
        assert_eq!(v["summary"]["removed"], 1);
        assert_eq!(v["summary"]["changed"], 1);
        assert_eq!(v["added"]["C"], "3");
        assert_eq!(v["removed"]["B"], "2");
        assert_eq!(v["changed"]["A"]["old"], "1");
        assert_eq!(v["changed"]["A"]["new"], "9");
        assert_eq!(v["format"], "env");
    }

    #[test]
    fn invalid_output_mode_errors() {
        let err = diff("A=1", "A=2", "env", false, false, "yaml").unwrap_err();
        assert!(err.contains("invalid output"), "{err}");
    }

    #[test]
    fn invalid_format_errors() {
        let err = diff("A=1", "A=2", "toml", false, false, "report").unwrap_err();
        assert!(err.contains("invalid format"), "{err}");
    }
}

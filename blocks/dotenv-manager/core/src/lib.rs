//! dotenv-manager core — parse, validate, merge and secret-mask `.env` files.
//! Pure compute (only serde_json for the JSON output), shared by the chat skill
//! block and the web page. No wafer/wasm-bindgen deps.
//!
//! Pipeline: parse `env` → overlay `merge` (its keys win) → collect lint
//! warnings + duplicate/missing-required diagnostics → render one of four
//! outputs (`report` | `normalized` | `example` | `json`), masking values whose
//! KEY name looks sensitive when `mask_secrets` is on.

/// A single parsed `KEY=value` assignment, with the 1-based source line it came
/// from (for duplicate/lint diagnostics).
#[derive(Clone, Debug)]
struct Entry {
    key: String,
    value: String,
    line: usize,
    /// True when the key came from the `merge` overlay rather than the primary file.
    from_overlay: bool,
}

/// A lint warning tied to a source line.
#[derive(Clone, Debug)]
struct Warning {
    line: usize,
    message: String,
}

/// Substrings (matched case-insensitively against the KEY name) that mark a
/// value as sensitive and therefore masked when `mask_secrets` is on. Chosen to
/// catch the common secret-bearing names without touching plain config keys.
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

/// Parse, validate, merge and (optionally) secret-mask a `.env` file.
///
/// - `env`: the primary `.env` file contents.
/// - `merge`: an optional overlay `.env`; its keys OVERRIDE matching keys from
///   `env` (later-file-wins, the runtime dotenv semantics).
/// - `required_keys`: comma-separated key names that MUST be present in the
///   merged result; missing ones are reported (matched case-sensitively).
/// - `mask_secrets`: when true, values of sensitive-looking keys are masked in
///   every value-bearing output.
/// - `sort_keys`: when true, keys are emitted alphabetically; otherwise in
///   first-seen order.
/// - `output`: `"report"` (default) a human diagnostic summary; `"normalized"`
///   deduped `KEY=value` lines (last value wins); `"example"` a `.env.example`
///   with blanked values; `"json"` a JSON object of the merged pairs.
///
/// Returns `Err` only for an unknown `output` mode — malformed lines are
/// reported as warnings, never fatal.
pub fn manage(
    env: &str,
    merge: &str,
    required_keys: &str,
    mask_secrets: bool,
    sort_keys: bool,
    output: &str,
) -> Result<String, String> {
    let mode = if output.trim().is_empty() {
        "report"
    } else {
        output.trim()
    };
    if !matches!(mode, "report" | "normalized" | "example" | "json") {
        return Err(format!(
            "invalid output {mode:?}: expected \"report\", \"normalized\", \"example\" or \"json\""
        ));
    }

    let mut warnings = Vec::new();
    let primary = parse(env, false, &mut warnings);
    let overlay = parse(merge, true, &mut warnings);

    // Duplicate detection within the primary file (over ALL primary entries).
    let duplicates = find_duplicates(&primary);

    // Merge: primary in first-seen order, then overlay keys override in place
    // (or append if new). This is the last-file-wins runtime semantics.
    let merged = merge_entries(&primary, &overlay);

    // Missing required keys (against the merged key set).
    let missing = missing_required(required_keys, &merged);

    // Ordered view for value-bearing outputs.
    let mut ordered: Vec<&Entry> = merged.iter().collect();
    if sort_keys {
        ordered.sort_by(|a, b| a.key.cmp(&b.key));
    }

    let out = match mode {
        "normalized" => render_normalized(&ordered, mask_secrets),
        "example" => render_example(&ordered),
        "json" => render_json(&ordered, mask_secrets),
        _ => render_report(
            env,
            &merged,
            &ordered,
            &duplicates,
            &missing,
            &warnings,
            mask_secrets,
        ),
    };
    Ok(out)
}

/// Parse one `.env` document into entries, pushing lint warnings for malformed
/// or non-conventional lines.
fn parse(text: &str, from_overlay: bool, warnings: &mut Vec<Warning>) -> Vec<Entry> {
    let mut entries = Vec::new();
    for (idx, raw) in text.lines().enumerate() {
        let line = idx + 1;
        let trimmed = raw.trim();
        // Blank line or full-line comment — skip silently.
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        // Strip an optional `export ` prefix (`export KEY=value`).
        let body = trimmed
            .strip_prefix("export ")
            .map(str::trim_start)
            .unwrap_or(trimmed);

        let Some(eq) = body.find('=') else {
            // A bareword with no '=' is a key without a value.
            warnings.push(Warning {
                line,
                message: format!("'{body}' has no value (missing '=')"),
            });
            continue;
        };

        let raw_key = &body[..eq];
        let raw_val = &body[eq + 1..];

        // Whitespace directly around '=' is non-standard for dotenv loaders.
        if raw_key.ends_with(char::is_whitespace) || raw_val.starts_with(char::is_whitespace) {
            warnings.push(Warning {
                line,
                message: format!(
                    "key '{}' has whitespace around '=' (some loaders keep it)",
                    raw_key.trim()
                ),
            });
        }

        let key = raw_key.trim().to_string();
        if key.is_empty() {
            warnings.push(Warning {
                line,
                message: "empty key before '='".to_string(),
            });
            continue;
        }
        if !is_conventional_key(&key) {
            warnings.push(Warning {
                line,
                message: format!("key '{key}' is not UPPER_SNAKE_CASE"),
            });
        }

        let value = parse_value(raw_val, line, warnings);
        if value.is_empty() {
            warnings.push(Warning {
                line,
                message: format!("key '{key}' has an empty value"),
            });
        }
        entries.push(Entry {
            key,
            value,
            line,
            from_overlay,
        });
    }
    entries
}

/// Extract the value from the right-hand side of a `KEY=` assignment: strip
/// surrounding quotes (unescaping `\n`/`\t`/`\\`/`\"` inside double quotes),
/// then drop any trailing ` # inline comment`. For an unquoted value drop a
/// trailing ` # inline comment` and trailing whitespace.
fn parse_value(raw: &str, line: usize, warnings: &mut Vec<Warning>) -> String {
    let v = raw.trim_start();
    if v.starts_with('"') {
        // Find the closing quote, skipping `\"` escapes.
        if let Some(close) = find_closing_quote(v, '"', true) {
            return unescape_double(&v[1..close]);
        }
        warnings.push(Warning {
            line,
            message: "unterminated double quote".to_string(),
        });
        return v.to_string();
    }
    if v.starts_with('\'') {
        // Single quotes are literal — no escape processing.
        if let Some(close) = find_closing_quote(v, '\'', false) {
            return v[1..close].to_string();
        }
        warnings.push(Warning {
            line,
            message: "unterminated single quote".to_string(),
        });
        return v.to_string();
    }
    // Unquoted: an inline ` #comment` (space then '#') ends the value.
    let end = find_inline_comment(v).unwrap_or(v.len());
    v[..end].trim_end().to_string()
}

/// Byte index of the closing `quote` in a value that opens with it. When
/// `escapes` is true a `\` escapes the following char (so `\"` is not a close).
/// Anything after the close (typically a ` # comment`) is ignored by the caller.
fn find_closing_quote(v: &str, quote: char, escapes: bool) -> Option<usize> {
    let bytes = v.as_bytes();
    let q = quote as u8;
    let mut i = 1;
    while i < bytes.len() {
        if escapes && bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if bytes[i] == q {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Find the start of an unquoted inline comment: a `#` preceded by whitespace
/// (a `#` with no leading space is treated as part of the value, e.g. a URL
/// fragment or a literal `pass#word`).
fn find_inline_comment(v: &str) -> Option<usize> {
    let bytes = v.as_bytes();
    for i in 1..bytes.len() {
        if bytes[i] == b'#' && bytes[i - 1].is_ascii_whitespace() {
            return Some(i);
        }
    }
    None
}

/// Unescape the common backslash sequences inside a double-quoted value.
fn unescape_double(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// A key is "conventional" when it is `UPPER_SNAKE_CASE`: starts with a letter
/// or underscore, then uppercase letters, digits and underscores only.
fn is_conventional_key(key: &str) -> bool {
    let mut chars = key.chars();
    match chars.next() {
        Some(c) if c == '_' || c.is_ascii_uppercase() => {}
        _ => return false,
    }
    key.chars()
        .all(|c| c == '_' || c.is_ascii_uppercase() || c.is_ascii_digit())
}

/// Keys that appear more than once in the primary file, with all their source
/// lines. Returned sorted by key for stable output.
fn find_duplicates(primary: &[Entry]) -> Vec<(String, Vec<usize>)> {
    let mut order: Vec<String> = Vec::new();
    let mut lines: std::collections::HashMap<String, Vec<usize>> = std::collections::HashMap::new();
    for e in primary {
        if !lines.contains_key(&e.key) {
            order.push(e.key.clone());
        }
        lines.entry(e.key.clone()).or_default().push(e.line);
    }
    let mut dups: Vec<(String, Vec<usize>)> = order
        .into_iter()
        .filter(|k| lines[k].len() > 1)
        .map(|k| {
            let v = lines[&k].clone();
            (k, v)
        })
        .collect();
    dups.sort_by(|a, b| a.0.cmp(&b.0));
    dups
}

/// Merge primary + overlay into the final key set. Primary keeps first-seen
/// order with the LAST value winning for in-file duplicates; overlay keys
/// override an existing key in place, or append if new.
fn merge_entries(primary: &[Entry], overlay: &[Entry]) -> Vec<Entry> {
    let mut order: Vec<String> = Vec::new();
    let mut map: std::collections::HashMap<String, Entry> = std::collections::HashMap::new();
    for e in primary.iter().chain(overlay.iter()) {
        if !map.contains_key(&e.key) {
            order.push(e.key.clone());
        }
        map.insert(e.key.clone(), e.clone());
    }
    order.into_iter().map(|k| map[&k].clone()).collect()
}

/// Required keys (comma-separated, trimmed, blanks ignored) absent from the
/// merged set, in the order they were requested.
fn missing_required(required_keys: &str, merged: &[Entry]) -> Vec<String> {
    let present: std::collections::HashSet<&str> = merged.iter().map(|e| e.key.as_str()).collect();
    required_keys
        .split(',')
        .map(str::trim)
        .filter(|k| !k.is_empty())
        .filter(|k| !present.contains(*k))
        .map(str::to_string)
        .collect()
}

/// Mask a value: reveal the first two and last two characters for longer
/// secrets, otherwise fully mask. Deterministic and independent of terminal
/// width. Empty values stay empty.
fn mask(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.is_empty() {
        return String::new();
    }
    if chars.len() <= 6 {
        return "****".to_string();
    }
    let first: String = chars[..2].iter().collect();
    let last: String = chars[chars.len() - 2..].iter().collect();
    format!("{first}****{last}")
}

/// True when this key's value should be masked (name matches a sensitive marker).
fn is_sensitive(key: &str) -> bool {
    let upper = key.to_ascii_uppercase();
    SENSITIVE_MARKERS.iter().any(|m| upper.contains(m))
}

/// Render a value for output, masking it when `mask_secrets` and the key is
/// sensitive.
fn value_for(entry: &Entry, mask_secrets: bool) -> String {
    if mask_secrets && is_sensitive(&entry.key) {
        mask(&entry.value)
    } else {
        entry.value.clone()
    }
}

/// Quote a value for `KEY=value` output only when it needs it (contains
/// whitespace, `#`, a quote, or is empty) so round-tripping stays clean.
fn quote_if_needed(value: &str) -> String {
    let needs = value.is_empty()
        || value.contains(char::is_whitespace)
        || value.contains('#')
        || value.contains('"');
    if needs {
        let escaped = value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\t', "\\t")
            .replace('\r', "\\r");
        format!("\"{escaped}\"")
    } else {
        value.to_string()
    }
}

fn render_normalized(ordered: &[&Entry], mask_secrets: bool) -> String {
    ordered
        .iter()
        .map(|e| format!("{}={}", e.key, quote_if_needed(&value_for(e, mask_secrets))))
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_example(ordered: &[&Entry]) -> String {
    ordered
        .iter()
        .map(|e| format!("{}=", e.key))
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_json(ordered: &[&Entry], mask_secrets: bool) -> String {
    let mut map = serde_json::Map::new();
    for e in ordered {
        map.insert(
            e.key.clone(),
            serde_json::Value::String(value_for(e, mask_secrets)),
        );
    }
    serde_json::to_string_pretty(&serde_json::Value::Object(map)).unwrap_or_else(|_| "{}".into())
}

#[allow(clippy::too_many_arguments)]
fn render_report(
    env: &str,
    merged: &[Entry],
    ordered: &[&Entry],
    duplicates: &[(String, Vec<usize>)],
    missing: &[String],
    warnings: &[Warning],
    mask_secrets: bool,
) -> String {
    let overlay_count = merged.iter().filter(|e| e.from_overlay).count();
    let mut out = String::new();
    out.push_str(".env summary\n");
    out.push_str("============\n");
    out.push_str(&format!("Lines: {}\n", env.lines().count()));
    out.push_str(&format!("Keys: {}\n", merged.len()));
    out.push_str(&format!("Duplicate keys: {}\n", duplicates.len()));
    out.push_str(&format!("Overridden by merge: {overlay_count}\n"));
    out.push_str(&format!("Missing required: {}\n", missing.len()));
    out.push_str(&format!("Warnings: {}\n", warnings.len()));

    if !duplicates.is_empty() {
        out.push_str("\nDuplicate keys (last value wins):\n");
        for (k, lines) in duplicates {
            let joined = lines
                .iter()
                .map(|l| l.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!("  {k} (lines {joined})\n"));
        }
    }
    if !missing.is_empty() {
        out.push_str("\nMissing required keys:\n");
        for k in missing {
            out.push_str(&format!("  {k}\n"));
        }
    }
    if !warnings.is_empty() {
        out.push_str("\nWarnings:\n");
        for w in warnings {
            out.push_str(&format!("  line {}: {}\n", w.line, w.message));
        }
    }

    out.push_str(if mask_secrets {
        "\nValues (secrets masked):\n"
    } else {
        "\nValues:\n"
    });
    for e in ordered {
        out.push_str(&format!("  {}={}\n", e.key, value_for(e, mask_secrets)));
    }
    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_dedupes_last_wins_and_masks() {
        let env = "# db\nDB_HOST=localhost\nDB_PORT=5432\nexport API_TOKEN=secret123456\nDB_HOST=127.0.0.1\n";
        let out = manage(env, "", "", true, false, "normalized").unwrap();
        assert_eq!(out, "DB_HOST=127.0.0.1\nDB_PORT=5432\nAPI_TOKEN=se****56");
    }

    #[test]
    fn merge_overlay_overrides_and_appends() {
        let env = "A_VAL=1\nB_VAL=2\n";
        let overlay = "B_VAL=99\nC_VAL=3\n";
        let out = manage(env, overlay, "", false, false, "normalized").unwrap();
        // A keeps place, B overridden in place, C appended.
        assert_eq!(out, "A_VAL=1\nB_VAL=99\nC_VAL=3");
    }

    #[test]
    fn masking_reveals_ends_for_long_and_hides_short() {
        assert_eq!(mask("secret123456"), "se****56");
        assert_eq!(mask("abc"), "****");
        assert_eq!(mask(""), "");
    }

    #[test]
    fn quotes_and_inline_comments_are_parsed() {
        let env = "NAME=\"John Doe\" # full name\nRAW=plain#notacomment\nESC=\"a\\nb\"\n";
        let out = manage(env, "", "", false, false, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["NAME"], "John Doe");
        assert_eq!(v["RAW"], "plain#notacomment");
        assert_eq!(v["ESC"], "a\nb");
    }

    #[test]
    fn report_flags_duplicates_missing_and_lint() {
        let env = "db_user=admin\nDB_USER=root\nDB_USER=root\n";
        let out = manage(env, "", "API_KEY, DB_USER", false, true, "report").unwrap();
        assert!(out.contains("Duplicate keys: 1"), "{out}");
        assert!(out.contains("DB_USER (lines 2, 3)"), "{out}");
        assert!(out.contains("Missing required keys:\n  API_KEY"), "{out}");
        assert!(out.contains("not UPPER_SNAKE_CASE"), "{out}");
    }

    #[test]
    fn example_blanks_values_and_sorts() {
        let env = "B_KEY=2\nA_KEY=1\n";
        let out = manage(env, "", "", true, true, "example").unwrap();
        assert_eq!(out, "A_KEY=\nB_KEY=");
    }

    #[test]
    fn sensitive_detection_is_name_based() {
        assert!(is_sensitive("API_SECRET"));
        assert!(is_sensitive("db_password"));
        assert!(is_sensitive("MY_TOKEN"));
        assert!(!is_sensitive("DB_HOST"));
        assert!(!is_sensitive("PORT"));
    }

    #[test]
    fn invalid_output_mode_errors() {
        let err = manage("A=1", "", "", true, false, "yaml").unwrap_err();
        assert!(err.contains("invalid output"), "{err}");
    }

    #[test]
    fn empty_input_produces_empty_normalized() {
        assert_eq!(manage("", "", "", true, false, "normalized").unwrap(), "");
    }
}

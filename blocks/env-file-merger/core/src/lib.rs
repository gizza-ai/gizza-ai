//! env-file-merger core — resolve a stack of layered `.env` files into one
//! environment and record which layer set each value. Shared by the chat skill
//! block and the web page; no wafer/wasm-bindgen deps (only serde_json for the
//! JSON output).
//!
//! Up to four layers are merged in ascending priority — the same cascade
//! real-world loaders use (`.env` → `.env.local` → `.env.<mode>` →
//! `.env.<mode>.local`). Each layer is parsed with the usual dotenv rules
//! (`KEY=VALUE`, `#` comments, blank lines, single/double quotes, inline
//! comments, an optional `export ` prefix); the LAST layer that assigns a key
//! wins, and every value it shadowed is kept so the tool can answer "which file
//! set this, and what did it override?".
//!
//! Nothing is fetched or written — a blank layer is simply skipped, matching
//! loaders that tolerate missing files.

use std::collections::{HashMap, HashSet};

/// Number of layer slots the tool accepts, lowest priority first.
pub const LAYER_SLOTS: usize = 4;

/// Layer names used when the caller does not supply their own. This is the
/// conventional cascade order shared by dotenv-flow, Vite and Next.js.
const DEFAULT_LAYER_NAMES: [&str; LAYER_SLOTS] = [
    ".env",
    ".env.local",
    ".env.production",
    ".env.production.local",
];

/// Substrings (matched case-insensitively against the KEY name) that mark a
/// value as sensitive and therefore masked when `mask_secrets` is on. Mirrors
/// the dotenv-manager / ini-env-diff list so masking is consistent across the
/// `.env` tools.
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

/// Upper bound on the number of distinct keys in the merged result, so a
/// pathological paste can't wedge the browser or CLI.
const MAX_KEYS: usize = 20_000;

/// How many nested `${VAR}` levels are followed before giving up.
const MAX_EXPAND_DEPTH: usize = 16;

/// One `KEY=value` assignment read out of a single layer.
#[derive(Clone)]
struct Assignment {
    key: String,
    value: String,
    /// True when the value was written in single quotes, which are literal:
    /// such a value is never `${VAR}`-expanded (POSIX / dotenvx semantics).
    literal: bool,
    /// Index into the list of PRESENT layers, lowest priority first.
    layer: usize,
}

/// A diagnostic tied to a layer + line.
struct Warning {
    layer: Option<usize>,
    line: usize,
    message: String,
}

/// A key after resolution: the winning value plus every assignment that led to
/// it, in ascending priority order (the last entry is the winner).
struct Resolved {
    key: String,
    value: String,
    literal: bool,
    /// (layer index, value) for every layer that assigned this key.
    chain: Vec<(usize, String)>,
}

impl Resolved {
    /// Index of the layer whose value won.
    fn source(&self) -> usize {
        self.chain.last().map(|(l, _)| *l).unwrap_or(0)
    }

    /// The assignments that were shadowed, lowest priority first.
    fn overridden(&self) -> &[(usize, String)] {
        let n = self.chain.len();
        &self.chain[..n.saturating_sub(1)]
    }
}

/// Merge up to four layered `.env` documents into one resolved environment.
///
/// - `layers`: the layer texts, LOWEST priority first. Blank entries are
///   skipped (a missing file is not an error).
/// - `layer_names`: comma-separated display names mapped positionally onto the
///   layer slots; blank falls back to the conventional cascade names.
/// - `output`: `report` | `env` | `json` | `shell` | `markdown` | `conflicts`.
/// - `mask_secrets`: mask values of sensitive-looking keys in every output.
/// - `sort_keys`: alphabetical instead of first-seen order.
/// - `prefix_filter`: keep only keys starting with this prefix (e.g. `VITE_`).
/// - `expand_vars`: resolve `${VAR}` / `$VAR` / `${VAR:-fallback}` references
///   against the merged result.
pub fn merge(
    layers: &[&str],
    layer_names: &str,
    output: &str,
    mask_secrets: bool,
    sort_keys: bool,
    prefix_filter: &str,
    expand_vars: bool,
) -> Result<String, String> {
    if !matches!(
        output,
        "report" | "env" | "json" | "shell" | "markdown" | "conflicts"
    ) {
        return Err(format!(
            "unknown output '{output}': expected report, env, json, shell, markdown or conflicts"
        ));
    }

    let names = resolve_names(layer_names);
    // Keep only the layers that actually carry content, remembering their name.
    let present: Vec<(String, &str)> = layers
        .iter()
        .take(LAYER_SLOTS)
        .enumerate()
        .filter(|(_, text)| !text.trim().is_empty())
        .map(|(i, text)| (names[i].clone(), *text))
        .collect();
    if present.is_empty() {
        return Err(
            "no .env content: paste at least one layer (layer 1 is the base file)".to_string(),
        );
    }

    let mut warnings: Vec<Warning> = Vec::new();
    let mut assignments: Vec<Assignment> = Vec::new();
    for (idx, (_, text)) in present.iter().enumerate() {
        parse_layer(text, idx, &mut assignments, &mut warnings);
    }

    let mut resolved = resolve(&assignments)?;
    if expand_vars {
        expand_all(&mut resolved, &mut warnings);
    }
    let prefix = prefix_filter.trim();
    if !prefix.is_empty() {
        resolved.retain(|r| r.key.starts_with(prefix));
    }
    if sort_keys {
        resolved.sort_by(|a, b| a.key.cmp(&b.key));
    }

    let names: Vec<&str> = present.iter().map(|(n, _)| n.as_str()).collect();
    Ok(match output {
        "env" => render_env(&resolved, &names, mask_secrets),
        "json" => render_json(&resolved, &names, &warnings, mask_secrets),
        "shell" => render_shell(&resolved, mask_secrets),
        "markdown" => render_markdown(&resolved, &names, mask_secrets),
        "conflicts" => render_conflicts(&resolved, &names, mask_secrets),
        _ => render_report(&resolved, &names, &warnings, mask_secrets),
    })
}

/// Map the comma-separated `layer_names` onto the layer slots, falling back to
/// the conventional cascade names (and then to `layer N`) for blanks.
fn resolve_names(layer_names: &str) -> Vec<String> {
    let given: Vec<&str> = layer_names.split(',').map(str::trim).collect();
    (0..LAYER_SLOTS)
        .map(|i| {
            let name = given.get(i).copied().unwrap_or("");
            if !name.is_empty() {
                name.to_string()
            } else if let Some(d) = DEFAULT_LAYER_NAMES.get(i) {
                d.to_string()
            } else {
                format!("layer {}", i + 1)
            }
        })
        .collect()
}

/// Parse one `.env` document into assignments, recording lint warnings for
/// malformed or non-conventional lines.
fn parse_layer(
    text: &str,
    layer: usize,
    assignments: &mut Vec<Assignment>,
    warnings: &mut Vec<Warning>,
) {
    let mut seen_here: HashMap<String, usize> = HashMap::new();
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
            warnings.push(Warning {
                layer: Some(layer),
                line,
                message: format!("'{body}' has no value (missing '=')"),
            });
            continue;
        };

        let raw_key = &body[..eq];
        let raw_val = &body[eq + 1..];
        let key = raw_key.trim().to_string();
        if key.is_empty() {
            warnings.push(Warning {
                layer: Some(layer),
                line,
                message: "empty key before '='".to_string(),
            });
            continue;
        }
        if !is_conventional_key(&key) {
            warnings.push(Warning {
                layer: Some(layer),
                line,
                message: format!("key '{key}' is not UPPER_SNAKE_CASE"),
            });
        }
        if let Some(first) = seen_here.insert(key.clone(), line) {
            warnings.push(Warning {
                layer: Some(layer),
                line,
                message: format!("key '{key}' repeats within this layer (also on line {first}); the later line wins"),
            });
        }

        let (value, literal) = parse_value(raw_val, layer, line, warnings);
        assignments.push(Assignment {
            key,
            value,
            literal,
            layer,
        });
    }
}

/// Extract the value from the right-hand side of a `KEY=` assignment: strip
/// surrounding quotes (unescaping `\n`/`\t`/`\\`/`\"` inside double quotes),
/// then drop any trailing ` # inline comment`. Returns the value plus whether
/// it was single-quoted (literal, never expanded).
fn parse_value(
    raw: &str,
    layer: usize,
    line: usize,
    warnings: &mut Vec<Warning>,
) -> (String, bool) {
    let v = raw.trim_start();
    if v.starts_with('"') {
        if let Some(close) = find_closing_quote(v, '"', true) {
            return (unescape_double(&v[1..close]), false);
        }
        warnings.push(Warning {
            layer: Some(layer),
            line,
            message: "unterminated double quote".to_string(),
        });
        return (v.to_string(), false);
    }
    if v.starts_with('\'') {
        // Single quotes are literal — no escape processing, no expansion.
        if let Some(close) = find_closing_quote(v, '\'', false) {
            return (v[1..close].to_string(), true);
        }
        warnings.push(Warning {
            layer: Some(layer),
            line,
            message: "unterminated single quote".to_string(),
        });
        return (v.to_string(), true);
    }
    // Unquoted: an inline ` #comment` (whitespace then '#') ends the value.
    let end = find_inline_comment(v).unwrap_or(v.len());
    (v[..end].trim_end().to_string(), false)
}

/// Byte index of the closing `quote` in a value that opens with it. When
/// `escapes` is true a `\` escapes the following char (so `\"` is not a close).
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

/// Start of an unquoted inline comment: a `#` preceded by whitespace (a `#`
/// with no leading space stays part of the value, e.g. a URL fragment).
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

/// Fold the per-layer assignments into one resolved set. Keys keep first-seen
/// order across the cascade; within a layer the later line wins, and a later
/// layer always overrides an earlier one.
fn resolve(assignments: &[Assignment]) -> Result<Vec<Resolved>, String> {
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut out: Vec<Resolved> = Vec::new();
    for a in assignments {
        match index.get(&a.key) {
            Some(&i) => {
                let r = &mut out[i];
                // A repeat within the SAME layer replaces that layer's entry
                // rather than adding a second link to the chain.
                if r.chain.last().map(|(l, _)| *l) == Some(a.layer) {
                    r.chain.pop();
                }
                r.chain.push((a.layer, a.value.clone()));
                r.value = a.value.clone();
                r.literal = a.literal;
            }
            None => {
                if out.len() >= MAX_KEYS {
                    return Err(format!(
                        "too many variables: the merged result is capped at {MAX_KEYS} keys"
                    ));
                }
                index.insert(a.key.clone(), out.len());
                out.push(Resolved {
                    key: a.key.clone(),
                    value: a.value.clone(),
                    literal: a.literal,
                    chain: vec![(a.layer, a.value.clone())],
                });
            }
        }
    }
    Ok(out)
}

/// Expand `${VAR}`, `${VAR:-fallback}` and `$VAR` references in every
/// non-literal value against the merged result. Unresolved names become empty
/// strings (the dotenv-expand convention) and are reported as warnings.
fn expand_all(resolved: &mut [Resolved], warnings: &mut Vec<Warning>) {
    let map: HashMap<String, (String, bool)> = resolved
        .iter()
        .map(|r| (r.key.clone(), (r.value.clone(), r.literal)))
        .collect();
    let mut unresolved: Vec<(String, String)> = Vec::new();
    let mut circular: Vec<(String, String)> = Vec::new();
    for r in resolved.iter_mut() {
        if r.literal {
            continue;
        }
        let mut seen: HashSet<String> = HashSet::new();
        seen.insert(r.key.clone());
        let expanded = expand_value(
            &r.value,
            &map,
            &mut seen,
            0,
            &r.key,
            &mut unresolved,
            &mut circular,
        );
        r.value = expanded;
        // The winning chain entry must reflect what the tool now reports.
        if let Some(last) = r.chain.last_mut() {
            last.1 = r.value.clone();
        }
    }
    for (key, name) in circular {
        warnings.push(Warning {
            layer: None,
            line: 0,
            message: format!("{key}: circular reference to ${name} — expanded to an empty string"),
        });
    }
    for (key, name) in unresolved {
        warnings.push(Warning {
            layer: None,
            line: 0,
            message: format!(
                "{key}: ${name} is not defined in any layer — expanded to an empty string"
            ),
        });
    }
}

/// Expand one value. `seen` holds the names currently being expanded so a cycle
/// terminates instead of recursing forever.
#[allow(clippy::too_many_arguments)]
fn expand_value(
    raw: &str,
    map: &HashMap<String, (String, bool)>,
    seen: &mut HashSet<String>,
    depth: usize,
    owner: &str,
    unresolved: &mut Vec<(String, String)>,
    circular: &mut Vec<(String, String)>,
) -> String {
    if depth >= MAX_EXPAND_DEPTH {
        return raw.to_string();
    }
    let bytes = raw.as_bytes();
    let mut out = String::with_capacity(raw.len());
    let mut i = 0;
    while i < bytes.len() {
        // `\$` is a literal dollar sign.
        if bytes[i] == b'\\' && i + 1 < bytes.len() && bytes[i + 1] == b'$' {
            out.push('$');
            i += 2;
            continue;
        }
        if bytes[i] != b'$' {
            let ch_len = char_len(raw, i);
            out.push_str(&raw[i..i + ch_len]);
            i += ch_len;
            continue;
        }
        // `${NAME}` / `${NAME:-fallback}` / `${NAME-fallback}`
        if i + 1 < bytes.len() && bytes[i + 1] == b'{' {
            let Some(close) = raw[i + 2..].find('}').map(|p| i + 2 + p) else {
                out.push('$');
                i += 1;
                continue;
            };
            let inner = &raw[i + 2..close];
            let (name, fallback) = match inner.find(":-").or_else(|| inner.find('-')) {
                Some(p) => {
                    let skip = if inner[p..].starts_with(":-") { 2 } else { 1 };
                    (&inner[..p], Some(&inner[p + skip..]))
                }
                None => (inner, None),
            };
            out.push_str(&lookup(
                name, fallback, map, seen, depth, owner, unresolved, circular,
            ));
            i = close + 1;
            continue;
        }
        // Bare `$NAME`.
        let start = i + 1;
        let mut end = start;
        while end < bytes.len() && (bytes[end] == b'_' || bytes[end].is_ascii_alphanumeric()) {
            end += 1;
        }
        if end == start || bytes[start].is_ascii_digit() {
            out.push('$');
            i += 1;
            continue;
        }
        out.push_str(&lookup(
            &raw[start..end],
            None,
            map,
            seen,
            depth,
            owner,
            unresolved,
            circular,
        ));
        i = end;
    }
    out
}

/// Resolve one referenced name, recursing so `A=${B}` where `B=${C}` works.
#[allow(clippy::too_many_arguments)]
fn lookup(
    name: &str,
    fallback: Option<&str>,
    map: &HashMap<String, (String, bool)>,
    seen: &mut HashSet<String>,
    depth: usize,
    owner: &str,
    unresolved: &mut Vec<(String, String)>,
    circular: &mut Vec<(String, String)>,
) -> String {
    if name.is_empty() {
        return String::new();
    }
    if seen.contains(name) {
        circular.push((owner.to_string(), name.to_string()));
        return String::new();
    }
    match map.get(name) {
        Some((value, literal)) => {
            if *literal {
                return value.clone();
            }
            seen.insert(name.to_string());
            let out = expand_value(value, map, seen, depth + 1, owner, unresolved, circular);
            seen.remove(name);
            out
        }
        None => match fallback {
            Some(f) => expand_value(f, map, seen, depth + 1, owner, unresolved, circular),
            None => {
                unresolved.push((owner.to_string(), name.to_string()));
                String::new()
            }
        },
    }
}

/// Byte length of the UTF-8 character starting at `i`.
fn char_len(s: &str, i: usize) -> usize {
    s[i..].chars().next().map(char::len_utf8).unwrap_or(1)
}

fn is_sensitive(key: &str) -> bool {
    let upper = key.to_ascii_uppercase();
    SENSITIVE_MARKERS.iter().any(|m| upper.contains(m))
}

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

/// Render a value for output, masking it when `mask_secrets` and the key looks
/// sensitive.
fn shown(key: &str, value: &str, mask_secrets: bool) -> String {
    if mask_secrets && is_sensitive(key) {
        mask(value)
    } else {
        value.to_string()
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

/// POSIX single-quote a value for `export KEY='value'`.
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// `KEY=value` for one resolved variable. A value that was written in single
/// quotes and still carries a `$` is re-emitted in SINGLE quotes, so a loader
/// that expands variables leaves it alone exactly as the source did — double
/// quotes would not stop the re-expansion.
fn assignment(key: &str, value: &str, literal: bool) -> String {
    if literal && value.contains('$') {
        format!("{key}={}", shell_quote(value))
    } else {
        format!("{key}={}", quote_if_needed(value))
    }
}

fn warning_line(w: &Warning, names: &[&str]) -> String {
    match w.layer {
        Some(l) => format!(
            "  {} line {}: {}",
            names.get(l).copied().unwrap_or("?"),
            w.line,
            w.message
        ),
        None => format!("  {}", w.message),
    }
}

/// Keys that were assigned by more than one layer.
fn conflicting(resolved: &[Resolved]) -> Vec<&Resolved> {
    resolved.iter().filter(|r| r.chain.len() > 1).collect()
}

/// The `NAME  = value` chain block shared by the report and conflicts outputs.
fn chain_block(r: &Resolved, names: &[&str], mask_secrets: bool) -> String {
    let width = r
        .chain
        .iter()
        .map(|(l, _)| names.get(*l).copied().unwrap_or("?").chars().count())
        .max()
        .unwrap_or(0);
    let mut out = format!("{}\n", r.key);
    for (i, (layer, value)) in r.chain.iter().enumerate() {
        let name = names.get(*layer).copied().unwrap_or("?");
        let tag = if i + 1 == r.chain.len() {
            "  (wins)"
        } else {
            ""
        };
        out.push_str(&format!(
            "  {name:<width$} = {}{tag}\n",
            shown(&r.key, value, mask_secrets)
        ));
    }
    out
}

fn render_report(
    resolved: &[Resolved],
    names: &[&str],
    warnings: &[Warning],
    mask_secrets: bool,
) -> String {
    let conflicts = conflicting(resolved);
    let mut out = String::from(".env merge report\n=================\n");
    out.push_str(&format!(
        "Layers merged: {} ({})\n",
        names.len(),
        names.join(" < ")
    ));
    out.push_str(&format!("Keys resolved: {}\n", resolved.len()));
    out.push_str(&format!("Keys overridden: {}\n", conflicts.len()));
    out.push_str(&format!("Warnings: {}\n", warnings.len()));

    out.push_str("\nResolved environment\n--------------------\n");
    if resolved.is_empty() {
        out.push_str("(no keys matched)\n");
    }
    for r in resolved {
        out.push_str(&format!(
            "{}  # set by {}\n",
            assignment(&r.key, &shown(&r.key, &r.value, mask_secrets), r.literal),
            names.get(r.source()).copied().unwrap_or("?")
        ));
    }

    if !conflicts.is_empty() {
        out.push_str(&format!(
            "\nOverride chain ({} {})\n",
            conflicts.len(),
            if conflicts.len() == 1 { "key" } else { "keys" }
        ));
        out.push_str("--------------\n");
        for r in conflicts {
            out.push_str(&chain_block(r, names, mask_secrets));
        }
    }

    if !warnings.is_empty() {
        out.push_str("\nWarnings\n--------\n");
        for w in warnings {
            out.push_str(&warning_line(w, names));
            out.push('\n');
        }
    }
    out
}

fn render_env(resolved: &[Resolved], names: &[&str], mask_secrets: bool) -> String {
    let mut out = format!("# Merged from: {}\n", names.join(" < "));
    for r in resolved {
        out.push_str(&assignment(
            &r.key,
            &shown(&r.key, &r.value, mask_secrets),
            r.literal,
        ));
        out.push('\n');
    }
    out
}

fn render_shell(resolved: &[Resolved], mask_secrets: bool) -> String {
    resolved
        .iter()
        .map(|r| {
            format!(
                "export {}={}",
                r.key,
                shell_quote(&shown(&r.key, &r.value, mask_secrets))
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_markdown(resolved: &[Resolved], names: &[&str], mask_secrets: bool) -> String {
    let cell = |s: &str| s.replace('|', "\\|").replace('\n', " ");
    let mut out = String::from("| Key | Value | Set by | Overrides |\n| --- | --- | --- | --- |\n");
    for r in resolved {
        let overrides = r
            .overridden()
            .iter()
            .map(|(l, _)| names.get(*l).copied().unwrap_or("?").to_string())
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "| `{}` | `{}` | `{}` | {} |\n",
            cell(&r.key),
            cell(&shown(&r.key, &r.value, mask_secrets)),
            cell(names.get(r.source()).copied().unwrap_or("?")),
            if overrides.is_empty() {
                "—".to_string()
            } else {
                format!("`{}`", cell(&overrides))
            }
        ));
    }
    out
}

fn render_conflicts(resolved: &[Resolved], names: &[&str], mask_secrets: bool) -> String {
    let conflicts = conflicting(resolved);
    if conflicts.is_empty() {
        return "No conflicts: every key is set by exactly one layer.\n".to_string();
    }
    let mut out = format!(
        "{} {} set by more than one layer.\n\n",
        conflicts.len(),
        if conflicts.len() == 1 {
            "key is"
        } else {
            "keys are"
        }
    );
    for r in conflicts {
        out.push_str(&chain_block(r, names, mask_secrets));
    }
    out
}

fn render_json(
    resolved: &[Resolved],
    names: &[&str],
    warnings: &[Warning],
    mask_secrets: bool,
) -> String {
    let variables: Vec<serde_json::Value> = resolved
        .iter()
        .map(|r| {
            let overrides: Vec<serde_json::Value> = r
                .overridden()
                .iter()
                .map(|(l, v)| {
                    serde_json::json!({
                        "layer": names.get(*l).copied().unwrap_or("?"),
                        "value": shown(&r.key, v, mask_secrets),
                    })
                })
                .collect();
            serde_json::json!({
                "key": r.key,
                "value": shown(&r.key, &r.value, mask_secrets),
                "source": names.get(r.source()).copied().unwrap_or("?"),
                "overrides": overrides,
            })
        })
        .collect();
    let doc = serde_json::json!({
        "layers": names,
        "keys": resolved.len(),
        "overridden": conflicting(resolved).len(),
        "variables": variables,
        "warnings": warnings
            .iter()
            .map(|w| warning_line(w, names).trim().to_string())
            .collect::<Vec<_>>(),
    });
    serde_json::to_string_pretty(&doc).unwrap_or_else(|_| "{}".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "APP_NAME=demo\nAPI_URL=https://dev.example.com\nDEBUG=true\n";
    const LOCAL: &str = "DEBUG=false\n";
    const PROD: &str = "API_URL=https://prod.example.com\nCDN_URL=https://cdn.example.com\n";

    fn three() -> Vec<&'static str> {
        vec![BASE, LOCAL, PROD, ""]
    }

    #[test]
    fn report_shows_resolved_values_and_their_source_file() {
        let out = merge(&three(), "", "report", true, false, "", false).unwrap();
        assert!(out.contains("Layers merged: 3 (.env < .env.local < .env.production)"));
        assert!(out.contains("Keys resolved: 4"));
        // Both API_URL and DEBUG are assigned by two layers.
        assert!(out.contains("Keys overridden: 2"), "{out}");
        assert!(out.contains("APP_NAME=demo  # set by .env"));
        assert!(out.contains("API_URL=https://prod.example.com  # set by .env.production"));
        assert!(out.contains("DEBUG=false  # set by .env.local"));
        // The shadowed value is kept, not discarded. Layer names in a chain are
        // padded to the widest name IN THAT CHAIN.
        assert!(
            out.contains("  .env            = https://dev.example.com\n"),
            "{out}"
        );
        assert!(
            out.contains("  .env.production = https://prod.example.com  (wins)\n"),
            "{out}"
        );
    }

    #[test]
    fn later_layers_win_and_new_keys_append_in_first_seen_order() {
        let out = merge(&three(), "", "env", false, false, "", false).unwrap();
        assert_eq!(
            out,
            "# Merged from: .env < .env.local < .env.production\n\
             APP_NAME=demo\n\
             API_URL=https://prod.example.com\n\
             DEBUG=false\n\
             CDN_URL=https://cdn.example.com\n"
        );
    }

    #[test]
    fn blank_layers_are_skipped_and_names_shift_with_the_slot() {
        // Only slots 1 and 3 carry content; slot 3 keeps ITS name.
        let out = merge(
            &["A=1\n", "", "A=2\n", ""],
            "",
            "env",
            false,
            false,
            "",
            false,
        )
        .unwrap();
        assert_eq!(out, "# Merged from: .env < .env.production\nA=2\n");
    }

    #[test]
    fn custom_layer_names_are_used_for_provenance() {
        let out = merge(
            &["A=1\n", "A=2\n", "", ""],
            "base.env, staging.env",
            "report",
            false,
            false,
            "",
            false,
        )
        .unwrap();
        assert!(out.contains("Layers merged: 2 (base.env < staging.env)"));
        assert!(out.contains("A=2  # set by staging.env"));
    }

    #[test]
    fn error_when_every_layer_is_blank() {
        let err = merge(&["", "   \n", "", ""], "", "report", true, false, "", false).unwrap_err();
        assert!(err.contains("no .env content"), "{err}");
    }

    #[test]
    fn error_on_unknown_output() {
        let err = merge(&["A=1", "", "", ""], "", "yaml", true, false, "", false).unwrap_err();
        assert!(err.contains("unknown output 'yaml'"), "{err}");
    }

    #[test]
    fn secrets_are_masked_by_default_in_every_output() {
        let layers = vec!["API_SECRET=supersecretvalue\nPLAIN=visible\n", "", "", ""];
        let report = merge(&layers, "", "report", true, false, "", false).unwrap();
        assert!(report.contains("API_SECRET=su****ue"));
        assert!(report.contains("PLAIN=visible"));
        let shell = merge(&layers, "", "shell", true, false, "", false).unwrap();
        assert!(shell.contains("export API_SECRET='su****ue'"));
        let unmasked = merge(&layers, "", "shell", false, false, "", false).unwrap();
        assert!(unmasked.contains("export API_SECRET='supersecretvalue'"));
    }

    #[test]
    fn shell_output_single_quotes_and_escapes() {
        let out = merge(
            &["MSG=it's here\n", "", "", ""],
            "",
            "shell",
            false,
            false,
            "",
            false,
        )
        .unwrap();
        assert_eq!(out, "export MSG='it'\\''s here'");
    }

    #[test]
    fn prefix_filter_keeps_only_matching_keys() {
        let layers = vec![
            "VITE_API=1\nDB_PASS=hunter2000\nVITE_MODE=dev\n",
            "",
            "",
            "",
        ];
        let out = merge(&layers, "", "shell", false, false, "VITE_", false).unwrap();
        assert_eq!(out, "export VITE_API='1'\nexport VITE_MODE='dev'");
    }

    #[test]
    fn sort_keys_orders_alphabetically() {
        let out = merge(
            &["Z=1\nA=2\n", "", "", ""],
            "",
            "shell",
            false,
            true,
            "",
            false,
        )
        .unwrap();
        assert_eq!(out, "export A='2'\nexport Z='1'");
    }

    #[test]
    fn expansion_resolves_across_layers_with_fallbacks_and_literals() {
        let layers = vec![
            "HOST=example.com\nAPI_URL=https://${HOST}/v1\nLITERAL='${HOST}'\n",
            "HOST=staging.example.com\nPORT_URL=$API_URL:${PORT:-8080}\n",
            "",
            "",
        ];
        let out = merge(&layers, "", "env", false, false, "", true).unwrap();
        // The later layer's HOST feeds the earlier layer's reference.
        assert!(
            out.contains("API_URL=https://staging.example.com/v1\n"),
            "{out}"
        );
        // A single-quoted value stays literal.
        assert!(out.contains("LITERAL='${HOST}'\n"), "{out}");
        // `${VAR:-fallback}` and nested `$VAR` both resolve.
        assert!(
            out.contains("PORT_URL=https://staging.example.com/v1:8080\n"),
            "{out}"
        );
    }

    #[test]
    fn expansion_is_off_by_default_and_escapes_are_honoured() {
        let layers = vec![
            "HOST=example.com\nRAW=https://${HOST}\nCASH=\\$5.00\n",
            "",
            "",
            "",
        ];
        let literal = merge(&layers, "", "env", false, false, "", false).unwrap();
        assert!(literal.contains("RAW=https://${HOST}\n"), "{literal}");
        let expanded = merge(&layers, "", "env", false, false, "", true).unwrap();
        assert!(expanded.contains("RAW=https://example.com\n"), "{expanded}");
        assert!(expanded.contains("CASH=$5.00\n"), "{expanded}");
    }

    #[test]
    fn unresolved_and_circular_references_warn_instead_of_hanging() {
        let layers = vec!["A=${B}\nB=${A}\nC=${NOPE}\n", "", "", ""];
        let out = merge(&layers, "", "report", false, false, "", true).unwrap();
        assert!(out.contains("circular reference"), "{out}");
        assert!(out.contains("$NOPE is not defined in any layer"), "{out}");
    }

    #[test]
    fn conflicts_output_lists_only_multi_layer_keys() {
        let out = merge(&three(), "", "conflicts", false, false, "", false).unwrap();
        assert_eq!(
            out,
            "2 keys are set by more than one layer.\n\n\
             API_URL\n\
             \x20 .env            = https://dev.example.com\n\
             \x20 .env.production = https://prod.example.com  (wins)\n\
             DEBUG\n\
             \x20 .env       = true\n\
             \x20 .env.local = false  (wins)\n"
        );
        let clean = merge(
            &["A=1\n", "B=2\n", "", ""],
            "",
            "conflicts",
            false,
            false,
            "",
            false,
        )
        .unwrap();
        assert_eq!(
            clean,
            "No conflicts: every key is set by exactly one layer.\n"
        );
    }

    #[test]
    fn json_output_carries_source_and_overrides() {
        let out = merge(&three(), "", "json", false, false, "", false).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["keys"], 4);
        assert_eq!(v["overridden"], 2);
        assert_eq!(v["layers"][2], ".env.production");
        let api = &v["variables"][1];
        assert_eq!(api["key"], "API_URL");
        assert_eq!(api["value"], "https://prod.example.com");
        assert_eq!(api["source"], ".env.production");
        assert_eq!(api["overrides"][0]["layer"], ".env");
        assert_eq!(api["overrides"][0]["value"], "https://dev.example.com");
    }

    #[test]
    fn markdown_output_has_a_set_by_column() {
        let out = merge(&three(), "", "markdown", false, false, "", false).unwrap();
        assert!(out.starts_with("| Key | Value | Set by | Overrides |\n"));
        assert!(
            out.contains("| `API_URL` | `https://prod.example.com` | `.env.production` | `.env` |")
        );
        assert!(out.contains("| `APP_NAME` | `demo` | `.env` | — |"));
    }

    #[test]
    fn dotenv_syntax_quirks_are_parsed() {
        let layers = vec![
            "# comment\nexport EXPORTED=yes\nQUOTED=\"a b\"\nSINGLE='c#d'\nINLINE=x # trailing\nFRAG=https://h/#anchor\n",
            "",
            "",
            "",
        ];
        let out = merge(&layers, "", "shell", false, false, "", false).unwrap();
        assert_eq!(
            out,
            "export EXPORTED='yes'\nexport QUOTED='a b'\nexport SINGLE='c#d'\nexport INLINE='x'\nexport FRAG='https://h/#anchor'"
        );
    }

    #[test]
    fn duplicate_key_inside_one_layer_warns_but_does_not_split_the_chain() {
        let out = merge(
            &["A=1\nA=2\n", "A=3\n", "", ""],
            "",
            "report",
            false,
            false,
            "",
            false,
        )
        .unwrap();
        assert!(
            out.contains("repeats within this layer (also on line 1)"),
            "{out}"
        );
        assert!(out.contains("A=3  # set by .env.local"));
        // Two layers touched A, so the chain has exactly two links.
        assert!(out.contains("  .env       = 2\n"), "{out}");
        assert!(out.contains("  .env.local = 3  (wins)\n"), "{out}");
    }

    #[test]
    fn malformed_lines_are_reported_with_layer_and_line() {
        let out = merge(
            &["GOOD=1\nBROKEN\nlower=2\n", "", "", ""],
            "",
            "report",
            false,
            false,
            "",
            false,
        )
        .unwrap();
        assert!(
            out.contains(".env line 2: 'BROKEN' has no value (missing '=')"),
            "{out}"
        );
        assert!(
            out.contains(".env line 3: key 'lower' is not UPPER_SNAKE_CASE"),
            "{out}"
        );
    }
}

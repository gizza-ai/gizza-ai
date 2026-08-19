//! gizza-ai/ini-json-converter core — pure, no wafer/wasm-bindgen deps.
//!
//! Converts INI / `.conf` text to JSON and back, and edits a single key in
//! place (`get` / `set` / `delete`) the way `crudini` does — the edit modes
//! rewrite only the target line, so comments, blank lines, key order and the
//! `key=value` vs `key = value` spacing of the original file all survive.
//!
//! The chat schema is single-sourced from the block's `descriptor()`; this
//! crate is the pure engine shared by the chat block, the CLI and the web page.

use serde::Serialize;
use serde_json::{Map, Number, Value};

/// Hard cap on input lines — a guard against a pathological paste. A real
/// config file is far under this.
pub const MAX_LINES: usize = 100_000;

/// Hard cap on JSON object nesting when rendering INI, so a deeply nested
/// document can't recurse without bound.
pub const MAX_DEPTH: usize = 64;

/// What the tool should do with the input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Pick `IniToJson` or `JsonToIni` from the input's first character.
    Auto,
    /// INI / `.conf` text → JSON.
    IniToJson,
    /// JSON → INI / `.conf` text.
    JsonToIni,
    /// Read one key (or dump one section) out of INI text.
    Get,
    /// Set one key in INI text, preserving everything else.
    Set,
    /// Remove one key (or a whole section) from INI text.
    Delete,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Mode, String> {
        let norm = s.trim().to_ascii_lowercase().replace('-', "_");
        Ok(match norm.as_str() {
            "" | "auto" => Mode::Auto,
            "ini_to_json" | "ini2json" => Mode::IniToJson,
            "json_to_ini" | "json2ini" => Mode::JsonToIni,
            "get" => Mode::Get,
            "set" => Mode::Set,
            "delete" | "del" => Mode::Delete,
            other => {
                return Err(format!(
                    "unknown mode '{other}' (use auto, ini_to_json, json_to_ini, get, set, or delete)"
                ))
            }
        })
    }
}

/// How a newly written `key <-> value` line separates the two.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delimiter {
    /// `key = value`
    EqualsSpaced,
    /// `key=value`
    Equals,
    /// `key: value`
    Colon,
}

impl Delimiter {
    pub fn parse(s: &str) -> Result<Delimiter, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "equals_spaced" | "equals-spaced" => Delimiter::EqualsSpaced,
            "equals" => Delimiter::Equals,
            "colon" => Delimiter::Colon,
            other => {
                return Err(format!(
                    "unknown delimiter '{other}' (use equals_spaced, equals, or colon)"
                ))
            }
        })
    }

    fn text(self) -> &'static str {
        match self {
            Delimiter::EqualsSpaced => " = ",
            Delimiter::Equals => "=",
            Delimiter::Colon => ": ",
        }
    }
}

/// One indentation step of pretty-printed JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Indent {
    TwoSpaces,
    FourSpaces,
    Tab,
}

impl Indent {
    pub fn parse(s: &str) -> Result<Indent, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "2" => Indent::TwoSpaces,
            "4" => Indent::FourSpaces,
            "tab" => Indent::Tab,
            other => return Err(format!("unknown indent '{other}' (use 2, 4, or tab)")),
        })
    }

    fn bytes(self) -> &'static [u8] {
        match self {
            Indent::TwoSpaces => b"  ",
            Indent::FourSpaces => b"    ",
            Indent::Tab => b"\t",
        }
    }
}

/// Convert or edit `input` per the requested `mode`.
///
/// - `mode`: `auto` | `ini_to_json` | `json_to_ini` | `get` | `set` | `delete`.
/// - `section` / `key`: the target of `get`/`set`/`delete`. A blank `section`
///   with a dotted `key` (`database.port`) is split at the first dot.
/// - `value`: the new value for `set`.
/// - `detect_types`: coerce unquoted booleans/numbers when producing JSON.
/// - `inline_comments`: treat a whitespace-preceded ` ;`/` #` as a trailing
///   comment rather than part of the value.
/// - `delimiter`: separator used for INI lines this tool writes.
/// - `pretty`: indent JSON output.
/// - `indent`: one indent step for pretty JSON (`2`, `4`, `tab`).
#[allow(clippy::too_many_arguments)]
pub fn convert(
    input: &str,
    mode: &str,
    section: &str,
    key: &str,
    value: &str,
    detect_types: bool,
    inline_comments: bool,
    delimiter: &str,
    pretty: bool,
    indent: &str,
) -> Result<String, String> {
    let mode = Mode::parse(mode)?;
    let delim = Delimiter::parse(delimiter)?;
    let indent = Indent::parse(indent)?;
    let (sec, key) = resolve_target(section, key);

    let mode = match mode {
        Mode::Auto => {
            if input.trim_start().starts_with('{') {
                Mode::JsonToIni
            } else {
                Mode::IniToJson
            }
        }
        other => other,
    };

    match mode {
        Mode::Auto => unreachable!("auto is resolved above"),
        Mode::IniToJson => ini_to_json(input, detect_types, inline_comments, pretty, indent),
        Mode::JsonToIni => json_to_ini(input, delim),
        Mode::Get => {
            reject_json_input(input, "get")?;
            get(
                input,
                sec.as_deref(),
                &key,
                detect_types,
                inline_comments,
                pretty,
                indent,
            )
        }
        Mode::Set => {
            reject_json_input(input, "set")?;
            set(input, sec.as_deref(), &key, value, delim, inline_comments)
        }
        Mode::Delete => {
            reject_json_input(input, "delete")?;
            delete(input, sec.as_deref(), &key)
        }
    }
}

/// The edit modes read and rewrite INI text; a JSON paste is almost certainly a
/// mistake, so say so instead of failing with a confusing parse error.
fn reject_json_input(input: &str, mode: &str) -> Result<(), String> {
    if input.trim_start().starts_with('{') {
        return Err(format!(
            "mode '{mode}' edits INI text, but the input looks like JSON — run mode=json_to_ini first, then edit the INI"
        ));
    }
    Ok(())
}

/// Split the addressed target. An explicit `section` always wins; otherwise a
/// dotted `key` (`database.port`) is split at the FIRST dot so chat-style
/// "set database.port to 5432" addresses work without a second field.
fn resolve_target(section: &str, key: &str) -> (Option<String>, String) {
    let section = section.trim();
    let key = key.trim();
    if section.is_empty() {
        if let Some((head, rest)) = key.split_once('.') {
            if !head.is_empty() && !rest.is_empty() {
                return (Some(head.to_string()), rest.to_string());
            }
        }
        return (None, key.to_string());
    }
    (Some(section.to_string()), key.to_string())
}

// ---------------------------------------------------------------------------
// INI → JSON
// ---------------------------------------------------------------------------

/// A parsed scope: the global (section-less) block, or a named section. Repeated
/// `[section]` headers merge into one scope; entries stay in encounter order.
struct Scope {
    /// `None` = the global (section-less) scope.
    name: Option<String>,
    entries: Vec<(String, Value)>,
}

fn parse_ini(text: &str, detect_types: bool, inline_comments: bool) -> Result<Vec<Scope>, String> {
    let mut scopes: Vec<Scope> = vec![Scope {
        name: None,
        entries: Vec::new(),
    }];
    let mut active = 0usize;

    for (i, raw_line) in text.lines().enumerate() {
        if i >= MAX_LINES {
            return Err(format!("input exceeds the {MAX_LINES}-line limit"));
        }
        let line_no = i + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            let name = section_header(line, line_no)?;
            active = match scopes.iter().position(|s| s.name.as_deref() == Some(name.as_str())) {
                Some(idx) => idx,
                None => {
                    scopes.push(Scope {
                        name: Some(name),
                        entries: Vec::new(),
                    });
                    scopes.len() - 1
                }
            };
            continue;
        }
        let Some(delim) = line.char_indices().find(|&(_, c)| c == '=' || c == ':').map(|(i, _)| i)
        else {
            return Err(format!(
                "line {line_no}: expected 'key = value', '[section]', or a comment: {line}"
            ));
        };
        let key = line[..delim].trim();
        if key.is_empty() {
            return Err(format!("line {line_no}: empty key before '{}'", &line[delim..delim + 1]));
        }
        let raw_value = line[delim + 1..].trim();
        let value = parse_value(raw_value, detect_types, inline_comments);
        scopes[active].entries.push((key.to_string(), value));
    }
    Ok(scopes)
}

/// Read the name out of a `[section]` header line (a trailing comment is allowed).
fn section_header(line: &str, line_no: usize) -> Result<String, String> {
    let Some(close) = line.find(']') else {
        return Err(format!(
            "line {line_no}: unterminated section header (expected '[name]'): {line}"
        ));
    };
    let name = line[1..close].trim();
    if name.is_empty() {
        return Err(format!("line {line_no}: empty section name '[]'"));
    }
    let tail = line[close + 1..].trim();
    if !tail.is_empty() && !tail.starts_with(';') && !tail.starts_with('#') {
        return Err(format!(
            "line {line_no}: unexpected text after the section header: {tail}"
        ));
    }
    Ok(name.to_string())
}

fn parse_value(raw: &str, detect_types: bool, inline_comments: bool) -> Value {
    let v = if inline_comments {
        split_inline_comment(raw).0.trim_end()
    } else {
        raw
    };
    if let Some(inner) = unquote(v) {
        return Value::String(inner.to_string());
    }
    if detect_types {
        match v.to_ascii_lowercase().as_str() {
            "true" | "yes" | "on" => return Value::Bool(true),
            "false" | "no" | "off" => return Value::Bool(false),
            _ => {}
        }
        if let Ok(n) = v.parse::<i64>() {
            return Value::Number(n.into());
        }
        if let Ok(f) = v.parse::<f64>() {
            if f.is_finite() {
                if let Some(n) = Number::from_f64(f) {
                    return Value::Number(n);
                }
            }
        }
    }
    Value::String(v.to_string())
}

/// Split a raw value into `(value, trailing comment)`. The comment marker must be
/// preceded by whitespace, and a leading quoted run is skipped so `"a ; b"` stays whole.
fn split_inline_comment(raw: &str) -> (&str, &str) {
    let scan_from = closing_quote_end(raw);
    let mut ws_start: Option<usize> = None;
    for (i, c) in raw.char_indices() {
        if i < scan_from {
            continue;
        }
        if c.is_whitespace() {
            if ws_start.is_none() {
                ws_start = Some(i);
            }
            continue;
        }
        if (c == ';' || c == '#') && ws_start.is_some() {
            let cut = ws_start.unwrap();
            return (&raw[..cut], &raw[cut..]);
        }
        ws_start = None;
    }
    (raw, "")
}

/// Byte index just past the closing quote of a leading quoted run, else 0.
fn closing_quote_end(s: &str) -> usize {
    let mut chars = s.char_indices();
    let Some((_, q)) = chars.next() else { return 0 };
    if q != '"' && q != '\'' {
        return 0;
    }
    for (i, c) in chars {
        if c == q {
            return i + c.len_utf8();
        }
    }
    0
}

fn unquote(s: &str) -> Option<&str> {
    let b = s.as_bytes();
    if b.len() >= 2 && (b[0] == b'"' || b[0] == b'\'') && b[b.len() - 1] == b[0] {
        Some(&s[1..s.len() - 1])
    } else {
        None
    }
}

fn ini_to_json(
    text: &str,
    detect_types: bool,
    inline_comments: bool,
    pretty: bool,
    indent: Indent,
) -> Result<String, String> {
    let scopes = parse_ini(text, detect_types, inline_comments)?;
    let mut root = Map::new();
    for scope in &scopes {
        match &scope.name {
            None => merge_entries(&mut root, &scope.entries),
            Some(name) => {
                if root.contains_key(name) {
                    return Err(format!(
                        "section '[{name}]' collides with a section-less key of the same name — rename one of them"
                    ));
                }
                let mut m = Map::new();
                merge_entries(&mut m, &scope.entries);
                root.insert(name.clone(), Value::Object(m));
            }
        }
    }
    render_json(&Value::Object(root), pretty, indent)
}

/// Insert entries, collapsing a key that repeats within one scope into a JSON
/// array so nothing is silently dropped (`.gitconfig`-style repeated keys).
fn merge_entries(map: &mut Map<String, Value>, entries: &[(String, Value)]) {
    for (k, v) in entries {
        match map.get_mut(k) {
            None => {
                map.insert(k.clone(), v.clone());
            }
            Some(existing) => {
                if let Value::Array(items) = existing {
                    items.push(v.clone());
                } else {
                    let prev = existing.take();
                    *existing = Value::Array(vec![prev, v.clone()]);
                }
            }
        }
    }
}

fn render_json(v: &Value, pretty: bool, indent: Indent) -> Result<String, String> {
    if !pretty {
        return serde_json::to_string(v).map_err(|e| format!("could not render JSON: {e}"));
    }
    // `to_string_pretty` hardcodes two spaces, so drive the formatter directly
    // to honour the requested indent step.
    let mut buf = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(indent.bytes());
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
    v.serialize(&mut ser)
        .map_err(|e| format!("could not render JSON: {e}"))?;
    String::from_utf8(buf).map_err(|e| format!("could not render JSON: {e}"))
}

// ---------------------------------------------------------------------------
// JSON → INI
// ---------------------------------------------------------------------------

fn json_to_ini(text: &str, delim: Delimiter) -> Result<String, String> {
    if text.trim().is_empty() {
        return Ok(String::new());
    }
    let parsed: Value = serde_json::from_str(text).map_err(|e| format!("invalid JSON: {e}"))?;
    let Value::Object(root) = parsed else {
        return Err(
            "json_to_ini needs a JSON object at the top level — an array or a bare scalar has no INI form"
                .into(),
        );
    };
    let mut out = String::new();
    for (k, v) in &root {
        if !v.is_object() {
            write_entry(&mut out, k, v, delim)?;
        }
    }
    for (k, v) in &root {
        if let Value::Object(m) = v {
            write_section(&mut out, k, m, delim, 1)?;
        }
    }
    Ok(out)
}

fn write_section(
    out: &mut String,
    path: &str,
    map: &Map<String, Value>,
    delim: Delimiter,
    depth: usize,
) -> Result<(), String> {
    if depth > MAX_DEPTH {
        return Err(format!("JSON nesting exceeds the {MAX_DEPTH}-level limit"));
    }
    if path.contains('\n') || path.contains(']') {
        return Err(format!("section name '{path}' can't be written as an INI header"));
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out.push('[');
    out.push_str(path);
    out.push_str("]\n");
    for (k, v) in map {
        if !v.is_object() {
            write_entry(out, k, v, delim)?;
        }
    }
    // Deeper objects become dotted sections: {"a":{"b":{…}}} → [a.b].
    for (k, v) in map {
        if let Value::Object(m) = v {
            write_section(out, &format!("{path}.{k}"), m, delim, depth + 1)?;
        }
    }
    Ok(())
}

fn write_entry(out: &mut String, key: &str, v: &Value, delim: Delimiter) -> Result<(), String> {
    if key.trim().is_empty() {
        return Err("JSON contains an empty key, which has no INI form".into());
    }
    if key.contains('\n') || key.contains('=') || key.contains(':') || key.contains('[') {
        return Err(format!(
            "key '{key}' contains a newline, '=', ':' or '[' and can't be written as an INI key"
        ));
    }
    match v {
        // A JSON array of scalars round-trips as a repeated key, which is how
        // list-valued INI settings are conventionally written.
        Value::Array(items) => {
            for item in items {
                if item.is_object() || item.is_array() {
                    return Err(format!(
                        "key '{key}': INI can't represent an array of objects or nested arrays — flatten it first"
                    ));
                }
                let text = scalar_text(key, item)?;
                push_line(out, key, &text, delim);
            }
        }
        _ => {
            let text = scalar_text(key, v)?;
            push_line(out, key, &text, delim);
        }
    }
    Ok(())
}

fn push_line(out: &mut String, key: &str, value: &str, delim: Delimiter) {
    out.push_str(key);
    out.push_str(delim.text());
    out.push_str(value);
    out.push('\n');
}

fn scalar_text(key: &str, v: &Value) -> Result<String, String> {
    Ok(match v {
        Value::String(s) => {
            if s.contains('\n') {
                return Err(format!(
                    "key '{key}': a value containing a newline has no INI form"
                ));
            }
            if needs_quoting(s) {
                format!("\"{s}\"")
            } else {
                s.clone()
            }
        }
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        _ => return Err(format!("key '{key}': unsupported value type")),
    })
}

/// Quote a value whose edges or comment markers would otherwise be eaten when
/// the file is read back.
fn needs_quoting(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let edged = s.starts_with(char::is_whitespace) || s.ends_with(char::is_whitespace);
    let quoted_look = s.starts_with('"') || s.starts_with('\'');
    let comment_look = !split_inline_comment(s).1.is_empty();
    edged || quoted_look || comment_look
}

// ---------------------------------------------------------------------------
// get
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn get(
    text: &str,
    section: Option<&str>,
    key: &str,
    detect_types: bool,
    inline_comments: bool,
    pretty: bool,
    indent: Indent,
) -> Result<String, String> {
    let scopes = parse_ini(text, detect_types, inline_comments)?;
    if section.is_none() && key.is_empty() {
        return Err("mode 'get' needs a key (e.g. key='database.port'), or a section to dump".into());
    }
    let Some(scope) = scopes.iter().find(|s| s.name.as_deref() == section) else {
        return Err(format!("section '[{}]' not found", section.unwrap_or("")));
    };
    if key.is_empty() {
        let mut m = Map::new();
        merge_entries(&mut m, &scope.entries);
        return render_json(&Value::Object(m), pretty, indent);
    }
    let hits: Vec<&Value> = scope.entries.iter().filter(|(k, _)| k == key).map(|(_, v)| v).collect();
    if hits.is_empty() {
        return Err(format!("key '{key}' not found in {}", scope_label(section)));
    }
    // One line per occurrence: a repeated key keeps every value.
    Ok(hits.iter().map(|v| value_text(v)).collect::<Vec<_>>().join("\n"))
}

fn value_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn scope_label(section: Option<&str>) -> String {
    match section {
        Some(s) => format!("section '[{s}]'"),
        None => "the section-less keys at the top of the file".to_string(),
    }
}

// ---------------------------------------------------------------------------
// set / delete — line-preserving edits
// ---------------------------------------------------------------------------

/// The raw file, split into lines so an edit can touch exactly one of them.
struct Doc {
    lines: Vec<String>,
    crlf: bool,
}

impl Doc {
    fn parse(text: &str) -> Result<Doc, String> {
        let crlf = text.contains("\r\n");
        let mut lines: Vec<String> = if text.is_empty() {
            Vec::new()
        } else {
            text.split('\n').map(|l| l.trim_end_matches('\r').to_string()).collect()
        };
        if text.ends_with('\n') {
            lines.pop();
        }
        if lines.len() > MAX_LINES {
            return Err(format!("input exceeds the {MAX_LINES}-line limit"));
        }
        Ok(Doc { lines, crlf })
    }

    fn render(&self) -> String {
        let nl = if self.crlf { "\r\n" } else { "\n" };
        if self.lines.is_empty() {
            return String::new();
        }
        let mut s = self.lines.join(nl);
        s.push_str(nl);
        s
    }
}

/// A section's extent in the raw line list: its header (if any) plus its body.
struct Block {
    name: Option<String>,
    header: Option<usize>,
    start: usize,
    end: usize,
}

fn blocks(lines: &[String]) -> Result<Vec<Block>, String> {
    let mut out = vec![Block {
        name: None,
        header: None,
        start: 0,
        end: lines.len(),
    }];
    for (i, raw) in lines.iter().enumerate() {
        let line = raw.trim();
        if !line.starts_with('[') {
            continue;
        }
        let name = section_header(line, i + 1)?;
        if let Some(prev) = out.last_mut() {
            prev.end = i;
        }
        out.push(Block {
            name: Some(name),
            header: Some(i),
            start: i + 1,
            end: lines.len(),
        });
    }
    Ok(out)
}

/// The `=`/`:` byte offset of a `key = value` line, or `None` for blanks,
/// comments and section headers.
fn entry_delim(line: &str) -> Option<usize> {
    let t = line.trim_start();
    if t.is_empty() || t.starts_with(';') || t.starts_with('#') || t.starts_with('[') {
        return None;
    }
    line.char_indices().find(|&(_, c)| c == '=' || c == ':').map(|(i, _)| i)
}

fn entry_key(line: &str) -> Option<(&str, usize)> {
    let d = entry_delim(line)?;
    let k = line[..d].trim();
    if k.is_empty() {
        None
    } else {
        Some((k, d))
    }
}

fn set(
    text: &str,
    section: Option<&str>,
    key: &str,
    value: &str,
    delim: Delimiter,
    inline_comments: bool,
) -> Result<String, String> {
    if key.is_empty() {
        return Err("mode 'set' needs a key (e.g. key='database.port')".into());
    }
    if key.contains('\n') || value.contains('\n') {
        return Err("mode 'set' can't write a key or value containing a newline".into());
    }
    let mut doc = Doc::parse(text)?;
    let bs = blocks(&doc.lines)?;
    let matching: Vec<usize> = bs
        .iter()
        .enumerate()
        .filter(|(_, b)| b.name.as_deref() == section)
        .map(|(i, _)| i)
        .collect();

    if matching.is_empty() {
        // Unknown section — append it, the way crudini creates missing sections.
        let name = section.unwrap_or_default();
        if !doc.lines.is_empty() && !doc.lines.last().map(|l| l.trim().is_empty()).unwrap_or(true) {
            doc.lines.push(String::new());
        }
        doc.lines.push(format!("[{name}]"));
        doc.lines.push(format!("{key}{}{value}", delim.text()));
        return Ok(doc.render());
    }

    let hits: Vec<usize> = matching
        .iter()
        .flat_map(|&bi| (bs[bi].start..bs[bi].end).collect::<Vec<_>>())
        .filter(|&i| entry_key(&doc.lines[i]).map(|(k, _)| k == key).unwrap_or(false))
        .collect();

    if let Some(&first) = hits.first() {
        let (_, d) = entry_key(&doc.lines[first]).expect("hit lines are entries");
        doc.lines[first] = rewrite_value(&doc.lines[first], d, value, inline_comments);
        // Collapse any repeats of the same key so `set` leaves exactly one line.
        for &i in hits[1..].iter().rev() {
            doc.lines.remove(i);
        }
        return Ok(doc.render());
    }

    // New key in an existing section: insert after the last non-blank body line
    // so it lands with its neighbours rather than past the trailing blank line.
    let block = &bs[matching[0]];
    let indent = (block.start..block.end)
        .find_map(|i| entry_key(&doc.lines[i]).map(|_| leading_ws(&doc.lines[i])))
        .unwrap_or("")
        .to_string();
    let at = (block.start..block.end)
        .rev()
        .find(|&i| !doc.lines[i].trim().is_empty())
        .map(|i| i + 1)
        .unwrap_or(block.start);
    doc.lines.insert(at, format!("{indent}{key}{}{value}", delim.text()));
    Ok(doc.render())
}

fn leading_ws(line: &str) -> &str {
    &line[..line.len() - line.trim_start().len()]
}

/// Swap in a new value, keeping the line's indentation, its `=`/`:` spacing and
/// (when inline comments are enabled) its trailing comment.
fn rewrite_value(line: &str, delim_idx: usize, new_value: &str, inline_comments: bool) -> String {
    let head = &line[..=delim_idx];
    let rest = &line[delim_idx + 1..];
    let ws_len = rest.len() - rest.trim_start().len();
    let (ws, after) = rest.split_at(ws_len);
    let comment = if inline_comments {
        split_inline_comment(after).1
    } else {
        ""
    };
    format!("{head}{ws}{new_value}{comment}")
}

fn delete(text: &str, section: Option<&str>, key: &str) -> Result<String, String> {
    let mut doc = Doc::parse(text)?;
    let bs = blocks(&doc.lines)?;
    let matching: Vec<usize> = bs
        .iter()
        .enumerate()
        .filter(|(_, b)| b.name.as_deref() == section)
        .map(|(i, _)| i)
        .collect();

    if key.is_empty() {
        let Some(name) = section else {
            return Err(
                "mode 'delete' needs a key, or a section name to remove a whole section".into(),
            );
        };
        if matching.is_empty() {
            return Err(format!("section '[{name}]' not found"));
        }
        // Remove header + body, last block first so earlier indices stay valid.
        for &bi in matching.iter().rev() {
            let from = bs[bi].header.unwrap_or(bs[bi].start);
            doc.lines.drain(from..bs[bi].end);
        }
        return Ok(doc.render());
    }

    if matching.is_empty() {
        return Err(format!("section '[{}]' not found", section.unwrap_or("")));
    }
    let hits: Vec<usize> = matching
        .iter()
        .flat_map(|&bi| (bs[bi].start..bs[bi].end).collect::<Vec<_>>())
        .filter(|&i| entry_key(&doc.lines[i]).map(|(k, _)| k == key).unwrap_or(false))
        .collect();
    if hits.is_empty() {
        return Err(format!("key '{key}' not found in {}", scope_label(section)));
    }
    for &i in hits.iter().rev() {
        doc.lines.remove(i);
    }
    Ok(doc.render())
}

#[cfg(test)]
mod tests {
    use super::*;

    const CFG: &str = "# app config\napp = demo\n\n[server]\nhost = localhost\nport = 8080  ; listen port\n\n[db]\nname = main\n";

    fn run(input: &str, mode: &str, section: &str, key: &str, value: &str) -> Result<String, String> {
        convert(input, mode, section, key, value, false, true, "equals_spaced", true, "2")
    }

    #[test]
    fn ini_to_json_nests_sections_and_keeps_globals_at_the_root() {
        let out = run(CFG, "ini_to_json", "", "", "").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["app"], "demo");
        assert_eq!(v["server"]["host"], "localhost");
        assert_eq!(v["server"]["port"], "8080", "inline comment stripped by default");
        assert_eq!(v["db"]["name"], "main");
    }

    #[test]
    fn auto_mode_picks_the_direction_from_the_input() {
        let json = run(CFG, "auto", "", "", "").unwrap();
        assert!(json.trim_start().starts_with('{'));
        let back = run(&json, "auto", "", "", "").unwrap();
        assert!(back.contains("[server]"));
        assert!(back.contains("host = localhost"));
    }

    #[test]
    fn detect_types_coerces_booleans_and_numbers_only_when_asked() {
        let ini = "[s]\nport = 8080\ndebug = true\nratio = 0.75\nname = \"8080\"\n";
        let strings: Value =
            serde_json::from_str(&run(ini, "ini_to_json", "", "", "").unwrap()).unwrap();
        assert_eq!(strings["s"]["port"], "8080");
        let typed: Value = serde_json::from_str(
            &convert(ini, "ini_to_json", "", "", "", true, true, "equals_spaced", true, "2").unwrap(),
        )
        .unwrap();
        assert_eq!(typed["s"]["port"], 8080);
        assert_eq!(typed["s"]["debug"], true);
        assert_eq!(typed["s"]["ratio"], 0.75);
        assert_eq!(typed["s"]["name"], "8080", "quoted values stay strings");
    }

    #[test]
    fn repeated_keys_collect_into_an_array_and_round_trip() {
        let ini = "[hosts]\nserver = a\nserver = b\n";
        let json = run(ini, "ini_to_json", "", "", "").unwrap();
        let v: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["hosts"]["server"], serde_json::json!(["a", "b"]));
        assert_eq!(run(&json, "json_to_ini", "", "", "").unwrap(), ini);
    }

    #[test]
    fn json_to_ini_writes_globals_first_then_sections() {
        let json = r#"{"app":"demo","server":{"host":"localhost","port":8080},"db":{"name":"main"}}"#;
        let ini = run(json, "json_to_ini", "", "", "").unwrap();
        assert_eq!(
            ini,
            "app = demo\n\n[server]\nhost = localhost\nport = 8080\n\n[db]\nname = main\n"
        );
    }

    #[test]
    fn json_to_ini_flattens_deep_objects_into_dotted_sections() {
        let json = r#"{"a":{"x":"1","b":{"y":"2"}}}"#;
        let ini = run(json, "json_to_ini", "", "", "").unwrap();
        assert_eq!(ini, "[a]\nx = 1\n\n[a.b]\ny = 2\n");
    }

    #[test]
    fn json_to_ini_quotes_values_that_would_not_survive_a_read_back() {
        let json = r#"{"s":{"pad":"  x  ","note":"8080 ; hint"}}"#;
        let ini = run(json, "json_to_ini", "", "", "").unwrap();
        assert!(ini.contains("pad = \"  x  \""), "{ini}");
        assert!(ini.contains("note = \"8080 ; hint\""), "{ini}");
        let back: Value =
            serde_json::from_str(&run(&ini, "ini_to_json", "", "", "").unwrap()).unwrap();
        assert_eq!(back["s"]["pad"], "  x  ");
        assert_eq!(back["s"]["note"], "8080 ; hint");
    }

    #[test]
    fn json_to_ini_honours_the_delimiter_choice() {
        let json = r#"{"s":{"k":"v"}}"#;
        let eq = convert(json, "json_to_ini", "", "", "", false, true, "equals", true, "2").unwrap();
        assert_eq!(eq, "[s]\nk=v\n");
        let colon = convert(json, "json_to_ini", "", "", "", false, true, "colon", true, "2").unwrap();
        assert_eq!(colon, "[s]\nk: v\n");
    }

    #[test]
    fn indent_controls_the_pretty_json_step_and_is_ignored_when_compact() {
        let ini = "[s]\nk = v\n";
        let two = convert(ini, "ini_to_json", "", "", "", false, true, "equals_spaced", true, "2")
            .unwrap();
        assert!(two.contains("\n  \"s\": {\n    \"k\": \"v\"\n  }"), "{two}");
        let four = convert(ini, "ini_to_json", "", "", "", false, true, "equals_spaced", true, "4")
            .unwrap();
        assert!(four.contains("\n    \"s\": {\n        \"k\": \"v\"\n    }"), "{four}");
        let tab = convert(ini, "ini_to_json", "", "", "", false, true, "equals_spaced", true, "tab")
            .unwrap();
        assert!(tab.contains("\n\t\"s\": {\n\t\t\"k\": \"v\"\n\t}"), "{tab}");
        let compact =
            convert(ini, "ini_to_json", "", "", "", false, true, "equals_spaced", false, "4")
                .unwrap();
        assert_eq!(compact, r#"{"s":{"k":"v"}}"#);
        assert!(
            convert(ini, "ini_to_json", "", "", "", false, true, "equals_spaced", true, "8")
                .unwrap_err()
                .contains("unknown indent")
        );
    }

    #[test]
    fn get_reads_a_dotted_key_and_a_whole_section() {
        assert_eq!(run(CFG, "get", "", "server.host", "").unwrap(), "localhost");
        assert_eq!(run(CFG, "get", "server", "port", "").unwrap(), "8080");
        assert_eq!(run(CFG, "get", "", "app", "").unwrap(), "demo");
        let section: Value =
            serde_json::from_str(&run(CFG, "get", "db", "", "").unwrap()).unwrap();
        assert_eq!(section["name"], "main");
    }

    #[test]
    fn set_rewrites_one_line_and_leaves_comments_and_layout_alone() {
        let out = run(CFG, "set", "", "server.port", "9090").unwrap();
        assert_eq!(
            out,
            "# app config\napp = demo\n\n[server]\nhost = localhost\nport = 9090  ; listen port\n\n[db]\nname = main\n"
        );
    }

    #[test]
    fn set_preserves_the_existing_spacing_style() {
        let out = run("[s]\nk=v\n", "set", "s", "k", "w").unwrap();
        assert_eq!(out, "[s]\nk=w\n");
    }

    #[test]
    fn set_adds_a_missing_key_inside_the_section_and_a_missing_section_at_the_end() {
        let added = run(CFG, "set", "db", "user", "admin").unwrap();
        assert_eq!(
            added,
            "# app config\napp = demo\n\n[server]\nhost = localhost\nport = 8080  ; listen port\n\n[db]\nname = main\nuser = admin\n"
        );
        let new_section = run(CFG, "set", "cache", "ttl", "60").unwrap();
        assert!(new_section.ends_with("[cache]\nttl = 60\n"), "{new_section}");
    }

    #[test]
    fn set_collapses_a_repeated_key_to_a_single_line() {
        let out = run("[h]\ns = a\ns = b\nx = 1\n", "set", "h", "s", "c").unwrap();
        assert_eq!(out, "[h]\ns = c\nx = 1\n");
    }

    #[test]
    fn delete_removes_a_key_or_a_whole_section() {
        let no_key = run(CFG, "delete", "", "server.host", "").unwrap();
        assert!(!no_key.contains("host"));
        assert!(no_key.contains("port = 8080"));
        let no_section = run(CFG, "delete", "server", "", "").unwrap();
        assert!(!no_section.contains("[server]"));
        assert!(no_section.contains("[db]"));
        assert!(no_section.contains("# app config"));
    }

    #[test]
    fn crlf_input_keeps_crlf_line_endings() {
        let out = run("[s]\r\nk = v\r\n", "set", "s", "k", "w").unwrap();
        assert_eq!(out, "[s]\r\nk = w\r\n");
    }

    #[test]
    fn errors_are_specific() {
        assert!(run("[s]\nk = v\n", "get", "s", "missing", "")
            .unwrap_err()
            .contains("not found"));
        assert!(run("[s]\nk = v\n", "get", "nope", "k", "")
            .unwrap_err()
            .contains("[nope]"));
        assert!(run("{\"a\":1}", "set", "a", "b", "c")
            .unwrap_err()
            .contains("looks like JSON"));
        assert!(run("[s]\nk = v\n", "sideways", "", "", "")
            .unwrap_err()
            .contains("unknown mode"));
        assert!(run("[unterminated\n", "ini_to_json", "", "", "")
            .unwrap_err()
            .contains("unterminated"));
        assert!(run("[1,2]", "json_to_ini", "", "", "")
            .unwrap_err()
            .contains("top level"));
        assert!(run(r#"{"a":[{"b":1}]}"#, "json_to_ini", "", "", "")
            .unwrap_err()
            .contains("array of objects"));
        assert!(run("[s]\nk = v\n", "set", "s", "", "x")
            .unwrap_err()
            .contains("needs a key"));
        assert!(run("[s]\nk = v\n", "delete", "", "", "")
            .unwrap_err()
            .contains("needs a key"));
    }
}

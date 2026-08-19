//! yaml-path-query core — read or edit a single value in a YAML document by a
//! dotted / bracketed path. Pure compute, shared by the chat skill block, the
//! CLI, and the web page. No wafer/wasm-bindgen deps and no host/WASI calls, so
//! it runs in the gizza wafer runtime (chat SW), the CLI, and the browser page
//! alike.
//!
//! Path notation (lodash / `dot-object` style, NOT RFC 9535 JSONPath):
//!   - dot segments:      `server.host`
//!   - array indices:     `items[0].name` or the equivalent `items.0.name`
//!   - quoted keys:       `["my.key"].id` / `['key with spaces']` (a key that
//!                        itself contains `.`, `[`, or a space)
//!   - an optional leading `$` root marker is accepted and ignored (`$.a.b`).
//!
//! Modes:
//!   - `query`  — return the value at the path.
//!   - `set`    — write a value at the path; returns the whole document.
//!   - `delete` — remove the key / list element; returns the whole document.
//!
//! ## Comment & formatting preservation
//!
//! `set` and `delete` are attempted as a **surgical text edit** on the original
//! source: the target's source span is located through the parser's Markers and
//! only those bytes are rewritten, so comments, blank lines, key order, quoting
//! style and indentation everywhere else survive untouched.
//!
//! Every surgical edit is then **verified** by re-parsing the result and
//! comparing it to the independently computed expected tree. If the shapes don't
//! match — or the edit isn't one of the supported surgical shapes (creating
//! missing intermediate levels, replacing a whole block, block scalars, anchors)
//! — the tool falls back to re-emitting the document from the parsed tree, which
//! is always correct but drops comments and normalizes formatting. Data is never
//! silently corrupted; at worst the output is a normalized document.

use yaml_rust2::parser::{Event, Parser};
use yaml_rust2::scanner::{Marker, TScalarStyle};
use yaml_rust2::yaml::Hash;
use yaml_rust2::{Yaml, YamlEmitter, YamlLoader};

/// Cap on how far a `set` may grow a list by index, so a typo like
/// `a[999999999]` can't allocate a giant list. Setting an index above this
/// errors instead.
pub const MAX_LIST_GROW: usize = 100_000;

// ---------------------------------------------------------------------------
// Path parsing
// ---------------------------------------------------------------------------

/// One parsed path segment.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Segment {
    key: String,
    /// True when the segment came from a *quoted* bracket (`["0"]`), which forces
    /// it to be treated as a mapping key even if it looks numeric.
    quoted: bool,
}

/// Parse a dotted / bracketed path into segments.
fn parse_path(path: &str) -> Result<Vec<Segment>, String> {
    let s = path.trim();
    // Optional leading `$` root marker (`$`, `$.a`, `$['a']`).
    let s = s.strip_prefix('$').unwrap_or(s);
    let chars: Vec<char> = s.chars().collect();
    let n = chars.len();
    let mut segs: Vec<Segment> = Vec::new();
    let mut i = 0;
    while i < n {
        match chars[i] {
            // Separator (also swallows a leading '.').
            '.' => {
                i += 1;
            }
            '[' => {
                i += 1;
                if i < n && (chars[i] == '"' || chars[i] == '\'') {
                    let quote = chars[i];
                    i += 1;
                    let mut buf = String::new();
                    let mut closed = false;
                    while i < n {
                        if chars[i] == '\\' && i + 1 < n {
                            // Allow escaping the quote (and backslash) inside a key.
                            buf.push(chars[i + 1]);
                            i += 2;
                            continue;
                        }
                        if chars[i] == quote {
                            closed = true;
                            i += 1;
                            break;
                        }
                        buf.push(chars[i]);
                        i += 1;
                    }
                    if !closed {
                        return Err(format!("unterminated quoted key in path near '{buf}'"));
                    }
                    if i >= n || chars[i] != ']' {
                        return Err("expected ']' after a quoted key in path".into());
                    }
                    i += 1;
                    segs.push(Segment {
                        key: buf,
                        quoted: true,
                    });
                } else {
                    let mut buf = String::new();
                    while i < n && chars[i] != ']' {
                        buf.push(chars[i]);
                        i += 1;
                    }
                    if i >= n {
                        return Err("unterminated '[' in path (missing ']')".into());
                    }
                    i += 1;
                    let key = buf.trim().to_string();
                    if key.is_empty() {
                        return Err("empty '[]' in path".into());
                    }
                    segs.push(Segment { key, quoted: false });
                }
            }
            ']' => return Err("unexpected ']' in path".into()),
            _ => {
                let mut buf = String::new();
                while i < n && chars[i] != '.' && chars[i] != '[' {
                    buf.push(chars[i]);
                    i += 1;
                }
                if !buf.is_empty() {
                    segs.push(Segment {
                        key: buf,
                        quoted: false,
                    });
                }
            }
        }
    }
    Ok(segs)
}

/// Parse a segment as a usize list index (only when it wasn't a quoted key).
fn as_index(seg: &Segment) -> Option<usize> {
    if seg.quoted {
        return None;
    }
    seg.key.parse::<usize>().ok()
}

// ---------------------------------------------------------------------------
// Yaml tree helpers
// ---------------------------------------------------------------------------

fn type_name(v: &Yaml) -> &'static str {
    match v {
        Yaml::Null => "null",
        Yaml::Boolean(_) => "boolean",
        Yaml::Integer(_) | Yaml::Real(_) => "number",
        Yaml::String(_) => "string",
        Yaml::Array(_) => "list",
        Yaml::Hash(_) => "mapping",
        Yaml::Alias(_) => "alias",
        Yaml::BadValue => "unresolved value",
    }
}

/// Canonical text form of a scalar used as a mapping key, so a path segment
/// (always text) can address `1: x` or `true: x` as well as `name: x`.
fn key_text(k: &Yaml) -> Option<String> {
    match k {
        Yaml::String(s) => Some(s.clone()),
        Yaml::Integer(i) => Some(i.to_string()),
        Yaml::Real(s) => Some(s.clone()),
        Yaml::Boolean(b) => Some(b.to_string()),
        Yaml::Null => Some("null".into()),
        _ => None,
    }
}

/// Find a mapping key whose canonical text equals `want`.
fn find_key(map: &Hash, want: &str) -> Option<Yaml> {
    map.keys()
        .find(|k| key_text(k).as_deref() == Some(want))
        .cloned()
}

/// Traverse to the value at `segs`, returning a reference.
fn resolve<'a>(root: &'a Yaml, segs: &[Segment]) -> Result<&'a Yaml, String> {
    let mut cur = root;
    for seg in segs {
        match cur {
            Yaml::Hash(map) => {
                let k = find_key(map, &seg.key)
                    .ok_or_else(|| format!("no value at path: mapping has no key '{}'", seg.key))?;
                cur = map.get(&k).expect("key just found");
            }
            Yaml::Array(arr) => {
                let idx = as_index(seg)
                    .ok_or_else(|| format!("expected a list index, got '{}'", seg.key))?;
                cur = arr.get(idx).ok_or_else(|| {
                    format!("list index {idx} out of bounds (length {})", arr.len())
                })?;
            }
            other => {
                return Err(format!(
                    "cannot descend into a {} at '{}'",
                    type_name(other),
                    seg.key
                ));
            }
        }
    }
    Ok(cur)
}

/// Recursively set `newval` at `segs` (non-empty), creating intermediates.
fn set_rec(cur: &mut Yaml, segs: &[Segment], newval: Yaml) -> Result<(), String> {
    let seg = &segs[0];
    let rest = &segs[1..];
    let numeric = as_index(seg).is_some();

    // An empty slot (null) becomes a list or a mapping depending on this segment.
    if matches!(cur, Yaml::Null) {
        *cur = if numeric {
            Yaml::Array(Vec::new())
        } else {
            Yaml::Hash(Hash::new())
        };
    }

    match cur {
        Yaml::Hash(map) => {
            let k = find_key(map, &seg.key).unwrap_or_else(|| Yaml::String(seg.key.clone()));
            // `LinkedHashMap::insert` moves an existing entry to the back, which
            // would silently reorder the document — overwrite in place instead
            // and only insert when the key is genuinely new.
            if !map.contains_key(&k) {
                map.insert(k.clone(), Yaml::Null);
            }
            let child = map.get_mut(&k).expect("key present");
            if rest.is_empty() {
                *child = newval;
            } else {
                set_rec(child, rest, newval)?;
            }
        }
        Yaml::Array(arr) => {
            let idx =
                as_index(seg).ok_or_else(|| format!("expected a list index, got '{}'", seg.key))?;
            if idx >= arr.len() {
                if idx >= MAX_LIST_GROW {
                    return Err(format!(
                        "list index {idx} exceeds the maximum of {}",
                        MAX_LIST_GROW - 1
                    ));
                }
                arr.resize(idx + 1, Yaml::Null);
            }
            if rest.is_empty() {
                arr[idx] = newval;
            } else {
                set_rec(&mut arr[idx], rest, newval)?;
            }
        }
        other => {
            return Err(format!(
                "cannot set into a {} at '{}' — the existing value is not a mapping or a list",
                type_name(other),
                seg.key
            ));
        }
    }
    Ok(())
}

/// Recursively delete the value at `segs` (non-empty).
fn delete_rec(cur: &mut Yaml, segs: &[Segment]) -> Result<(), String> {
    let seg = &segs[0];
    let rest = &segs[1..];
    match cur {
        Yaml::Hash(map) => {
            let k = find_key(map, &seg.key);
            if rest.is_empty() {
                let k = k.ok_or_else(|| {
                    format!("nothing to delete: mapping has no key '{}'", seg.key)
                })?;
                map.remove(&k);
            } else {
                let k =
                    k.ok_or_else(|| format!("no value at path: mapping has no key '{}'", seg.key))?;
                let child = map.get_mut(&k).expect("key just found");
                delete_rec(child, rest)?;
            }
        }
        Yaml::Array(arr) => {
            let idx =
                as_index(seg).ok_or_else(|| format!("expected a list index, got '{}'", seg.key))?;
            if idx >= arr.len() {
                return Err(format!(
                    "list index {idx} out of bounds (length {})",
                    arr.len()
                ));
            }
            if rest.is_empty() {
                arr.remove(idx);
            } else {
                delete_rec(&mut arr[idx], rest)?;
            }
        }
        other => {
            return Err(format!(
                "cannot delete from a {} at '{}'",
                type_name(other),
                seg.key
            ));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Value parsing + emitting
// ---------------------------------------------------------------------------

/// Parse the `value` field as YAML (which is a superset of JSON), falling back
/// to a plain string when it isn't parseable as a single YAML document.
fn parse_value(value: &str) -> Yaml {
    if value.is_empty() {
        return Yaml::String(String::new());
    }
    match YamlLoader::load_from_str(value) {
        Ok(docs) if docs.len() == 1 => docs.into_iter().next().expect("len checked"),
        _ => Yaml::String(value.to_string()),
    }
}

/// Re-emit a whole document (or a sub-tree) as block YAML. Always correct, but
/// normalizes formatting and drops comments.
fn emit_yaml(v: &Yaml) -> Result<String, String> {
    if matches!(v, Yaml::Null) {
        return Ok("null".into());
    }
    let mut out = String::new();
    {
        let mut emitter = YamlEmitter::new(&mut out);
        emitter
            .dump(v)
            .map_err(|e| format!("YAML emit error: {e:?}"))?;
    }
    // YamlEmitter always writes the `---` document marker; drop it so the output
    // looks like the input did.
    let out = out.strip_prefix("---\n").unwrap_or(&out).to_string();
    Ok(out.trim_start_matches(' ').trim_end().to_string() + "\n")
}

/// Would `s` be re-read as a non-string scalar (number / bool / null) if written
/// plain? Then it has to be quoted to survive a round trip as a string.
fn plain_would_change_type(s: &str) -> bool {
    !matches!(Yaml::from_str(s), Yaml::String(_))
}

/// Render a string as an inline YAML scalar, quoting only when necessary.
/// Returns `None` for strings that cannot live on one line (newlines, control
/// characters) — those force the re-emit fallback.
fn inline_string(s: &str) -> Option<String> {
    if s.chars().any(|c| c.is_control()) {
        return None;
    }
    let first = s.chars().next();
    let needs_quotes = s.is_empty()
        || s.starts_with(' ')
        || s.ends_with(' ')
        || s.contains(": ")
        || s.contains(" #")
        || s.ends_with(':')
        || s.contains(['[', ']', '{', '}', ',', '"', '\'', '\\'])
        || matches!(
            first,
            Some('-' | '?' | ':' | '#' | '&' | '*' | '!' | '|' | '>' | '%' | '@' | '`')
        )
        || plain_would_change_type(s);
    if !needs_quotes {
        return Some(s.to_string());
    }
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            _ => out.push(c),
        }
    }
    out.push('"');
    Some(out)
}

/// Render a value as a single-line YAML node (flow style for collections), so it
/// can be spliced into the source without disturbing the surrounding lines.
fn inline_value(v: &Yaml) -> Option<String> {
    match v {
        Yaml::Null => Some("null".into()),
        Yaml::Boolean(b) => Some(if *b { "true".into() } else { "false".into() }),
        Yaml::Integer(i) => Some(i.to_string()),
        Yaml::Real(s) => Some(s.clone()),
        Yaml::String(s) => inline_string(s),
        Yaml::Array(arr) => {
            let mut parts = Vec::with_capacity(arr.len());
            for item in arr {
                parts.push(inline_value(item)?);
            }
            Some(format!("[{}]", parts.join(", ")))
        }
        Yaml::Hash(map) => {
            let mut parts = Vec::with_capacity(map.len());
            for (k, val) in map {
                parts.push(format!("{}: {}", inline_value(k)?, inline_value(val)?));
            }
            Some(format!("{{{}}}", parts.join(", ")))
        }
        Yaml::Alias(_) | Yaml::BadValue => None,
    }
}

/// Convert a YAML tree to JSON.
fn to_json(v: &Yaml) -> Result<serde_json::Value, String> {
    Ok(match v {
        Yaml::Null => serde_json::Value::Null,
        Yaml::Boolean(b) => serde_json::Value::Bool(*b),
        Yaml::Integer(i) => serde_json::Value::from(*i),
        Yaml::Real(s) => match s.parse::<f64>() {
            Ok(f) => serde_json::Number::from_f64(f)
                .map(serde_json::Value::Number)
                .unwrap_or_else(|| serde_json::Value::String(s.clone())),
            Err(_) => serde_json::Value::String(s.clone()),
        },
        Yaml::String(s) => serde_json::Value::String(s.clone()),
        Yaml::Array(arr) => {
            let mut out = Vec::with_capacity(arr.len());
            for item in arr {
                out.push(to_json(item)?);
            }
            serde_json::Value::Array(out)
        }
        Yaml::Hash(map) => {
            let mut out = serde_json::Map::new();
            for (k, val) in map {
                let ks = key_text(k).ok_or_else(|| {
                    "JSON output needs plain scalar keys, but this mapping has a complex key"
                        .to_string()
                })?;
                out.insert(ks, to_json(val)?);
            }
            serde_json::Value::Object(out)
        }
        Yaml::Alias(_) | Yaml::BadValue => {
            return Err("the document contains an unresolved alias".into())
        }
    })
}

// ---------------------------------------------------------------------------
// Source-span location (what makes comment-preserving edits possible)
// ---------------------------------------------------------------------------

struct Ev {
    ev: Event,
    mark: Marker,
}

fn events(src: &str) -> Result<Vec<Ev>, String> {
    let mut parser = Parser::new_from_str(src);
    let mut out = Vec::new();
    loop {
        let (ev, mark) = parser
            .next_token()
            .map_err(|e| format!("invalid YAML: {e}"))?;
        let done = ev == Event::StreamEnd;
        out.push(Ev { ev, mark });
        if done {
            return Ok(out);
        }
    }
}

/// Index of the event just after the complete node starting at `i`.
fn skip_node(evs: &[Ev], i: usize) -> usize {
    match &evs[i].ev {
        Event::SequenceStart(..) => {
            let mut j = i + 1;
            while j < evs.len() && !matches!(evs[j].ev, Event::SequenceEnd) {
                j = skip_node(evs, j);
            }
            j + 1
        }
        Event::MappingStart(..) => {
            let mut j = i + 1;
            while j < evs.len() && !matches!(evs[j].ev, Event::MappingEnd) {
                j = skip_node(evs, j); // key
                if j >= evs.len() {
                    break;
                }
                j = skip_node(evs, j); // value
            }
            j + 1
        }
        _ => i + 1,
    }
}

/// Where the target node — and its enclosing entry — live in the event stream.
struct Loc {
    /// Event index of the target value node.
    node_i: usize,
    /// Event index of the key scalar (mapping entries only).
    key_i: Option<usize>,
    /// Event index of the parent's MappingStart / SequenceStart.
    parent_i: Option<usize>,
}

/// Walk the event stream along `segs`. Returns `None` when the path can't be
/// followed structurally (aliases, unexpected node kinds) — the caller then
/// falls back to re-emitting the tree.
fn locate(evs: &[Ev], segs: &[Segment]) -> Option<Loc> {
    let mut cur = 0;
    while cur < evs.len() && matches!(evs[cur].ev, Event::StreamStart | Event::DocumentStart) {
        cur += 1;
    }
    if cur >= evs.len() {
        return None;
    }
    let mut loc = Loc {
        node_i: cur,
        key_i: None,
        parent_i: None,
    };
    for seg in segs {
        match &evs[cur].ev {
            Event::MappingStart(..) => {
                let start = cur;
                let mut j = cur + 1;
                let mut found: Option<(usize, usize, Option<usize>)> = None;
                while j < evs.len() && !matches!(evs[j].ev, Event::MappingEnd) {
                    let key_i = j;
                    let val_i = skip_node(evs, key_i);
                    let after = skip_node(evs, val_i);
                    if found.is_none() {
                        if let Event::Scalar(k, ..) = &evs[key_i].ev {
                            if k == &seg.key {
                                let next = if after < evs.len()
                                    && !matches!(evs[after].ev, Event::MappingEnd)
                                {
                                    Some(after)
                                } else {
                                    None
                                };
                                found = Some((key_i, val_i, next));
                            }
                        }
                    }
                    j = after;
                }
                let (key_i, val_i, _next_i) = found?;
                loc = Loc {
                    node_i: val_i,
                    key_i: Some(key_i),
                    parent_i: Some(start),
                };
                cur = val_i;
            }
            Event::SequenceStart(..) => {
                if seg.quoted {
                    return None;
                }
                let idx = seg.key.parse::<usize>().ok()?;
                let start = cur;
                let mut j = cur + 1;
                let mut n = 0usize;
                let mut found: Option<(usize, Option<usize>)> = None;
                while j < evs.len() && !matches!(evs[j].ev, Event::SequenceEnd) {
                    let item_i = j;
                    let after = skip_node(evs, item_i);
                    if n == idx {
                        let next =
                            if after < evs.len() && !matches!(evs[after].ev, Event::SequenceEnd) {
                                Some(after)
                            } else {
                                None
                            };
                        found = Some((item_i, next));
                    }
                    n += 1;
                    j = after;
                }
                let (item_i, _next_i) = found?;
                loc = Loc {
                    node_i: item_i,
                    key_i: None,
                    parent_i: Some(start),
                };
                cur = item_i;
            }
            _ => return None,
        }
    }
    Some(loc)
}

// ---------------------------------------------------------------------------
// Surgical (comment-preserving) edits
// ---------------------------------------------------------------------------

/// Byte offsets of the start of every line, plus the total length as a sentinel.
fn line_starts(src: &str) -> Vec<usize> {
    let mut v = vec![0usize];
    for (i, b) in src.bytes().enumerate() {
        if b == b'\n' {
            v.push(i + 1);
        }
    }
    v.push(src.len());
    v
}

/// Byte offset of a Marker (its column is counted in chars, its line is 1-based).
fn mark_offset(src: &str, starts: &[usize], mark: &Marker) -> Option<usize> {
    let line = mark.line().checked_sub(1)?;
    let start = *starts.get(line)?;
    let end = src[start..].find('\n').map_or(src.len(), |n| start + n);
    let line_text = &src[start..end];
    let mut off = start;
    for (n, (bi, _)) in line_text.char_indices().enumerate() {
        if n == mark.col() {
            return Some(start + bi);
        }
        off = start + bi;
    }
    if mark.col() == line_text.chars().count() {
        Some(end)
    } else {
        let _ = off;
        None
    }
}

/// Is the collection starting at event `i` written in flow style (`{...}` / `[...]`)?
fn is_flow(src: &str, starts: &[usize], evs: &[Ev], i: usize) -> bool {
    match mark_offset(src, starts, &evs[i].mark) {
        Some(off) => matches!(src[off..].chars().next(), Some('{' | '[')),
        None => false,
    }
}

/// End offset (exclusive) of the scalar that begins at `off`, if it fits on one
/// source line. `None` means "not a shape we splice into".
fn scalar_end(src: &str, off: usize, style: TScalarStyle, flow: bool) -> Option<usize> {
    let line_end = src[off..].find('\n').map_or(src.len(), |n| off + n);
    let rest = &src[off..line_end];
    match style {
        TScalarStyle::SingleQuoted | TScalarStyle::DoubleQuoted => {
            let quote = if style == TScalarStyle::SingleQuoted {
                '\''
            } else {
                '"'
            };
            let mut it = rest.char_indices();
            if it.next()?.1 != quote {
                return None;
            }
            while let Some((i, c)) = it.next() {
                if style == TScalarStyle::DoubleQuoted && c == '\\' {
                    it.next()?;
                    continue;
                }
                if c == quote {
                    // A doubled quote inside a single-quoted scalar is an escape.
                    if style == TScalarStyle::SingleQuoted
                        && rest[i + c.len_utf8()..].starts_with('\'')
                    {
                        it.next()?;
                        continue;
                    }
                    return Some(off + i + c.len_utf8());
                }
            }
            None
        }
        TScalarStyle::Plain => {
            let mut end = rest.len();
            let bytes = rest.as_bytes();
            for (i, &b) in bytes.iter().enumerate() {
                // A comment starts at a `#` preceded by whitespace (or at column 0).
                if b == b'#' && (i == 0 || bytes[i - 1] == b' ' || bytes[i - 1] == b'\t') {
                    end = i;
                    break;
                }
                // Inside `{...}` / `[...]` a plain scalar also ends at a separator.
                if flow && matches!(b, b',' | b'}' | b']') {
                    end = i;
                    break;
                }
            }
            let trimmed = rest[..end].trim_end();
            if trimmed.is_empty() {
                return None;
            }
            Some(off + trimmed.len())
        }
        // Block scalars (`|` / `>`) span several lines — not spliced.
        TScalarStyle::Literal | TScalarStyle::Folded => None,
    }
}

/// Number of leading spaces on a line (tabs disqualify a line from the scan).
fn indent_of(line: &str) -> Option<usize> {
    if line.starts_with('\t') {
        return None;
    }
    Some(line.len() - line.trim_start_matches(' ').len())
}

/// Last source line (0-based, inclusive) that belongs to the block entry which
/// starts on line `start_line` at indentation `indent`. Trailing blank and
/// comment lines are left for the following entry.
fn entry_last_line(lines: &[&str], start_line: usize, indent: usize) -> usize {
    let mut last = start_line;
    let mut i = start_line + 1;
    while i < lines.len() {
        let line = lines[i];
        if line.trim().is_empty() {
            i += 1;
            continue;
        }
        let Some(ind) = indent_of(line) else { break };
        if ind <= indent {
            break;
        }
        if !line.trim_start().starts_with('#') {
            last = i;
        }
        i += 1;
    }
    last
}

/// Replace an existing scalar value in place, keeping the rest of the line
/// (including a trailing `# comment`) untouched.
fn splice_scalar(src: &str, evs: &[Ev], loc: &Loc, newval: &Yaml) -> Option<String> {
    let Event::Scalar(_, style, anchor, tag) = &evs[loc.node_i].ev else {
        return None;
    };
    if *anchor != 0 || tag.is_some() {
        return None; // anchored / explicitly tagged values keep their own syntax
    }
    let text = inline_value(newval)?;
    let starts = line_starts(src);
    let off = mark_offset(src, &starts, &evs[loc.node_i].mark)?;
    let flow = loc
        .parent_i
        .map_or(false, |p| is_flow(src, &starts, evs, p));
    let end = scalar_end(src, off, *style, flow)?;
    let mut out = String::with_capacity(src.len() + text.len());
    out.push_str(&src[..off]);
    out.push_str(&text);
    out.push_str(&src[end..]);
    Some(out)
}

/// Append a brand-new `key: value` line to an existing block mapping, right
/// after its last entry.
fn splice_new_key(src: &str, evs: &[Ev], parent: &Loc, key: &str, newval: &Yaml) -> Option<String> {
    let Event::MappingStart(..) = &evs[parent.node_i].ev else {
        return None;
    };
    let starts = line_starts(src);
    if is_flow(src, &starts, evs, parent.node_i) {
        return None;
    }
    let text = inline_value(newval)?;
    let key_text = inline_string(key)?;
    let lines: Vec<&str> = src.split('\n').collect();

    // Walk the mapping's entries to find the last key's line + the indentation.
    let mut j = parent.node_i + 1;
    let mut first_key: Option<usize> = None;
    let mut last_key: Option<usize> = None;
    while j < evs.len() && !matches!(evs[j].ev, Event::MappingEnd) {
        first_key.get_or_insert(j);
        last_key = Some(j);
        j = skip_node(evs, j);
        if j >= evs.len() {
            return None;
        }
        j = skip_node(evs, j);
    }
    let first_key = first_key?;
    let last_key = last_key?;
    let first_line = evs[first_key].mark.line().checked_sub(1)?;
    let indent = indent_of(lines.get(first_line)?)?;
    let last_line = evs[last_key].mark.line().checked_sub(1)?;
    let end_line = entry_last_line(&lines, last_line, indent);

    let insert_at = *starts.get(end_line + 1)?;
    let pad = " ".repeat(indent);
    let mut out = String::with_capacity(src.len() + text.len() + indent + key.len() + 4);
    out.push_str(&src[..insert_at]);
    out.push_str(&format!("{pad}{key_text}: {text}\n"));
    out.push_str(&src[insert_at..]);
    Some(out)
}

/// Remove a whole block-mapping entry or block-sequence item, line by line.
fn splice_delete(src: &str, evs: &[Ev], loc: &Loc) -> Option<String> {
    let starts = line_starts(src);
    if loc.parent_i.map_or(true, |p| is_flow(src, &starts, evs, p)) {
        return None;
    }
    let lines: Vec<&str> = src.split('\n').collect();

    let (start_line, indent) = match loc.key_i {
        // Mapping entry: the entry starts at its key.
        Some(key_i) => {
            let line = evs[key_i].mark.line().checked_sub(1)?;
            let ind = indent_of(lines.get(line)?)?;
            if ind != evs[key_i].mark.col() {
                return None; // key isn't the first thing on its line
            }
            (line, ind)
        }
        // Sequence item: back up from the value to the `- ` bullet. yaml-rust2
        // may mark a block-mapping item either at the value after `- ` or at the
        // dash itself, so find the first dash on the line before-or-at the node
        // marker instead of assuming the marker is always after the bullet.
        None => {
            let mark = &evs[loc.node_i].mark;
            let line = mark.line().checked_sub(1)?;
            let text = lines.get(line)?;
            let off = mark_offset(src, &starts, mark)? - *starts.get(line)?;
            let scan = text.get(..=off.min(text.len().saturating_sub(1)))?;
            let dash = scan.find('-')?;
            if !text[..dash].chars().all(|c| c == ' ') {
                return None;
            }
            (line, dash)
        }
    };

    let end_line = entry_last_line(&lines, start_line, indent);
    let from = *starts.get(start_line)?;
    let to = *starts.get(end_line + 1)?;
    let mut out = String::with_capacity(src.len());
    out.push_str(&src[..from]);
    out.push_str(&src[to..]);
    Some(out)
}

/// Accept a surgical edit only if re-parsing it yields exactly the expected tree.
fn verified(candidate: String, expected: &Yaml) -> Option<String> {
    let docs = YamlLoader::load_from_str(&candidate).ok()?;
    let got = match docs.len() {
        0 => Yaml::Null,
        1 => docs.into_iter().next().expect("len checked"),
        _ => return None,
    };
    if &got == expected {
        Some(candidate)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Query or edit the value at `path` in the YAML document `yaml`.
///
/// - `mode`: `"query"` (default), `"set"`, or `"delete"`.
/// - `value`: for `set`, the value to write. Parsed as YAML (`8080` → number,
///   `true` → boolean, `null` → null, `[a, b]` → list); anything unparseable is
///   stored as a plain string, and quoting forces a string (`"8080"`).
/// - `format`: `"yaml"` (default) or `"json"` for the output.
pub fn run(
    yaml: &str,
    path: &str,
    mode: &str,
    value: &str,
    format: &str,
) -> Result<String, String> {
    let docs = YamlLoader::load_from_str(yaml).map_err(|e| format!("invalid YAML: {e}"))?;
    if docs.len() > 1 {
        return Err(format!(
            "this input holds {} YAML documents separated by '---'; yaml-path-query works on a single document — split it first",
            docs.len()
        ));
    }
    let root = docs.into_iter().next().unwrap_or(Yaml::Null);
    let segs = parse_path(path)?;

    let fmt = format.trim().to_ascii_lowercase();
    let fmt = if fmt.is_empty() { "yaml" } else { fmt.as_str() };
    if fmt != "yaml" && fmt != "json" {
        return Err(format!(
            "unknown format '{fmt}' — expected 'yaml' or 'json'"
        ));
    }

    let mode_lc = mode.trim().to_ascii_lowercase();
    let mode_lc = match mode_lc.as_str() {
        // The CLI/page send an empty string when the field is omitted; `get` is
        // accepted as a familiar alias for `query`.
        "" | "get" | "query" => "query",
        "set" => "set",
        "delete" | "unset" => "delete",
        other => {
            return Err(format!(
                "unknown mode '{other}' — expected 'query', 'set', or 'delete'"
            ))
        }
    };

    match mode_lc {
        "query" => {
            let found = resolve(&root, &segs)?;
            if fmt == "json" {
                return serde_json::to_string_pretty(&to_json(found)?)
                    .map_err(|e| format!("JSON serialize error: {e}"));
            }
            Ok(match found {
                // A scalar hit is returned raw (no surrounding quotes) so it can
                // be piped straight into another command.
                Yaml::String(s) => s.clone(),
                Yaml::Integer(i) => i.to_string(),
                Yaml::Real(s) => s.clone(),
                Yaml::Boolean(b) => b.to_string(),
                Yaml::Null => "null".into(),
                other => emit_yaml(other)?,
            })
        }
        "set" => {
            let newval = parse_value(value);
            if segs.is_empty() {
                return finish(&newval, fmt);
            }
            let mut expected = root.clone();
            set_rec(&mut expected, &segs, newval.clone())?;
            if fmt == "json" {
                return finish(&expected, fmt);
            }
            // Try to keep comments/formatting: replace the existing scalar, or
            // append a new key to an existing block mapping.
            if let Ok(evs) = events(yaml) {
                let candidate = if resolve(&root, &segs).is_ok() {
                    locate(&evs, &segs).and_then(|loc| splice_scalar(yaml, &evs, &loc, &newval))
                } else if segs.len() >= 1 && resolve(&root, &segs[..segs.len() - 1]).is_ok() {
                    let last = &segs[segs.len() - 1];
                    locate(&evs, &segs[..segs.len() - 1])
                        .and_then(|loc| splice_new_key(yaml, &evs, &loc, &last.key, &newval))
                } else {
                    None
                };
                if let Some(out) = candidate.and_then(|c| verified(c, &expected)) {
                    return Ok(out);
                }
            }
            finish(&expected, fmt)
        }
        _ => {
            if segs.is_empty() {
                return Err(
                    "path is empty — nothing to delete (an empty path selects the whole document)"
                        .into(),
                );
            }
            let mut expected = root.clone();
            delete_rec(&mut expected, &segs)?;
            if fmt == "json" {
                return finish(&expected, fmt);
            }
            if let Ok(evs) = events(yaml) {
                let candidate = locate(&evs, &segs).and_then(|loc| splice_delete(yaml, &evs, &loc));
                if let Some(out) = candidate.and_then(|c| verified(c, &expected)) {
                    return Ok(out);
                }
            }
            finish(&expected, fmt)
        }
    }
}

fn finish(v: &Yaml, fmt: &str) -> Result<String, String> {
    if fmt == "json" {
        serde_json::to_string_pretty(&to_json(v)?).map_err(|e| format!("JSON serialize error: {e}"))
    } else {
        emit_yaml(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = "# App config\nserver:\n  host: localhost   # bind address\n  port: 8080\n  tls: false\nitems:\n  - name: alpha\n    qty: 1\n  - name: beta\n    qty: 2\n";

    #[test]
    fn query_dotted_path() {
        assert_eq!(
            run(DOC, "server.host", "query", "", "yaml").unwrap(),
            "localhost"
        );
        assert_eq!(
            run(DOC, "server.port", "query", "", "yaml").unwrap(),
            "8080"
        );
        assert_eq!(
            run(DOC, "server.tls", "query", "", "yaml").unwrap(),
            "false"
        );
    }

    #[test]
    fn query_bracket_and_dotted_index() {
        assert_eq!(
            run(DOC, "items[0].name", "query", "", "yaml").unwrap(),
            "alpha"
        );
        assert_eq!(
            run(DOC, "items.1.name", "query", "", "yaml").unwrap(),
            "beta"
        );
        // A leading `$` root marker is accepted and ignored.
        assert_eq!(
            run(DOC, "$.items[1].qty", "query", "", "yaml").unwrap(),
            "2"
        );
    }

    #[test]
    fn query_quoted_key() {
        let doc = "\"my.key\":\n  id: 7\nkey with spaces: ok\n";
        assert_eq!(
            run(doc, "[\"my.key\"].id", "query", "", "yaml").unwrap(),
            "7"
        );
        assert_eq!(
            run(doc, "['key with spaces']", "query", "", "yaml").unwrap(),
            "ok"
        );
    }

    #[test]
    fn query_subtree_and_json_format() {
        let out = run(DOC, "items[0]", "query", "", "yaml").unwrap();
        assert_eq!(out, "name: alpha\nqty: 1\n");
        let out = run(DOC, "items[0]", "query", "", "json").unwrap();
        assert_eq!(out, "{\n  \"name\": \"alpha\",\n  \"qty\": 1\n}");
        // An empty path selects the whole document.
        let out = run("a: 1\n", "", "query", "", "json").unwrap();
        assert_eq!(out, "{\n  \"a\": 1\n}");
    }

    #[test]
    fn query_json_quotes_strings() {
        assert_eq!(
            run(DOC, "server.host", "query", "", "json").unwrap(),
            "\"localhost\""
        );
    }

    #[test]
    fn empty_mode_defaults_to_query() {
        assert_eq!(run(DOC, "server.port", "", "", "").unwrap(), "8080");
        // `get` is accepted as an alias.
        assert_eq!(run(DOC, "server.port", "get", "", "").unwrap(), "8080");
    }

    #[test]
    fn set_preserves_comments_and_layout() {
        let out = run(DOC, "server.port", "set", "9090", "yaml").unwrap();
        assert_eq!(
            out,
            "# App config\nserver:\n  host: localhost   # bind address\n  port: 9090\n  tls: false\nitems:\n  - name: alpha\n    qty: 1\n  - name: beta\n    qty: 2\n"
        );
        // The inline comment on the edited line survives too.
        let out = run(DOC, "server.host", "set", "0.0.0.0", "yaml").unwrap();
        assert!(out.contains("host: 0.0.0.0   # bind address"), "{out}");
        assert!(out.starts_with("# App config\n"), "{out}");
    }

    #[test]
    fn set_infers_types_and_quotes_when_needed() {
        assert_eq!(
            run("a: 1\n", "a", "set", "true", "yaml").unwrap(),
            "a: true\n"
        );
        assert_eq!(
            run("a: 1\n", "a", "set", "null", "yaml").unwrap(),
            "a: null\n"
        );
        assert_eq!(
            run("a: 1\n", "a", "set", "hello", "yaml").unwrap(),
            "a: hello\n"
        );
        // A string that would otherwise be re-read as a number gets quoted.
        assert_eq!(
            run("a: x\n", "a", "set", "\"8080\"", "yaml").unwrap(),
            "a: \"8080\"\n"
        );
        assert_eq!(run("a: 1\n", "a", "query", "", "json").unwrap(), "1");
        // Flow collections can be written inline.
        assert_eq!(
            run("a: 1\n", "a", "set", "[x, y]", "yaml").unwrap(),
            "a: [x, y]\n"
        );
    }

    #[test]
    fn set_replaces_quoted_scalar_in_place() {
        let src = "name: 'old value'  # keep me\n";
        let out = run(src, "name", "set", "new", "yaml").unwrap();
        assert_eq!(out, "name: new  # keep me\n");
        let src = "name: \"old\"\n";
        assert_eq!(
            run(src, "name", "set", "new", "yaml").unwrap(),
            "name: new\n"
        );
    }

    #[test]
    fn set_inside_a_list_item() {
        let out = run(DOC, "items[1].qty", "set", "42", "yaml").unwrap();
        assert!(out.contains("  - name: beta\n    qty: 42\n"), "{out}");
        assert!(out.contains("# bind address"), "{out}");
    }

    #[test]
    fn set_appends_a_new_key_keeping_comments() {
        let out = run(DOC, "server.workers", "set", "4", "yaml").unwrap();
        assert!(out.contains("  tls: false\n  workers: 4\n"), "{out}");
        assert!(out.starts_with("# App config\n"), "{out}");
    }

    #[test]
    fn set_creating_intermediates_falls_back_to_reemit() {
        // Creating a whole missing branch can't be spliced; the document is
        // re-emitted from the tree (correct, but comments are dropped).
        let out = run("a: 1\n", "b.c.d", "set", "x", "yaml").unwrap();
        assert_eq!(out, "a: 1\nb:\n  c:\n    d: x\n");
    }

    #[test]
    fn set_in_flow_mapping_keeps_the_line() {
        let src = "# top\nserver: {host: localhost, port: 8080}\n";
        let out = run(src, "server.port", "set", "9090", "yaml").unwrap();
        assert_eq!(out, "# top\nserver: {host: localhost, port: 9090}\n");
    }

    #[test]
    fn delete_mapping_key_preserves_the_rest() {
        let out = run(DOC, "server.tls", "delete", "", "yaml").unwrap();
        assert_eq!(
            out,
            "# App config\nserver:\n  host: localhost   # bind address\n  port: 8080\nitems:\n  - name: alpha\n    qty: 1\n  - name: beta\n    qty: 2\n"
        );
    }

    #[test]
    fn delete_list_item_removes_the_whole_block() {
        let out = run(DOC, "items[0]", "delete", "", "yaml").unwrap();
        assert_eq!(
            out,
            "# App config\nserver:\n  host: localhost   # bind address\n  port: 8080\n  tls: false\nitems:\n  - name: beta\n    qty: 2\n"
        );
    }

    #[test]
    fn delete_nested_block_keeps_siblings() {
        let out = run(DOC, "server", "delete", "", "yaml").unwrap();
        assert_eq!(
            out,
            "# App config\nitems:\n  - name: alpha\n    qty: 1\n  - name: beta\n    qty: 2\n"
        );
    }

    #[test]
    fn delete_with_json_output() {
        let out = run("a: 1\nb: 2\n", "b", "delete", "", "json").unwrap();
        assert_eq!(out, "{\n  \"a\": 1\n}");
    }

    #[test]
    fn edits_never_corrupt_when_splicing_is_impossible() {
        // A literal block scalar can't be spliced — the fallback re-emit still
        // produces the right data.
        let src = "script: |\n  line one\n  line two\nname: x\n";
        let out = run(src, "script", "set", "echo hi", "yaml").unwrap();
        let back = YamlLoader::load_from_str(&out).unwrap();
        assert_eq!(back[0]["script"].as_str().unwrap(), "echo hi");
        assert_eq!(back[0]["name"].as_str().unwrap(), "x");
    }

    #[test]
    fn query_keeps_multiline_block_scalar_text() {
        let src = "script: |\n  line one\n  line two\n";
        assert_eq!(
            run(src, "script", "query", "", "yaml").unwrap(),
            "line one\nline two\n"
        );
    }

    #[test]
    fn numeric_and_boolean_keys_are_addressable() {
        let src = "1: one\ntrue: yes-value\n";
        assert_eq!(run(src, "1", "query", "", "yaml").unwrap(), "one");
        assert_eq!(run(src, "true", "query", "", "yaml").unwrap(), "yes-value");
    }

    #[test]
    fn err_invalid_yaml() {
        let e = run("a: [1, 2\n", "a", "query", "", "yaml").unwrap_err();
        assert!(e.contains("invalid YAML"), "{e}");
    }

    #[test]
    fn err_missing_key() {
        let e = run(DOC, "server.nope", "query", "", "yaml").unwrap_err();
        assert!(e.contains("no value at path"), "{e}");
    }

    #[test]
    fn err_index_out_of_bounds() {
        let e = run(DOC, "items[9]", "query", "", "yaml").unwrap_err();
        assert!(e.contains("out of bounds"), "{e}");
    }

    #[test]
    fn err_descend_into_scalar() {
        let e = run(DOC, "server.port.deep", "query", "", "yaml").unwrap_err();
        assert!(e.contains("cannot descend into a number"), "{e}");
    }

    #[test]
    fn err_set_into_scalar() {
        let e = run("a: 5\n", "a.b", "set", "1", "yaml").unwrap_err();
        assert!(e.contains("not a mapping or a list"), "{e}");
    }

    #[test]
    fn err_delete_missing_key() {
        let e = run(DOC, "server.nope", "delete", "", "yaml").unwrap_err();
        assert!(e.contains("nothing to delete"), "{e}");
    }

    #[test]
    fn err_unknown_mode_and_format() {
        let e = run("a: 1\n", "a", "frobnicate", "", "yaml").unwrap_err();
        assert!(e.contains("unknown mode"), "{e}");
        let e = run("a: 1\n", "a", "query", "", "xml").unwrap_err();
        assert!(e.contains("unknown format"), "{e}");
    }

    #[test]
    fn err_multi_document() {
        let e = run("a: 1\n---\nb: 2\n", "a", "query", "", "yaml").unwrap_err();
        assert!(e.contains("single document"), "{e}");
    }

    #[test]
    fn err_bad_path_syntax() {
        let e = run("a: 1\n", "a[", "query", "", "yaml").unwrap_err();
        assert!(e.contains("unterminated"), "{e}");
        let e = run("a: 1\n", "a[]", "query", "", "yaml").unwrap_err();
        assert!(e.contains("empty '[]'"), "{e}");
    }

    #[test]
    fn err_list_grow_cap() {
        let e = run("a: []\n", "a[999999]", "set", "1", "yaml").unwrap_err();
        assert!(e.contains("maximum"), "{e}");
    }

    #[test]
    fn err_empty_path_delete() {
        let e = run("a: 1\n", "", "delete", "", "yaml").unwrap_err();
        assert!(e.contains("nothing to delete"), "{e}");
    }

    #[test]
    fn anchors_are_resolved_on_query() {
        let src = "base: &b\n  x: 1\nuse:\n  <<: *b\nalias: *b\n";
        assert_eq!(run(src, "alias.x", "query", "", "yaml").unwrap(), "1");
    }
}

//! gizza-ai/api-response-diff core — compare two JSON API responses while
//! ignoring volatile fields, so only meaningful changes surface.
//!
//! Walks both documents recursively and reports, per JSON path, whether a value
//! was added, removed, changed, or changed type. Paths matching an ignore
//! pattern are skipped entirely; optional value-shape filters drop
//! timestamp-only and UUID-only churn. Arrays can be matched by index, by a key
//! field, or as unordered sets. Pure-Rust (serde_json, preserve_order).

use serde::Serialize;
use serde_json::{Map, Value};

/// Largest accepted response body, per side.
pub const MAX_INPUT_BYTES: usize = 4 * 1024 * 1024;
/// Largest number of change/ignored entries listed in one report.
pub const MAX_CHANGES: usize = 2000;
/// Arrays longer than this fall back to index matching in `set`/`key` mode.
pub const MAX_UNORDERED_ARRAY: usize = 2000;
/// Largest number of fallback notes recorded.
const MAX_NOTES: usize = 20;

/// How array elements are paired up before being compared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrayMatch {
    /// Position by position (`$.items[0]` vs `$.items[0]`).
    Index,
    /// By the value of a key field, so reordered lists diff field-by-field.
    Key,
    /// Order-insensitive: arrays are compared as unordered multisets.
    Set,
}

/// Shape of the rendered result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// Full JSON report: counts, ignored paths, notes, per-change entries.
    Report,
    /// One human-readable line per change.
    Summary,
    /// RFC 6902 JSON Patch array (add/remove/replace ops).
    Patch,
}

/// Every knob the diff understands.
#[derive(Debug, Clone)]
pub struct Options {
    /// Comma/newline separated ignore patterns: a bare field name matches that
    /// key at any depth, a dotted path is anchored at the root, `*` matches one
    /// segment, `**` any number, `[2]`/`[*]` array indices.
    pub ignore: String,
    pub ignore_timestamps: bool,
    pub ignore_uuids: bool,
    pub array_match: ArrayMatch,
    pub array_key: String,
    pub numeric_tolerance: f64,
    pub ignore_case: bool,
    pub trim_strings: bool,
    pub null_equals_missing: bool,
    pub coerce_types: bool,
    pub output: OutputFormat,
    pub indent: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            ignore: String::new(),
            ignore_timestamps: false,
            ignore_uuids: false,
            array_match: ArrayMatch::Index,
            array_key: "id".to_string(),
            numeric_tolerance: 0.0,
            ignore_case: false,
            trim_strings: false,
            null_equals_missing: false,
            coerce_types: false,
            output: OutputFormat::Report,
            indent: 2,
        }
    }
}

/// One difference between the two responses.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Change {
    /// JSON path of the location, e.g. `$.data.total` or `$.items[2].name`.
    /// In `key` array mode elements are addressed as `$.items[id=42]`.
    pub path: String,
    /// `added`, `removed`, `changed` or `type_changed`.
    pub kind: String,
    /// Value in the first (left/old) response, when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old: Option<Value>,
    /// Value in the second (right/new) response, when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new: Option<Value>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct Counts {
    added: usize,
    removed: usize,
    changed: usize,
    type_changed: usize,
    ignored: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
struct Report {
    equal: bool,
    counts: Counts,
    truncated: bool,
    notes: Vec<String>,
    ignored_paths: Vec<String>,
    changes: Vec<Change>,
}

// ---------------------------------------------------------------------------
// paths
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Seg {
    Key(String),
    Idx(usize),
    /// `items[id=42]` — a keyed array element (only in `key` array mode).
    Keyed(String, String),
}

fn path_string(segs: &[Seg]) -> String {
    let mut s = String::from("$");
    for seg in segs {
        match seg {
            Seg::Key(k) => {
                s.push('.');
                s.push_str(k);
            }
            Seg::Idx(i) => s.push_str(&format!("[{i}]")),
            Seg::Keyed(k, v) => s.push_str(&format!("[{k}={v}]")),
        }
    }
    s
}

/// RFC 6901 JSON Pointer for a path (only used for `output=patch`, which is
/// restricted to index array matching, so `Seg::Keyed` cannot appear).
fn pointer_string(segs: &[Seg]) -> String {
    let mut s = String::new();
    for seg in segs {
        s.push('/');
        match seg {
            Seg::Key(k) => s.push_str(&k.replace('~', "~0").replace('/', "~1")),
            Seg::Idx(i) => s.push_str(&i.to_string()),
            Seg::Keyed(k, v) => s.push_str(&format!("{k}={v}")),
        }
    }
    s
}

// ---------------------------------------------------------------------------
// ignore patterns
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Pat {
    /// A key name, possibly containing `*` wildcards (`*_at`).
    Key(String),
    /// `*` — exactly one segment of any kind.
    AnySeg,
    /// `**` — any number of segments (including none).
    AnyDepth,
    Idx(usize),
    AnyIdx,
}

/// Parse the comma/newline separated ignore list.
///
/// * a bare field name (`updatedAt`, `*_at`) matches that key at ANY depth;
/// * a dotted path (`data.token`, `$.data.token`) is anchored at the root;
/// * `*` matches one path segment, `**` matches any number of segments;
/// * `[2]` matches one array index, `[*]` matches any index.
fn parse_patterns(raw: &str) -> Result<Vec<Vec<Pat>>, String> {
    let mut out = Vec::new();
    for item in raw.split([',', '\n', '\r']) {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        out.push(parse_pattern(item)?);
    }
    Ok(out)
}

fn parse_pattern(raw: &str) -> Result<Vec<Pat>, String> {
    let anchored = raw.starts_with('$') || raw.contains('.') || raw.contains('[');
    let body = raw.strip_prefix("$.").unwrap_or_else(|| raw.strip_prefix('$').unwrap_or(raw));
    let mut pats: Vec<Pat> = Vec::new();
    if !anchored {
        pats.push(Pat::AnyDepth);
    }
    for token in body.split('.') {
        if token.is_empty() {
            continue;
        }
        // split a token like `items[0][*]` into its name and index parts
        let (name, rest) = match token.find('[') {
            Some(i) => (&token[..i], &token[i..]),
            None => (token, ""),
        };
        if !name.is_empty() {
            match name {
                "**" => pats.push(Pat::AnyDepth),
                "*" => pats.push(Pat::AnySeg),
                _ => pats.push(Pat::Key(name.to_string())),
            }
        }
        let mut rest = rest;
        while !rest.is_empty() {
            let close = rest.find(']').ok_or_else(|| {
                format!("ignore pattern \"{raw}\": unclosed '[' — write items[0], items[*] or items")
            })?;
            let inner = &rest[1..close];
            if inner == "*" {
                pats.push(Pat::AnyIdx);
            } else {
                let n: usize = inner.parse().map_err(|_| {
                    format!(
                        "ignore pattern \"{raw}\": array index must be a number or '*', got \"{inner}\""
                    )
                })?;
                pats.push(Pat::Idx(n));
            }
            rest = &rest[close + 1..];
        }
    }
    if pats.is_empty() || pats.iter().all(|p| matches!(p, Pat::AnyDepth)) {
        return Err(format!("ignore pattern \"{raw}\" is empty — give a field name or path"));
    }
    Ok(pats)
}

/// `*`-only glob match (no `?`, no character classes).
fn glob_match(pat: &str, text: &str) -> bool {
    if !pat.contains('*') {
        return pat == text;
    }
    let parts: Vec<&str> = pat.split('*').collect();
    let mut pos = 0usize;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if i == 0 {
            if !text[pos..].starts_with(part) {
                return false;
            }
            pos += part.len();
        } else if i == parts.len() - 1 {
            if text.len() < pos + part.len() || !text[pos..].ends_with(part) {
                return false;
            }
            pos = text.len();
        } else {
            match text[pos..].find(part) {
                Some(at) => pos += at + part.len(),
                None => return false,
            }
        }
    }
    true
}

fn seg_matches(pat: &Pat, seg: &Seg) -> bool {
    match (pat, seg) {
        (Pat::AnySeg, _) => true,
        (Pat::Key(k), Seg::Key(s)) => glob_match(k, s),
        (Pat::Key(k), Seg::Keyed(_, v)) => glob_match(k, v),
        (Pat::Idx(n), Seg::Idx(m)) => n == m,
        (Pat::AnyIdx, Seg::Idx(_)) => true,
        (Pat::AnyIdx, Seg::Keyed(_, _)) => true,
        _ => false,
    }
}

fn match_pats(pats: &[Pat], segs: &[Seg]) -> bool {
    match pats.first() {
        None => segs.is_empty(),
        Some(Pat::AnyDepth) => {
            (0..=segs.len()).any(|i| match_pats(&pats[1..], &segs[i..]))
        }
        Some(p) => match segs.first() {
            Some(s) if seg_matches(p, s) => match_pats(&pats[1..], &segs[1..]),
            _ => false,
        },
    }
}

fn is_ignored(patterns: &[Vec<Pat>], segs: &[Seg]) -> bool {
    !segs.is_empty() && patterns.iter().any(|p| match_pats(p, segs))
}

// ---------------------------------------------------------------------------
// value-shape filters
// ---------------------------------------------------------------------------

fn is_iso_timestamp(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() < 10 {
        return false;
    }
    let digits = |r: std::ops::Range<usize>| b[r].iter().all(|c| c.is_ascii_digit());
    if !(digits(0..4) && b[4] == b'-' && digits(5..7) && b[7] == b'-' && digits(8..10)) {
        return false;
    }
    match b.get(10) {
        None => true,
        Some(&c) => (c == b'T' || c == b't' || c == b' ') && b.len() >= 16,
    }
}

fn is_epoch_number(f: f64) -> bool {
    f.fract() == 0.0
        && ((1_000_000_000.0..=4_102_444_800.0).contains(&f)
            || (1.0e12..=4.1024448e12).contains(&f))
}

/// Does this value look like a timestamp (ISO-8601 string, epoch seconds/ms)?
pub fn looks_like_timestamp(v: &Value) -> bool {
    match v {
        Value::String(s) => {
            let s = s.trim();
            is_iso_timestamp(s)
                || ((s.len() == 10 || s.len() == 13)
                    && s.bytes().all(|c| c.is_ascii_digit())
                    && s.parse::<f64>().map(is_epoch_number).unwrap_or(false))
        }
        Value::Number(n) => n.as_f64().map(is_epoch_number).unwrap_or(false),
        _ => false,
    }
}

/// Does this value look like a canonical 8-4-4-4-12 UUID string?
pub fn looks_like_uuid(v: &Value) -> bool {
    let s = match v {
        Value::String(s) => s.trim(),
        _ => return false,
    };
    let b = s.as_bytes();
    if b.len() != 36 {
        return false;
    }
    for (i, c) in b.iter().enumerate() {
        let dash = matches!(i, 8 | 13 | 18 | 23);
        if dash {
            if *c != b'-' {
                return false;
            }
        } else if !c.is_ascii_hexdigit() {
            return false;
        }
    }
    true
}

// ---------------------------------------------------------------------------
// value comparison
// ---------------------------------------------------------------------------

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn norm_string(s: &str, opts: &Options) -> String {
    let s = if opts.trim_strings { s.trim() } else { s };
    if opts.ignore_case {
        s.to_lowercase()
    } else {
        s.to_string()
    }
}

fn numbers_equal(a: f64, b: f64, opts: &Options) -> bool {
    (a - b).abs() <= opts.numeric_tolerance
}

/// Scalar (non-container) equality under the current options.
fn scalars_equal(a: &Value, b: &Value, opts: &Options) -> bool {
    if opts.ignore_timestamps && looks_like_timestamp(a) && looks_like_timestamp(b) {
        return true;
    }
    if opts.ignore_uuids && looks_like_uuid(a) && looks_like_uuid(b) {
        return true;
    }
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            match (x.as_f64(), y.as_f64()) {
                (Some(x), Some(y)) => numbers_equal(x, y, opts),
                _ => x == y,
            }
        }
        (Value::String(x), Value::String(y)) => norm_string(x, opts) == norm_string(y, opts),
        (Value::Null, Value::Null) => true,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        _ if opts.coerce_types => coerced_equal(a, b, opts),
        _ => false,
    }
}

/// `"5" == 5`, `"true" == true` — only consulted when `coerce_types` is on.
fn coerced_equal(a: &Value, b: &Value, opts: &Options) -> bool {
    let as_num = |v: &Value| -> Option<f64> {
        match v {
            Value::Number(n) => n.as_f64(),
            Value::String(s) => s.trim().parse::<f64>().ok(),
            _ => None,
        }
    };
    if let (Some(x), Some(y)) = (as_num(a), as_num(b)) {
        return numbers_equal(x, y, opts);
    }
    let as_bool = |v: &Value| -> Option<bool> {
        match v {
            Value::Bool(x) => Some(*x),
            Value::String(s) => match s.trim().to_ascii_lowercase().as_str() {
                "true" => Some(true),
                "false" => Some(false),
                _ => None,
            },
            _ => None,
        }
    };
    if let (Some(x), Some(y)) = (as_bool(a), as_bool(b)) {
        return x == y;
    }
    false
}

/// Treat an explicit `null` as "absent" when `null_equals_missing` is on.
fn is_absent(v: Option<&Value>, opts: &Options) -> bool {
    match v {
        None => true,
        Some(Value::Null) => opts.null_equals_missing,
        Some(_) => false,
    }
}

// ---------------------------------------------------------------------------
// walker
// ---------------------------------------------------------------------------

struct State<'a> {
    patterns: &'a [Vec<Pat>],
    changes: Vec<Change>,
    /// Segment path of each recorded change, kept for JSON Pointer rendering.
    segs_of: Vec<Vec<Seg>>,
    ignored: Vec<String>,
    counts: Counts,
    truncated: bool,
    notes: Vec<String>,
}

impl State<'_> {
    fn push(&mut self, kind: &str, segs: &[Seg], old: Option<&Value>, new: Option<&Value>) {
        match kind {
            "added" => self.counts.added += 1,
            "removed" => self.counts.removed += 1,
            "changed" => self.counts.changed += 1,
            _ => self.counts.type_changed += 1,
        }
        if self.changes.len() >= MAX_CHANGES {
            self.truncated = true;
            return;
        }
        self.changes.push(Change {
            path: path_string(segs),
            kind: kind.to_string(),
            old: old.cloned(),
            new: new.cloned(),
        });
        self.segs_of.push(segs.to_vec());
    }

    fn ignore(&mut self, segs: &[Seg]) {
        self.counts.ignored += 1;
        if self.ignored.len() >= MAX_CHANGES {
            self.truncated = true;
            return;
        }
        self.ignored.push(path_string(segs));
    }

    fn note(&mut self, msg: String) {
        if self.notes.len() < MAX_NOTES && !self.notes.contains(&msg) {
            self.notes.push(msg);
        }
    }
}

fn walk(
    segs: &mut Vec<Seg>,
    a: Option<&Value>,
    b: Option<&Value>,
    opts: &Options,
    st: &mut State,
) {
    if is_ignored(st.patterns, segs) {
        st.ignore(segs);
        return;
    }
    match (a, b) {
        (None, None) => {}
        (Some(x), None) => {
            if !is_absent(Some(x), opts) {
                st.push("removed", segs, Some(x), None);
            }
        }
        (None, Some(y)) => {
            if !is_absent(Some(y), opts) {
                st.push("added", segs, None, Some(y));
            }
        }
        (Some(x), Some(y)) => match (x, y) {
            (Value::Object(m), Value::Object(n)) => walk_object(segs, m, n, opts, st),
            (Value::Array(p), Value::Array(q)) => walk_array(segs, p, q, opts, st),
            _ => {
                if x == y {
                    return;
                }
                // a value-shape filter matched both sides → volatile, not a change
                if (opts.ignore_timestamps && looks_like_timestamp(x) && looks_like_timestamp(y))
                    || (opts.ignore_uuids && looks_like_uuid(x) && looks_like_uuid(y))
                {
                    st.ignore(segs);
                    return;
                }
                if scalars_equal(x, y, opts) {
                    return;
                }
                if type_name(x) != type_name(y) {
                    st.push("type_changed", segs, Some(x), Some(y));
                } else {
                    st.push("changed", segs, Some(x), Some(y));
                }
            }
        },
    }
}

fn walk_object(
    segs: &mut Vec<Seg>,
    a: &Map<String, Value>,
    b: &Map<String, Value>,
    opts: &Options,
    st: &mut State,
) {
    for (k, av) in a {
        segs.push(Seg::Key(k.clone()));
        walk(segs, Some(av), b.get(k), opts, st);
        segs.pop();
    }
    for (k, bv) in b {
        if a.contains_key(k) {
            continue;
        }
        segs.push(Seg::Key(k.clone()));
        walk(segs, None, Some(bv), opts, st);
        segs.pop();
    }
}

fn walk_array(segs: &mut Vec<Seg>, a: &[Value], b: &[Value], opts: &Options, st: &mut State) {
    let too_big = a.len() > MAX_UNORDERED_ARRAY || b.len() > MAX_UNORDERED_ARRAY;
    match opts.array_match {
        ArrayMatch::Index => walk_array_by_index(segs, a, b, opts, st),
        ArrayMatch::Set if too_big => {
            st.note(format!(
                "{}: over {MAX_UNORDERED_ARRAY} elements — compared by index instead of as a set",
                path_string(segs)
            ));
            walk_array_by_index(segs, a, b, opts, st)
        }
        ArrayMatch::Set => walk_array_as_set(segs, a, b, opts, st),
        ArrayMatch::Key if too_big => {
            st.note(format!(
                "{}: over {MAX_UNORDERED_ARRAY} elements — compared by index instead of by \"{}\"",
                path_string(segs),
                opts.array_key
            ));
            walk_array_by_index(segs, a, b, opts, st)
        }
        ArrayMatch::Key => match keys_of(a, &opts.array_key).zip(keys_of(b, &opts.array_key)) {
            Some((ka, kb)) => walk_array_by_key(segs, a, &ka, b, &kb, opts, st),
            None => {
                st.note(format!(
                    "{}: elements are not objects with a unique \"{}\" — compared by index",
                    path_string(segs),
                    opts.array_key
                ));
                walk_array_by_index(segs, a, b, opts, st)
            }
        },
    }
}

fn walk_array_by_index(
    segs: &mut Vec<Seg>,
    a: &[Value],
    b: &[Value],
    opts: &Options,
    st: &mut State,
) {
    for i in 0..a.len().max(b.len()) {
        segs.push(Seg::Idx(i));
        walk(segs, a.get(i), b.get(i), opts, st);
        segs.pop();
    }
}

/// The array-key value of every element, as a display string — `None` if any
/// element is not an object carrying the key, or if the keys are not unique.
fn keys_of(items: &[Value], key: &str) -> Option<Vec<String>> {
    let mut out = Vec::with_capacity(items.len());
    for it in items {
        let v = it.as_object()?.get(key)?;
        let s = match v {
            Value::String(s) => s.clone(),
            Value::Null | Value::Array(_) | Value::Object(_) => return None,
            other => other.to_string(),
        };
        if out.contains(&s) {
            return None;
        }
        out.push(s);
    }
    Some(out)
}

#[allow(clippy::too_many_arguments)]
fn walk_array_by_key(
    segs: &mut Vec<Seg>,
    a: &[Value],
    ka: &[String],
    b: &[Value],
    kb: &[String],
    opts: &Options,
    st: &mut State,
) {
    for (i, key) in ka.iter().enumerate() {
        segs.push(Seg::Keyed(opts.array_key.clone(), key.clone()));
        let other = kb.iter().position(|k| k == key).map(|j| &b[j]);
        walk(segs, Some(&a[i]), other, opts, st);
        segs.pop();
    }
    for (j, key) in kb.iter().enumerate() {
        if ka.iter().any(|k| k == key) {
            continue;
        }
        segs.push(Seg::Keyed(opts.array_key.clone(), key.clone()));
        walk(segs, None, Some(&b[j]), opts, st);
        segs.pop();
    }
}

fn walk_array_as_set(
    segs: &mut Vec<Seg>,
    a: &[Value],
    b: &[Value],
    opts: &Options,
    st: &mut State,
) {
    let mut taken = vec![false; b.len()];
    let mut unmatched_left = Vec::new();
    for (i, av) in a.iter().enumerate() {
        segs.push(Seg::Idx(i));
        let hit = b.iter().enumerate().position(|(j, bv)| {
            !taken[j] && deep_equal(segs, av, bv, opts, st.patterns)
        });
        segs.pop();
        match hit {
            Some(j) => taken[j] = true,
            None => unmatched_left.push(i),
        }
    }
    for i in unmatched_left {
        segs.push(Seg::Idx(i));
        walk(segs, Some(&a[i]), None, opts, st);
        segs.pop();
    }
    for (j, bv) in b.iter().enumerate() {
        if taken[j] {
            continue;
        }
        segs.push(Seg::Idx(j));
        walk(segs, None, Some(bv), opts, st);
        segs.pop();
    }
}

/// Whole-subtree equality under the current options (used to pair up elements
/// in `set` mode). Ignored paths count as equal.
fn deep_equal(
    segs: &mut Vec<Seg>,
    a: &Value,
    b: &Value,
    opts: &Options,
    patterns: &[Vec<Pat>],
) -> bool {
    if is_ignored(patterns, segs) {
        return true;
    }
    match (a, b) {
        (Value::Object(m), Value::Object(n)) => {
            for (k, av) in m {
                segs.push(Seg::Key(k.clone()));
                let ok = match n.get(k) {
                    Some(bv) => deep_equal(segs, av, bv, opts, patterns),
                    None => is_ignored(patterns, segs) || is_absent(Some(av), opts),
                };
                segs.pop();
                if !ok {
                    return false;
                }
            }
            for (k, bv) in n {
                if m.contains_key(k) {
                    continue;
                }
                segs.push(Seg::Key(k.clone()));
                let ok = is_ignored(patterns, segs) || is_absent(Some(bv), opts);
                segs.pop();
                if !ok {
                    return false;
                }
            }
            true
        }
        (Value::Array(p), Value::Array(q)) => {
            if opts.array_match == ArrayMatch::Set && p.len() <= MAX_UNORDERED_ARRAY {
                if p.len() != q.len() {
                    return false;
                }
                let mut taken = vec![false; q.len()];
                for (i, av) in p.iter().enumerate() {
                    segs.push(Seg::Idx(i));
                    let hit = q
                        .iter()
                        .enumerate()
                        .position(|(j, bv)| !taken[j] && deep_equal(segs, av, bv, opts, patterns));
                    segs.pop();
                    match hit {
                        Some(j) => taken[j] = true,
                        None => return false,
                    }
                }
                true
            } else {
                if p.len() != q.len() {
                    return false;
                }
                for (i, (av, bv)) in p.iter().zip(q.iter()).enumerate() {
                    segs.push(Seg::Idx(i));
                    let ok = deep_equal(segs, av, bv, opts, patterns);
                    segs.pop();
                    if !ok {
                        return false;
                    }
                }
                true
            }
        }
        _ => scalars_equal(a, b, opts),
    }
}

// ---------------------------------------------------------------------------
// rendering
// ---------------------------------------------------------------------------

fn render_value(v: &Value) -> String {
    let s = serde_json::to_string(v).unwrap_or_else(|_| "null".into());
    if s.chars().count() > 200 {
        let cut: String = s.chars().take(200).collect();
        format!("{cut}…")
    } else {
        s
    }
}

fn to_json(v: &impl Serialize, indent: usize) -> Result<String, String> {
    if indent == 0 {
        serde_json::to_string(v).map_err(|e| e.to_string())
    } else {
        let pad = vec![b' '; indent.min(8)];
        let mut buf = Vec::new();
        let fmt = serde_json::ser::PrettyFormatter::with_indent(&pad);
        let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
        v.serialize(&mut ser).map_err(|e| e.to_string())?;
        String::from_utf8(buf).map_err(|e| e.to_string())
    }
}

fn render_summary(rep: &Report) -> String {
    let c = &rep.counts;
    let total = c.added + c.removed + c.changed + c.type_changed;
    let mut lines = Vec::new();
    if total == 0 {
        lines.push(format!("No differences ({} ignored).", c.ignored));
    } else {
        lines.push(format!(
            "{total} difference{} ({} added, {} removed, {} changed, {} type changed); {} ignored",
            if total == 1 { "" } else { "s" },
            c.added,
            c.removed,
            c.changed,
            c.type_changed,
            c.ignored
        ));
    }
    for ch in &rep.changes {
        let line = match ch.kind.as_str() {
            "added" => format!(
                "+ {}: {}",
                ch.path,
                ch.new.as_ref().map(render_value).unwrap_or_default()
            ),
            "removed" => format!(
                "- {}: {}",
                ch.path,
                ch.old.as_ref().map(render_value).unwrap_or_default()
            ),
            "type_changed" => format!(
                "! {}: {} {} -> {} {}",
                ch.path,
                ch.old.as_ref().map(type_name).unwrap_or("null"),
                ch.old.as_ref().map(render_value).unwrap_or_default(),
                ch.new.as_ref().map(type_name).unwrap_or("null"),
                ch.new.as_ref().map(render_value).unwrap_or_default()
            ),
            _ => format!(
                "~ {}: {} -> {}",
                ch.path,
                ch.old.as_ref().map(render_value).unwrap_or_default(),
                ch.new.as_ref().map(render_value).unwrap_or_default()
            ),
        };
        lines.push(line);
    }
    if rep.truncated {
        lines.push(format!("… listing capped at {MAX_CHANGES} entries."));
    }
    for n in &rep.notes {
        lines.push(format!("note: {n}"));
    }
    lines.join("\n")
}

fn render_patch(rep: &Report, segs_of: &[Vec<Seg>], indent: usize) -> Result<String, String> {
    let mut ops: Vec<Value> = Vec::new();
    for (ch, segs) in rep.changes.iter().zip(segs_of.iter()) {
        let ptr = pointer_string(segs);
        let mut op = Map::new();
        match ch.kind.as_str() {
            "added" => {
                op.insert("op".into(), Value::String("add".into()));
                op.insert("path".into(), Value::String(ptr));
                op.insert("value".into(), ch.new.clone().unwrap_or(Value::Null));
            }
            "removed" => {
                op.insert("op".into(), Value::String("remove".into()));
                op.insert("path".into(), Value::String(ptr));
            }
            _ => {
                op.insert("op".into(), Value::String("replace".into()));
                op.insert("path".into(), Value::String(ptr));
                op.insert("value".into(), ch.new.clone().unwrap_or(Value::Null));
            }
        }
        ops.push(Value::Object(op));
    }
    to_json(&Value::Array(ops), indent)
}

// ---------------------------------------------------------------------------
// entry point
// ---------------------------------------------------------------------------

/// Diff two JSON API responses and render the result per `opts.output`.
pub fn diff_responses(left: &str, right: &str, opts: &Options) -> Result<String, String> {
    for (label, body) in [("left", left), ("right", right)] {
        if body.len() > MAX_INPUT_BYTES {
            return Err(format!(
                "{label} response is {} bytes, over the {} MiB limit — trim the payload before diffing",
                body.len(),
                MAX_INPUT_BYTES / 1024 / 1024
            ));
        }
    }
    if opts.numeric_tolerance < 0.0 || !opts.numeric_tolerance.is_finite() {
        return Err(format!(
            "numeric_tolerance must be a finite value >= 0, got {}",
            opts.numeric_tolerance
        ));
    }
    if opts.array_match == ArrayMatch::Key && opts.array_key.trim().is_empty() {
        return Err("array_key must name a field (e.g. id) when array_match=key".into());
    }
    if opts.output == OutputFormat::Patch && opts.array_match != ArrayMatch::Index {
        return Err(
            "output=patch needs array_match=index — JSON Pointer paths are only well-defined for index matching; use output=report or output=summary with key/set matching"
                .into(),
        );
    }
    let a: Value = serde_json::from_str(left)
        .map_err(|e| format!("left response is not valid JSON: {e}"))?;
    let b: Value = serde_json::from_str(right)
        .map_err(|e| format!("right response is not valid JSON: {e}"))?;

    let patterns = parse_patterns(&opts.ignore)?;
    let mut st = State {
        patterns: &patterns,
        changes: Vec::new(),
        segs_of: Vec::new(),
        ignored: Vec::new(),
        counts: Counts {
            added: 0,
            removed: 0,
            changed: 0,
            type_changed: 0,
            ignored: 0,
        },
        truncated: false,
        notes: Vec::new(),
    };
    let mut segs: Vec<Seg> = Vec::new();
    walk(&mut segs, Some(&a), Some(&b), opts, &mut st);
    let segs_of = std::mem::take(&mut st.segs_of);

    let total = st.counts.added + st.counts.removed + st.counts.changed + st.counts.type_changed;
    let rep = Report {
        equal: total == 0,
        counts: st.counts,
        truncated: st.truncated,
        notes: st.notes,
        ignored_paths: st.ignored,
        changes: st.changes,
    };
    match opts.output {
        OutputFormat::Report => to_json(&rep, opts.indent),
        OutputFormat::Summary => Ok(render_summary(&rep)),
        OutputFormat::Patch => render_patch(&rep, &segs_of, opts.indent),
    }
}

// ---------------------------------------------------------------------------
// string-driven option parsing (shared by the page wrapper)
// ---------------------------------------------------------------------------

/// Lenient boolean parse — blank falls back to `default`.
pub fn parse_bool(raw: &str, default: bool) -> bool {
    match raw.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "on" | "yes" => true,
        _ => false,
    }
}

/// Parse an `array_match` choice.
pub fn parse_array_match(raw: &str) -> Result<ArrayMatch, String> {
    match raw.trim() {
        "" | "index" => Ok(ArrayMatch::Index),
        "key" => Ok(ArrayMatch::Key),
        "set" => Ok(ArrayMatch::Set),
        other => Err(format!(
            "array_match must be one of: index, key, set — got \"{other}\""
        )),
    }
}

/// Parse an `output` choice.
pub fn parse_output(raw: &str) -> Result<OutputFormat, String> {
    match raw.trim() {
        "" | "report" => Ok(OutputFormat::Report),
        "summary" => Ok(OutputFormat::Summary),
        "patch" => Ok(OutputFormat::Patch),
        other => Err(format!(
            "output must be one of: report, summary, patch — got \"{other}\""
        )),
    }
}

/// Build [`Options`] from raw strings — the page passes every field as a string.
#[allow(clippy::too_many_arguments)]
pub fn options_from_strings(
    ignore: &str,
    ignore_timestamps: &str,
    ignore_uuids: &str,
    array_match: &str,
    array_key: &str,
    numeric_tolerance: &str,
    ignore_case: &str,
    trim_strings: &str,
    null_equals_missing: &str,
    coerce_types: &str,
    output: &str,
    indent: &str,
) -> Result<Options, String> {
    let d = Options::default();
    let tol = match numeric_tolerance.trim() {
        "" => 0.0,
        s => s
            .parse::<f64>()
            .map_err(|_| format!("numeric_tolerance must be a number, got \"{s}\""))?,
    };
    let key = match array_key.trim() {
        "" => d.array_key.clone(),
        s => s.to_string(),
    };
    let ind = match indent.trim() {
        "" => 2,
        s => s
            .parse::<usize>()
            .map_err(|_| format!("indent must be a whole number 0-8, got \"{s}\""))?,
    };
    Ok(Options {
        ignore: ignore.to_string(),
        ignore_timestamps: parse_bool(ignore_timestamps, d.ignore_timestamps),
        ignore_uuids: parse_bool(ignore_uuids, d.ignore_uuids),
        array_match: parse_array_match(array_match)?,
        array_key: key,
        numeric_tolerance: tol,
        ignore_case: parse_bool(ignore_case, d.ignore_case),
        trim_strings: parse_bool(trim_strings, d.trim_strings),
        null_equals_missing: parse_bool(null_equals_missing, d.null_equals_missing),
        coerce_types: parse_bool(coerce_types, d.coerce_types),
        output: parse_output(output)?,
        indent: ind.min(8),
    })
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn rep(left: &str, right: &str, opts: &Options) -> Value {
        serde_json::from_str(&diff_responses(left, right, opts).unwrap()).unwrap()
    }

    #[test]
    fn reports_added_removed_and_changed() {
        let v = rep(
            r#"{"name":"alice","age":30,"legacy":true}"#,
            r#"{"name":"alice","age":31,"city":"NYC"}"#,
            &Options::default(),
        );
        assert_eq!(v["equal"], false);
        assert_eq!(v["counts"]["added"], 1);
        assert_eq!(v["counts"]["removed"], 1);
        assert_eq!(v["counts"]["changed"], 1);
        let changes = v["changes"].as_array().unwrap();
        assert!(changes.iter().any(|c| c["path"] == "$.age"
            && c["kind"] == "changed"
            && c["old"] == 30
            && c["new"] == 31));
        assert!(changes
            .iter()
            .any(|c| c["path"] == "$.legacy" && c["kind"] == "removed"));
        assert!(changes
            .iter()
            .any(|c| c["path"] == "$.city" && c["kind"] == "added"));
    }

    #[test]
    fn ignores_named_field_at_any_depth() {
        let opts = Options {
            ignore: "updatedAt, requestId".into(),
            ..Options::default()
        };
        let v = rep(
            r#"{"requestId":"a1","data":{"total":2,"updatedAt":"2026-08-01T00:00:00Z"}}"#,
            r#"{"requestId":"b2","data":{"total":2,"updatedAt":"2026-08-16T12:00:00Z"}}"#,
            &opts,
        );
        assert_eq!(v["equal"], true);
        assert_eq!(v["counts"]["ignored"], 2);
        let ignored = v["ignored_paths"].as_array().unwrap();
        assert!(ignored.iter().any(|p| p == "$.requestId"));
        assert!(ignored.iter().any(|p| p == "$.data.updatedAt"));
    }

    #[test]
    fn ignores_anchored_path_and_wildcards() {
        let opts = Options {
            ignore: "$.data.items[*].token\n*_at".into(),
            ..Options::default()
        };
        let v = rep(
            r#"{"data":{"items":[{"token":"x","created_at":1},{"token":"y"}]},"top":1}"#,
            r#"{"data":{"items":[{"token":"z","created_at":2},{"token":"w"}]},"top":1}"#,
            &opts,
        );
        assert_eq!(v["equal"], true);
        assert_eq!(v["counts"]["ignored"], 3);
    }

    #[test]
    fn ignore_timestamps_and_uuids_skip_volatile_values() {
        let opts = Options {
            ignore_timestamps: true,
            ignore_uuids: true,
            ..Options::default()
        };
        let v = rep(
            r#"{"id":"3f2504e0-4f89-11d3-9a0c-0305e82c3301","ts":"2026-08-01T10:00:00Z","epoch":1754000000,"n":1}"#,
            r#"{"id":"9c858901-8a57-4791-81fe-4c455b099bc9","ts":"2026-08-16T11:30:00Z","epoch":1755300000,"n":2}"#,
            &opts,
        );
        assert_eq!(v["counts"]["ignored"], 3);
        assert_eq!(v["counts"]["changed"], 1);
        assert_eq!(v["changes"][0]["path"], "$.n");
    }

    #[test]
    fn timestamp_filter_still_reports_shape_changes() {
        let opts = Options {
            ignore_timestamps: true,
            ..Options::default()
        };
        let v = rep(
            r#"{"ts":"2026-08-01T10:00:00Z"}"#,
            r#"{"ts":null}"#,
            &opts,
        );
        assert_eq!(v["counts"]["type_changed"], 1);
        assert_eq!(v["changes"][0]["kind"], "type_changed");
    }

    #[test]
    fn array_set_mode_ignores_order() {
        let json_a = r#"{"tags":["b","a","c"]}"#;
        let json_b = r#"{"tags":["a","c","b"]}"#;
        // index mode: every position differs
        let v = rep(json_a, json_b, &Options::default());
        assert_eq!(v["counts"]["changed"], 3);
        let opts = Options {
            array_match: ArrayMatch::Set,
            ..Options::default()
        };
        let v = rep(json_a, json_b, &opts);
        assert_eq!(v["equal"], true);
    }

    #[test]
    fn array_key_mode_pairs_by_id() {
        let opts = Options {
            array_match: ArrayMatch::Key,
            array_key: "id".into(),
            ..Options::default()
        };
        let v = rep(
            r#"{"items":[{"id":"a","qty":1},{"id":"b","qty":2}]}"#,
            r#"{"items":[{"id":"b","qty":5},{"id":"a","qty":1}]}"#,
            &opts,
        );
        assert_eq!(v["counts"]["changed"], 1);
        assert_eq!(v["changes"][0]["path"], "$.items[id=b].qty");
        assert_eq!(v["changes"][0]["old"], 2);
        assert_eq!(v["changes"][0]["new"], 5);
    }

    #[test]
    fn array_key_mode_falls_back_and_notes_it() {
        let opts = Options {
            array_match: ArrayMatch::Key,
            array_key: "id".into(),
            ..Options::default()
        };
        let v = rep(r#"{"items":[1,2]}"#, r#"{"items":[1,3]}"#, &opts);
        assert_eq!(v["counts"]["changed"], 1);
        assert!(v["notes"][0]
            .as_str()
            .unwrap()
            .contains("compared by index"));
    }

    #[test]
    fn numeric_tolerance_absorbs_small_drift() {
        let opts = Options {
            numeric_tolerance: 0.01,
            ..Options::default()
        };
        let v = rep(r#"{"price":10.0}"#, r#"{"price":10.005}"#, &opts);
        assert_eq!(v["equal"], true);
        let v = rep(r#"{"price":10.0}"#, r#"{"price":10.5}"#, &opts);
        assert_eq!(v["counts"]["changed"], 1);
    }

    #[test]
    fn string_and_type_leniency_flags() {
        let lenient = Options {
            ignore_case: true,
            trim_strings: true,
            null_equals_missing: true,
            coerce_types: true,
            ..Options::default()
        };
        let v = rep(
            r#"{"name":" Alice ","count":"5","gone":null}"#,
            r#"{"name":"alice","count":5}"#,
            &lenient,
        );
        assert_eq!(v["equal"], true);
        let v = rep(
            r#"{"name":" Alice ","count":"5","gone":null}"#,
            r#"{"name":"alice","count":5}"#,
            &Options::default(),
        );
        assert_eq!(v["counts"]["changed"], 1);
        assert_eq!(v["counts"]["type_changed"], 1);
        assert_eq!(v["counts"]["removed"], 1);
    }

    #[test]
    fn summary_output_is_readable_text() {
        let opts = Options {
            output: OutputFormat::Summary,
            ignore: "requestId".into(),
            ..Options::default()
        };
        let out = diff_responses(
            r#"{"requestId":"a","age":30,"legacy":1}"#,
            r#"{"requestId":"b","age":31,"city":"NYC"}"#,
            &opts,
        )
        .unwrap();
        assert_eq!(
            out,
            "3 differences (1 added, 1 removed, 1 changed, 0 type changed); 1 ignored\n\
             ~ $.age: 30 -> 31\n\
             - $.legacy: 1\n\
             + $.city: \"NYC\""
        );
    }

    #[test]
    fn summary_output_when_equal() {
        let opts = Options {
            output: OutputFormat::Summary,
            ignore: "ts".into(),
            ..Options::default()
        };
        let out = diff_responses(r#"{"ts":1,"a":2}"#, r#"{"ts":9,"a":2}"#, &opts).unwrap();
        assert_eq!(out, "No differences (1 ignored).");
    }

    #[test]
    fn patch_output_is_rfc6902() {
        let opts = Options {
            output: OutputFormat::Patch,
            indent: 0,
            ..Options::default()
        };
        let out = diff_responses(
            r#"{"a":1,"drop":true,"list":[1,2]}"#,
            r#"{"a":2,"list":[1,2,3],"new":"x"}"#,
            &opts,
        )
        .unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0], serde_json::json!({"op":"replace","path":"/a","value":2}));
        assert_eq!(v[1], serde_json::json!({"op":"remove","path":"/drop"}));
        assert_eq!(v[2], serde_json::json!({"op":"add","path":"/list/2","value":3}));
        assert_eq!(v[3], serde_json::json!({"op":"add","path":"/new","value":"x"}));
    }

    #[test]
    fn patch_pointer_escapes_special_characters() {
        let opts = Options {
            output: OutputFormat::Patch,
            indent: 0,
            ..Options::default()
        };
        let out = diff_responses(r#"{"a/b":1}"#, r#"{"a/b":2}"#, &opts).unwrap();
        assert!(out.contains("\"path\":\"/a~1b\""), "got {out}");
    }

    #[test]
    fn indent_zero_minifies() {
        let opts = Options {
            indent: 0,
            ..Options::default()
        };
        let out = diff_responses(r#"{"a":1}"#, r#"{"a":2}"#, &opts).unwrap();
        assert!(!out.contains('\n'));
    }

    #[test]
    fn err_on_invalid_left_json() {
        let err = diff_responses("{oops", "{}", &Options::default()).unwrap_err();
        assert!(err.starts_with("left response is not valid JSON:"), "got {err}");
    }

    #[test]
    fn err_on_invalid_right_json() {
        let err = diff_responses("{}", "[1,", &Options::default()).unwrap_err();
        assert!(err.starts_with("right response is not valid JSON:"), "got {err}");
    }

    #[test]
    fn err_on_bad_ignore_pattern() {
        let opts = Options {
            ignore: "items[abc]".into(),
            ..Options::default()
        };
        let err = diff_responses("{}", "{}", &opts).unwrap_err();
        assert!(err.contains("array index must be a number or '*'"), "got {err}");
    }

    #[test]
    fn err_on_patch_with_unordered_arrays() {
        let opts = Options {
            output: OutputFormat::Patch,
            array_match: ArrayMatch::Set,
            ..Options::default()
        };
        let err = diff_responses("{}", "{}", &opts).unwrap_err();
        assert!(err.contains("output=patch needs array_match=index"), "got {err}");
    }

    #[test]
    fn err_on_negative_tolerance() {
        let opts = Options {
            numeric_tolerance: -1.0,
            ..Options::default()
        };
        let err = diff_responses("{}", "{}", &opts).unwrap_err();
        assert!(err.contains("numeric_tolerance must be"), "got {err}");
    }

    #[test]
    fn err_on_oversized_input() {
        let big = format!("\"{}\"", "x".repeat(MAX_INPUT_BYTES));
        let err = diff_responses(&big, "{}", &Options::default()).unwrap_err();
        assert!(err.contains("over the 4 MiB limit"), "got {err}");
    }

    #[test]
    fn options_from_strings_matches_page_marshaling() {
        let o = options_from_strings(
            "requestId", "true", "false", "set", "", "0.5", "true", "true", "false", "true",
            "summary", "4",
        )
        .unwrap();
        assert!(o.ignore_timestamps);
        assert!(!o.ignore_uuids);
        assert_eq!(o.array_match, ArrayMatch::Set);
        assert_eq!(o.array_key, "id");
        assert_eq!(o.numeric_tolerance, 0.5);
        assert_eq!(o.output, OutputFormat::Summary);
        assert_eq!(o.indent, 4);
        // blank strings keep the descriptor defaults
        let d = options_from_strings("", "", "", "", "", "", "", "", "", "", "", "").unwrap();
        assert_eq!(d.array_match, ArrayMatch::Index);
        assert_eq!(d.output, OutputFormat::Report);
        assert_eq!(d.indent, 2);
        assert!(!d.ignore_case);
    }

    #[test]
    fn err_on_unknown_choice_strings() {
        assert!(parse_array_match("shuffle")
            .unwrap_err()
            .contains("index, key, set"));
        assert!(parse_output("html").unwrap_err().contains("report, summary, patch"));
    }

    #[test]
    fn truncates_giant_change_lists() {
        let left = Value::Object(
            (0..MAX_CHANGES + 50)
                .map(|i| (format!("k{i}"), Value::from(i)))
                .collect::<Map<String, Value>>(),
        );
        let right = Value::Object(
            (0..MAX_CHANGES + 50)
                .map(|i| (format!("k{i}"), Value::from(i + 1)))
                .collect::<Map<String, Value>>(),
        );
        let v = rep(
            &serde_json::to_string(&left).unwrap(),
            &serde_json::to_string(&right).unwrap(),
            &Options::default(),
        );
        assert_eq!(v["truncated"], true);
        assert_eq!(v["changes"].as_array().unwrap().len(), MAX_CHANGES);
        assert_eq!(v["counts"]["changed"], MAX_CHANGES + 50);
    }

    #[test]
    fn glob_and_timestamp_helpers() {
        assert!(glob_match("*_at", "created_at"));
        assert!(glob_match("x*y", "xaby"));
        assert!(!glob_match("*_at", "created"));
        assert!(looks_like_timestamp(&Value::String("2026-08-16".into())));
        assert!(looks_like_timestamp(&Value::String(
            "2026-08-16T12:00:00.123Z".into()
        )));
        assert!(!looks_like_timestamp(&Value::String("hello".into())));
        assert!(!looks_like_timestamp(&Value::from(42)));
        assert!(looks_like_uuid(&Value::String(
            "3f2504e0-4f89-11d3-9a0c-0305e82c3301".into()
        )));
        assert!(!looks_like_uuid(&Value::String("3f2504e0".into())));
    }
}

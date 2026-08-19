//! sigma-rule-matcher core — evaluate Sigma detection rules against already-parsed
//! JSON event records. Pure compute, shared by the chat skill block and the web page
//! (no wafer/wasm-bindgen deps, no I/O, no clock).
//!
//! What it implements of the Sigma rule format:
//! * multi-document rule files (`---` separated), one rule per document;
//! * metadata (`title`, `id`, `status`, `level`, `description`, `author`, `tags`,
//!   `logsource`) parsed and surfaced in the output;
//! * `detection:` selections as field maps (AND of fields), lists of maps (OR of
//!   groups), and keyword lists (substring search over every scalar in the event);
//! * a list of values under one field is an OR by default, an AND with `|all`;
//! * `null` matches "field absent or JSON null";
//! * `*` / `?` wildcards with `\*`, `\?`, `\\` escapes, matched case-insensitively
//!   unless `|cased` is used;
//! * field modifiers `contains`, `startswith`, `endswith`, `re` (+ `i`/`m`/`s`),
//!   `cased`, `all`, `exists`, `fieldref`, `gt`/`gte`/`lt`/`lte`, `cidr`, `base64`,
//!   `base64offset`, `utf16`/`utf16le`/`utf16be`/`wide`, `windash`;
//! * the `condition` grammar: selection names, `and` / `or` / `not`, parentheses,
//!   `1 of x` / `any of x` / `all of x` / `N of x`, `them`, and `prefix*` patterns.
//!
//! A rule that uses something outside that subset (an aggregation `| count() by …`
//! condition, a `correlation:` document, an unknown modifier) is SKIPPED with a
//! stated reason rather than silently mis-evaluated.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use regex::RegexBuilder;
use serde::Deserialize;
use serde_json::{Map as JMap, Value as J};
use serde_yml::Value as Y;
use std::collections::BTreeMap;
use std::net::IpAddr;

/// Most rule-YAML bytes a single run accepts (1 MiB).
pub const MAX_RULES_BYTES: usize = 1024 * 1024;
/// Most event-JSON bytes a single run accepts (8 MiB).
pub const MAX_EVENTS_BYTES: usize = 8 * 1024 * 1024;
/// Most rule documents a single run accepts.
pub const MAX_RULES: usize = 500;
/// Most event records a single run accepts.
pub const MAX_EVENTS: usize = 50_000;
/// Upper bound for the `max_matches` cap.
pub const MAX_MATCHES_CAP: u32 = 10_000;
/// `max_matches` when the caller leaves it unset (0).
pub const DEFAULT_MAX_MATCHES: u32 = 500;

// ---------------------------------------------------------------------------
// Severity
// ---------------------------------------------------------------------------

/// Sigma's `level` field, ordered lowest to highest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Informational,
    Low,
    Medium,
    High,
    Critical,
}

impl Level {
    fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "informational" | "info" => Some(Level::Informational),
            "low" => Some(Level::Low),
            "medium" => Some(Level::Medium),
            "high" => Some(Level::High),
            "critical" => Some(Level::Critical),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Level::Informational => "informational",
            Level::Low => "low",
            Level::Medium => "medium",
            Level::High => "high",
            Level::Critical => "critical",
        }
    }
}

/// Every level, highest first — the order hit summaries are printed in.
const LEVELS_DESC: [Level; 5] = [
    Level::Critical,
    Level::High,
    Level::Medium,
    Level::Low,
    Level::Informational,
];

const STATUSES: [&str; 5] = [
    "stable",
    "test",
    "experimental",
    "deprecated",
    "unsupported",
];

// ---------------------------------------------------------------------------
// Field modifiers
// ---------------------------------------------------------------------------

/// The comparison a field matcher performs. At most one per field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Equals,
    Contains,
    StartsWith,
    EndsWith,
    Re,
    Cidr,
    Gt,
    Gte,
    Lt,
    Lte,
    Exists,
}

/// A value transform applied to the RULE value before comparing, in the order
/// the modifiers were written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Transform {
    Windash,
    Utf16,
    Utf16Le,
    Utf16Be,
    Base64,
    Base64Offset,
}

#[derive(Debug, Clone)]
struct Mods {
    op: Op,
    op_set: bool,
    transforms: Vec<Transform>,
    all: bool,
    cased: bool,
    fieldref: bool,
    re_i: bool,
    re_m: bool,
    re_s: bool,
}

impl Default for Mods {
    fn default() -> Self {
        Mods {
            op: Op::Equals,
            op_set: false,
            transforms: Vec::new(),
            all: false,
            cased: false,
            fieldref: false,
            re_i: false,
            re_m: false,
            re_s: false,
        }
    }
}

/// Split `"CommandLine|base64offset|contains"` into the field name and its modifiers.
fn parse_mods(key: &str) -> Result<(String, Mods), String> {
    let mut parts = key.split('|');
    let field = parts.next().unwrap_or("").trim().to_string();
    if field.is_empty() {
        return Err(format!("field name is empty in '{key}'"));
    }
    let mut m = Mods::default();
    for raw in parts {
        let name = raw.trim().to_ascii_lowercase();
        let set_op = |op: Op, m: &mut Mods| -> Result<(), String> {
            if m.op_set {
                return Err(format!(
                    "field '{field}' combines two comparison modifiers ('{name}' after another)"
                ));
            }
            m.op = op;
            m.op_set = true;
            Ok(())
        };
        match name.as_str() {
            "contains" => set_op(Op::Contains, &mut m)?,
            "startswith" => set_op(Op::StartsWith, &mut m)?,
            "endswith" => set_op(Op::EndsWith, &mut m)?,
            "re" => set_op(Op::Re, &mut m)?,
            "cidr" => set_op(Op::Cidr, &mut m)?,
            "gt" => set_op(Op::Gt, &mut m)?,
            "gte" => set_op(Op::Gte, &mut m)?,
            "lt" => set_op(Op::Lt, &mut m)?,
            "lte" => set_op(Op::Lte, &mut m)?,
            "exists" => set_op(Op::Exists, &mut m)?,
            "all" => m.all = true,
            "cased" => m.cased = true,
            "fieldref" => m.fieldref = true,
            "windash" => m.transforms.push(Transform::Windash),
            "utf16" => m.transforms.push(Transform::Utf16),
            "utf16le" | "wide" => m.transforms.push(Transform::Utf16Le),
            "utf16be" => m.transforms.push(Transform::Utf16Be),
            "base64" => m.transforms.push(Transform::Base64),
            "base64offset" => m.transforms.push(Transform::Base64Offset),
            "i" if m.op == Op::Re => m.re_i = true,
            "m" if m.op == Op::Re => m.re_m = true,
            "s" if m.op == Op::Re => m.re_s = true,
            other => return Err(format!("unsupported modifier '{other}' on field '{field}'")),
        }
    }
    // utf16* only produces meaningful text once it is base64-encoded.
    let has_utf16 = m.transforms.iter().any(|t| {
        matches!(
            t,
            Transform::Utf16 | Transform::Utf16Le | Transform::Utf16Be
        )
    });
    let has_b64 = m
        .transforms
        .iter()
        .any(|t| matches!(t, Transform::Base64 | Transform::Base64Offset));
    if has_utf16 && !has_b64 {
        return Err(format!(
            "field '{field}' uses a utf16/wide modifier without base64 or base64offset"
        ));
    }
    Ok((field, m))
}

/// Apply the rule-value transform chain, producing every alternative encoding.
fn apply_transforms(value: &str, transforms: &[Transform]) -> Vec<String> {
    let mut cur: Vec<Vec<u8>> = vec![value.as_bytes().to_vec()];
    for t in transforms {
        let mut next: Vec<Vec<u8>> = Vec::new();
        for b in &cur {
            match t {
                Transform::Windash => next.extend(windash_variants(b)),
                Transform::Utf16Le => next.push(to_utf16(b, false, false)),
                Transform::Utf16Be => next.push(to_utf16(b, true, false)),
                Transform::Utf16 => next.push(to_utf16(b, false, true)),
                Transform::Base64 => next.push(B64.encode(b).into_bytes()),
                Transform::Base64Offset => {
                    next.extend(base64_offsets(b).into_iter().map(|s| s.into_bytes()))
                }
            }
        }
        cur = next;
    }
    let mut out: Vec<String> = Vec::new();
    for b in cur {
        let s = String::from_utf8_lossy(&b).into_owned();
        if !out.contains(&s) {
            out.push(s);
        }
    }
    out
}

/// UTF-16 encode, optionally big-endian, optionally with a byte-order mark.
fn to_utf16(bytes: &[u8], big_endian: bool, bom: bool) -> Vec<u8> {
    let text = String::from_utf8_lossy(bytes);
    let mut out = Vec::with_capacity(text.len() * 2 + 2);
    if bom {
        out.extend_from_slice(&[0xff, 0xfe]);
    }
    for unit in text.encode_utf16() {
        if big_endian {
            out.extend_from_slice(&unit.to_be_bytes());
        } else {
            out.extend_from_slice(&unit.to_le_bytes());
        }
    }
    out
}

/// The three base64 fragments a value can appear as when it starts at an
/// arbitrary byte offset inside a longer base64 stream.
fn base64_offsets(bytes: &[u8]) -> Vec<String> {
    const START: [usize; 3] = [0, 2, 3];
    const TRIM: [usize; 3] = [0, 3, 2];
    let mut out = Vec::with_capacity(3);
    for i in 0..3usize {
        let mut padded = vec![b' '; i];
        padded.extend_from_slice(bytes);
        let encoded = B64.encode(&padded);
        let trim = TRIM[(bytes.len() + i) % 3];
        let end = encoded.len().saturating_sub(trim);
        let start = START[i].min(end);
        let frag = encoded[start..end].to_string();
        if !frag.is_empty() && !out.contains(&frag) {
            out.push(frag);
        }
    }
    out
}

/// Windows command-line dash variants. Only dashes/slashes that are NOT preceded
/// by a word character are treated as option leaders, so `C:/path/x-y` is left
/// alone while `-param` expands. Every slot in one variant uses the same
/// character.
fn windash_variants(bytes: &[u8]) -> Vec<Vec<u8>> {
    const DASHES: [char; 5] = ['-', '/', '\u{2013}', '\u{2014}', '\u{2015}'];
    let text = String::from_utf8_lossy(bytes).into_owned();
    let chars: Vec<char> = text.chars().collect();
    let mut slots: Vec<usize> = Vec::new();
    for (i, c) in chars.iter().enumerate() {
        if DASHES.contains(c) {
            let prev_word = i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_');
            if !prev_word {
                slots.push(i);
            }
        }
    }
    if slots.is_empty() {
        return vec![bytes.to_vec()];
    }
    let mut out: Vec<Vec<u8>> = Vec::new();
    for d in DASHES {
        let mut variant = chars.clone();
        for &i in &slots {
            variant[i] = d;
        }
        let s: String = variant.into_iter().collect();
        let b = s.into_bytes();
        if !out.contains(&b) {
            out.push(b);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Wildcard matching
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
enum PatTok {
    Star,
    Any,
    Lit(char),
}

/// Tokenize a Sigma value into wildcard tokens, honouring `\*`, `\?` and `\\`.
fn tokenize_pattern(pat: &str) -> Vec<PatTok> {
    let mut out = Vec::new();
    let mut it = pat.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '\\' => match it.peek() {
                Some('*') => {
                    it.next();
                    out.push(PatTok::Lit('*'));
                }
                Some('?') => {
                    it.next();
                    out.push(PatTok::Lit('?'));
                }
                Some('\\') => {
                    it.next();
                    out.push(PatTok::Lit('\\'));
                }
                _ => out.push(PatTok::Lit('\\')),
            },
            '*' => out.push(PatTok::Star),
            '?' => out.push(PatTok::Any),
            other => out.push(PatTok::Lit(other)),
        }
    }
    out
}

/// Classic backtracking wildcard match of `toks` against `text`.
fn glob_match(toks: &[PatTok], text: &[char]) -> bool {
    let (mut ti, mut si) = (0usize, 0usize);
    let (mut star, mut star_s) = (usize::MAX, 0usize);
    while si < text.len() {
        match toks.get(ti) {
            Some(PatTok::Lit(c)) if *c == text[si] => {
                ti += 1;
                si += 1;
            }
            Some(PatTok::Any) => {
                ti += 1;
                si += 1;
            }
            Some(PatTok::Star) => {
                star = ti;
                star_s = si;
                ti += 1;
            }
            _ => {
                if star == usize::MAX {
                    return false;
                }
                ti = star + 1;
                star_s += 1;
                si = star_s;
            }
        }
    }
    while matches!(toks.get(ti), Some(PatTok::Star)) {
        ti += 1;
    }
    ti == toks.len()
}

fn fold(s: &str, cased: bool) -> String {
    if cased {
        s.to_string()
    } else {
        s.to_lowercase()
    }
}

/// Match one event string against one rule value under `op`.
fn text_match(op: Op, cand: &str, pattern: &str, cased: bool, literal: bool) -> bool {
    let c = fold(cand, cased);
    let p = fold(pattern, cased);
    if literal {
        return match op {
            Op::Contains => c.contains(&p),
            Op::StartsWith => c.starts_with(&p),
            Op::EndsWith => c.ends_with(&p),
            _ => c == p,
        };
    }
    let mut toks = tokenize_pattern(&p);
    match op {
        Op::Contains => {
            toks.insert(0, PatTok::Star);
            toks.push(PatTok::Star);
        }
        Op::StartsWith => toks.push(PatTok::Star),
        Op::EndsWith => toks.insert(0, PatTok::Star),
        _ => {}
    }
    let chars: Vec<char> = c.chars().collect();
    glob_match(&toks, &chars)
}

// ---------------------------------------------------------------------------
// Rule model
// ---------------------------------------------------------------------------

/// One compiled rule value: what the field is compared against.
#[derive(Debug)]
enum RuleVal {
    /// YAML `null` — matches when the field is absent or JSON null.
    Null,
    /// Text alternatives (after the transform chain); wildcard-matched.
    Text(Vec<String>),
    Regex(regex::Regex),
    Cidr(IpAddr, u32),
    Num(f64),
    Exists(bool),
}

#[derive(Debug)]
struct FieldMatcher {
    field: String,
    mods: Mods,
    values: Vec<RuleVal>,
}

#[derive(Debug)]
enum Alternative {
    /// A map of fields — every field must match.
    Fields(Vec<FieldMatcher>),
    /// A bare keyword — matched as a substring of any scalar in the event.
    Keyword(Vec<String>),
}

#[derive(Debug)]
struct Selection {
    name: String,
    /// OR over the alternatives.
    alternatives: Vec<Alternative>,
}

#[derive(Debug)]
enum Cond {
    Sel(String),
    /// At least N of the listed selections.
    AtLeast(usize, Vec<String>),
    AllOf(Vec<String>),
    And(Box<Cond>, Box<Cond>),
    Or(Box<Cond>, Box<Cond>),
    Not(Box<Cond>),
}

#[derive(Debug)]
struct Rule {
    title: String,
    id: String,
    status: String,
    level: Option<Level>,
    description: String,
    author: String,
    tags: Vec<String>,
    logsource: BTreeMap<String, String>,
    selections: Vec<Selection>,
    condition: Cond,
}

/// A rule document that could not be used, and why.
#[derive(Debug, Clone)]
pub struct SkippedRule {
    pub title: String,
    pub reason: String,
}

// ---------------------------------------------------------------------------
// Rule parsing
// ---------------------------------------------------------------------------

fn y_scalar_text(v: &Y) -> Option<String> {
    match v {
        Y::String(s) => Some(s.clone()),
        Y::Number(n) => Some(n.to_string()),
        Y::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn y_str(map: &serde_yml::Mapping, key: &str) -> String {
    map.get(Y::String(key.to_string()))
        .and_then(y_scalar_text)
        .unwrap_or_default()
}

fn y_list(map: &serde_yml::Mapping, key: &str) -> Vec<String> {
    match map.get(Y::String(key.to_string())) {
        Some(Y::Sequence(s)) => s.iter().filter_map(y_scalar_text).collect(),
        Some(other) => y_scalar_text(other).into_iter().collect(),
        None => Vec::new(),
    }
}

/// Compile one `field|mods: value(s)` entry.
fn compile_field(key: &str, val: &Y) -> Result<FieldMatcher, String> {
    let (field, mods) = parse_mods(key)?;
    let raw: Vec<&Y> = match val {
        Y::Sequence(s) => s.iter().collect(),
        other => vec![other],
    };
    if raw.is_empty() {
        return Err(format!("field '{field}' has an empty value list"));
    }
    let mut values = Vec::with_capacity(raw.len());
    for v in raw {
        values.push(compile_value(&field, &mods, v)?);
    }
    Ok(FieldMatcher {
        field,
        mods,
        values,
    })
}

fn compile_value(field: &str, mods: &Mods, v: &Y) -> Result<RuleVal, String> {
    if matches!(v, Y::Null) {
        if mods.op == Op::Exists {
            return Err(format!("field '{field}' uses |exists with a null value"));
        }
        return Ok(RuleVal::Null);
    }
    let text = y_scalar_text(v).ok_or_else(|| {
        format!("field '{field}' has a nested map/list value, which Sigma does not define")
    })?;
    match mods.op {
        Op::Exists => {
            let b = match text.trim().to_ascii_lowercase().as_str() {
                "true" | "yes" | "1" => true,
                "false" | "no" | "0" => false,
                other => {
                    return Err(format!(
                        "field '{field}' |exists wants true/false, got '{other}'"
                    ))
                }
            };
            Ok(RuleVal::Exists(b))
        }
        Op::Re => {
            if mods.fieldref {
                return Err(format!("field '{field}' combines |re with |fieldref"));
            }
            let re = RegexBuilder::new(&text)
                .case_insensitive(mods.re_i)
                .multi_line(mods.re_m)
                .dot_matches_new_line(mods.re_s)
                .size_limit(1 << 20)
                .build()
                .map_err(|e| format!("field '{field}' has an invalid regex: {e}"))?;
            Ok(RuleVal::Regex(re))
        }
        Op::Cidr => {
            let (net, bits) = parse_cidr(&text)
                .ok_or_else(|| format!("field '{field}' has an invalid CIDR '{text}'"))?;
            Ok(RuleVal::Cidr(net, bits))
        }
        Op::Gt | Op::Gte | Op::Lt | Op::Lte => {
            let n = text
                .trim()
                .parse::<f64>()
                .map_err(|_| format!("field '{field}' compares against a non-number '{text}'"))?;
            Ok(RuleVal::Num(n))
        }
        _ => {
            if mods.fieldref {
                return Ok(RuleVal::Text(vec![text]));
            }
            Ok(RuleVal::Text(apply_transforms(&text, &mods.transforms)))
        }
    }
}

fn parse_cidr(text: &str) -> Option<(IpAddr, u32)> {
    let (addr, bits) = match text.split_once('/') {
        Some((a, b)) => (a, b.parse::<u32>().ok()?),
        None => (text, u32::MAX),
    };
    let ip: IpAddr = addr.trim().parse().ok()?;
    let max = if ip.is_ipv4() { 32 } else { 128 };
    let bits = if bits == u32::MAX { max } else { bits };
    if bits > max {
        return None;
    }
    Some((ip, bits))
}

fn compile_selection(name: &str, val: &Y) -> Result<Selection, String> {
    let mut alternatives = Vec::new();
    match val {
        Y::Mapping(map) => {
            alternatives.push(Alternative::Fields(compile_field_map(map)?));
        }
        Y::Sequence(items) => {
            for item in items {
                match item {
                    Y::Mapping(map) => {
                        alternatives.push(Alternative::Fields(compile_field_map(map)?));
                    }
                    other => {
                        let text = y_scalar_text(other).ok_or_else(|| {
                            format!("selection '{name}' has a value Sigma does not define")
                        })?;
                        alternatives.push(Alternative::Keyword(vec![text]));
                    }
                }
            }
        }
        other => {
            let text = y_scalar_text(other)
                .ok_or_else(|| format!("selection '{name}' has an empty definition"))?;
            alternatives.push(Alternative::Keyword(vec![text]));
        }
    }
    if alternatives.is_empty() {
        return Err(format!("selection '{name}' is empty"));
    }
    Ok(Selection {
        name: name.to_string(),
        alternatives,
    })
}

fn compile_field_map(map: &serde_yml::Mapping) -> Result<Vec<FieldMatcher>, String> {
    let mut out = Vec::new();
    for (k, v) in map {
        let key = y_scalar_text(k).ok_or_else(|| "a detection key is not a string".to_string())?;
        out.push(compile_field(&key, v)?);
    }
    if out.is_empty() {
        return Err("a detection map has no fields".to_string());
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Condition grammar
// ---------------------------------------------------------------------------

fn tokenize_condition(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in s.chars() {
        match c {
            '(' | ')' => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
                out.push(c.to_string());
            }
            c if c.is_whitespace() => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

struct CondParser<'a> {
    toks: Vec<String>,
    pos: usize,
    names: &'a [String],
}

impl<'a> CondParser<'a> {
    fn peek(&self) -> Option<&str> {
        self.toks.get(self.pos).map(|s| s.as_str())
    }

    fn peek_lower(&self) -> Option<String> {
        self.peek().map(|s| s.to_ascii_lowercase())
    }

    fn next_tok(&mut self) -> Option<String> {
        let t = self.toks.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn parse_or(&mut self) -> Result<Cond, String> {
        let mut lhs = self.parse_and()?;
        while self.peek_lower().as_deref() == Some("or") {
            self.pos += 1;
            let rhs = self.parse_and()?;
            lhs = Cond::Or(Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_and(&mut self) -> Result<Cond, String> {
        let mut lhs = self.parse_not()?;
        while self.peek_lower().as_deref() == Some("and") {
            self.pos += 1;
            let rhs = self.parse_not()?;
            lhs = Cond::And(Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_not(&mut self) -> Result<Cond, String> {
        if self.peek_lower().as_deref() == Some("not") {
            self.pos += 1;
            return Ok(Cond::Not(Box::new(self.parse_not()?)));
        }
        self.parse_atom()
    }

    fn parse_atom(&mut self) -> Result<Cond, String> {
        let tok = self
            .next_tok()
            .ok_or_else(|| "condition ends unexpectedly".to_string())?;
        if tok == "(" {
            let inner = self.parse_or()?;
            match self.next_tok().as_deref() {
                Some(")") => return Ok(inner),
                _ => return Err("condition has an unclosed '('".to_string()),
            }
        }
        if tok == ")" {
            return Err("condition has an unmatched ')'".to_string());
        }
        let lower = tok.to_ascii_lowercase();
        // Quantifier: `all of x`, `1 of x`, `any of x`, `N of x`.
        if self.peek_lower().as_deref() == Some("of") {
            self.pos += 1;
            let pattern = self
                .next_tok()
                .ok_or_else(|| format!("'{tok} of' is missing what it applies to"))?;
            let names = self.expand(&pattern)?;
            return match lower.as_str() {
                "all" => Ok(Cond::AllOf(names)),
                "any" | "1" => Ok(Cond::AtLeast(1, names)),
                n => match n.parse::<usize>() {
                    Ok(0) => Err("condition uses '0 of …', which never matches".to_string()),
                    Ok(k) => Ok(Cond::AtLeast(k, names)),
                    Err(_) => Err(format!("condition has an unknown quantifier '{tok}'")),
                },
            };
        }
        if lower == "them" {
            return Err("condition uses 'them' without a quantifier".to_string());
        }
        let names = self.expand(&tok)?;
        if names.len() == 1 {
            Ok(Cond::Sel(names.into_iter().next().unwrap()))
        } else {
            Ok(Cond::AtLeast(1, names))
        }
    }

    /// Resolve `them`, a `prefix*` pattern, or a plain selection name.
    fn expand(&self, pattern: &str) -> Result<Vec<String>, String> {
        if pattern.eq_ignore_ascii_case("them") {
            return Ok(self.names.to_vec());
        }
        if pattern.contains('*') || pattern.contains('?') {
            let toks = tokenize_pattern(&pattern.to_lowercase());
            let hits: Vec<String> = self
                .names
                .iter()
                .filter(|n| glob_match(&toks, &n.to_lowercase().chars().collect::<Vec<_>>()))
                .cloned()
                .collect();
            if hits.is_empty() {
                return Err(format!(
                    "condition pattern '{pattern}' matches no selection in this rule"
                ));
            }
            return Ok(hits);
        }
        if self.names.iter().any(|n| n == pattern) {
            return Ok(vec![pattern.to_string()]);
        }
        Err(format!(
            "condition references '{pattern}', which is not a selection in this rule"
        ))
    }
}

fn parse_condition(src: &str, names: &[String]) -> Result<Cond, String> {
    if src.trim().is_empty() {
        return Err("the rule has an empty condition".to_string());
    }
    if src.contains('|') {
        return Err(
            "aggregation conditions (the '|' pipe, e.g. '| count() by …') are not supported"
                .to_string(),
        );
    }
    if src
        .split_whitespace()
        .any(|w| w.eq_ignore_ascii_case("near"))
    {
        return Err("'near' correlation conditions are not supported".to_string());
    }
    let toks = tokenize_condition(src);
    let mut p = CondParser {
        toks,
        pos: 0,
        names,
    };
    let cond = p.parse_or()?;
    if p.pos != p.toks.len() {
        return Err(format!(
            "condition has trailing text at '{}'",
            p.toks[p.pos]
        ));
    }
    Ok(cond)
}

// ---------------------------------------------------------------------------
// Whole-rule parsing
// ---------------------------------------------------------------------------

fn parse_rules(text: &str) -> Result<(Vec<Rule>, Vec<SkippedRule>), String> {
    let mut rules = Vec::new();
    let mut skipped = Vec::new();
    let mut index = 0usize;
    for doc in serde_yml::Deserializer::from_str(text) {
        index += 1;
        if rules.len() + skipped.len() >= MAX_RULES {
            return Err(format!(
                "too many rule documents — the cap is {MAX_RULES} per run"
            ));
        }
        let value = match Y::deserialize(doc) {
            Ok(v) => v,
            Err(e) => {
                skipped.push(SkippedRule {
                    title: format!("document {index}"),
                    reason: format!("YAML parse error: {e}"),
                });
                continue;
            }
        };
        match parse_rule(&value) {
            Ok(Some(r)) => rules.push(r),
            Ok(None) => {
                index -= 1; // an empty document is not a rule at all
            }
            Err((title, reason)) => skipped.push(SkippedRule {
                title: if title.is_empty() {
                    format!("document {index}")
                } else {
                    title
                },
                reason,
            }),
        }
    }
    if rules.is_empty() && skipped.is_empty() {
        return Err("no Sigma rule documents found in the rules input".to_string());
    }
    Ok((rules, skipped))
}

type RuleErr = (String, String);

fn parse_rule(value: &Y) -> Result<Option<Rule>, RuleErr> {
    let map = match value {
        Y::Mapping(m) => m,
        Y::Null => return Ok(None),
        _ => return Err((String::new(), "document is not a YAML mapping".to_string())),
    };
    if map.is_empty() {
        return Ok(None);
    }
    let title = {
        let t = y_str(map, "title");
        if t.is_empty() {
            "(untitled rule)".to_string()
        } else {
            t
        }
    };
    let fail = |reason: String| -> RuleErr { (title.clone(), reason) };

    if map.contains_key(Y::String("correlation".into())) {
        return Err(fail(
            "Sigma correlation documents are not supported — only detection rules".to_string(),
        ));
    }
    let detection = map
        .get(Y::String("detection".into()))
        .ok_or_else(|| fail("document has no 'detection' block".to_string()))?;
    let detection = match detection {
        Y::Mapping(m) => m,
        _ => return Err(fail("'detection' is not a mapping".to_string())),
    };

    let mut selections = Vec::new();
    let mut condition_src: Option<String> = None;
    for (k, v) in detection {
        let key = y_scalar_text(k)
            .ok_or_else(|| fail("a 'detection' key is not a string".to_string()))?;
        match key.as_str() {
            "condition" => {
                let src = match v {
                    Y::Sequence(items) => {
                        let parts: Vec<String> = items
                            .iter()
                            .filter_map(y_scalar_text)
                            .map(|s| format!("({s})"))
                            .collect();
                        parts.join(" or ")
                    }
                    other => y_scalar_text(other).unwrap_or_default(),
                };
                condition_src = Some(src);
            }
            // `timeframe` only has meaning for aggregations, which are skipped anyway.
            "timeframe" => {}
            _ => selections.push(compile_selection(&key, v).map_err(fail)?),
        }
    }
    let condition_src =
        condition_src.ok_or_else(|| fail("'detection' has no 'condition'".to_string()))?;
    if selections.is_empty() {
        return Err(fail("'detection' has no selections".to_string()));
    }
    let names: Vec<String> = selections.iter().map(|s| s.name.clone()).collect();
    let condition = parse_condition(&condition_src, &names).map_err(fail)?;

    let level_raw = y_str(map, "level");
    let mut logsource = BTreeMap::new();
    if let Some(Y::Mapping(ls)) = map.get(Y::String("logsource".into())) {
        for (k, v) in ls {
            if let (Some(k), Some(v)) = (y_scalar_text(k), y_scalar_text(v)) {
                logsource.insert(k, v);
            }
        }
    }

    Ok(Some(Rule {
        title,
        id: y_str(map, "id"),
        status: y_str(map, "status").to_ascii_lowercase(),
        level: Level::parse(&level_raw),
        description: y_str(map, "description"),
        author: y_str(map, "author"),
        tags: y_list(map, "tags"),
        logsource,
        selections,
        condition,
    }))
}

// ---------------------------------------------------------------------------
// Event parsing + field resolution
// ---------------------------------------------------------------------------

fn parse_events(text: &str) -> Result<Vec<J>, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(
            "no events provided — paste a JSON array or newline-delimited JSON".to_string(),
        );
    }
    if trimmed.starts_with('[') {
        let v: J = serde_json::from_str(trimmed)
            .map_err(|e| format!("the events are not valid JSON: {e}"))?;
        let arr = v
            .as_array()
            .ok_or_else(|| "the events JSON is not an array".to_string())?;
        return Ok(arr.clone());
    }
    // A single pretty-printed object is a common paste; try it before NDJSON.
    if trimmed.starts_with('{') {
        if let Ok(v) = serde_json::from_str::<J>(trimmed) {
            if v.is_object() {
                return Ok(vec![v]);
            }
        }
    }
    let mut out = Vec::new();
    for (i, line) in trimmed.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let v: J = serde_json::from_str(line).map_err(|e| {
            format!(
                "line {} is not valid JSON: {e} — events must be a JSON array or one JSON object per line",
                i + 1
            )
        })?;
        out.push(v);
    }
    if out.is_empty() {
        return Err(
            "no events provided — paste a JSON array or newline-delimited JSON".to_string(),
        );
    }
    Ok(out)
}

fn get_ci<'a>(obj: &'a JMap<String, J>, key: &str) -> Option<&'a J> {
    if let Some(v) = obj.get(key) {
        return Some(v);
    }
    obj.iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .map(|(_, v)| v)
}

fn walk_path<'a>(obj: &'a JMap<String, J>, path: &str) -> Option<&'a J> {
    let mut cur: &J = get_ci(obj, path.split('.').next()?)?;
    for seg in path.split('.').skip(1) {
        cur = get_ci(cur.as_object()?, seg)?;
    }
    Some(cur)
}

/// Containers real EVTX-shaped JSON nests its fields under.
const CONTAINERS: [&str; 6] = ["Event", "System", "EventData", "UserData", "winlog", "data"];

/// Look a Sigma field name up in an event: exact key, case-insensitive key, a
/// dotted path, then the same search inside the well-known nesting containers.
fn resolve_field<'a>(ev: &'a J, name: &str) -> Option<&'a J> {
    resolve_in(ev, name, 0)
}

fn resolve_in<'a>(v: &'a J, name: &str, depth: u32) -> Option<&'a J> {
    let obj = v.as_object()?;
    if let Some(x) = get_ci(obj, name) {
        return Some(x);
    }
    if name.contains('.') {
        if let Some(x) = walk_path(obj, name) {
            return Some(x);
        }
    }
    if depth >= 3 {
        return None;
    }
    for c in CONTAINERS {
        if let Some(sub) = get_ci(obj, c) {
            if let Some(x) = resolve_in(sub, name, depth + 1) {
                return Some(x);
            }
        }
    }
    None
}

/// Every comparable string a JSON value contributes.
fn value_strings(v: &J) -> Vec<String> {
    match v {
        J::Null => Vec::new(),
        J::String(s) => vec![s.clone()],
        J::Number(n) => vec![n.to_string()],
        J::Bool(b) => vec![b.to_string()],
        J::Array(a) => a.iter().flat_map(value_strings).collect(),
        J::Object(_) => vec![v.to_string()],
    }
}

/// Every scalar string anywhere in the event — what a keyword list searches.
fn all_scalars(v: &J, out: &mut Vec<String>) {
    match v {
        J::String(s) => out.push(s.clone()),
        J::Number(n) => out.push(n.to_string()),
        J::Bool(b) => out.push(b.to_string()),
        J::Array(a) => a.iter().for_each(|x| all_scalars(x, out)),
        J::Object(m) => m.values().for_each(|x| all_scalars(x, out)),
        J::Null => {}
    }
}

// ---------------------------------------------------------------------------
// Evaluation
// ---------------------------------------------------------------------------

fn ip_in_net(ip: &IpAddr, net: &IpAddr, bits: u32) -> bool {
    match (ip, net) {
        (IpAddr::V4(a), IpAddr::V4(n)) => {
            if bits == 0 {
                return true;
            }
            let mask = u32::MAX << (32 - bits);
            (u32::from(*a) & mask) == (u32::from(*n) & mask)
        }
        (IpAddr::V6(a), IpAddr::V6(n)) => {
            if bits == 0 {
                return true;
            }
            let mask = u128::MAX << (128 - bits);
            (u128::from(*a) & mask) == (u128::from(*n) & mask)
        }
        _ => false,
    }
}

fn value_matches(m: &FieldMatcher, val: &RuleVal, ev: &J) -> bool {
    let found = resolve_field(ev, &m.field);
    match val {
        RuleVal::Exists(want) => {
            let present = matches!(found, Some(v) if !v.is_null());
            present == *want
        }
        RuleVal::Null => matches!(found, None | Some(J::Null)),
        RuleVal::Regex(re) => match found {
            Some(v) => value_strings(v).iter().any(|s| re.is_match(s)),
            None => false,
        },
        RuleVal::Cidr(net, bits) => match found {
            Some(v) => value_strings(v)
                .iter()
                .filter_map(|s| s.trim().parse::<IpAddr>().ok())
                .any(|ip| ip_in_net(&ip, net, *bits)),
            None => false,
        },
        RuleVal::Num(n) => match found {
            Some(v) => value_strings(v)
                .iter()
                .filter_map(|s| s.trim().parse::<f64>().ok())
                .any(|x| match m.mods.op {
                    Op::Gt => x > *n,
                    Op::Gte => x >= *n,
                    Op::Lt => x < *n,
                    Op::Lte => x <= *n,
                    _ => false,
                }),
            None => false,
        },
        RuleVal::Text(variants) => {
            let Some(found) = found else { return false };
            let cands = value_strings(found);
            if cands.is_empty() {
                return false;
            }
            if m.mods.fieldref {
                // The rule value names another field; compare the two values.
                return variants.iter().any(|other_field| {
                    let Some(other) = resolve_field(ev, other_field) else {
                        return false;
                    };
                    let others = value_strings(other);
                    cands.iter().any(|c| {
                        others
                            .iter()
                            .any(|o| text_match(m.mods.op, c, o, m.mods.cased, true))
                    })
                });
            }
            variants.iter().any(|p| {
                cands
                    .iter()
                    .any(|c| text_match(m.mods.op, c, p, m.mods.cased, false))
            })
        }
    }
}

fn field_matches(m: &FieldMatcher, ev: &J) -> bool {
    if m.mods.all {
        m.values.iter().all(|v| value_matches(m, v, ev))
    } else {
        m.values.iter().any(|v| value_matches(m, v, ev))
    }
}

fn selection_matches(sel: &Selection, ev: &J, scalars: &[String]) -> bool {
    sel.alternatives.iter().any(|alt| match alt {
        Alternative::Fields(fields) => fields.iter().all(|f| field_matches(f, ev)),
        Alternative::Keyword(words) => words.iter().any(|w| {
            let mut toks = tokenize_pattern(&w.to_lowercase());
            toks.insert(0, PatTok::Star);
            toks.push(PatTok::Star);
            scalars
                .iter()
                .any(|s| glob_match(&toks, &s.to_lowercase().chars().collect::<Vec<_>>()))
        }),
    })
}

fn eval_cond(c: &Cond, results: &BTreeMap<&str, bool>) -> bool {
    match c {
        Cond::Sel(n) => *results.get(n.as_str()).unwrap_or(&false),
        Cond::AtLeast(k, names) => {
            names
                .iter()
                .filter(|n| *results.get(n.as_str()).unwrap_or(&false))
                .count()
                >= *k
        }
        Cond::AllOf(names) => names
            .iter()
            .all(|n| *results.get(n.as_str()).unwrap_or(&false)),
        Cond::And(a, b) => eval_cond(a, results) && eval_cond(b, results),
        Cond::Or(a, b) => eval_cond(a, results) || eval_cond(b, results),
        Cond::Not(a) => !eval_cond(a, results),
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

struct Detection {
    event_index: usize,
    rule_index: usize,
    timestamp: String,
}

/// Best-effort event timestamp, for display only.
const TIME_FIELDS: [&str; 7] = [
    "TimeCreated",
    "SystemTime",
    "UtcTime",
    "@timestamp",
    "timestamp",
    "EventTime",
    "TimeGenerated",
];

fn event_timestamp(ev: &J) -> String {
    for f in TIME_FIELDS {
        if let Some(v) = resolve_field(ev, f) {
            if let Some(s) = v.as_str() {
                return s.to_string();
            }
            if let Some(o) = v.as_object() {
                if let Some(s) = get_ci(o, "SystemTime").and_then(|x| x.as_str()) {
                    return s.to_string();
                }
            }
        }
    }
    String::new()
}

/// Run `rules` (Sigma YAML, one or more `---`-separated documents) against
/// `events` (a JSON array, newline-delimited JSON, or a single JSON object).
///
/// * `min_level` — `any` (default) or the lowest severity to keep.
/// * `status` — `any` (default) or the only rule status to keep.
/// * `output` — `report` (default) | `table` | `json`.
/// * `max_matches` — cap on reported detections; 0 uses [`DEFAULT_MAX_MATCHES`].
/// * `show_event` — include the matching event record with each detection.
#[allow(clippy::too_many_arguments)]
pub fn match_rules(
    rules: &str,
    events: &str,
    min_level: &str,
    status: &str,
    output: &str,
    max_matches: u32,
    show_event: bool,
) -> Result<String, String> {
    if rules.trim().is_empty() {
        return Err("no Sigma rules provided — paste at least one rule in Sigma YAML".to_string());
    }
    if rules.len() > MAX_RULES_BYTES {
        return Err(format!(
            "the rules input is {} bytes; the cap is {MAX_RULES_BYTES}",
            rules.len()
        ));
    }
    if events.len() > MAX_EVENTS_BYTES {
        return Err(format!(
            "the events input is {} bytes; the cap is {MAX_EVENTS_BYTES}",
            events.len()
        ));
    }
    let output = if output.trim().is_empty() {
        "report"
    } else {
        output.trim()
    };
    if !matches!(output, "report" | "table" | "json") {
        return Err(format!(
            "unknown output '{output}' — use report, table or json"
        ));
    }
    let min_level_raw = if min_level.trim().is_empty() {
        "any"
    } else {
        min_level.trim()
    };
    let min_level = if min_level_raw.eq_ignore_ascii_case("any") {
        None
    } else {
        Some(Level::parse(min_level_raw).ok_or_else(|| {
            format!(
                "unknown min_level '{min_level_raw}' — use any, informational, low, medium, high or critical"
            )
        })?)
    };
    let status_raw = if status.trim().is_empty() {
        "any"
    } else {
        status.trim()
    };
    let status_filter = if status_raw.eq_ignore_ascii_case("any") {
        None
    } else {
        let s = status_raw.to_ascii_lowercase();
        if !STATUSES.contains(&s.as_str()) {
            return Err(format!(
                "unknown status '{status_raw}' — use any, {}",
                STATUSES.join(", ")
            ));
        }
        Some(s)
    };
    let cap = if max_matches == 0 {
        DEFAULT_MAX_MATCHES
    } else {
        max_matches.min(MAX_MATCHES_CAP)
    } as usize;

    let (all_rules, skipped) = parse_rules(rules)?;
    let events = parse_events(events)?;
    if events.len() > MAX_EVENTS {
        return Err(format!(
            "{} events supplied; the cap is {MAX_EVENTS} per run",
            events.len()
        ));
    }

    // Severity / status filtering.
    let mut filtered = 0usize;
    let kept: Vec<&Rule> = all_rules
        .iter()
        .filter(|r| {
            let level_ok = match min_level {
                None => true,
                Some(min) => r.level.unwrap_or(Level::Informational) >= min,
            };
            let status_ok = match &status_filter {
                None => true,
                Some(s) => &r.status == s,
            };
            if !(level_ok && status_ok) {
                filtered += 1;
            }
            level_ok && status_ok
        })
        .collect();

    let mut detections: Vec<Detection> = Vec::new();
    let mut total_detections = 0usize;
    let mut events_matched = 0usize;
    for (ei, ev) in events.iter().enumerate() {
        let mut scalars = Vec::new();
        all_scalars(ev, &mut scalars);
        let mut hit_this_event = false;
        for (ri, rule) in kept.iter().enumerate() {
            let results: BTreeMap<&str, bool> = rule
                .selections
                .iter()
                .map(|s| (s.name.as_str(), selection_matches(s, ev, &scalars)))
                .collect();
            if !eval_cond(&rule.condition, &results) {
                continue;
            }
            total_detections += 1;
            hit_this_event = true;
            if detections.len() < cap {
                detections.push(Detection {
                    event_index: ei,
                    rule_index: ri,
                    timestamp: event_timestamp(ev),
                });
            }
        }
        if hit_this_event {
            events_matched += 1;
        }
    }

    let summary = Summary {
        rules_loaded: kept.len(),
        rules_filtered: filtered,
        rules_skipped: skipped.len(),
        events_scanned: events.len(),
        detections: total_detections,
        events_matched,
        shown: detections.len(),
        truncated: total_detections > detections.len(),
    };

    Ok(match output {
        "table" => render_table(&summary, &kept, &detections, &events, show_event),
        "json" => render_json(&summary, &kept, &skipped, &detections, &events, show_event),
        _ => render_report(&summary, &kept, &skipped, &detections, &events, show_event),
    })
}

struct Summary {
    rules_loaded: usize,
    rules_filtered: usize,
    rules_skipped: usize,
    events_scanned: usize,
    detections: usize,
    events_matched: usize,
    shown: usize,
    truncated: bool,
}

fn level_name(r: &Rule) -> &'static str {
    r.level.map(Level::as_str).unwrap_or("unknown")
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("{n} {one}")
    } else {
        format!("{n} {many}")
    }
}

fn render_report(
    s: &Summary,
    kept: &[&Rule],
    skipped: &[SkippedRule],
    dets: &[Detection],
    events: &[J],
    show_event: bool,
) -> String {
    let mut out = String::new();
    out.push_str("Sigma detection report\n");
    out.push_str("======================\n");
    out.push_str(&format!("Rules loaded:   {}\n", s.rules_loaded));
    if s.rules_filtered > 0 {
        out.push_str(&format!("Rules filtered: {}\n", s.rules_filtered));
    }
    if s.rules_skipped > 0 {
        out.push_str(&format!("Rules skipped:  {}\n", s.rules_skipped));
    }
    out.push_str(&format!("Events scanned: {}\n", s.events_scanned));
    out.push_str(&format!(
        "Detections:     {} ({} of {} events matched)\n",
        s.detections, s.events_matched, s.events_scanned
    ));

    if s.detections > 0 {
        // Hits by level, highest first.
        let mut by_level: BTreeMap<&str, usize> = BTreeMap::new();
        let mut by_rule: Vec<(usize, &Rule)> = Vec::new();
        for (i, r) in kept.iter().enumerate() {
            let n = dets.iter().filter(|d| d.rule_index == i).count();
            if n > 0 {
                *by_level.entry(level_name(r)).or_insert(0) += n;
                by_rule.push((n, r));
            }
        }
        out.push('\n');
        out.push_str("Hits by level:\n");
        for l in LEVELS_DESC {
            if let Some(n) = by_level.get(l.as_str()) {
                out.push_str(&format!("  {:<14}{}\n", l.as_str(), n));
            }
        }
        if let Some(n) = by_level.get("unknown") {
            out.push_str(&format!("  {:<14}{}\n", "unknown", n));
        }

        by_rule.sort_by(|a, b| b.0.cmp(&a.0));
        out.push('\n');
        out.push_str("Hits by rule:\n");
        for (n, r) in &by_rule {
            out.push_str(&format!("  {n}  [{}] {}\n", level_name(r), r.title));
        }

        out.push('\n');
        out.push_str("Detections:\n");
        for (i, d) in dets.iter().enumerate() {
            let r = kept[d.rule_index];
            let when = if d.timestamp.is_empty() {
                String::new()
            } else {
                format!(" ({})", d.timestamp)
            };
            out.push_str(&format!(
                "  {}. [{}] {} — event {}{}\n",
                i + 1,
                level_name(r),
                r.title,
                d.event_index + 1,
                when
            ));
            if show_event {
                out.push_str(&format!("     {}\n", events[d.event_index]));
            }
        }
        if s.truncated {
            out.push_str(&format!(
                "  … {} more detection(s) not shown (max_matches = {})\n",
                s.detections - s.shown,
                s.shown
            ));
        }
    } else {
        out.push('\n');
        out.push_str("No rule matched any event.\n");
    }

    if !skipped.is_empty() {
        out.push('\n');
        out.push_str("Skipped rules:\n");
        for sk in skipped {
            out.push_str(&format!("  - {}: {}\n", sk.title, sk.reason));
        }
    }
    out.trim_end().to_string()
}

fn md_cell(s: &str) -> String {
    s.replace('|', "\\|").replace(['\n', '\r'], " ")
}

fn render_table(
    s: &Summary,
    kept: &[&Rule],
    dets: &[Detection],
    events: &[J],
    show_event: bool,
) -> String {
    let mut out = format!(
        "{} · {} · {}\n",
        plural(s.detections, "detection", "detections"),
        plural(s.rules_loaded, "rule loaded", "rules loaded"),
        plural(s.events_scanned, "event scanned", "events scanned"),
    );
    if s.detections == 0 {
        out.push('\n');
        out.push_str("No rule matched any event.");
        return out;
    }
    out.push('\n');
    if show_event {
        out.push_str("| # | Level | Rule | Event | Timestamp | Record |\n");
        out.push_str("| --- | --- | --- | --- | --- | --- |\n");
    } else {
        out.push_str("| # | Level | Rule | Event | Timestamp |\n");
        out.push_str("| --- | --- | --- | --- | --- |\n");
    }
    for (i, d) in dets.iter().enumerate() {
        let r = kept[d.rule_index];
        let ts = if d.timestamp.is_empty() {
            "-".to_string()
        } else {
            md_cell(&d.timestamp)
        };
        if show_event {
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} |\n",
                i + 1,
                level_name(r),
                md_cell(&r.title),
                d.event_index + 1,
                ts,
                md_cell(&events[d.event_index].to_string()),
            ));
        } else {
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                i + 1,
                level_name(r),
                md_cell(&r.title),
                d.event_index + 1,
                ts,
            ));
        }
    }
    if s.truncated {
        out.push_str(&format!(
            "\n{} more detection(s) not shown (max_matches = {}).",
            s.detections - s.shown,
            s.shown
        ));
    }
    out.trim_end().to_string()
}

fn render_json(
    s: &Summary,
    kept: &[&Rule],
    skipped: &[SkippedRule],
    dets: &[Detection],
    events: &[J],
    show_event: bool,
) -> String {
    let mut by_level = serde_json::Map::new();
    let mut by_rule = Vec::new();
    for (i, r) in kept.iter().enumerate() {
        let n = dets.iter().filter(|d| d.rule_index == i).count();
        if n == 0 {
            continue;
        }
        let entry = by_level
            .entry(level_name(r).to_string())
            .or_insert(J::from(0));
        *entry = J::from(entry.as_u64().unwrap_or(0) + n as u64);
        by_rule.push(serde_json::json!({
            "title": r.title,
            "id": r.id,
            "level": level_name(r),
            "hits": n,
        }));
    }
    let detections: Vec<J> = dets
        .iter()
        .map(|d| {
            let r = kept[d.rule_index];
            let mut o = serde_json::Map::new();
            o.insert("event_index".into(), J::from(d.event_index + 1));
            o.insert("title".into(), J::from(r.title.clone()));
            o.insert("id".into(), J::from(r.id.clone()));
            o.insert("level".into(), J::from(level_name(r)));
            o.insert("status".into(), J::from(r.status.clone()));
            o.insert("description".into(), J::from(r.description.clone()));
            o.insert("author".into(), J::from(r.author.clone()));
            o.insert(
                "tags".into(),
                J::Array(r.tags.iter().cloned().map(J::from).collect()),
            );
            o.insert(
                "logsource".into(),
                J::Object(
                    r.logsource
                        .iter()
                        .map(|(k, v)| (k.clone(), J::from(v.clone())))
                        .collect(),
                ),
            );
            o.insert("timestamp".into(), J::from(d.timestamp.clone()));
            if show_event {
                o.insert("event".into(), events[d.event_index].clone());
            }
            J::Object(o)
        })
        .collect();

    let value = serde_json::json!({
        "summary": {
            "rules_loaded": s.rules_loaded,
            "rules_filtered": s.rules_filtered,
            "rules_skipped": s.rules_skipped,
            "events_scanned": s.events_scanned,
            "detections": s.detections,
            "events_matched": s.events_matched,
            "detections_shown": s.shown,
            "truncated": s.truncated,
            "hits_by_level": J::Object(by_level),
            "hits_by_rule": by_rule,
        },
        "skipped_rules": skipped.iter().map(|k| serde_json::json!({
            "title": k.title,
            "reason": k.reason,
        })).collect::<Vec<_>>(),
        "detections": detections,
    });
    serde_json::to_string_pretty(&value).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const PS_RULE: &str = r#"
title: Encoded PowerShell command
id: 1111-aaaa
status: test
level: high
logsource:
  product: windows
  service: powershell
detection:
  selection:
    EventID: 4104
    ScriptBlockText|contains:
      - '-enc '
      - '-EncodedCommand'
  condition: selection
"#;

    const EVENTS: &str = r#"[
      {"EventID": 4104, "ScriptBlockText": "powershell.exe -enc SQBFAFgA"},
      {"EventID": 4104, "ScriptBlockText": "Get-ChildItem C:\\Users"}
    ]"#;

    fn run(rules: &str, events: &str, output: &str) -> String {
        match_rules(rules, events, "any", "any", output, 0, false).unwrap()
    }

    #[test]
    fn happy_path_report_lists_the_hit() {
        let out = run(PS_RULE, EVENTS, "report");
        assert_eq!(
            out,
            "Sigma detection report\n\
             ======================\n\
             Rules loaded:   1\n\
             Events scanned: 2\n\
             Detections:     1 (1 of 2 events matched)\n\
             \n\
             Hits by level:\n\
             \x20 high          1\n\
             \n\
             Hits by rule:\n\
             \x20 1  [high] Encoded PowerShell command\n\
             \n\
             Detections:\n\
             \x20 1. [high] Encoded PowerShell command — event 1"
        );
    }

    #[test]
    fn no_match_is_reported_not_an_error() {
        let out = run(
            PS_RULE,
            r#"[{"EventID": 4104, "ScriptBlockText": "dir"}]"#,
            "report",
        );
        assert!(out.contains("Detections:     0 (0 of 1 events matched)"));
        assert!(out.ends_with("No rule matched any event."));
    }

    #[test]
    fn empty_rules_is_an_error() {
        let e = match_rules("   ", EVENTS, "any", "any", "report", 0, false).unwrap_err();
        assert!(e.contains("no Sigma rules provided"), "{e}");
    }

    #[test]
    fn bad_output_is_an_error() {
        let e = match_rules(PS_RULE, EVENTS, "any", "any", "csv", 0, false).unwrap_err();
        assert_eq!(e, "unknown output 'csv' — use report, table or json");
    }

    #[test]
    fn events_accept_ndjson_and_a_single_object() {
        let ndjson = "{\"EventID\": 4104, \"ScriptBlockText\": \"x -enc y\"}\n\n{\"EventID\": 1}";
        let out = run(PS_RULE, ndjson, "report");
        assert!(out.contains("Events scanned: 2"), "{out}");
        assert!(out.contains("Detections:     1"), "{out}");

        let single = "{\n  \"EventID\": 4104,\n  \"ScriptBlockText\": \"a -EncodedCommand b\"\n}";
        let out = run(PS_RULE, single, "report");
        assert!(out.contains("Events scanned: 1"), "{out}");
        assert!(out.contains("Detections:     1"), "{out}");
    }

    #[test]
    fn malformed_event_line_names_the_line() {
        let e = match_rules(
            PS_RULE,
            "{\"a\":1}\nnot json",
            "any",
            "any",
            "report",
            0,
            false,
        )
        .unwrap_err();
        assert!(e.starts_with("line 2 is not valid JSON"), "{e}");
    }

    #[test]
    fn table_output_has_one_row_per_hit() {
        let out = run(PS_RULE, EVENTS, "table");
        assert_eq!(
            out,
            "1 detection · 1 rule loaded · 2 events scanned\n\
             \n\
             | # | Level | Rule | Event | Timestamp |\n\
             | --- | --- | --- | --- | --- |\n\
             | 1 | high | Encoded PowerShell command | 1 | - |"
        );
    }

    #[test]
    fn json_output_carries_summary_and_detections() {
        let out = run(PS_RULE, EVENTS, "json");
        let v: J = serde_json::from_str(&out).unwrap();
        assert_eq!(v["summary"]["detections"], 1);
        assert_eq!(v["summary"]["events_scanned"], 2);
        assert_eq!(v["summary"]["hits_by_level"]["high"], 1);
        assert_eq!(v["detections"][0]["title"], "Encoded PowerShell command");
        assert_eq!(v["detections"][0]["event_index"], 1);
        assert_eq!(v["detections"][0]["logsource"]["service"], "powershell");
        assert!(v["detections"][0].get("event").is_none());
    }

    #[test]
    fn show_event_includes_the_record() {
        let out = match_rules(PS_RULE, EVENTS, "any", "any", "json", 0, true).unwrap();
        let v: J = serde_json::from_str(&out).unwrap();
        assert_eq!(v["detections"][0]["event"]["EventID"], 4104);
    }

    #[test]
    fn min_level_and_status_filter_rules_out() {
        let out = match_rules(PS_RULE, EVENTS, "critical", "any", "report", 0, false).unwrap();
        assert!(out.contains("Rules loaded:   0"), "{out}");
        assert!(out.contains("Rules filtered: 1"), "{out}");

        let out = match_rules(PS_RULE, EVENTS, "any", "stable", "report", 0, false).unwrap();
        assert!(out.contains("Rules loaded:   0"), "{out}");
    }

    #[test]
    fn max_matches_caps_the_listing_but_not_the_count() {
        let events = r#"[
          {"EventID":4104,"ScriptBlockText":"a -enc 1"},
          {"EventID":4104,"ScriptBlockText":"b -enc 2"},
          {"EventID":4104,"ScriptBlockText":"c -enc 3"}
        ]"#;
        let out = match_rules(PS_RULE, events, "any", "any", "report", 2, false).unwrap();
        assert!(
            out.contains("Detections:     3 (3 of 3 events matched)"),
            "{out}"
        );
        assert!(
            out.contains("… 1 more detection(s) not shown (max_matches = 2)"),
            "{out}"
        );
    }

    // --- modifiers -------------------------------------------------------

    fn hits(rule: &str, event: &str) -> usize {
        let out = match_rules(rule, event, "any", "any", "json", 0, false).unwrap();
        let v: J = serde_json::from_str(&out).unwrap();
        v["summary"]["detections"].as_u64().unwrap() as usize
    }

    fn rule_with(detection: &str) -> String {
        format!("title: t\nlevel: low\ndetection:\n{detection}  condition: selection\n")
    }

    #[test]
    fn startswith_endswith_and_case_insensitivity() {
        let r = rule_with("  selection:\n    Image|startswith: 'C:\\\\Windows'\n");
        assert_eq!(
            hits(&r, r#"[{"Image":"c:\\windows\\system32\\cmd.exe"}]"#),
            1
        );
        let r = rule_with("  selection:\n    Image|endswith: '\\cmd.exe'\n");
        assert_eq!(hits(&r, r#"[{"Image":"C:\\Windows\\CMD.EXE"}]"#), 1);
        let r = rule_with("  selection:\n    Image|endswith|cased: '\\cmd.exe'\n");
        assert_eq!(hits(&r, r#"[{"Image":"C:\\Windows\\CMD.EXE"}]"#), 0);
    }

    #[test]
    fn wildcards_and_escapes() {
        let r = rule_with("  selection:\n    CommandLine: '*\\\\temp\\\\*.ps1'\n");
        assert_eq!(hits(&r, r#"[{"CommandLine":"c:\\temp\\run.ps1"}]"#), 1);
        let r = rule_with("  selection:\n    CommandLine: 'a?c'\n");
        assert_eq!(hits(&r, r#"[{"CommandLine":"abc"}]"#), 1);
        assert_eq!(hits(&r, r#"[{"CommandLine":"ac"}]"#), 0);
        // An escaped star is a literal star.
        let r = rule_with("  selection:\n    CommandLine: 'a\\*c'\n");
        assert_eq!(hits(&r, r#"[{"CommandLine":"a*c"}]"#), 1);
        assert_eq!(hits(&r, r#"[{"CommandLine":"abc"}]"#), 0);
    }

    #[test]
    fn list_values_are_or_unless_all() {
        let r = rule_with("  selection:\n    CommandLine|contains:\n      - foo\n      - bar\n");
        assert_eq!(hits(&r, r#"[{"CommandLine":"xx foo yy"}]"#), 1);
        let r =
            rule_with("  selection:\n    CommandLine|contains|all:\n      - foo\n      - bar\n");
        assert_eq!(hits(&r, r#"[{"CommandLine":"xx foo yy"}]"#), 0);
        assert_eq!(hits(&r, r#"[{"CommandLine":"foo and bar"}]"#), 1);
    }

    #[test]
    fn null_matches_absent_or_null_and_exists_is_the_inverse() {
        let r = rule_with("  selection:\n    ParentImage: null\n");
        assert_eq!(hits(&r, r#"[{"Image":"a"}]"#), 1);
        assert_eq!(hits(&r, r#"[{"ParentImage":null}]"#), 1);
        assert_eq!(hits(&r, r#"[{"ParentImage":"x"}]"#), 0);
        let r = rule_with("  selection:\n    ParentImage|exists: true\n");
        assert_eq!(hits(&r, r#"[{"ParentImage":"x"}]"#), 1);
        assert_eq!(hits(&r, r#"[{"Image":"a"}]"#), 0);
    }

    #[test]
    fn regex_with_sub_modifiers() {
        let r = rule_with("  selection:\n    CommandLine|re: 'p[aA]ss\\d+'\n");
        assert_eq!(hits(&r, r#"[{"CommandLine":"pass42"}]"#), 1);
        assert_eq!(hits(&r, r#"[{"CommandLine":"PASS42"}]"#), 0);
        let r = rule_with("  selection:\n    CommandLine|re|i: 'pass\\d+'\n");
        assert_eq!(hits(&r, r#"[{"CommandLine":"PASS42"}]"#), 1);
    }

    #[test]
    fn numeric_comparison_modifiers() {
        let r = rule_with("  selection:\n    Count|gte: 5\n");
        assert_eq!(hits(&r, r#"[{"Count":5}]"#), 1);
        assert_eq!(hits(&r, r#"[{"Count":4}]"#), 0);
        let r = rule_with("  selection:\n    Count|lt: 5\n");
        assert_eq!(hits(&r, r#"[{"Count":"4"}]"#), 1);
    }

    #[test]
    fn cidr_modifier_matches_v4_and_v6() {
        let r = rule_with("  selection:\n    DestinationIp|cidr: '10.0.0.0/8'\n");
        assert_eq!(hits(&r, r#"[{"DestinationIp":"10.4.5.6"}]"#), 1);
        assert_eq!(hits(&r, r#"[{"DestinationIp":"11.4.5.6"}]"#), 0);
        let r = rule_with("  selection:\n    DestinationIp|cidr: '2001:db8::/32'\n");
        assert_eq!(hits(&r, r#"[{"DestinationIp":"2001:db8:1::9"}]"#), 1);
    }

    #[test]
    fn base64_and_utf16_encodings_are_matched() {
        // "whoami" base64 is d2hvYW1p
        let r = rule_with("  selection:\n    CommandLine|base64|contains: 'whoami'\n");
        assert_eq!(
            hits(&r, r#"[{"CommandLine":"powershell -enc d2hvYW1p"}]"#),
            1
        );
        // UTF-16LE "whoami" base64 is dwBoAG8AYQBtAGkA
        let r = rule_with("  selection:\n    CommandLine|utf16le|base64|contains: 'whoami'\n");
        assert_eq!(
            hits(
                &r,
                r#"[{"CommandLine":"powershell -enc dwBoAG8AYQBtAGkA"}]"#
            ),
            1
        );
    }

    #[test]
    fn base64offset_matches_at_every_alignment() {
        let r = rule_with("  selection:\n    CommandLine|base64offset|contains: 'whoami'\n");
        for prefix in ["", "x", "xy"] {
            let blob = B64.encode(format!("{prefix}whoami trailing"));
            let ev = format!(r#"[{{"CommandLine":"{blob}"}}]"#);
            assert_eq!(hits(&r, &ev), 1, "offset {} failed ({blob})", prefix.len());
        }
    }

    #[test]
    fn windash_expands_option_leaders_only() {
        let r = rule_with("  selection:\n    CommandLine|windash|contains: '-EncodedCommand'\n");
        assert_eq!(
            hits(&r, r#"[{"CommandLine":"powershell /EncodedCommand x"}]"#),
            1
        );
        assert_eq!(
            hits(
                &r,
                r#"[{"CommandLine":"powershell \u2013EncodedCommand x"}]"#
            ),
            1
        );
        // A dash inside a word is not an option leader, so it is left alone.
        let vars = windash_variants(b"foo-bar");
        assert_eq!(vars, vec![b"foo-bar".to_vec()]);
    }

    #[test]
    fn fieldref_compares_two_event_fields() {
        let r = rule_with("  selection:\n    User|fieldref: TargetUser\n");
        assert_eq!(hits(&r, r#"[{"User":"alice","TargetUser":"alice"}]"#), 1);
        assert_eq!(hits(&r, r#"[{"User":"alice","TargetUser":"bob"}]"#), 0);
    }

    #[test]
    fn keyword_lists_search_every_scalar() {
        let r = rule_with("  selection:\n    - mimikatz\n    - sekurlsa\n");
        assert_eq!(hits(&r, r#"[{"Nested":{"Cmd":"run MIMIKATZ.exe"}}]"#), 1);
        assert_eq!(hits(&r, r#"[{"Nested":{"Cmd":"run notepad"}}]"#), 0);
    }

    #[test]
    fn list_of_maps_is_an_or_of_groups() {
        let r = rule_with("  selection:\n    - {A: 1, B: 2}\n    - {C: 3}\n");
        assert_eq!(hits(&r, r#"[{"A":1,"B":2}]"#), 1);
        assert_eq!(hits(&r, r#"[{"A":1}]"#), 0);
        assert_eq!(hits(&r, r#"[{"C":3}]"#), 1);
    }

    #[test]
    fn nested_evtx_shaped_fields_resolve() {
        let r = rule_with("  selection:\n    EventID: 4625\n    TargetUserName: admin\n");
        let ev =
            r#"[{"Event":{"System":{"EventID":4625},"EventData":{"TargetUserName":"admin"}}}]"#;
        assert_eq!(hits(&r, ev), 1);
    }

    // --- condition grammar ----------------------------------------------

    #[test]
    fn condition_supports_and_or_not_and_parentheses() {
        let d = "  sel:\n    A: 1\n  filt:\n    B: 2\n";
        let rule = format!("title: t\ndetection:\n{d}  condition: sel and not filt\n");
        assert_eq!(hits(&rule, r#"[{"A":1}]"#), 1);
        assert_eq!(hits(&rule, r#"[{"A":1,"B":2}]"#), 0);

        let rule =
            format!("title: t\ndetection:\n{d}  condition: (sel or filt) and not (sel and filt)\n");
        assert_eq!(hits(&rule, r#"[{"A":1}]"#), 1);
        assert_eq!(hits(&rule, r#"[{"A":1,"B":2}]"#), 0);
    }

    #[test]
    fn condition_supports_quantifiers_over_prefixes_and_them() {
        let d = "  sel_a:\n    A: 1\n  sel_b:\n    B: 2\n  sel_c:\n    C: 3\n";
        let one = format!("title: t\ndetection:\n{d}  condition: 1 of sel_*\n");
        assert_eq!(hits(&one, r#"[{"A":1}]"#), 1);
        let all = format!("title: t\ndetection:\n{d}  condition: all of them\n");
        assert_eq!(hits(&all, r#"[{"A":1,"B":2}]"#), 0);
        assert_eq!(hits(&all, r#"[{"A":1,"B":2,"C":3}]"#), 1);
        let two = format!("title: t\ndetection:\n{d}  condition: 2 of them\n");
        assert_eq!(hits(&two, r#"[{"A":1,"B":2}]"#), 1);
        assert_eq!(hits(&two, r#"[{"A":1}]"#), 0);
    }

    #[test]
    fn condition_list_is_an_or() {
        let d = "  sel_a:\n    A: 1\n  sel_b:\n    B: 2\n";
        let rule = format!("title: t\ndetection:\n{d}  condition:\n    - sel_a\n    - sel_b\n");
        assert_eq!(hits(&rule, r#"[{"B":2}]"#), 1);
    }

    // --- skipping --------------------------------------------------------

    #[test]
    fn unsupported_rules_are_skipped_with_a_reason() {
        let agg = "title: Aggregated\ndetection:\n  sel:\n    A: 1\n  condition: sel | count() by B > 5\n";
        let out = match_rules(agg, r#"[{"A":1}]"#, "any", "any", "report", 0, false).unwrap();
        assert!(out.contains("Rules loaded:   0"), "{out}");
        assert!(out.contains("Rules skipped:  1"), "{out}");
        assert!(
            out.contains("- Aggregated: aggregation conditions (the '|' pipe"),
            "{out}"
        );

        let bad_mod =
            "title: Expanded\ndetection:\n  sel:\n    A|expand: '%x%'\n  condition: sel\n";
        let out = match_rules(bad_mod, r#"[{"A":1}]"#, "any", "any", "report", 0, false).unwrap();
        assert!(
            out.contains("- Expanded: unsupported modifier 'expand' on field 'A'"),
            "{out}"
        );

        let corr = "title: Corr\ncorrelation:\n  type: event_count\ndetection:\n  sel:\n    A: 1\n  condition: sel\n";
        let out = match_rules(corr, r#"[{"A":1}]"#, "any", "any", "report", 0, false).unwrap();
        assert!(
            out.contains("Sigma correlation documents are not supported"),
            "{out}"
        );
    }

    #[test]
    fn multi_document_rule_files_load_every_rule() {
        let two = format!("{PS_RULE}\n---\ntitle: Second\nlevel: low\ndetection:\n  sel:\n    EventID: 4104\n  condition: sel\n");
        let out = match_rules(&two, EVENTS, "any", "any", "report", 0, false).unwrap();
        assert!(out.contains("Rules loaded:   2"), "{out}");
        assert!(
            out.contains("Detections:     3 (2 of 2 events matched)"),
            "{out}"
        );
    }

    #[test]
    fn unknown_selection_in_condition_skips_the_rule() {
        let r = "title: Bad\ndetection:\n  sel:\n    A: 1\n  condition: sel and missing\n";
        let out = match_rules(r, r#"[{"A":1}]"#, "any", "any", "report", 0, false).unwrap();
        assert!(
            out.contains("condition references 'missing', which is not a selection"),
            "{out}"
        );
    }

    #[test]
    fn base64_offset_fragments_are_the_three_alignments() {
        assert_eq!(
            base64_offsets(b"whoami"),
            vec!["d2hvYW1p", "dob2Fta", "3aG9hbW"]
        );
    }
}

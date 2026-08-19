//! log-pattern-miner core — cluster raw log lines into a handful of message
//! templates (Drain-style) with occurrence counts. No wafer/wasm-bindgen deps;
//! shared by the chat skill block, the CLI and the web page.
//!
//! Pipeline, in order:
//!
//! 1. **Tokenize** each line on whitespace (plus any extra delimiter
//!    characters). A double-quoted run is kept together as one token.
//! 2. **Mask** the variable parts of every token — numbers, hex blobs, IPs,
//!    MACs, UUIDs, dates, clock times, paths, URLs, e-mail addresses and
//!    quoted strings become typed placeholders such as `<NUM>` or `<IP>`.
//! 3. **Cluster** with a fixed-depth parse tree: the first layer is the token
//!    count, the next `depth - 2` layers are the leading tokens (a token that
//!    holds a digit or is already a placeholder takes the `<*>` branch), and
//!    each leaf holds the clusters whose templates share that prefix.
//! 4. **Merge** a line into the leaf's most similar template when the fraction
//!    of matching token positions reaches the similarity threshold; positions
//!    that disagree collapse to `<*>`. Otherwise the line seeds a new cluster.
//! 5. **Rank** the clusters by occurrence count and render them.
//!
//! Everything is deterministic — one pass in input order, ties resolved by
//! first appearance — so the same log always produces the same templates on
//! every surface. There is no persisted state between runs.

use std::collections::HashMap;

use serde::Serialize;

/// Largest input accepted, in Unicode characters.
pub const MAX_CHARS: usize = 2_000_000;
/// Largest number of input lines accepted.
pub const MAX_LINES: usize = 200_000;
/// Sample raw values kept per variable slot, and sample lines kept per pattern.
const MAX_SAMPLES: usize = 3;

/// The wildcard a merge leaves behind, and the branch key for variable tokens.
const WILDCARD: &str = "<*>";

/// Numeric suffixes kept next to a masked number, so `250ms` masks to
/// `<NUM>ms` instead of staying a literal (units are what make otherwise
/// identical duration lines look different).
const UNIT_SUFFIXES: &[&str] = &[
    "ms", "us", "ns", "s", "m", "h", "d", "kb", "mb", "gb", "tb", "kib", "mib", "gib", "tib", "b",
    "%", "k", "x",
];

// ---------------------------------------------------------------------------
// Options.
// ---------------------------------------------------------------------------

/// How variable tokens are rendered before clustering.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mask {
    /// Typed placeholders: `<NUM>`, `<IP>`, `<UUID>`, …
    Typed,
    /// Every masked value renders as `<*>`.
    Wildcard,
    /// No pre-masking; only the similarity merge introduces `<*>`.
    None,
}

impl Mask {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "typed" => Ok(Mask::Typed),
            "wildcard" => Ok(Mask::Wildcard),
            "none" => Ok(Mask::None),
            other => Err(format!(
                "invalid mask {other:?}: expected \"typed\", \"wildcard\" or \"none\""
            )),
        }
    }

    /// Render a placeholder under this mode.
    fn ph(self, typed: &str) -> String {
        match self {
            Mask::Typed => typed.to_string(),
            _ => WILDCARD.to_string(),
        }
    }
}

/// Output rendering.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Json,
    Table,
    Lines,
}

impl Format {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "table" => Ok(Format::Table),
            "json" => Ok(Format::Json),
            "lines" => Ok(Format::Lines),
            other => Err(format!(
                "invalid format {other:?}: expected \"table\", \"json\" or \"lines\""
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// Tokenizing.
// ---------------------------------------------------------------------------

/// Split one line into tokens. Whitespace always separates; every character in
/// `extra` separates too. A `"…"` run (with `\"` escapes) stays one token so a
/// quoted request line or message can be masked as a single value.
fn tokenize(line: &str, extra: &[char]) -> Vec<String> {
    let chars: Vec<char> = line.chars().collect();
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        if c == '"' {
            if let Some(end) = closing_quote(&chars, i) {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
                out.push(chars[i..=end].iter().collect());
                i = end + 1;
                continue;
            }
        }
        if c.is_whitespace() || extra.contains(&c) {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(c);
        }
        i += 1;
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Index of the `"` closing the one at `start`, honouring `\"` escapes.
fn closing_quote(chars: &[char], start: usize) -> Option<usize> {
    let mut i = start + 1;
    while i < chars.len() {
        match chars[i] {
            '\\' => i += 2,
            '"' => return Some(i),
            _ => i += 1,
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Masking.
// ---------------------------------------------------------------------------

fn is_hex(c: char) -> bool {
    c.is_ascii_hexdigit()
}

fn boundary_before(chars: &[char], i: usize) -> bool {
    i == 0 || !(chars[i - 1].is_ascii_alphanumeric() || chars[i - 1] == '_')
}

fn boundary_after(chars: &[char], i: usize) -> bool {
    i >= chars.len() || !(chars[i].is_ascii_alphanumeric() || chars[i] == '_')
}

/// Count of consecutive characters from `i` matching `pred`.
fn run_len(chars: &[char], i: usize, pred: impl Fn(char) -> bool) -> usize {
    let mut n = 0;
    while i + n < chars.len() && pred(chars[i + n]) {
        n += 1;
    }
    n
}

fn digits(chars: &[char], i: usize) -> usize {
    run_len(chars, i, |c| c.is_ascii_digit())
}

/// `8-4-4-4-12` hex groups.
fn match_uuid(c: &[char], i: usize) -> Option<usize> {
    let groups = [8usize, 4, 4, 4, 12];
    let mut p = i;
    for (n, want) in groups.iter().enumerate() {
        if run_len(c, p, is_hex) < *want {
            return None;
        }
        // The group must not run longer than its fixed width.
        if p + want < c.len() && is_hex(c[p + want]) {
            return None;
        }
        p += want;
        if n < 4 {
            if p >= c.len() || c[p] != '-' {
                return None;
            }
            p += 1;
        }
    }
    Some(p)
}

/// Dotted-quad IPv4, each octet 0–255.
fn match_ipv4(c: &[char], i: usize) -> Option<usize> {
    let mut p = i;
    for n in 0..4 {
        let d = digits(c, p);
        if d == 0 || d > 3 {
            return None;
        }
        let v: u32 = c[p..p + d].iter().collect::<String>().parse().ok()?;
        if v > 255 {
            return None;
        }
        p += d;
        if n < 3 {
            if p >= c.len() || c[p] != '.' {
                return None;
            }
            p += 1;
        }
    }
    Some(p)
}

/// Compressed or full IPv6 (`::1`, `fe80::1`, `2001:db8::8a2e:370:7334`).
fn match_ipv6(c: &[char], i: usize) -> Option<usize> {
    let mut p = i;
    let mut colons = 0usize;
    let mut groups = 0usize;
    let mut has_double = false;
    let mut has_letter = false;
    while p < c.len() {
        if c[p] == ':' {
            if p + 1 < c.len() && c[p + 1] == ':' {
                has_double = true;
                colons += 2;
                p += 2;
            } else {
                colons += 1;
                p += 1;
            }
            continue;
        }
        let n = run_len(c, p, is_hex);
        if n == 0 || n > 4 {
            break;
        }
        if c[p..p + n].iter().any(|ch| ch.is_ascii_alphabetic()) {
            has_letter = true;
        }
        groups += 1;
        p += n;
    }
    // Trailing colon is not part of the address unless it closed a `::`.
    if p > i && c[p - 1] == ':' && !(p >= i + 2 && c[p - 2] == ':') {
        p -= 1;
        colons -= 1;
    }
    let plausible = colons >= 2 && groups >= 1 && (has_double || colons >= 7 || has_letter);
    if plausible && boundary_after(c, p) {
        Some(p)
    } else {
        None
    }
}

/// Six hex pairs separated by `:` or `-`.
fn match_mac(c: &[char], i: usize) -> Option<usize> {
    let mut p = i;
    let sep = if i + 2 < c.len() { c[i + 2] } else { return None };
    if sep != ':' && sep != '-' {
        return None;
    }
    for n in 0..6 {
        if run_len(c, p, is_hex) < 2 {
            return None;
        }
        if p + 2 < c.len() && is_hex(c[p + 2]) {
            return None;
        }
        p += 2;
        if n < 5 {
            if p >= c.len() || c[p] != sep {
                return None;
            }
            p += 1;
        }
    }
    Some(p)
}

/// `HH:MM(:SS)(.fff)` clock time, optionally with a `Z`/`±HH:MM` zone.
fn match_clock(c: &[char], i: usize) -> Option<usize> {
    let mut p = i;
    if digits(c, p) != 2 {
        return None;
    }
    p += 2;
    for _ in 0..2 {
        if p < c.len() && c[p] == ':' && digits(c, p + 1) == 2 {
            p += 3;
        } else {
            break;
        }
    }
    if p < i + 5 {
        return None; // needs at least HH:MM
    }
    if p < c.len() && (c[p] == '.' || c[p] == ',') {
        let d = digits(c, p + 1);
        if d > 0 {
            p += 1 + d;
        }
    }
    Some(p + zone_len(c, p))
}

/// Length of a trailing `Z` or `±HH(:MM)` timezone at `p`.
fn zone_len(c: &[char], p: usize) -> usize {
    if p < c.len() && (c[p] == 'Z' || c[p] == 'z') && boundary_after(c, p + 1) {
        return 1;
    }
    if p < c.len() && (c[p] == '+' || c[p] == '-') && digits(c, p + 1) == 2 {
        let mut n = 3;
        if p + n < c.len() && c[p + n] == ':' && digits(c, p + n + 1) == 2 {
            n += 3;
        } else if digits(c, p + n) == 2 {
            n += 2;
        }
        return n;
    }
    0
}

/// A calendar date, optionally followed by `T`/`_` and a clock time. Returns
/// the end offset plus whether a time part was consumed.
fn match_date(c: &[char], i: usize) -> Option<(usize, bool)> {
    let mut p = i;
    let sep;
    if digits(c, p) == 4 {
        // YYYY-MM-DD / YYYY/MM/DD
        sep = *c.get(p + 4)?;
        if sep != '-' && sep != '/' {
            return None;
        }
        p += 5;
        let m = digits(c, p);
        if m == 0 || m > 2 {
            return None;
        }
        p += m;
        if *c.get(p)? != sep {
            return None;
        }
        p += 1;
        let d = digits(c, p);
        if d == 0 || d > 2 {
            return None;
        }
        p += d;
    } else {
        // DD/MM/YYYY or MM/DD/YYYY
        let a = digits(c, p);
        if a == 0 || a > 2 {
            return None;
        }
        p += a;
        sep = *c.get(p)?;
        if sep != '/' && sep != '-' {
            return None;
        }
        p += 1;
        let b = digits(c, p);
        if b == 0 || b > 2 {
            return None;
        }
        p += b;
        if *c.get(p)? != sep {
            return None;
        }
        p += 1;
        if digits(c, p) != 4 {
            return None;
        }
        p += 4;
    }
    if p < c.len() && (c[p] == 'T' || c[p] == '_') {
        if let Some(end) = match_clock(c, p + 1) {
            return Some((end, true));
        }
    }
    Some((p, false))
}

/// `scheme://…` up to the end of the token, minus trailing punctuation.
fn match_url(c: &[char], i: usize) -> Option<usize> {
    let scheme = run_len(c, i, |ch| ch.is_ascii_alphabetic());
    if scheme < 2 {
        return None;
    }
    let mut p = i + scheme;
    if !(c.get(p) == Some(&':') && c.get(p + 1) == Some(&'/') && c.get(p + 2) == Some(&'/')) {
        return None;
    }
    p += 3;
    while p < c.len() && !c[p].is_whitespace() {
        p += 1;
    }
    while p > i && matches!(c[p - 1], '.' | ',' | ';' | ':' | ')' | ']' | '}' | '"' | '\'') {
        p -= 1;
    }
    Some(p)
}

/// `local@domain.tld`.
fn match_email(c: &[char], i: usize) -> Option<usize> {
    let local = run_len(c, i, |ch| {
        ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '%' | '+' | '-')
    });
    if local == 0 || c.get(i + local) != Some(&'@') {
        return None;
    }
    let mut p = i + local + 1;
    let host = run_len(c, p, |ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '.');
    if host < 3 || !c[p..p + host].contains(&'.') {
        return None;
    }
    p += host;
    while p > i && c[p - 1] == '.' {
        p -= 1;
    }
    Some(p)
}

/// A filesystem path: `/a/b`, `./a`, `../a`, `C:\a\b`.
fn match_path(c: &[char], i: usize) -> Option<usize> {
    let win = c.get(i).is_some_and(|ch| ch.is_ascii_alphabetic())
        && c.get(i + 1) == Some(&':')
        && matches!(c.get(i + 2), Some('\\') | Some('/'));
    let unix = c[i] == '/' || (c[i] == '.' && matches!(c.get(i + 1), Some('/') | Some('.')));
    if !win && !unix {
        return None;
    }
    let mut p = i;
    let mut seps = 0usize;
    while p < c.len() && !c[p].is_whitespace() && c[p] != '"' {
        if c[p] == '/' || c[p] == '\\' {
            seps += 1;
        }
        p += 1;
    }
    while p > i && matches!(c[p - 1], ',' | ';' | ':' | ')' | ']' | '}' | '\'') {
        p -= 1;
    }
    // `/` alone, or `//` from a stray URL remnant, is not a path.
    if seps < 2 && !win {
        return None;
    }
    if p <= i + 1 {
        return None;
    }
    Some(p)
}

/// `0x…` or a long bare hex blob (needs a letter, else it is just a number).
fn match_hex(c: &[char], i: usize) -> Option<usize> {
    if c[i] == '0' && matches!(c.get(i + 1), Some('x') | Some('X')) {
        let n = run_len(c, i + 2, is_hex);
        if n > 0 {
            return Some(i + 2 + n);
        }
        return None;
    }
    let n = run_len(c, i, is_hex);
    if n >= 6
        && c[i..i + n].iter().any(|ch| ch.is_ascii_alphabetic())
        && c[i..i + n].iter().any(|ch| ch.is_ascii_digit())
        && boundary_after(c, i + n)
    {
        return Some(i + n);
    }
    None
}

/// A signed decimal with optional thousands separators, fraction, exponent and
/// unit suffix. Returns the end offset (the suffix is emitted verbatim).
fn match_number(c: &[char], i: usize) -> Option<(usize, usize)> {
    let mut p = i;
    if matches!(c[p], '-' | '+') {
        p += 1;
    }
    let d = digits(c, p);
    if d == 0 {
        return None;
    }
    p += d;
    loop {
        if c.get(p) == Some(&',') && digits(c, p + 1) == 3 {
            p += 4;
        } else {
            break;
        }
    }
    if c.get(p) == Some(&'.') {
        let f = digits(c, p + 1);
        if f > 0 {
            p += 1 + f;
        }
    }
    if matches!(c.get(p), Some('e') | Some('E')) {
        let mut q = p + 1;
        if matches!(c.get(q), Some('-') | Some('+')) {
            q += 1;
        }
        let e = digits(c, q);
        if e > 0 {
            p = q + e;
        }
    }
    let num_end = p;
    if boundary_after(c, p) {
        return Some((p, p));
    }
    // A known unit may follow directly: 250ms, 12kB, 80%.
    let alpha = run_len(c, p, |ch| ch.is_ascii_alphabetic() || ch == '%');
    if alpha > 0 && boundary_after(c, p + alpha) {
        let unit: String = c[p..p + alpha].iter().collect::<String>().to_ascii_lowercase();
        if UNIT_SUFFIXES.contains(&unit.as_str()) {
            return Some((p + alpha, num_end));
        }
    }
    None
}

/// Replace the variable substrings of one token with placeholders.
fn mask_token(tok: &str, mode: Mask) -> String {
    if mode == Mask::None {
        return tok.to_string();
    }
    if tok.starts_with('"') && tok.ends_with('"') && tok.chars().count() >= 2 {
        return mode.ph("<STR>");
    }
    let c: Vec<char> = tok.chars().collect();
    let mut out = String::new();
    let mut i = 0usize;
    while i < c.len() {
        if boundary_before(&c, i) {
            if let Some(end) = match_uuid(&c, i) {
                out.push_str(&mode.ph("<UUID>"));
                i = end;
                continue;
            }
            if let Some((end, has_time)) = match_date(&c, i) {
                out.push_str(&mode.ph(if has_time { "<TIME>" } else { "<DATE>" }));
                i = end;
                continue;
            }
            if let Some(end) = match_ipv4(&c, i) {
                if boundary_after(&c, end) || c[end] == ':' {
                    out.push_str(&mode.ph("<IP>"));
                    i = end;
                    continue;
                }
            }
            if let Some(end) = match_mac(&c, i) {
                out.push_str(&mode.ph("<MAC>"));
                i = end;
                continue;
            }
            if let Some(end) = match_clock(&c, i) {
                out.push_str(&mode.ph("<TIME>"));
                i = end;
                continue;
            }
            if let Some(end) = match_ipv6(&c, i) {
                out.push_str(&mode.ph("<IP>"));
                i = end;
                continue;
            }
            if let Some(end) = match_url(&c, i) {
                out.push_str(&mode.ph("<URL>"));
                i = end;
                continue;
            }
            if let Some(end) = match_email(&c, i) {
                out.push_str(&mode.ph("<EMAIL>"));
                i = end;
                continue;
            }
            if let Some(end) = match_path(&c, i) {
                out.push_str(&mode.ph("<PATH>"));
                i = end;
                continue;
            }
            if let Some(end) = match_hex(&c, i) {
                out.push_str(&mode.ph("<HEX>"));
                i = end;
                continue;
            }
            if let Some((end, num_end)) = match_number(&c, i) {
                out.push_str(&mode.ph("<NUM>"));
                out.extend(c[num_end..end].iter());
                i = end;
                continue;
            }
        }
        out.push(c[i]);
        i += 1;
    }
    out
}

/// Is this template token a placeholder (fully or partly variable)?
fn is_variable(tok: &str) -> bool {
    tok.contains(WILDCARD)
        || (tok.contains('<') && tok.contains('>') && tok.chars().any(|c| c.is_ascii_uppercase()))
}

/// Parse-tree branch key for a token: variable tokens share the `<*>` branch.
fn branch_key(tok: &str) -> String {
    if tok.chars().any(|c| c.is_ascii_digit()) || is_variable(tok) {
        WILDCARD.to_string()
    } else {
        tok.to_string()
    }
}

// ---------------------------------------------------------------------------
// Clustering.
// ---------------------------------------------------------------------------

struct Cluster {
    template: Vec<String>,
    count: usize,
    first_idx: usize,
    last_idx: usize,
    examples: Vec<usize>,
    /// Sample raw token values per variable position, first-seen order.
    vars: Vec<Vec<String>>,
}

#[derive(Default)]
struct Node {
    children: HashMap<String, Node>,
    clusters: Vec<usize>,
}

/// Fraction of positions where the template's literal tokens match the line;
/// `<*>` positions never count as a match (that is the reference behaviour).
fn similarity_of(template: &[String], tokens: &[String]) -> f64 {
    if template.is_empty() {
        return 1.0;
    }
    let mut hits = 0usize;
    for (t, l) in template.iter().zip(tokens.iter()) {
        if t == WILDCARD {
            continue;
        }
        if t == l {
            hits += 1;
        }
    }
    hits as f64 / template.len() as f64
}

/// Count literal disagreements between a template and a line. Placeholder
/// positions are ignored: this keeps low-threshold Drain merging from folding
/// different message verbs together while still letting one changing literal
/// field inside a repeated message family become `<*>`.
fn literal_mismatches(template: &[String], tokens: &[String]) -> usize {
    template
        .iter()
        .zip(tokens.iter())
        .filter(|(t, l)| *t != *l && !is_variable(t) && !is_variable(l))
        .count()
}

fn push_sample(slot: &mut Vec<String>, value: &str) {
    if slot.len() < MAX_SAMPLES && !slot.iter().any(|v| v == value) {
        slot.push(value.to_string());
    }
}

// ---------------------------------------------------------------------------
// Output shapes.
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct VarOut {
    position: usize,
    placeholder: String,
    values: Vec<String>,
}

#[derive(Serialize)]
struct PatternOut {
    rank: usize,
    count: usize,
    percent: f64,
    template: String,
    first_index: usize,
    first_line: String,
    last_index: usize,
    last_line: String,
    examples: Vec<String>,
    variables: Vec<VarOut>,
}

#[derive(Serialize)]
struct SettingsOut {
    similarity: f64,
    depth: u32,
    max_children: u32,
    max_patterns: u32,
    min_count: u32,
    mask: String,
    skip_tokens: u32,
    extra_delimiters: String,
}

#[derive(Serialize)]
struct Report {
    total_lines: usize,
    mined_lines: usize,
    blank_lines: usize,
    patterns_found: usize,
    patterns_shown: usize,
    coverage_percent: f64,
    settings: SettingsOut,
    patterns: Vec<PatternOut>,
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

// ---------------------------------------------------------------------------
// Entry point.
// ---------------------------------------------------------------------------

/// Mine `logs` into ranked message templates.
///
/// * `format` — `"table"` (default), `"json"` or `"lines"`.
/// * `similarity` — merge threshold, 0–1 (default 0.4).
/// * `depth` — parse-tree depth, 2–8 (default 4); `depth - 2` leading tokens
///   are used as tree layers.
/// * `max_children` — branches per tree node before overflow into `<*>`.
/// * `max_patterns` — how many ranked templates to return.
/// * `min_count` — drop templates seen fewer than this many times.
/// * `mask` — `"typed"`, `"wildcard"` or `"none"`.
/// * `extra_delimiters` — extra characters that split tokens.
/// * `skip_tokens` — leading tokens dropped from every line before mining.
#[allow(clippy::too_many_arguments)]
pub fn mine(
    logs: &str,
    format: &str,
    similarity: f64,
    depth: u32,
    max_children: u32,
    max_patterns: u32,
    min_count: u32,
    mask: &str,
    extra_delimiters: &str,
    skip_tokens: u32,
) -> Result<String, String> {
    let format = Format::parse(format)?;
    let mask_mode = Mask::parse(mask)?;
    if !(0.0..=1.0).contains(&similarity) || similarity.is_nan() {
        return Err(format!(
            "similarity must be between 0 and 1 (got {similarity}); the usual value is 0.4"
        ));
    }
    if !(2..=8).contains(&depth) {
        return Err(format!("depth must be between 2 and 8 (got {depth})"));
    }
    if !(2..=1000).contains(&max_children) {
        return Err(format!(
            "max_children must be between 2 and 1000 (got {max_children})"
        ));
    }
    if !(1..=500).contains(&max_patterns) {
        return Err(format!(
            "max_patterns must be between 1 and 500 (got {max_patterns})"
        ));
    }
    if min_count == 0 {
        return Err("min_count must be at least 1".to_string());
    }
    if skip_tokens > 16 {
        return Err(format!("skip_tokens must be between 0 and 16 (got {skip_tokens})"));
    }
    if extra_delimiters.chars().count() > 16 {
        return Err("extra_delimiters accepts at most 16 characters".to_string());
    }
    let extra: Vec<char> = extra_delimiters.chars().filter(|c| !c.is_whitespace()).collect();

    if logs.trim().is_empty() {
        return Err("logs is empty: paste at least one log line".to_string());
    }
    if logs.chars().count() > MAX_CHARS {
        return Err(format!(
            "logs is too long: {} characters, the limit is {MAX_CHARS}",
            logs.chars().count()
        ));
    }
    let raw_lines: Vec<&str> = logs.lines().collect();
    if raw_lines.len() > MAX_LINES {
        return Err(format!(
            "too many lines: {}, the limit is {MAX_LINES}",
            raw_lines.len()
        ));
    }

    let token_layers = depth.saturating_sub(2) as usize;
    let mut root: HashMap<usize, Node> = HashMap::new();
    let mut clusters: Vec<Cluster> = Vec::new();
    let mut mined = 0usize;
    let mut blank = 0usize;

    for (idx, raw) in raw_lines.iter().enumerate() {
        if raw.trim().is_empty() {
            blank += 1;
            continue;
        }
        mined += 1;
        let mut tokens = tokenize(raw, &extra);
        if skip_tokens as usize >= tokens.len() {
            tokens.clear();
        } else {
            tokens.drain(..skip_tokens as usize);
        }
        let masked: Vec<String> = tokens.iter().map(|t| mask_token(t, mask_mode)).collect();

        // Walk the fixed-depth tree down to the leaf for this line.
        let node = root.entry(masked.len()).or_default();
        let mut cur = node;
        for tok in masked.iter().take(token_layers) {
            let key = branch_key(tok);
            if !cur.children.contains_key(&key) && cur.children.len() >= max_children as usize {
                cur = cur.children.entry(WILDCARD.to_string()).or_default();
            } else {
                cur = cur.children.entry(key).or_default();
            }
        }

        // Best matching cluster in this leaf, first one winning a tie.
        let mut best: Option<(usize, f64)> = None;
        for &ci in &cur.clusters {
            let sim = similarity_of(&clusters[ci].template, &masked);
            if best.is_none_or(|(_, b)| sim > b) {
                best = Some((ci, sim));
            }
        }
        match best.filter(|&(ci, sim)| {
            sim >= similarity && literal_mismatches(&clusters[ci].template, &masked) <= 1
        }) {
            Some((ci, _)) => {
                let c = &mut clusters[ci];
                c.count += 1;
                c.last_idx = idx;
                if c.examples.len() < MAX_SAMPLES {
                    c.examples.push(idx);
                }
                for i in 0..c.template.len() {
                    let incoming = &masked[i];
                    if &c.template[i] != incoming {
                        if c.template[i] != WILDCARD && !is_variable(&c.template[i]) {
                            // Seed the slot with the literal it is replacing.
                            let lit = c.template[i].clone();
                            push_sample(&mut c.vars[i], &lit);
                        }
                        c.template[i] = WILDCARD.to_string();
                    }
                    if is_variable(&c.template[i]) {
                        let raw_tok = tokens.get(i).cloned().unwrap_or_default();
                        push_sample(&mut c.vars[i], &raw_tok);
                    }
                }
            }
            None => {
                let vars: Vec<Vec<String>> = masked
                    .iter()
                    .enumerate()
                    .map(|(i, t)| {
                        if is_variable(t) {
                            vec![tokens.get(i).cloned().unwrap_or_default()]
                        } else {
                            Vec::new()
                        }
                    })
                    .collect();
                clusters.push(Cluster {
                    template: masked,
                    count: 1,
                    first_idx: idx,
                    last_idx: idx,
                    examples: vec![idx],
                    vars,
                });
                cur.clusters.push(clusters.len() - 1);
            }
        }
    }

    if mined == 0 {
        return Err("logs is empty: paste at least one log line".to_string());
    }

    let mut order: Vec<usize> = (0..clusters.len()).collect();
    order.sort_by(|&a, &b| {
        clusters[b]
            .count
            .cmp(&clusters[a].count)
            .then(clusters[a].first_idx.cmp(&clusters[b].first_idx))
    });
    let kept: Vec<usize> = order
        .iter()
        .copied()
        .filter(|&ci| clusters[ci].count >= min_count as usize)
        .collect();
    if kept.is_empty() {
        let largest = order.first().map(|&ci| clusters[ci].count).unwrap_or(0);
        return Err(format!(
            "no template reached min_count={min_count}: {} template(s) were found and the largest covers {largest} line(s)",
            clusters.len()
        ));
    }
    let shown: Vec<usize> = kept.iter().copied().take(max_patterns as usize).collect();

    let line_of = |i: usize| raw_lines.get(i).map(|s| s.trim_end().to_string()).unwrap_or_default();
    let pct = |n: usize| round1(n as f64 * 100.0 / mined as f64);

    match format {
        Format::Lines => Ok(shown
            .iter()
            .map(|&ci| clusters[ci].template.join(" "))
            .collect::<Vec<_>>()
            .join("\n")),
        Format::Table => {
            let mut out = String::from("count\tpercent\tfirst\tlast\ttemplate");
            for &ci in &shown {
                let c = &clusters[ci];
                out.push_str(&format!(
                    "\n{}\t{}\t{}\t{}\t{}",
                    c.count,
                    pct(c.count),
                    c.first_idx + 1,
                    c.last_idx + 1,
                    c.template.join(" ")
                ));
            }
            Ok(out)
        }
        Format::Json => {
            let covered: usize = shown.iter().map(|&ci| clusters[ci].count).sum();
            let patterns: Vec<PatternOut> = shown
                .iter()
                .enumerate()
                .map(|(rank, &ci)| {
                    let c = &clusters[ci];
                    let variables = c
                        .template
                        .iter()
                        .enumerate()
                        .filter(|(i, t)| is_variable(t) && !c.vars[*i].is_empty())
                        .map(|(i, t)| VarOut {
                            position: i + 1,
                            placeholder: t.clone(),
                            values: c.vars[i].clone(),
                        })
                        .collect();
                    PatternOut {
                        rank: rank + 1,
                        count: c.count,
                        percent: pct(c.count),
                        template: c.template.join(" "),
                        first_index: c.first_idx + 1,
                        first_line: line_of(c.first_idx),
                        last_index: c.last_idx + 1,
                        last_line: line_of(c.last_idx),
                        examples: c.examples.iter().map(|&i| line_of(i)).collect(),
                        variables,
                    }
                })
                .collect();
            let report = Report {
                total_lines: raw_lines.len(),
                mined_lines: mined,
                blank_lines: blank,
                patterns_found: kept.len(),
                patterns_shown: shown.len(),
                coverage_percent: pct(covered),
                settings: SettingsOut {
                    similarity,
                    depth,
                    max_children,
                    max_patterns,
                    min_count,
                    mask: match mask_mode {
                        Mask::Typed => "typed".into(),
                        Mask::Wildcard => "wildcard".into(),
                        Mask::None => "none".into(),
                    },
                    skip_tokens,
                    extra_delimiters: extra_delimiters.to_string(),
                },
                patterns,
            };
            serde_json::to_string_pretty(&report).map_err(|e| format!("could not render JSON: {e}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SSH: &str = "Jan 12 03:04:05 web1 sshd[2311]: Failed password for root from 10.0.0.1 port 51234 ssh2\n\
Jan 12 03:04:07 web1 sshd[2312]: Failed password for admin from 10.0.0.2 port 51999 ssh2\n\
Jan 12 03:04:09 web1 sshd[2313]: Failed password for root from 10.0.0.3 port 52001 ssh2\n\
Jan 12 03:05:00 web1 sshd[2400]: Accepted publickey for deploy from 10.0.0.9 port 60001 ssh2";

    fn mine_default(logs: &str, format: &str) -> String {
        mine(logs, format, 0.4, 4, 100, 20, 1, "typed", "", 0).unwrap()
    }

    #[test]
    fn clusters_ssh_lines_into_two_templates() {
        let out = mine_default(SSH, "table");
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "count\tpercent\tfirst\tlast\ttemplate");
        assert_eq!(lines.len(), 3, "two templates expected, got: {out}");
        assert_eq!(
            lines[1],
            "3\t75\t1\t3\tJan <NUM> <TIME> web1 sshd[<NUM>]: Failed password for <*> from <IP> port <NUM> ssh2"
        );
        assert!(lines[2].starts_with("1\t25\t4\t4\tJan <NUM> <TIME> web1 sshd[<NUM>]: Accepted publickey"));
    }

    #[test]
    fn json_reports_counts_examples_and_variables() {
        let out = mine_default(SSH, "json");
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["total_lines"], 4);
        assert_eq!(v["mined_lines"], 4);
        assert_eq!(v["patterns_found"], 2);
        assert_eq!(v["coverage_percent"], 100.0);
        let p0 = &v["patterns"][0];
        assert_eq!(p0["count"], 3);
        assert_eq!(p0["percent"], 75.0);
        assert_eq!(p0["first_index"], 1);
        assert_eq!(p0["last_index"], 3);
        assert_eq!(p0["examples"].as_array().unwrap().len(), 3);
        assert!(p0["first_line"].as_str().unwrap().contains("10.0.0.1"));
        let vars = p0["variables"].as_array().unwrap();
        let ip_slot = vars
            .iter()
            .find(|s| s["placeholder"] == "<IP>")
            .expect("an <IP> slot");
        assert_eq!(
            ip_slot["values"],
            serde_json::json!(["10.0.0.1", "10.0.0.2", "10.0.0.3"])
        );
        let user_slot = vars.iter().find(|s| s["placeholder"] == "<*>").unwrap();
        assert_eq!(user_slot["values"], serde_json::json!(["root", "admin"]));
    }

    #[test]
    fn lines_format_returns_one_template_per_line() {
        let out = mine_default("get user 1\nget user 2\nget user 3", "lines");
        assert_eq!(out, "get user <NUM>");
    }

    #[test]
    fn masks_typed_values() {
        let logs = "id 550e8400-e29b-41d4-a716-446655440000 ip 192.168.1.7 mac de:ad:be:ef:00:01 \
hex 0xdeadbeef date 2024-05-06 ts 2024-05-06T07:08:09.123Z clock 07:08:09 path /var/log/app.log \
url https://example.com/a?b=1 mail ops@example.com num 250ms text \"hello there\"";
        let out = mine_default(logs, "lines");
        assert_eq!(
            out,
            "id <UUID> ip <IP> mac <MAC> hex <HEX> date <DATE> ts <TIME> clock <TIME> path <PATH> \
url <URL> mail <EMAIL> num <NUM>ms text <STR>"
        );
    }

    #[test]
    fn wildcard_and_none_mask_modes() {
        let logs = "worker 7 done in 12ms";
        let wild = mine(logs, "lines", 0.4, 4, 100, 20, 1, "wildcard", "", 0).unwrap();
        assert_eq!(wild, "worker <*> done in <*>ms");
        let none = mine(logs, "lines", 0.4, 4, 100, 20, 1, "none", "", 0).unwrap();
        assert_eq!(none, "worker 7 done in 12ms");
    }

    #[test]
    fn skip_tokens_drops_the_line_prefix() {
        let logs = "2024-05-06 07:08:09 INFO cache warm\n2024-05-07 09:10:11 INFO cache warm";
        let kept = mine_default(logs, "lines");
        assert_eq!(kept, "<DATE> <TIME> INFO cache warm");
        let skipped = mine(logs, "lines", 0.4, 4, 100, 20, 1, "typed", "", 2).unwrap();
        assert_eq!(skipped, "INFO cache warm");
    }

    #[test]
    fn extra_delimiters_split_key_value_pairs() {
        let logs = "req=1 status=200\nreq=2 status=500";
        let joined = mine_default(logs, "lines");
        assert_eq!(joined, "req=<NUM> status=<NUM>");
        let split = mine(logs, "lines", 0.4, 4, 100, 20, 1, "typed", "=", 0).unwrap();
        assert_eq!(split, "req <NUM> status <NUM>");
    }

    #[test]
    fn min_count_and_max_patterns_filter_the_ranking() {
        let logs = "a 1\na 2\na 3\nb 1\nc 1";
        let all = mine_default(logs, "lines");
        assert_eq!(all.lines().count(), 3);
        let top1 = mine(logs, "lines", 0.4, 4, 100, 1, 1, "typed", "", 0).unwrap();
        assert_eq!(top1, "a <NUM>");
        let frequent = mine(logs, "lines", 0.4, 4, 100, 20, 2, "typed", "", 0).unwrap();
        assert_eq!(frequent, "a <NUM>");
    }

    #[test]
    fn blank_lines_are_skipped_not_mined() {
        let v: serde_json::Value =
            serde_json::from_str(&mine_default("ping ok\n\n\nping ok", "json")).unwrap();
        assert_eq!(v["total_lines"], 4);
        assert_eq!(v["mined_lines"], 2);
        assert_eq!(v["blank_lines"], 2);
        assert_eq!(v["patterns_found"], 1);
    }

    #[test]
    fn a_high_threshold_keeps_distinct_messages_apart() {
        let logs = "user action login ok\nuser action logout ok";
        let loose = mine(logs, "lines", 0.4, 4, 100, 20, 1, "typed", "", 0).unwrap();
        assert_eq!(loose.lines().count(), 1);
        let strict = mine(logs, "lines", 0.9, 4, 100, 20, 1, "typed", "", 0).unwrap();
        assert_eq!(strict.lines().count(), 2);
    }

    #[test]
    fn rejects_empty_input() {
        let err = mine("   \n\n", "table", 0.4, 4, 100, 20, 1, "typed", "", 0).unwrap_err();
        assert!(err.contains("logs is empty"), "got: {err}");
    }

    #[test]
    fn rejects_bad_options() {
        assert!(mine("x", "yaml", 0.4, 4, 100, 20, 1, "typed", "", 0)
            .unwrap_err()
            .contains("invalid format"));
        assert!(mine("x", "table", 0.4, 4, 100, 20, 1, "regex", "", 0)
            .unwrap_err()
            .contains("invalid mask"));
        assert!(mine("x", "table", 1.5, 4, 100, 20, 1, "typed", "", 0)
            .unwrap_err()
            .contains("similarity must be between 0 and 1"));
        assert!(mine("x", "table", 0.4, 9, 100, 20, 1, "typed", "", 0)
            .unwrap_err()
            .contains("depth must be between 2 and 8"));
        assert!(mine("x", "table", 0.4, 4, 1, 20, 1, "typed", "", 0)
            .unwrap_err()
            .contains("max_children"));
        assert!(mine("x", "table", 0.4, 4, 100, 0, 1, "typed", "", 0)
            .unwrap_err()
            .contains("max_patterns"));
        assert!(mine("x", "table", 0.4, 4, 100, 20, 0, "typed", "", 0)
            .unwrap_err()
            .contains("min_count"));
        assert!(mine("x", "table", 0.4, 4, 100, 20, 1, "typed", "", 99)
            .unwrap_err()
            .contains("skip_tokens"));
    }

    #[test]
    fn rejects_a_min_count_no_template_reaches() {
        let err = mine("a 1\nb 2", "table", 0.4, 4, 100, 20, 5, "typed", "", 0).unwrap_err();
        assert!(err.contains("no template reached min_count=5"), "got: {err}");
        assert!(err.contains("largest covers 1 line"), "got: {err}");
    }

    #[test]
    fn rejects_input_over_the_line_limit() {
        let logs = "x\n".repeat(MAX_LINES + 1);
        let err = mine(&logs, "table", 0.4, 4, 100, 20, 1, "typed", "", 0).unwrap_err();
        assert!(err.contains("too many lines"), "got: {err}");
    }

    #[test]
    fn is_deterministic_across_runs() {
        let a = mine_default(SSH, "json");
        let b = mine_default(SSH, "json");
        assert_eq!(a, b);
    }
}

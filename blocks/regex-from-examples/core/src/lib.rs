//! gizza-ai/regex-from-examples core — infer a regular expression that matches a
//! set of example strings, and (optionally) rejects a set of counter-examples.
//!
//! Deterministic structural inference, NOT machine learning. Two engines:
//!
//!   * **generalize** — each example is tokenised into runs of one character class
//!     (digits / lowercase / uppercase / whitespace / a repeated punctuation char).
//!     Examples that share a token-class *shape* are merged position by position:
//!     classes union (`a` + `A` → `[A-Za-z]`), punctuation unions into a set
//!     (`-` + `/` → `[-/]`), and run lengths become `{m,n}` quantifiers. Several
//!     shapes become an alternation of merged shapes.
//!   * **alternation** — a prefix trie over the literal examples, rendered with the
//!     common prefix hoisted, siblings whose continuations are identical folded into
//!     one class (`cat`, `cot`, `cut` → `c[aou]t`) and an early terminal rendered as
//!     `?` (`foobar`, `foobaz`, `fooza`, `foozap` → `foo(?:ba[rz]|zap?)`).
//!
//! Every candidate is COMPILED and RUN against the samples before it is returned:
//! a pattern is only accepted if it matches every example and matches no negative.
//! `strategy = "auto"` walks generalize → tighter quantifiers → literal alternation
//! until one passes, so the returned pattern is verified rather than merely plausible.
//! Auto tries the folded literals FIRST in the one case where generalizing is a leap
//! it cannot justify: the examples agree on every position but one, and the characters
//! at that position are scattered rather than a contiguous run.
//!
//! Shared by the chat skill block, the CLI and the web page.

use regex::RegexBuilder;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// Hard cap on the combined size of the examples + negatives text.
pub const MAX_INPUT_CHARS: usize = 200_000;
/// Hard cap on how many examples (or negatives) a single run accepts.
pub const MAX_EXAMPLES: usize = 5_000;

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// How the examples text is split into individual examples.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Separator {
    Newline,
    Comma,
    Tab,
    Semicolon,
    Space,
}

/// Which inference engine to use.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Strategy {
    Auto,
    Generalize,
    Alternation,
    CharacterClass,
}

/// How observed run lengths become quantifiers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Quantifiers {
    /// `{m,n}` from the shortest/longest run observed (`{n}` when they agree).
    Range,
    /// `{m,}` — at least the shortest run observed, no upper bound.
    Open,
    /// `+` / `*` / `?` — ignore the observed lengths entirely.
    Loose,
}

/// Target regex dialect. Affects pattern SYNTAX only (never host-language code).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Flavor {
    Rust,
    Pcre,
    Python,
    Javascript,
    Posix,
}

/// What the tool returns.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Output {
    Pattern,
    Json,
    Report,
}

fn one_of(name: &str, got: &str, allowed: &[&str]) -> String {
    format!(
        "{name}: expected one of {}, got '{got}'",
        allowed
            .iter()
            .map(|a| format!("'{a}'"))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

impl Separator {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "newline" => Ok(Self::Newline),
            "comma" => Ok(Self::Comma),
            "tab" => Ok(Self::Tab),
            "semicolon" => Ok(Self::Semicolon),
            "space" => Ok(Self::Space),
            other => Err(one_of(
                "separator",
                other,
                &["newline", "comma", "tab", "semicolon", "space"],
            )),
        }
    }
}

impl Strategy {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "auto" => Ok(Self::Auto),
            "generalize" => Ok(Self::Generalize),
            "alternation" => Ok(Self::Alternation),
            "character-class" => Ok(Self::CharacterClass),
            other => Err(one_of(
                "strategy",
                other,
                &["auto", "generalize", "alternation", "character-class"],
            )),
        }
    }
}

impl Quantifiers {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "range" => Ok(Self::Range),
            "open" => Ok(Self::Open),
            "loose" => Ok(Self::Loose),
            other => Err(one_of("quantifiers", other, &["range", "open", "loose"])),
        }
    }
}

impl Flavor {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "rust" => Ok(Self::Rust),
            "pcre" => Ok(Self::Pcre),
            "python" => Ok(Self::Python),
            "javascript" => Ok(Self::Javascript),
            "posix" => Ok(Self::Posix),
            other => Err(one_of(
                "flavor",
                other,
                &["rust", "pcre", "python", "javascript", "posix"],
            )),
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Pcre => "pcre",
            Self::Python => "python",
            Self::Javascript => "javascript",
            Self::Posix => "posix",
        }
    }
}

impl Output {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "pattern" => Ok(Self::Pattern),
            "json" => Ok(Self::Json),
            "report" => Ok(Self::Report),
            other => Err(one_of("output", other, &["pattern", "json", "report"])),
        }
    }
}

/// Everything that shapes the emitted pattern.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub separator: Separator,
    pub strategy: Strategy,
    pub quantifiers: Quantifiers,
    pub flavor: Flavor,
    pub anchors: bool,
    pub case_insensitive: bool,
    pub capture_groups: bool,
    pub max_alternatives: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            separator: Separator::Newline,
            strategy: Strategy::Auto,
            quantifiers: Quantifiers::Range,
            flavor: Flavor::Rust,
            anchors: true,
            case_insensitive: false,
            capture_groups: false,
            max_alternatives: 50,
        }
    }
}

// ---------------------------------------------------------------------------
// Result
// ---------------------------------------------------------------------------

/// The full inference result, serialised for the `json` output and the chat/CLI surface.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Inference {
    /// The inferred pattern, rendered in the requested flavor (anchors + flags applied).
    pub pattern: String,
    /// The same pattern in Rust `regex` syntax — this is what was compiled to verify it.
    pub verified_with: String,
    /// Which engine produced it: generalize, shape-alternation, alternation, character-class.
    pub strategy: String,
    pub flavor: String,
    pub anchored: bool,
    pub case_insensitive: bool,
    pub example_count: usize,
    pub distinct_examples: usize,
    pub negative_count: usize,
    /// How many of the examples the pattern actually matches (should equal `example_count`).
    pub examples_matched: usize,
    /// How many negatives the pattern correctly rejects.
    pub negatives_excluded: usize,
    /// Negatives the pattern still matches — empty means the pattern is fully verified.
    pub negatives_still_matching: Vec<String>,
    /// One entry per merged token shape, as a readable signature.
    pub shapes: Vec<String>,
    /// Plain-English breakdown of the pattern, one line per element.
    pub explanation: Vec<String>,
    /// Anything the caller should know (escalations, fallbacks, flavor caveats).
    pub notes: Vec<String>,
}

// ---------------------------------------------------------------------------
// Tokenising
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Kind {
    Digit,
    Lower,
    Upper,
    Space,
    Punct,
}

impl Kind {
    fn tag(self) -> &'static str {
        match self {
            Self::Digit => "digit",
            Self::Lower => "lower",
            Self::Upper => "upper",
            Self::Space => "space",
            Self::Punct => "punct",
        }
    }
}

/// One merged run: which character classes may appear, which literal characters
/// may appear, and the shortest/longest run length observed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Tok {
    digit: bool,
    lower: bool,
    upper: bool,
    space: bool,
    lits: BTreeSet<char>,
    min: usize,
    max: usize,
}

impl Tok {
    fn is_class(&self) -> bool {
        self.digit || self.lower || self.upper || self.space || self.lits.len() > 1
    }
    fn merge(&mut self, other: &Tok) {
        self.digit |= other.digit;
        self.lower |= other.lower;
        self.upper |= other.upper;
        self.space |= other.space;
        for c in &other.lits {
            self.lits.insert(*c);
        }
        self.min = self.min.min(other.min);
        self.max = self.max.max(other.max);
    }
}

fn classify(c: char) -> Kind {
    if c.is_ascii_digit() {
        Kind::Digit
    } else if c.is_ascii_lowercase() {
        Kind::Lower
    } else if c.is_ascii_uppercase() {
        Kind::Upper
    } else if c.is_whitespace() {
        Kind::Space
    } else {
        Kind::Punct
    }
}

/// Split one example into class runs. Class runs (digits, letters, whitespace)
/// absorb any number of same-class characters; a punctuation run only absorbs
/// repeats of the SAME character, so `-` and `/` stay distinguishable.
fn tokenize(s: &str) -> Vec<(Kind, Tok)> {
    let chars: Vec<char> = s.chars().collect();
    let mut out: Vec<(Kind, Tok)> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let kind = classify(c);
        let mut n = 1;
        while i + n < chars.len() {
            let d = chars[i + n];
            let same = if kind == Kind::Punct {
                d == c
            } else {
                classify(d) == kind
            };
            if !same {
                break;
            }
            n += 1;
        }
        let mut t = Tok {
            min: n,
            max: n,
            ..Default::default()
        };
        match kind {
            Kind::Digit => t.digit = true,
            Kind::Lower => t.lower = true,
            Kind::Upper => t.upper = true,
            Kind::Space => t.space = true,
            Kind::Punct => {
                t.lits.insert(c);
            }
        }
        out.push((kind, t));
        i += n;
    }
    out
}

fn signature(toks: &[(Kind, Tok)]) -> Vec<Kind> {
    toks.iter().map(|(k, _)| *k).collect()
}

fn signature_label(sig: &[Kind]) -> String {
    if sig.is_empty() {
        "(empty)".into()
    } else {
        sig.iter().map(|k| k.tag()).collect::<Vec<_>>().join("+")
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct Ctx {
    flavor: Flavor,
    quant: Quantifiers,
    ci: bool,
    groups: bool,
}

fn case_pair(c: char) -> Option<char> {
    if c.is_ascii_lowercase() {
        Some(c.to_ascii_uppercase())
    } else if c.is_ascii_uppercase() {
        Some(c.to_ascii_lowercase())
    } else {
        None
    }
}

/// Group opener: POSIX ERE has no non-capturing group.
fn group_open(ctx: &Ctx) -> &'static str {
    if ctx.flavor == Flavor::Posix {
        "("
    } else {
        "(?:"
    }
}

fn wrap(inner: &str, ctx: &Ctx) -> String {
    format!("{}{}{}", group_open(ctx), inner, ")")
}

/// Escape one character for use OUTSIDE a bracket expression.
fn esc_lit(c: char, ctx: &Ctx) -> String {
    const META: &str = ".^$*+?()[]{}|\\";
    if META.contains(c) {
        return format!("\\{c}");
    }
    if ctx.flavor == Flavor::Javascript && c == '/' {
        return "\\/".into();
    }
    if ctx.flavor != Flavor::Posix {
        match c {
            '\n' => return "\\n".into(),
            '\t' => return "\\t".into(),
            '\r' => return "\\r".into(),
            _ => {}
        }
    }
    c.to_string()
}

/// Escape one character for use INSIDE a bracket expression. POSIX bracket
/// expressions treat `\` literally, so there the special characters are placed
/// positionally instead (see `bracket`).
fn esc_in_class(c: char, ctx: &Ctx) -> String {
    if ctx.flavor == Flavor::Posix {
        return c.to_string();
    }
    match c {
        '\\' | ']' | '^' | '-' | '[' => format!("\\{c}"),
        '\n' => "\\n".into(),
        '\t' => "\\t".into(),
        '\r' => "\\r".into(),
        _ => c.to_string(),
    }
}

/// Render a sorted literal set as bracket-expression contents, compacting runs
/// of three or more consecutive code points into `a-c` ranges. POSIX ordering
/// rules are honoured: `]` first, `^` never first, `-` last.
fn lits_body(lits: &BTreeSet<char>, ctx: &Ctx, ranges_ok: bool) -> String {
    let mut chars: Vec<char> = lits.iter().copied().collect();
    chars.sort_unstable();
    let mut lead = String::new();
    let mut tail = String::new();
    if ctx.flavor == Flavor::Posix {
        if let Some(p) = chars.iter().position(|c| *c == ']') {
            chars.remove(p);
            lead.push(']');
        }
        if let Some(p) = chars.iter().position(|c| *c == '-') {
            chars.remove(p);
            tail.push('-');
        }
        if let Some(p) = chars.iter().position(|c| *c == '^') {
            chars.remove(p);
            tail.insert(0, '^');
        }
    }
    let mut body = String::new();
    let mut i = 0;
    while i < chars.len() {
        let mut j = i;
        while j + 1 < chars.len() && (chars[j + 1] as u32) == (chars[j] as u32) + 1 {
            j += 1;
        }
        let run = j - i + 1;
        if ranges_ok && run >= 3 {
            body.push_str(&esc_in_class(chars[i], ctx));
            body.push('-');
            body.push_str(&esc_in_class(chars[j], ctx));
            i = j + 1;
        } else {
            body.push_str(&esc_in_class(chars[i], ctx));
            i += 1;
        }
    }
    format!("{lead}{body}{tail}")
}

/// Render the character-matching part of a token (no quantifier).
fn tok_body(t: &Tok, ctx: &Ctx) -> String {
    let mut t = t.clone();
    // POSIX ERE has no inline `(?i)`, so case-insensitivity is baked into the
    // character sets instead.
    if ctx.ci && ctx.flavor == Flavor::Posix {
        if t.lower || t.upper {
            t.lower = true;
            t.upper = true;
        }
        let extra: Vec<char> = t.lits.iter().filter_map(|c| case_pair(*c)).collect();
        for c in extra {
            t.lits.insert(c);
        }
    }

    let classes = [t.digit, t.lower, t.upper, t.space]
        .iter()
        .filter(|b| **b)
        .count();

    // A single literal character stays a plain literal.
    if classes == 0 && t.lits.len() == 1 {
        let c = *t.lits.iter().next().unwrap();
        return esc_lit(c, ctx);
    }
    if classes == 0 {
        return format!("[{}]", lits_body(&t.lits, ctx, true));
    }
    if ctx.flavor != Flavor::Posix && t.lits.is_empty() {
        if classes == 1 {
            if t.digit {
                return "\\d".into();
            }
            if t.space {
                return "\\s".into();
            }
        }
        if t.lower && t.upper && !t.digit && !t.space {
            return "[A-Za-z]".into();
        }
    }
    if ctx.flavor == Flavor::Posix && t.lits.is_empty() && classes == 1 {
        if t.digit {
            return "[0-9]".into();
        }
        if t.space {
            return "[[:space:]]".into();
        }
    }
    let mut body = String::new();
    if t.upper {
        body.push_str("A-Z");
    }
    if t.lower {
        body.push_str("a-z");
    }
    if t.digit {
        body.push_str("0-9");
    }
    if t.space {
        body.push_str(if ctx.flavor == Flavor::Posix {
            "[:space:]"
        } else {
            "\\s"
        });
    }
    body.push_str(&lits_body(&t.lits, ctx, true));
    format!("[{body}]")
}

fn quantifier(t: &Tok, ctx: &Ctx) -> String {
    match ctx.quant {
        Quantifiers::Loose => {
            if t.min == 1 && t.max == 1 {
                String::new()
            } else if t.min == 0 && t.max == 1 {
                "?".into()
            } else if t.min == 0 {
                "*".into()
            } else {
                "+".into()
            }
        }
        Quantifiers::Open => {
            if t.min == 1 && t.max == 1 {
                String::new()
            } else if t.min == 0 {
                "*".into()
            } else if t.min == 1 {
                "+".into()
            } else {
                format!("{{{},}}", t.min)
            }
        }
        Quantifiers::Range => {
            if t.min == 1 && t.max == 1 {
                String::new()
            } else if t.min == t.max {
                format!("{{{}}}", t.min)
            } else {
                format!("{{{},{}}}", t.min, t.max)
            }
        }
    }
}

/// `true` when `s` is a single regex atom that a quantifier can follow directly.
fn is_atomic(s: &str) -> bool {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() == 1 {
        return true;
    }
    if chars.len() == 2 && chars[0] == '\\' {
        return true;
    }
    if chars.first() == Some(&'[') && chars.last() == Some(&']') {
        // Atomic only if the class closes exactly once, at the end.
        let mut i = 1;
        let mut esc = false;
        while i < chars.len() - 1 {
            if esc {
                esc = false;
            } else if chars[i] == '\\' {
                esc = true;
            } else if chars[i] == ']' && i > 1 {
                return false;
            }
            i += 1;
        }
        return true;
    }
    false
}

fn render_tok(t: &Tok, ctx: &Ctx) -> String {
    let body = tok_body(t, ctx);
    let q = quantifier(t, ctx);
    let atom = if q.is_empty() || is_atomic(&body) {
        body
    } else {
        wrap(&body, ctx)
    };
    let piece = format!("{atom}{q}");
    if ctx.groups && t.is_class() {
        format!("({piece})")
    } else {
        piece
    }
}

fn render_shape(toks: &[Tok], ctx: &Ctx) -> String {
    toks.iter().map(|t| render_tok(t, ctx)).collect()
}

// ---------------------------------------------------------------------------
// Literal-alternation engine (prefix trie)
// ---------------------------------------------------------------------------

#[derive(Default, PartialEq, Eq)]
struct Node {
    terminal: bool,
    kids: BTreeMap<char, Node>,
}

fn trie_insert(root: &mut Node, s: &str) {
    let mut cur = root;
    for c in s.chars() {
        cur = cur.kids.entry(c).or_default();
    }
    cur.terminal = true;
}

/// Render one child character as the head of a branch.
fn trie_head(c: char, ctx: &Ctx) -> String {
    if ctx.ci && ctx.flavor == Flavor::Posix && case_pair(c).is_some() {
        let mut t = Tok {
            min: 1,
            max: 1,
            ..Default::default()
        };
        t.lits.insert(c);
        tok_body(&t, ctx)
    } else {
        esc_lit(c, ctx)
    }
}

fn trie_render(n: &Node, ctx: &Ctx) -> String {
    if n.kids.is_empty() {
        return String::new();
    }
    // Children whose subtrees are IDENTICAL differ only in their own character,
    // so they fold into one class in front of the shared continuation:
    // `cat`+`cot`+`cut` → `c[aou]t`. Sibling leaves are the degenerate case of
    // this (their shared continuation is empty), which folds `bar`+`baz` → `ba[rz]`.
    let mut groups: Vec<(BTreeSet<char>, &Node)> = Vec::new();
    for (c, kid) in &n.kids {
        match groups.iter_mut().find(|(_, seen)| *seen == kid) {
            Some((chars, _)) => {
                chars.insert(*c);
            }
            None => groups.push((BTreeSet::from([*c]), kid)),
        }
    }
    let mut branches: Vec<String> = groups
        .into_iter()
        .map(|(chars, kid)| {
            let mut head = if chars.len() > 1 {
                let t = Tok {
                    min: 1,
                    max: 1,
                    lits: chars,
                    ..Default::default()
                };
                tok_body(&t, ctx)
            } else {
                trie_head(*chars.iter().next().unwrap(), ctx)
            };
            head.push_str(&trie_render(kid, ctx));
            head
        })
        .collect();
    if branches.len() > 1 {
        // Longest branch first: leftmost-first engines must not settle for a
        // shorter alternative when a longer one also matches.
        branches.sort_by(|a, b| b.chars().count().cmp(&a.chars().count()).then(a.cmp(b)));
    }
    let joined = if branches.len() == 1 {
        branches.remove(0)
    } else {
        wrap(&branches.join("|"), ctx)
    };
    if n.terminal {
        if is_atomic(&joined) {
            format!("{joined}?")
        } else {
            format!("{}?", wrap(&joined, ctx))
        }
    } else {
        joined
    }
}

// ---------------------------------------------------------------------------
// Explanation
// ---------------------------------------------------------------------------

fn count_phrase(t: &Tok, ctx: &Ctx) -> String {
    match ctx.quant {
        Quantifiers::Loose => {
            if t.min == 1 && t.max == 1 {
                "one".into()
            } else if t.min == 0 {
                "any number of".into()
            } else {
                "one or more".into()
            }
        }
        Quantifiers::Open => {
            if t.min == 1 && t.max == 1 {
                "one".into()
            } else {
                format!("{} or more", t.min)
            }
        }
        Quantifiers::Range => {
            if t.min == t.max {
                if t.min == 1 {
                    "one".into()
                } else {
                    format!("exactly {}", t.min)
                }
            } else {
                format!("{} to {}", t.min, t.max)
            }
        }
    }
}

fn noun(t: &Tok) -> String {
    let mut parts: Vec<String> = Vec::new();
    if t.digit {
        parts.push("digit".into());
    }
    if t.lower && t.upper {
        parts.push("letter".into());
    } else if t.lower {
        parts.push("lowercase letter".into());
    } else if t.upper {
        parts.push("uppercase letter".into());
    }
    if t.space {
        parts.push("whitespace character".into());
    }
    if !t.lits.is_empty() {
        let shown: String = t.lits.iter().take(8).collect();
        let more = if t.lits.len() > 8 { "…" } else { "" };
        parts.push(format!("character from \"{shown}{more}\""));
    }
    if parts.is_empty() {
        "character".into()
    } else {
        parts.join(" or ")
    }
}

fn explain_tok(t: &Tok, ctx: &Ctx) -> String {
    let rendered = render_tok(t, ctx);
    if !t.is_class() && t.lits.len() == 1 {
        let c = *t.lits.iter().next().unwrap();
        let times = count_phrase(t, ctx);
        return format!("{rendered}\tliteral \"{c}\" ({times})");
    }
    format!("{rendered}\t{} {}", count_phrase(t, ctx), plural(&noun(t), t))
}

fn plural(n: &str, t: &Tok) -> String {
    let one = t.min == 1 && t.max == 1;
    if one || n.ends_with('"') {
        n.to_string()
    } else {
        format!("{n}s")
    }
}

// ---------------------------------------------------------------------------
// Inference
// ---------------------------------------------------------------------------

fn split_examples(text: &str, sep: Separator) -> Vec<String> {
    let pieces: Vec<&str> = match sep {
        Separator::Newline => text.split('\n').collect(),
        Separator::Comma => text.split(',').collect(),
        Separator::Tab => text.split('\t').collect(),
        Separator::Semicolon => text.split(';').collect(),
        Separator::Space => text.split_whitespace().collect(),
    };
    pieces
        .into_iter()
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

fn dedup(items: &[String]) -> Vec<String> {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut out = Vec::new();
    for i in items {
        if seen.insert(i.as_str()) {
            out.push(i.clone());
        }
    }
    out
}

/// Group the examples by token-class shape and merge each group position by position.
fn shapes_of(examples: &[String]) -> Vec<(Vec<Kind>, Vec<Tok>)> {
    let mut order: Vec<Vec<Kind>> = Vec::new();
    let mut merged: BTreeMap<Vec<Kind>, Vec<Tok>> = BTreeMap::new();
    for ex in examples {
        let toks = tokenize(ex);
        let sig = signature(&toks);
        let plain: Vec<Tok> = toks.into_iter().map(|(_, t)| t).collect();
        match merged.get_mut(&sig) {
            Some(existing) => {
                for (slot, t) in existing.iter_mut().zip(plain.iter()) {
                    slot.merge(t);
                }
            }
            None => {
                order.push(sig.clone());
                merged.insert(sig, plain);
            }
        }
    }
    order
        .into_iter()
        .map(|sig| {
            let toks = merged.get(&sig).cloned().unwrap_or_default();
            (sig, toks)
        })
        .collect()
}

/// Union every character of every example into one token spanning the observed lengths.
fn char_class_tok(examples: &[String]) -> Tok {
    let mut t = Tok {
        min: usize::MAX,
        max: 0,
        ..Default::default()
    };
    for ex in examples {
        let n = ex.chars().count();
        t.min = t.min.min(n);
        t.max = t.max.max(n);
        for c in ex.chars() {
            match classify(c) {
                Kind::Digit => t.digit = true,
                Kind::Lower => t.lower = true,
                Kind::Upper => t.upper = true,
                Kind::Space => t.space = true,
                Kind::Punct => {
                    t.lits.insert(c);
                }
            }
        }
    }
    if t.min == usize::MAX {
        t.min = 0;
    }
    t
}

/// The characters of the single position the distinct examples disagree on, when
/// they are all the same length and agree everywhere else (`cat`/`cot`/`cut` → `aou`).
/// Such a set is closed: the alternation engine folds it into one character class
/// that matches the examples EXACTLY — no generalization, nothing else admitted.
fn one_varying_position(distinct: &[String]) -> Option<BTreeSet<char>> {
    if distinct.len() < 2 {
        return None;
    }
    let rows: Vec<Vec<char>> = distinct.iter().map(|e| e.chars().collect()).collect();
    let len = rows[0].len();
    if rows.iter().any(|r| r.len() != len) {
        return None;
    }
    let mut varying: Option<BTreeSet<char>> = None;
    for i in 0..len {
        let seen: BTreeSet<char> = rows.iter().map(|r| r[i]).collect();
        if seen.len() > 1 {
            if varying.is_some() {
                return None;
            }
            varying = Some(seen);
        }
    }
    varying
}

/// `true` when the characters form one unbroken run of code points (`1`,`2`,`3`).
fn is_contiguous(chars: &BTreeSet<char>) -> bool {
    chars
        .iter()
        .zip(chars.iter().skip(1))
        .all(|(a, b)| *b as u32 == *a as u32 + 1)
}

struct Candidate {
    strategy: &'static str,
    body: String,
    shapes: Vec<String>,
    explanation: Vec<String>,
}

fn build_generalized(
    examples: &[String],
    ctx: &Ctx,
    max_alternatives: usize,
) -> Result<Candidate, String> {
    let shapes = shapes_of(examples);
    if shapes.len() > max_alternatives {
        return Err(format!(
            "the examples fall into {} different token shapes, more than max_alternatives={}; raise max_alternatives, split the list, or use strategy='character-class'",
            shapes.len(),
            max_alternatives
        ));
    }
    let mut rendered = Vec::new();
    let mut labels = Vec::new();
    let mut explanation = Vec::new();
    for (sig, toks) in &shapes {
        rendered.push(render_shape(toks, ctx));
        labels.push(signature_label(sig));
        if shapes.len() > 1 {
            explanation.push(format!("--\tshape {}", signature_label(sig)));
        }
        for t in toks {
            explanation.push(explain_tok(t, ctx));
        }
    }
    let (strategy, body) = if rendered.len() == 1 {
        ("generalize", rendered.remove(0))
    } else {
        ("shape-alternation", wrap(&rendered.join("|"), ctx))
    };
    Ok(Candidate {
        strategy,
        body,
        shapes: labels,
        explanation,
    })
}

fn build_alternation(
    examples: &[String],
    ctx: &Ctx,
    max_alternatives: usize,
) -> Result<Candidate, String> {
    let distinct = dedup(examples);
    if distinct.len() > max_alternatives {
        return Err(format!(
            "literal alternation needs one branch per distinct example: {} distinct examples exceed max_alternatives={}; raise max_alternatives or use strategy='generalize'",
            distinct.len(),
            max_alternatives
        ));
    }
    let mut root = Node::default();
    for ex in &distinct {
        trie_insert(&mut root, ex);
    }
    let body = trie_render(&root, ctx);
    Ok(Candidate {
        strategy: "alternation",
        body,
        shapes: vec![format!("{} literal example(s)", distinct.len())],
        explanation: vec![format!(
            "{}\tone of the {} literal example(s), shared prefixes factored out",
            if distinct.len() == 1 { "literal" } else { "trie" },
            distinct.len()
        )],
    })
}

fn build_char_class(examples: &[String], ctx: &Ctx) -> Candidate {
    let t = char_class_tok(examples);
    Candidate {
        strategy: "character-class",
        body: render_tok(&t, ctx),
        shapes: vec!["single character class".into()],
        explanation: vec![explain_tok(&t, ctx)],
    }
}

/// Wrap a bare body with anchors and the flavor's case-insensitivity marker.
fn finish(body: &str, ctx: &Ctx, anchors: bool) -> String {
    let anchored = if anchors {
        format!("^{body}$")
    } else {
        body.to_string()
    };
    match (ctx.flavor, ctx.ci) {
        (Flavor::Javascript, ci) => format!("/{}/{}", anchored, if ci { "i" } else { "" }),
        (Flavor::Posix, _) => anchored,
        (_, true) => format!("(?i){anchored}"),
        (_, false) => anchored,
    }
}

struct Verdict {
    matched: usize,
    excluded: usize,
    leaks: Vec<String>,
}

fn verify(
    rust_pattern: &str,
    ci: bool,
    examples: &[String],
    negatives: &[String],
) -> Result<Verdict, String> {
    let re = RegexBuilder::new(rust_pattern)
        .case_insensitive(ci)
        .size_limit(20 * (1 << 20))
        .build()
        .map_err(|e| format!("the inferred pattern did not compile ({e}); this is a bug — please report the examples that triggered it"))?;
    let matched = examples.iter().filter(|e| re.is_match(e)).count();
    let leaks: Vec<String> = negatives
        .iter()
        .filter(|n| re.is_match(n))
        .take(20)
        .cloned()
        .collect();
    let excluded = negatives.len() - negatives.iter().filter(|n| re.is_match(n)).count();
    Ok(Verdict {
        matched,
        excluded,
        leaks,
    })
}

/// Infer a pattern from `examples` (and optional `negatives`) under `opt`.
pub fn infer(examples_text: &str, negatives_text: &str, opt: &Options) -> Result<Inference, String> {
    let total = examples_text.chars().count() + negatives_text.chars().count();
    if total > MAX_INPUT_CHARS {
        return Err(format!(
            "input too large: {total} characters, the limit is {MAX_INPUT_CHARS}"
        ));
    }
    if opt.max_alternatives == 0 {
        return Err("max_alternatives: expected 1 or more, got 0".into());
    }

    let examples = split_examples(examples_text, opt.separator);
    let negatives = split_examples(negatives_text, opt.separator);
    if examples.is_empty() {
        return Err(
            "examples: expected at least one non-empty example, got none (blank lines are ignored)"
                .into(),
        );
    }
    if examples.len() > MAX_EXAMPLES {
        return Err(format!(
            "examples: {} examples exceed the limit of {MAX_EXAMPLES}",
            examples.len()
        ));
    }
    if negatives.len() > MAX_EXAMPLES {
        return Err(format!(
            "negatives: {} negatives exceed the limit of {MAX_EXAMPLES}",
            negatives.len()
        ));
    }
    let distinct = dedup(&examples);
    if let Some(clash) = distinct.iter().find(|e| negatives.contains(e)) {
        return Err(format!(
            "\"{clash}\" appears in both examples and negatives — no pattern can match it and reject it at the same time"
        ));
    }

    let mut notes: Vec<String> = Vec::new();
    let ctx = Ctx {
        flavor: opt.flavor,
        quant: opt.quantifiers,
        ci: opt.case_insensitive,
        groups: opt.capture_groups,
    };
    // The same construction in Rust syntax is what actually gets compiled and run.
    let vctx = Ctx {
        flavor: Flavor::Rust,
        ..ctx
    };

    // Candidate ladder. `auto` walks it until one verifies; an explicit strategy
    // uses exactly the engine that was asked for.
    let mut plan: Vec<(Strategy, Quantifiers)> = Vec::new();
    let mut exact_fold: Option<BTreeSet<char>> = None;
    match opt.strategy {
        Strategy::Auto => {
            // When the examples agree on every position but one, and the characters
            // they disagree on are SCATTERED, no class generalization describes them:
            // widening `a`/`o`/`u` to `[a-z]` admits 23 characters never seen. Fold
            // that position instead. A contiguous run (`1`,`2`,`3`) is exactly what a
            // class describes, so those still generalize.
            exact_fold = one_varying_position(&distinct).filter(|c| !is_contiguous(c));
            if exact_fold.is_some() {
                plan.push((Strategy::Alternation, opt.quantifiers));
            }
            plan.push((Strategy::Generalize, opt.quantifiers));
            if opt.quantifiers != Quantifiers::Range {
                plan.push((Strategy::Generalize, Quantifiers::Range));
            }
            plan.push((Strategy::Alternation, opt.quantifiers));
        }
        s => plan.push((s, opt.quantifiers)),
    }

    let mut chosen: Option<(Candidate, String, String, Verdict)> = None;
    let mut last_err: Option<String> = None;
    for (idx, (strategy, quant)) in plan.iter().enumerate() {
        let ctx = Ctx {
            quant: *quant,
            ..ctx
        };
        let vctx = Ctx {
            quant: *quant,
            ..vctx
        };
        let built = match strategy {
            Strategy::Generalize => build_generalized(&examples, &ctx, opt.max_alternatives),
            Strategy::Alternation => build_alternation(&examples, &ctx, opt.max_alternatives),
            Strategy::CharacterClass => Ok(build_char_class(&examples, &ctx)),
            Strategy::Auto => unreachable!("auto is expanded into concrete strategies"),
        };
        let cand = match built {
            Ok(c) => c,
            Err(e) => {
                last_err = Some(e);
                continue;
            }
        };
        let verify_body = match strategy {
            Strategy::Generalize => build_generalized(&examples, &vctx, opt.max_alternatives)?.body,
            Strategy::Alternation => build_alternation(&examples, &vctx, opt.max_alternatives)?.body,
            Strategy::CharacterClass => build_char_class(&examples, &vctx).body,
            Strategy::Auto => unreachable!(),
        };
        let rust_pattern = finish(&verify_body, &vctx, opt.anchors);
        let verdict = verify(
            &rust_pattern,
            opt.case_insensitive,
            &examples,
            &negatives,
        )?;
        let pattern = finish(&cand.body, &ctx, opt.anchors);
        let clean = verdict.matched == examples.len() && verdict.leaks.is_empty();
        if clean || opt.strategy != Strategy::Auto || idx == plan.len() - 1 {
            if opt.strategy == Strategy::Auto && idx > 0 {
                notes.push(format!(
                    "escalated to the '{}' engine because an earlier candidate did not reject every negative",
                    cand.strategy
                ));
            }
            if let (0, Some(chars)) = (idx, exact_fold.as_ref()) {
                notes.push(format!(
                    "the examples differ in exactly one position, so that position was kept as the exact set \"{}\" instead of being generalized to its character class",
                    chars.iter().collect::<String>()
                ));
            }
            chosen = Some((cand, pattern, rust_pattern, verdict));
            if clean || opt.strategy != Strategy::Auto {
                break;
            }
        }
    }

    let (cand, pattern, rust_pattern, verdict) = match chosen {
        Some(c) => c,
        None => {
            return Err(last_err.unwrap_or_else(|| {
                "could not infer a pattern from these examples".to_string()
            }))
        }
    };

    if !verdict.leaks.is_empty() {
        notes.push(format!(
            "{} negative(s) still match — with anchors off a negative that CONTAINS an example cannot be excluded; try anchors=true or remove the overlapping negative",
            verdict.leaks.len()
        ));
    }
    if verdict.matched < examples.len() {
        notes.push(format!(
            "only {} of {} examples match the emitted pattern",
            verdict.matched,
            examples.len()
        ));
    }
    if opt.case_insensitive && opt.flavor == Flavor::Posix {
        notes.push(
            "POSIX ERE has no inline (?i) flag, so case-insensitivity is expanded into the character sets".into(),
        );
    }
    if opt.case_insensitive && opt.flavor == Flavor::Javascript {
        notes.push("JavaScript flavor is emitted as a /pattern/flags literal".into());
    }
    if opt.capture_groups && cand.strategy == "alternation" {
        notes.push(
            "capture_groups has no effect on the literal alternation engine — there are no variable fields to group".into(),
        );
    }

    Ok(Inference {
        pattern,
        verified_with: rust_pattern,
        strategy: cand.strategy.to_string(),
        flavor: opt.flavor.label().to_string(),
        anchored: opt.anchors,
        case_insensitive: opt.case_insensitive,
        example_count: examples.len(),
        distinct_examples: distinct.len(),
        negative_count: negatives.len(),
        examples_matched: verdict.matched,
        negatives_excluded: verdict.excluded,
        negatives_still_matching: verdict.leaks,
        shapes: cand.shapes,
        explanation: cand.explanation,
        notes,
    })
}

// ---------------------------------------------------------------------------
// Surface entry point
// ---------------------------------------------------------------------------

fn report_text(r: &Inference) -> String {
    let mut out = String::new();
    out.push_str("Pattern\n");
    out.push_str(&format!("  {}\n\n", r.pattern));
    out.push_str(&format!(
        "Strategy: {} ({} example(s), {} shape(s))\n",
        r.strategy,
        r.example_count,
        r.shapes.len()
    ));
    out.push_str(&format!(
        "Flavor:   {} · {} · {}\n\n",
        r.flavor,
        if r.anchored { "anchored" } else { "unanchored" },
        if r.case_insensitive {
            "case-insensitive"
        } else {
            "case-sensitive"
        }
    ));
    out.push_str("Breakdown\n");
    for line in &r.explanation {
        out.push_str(&format!("  {line}\n"));
    }
    out.push_str("\nVerification\n");
    out.push_str(&format!(
        "  examples:  {}/{} match\n",
        r.examples_matched, r.example_count
    ));
    out.push_str(&format!(
        "  negatives: {}/{} excluded\n",
        r.negatives_excluded, r.negative_count
    ));
    for leak in &r.negatives_still_matching {
        out.push_str(&format!("  still matching: {leak}\n"));
    }
    if !r.notes.is_empty() {
        out.push_str("\nNotes\n");
        for n in &r.notes {
            out.push_str(&format!("  - {n}\n"));
        }
    }
    out.trim_end().to_string()
}

/// String-in / string-out entry point shared by the chat block, the CLI and the page.
#[allow(clippy::too_many_arguments)]
pub fn render(
    examples: &str,
    negatives: &str,
    separator: &str,
    strategy: &str,
    quantifiers: &str,
    flavor: &str,
    anchors: bool,
    case_insensitive: bool,
    capture_groups: bool,
    output: &str,
    max_alternatives: f64,
) -> Result<String, String> {
    if !(1.0..=500.0).contains(&max_alternatives) || max_alternatives.fract() != 0.0 {
        return Err(format!(
            "max_alternatives: expected a whole number between 1 and 500, got {max_alternatives}"
        ));
    }
    let opt = Options {
        separator: Separator::parse(separator)?,
        strategy: Strategy::parse(strategy)?,
        quantifiers: Quantifiers::parse(quantifiers)?,
        flavor: Flavor::parse(flavor)?,
        anchors,
        case_insensitive,
        capture_groups,
        max_alternatives: max_alternatives as usize,
    };
    let out = Output::parse(output)?;
    let r = infer(examples, negatives, &opt)?;
    Ok(match out {
        Output::Pattern => r.pattern,
        Output::Json => {
            serde_json::to_string_pretty(&r).map_err(|e| format!("could not serialise: {e}"))?
        }
        Output::Report => report_text(&r),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pat(examples: &str) -> String {
        infer(examples, "", &Options::default()).unwrap().pattern
    }

    #[test]
    fn merges_a_single_date_shape() {
        assert_eq!(pat("2024-01-15\n2023-11-02\n1999-12-31"), r"^\d{4}-\d{2}-\d{2}$");
    }

    #[test]
    fn unions_separators_and_length_ranges() {
        let r = infer("AB-12\nCD/345\nEF-6", "", &Options::default()).unwrap();
        assert_eq!(r.pattern, r"^[A-Z]{2}[\-/]\d{1,3}$");
        assert_eq!(r.strategy, "generalize");
        assert_eq!(r.examples_matched, 3);
    }

    #[test]
    fn alternates_distinct_shapes() {
        let r = infer("ab12\nxyz\n", "", &Options::default()).unwrap();
        assert_eq!(r.strategy, "shape-alternation");
        assert_eq!(r.pattern, r"^(?:[a-z]{2}\d{2}|[a-z]{3})$");
        assert_eq!(r.examples_matched, 2);
    }

    #[test]
    fn trie_hoists_common_prefixes() {
        let r = infer(
            "foobar\nfoobaz\nfooza\nfoozap",
            "",
            &Options {
                strategy: Strategy::Alternation,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(r.pattern, "^foo(?:ba[rz]|zap?)$");
        assert_eq!(r.examples_matched, 4);
    }

    #[test]
    fn negatives_force_an_escalation_to_literals() {
        // The generalized shape (\d{3}) would also match 999, so auto escalates.
        let r = infer("100\n200\n300", "999", &Options::default()).unwrap();
        assert_eq!(r.strategy, "alternation");
        assert!(r.negatives_still_matching.is_empty());
        assert_eq!(r.negatives_excluded, 1);
        assert!(r.notes.iter().any(|n| n.contains("escalated")));
    }

    #[test]
    fn keeps_the_generalization_when_negatives_already_fail_it() {
        let r = infer("100\n200\n300", "abc\n12", &Options::default()).unwrap();
        assert_eq!(r.strategy, "generalize");
        assert_eq!(r.pattern, r"^\d{3}$");
        assert_eq!(r.negatives_excluded, 2);
    }

    #[test]
    fn posix_flavor_uses_bracket_classes_and_expands_case() {
        let r = infer(
            "ab-1",
            "",
            &Options {
                flavor: Flavor::Posix,
                case_insensitive: true,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(r.pattern, "^[A-Za-z]{2}-[0-9]$");
        assert!(r.notes.iter().any(|n| n.contains("POSIX")));
    }

    #[test]
    fn javascript_flavor_emits_a_regex_literal() {
        let r = infer(
            "ab\ncd",
            "",
            &Options {
                flavor: Flavor::Javascript,
                case_insensitive: true,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(r.pattern, "/^[a-z]{2}$/i");
    }

    #[test]
    fn loose_and_open_quantifiers() {
        let loose = infer(
            "2024-01-15",
            "",
            &Options {
                quantifiers: Quantifiers::Loose,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(loose.pattern, r"^\d+-\d+-\d+$");
        let open = infer(
            "ab1\nab22",
            "",
            &Options {
                quantifiers: Quantifiers::Open,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(open.pattern, r"^[a-z]{2,}\d+$");
    }

    #[test]
    fn capture_groups_wrap_variable_fields_only() {
        let r = infer(
            "2024-01-15",
            "",
            &Options {
                capture_groups: true,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(r.pattern, r"^(\d{4})-(\d{2})-(\d{2})$");
    }

    #[test]
    fn character_class_strategy_unions_everything() {
        let r = infer(
            "abc\nab1",
            "",
            &Options {
                strategy: Strategy::CharacterClass,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(r.pattern, "^[a-z0-9]{3}$");
    }

    #[test]
    fn escapes_regex_metacharacters_in_literals() {
        let r = infer("a.b+c\na.b+d", "", &Options::default()).unwrap();
        assert_eq!(r.pattern, r"^[a-z]\.[a-z]\+[a-z]$");
        assert_eq!(r.examples_matched, 2);
        let lit = infer(
            "a.b+c\na.b+d",
            "",
            &Options {
                strategy: Strategy::Alternation,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(lit.pattern, r"^a\.b\+[cd]$");
        assert_eq!(lit.examples_matched, 2);
    }

    #[test]
    fn unanchored_patterns_search_instead_of_full_match() {
        let r = infer(
            "cat\ncot",
            "",
            &Options {
                anchors: false,
                strategy: Strategy::Alternation,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(r.pattern, "c[ao]t");
    }

    #[test]
    fn report_output_explains_every_element() {
        let text = render(
            "2024-01-15",
            "",
            "newline",
            "auto",
            "range",
            "rust",
            true,
            false,
            false,
            "report",
            50.0,
        )
        .unwrap();
        assert!(text.contains(r"^\d{4}-\d{2}-\d{2}$"));
        assert!(text.contains("exactly 4 digits"));
        assert!(text.contains("examples:  1/1 match"));
    }

    #[test]
    fn json_output_is_structured() {
        let text = render(
            "a1\nb2", "c3", "newline", "auto", "range", "rust", true, false, false, "json", 50.0,
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["negative_count"], 1);
        assert_eq!(v["example_count"], 2);
        assert!(v["pattern"].as_str().unwrap().starts_with('^'));
    }

    #[test]
    fn comma_separator_splits_inline_lists() {
        let r = infer(
            "cat, cot, cut",
            "",
            &Options {
                separator: Separator::Comma,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(r.example_count, 3);
        assert_eq!(r.pattern, "^c[aou]t$");
    }

    #[test]
    fn errors_on_empty_examples() {
        let e = infer("   \n\n", "", &Options::default()).unwrap_err();
        assert!(e.contains("at least one non-empty example"), "{e}");
    }

    #[test]
    fn errors_when_an_example_is_also_a_negative() {
        let e = infer("abc\ndef", "abc", &Options::default()).unwrap_err();
        assert!(e.contains("both examples and negatives"), "{e}");
    }

    #[test]
    fn errors_on_an_unknown_enum_value() {
        let e = render(
            "a", "", "newline", "magic", "range", "rust", true, false, false, "pattern", 50.0,
        )
        .unwrap_err();
        assert!(e.contains("strategy: expected one of"), "{e}");
    }

    #[test]
    fn errors_when_shapes_exceed_max_alternatives() {
        let e = infer(
            "a\n1\na1\n1a",
            "",
            &Options {
                strategy: Strategy::Generalize,
                max_alternatives: 2,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(e.contains("max_alternatives=2"), "{e}");
    }

    #[test]
    fn unicode_letters_stay_literal() {
        let r = infer("café\ncafé", "", &Options::default()).unwrap();
        assert_eq!(r.examples_matched, 2);
        assert!(r.pattern.contains('é'));
    }

    #[test]
    fn every_emitted_pattern_compiles_and_matches_its_examples() {
        let sets = [
            "192.168.0.1\n10.0.0.255",
            "ORD-2024-0001\nORD-2023-9999",
            "a@b.com\nlonger.name@example.org",
            "  spaced out  \ntwo words",
            "[bracket]\n(paren)\n{brace}",
            "3.14\n-2.5\n+0.001",
        ];
        for s in sets {
            for strategy in [
                Strategy::Auto,
                Strategy::Alternation,
                Strategy::CharacterClass,
            ] {
                for flavor in [Flavor::Rust, Flavor::Posix, Flavor::Javascript] {
                    for quant in [Quantifiers::Range, Quantifiers::Open, Quantifiers::Loose] {
                        let r = infer(
                            s,
                            "",
                            &Options {
                                strategy,
                                flavor,
                                quantifiers: quant,
                                ..Options::default()
                            },
                        )
                        .unwrap_or_else(|e| panic!("{s:?} {strategy:?} {flavor:?}: {e}"));
                        assert_eq!(
                            r.examples_matched, r.example_count,
                            "{s:?} {strategy:?} {flavor:?} {quant:?} -> {}",
                            r.verified_with
                        );
                    }
                }
            }
        }
    }
}

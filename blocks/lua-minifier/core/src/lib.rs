//! gizza-ai/lua-minifier core — shrink Lua source by stripping comments and
//! whitespace, and (optionally) renaming local variables to short names.
//! Pure-Rust, dependency-free.
//!
//! This is a token-aware minifier, not a full Lua parser. It scans the source
//! into tokens (short/long strings, line/long comments, names, numbers,
//! operators) and re-emits them with the minimum whitespace needed to keep every
//! token meaning exactly what it did before. Lua's grammar ignores line breaks
//! entirely (there is no automatic-semicolon-insertion rule as in JavaScript), so
//! a whole script can safely collapse onto one line — the only newline that must
//! survive is the one that terminates a kept `--` line comment, plus a leading
//! `#!` shebang line.
//!
//! String and long-comment interiors are never touched, table fields and method
//! names are never renamed, and no code is reordered or dropped (other than
//! comments). With `rename_locals` the minifier additionally runs a scope-aware
//! pass that renames `local` variables, `local function` names and function
//! parameters to short unique aliases:
//!
//! * every alias is unique across the whole file, so an inner block can never
//!   shadow an outer local that is still referenced;
//! * aliases avoid every global name used anywhere in the file;
//! * globals, table keys, fields (`t.x`), method names (`t:m()`), `goto` labels
//!   and string contents are left alone;
//! * if the block structure does not balance (a missing or extra `end`), the
//!   rename refuses with an error instead of emitting plausible-but-wrong code.

use std::collections::{HashMap, HashSet};

/// What to do with the source's line structure.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LineBreaks {
    /// Join everything onto one line (smallest output). Default.
    Strip,
    /// Keep one output line per non-empty source line, without the indentation.
    Keep,
}

impl LineBreaks {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "strip" | "none" | "remove" | "join" => Ok(LineBreaks::Strip),
            "keep" | "preserve" | "lines" => Ok(LineBreaks::Keep),
            other => Err(format!(
                "invalid line_breaks '{other}' (expected strip or keep)"
            )),
        }
    }
}

/// Minifier options. See the crate docs for the safety model.
#[derive(Clone, Debug)]
pub struct Options {
    /// Strip `--` line comments and `--[[ … ]]` long comments.
    pub remove_comments: bool,
    /// When removing comments, still keep license/banner comments.
    pub keep_license: bool,
    /// Rename locals, `local function` names and parameters to short aliases.
    pub rename_locals: bool,
    /// What to do with the source's line structure.
    pub line_breaks: LineBreaks,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            remove_comments: true,
            keep_license: true,
            rename_locals: false,
            line_breaks: LineBreaks::Strip,
        }
    }
}

// ---------------------------------------------------------------- tokenizer

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    /// Identifier or keyword.
    Name,
    /// Numeric literal (decimal, hex, exponent).
    Number,
    /// `"…"` / `'…'`, delimiters included.
    ShortString,
    /// `[[…]]` / `[=[…]=]`, delimiters included; may span lines.
    LongString,
    /// `-- …` to end of line.
    LineComment,
    /// `--[[ … ]]`; may span lines.
    LongComment,
    /// Operator or punctuation.
    Op,
}

#[derive(Clone, Debug)]
struct Tok {
    kind: Kind,
    text: String,
    /// At least one newline appeared in the whitespace before this token.
    nl_before: bool,
}

const KEYWORDS: &[&str] = &[
    "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "goto", "if", "in",
    "local", "nil", "not", "or", "repeat", "return", "then", "true", "until", "while",
];

fn is_keyword(w: &str) -> bool {
    KEYWORDS.contains(&w)
}

/// Keywords that can only start a new statement / block boundary.
const STMT_KEYWORDS: &[&str] = &[
    "local", "if", "while", "for", "repeat", "return", "do", "function", "break", "goto", "end",
    "else", "elseif", "until", "then",
];

/// Multi-character operators, longest first so the scanner is greedy.
const MULTI_OPS: &[&str] = &["...", "..", "::", "==", "~=", "<=", ">=", "//", "<<", ">>"];

/// Adjacent character pairs that would fuse into a different token if the two
/// tokens were written without a separating space.
const FUSING_PAIRS: &[&str] = &[
    "--", "..", "::", "==", "~=", "<=", ">=", "//", "<<", ">>", "[[", "[=",
];

fn is_name_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}
fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// If `b[i]` opens a Lua long bracket (`[`, zero or more `=`, `[`), return its
/// level (the count of `=`); otherwise `None`.
fn long_bracket_level(b: &[char], i: usize) -> Option<usize> {
    if b.get(i) != Some(&'[') {
        return None;
    }
    let mut j = i + 1;
    let mut level = 0;
    while b.get(j) == Some(&'=') {
        level += 1;
        j += 1;
    }
    if b.get(j) == Some(&'[') {
        Some(level)
    } else {
        None
    }
}

/// Consume a long-bracket body opening at `i` with `level` `=` signs, returning
/// the exclusive end index. Forgiving: an unterminated bracket runs to EOF.
fn scan_long_bracket(b: &[char], i: usize, level: usize) -> usize {
    let n = b.len();
    let close: Vec<char> = std::iter::once(']')
        .chain(std::iter::repeat('=').take(level))
        .chain(std::iter::once(']'))
        .collect();
    let mut j = i + 2 + level;
    while j < n {
        if b[j] == ']' && b[j..].starts_with(close.as_slice()) {
            return j + close.len();
        }
        j += 1;
    }
    n
}

/// Tokenize Lua source. Forgiving: unterminated strings/comments run to EOF or
/// end of line, so malformed input still minifies rather than panicking.
fn tokenize(src: &str) -> Vec<Tok> {
    let b: Vec<char> = src.chars().collect();
    let n = b.len();
    let mut i = 0usize;
    let mut out: Vec<Tok> = Vec::new();
    let mut nl = false;

    macro_rules! push {
        ($kind:expr, $text:expr) => {{
            out.push(Tok {
                kind: $kind,
                text: $text,
                nl_before: nl,
            });
            nl = false;
        }};
    }

    while i < n {
        let c = b[i];
        if c == '\n' || c == '\r' {
            nl = true;
            i += 1;
            continue;
        }
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        // comments: -- … / --[[ … ]]
        if c == '-' && b.get(i + 1) == Some(&'-') {
            if let Some(level) = long_bracket_level(&b, i + 2) {
                let end = scan_long_bracket(&b, i + 2, level);
                push!(Kind::LongComment, b[i..end].iter().collect());
                i = end;
                continue;
            }
            let start = i;
            while i < n && b[i] != '\n' && b[i] != '\r' {
                i += 1;
            }
            push!(Kind::LineComment, b[start..i].iter().collect());
            continue;
        }
        // long string
        if c == '[' {
            if let Some(level) = long_bracket_level(&b, i) {
                let end = scan_long_bracket(&b, i, level);
                push!(Kind::LongString, b[i..end].iter().collect());
                i = end;
                continue;
            }
        }
        // short string
        if c == '"' || c == '\'' {
            let quote = c;
            let start = i;
            i += 1;
            while i < n {
                if b[i] == '\\' {
                    i += 2;
                    continue;
                }
                if b[i] == quote {
                    i += 1;
                    break;
                }
                if b[i] == '\n' {
                    break; // unterminated — stay forgiving
                }
                i += 1;
            }
            push!(Kind::ShortString, b[start..i.min(n)].iter().collect());
            continue;
        }
        // number
        if c.is_ascii_digit() || (c == '.' && b.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            let start = i;
            let mut seen_dot = false;
            while i < n {
                let d = b[i];
                if d.is_ascii_alphanumeric() {
                    // an exponent sign belongs to the literal
                    if matches!(d, 'e' | 'E' | 'p' | 'P')
                        && matches!(b.get(i + 1), Some('+') | Some('-'))
                    {
                        i += 2;
                        continue;
                    }
                    i += 1;
                    continue;
                }
                if d == '.' && !seen_dot {
                    seen_dot = true;
                    i += 1;
                    continue;
                }
                break;
            }
            push!(Kind::Number, b[start..i].iter().collect());
            continue;
        }
        // name / keyword
        if is_name_start(c) {
            let start = i;
            while i < n && is_name_char(b[i]) {
                i += 1;
            }
            push!(Kind::Name, b[start..i].iter().collect());
            continue;
        }
        // operator / punctuation
        let rest: String = b[i..n.min(i + 3)].iter().collect();
        if let Some(op) = MULTI_OPS.iter().find(|op| rest.starts_with(**op)) {
            push!(Kind::Op, (*op).to_string());
            i += op.chars().count();
            continue;
        }
        push!(Kind::Op, c.to_string());
        i += 1;
    }
    out
}

// ------------------------------------------------------------- rename pass

/// Scope-aware local renamer over the token stream.
struct Renamer<'a> {
    toks: &'a [Tok],
    /// Indices of the significant (non-comment) tokens.
    sig: Vec<usize>,
    scopes: Vec<HashMap<String, usize>>,
    /// Bracket depth captured when each scope was pushed.
    scope_depth: Vec<usize>,
    brackets: Vec<char>,
    pending_locals: Vec<(String, usize)>,
    pending_loop_vars: Vec<(String, usize)>,
    pending_pops: usize,
    pending_function: bool,
    decls: usize,
    /// (token index, declaration id) for every site that must be rewritten.
    resolved: Vec<(usize, usize)>,
    globals: HashSet<String>,
    imbalance: bool,
}

impl<'a> Renamer<'a> {
    fn new(toks: &'a [Tok]) -> Self {
        let sig = toks
            .iter()
            .enumerate()
            .filter(|(_, t)| !matches!(t.kind, Kind::LineComment | Kind::LongComment))
            .map(|(i, _)| i)
            .collect();
        Self {
            toks,
            sig,
            scopes: vec![HashMap::new()],
            scope_depth: vec![0],
            brackets: Vec::new(),
            pending_locals: Vec::new(),
            pending_loop_vars: Vec::new(),
            pending_pops: 0,
            pending_function: false,
            decls: 0,
            resolved: Vec::new(),
            globals: HashSet::new(),
            imbalance: false,
        }
    }

    fn tok(&self, k: usize) -> Option<&'a Tok> {
        self.sig.get(k).map(|&i| &self.toks[i])
    }
    fn text(&self, k: usize) -> Option<&'a str> {
        self.tok(k).map(|t| t.text.as_str())
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
        self.scope_depth.push(self.brackets.len());
    }

    fn pop_scope(&mut self) {
        // Anything still pending belongs to the block that is closing.
        self.register_pending_locals();
        if self.scopes.len() <= 1 {
            self.imbalance = true;
            return;
        }
        self.scopes.pop();
        self.scope_depth.pop();
    }

    fn declare(&mut self, name: String, idx: usize) {
        let id = self.decls;
        self.decls += 1;
        self.resolved.push((idx, id));
        if let Some(top) = self.scopes.last_mut() {
            top.insert(name, id);
        }
    }

    fn lookup(&self, name: &str) -> Option<usize> {
        self.scopes.iter().rev().find_map(|s| s.get(name).copied())
    }

    fn register_pending_locals(&mut self) {
        for (name, idx) in std::mem::take(&mut self.pending_locals) {
            self.declare(name, idx);
        }
    }

    /// End of a statement: register pending `local`s and apply deferred pops
    /// (a `repeat` block's scope stays alive through its `until` expression).
    fn flush(&mut self) {
        self.register_pending_locals();
        while self.pending_pops > 0 {
            self.pending_pops -= 1;
            self.pop_scope();
        }
    }

    /// Could `t` be the last token of an expression / statement?
    fn ends_expr(t: &Tok) -> bool {
        match t.kind {
            Kind::Number | Kind::ShortString | Kind::LongString => true,
            Kind::Name => !is_keyword(&t.text) || matches!(t.text.as_str(), "end" | "true" | "false" | "nil"),
            Kind::Op => matches!(t.text.as_str(), ")" | "]" | "}" | "..."),
            _ => false,
        }
    }

    /// Does a new statement begin at `cur`, given the previous token `prev`?
    fn is_boundary(prev: Option<&Tok>, cur: &Tok) -> bool {
        let Some(p) = prev else { return false };
        if !Self::ends_expr(p) {
            return false;
        }
        match cur.kind {
            Kind::Op => matches!(cur.text.as_str(), ";" | "::"),
            Kind::Name => {
                !is_keyword(&cur.text) || STMT_KEYWORDS.contains(&cur.text.as_str())
            }
            _ => false,
        }
    }

    /// Is this identifier something other than a variable reference (a field, a
    /// method name, a table key, a `goto`/`::` label)?
    fn is_non_variable(&self, k: usize) -> bool {
        let prev = if k > 0 { self.text(k - 1) } else { None };
        if matches!(prev, Some(".") | Some(":")) {
            return true;
        }
        if self.brackets.last() == Some(&'{')
            && matches!(prev, Some("{") | Some(",") | Some(";"))
            && self.text(k + 1) == Some("=")
        {
            return true; // table-constructor key: { name = value }
        }
        false
    }

    /// Collect the name list of a `local a, b <const>` declaration. `k` points at
    /// the first name; returns the cursor just past the list.
    fn collect_locals(&mut self, mut k: usize) -> usize {
        loop {
            let Some(t) = self.tok(k) else { break };
            if t.kind != Kind::Name || is_keyword(&t.text) {
                break;
            }
            self.pending_locals.push((t.text.clone(), self.sig[k]));
            k += 1;
            // Lua 5.4 attribute: `local x <const>` / `<close>`
            if self.text(k) == Some("<") {
                k += 1;
                if self.tok(k).is_some() {
                    k += 1;
                }
                if self.text(k) == Some(">") {
                    k += 1;
                }
            }
            if self.text(k) == Some(",") {
                k += 1;
                continue;
            }
            break;
        }
        k
    }

    /// Collect `for i, v` loop variables; they enter scope at the matching `do`.
    fn collect_loop_vars(&mut self, mut k: usize) -> usize {
        loop {
            let Some(t) = self.tok(k) else { break };
            if t.kind != Kind::Name || is_keyword(&t.text) {
                break;
            }
            self.pending_loop_vars.push((t.text.clone(), self.sig[k]));
            k += 1;
            if self.text(k) == Some(",") {
                k += 1;
                continue;
            }
            break;
        }
        k
    }

    /// Declare a parameter list. `k` points just after the opening `(`; returns
    /// the cursor just past the closing `)`.
    fn collect_params(&mut self, mut k: usize) -> usize {
        loop {
            let Some(t) = self.tok(k) else { break };
            match t.text.as_str() {
                ")" => {
                    k += 1;
                    break;
                }
                "," | "..." => k += 1,
                _ => {
                    if t.kind == Kind::Name && !is_keyword(&t.text) {
                        self.declare(t.text.clone(), self.sig[k]);
                    }
                    k += 1;
                }
            }
        }
        k
    }

    fn run(&mut self) {
        let mut k = 0usize;
        while k < self.sig.len() {
            let idx = self.sig[k];
            let t = &self.toks[idx];

            if (!self.pending_locals.is_empty() || self.pending_pops > 0)
                && self.brackets.len() == *self.scope_depth.last().unwrap_or(&0)
            {
                let prev = if k > 0 { self.tok(k - 1) } else { None };
                if Self::is_boundary(prev, t) {
                    self.flush();
                }
            }

            match t.kind {
                Kind::Name if is_keyword(&t.text) => match t.text.as_str() {
                    "local" => {
                        if self.text(k + 1) != Some("function") {
                            k = self.collect_locals(k + 1);
                            continue;
                        }
                    }
                    "function" => {
                        let is_local_fn = k > 0 && self.text(k - 1) == Some("local");
                        if is_local_fn {
                            if let Some(name_tok) = self.tok(k + 1) {
                                if name_tok.kind == Kind::Name && !is_keyword(&name_tok.text) {
                                    let name = name_tok.text.clone();
                                    let name_idx = self.sig[k + 1];
                                    self.declare(name, name_idx);
                                    k += 1;
                                }
                            }
                        }
                        self.pending_function = true;
                    }
                    "do" => {
                        self.push_scope();
                        for (name, idx) in std::mem::take(&mut self.pending_loop_vars) {
                            self.declare(name, idx);
                        }
                    }
                    "then" | "repeat" => self.push_scope(),
                    "end" => self.pop_scope(),
                    "else" => {
                        self.pop_scope();
                        self.push_scope();
                    }
                    "elseif" => self.pop_scope(),
                    "until" => self.pending_pops += 1,
                    "for" => {
                        k = self.collect_loop_vars(k + 1);
                        continue;
                    }
                    "goto" => {
                        k += 2; // skip the label name
                        continue;
                    }
                    _ => {}
                },
                Kind::Name => {
                    if !self.is_non_variable(k) {
                        match self.lookup(&t.text) {
                            Some(id) => self.resolved.push((idx, id)),
                            None => {
                                self.globals.insert(t.text.clone());
                            }
                        }
                    }
                }
                Kind::Op => match t.text.as_str() {
                    "(" => {
                        if self.pending_function {
                            self.pending_function = false;
                            self.push_scope();
                            k = self.collect_params(k + 1);
                            continue;
                        }
                        self.brackets.push('(');
                    }
                    "::" => {
                        // `::label::` — the name between the delimiters is a
                        // goto label, not a variable. Skip the name AND the
                        // closing `::`, so the statement that follows the label
                        // is still resolved normally (a bare `prev == "::"`
                        // test would mistake it for a second label name).
                        let mut j = k + 1;
                        if self.tok(j).is_some_and(|t| t.kind == Kind::Name) {
                            j += 1;
                        }
                        if self.text(j) == Some("::") {
                            j += 1;
                        }
                        k = j;
                        continue;
                    }
                    "[" => self.brackets.push('['),
                    "{" => self.brackets.push('{'),
                    ")" | "]" | "}" => {
                        self.brackets.pop();
                    }
                    _ => {}
                },
                _ => {}
            }
            k += 1;
        }
        self.flush();
    }
}

/// `0 → a`, `25 → z`, `26 → A`, `51 → Z`, `52 → aa`, …
fn short_name(mut n: usize) -> String {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let base = ALPHABET.len();
    let mut out = Vec::new();
    loop {
        out.push(ALPHABET[n % base]);
        if n < base {
            break;
        }
        n = n / base - 1;
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_else(|_| "a".to_string())
}

/// Rename locals in place. Errors when block structure does not balance.
fn rename_locals(toks: &mut [Tok]) -> Result<(), String> {
    let (resolved, decls, globals, ok) = {
        let mut r = Renamer::new(toks);
        r.run();
        let ok = !r.imbalance && r.scopes.len() == 1;
        (r.resolved, r.decls, r.globals, ok)
    };
    if !ok {
        return Err(
            "cannot rename locals: the block structure does not balance — check for a missing or \
             extra 'end' (minifying without rename_locals still works)"
                .to_string(),
        );
    }
    let mut aliases: Vec<String> = Vec::with_capacity(decls);
    let mut counter = 0usize;
    for _ in 0..decls {
        loop {
            let cand = short_name(counter);
            counter += 1;
            if !is_keyword(&cand) && !globals.contains(&cand) {
                aliases.push(cand);
                break;
            }
        }
    }
    for (idx, id) in resolved {
        if let Some(alias) = aliases.get(id) {
            toks[idx].text = alias.clone();
        }
    }
    Ok(())
}

// ----------------------------------------------------------------- rendering

/// Is this comment a license / banner comment that survives comment stripping?
fn is_license_comment(text: &str) -> bool {
    if text.contains("@license") || text.contains("@preserve") || text.contains("@copyright") {
        return true;
    }
    // `--!` (short) or `--[[!` / `--[=[!` (long) marks an important banner.
    let body = text.trim_start_matches('-');
    let body = body.trim_start_matches('[').trim_start_matches('=');
    let body = body.trim_start_matches('[');
    body.starts_with('!')
}

/// Would `a` and `b` fuse into a different token if written adjacently?
fn needs_space(a: &Tok, b: &Tok) -> bool {
    let numlike = |k: Kind| matches!(k, Kind::Name | Kind::Number);
    if numlike(a.kind) && numlike(b.kind) {
        return true;
    }
    if a.kind == Kind::Number && b.text.starts_with('.') {
        return true; // `1 ..x` must not become the malformed number `1..x`
    }
    let (Some(last), Some(first)) = (a.text.chars().last(), b.text.chars().next()) else {
        return false;
    };
    let pair = format!("{last}{first}");
    FUSING_PAIRS.contains(&pair.as_str())
}

/// Minify Lua `src` with the default options (strip comments, keep banners,
/// no renaming, join every line).
pub fn minify(src: &str) -> Result<String, String> {
    minify_with(src, &Options::default())
}

/// Minify Lua `src` with explicit [`Options`].
pub fn minify_with(src: &str, opts: &Options) -> Result<String, String> {
    if src.trim().is_empty() {
        return Err("no Lua input".to_string());
    }

    // A leading `#!` line is consumed by the Lua interpreter itself — keep it
    // verbatim on its own line, minify everything after it.
    let (shebang, body) = match src.strip_prefix('#') {
        Some(_) => {
            let end = src.find('\n').unwrap_or(src.len());
            (Some(src[..end].trim_end().to_string()), &src[end..])
        }
        None => (None, src),
    };

    let mut toks = tokenize(body);
    if opts.rename_locals {
        rename_locals(&mut toks)?;
    }

    let mut out = String::new();
    let mut prev: Option<Tok> = None;
    let mut force_newline = false;
    let mut carried_nl = false;

    for t in &toks {
        let drop = match t.kind {
            Kind::LineComment | Kind::LongComment => {
                opts.remove_comments && !(opts.keep_license && is_license_comment(&t.text))
            }
            _ => false,
        };
        if drop {
            carried_nl |= t.nl_before;
            continue;
        }
        let nl_before = t.nl_before || carried_nl;
        carried_nl = false;

        if let Some(p) = &prev {
            if force_newline {
                out.push('\n');
            } else if opts.line_breaks == LineBreaks::Keep && nl_before {
                out.push('\n');
            } else if needs_space(p, t) {
                out.push(' ');
            }
        }
        force_newline = t.kind == Kind::LineComment;
        out.push_str(&t.text);
        prev = Some(t.clone());
    }

    if out.is_empty() {
        return Err("nothing left to minify — the input was only comments or whitespace".to_string());
    }
    Ok(match shebang {
        Some(line) => format!("{line}\n{out}"),
        None => out,
    })
}

/// String-keyed entry point shared by the chat block, the CLI and the page.
/// Every surface hands the options over as already-parsed booleans plus the
/// `line_breaks` choice as text, so the enum has exactly one parser.
pub fn run(
    code: &str,
    remove_comments: bool,
    keep_license: bool,
    rename_locals: bool,
    line_breaks: &str,
) -> Result<String, String> {
    minify_with(
        code,
        &Options {
            remove_comments,
            keep_license,
            rename_locals,
            line_breaks: LineBreaks::parse(line_breaks)?,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn min(src: &str) -> String {
        minify(src).unwrap()
    }

    fn renamed(src: &str) -> String {
        minify_with(
            src,
            &Options {
                rename_locals: true,
                ..Options::default()
            },
        )
        .unwrap()
    }

    #[test]
    fn strips_comments_and_whitespace() {
        let src = "-- a note\nlocal x = 1\n\nprint( x )   -- trailing\n";
        assert_eq!(min(src), "local x=1 print(x)");
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = minify("   \n\n").unwrap_err();
        assert!(err.contains("no Lua input"), "got: {err}");
    }

    #[test]
    fn only_comments_is_an_error() {
        let err = minify("-- just a note\n--[[ and another ]]\n").unwrap_err();
        assert!(err.contains("nothing left to minify"), "got: {err}");
    }

    #[test]
    fn keeps_comments_when_asked() {
        let out = minify_with(
            "local a = 1 -- note\nlocal b = 2",
            &Options {
                remove_comments: false,
                ..Options::default()
            },
        )
        .unwrap();
        // `1--` is safe: the number scanner stops at `-`, so the comment still
        // starts where it did. Only `-` followed by `--` needs the separator.
        assert_eq!(out, "local a=1-- note\nlocal b=2");
    }

    #[test]
    fn keeps_license_banner_by_default() {
        let out = min("--! (c) 2026 ACME\n-- internal note\nreturn 1");
        assert_eq!(out, "--! (c) 2026 ACME\nreturn 1");
    }

    #[test]
    fn keep_license_can_be_turned_off() {
        let out = minify_with(
            "--[[ @license MIT ]]\nreturn 1",
            &Options {
                keep_license: false,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(out, "return 1");
    }

    #[test]
    fn string_and_long_string_interiors_are_untouched() {
        let out = min("local s = [[\n  keep   me\n]]\nlocal t = \"a  b -- c\"\nprint(s, t)");
        assert_eq!(
            out,
            "local s=[[\n  keep   me\n]]local t=\"a  b -- c\"print(s,t)"
        );
    }

    #[test]
    fn separates_tokens_that_would_fuse() {
        // `a - -b` must not become `a--b` (a line comment).
        assert_eq!(min("local c = a - -b"), "local c=a- -b");
        // `1 .. 2` must not become the malformed number `1..2`.
        assert_eq!(min("local s = 1 .. 2"), "local s=1 ..2");
        // `t[ [[x]] ]` must not become `t[[[x]]]` (a long bracket).
        assert_eq!(min("local v = t[ [[x]] ]"), "local v=t[ [[x]]]");
    }

    #[test]
    fn keywords_stay_separated() {
        assert_eq!(min("if a and b then return 1 end"), "if a and b then return 1 end");
    }

    #[test]
    fn line_breaks_keep_preserves_line_structure() {
        let out = minify_with(
            "local a = 1\n\n   local b = 2\nprint(a + b)",
            &Options {
                line_breaks: LineBreaks::Keep,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(out, "local a=1\nlocal b=2\nprint(a+b)");
    }

    #[test]
    fn shebang_survives() {
        let out = min("#!/usr/bin/env lua\nlocal x = 1\nprint(x)");
        assert_eq!(out, "#!/usr/bin/env lua\nlocal x=1 print(x)");
    }

    #[test]
    fn renames_locals_and_parameters() {
        let out = renamed("local function add(first, second)\n  return first + second\nend\nprint(add(1, 2))");
        assert_eq!(out, "local function a(b,c)return b+c end print(a(1,2))");
    }

    #[test]
    fn rename_leaves_globals_fields_and_methods_alone() {
        let out = renamed("local obj = setmetatable({}, Meta)\nobj.field = 1\nobj:method(obj.other)");
        assert_eq!(
            out,
            "local a=setmetatable({},Meta)a.field=1 a:method(a.other)"
        );
    }

    #[test]
    fn rename_keeps_table_keys() {
        let out = renamed("local name = 1\nlocal t = { name = name, [name] = 2 }\nreturn t");
        assert_eq!(out, "local a=1 local b={name=a,[a]=2}return b");
    }

    #[test]
    fn rename_respects_declaration_order() {
        // `local print = print` must read the GLOBAL print on the right.
        let out = renamed("local print = print\nprint('hi')");
        assert_eq!(out, "local a=print a('hi')");
    }

    #[test]
    fn rename_never_shadows_an_outer_local() {
        let out = renamed("local outer = 1\ndo local inner = 2 print(outer, inner) end\nreturn outer");
        assert_eq!(out, "local a=1 do local b=2 print(a,b)end return a");
    }

    #[test]
    fn rename_scopes_are_popped_at_end() {
        // `x` after the block is a global, not the block's local.
        let out = renamed("do local x = 1 print(x) end\nx = 2");
        assert_eq!(out, "do local a=1 print(a)end x=2");
    }

    #[test]
    fn rename_handles_for_loops() {
        let out = renamed("local total = 0\nfor idx = 1, 10 do total = total + idx end\nfor key, value in pairs(t) do print(key, value) end\nreturn total");
        assert_eq!(
            out,
            "local a=0 for b=1,10 do a=a+b end for c,d in pairs(t)do print(c,d)end return a"
        );
    }

    #[test]
    fn rename_handles_repeat_until_scope() {
        // the `until` expression can still see the body's locals
        let out = renamed("repeat local done = check() until done\ndone = 1");
        assert_eq!(out, "repeat local a=check()until a done=1");
    }

    #[test]
    fn rename_avoids_global_names() {
        // the file already uses a global called `a`, so no local may become `a`
        let out = renamed("local first = a\nlocal second = 2\nreturn first + second + a");
        assert_eq!(out, "local b=a local c=2 return b+c+a");
    }

    #[test]
    fn rename_leaves_goto_labels_alone() {
        let out = renamed("local i = 1\n::top::\ni = i + 1\nif i < 3 then goto top end\nreturn i");
        assert_eq!(out, "local a=1::top::a=a+1 if a<3 then goto top end return a");
    }

    #[test]
    fn rename_handles_method_definitions_and_self() {
        let out = renamed("local M = {}\nfunction M:greet(who)\n  return self.name .. who\nend\nreturn M");
        // `name..b` is safe — only a NUMBER before `..` needs the separator.
        assert_eq!(out, "local a={}function a:greet(b)return self.name..b end return a");
    }

    #[test]
    fn rename_handles_lua54_attributes() {
        let out = renamed("local limit <const> = 10\nreturn limit");
        // The space goes AFTER the attribute, not before it: `<const>=` would
        // lex as `<`, `const`, `>=` and break the declaration.
        assert_eq!(out, "local a<const> =10 return a");
    }

    #[test]
    fn rename_refuses_unbalanced_blocks() {
        let err = minify_with(
            "local function f()\n  return 1\n",
            &Options {
                rename_locals: true,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("does not balance"), "got: {err}");
    }

    #[test]
    fn unbalanced_input_still_minifies_without_rename() {
        assert_eq!(min("local function f()\n  return 1\n"), "local function f()return 1");
    }

    #[test]
    fn short_name_sequence() {
        assert_eq!(short_name(0), "a");
        assert_eq!(short_name(25), "z");
        assert_eq!(short_name(26), "A");
        assert_eq!(short_name(51), "Z");
        assert_eq!(short_name(52), "aa");
    }

    #[test]
    fn line_breaks_parse() {
        assert_eq!(LineBreaks::parse("strip").unwrap(), LineBreaks::Strip);
        assert_eq!(LineBreaks::parse("KEEP").unwrap(), LineBreaks::Keep);
        assert!(LineBreaks::parse("wrap").is_err());
    }

    #[test]
    fn run_matches_the_defaults_every_surface_sends() {
        let out = run(
            "-- a note\nlocal greeting = \"hi\"\nprint( greeting )\n",
            true,
            true,
            false,
            "strip",
        )
        .unwrap();
        assert_eq!(out, "local greeting=\"hi\"print(greeting)");
    }

    #[test]
    fn run_threads_every_option_through() {
        let src = "--! banner\n-- drop me\nlocal counter = 0\ncounter = counter + 1\nreturn counter";
        assert_eq!(
            run(src, true, true, true, "keep").unwrap(),
            "--! banner\nlocal a=0\na=a+1\nreturn a"
        );
        assert_eq!(
            run(src, true, false, true, "strip").unwrap(),
            "local a=0 a=a+1 return a"
        );
        assert_eq!(
            run(src, false, true, false, "keep").unwrap(),
            "--! banner\n-- drop me\nlocal counter=0\ncounter=counter+1\nreturn counter"
        );
    }

    #[test]
    fn run_rejects_an_unknown_line_breaks_value() {
        let err = run("return 1", true, true, false, "wrap").unwrap_err();
        assert!(err.contains("invalid line_breaks 'wrap'"), "got: {err}");
    }

    #[test]
    fn run_rejects_empty_input() {
        assert!(run("", true, true, false, "strip")
            .unwrap_err()
            .contains("no Lua input"));
    }
}

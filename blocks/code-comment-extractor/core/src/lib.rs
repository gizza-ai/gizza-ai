//! code-comment-extractor core — pull every comment out of pasted source code
//! (line, block, and doc comments) across the common comment syntaxes, and/or
//! emit the same source with those comments removed. Pure-Rust (only
//! `serde_json` for the JSON output). No wafer/wasm-bindgen deps — shared by the
//! chat skill block and the web page.
//!
//! There is **no full language parser** here: a parser-generator's per-language
//! grammars are C libraries that neither build nor instantiate in the wasm
//! sandbox, so extraction comes from a deterministic, dependency-free comment /
//! string tokenizer:
//!
//! - It scans the source character by character. String and character literals
//!   are consumed so a `//` or `#` *inside a string* is never mistaken for a
//!   comment opener.
//! - Each language has a profile listing its line-comment openers (longest
//!   first, so `///` wins over `//`), its block-comment pairs, and which of
//!   those openers mark a **doc** comment (`///`, `//!`, `/**`, `##`).
//! - Python has no block-comment syntax, so a triple-quoted string that is the
//!   first thing on its line is classified as a doc comment (a docstring) when
//!   `docstrings` is on.
//!
//! Every comment carries its 1-based line and column, its end line, and its
//! text — rendered as a plain list, a Markdown table, JSON, a statistics
//! summary, or as the original source with the matched comments removed.

use serde_json::{json, Value};

/// Cap on the number of comments returned (keeps output bounded for huge inputs).
pub const MAX_COMMENTS: usize = 50_000;

/// What kind of comment was matched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// A single-line comment (`//`, `#`, `--`, …).
    Line,
    /// A delimited block comment (`/* */`, `<!-- -->`, `--[[ ]]`, `=begin/=end`).
    Block,
    /// A documentation comment (`///`, `//!`, `/**`, `##`, a Python docstring).
    Doc,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Line => "line",
            Kind::Block => "block",
            Kind::Doc => "doc",
        }
    }
}

/// One extracted comment.
#[derive(Debug, Clone)]
struct Comment {
    /// 1-based source line of the opening marker.
    line: usize,
    /// 1-based source column (in characters) of the opening marker.
    column: usize,
    /// 1-based source line the comment ends on (== `line` for single-line ones).
    end_line: usize,
    kind: Kind,
    /// The comment as it appeared, markers included.
    raw: String,
    /// Character range `[start, end)` in the source, for the strip pass.
    start: usize,
    end: usize,
}

/// A block-comment delimiter pair.
struct BlockPair {
    open: &'static str,
    close: &'static str,
    /// The opener only counts at the start of a line (Ruby `=begin`).
    line_start_only: bool,
}

const fn block(open: &'static str, close: &'static str) -> BlockPair {
    BlockPair {
        open,
        close,
        line_start_only: false,
    }
}

/// Per-language tokenizer configuration.
struct Profile {
    /// Resolved language name (what `auto` decided), for the stats output.
    name: &'static str,
    /// Line-comment openers, **longest first** so `///` matches before `//`.
    line_comments: &'static [&'static str],
    /// Which of `line_comments` mark a doc comment.
    doc_line: &'static [&'static str],
    /// Block-comment pairs, tried in order (`--[[` before the `--` line opener).
    block_comments: &'static [BlockPair],
    /// Which block openers mark a doc comment.
    doc_block: &'static [&'static str],
    /// Characters that open a string literal (consumed, never reported).
    str_delims: &'static [char],
    /// True if `'` opens a character/rune literal rather than a string.
    char_literal: bool,
    /// Recognize Python triple-quoted strings (`"""` / `'''`).
    triple_quotes: bool,
    /// Backtick is a *raw* string (Go): no escape processing.
    raw_backtick: bool,
    /// Recognize Rust raw strings (`r"…"`, `r#"…"#`).
    rust_raw: bool,
    /// A line-start triple-quoted string is a docstring (Python).
    docstring_lang: bool,
    /// `#` only opens a comment at line start or after whitespace (YAML).
    hash_needs_space: bool,
}

/// `/* */` — the C/JS/CSS/SQL block pair.
const C_BLOCKS: &[BlockPair] = &[block("/*", "*/")];
/// `<!-- -->` — markup.
const HTML_BLOCKS: &[BlockPair] = &[block("<!--", "-->")];
/// Lua long-bracket comments; the `--[[` form must be tried before `--[=[`.
const LUA_BLOCKS: &[BlockPair] = &[block("--[[", "]]"), block("--[=[", "]=]")];
/// Ruby's `=begin`/`=end`, which only count at the start of a line.
const RUBY_BLOCKS: &[BlockPair] = &[BlockPair {
    open: "=begin",
    close: "=end",
    line_start_only: true,
}];
/// Languages with no block-comment syntax at all.
const NO_BLOCKS: &[BlockPair] = &[];

/// The C/JS-family defaults every brace-language profile starts from.
const fn c_like(name: &'static str) -> Profile {
    Profile {
        name,
        line_comments: &["///", "//!", "//"],
        doc_line: &["///", "//!"],
        block_comments: C_BLOCKS,
        doc_block: &["/**"],
        str_delims: &['"'],
        char_literal: true,
        triple_quotes: false,
        raw_backtick: false,
        rust_raw: false,
        docstring_lang: false,
        hash_needs_space: false,
    }
}

/// Every language the `language` param accepts, for the error message.
const LANGUAGES: &str = "auto, javascript, typescript, python, java, csharp, c, cpp, go, rust, \
php, ruby, shell, sql, html, css, lua, yaml";

/// Resolve the public `language` value into a profile, resolving `auto`.
fn resolve_profile(language: &str, code: &str) -> Result<Profile, String> {
    let lang = language.trim().to_ascii_lowercase();
    let lang = if lang.is_empty() || lang == "auto" {
        detect_language(code)
    } else {
        lang
    };
    let p = match lang.as_str() {
        "javascript" | "js" | "generic" => Profile {
            str_delims: &['"', '\'', '`'],
            char_literal: false,
            ..c_like("javascript")
        },
        "typescript" | "ts" => Profile {
            str_delims: &['"', '\'', '`'],
            char_literal: false,
            ..c_like("typescript")
        },
        "java" => c_like("java"),
        "csharp" | "cs" | "c#" => c_like("csharp"),
        "c" => c_like("c"),
        "cpp" | "c++" | "cxx" => c_like("cpp"),
        "go" | "golang" => Profile {
            str_delims: &['"', '`'],
            raw_backtick: true,
            ..c_like("go")
        },
        "rust" | "rs" => Profile {
            rust_raw: true,
            ..c_like("rust")
        },
        "php" => Profile {
            line_comments: &["///", "//", "#"],
            doc_line: &["///"],
            str_delims: &['"', '\''],
            char_literal: false,
            ..c_like("php")
        },
        "python" | "py" => Profile {
            name: "python",
            line_comments: &["##", "#"],
            doc_line: &["##"],
            block_comments: NO_BLOCKS,
            doc_block: &[],
            str_delims: &['"', '\''],
            char_literal: false,
            triple_quotes: true,
            raw_backtick: false,
            rust_raw: false,
            docstring_lang: true,
            hash_needs_space: false,
        },
        "ruby" | "rb" => Profile {
            name: "ruby",
            line_comments: &["##", "#"],
            doc_line: &["##"],
            block_comments: RUBY_BLOCKS,
            doc_block: &[],
            str_delims: &['"', '\''],
            char_literal: false,
            triple_quotes: false,
            raw_backtick: false,
            rust_raw: false,
            docstring_lang: false,
            hash_needs_space: false,
        },
        "shell" | "bash" | "sh" => Profile {
            name: "shell",
            line_comments: &["##", "#"],
            doc_line: &["##"],
            block_comments: NO_BLOCKS,
            doc_block: &[],
            str_delims: &['"', '\''],
            char_literal: false,
            triple_quotes: false,
            raw_backtick: false,
            rust_raw: false,
            docstring_lang: false,
            hash_needs_space: false,
        },
        "sql" => Profile {
            name: "sql",
            line_comments: &["--"],
            doc_line: &[],
            block_comments: C_BLOCKS,
            doc_block: &[],
            str_delims: &['"', '\''],
            char_literal: false,
            triple_quotes: false,
            raw_backtick: false,
            rust_raw: false,
            docstring_lang: false,
            hash_needs_space: false,
        },
        "html" | "xml" | "svg" => Profile {
            name: "html",
            line_comments: &[],
            doc_line: &[],
            block_comments: HTML_BLOCKS,
            doc_block: &[],
            // Apostrophes are ordinary text in markup, so don't track strings.
            str_delims: &[],
            char_literal: false,
            triple_quotes: false,
            raw_backtick: false,
            rust_raw: false,
            docstring_lang: false,
            hash_needs_space: false,
        },
        "css" | "scss" | "less" => Profile {
            name: "css",
            line_comments: &["//"],
            doc_line: &[],
            block_comments: C_BLOCKS,
            doc_block: &["/**"],
            str_delims: &['"', '\''],
            char_literal: false,
            triple_quotes: false,
            raw_backtick: false,
            rust_raw: false,
            docstring_lang: false,
            hash_needs_space: false,
        },
        "lua" => Profile {
            name: "lua",
            line_comments: &["---", "--"],
            doc_line: &["---"],
            // The long-bracket block MUST be tried before the `--` line opener.
            block_comments: LUA_BLOCKS,
            doc_block: &[],
            str_delims: &['"', '\''],
            char_literal: false,
            triple_quotes: false,
            raw_backtick: false,
            rust_raw: false,
            docstring_lang: false,
            hash_needs_space: false,
        },
        "yaml" | "yml" | "toml" | "ini" => Profile {
            name: "yaml",
            line_comments: &["##", "#"],
            doc_line: &["##"],
            block_comments: NO_BLOCKS,
            doc_block: &[],
            str_delims: &['"', '\''],
            char_literal: false,
            triple_quotes: false,
            raw_backtick: false,
            rust_raw: false,
            docstring_lang: false,
            hash_needs_space: true,
        },
        other => return Err(format!("unknown language '{other}' (use {LANGUAGES})")),
    };
    Ok(p)
}

/// Guess a profile from the shapes present in the source.
fn detect_language(code: &str) -> String {
    let trimmed = code.trim_start();
    if code.contains("<!--") || (trimmed.starts_with('<') && code.contains("</")) {
        return "html".to_string();
    }
    if code.contains("--[[") || (code.contains("--") && code.contains("local ")) {
        return "lua".to_string();
    }
    let has_slash_comment = code.contains("//");
    if !has_slash_comment {
        let upper = code.to_ascii_uppercase();
        if upper.contains("SELECT ") && code.contains("--") {
            return "sql".to_string();
        }
        let looks_python = code.contains("def ")
            || code.contains("elif ")
            || code.contains("\nimport ")
            || trimmed.starts_with("import ")
            || code.contains("print(");
        if looks_python {
            return "python".to_string();
        }
    }
    "generic".to_string()
}

/// Which comment kinds to act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KindFilter {
    All,
    Line,
    Block,
    Doc,
}

fn parse_kind(s: &str) -> Result<KindFilter, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "all" => Ok(KindFilter::All),
        "line" | "single" => Ok(KindFilter::Line),
        "block" | "multi" => Ok(KindFilter::Block),
        "doc" | "docs" => Ok(KindFilter::Doc),
        other => Err(format!(
            "unknown kind '{other}' (use all, line, block, or doc)"
        )),
    }
}

impl KindFilter {
    fn accepts(self, k: Kind) -> bool {
        match self {
            KindFilter::All => true,
            KindFilter::Line => k == Kind::Line,
            KindFilter::Block => k == Kind::Block,
            KindFilter::Doc => k == Kind::Doc,
        }
    }
}

/// Output shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Output {
    Comments,
    Stripped,
    Json,
    Markdown,
    Stats,
}

fn parse_output(s: &str) -> Result<Output, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "comments" | "list" => Ok(Output::Comments),
        "stripped" | "strip" => Ok(Output::Stripped),
        "json" => Ok(Output::Json),
        "markdown" | "md" => Ok(Output::Markdown),
        "stats" | "summary" => Ok(Output::Stats),
        other => Err(format!(
            "unknown output '{other}' (use comments, stripped, json, markdown, or stats)"
        )),
    }
}

/// Scanner over the source characters, tracking line/column.
struct Scanner {
    chars: Vec<char>,
    pos: usize,
    line: usize,
    col: usize,
}

impl Scanner {
    fn new(code: &str) -> Scanner {
        Scanner {
            chars: code.chars().collect(),
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_at(&self, off: usize) -> Option<char> {
        self.chars.get(self.pos + off).copied()
    }

    /// Advance one char, updating line/column.
    fn bump(&mut self) -> Option<char> {
        let c = self.chars.get(self.pos).copied()?;
        self.pos += 1;
        if c == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(c)
    }

    /// True if the upcoming chars match `s`.
    fn starts_with(&self, s: &str) -> bool {
        let mut i = self.pos;
        for want in s.chars() {
            match self.chars.get(i) {
                Some(&c) if c == want => i += 1,
                _ => return false,
            }
        }
        true
    }

    /// Consume `n` characters.
    fn advance(&mut self, n: usize) {
        for _ in 0..n {
            if self.bump().is_none() {
                break;
            }
        }
    }

    /// True if only whitespace precedes the current position on this line.
    fn at_line_start(&self) -> bool {
        let mut i = self.pos;
        while i > 0 {
            let c = self.chars[i - 1];
            if c == '\n' {
                return true;
            }
            if !c.is_whitespace() {
                return false;
            }
            i -= 1;
        }
        true
    }

    /// The character before the current position, if any.
    fn prev(&self) -> Option<char> {
        if self.pos == 0 {
            None
        } else {
            self.chars.get(self.pos - 1).copied()
        }
    }

    /// The source text of the character range `[start, end)`.
    fn slice(&self, start: usize, end: usize) -> String {
        self.chars[start..end.min(self.chars.len())]
            .iter()
            .collect()
    }
}

/// Extract (or remove) the comments in `code`, returning the chosen rendering.
#[allow(clippy::too_many_arguments)]
pub fn extract(
    code: &str,
    language: &str,
    output: &str,
    kind: &str,
    strip_markers: bool,
    line_numbers: bool,
    min_length: i64,
    docstrings: bool,
) -> Result<String, String> {
    if code.trim().is_empty() {
        return Err("code is empty — paste some source to extract comments from".into());
    }
    if min_length < 0 {
        return Err("min_length must be 0 or greater".into());
    }
    let profile = resolve_profile(language, code)?;
    let out = parse_output(output)?;
    let filter = parse_kind(kind)?;
    let min_len = min_length as usize;

    let mut comments = scan(code, &profile, docstrings);
    comments.retain(|c| {
        filter.accepts(c.kind) && clean(&c.raw, c.kind, strip_markers).chars().count() >= min_len
    });

    let mut truncated = false;
    if comments.len() > MAX_COMMENTS {
        comments.truncate(MAX_COMMENTS);
        truncated = true;
    }

    let rendered = match out {
        Output::Comments => render_list(&comments, strip_markers, line_numbers),
        Output::Stripped => strip_source(code, &comments),
        Output::Json => render_json(&comments, strip_markers),
        Output::Markdown => render_markdown(&comments, strip_markers),
        Output::Stats => render_stats(code, &comments, profile.name),
    };
    if truncated {
        return Ok(format!(
            "{rendered}\n\n[output truncated at {MAX_COMMENTS} comments]"
        ));
    }
    Ok(rendered)
}

/// Scan the source and collect every comment.
fn scan(code: &str, p: &Profile, docstrings: bool) -> Vec<Comment> {
    let mut s = Scanner::new(code);
    let mut out = Vec::new();

    'outer: while s.peek().is_some() {
        // Block comments first: `--[[` must beat the `--` line opener.
        for pair in p.block_comments {
            if !s.starts_with(pair.open) || (pair.line_start_only && !s.at_line_start()) {
                continue;
            }
            let (line, column, start) = (s.line, s.col, s.pos);
            s.advance(pair.open.chars().count());
            while s.peek().is_some() && !s.starts_with(pair.close) {
                s.bump();
            }
            if s.peek().is_some() {
                s.advance(pair.close.chars().count());
            }
            let kind = if p.doc_block.contains(&pair.open)
                || p.doc_block
                    .iter()
                    .any(|d| s.slice(start, s.pos).starts_with(d))
            {
                Kind::Doc
            } else {
                Kind::Block
            };
            let end = s.pos;
            out.push(Comment {
                line,
                column,
                end_line: s.line,
                kind,
                raw: s.slice(start, end),
                start,
                end,
            });
            continue 'outer;
        }

        // Line comments, longest opener first.
        for lc in p.line_comments {
            if !s.starts_with(lc) {
                continue;
            }
            if p.hash_needs_space
                && lc.starts_with('#')
                && s.prev().is_some_and(|c| !c.is_whitespace())
            {
                continue;
            }
            let (line, column, start) = (s.line, s.col, s.pos);
            while let Some(ch) = s.peek() {
                if ch == '\n' {
                    break;
                }
                s.bump();
            }
            let end = s.pos;
            out.push(Comment {
                line,
                column,
                end_line: line,
                kind: if p.doc_line.contains(lc) {
                    Kind::Doc
                } else {
                    Kind::Line
                },
                raw: s.slice(start, end),
                start,
                end,
            });
            continue 'outer;
        }

        let c = match s.peek() {
            Some(c) => c,
            None => break,
        };

        // Rust raw string: r"…" / r#"…"#
        if p.rust_raw
            && c == 'r'
            && matches!(s.peek_at(1), Some('"') | Some('#'))
            && skip_rust_raw(&mut s)
        {
            continue;
        }

        // Python triple-quoted string — a docstring when it opens its own line.
        if p.triple_quotes
            && (c == '"' || c == '\'')
            && s.peek_at(1) == Some(c)
            && s.peek_at(2) == Some(c)
        {
            let is_docstring = p.docstring_lang && docstrings && s.at_line_start();
            let (line, column, start) = (s.line, s.col, s.pos);
            skip_triple(&mut s, c);
            if is_docstring {
                let end = s.pos;
                out.push(Comment {
                    line,
                    column,
                    end_line: s.line,
                    kind: Kind::Doc,
                    raw: s.slice(start, end),
                    start,
                    end,
                });
            }
            continue;
        }

        // Ordinary string / char literal — consumed, never reported.
        if p.str_delims.contains(&c) {
            skip_string(&mut s, c, c == '`' && p.raw_backtick);
            continue;
        }
        if c == '\'' && p.char_literal {
            skip_char_literal(&mut s);
            continue;
        }
        s.bump();
    }
    out
}

/// Consume a normal string starting at its opening delimiter.
fn skip_string(s: &mut Scanner, delim: char, raw: bool) {
    s.bump(); // opening quote
    let allow_newline = delim == '`';
    while let Some(c) = s.peek() {
        if c == delim {
            s.bump();
            return;
        }
        if c == '\n' && !allow_newline {
            return; // unterminated string; stop at end of line
        }
        if c == '\\' && !raw {
            s.bump();
            s.bump();
            continue;
        }
        s.bump();
    }
}

/// Consume a Python triple-quoted string (`"""…"""` / `'''…'''`).
fn skip_triple(s: &mut Scanner, delim: char) {
    let triple: String = std::iter::repeat(delim).take(3).collect();
    s.advance(3);
    while s.peek().is_some() {
        if s.starts_with(&triple) {
            s.advance(3);
            return;
        }
        if s.peek() == Some('\\') {
            s.bump();
            s.bump();
            continue;
        }
        s.bump();
    }
}

/// Consume a Rust raw string. Returns false (consuming nothing) if it isn't one.
fn skip_rust_raw(s: &mut Scanner) -> bool {
    let mut off = 1;
    let mut hashes = 0;
    while s.peek_at(off) == Some('#') {
        hashes += 1;
        off += 1;
    }
    if s.peek_at(off) != Some('"') {
        return false;
    }
    s.advance(off + 1); // r, the hashes, and the opening quote
    let mut closing = String::from("\"");
    for _ in 0..hashes {
        closing.push('#');
    }
    while s.peek().is_some() {
        if s.starts_with(&closing) {
            s.advance(closing.chars().count());
            return true;
        }
        s.bump();
    }
    true
}

/// Consume a `'…'` char/rune literal.
fn skip_char_literal(s: &mut Scanner) {
    s.bump(); // opening '
    while let Some(c) = s.peek() {
        if c == '\n' {
            return;
        }
        if c == '\\' {
            s.bump();
            s.bump();
            continue;
        }
        if c == '\'' {
            s.bump();
            return;
        }
        s.bump();
    }
}

/// The comment's reportable text: markers removed when `strip_markers` is set.
fn clean(raw: &str, kind: Kind, strip_markers: bool) -> String {
    if !strip_markers {
        return raw.trim_end().to_string();
    }
    let mut body = raw.to_string();
    // Drop the opener/closer.
    for open in [
        "<!--", "--[=[", "--[[", "/**", "/*", "///", "//!", "//", "##", "#", "---", "--", "=begin",
        "\"\"\"", "'''",
    ] {
        if let Some(rest) = body.strip_prefix(open) {
            body = rest.to_string();
            break;
        }
    }
    for close in ["-->", "*/", "]=]", "]]", "=end", "\"\"\"", "'''"] {
        if let Some(rest) = body.strip_suffix(close) {
            body = rest.to_string();
            break;
        }
    }
    // JSDoc/Javadoc continuation: drop a leading `*` from each interior line.
    let lines: Vec<String> = body
        .lines()
        .map(|l| {
            let t = l.trim_start();
            match t.strip_prefix('*') {
                Some(rest) if kind == Kind::Doc || kind == Kind::Block => {
                    rest.strip_prefix(' ').unwrap_or(rest).to_string()
                }
                _ if kind == Kind::Block || kind == Kind::Doc => l.trim().to_string(),
                _ => l.trim_end().to_string(),
            }
        })
        .map(|l| l.trim_end().to_string())
        .collect();
    // Trim blank lines at both ends, and the single space after the opener.
    let mut out: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
    while out.first().is_some_and(|l| l.trim().is_empty()) {
        out.remove(0);
    }
    while out.last().is_some_and(|l| l.trim().is_empty()) {
        out.pop();
    }
    if out.len() == 1 {
        return out[0].trim().to_string();
    }
    out.join("\n").trim_end().to_string()
}

/// Plain list: one comment per entry, optionally prefixed with its line.
fn render_list(comments: &[Comment], strip_markers: bool, line_numbers: bool) -> String {
    if comments.is_empty() {
        return "No comments found.".to_string();
    }
    comments
        .iter()
        .map(|c| {
            let text = clean(&c.raw, c.kind, strip_markers);
            if line_numbers {
                format!("[L{}] {text}", c.line)
            } else {
                text
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The original source with the matched comments removed. Newlines inside a
/// removed block are kept so the remaining code's line numbers don't shift.
fn strip_source(code: &str, comments: &[Comment]) -> String {
    let chars: Vec<char> = code.chars().collect();
    let mut removed = vec![false; chars.len()];
    for c in comments {
        for slot in removed
            .iter_mut()
            .take(c.end.min(chars.len()))
            .skip(c.start)
        {
            *slot = true;
        }
    }
    let mut kept = String::with_capacity(code.len());
    for (i, ch) in chars.iter().enumerate() {
        if !removed[i] || *ch == '\n' {
            kept.push(*ch);
        }
    }
    kept.lines()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}

/// JSON array of `{line, column, end_line, kind, text}` objects.
fn render_json(comments: &[Comment], strip_markers: bool) -> String {
    let items: Vec<Value> = comments
        .iter()
        .map(|c| {
            json!({
                "line": c.line,
                "column": c.column,
                "end_line": c.end_line,
                "kind": c.kind.name(),
                "text": clean(&c.raw, c.kind, strip_markers),
            })
        })
        .collect();
    serde_json::to_string_pretty(&Value::Array(items)).unwrap_or_else(|_| "[]".to_string())
}

/// Markdown table with one row per comment.
fn render_markdown(comments: &[Comment], strip_markers: bool) -> String {
    let mut s = String::from("| Line | Kind | Comment |\n| --- | --- | --- |\n");
    if comments.is_empty() {
        s.push_str("| — | — | No comments found. |");
        return s;
    }
    for c in comments {
        let text = clean(&c.raw, c.kind, strip_markers)
            .replace('|', "\\|")
            .replace('\n', "<br>");
        s.push_str(&format!("| {} | {} | {} |\n", c.line, c.kind.name(), text));
    }
    s.trim_end().to_string()
}

/// Counts + comment density for the whole input.
fn render_stats(code: &str, comments: &[Comment], language: &str) -> String {
    let total_lines = code.lines().count();
    let blank_lines = code.lines().filter(|l| l.trim().is_empty()).count();
    let mut comment_lines: Vec<usize> = comments.iter().flat_map(|c| c.line..=c.end_line).collect();
    comment_lines.sort_unstable();
    comment_lines.dedup();
    let comment_lines = comment_lines.len();

    let stripped = strip_source(code, comments);
    let code_lines = stripped.lines().filter(|l| !l.trim().is_empty()).count();

    let (mut line_n, mut block_n, mut doc_n) = (0usize, 0usize, 0usize);
    for c in comments {
        match c.kind {
            Kind::Line => line_n += 1,
            Kind::Block => block_n += 1,
            Kind::Doc => doc_n += 1,
        }
    }
    let non_blank = total_lines.saturating_sub(blank_lines);
    let density = if non_blank == 0 {
        0.0
    } else {
        comment_lines as f64 * 100.0 / non_blank as f64
    };
    format!(
        "Language: {language}\nComments: {} ({line_n} line, {block_n} block, {doc_n} doc)\nComment lines: {comment_lines}\nCode lines: {code_lines}\nBlank lines: {blank_lines}\nTotal lines: {total_lines}\nComment density: {density:.1}% (comment lines / non-blank lines)",
        comments.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(code: &str, lang: &str, out: &str) -> String {
        extract(code, lang, out, "all", true, false, 0, true).unwrap()
    }

    #[test]
    fn extracts_line_and_block_comments_from_javascript() {
        let code = "// hello\nconst a = 1; // trailing\n/* block\n   comment */\n";
        assert_eq!(
            run(code, "javascript", "comments"),
            "hello\ntrailing\nblock\ncomment"
        );
    }

    #[test]
    fn ignores_comment_markers_inside_strings() {
        let code = "const url = \"https://example.com//path\"; // real\nconst s = '/* not a comment */';\n";
        assert_eq!(run(code, "javascript", "comments"), "real");
    }

    #[test]
    fn classifies_doc_comments() {
        let code = "/** Doc block */\n/// Doc line\n// Plain line\n/* Plain block */\n";
        let json = extract(code, "rust", "json", "doc", true, false, 0, true).unwrap();
        let v: Value = serde_json::from_str(&json).unwrap();
        let texts: Vec<&str> = v
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o["text"].as_str().unwrap())
            .collect();
        assert_eq!(texts, vec!["Doc block", "Doc line"]);
    }

    #[test]
    fn kind_filter_selects_only_line_comments() {
        let code = "// one\n/* two */\n/** three */\n";
        assert_eq!(run(code, "javascript", "comments"), "one\ntwo\nthree");
        assert_eq!(
            extract(code, "javascript", "comments", "line", true, false, 0, true).unwrap(),
            "one"
        );
    }

    #[test]
    fn strips_comments_preserving_line_numbers() {
        let code = "// header\nlet a = 1; // trailing\n/* two\n   lines */\nlet b = 2;\n";
        assert_eq!(
            run(code, "javascript", "stripped"),
            "\nlet a = 1;\n\n\nlet b = 2;"
        );
    }

    #[test]
    fn strip_can_keep_doc_comments_by_filtering_kind() {
        let code = "/** keep me */\n// drop me\nlet a = 1;\n";
        assert_eq!(
            extract(code, "javascript", "stripped", "line", true, false, 0, true).unwrap(),
            "/** keep me */\n\nlet a = 1;"
        );
    }

    #[test]
    fn keeps_markers_when_strip_markers_is_off() {
        let code = "// hello\n";
        assert_eq!(
            extract(code, "javascript", "comments", "all", false, false, 0, true).unwrap(),
            "// hello"
        );
    }

    #[test]
    fn python_hash_comments_and_docstrings() {
        let code =
            "\"\"\"Module docstring.\"\"\"\nimport os  # trailing\nname = \"# not a comment\"\n";
        assert_eq!(
            run(code, "python", "comments"),
            "Module docstring.\ntrailing"
        );
        // With docstrings off, the triple-quoted string is just a string.
        assert_eq!(
            extract(code, "python", "comments", "all", true, false, 0, false).unwrap(),
            "trailing"
        );
    }

    #[test]
    fn html_and_sql_and_lua_syntaxes() {
        assert_eq!(
            run("<p>hi</p>\n<!-- markup note -->\n", "html", "comments"),
            "markup note"
        );
        assert_eq!(
            run("SELECT 1; -- sql note\n/* block */\n", "sql", "comments"),
            "sql note\nblock"
        );
        assert_eq!(
            run(
                "--[[ long note ]]\nlocal x = 1 -- short\n",
                "lua",
                "comments"
            ),
            "long note\nshort"
        );
    }

    #[test]
    fn yaml_hash_needs_whitespace_before_it() {
        let code = "url: http://example.com/p#frag\nport: 80 # the port\n";
        assert_eq!(run(code, "yaml", "comments"), "the port");
    }

    #[test]
    fn rust_raw_strings_are_not_comments() {
        let code = "let s = r#\"// not a comment\"#; // real one\n";
        assert_eq!(run(code, "rust", "comments"), "real one");
    }

    #[test]
    fn go_backtick_raw_string_is_skipped() {
        let code = "s := `// inside raw`\n// outside\n";
        assert_eq!(run(code, "go", "comments"), "outside");
    }

    #[test]
    fn char_literal_apostrophe_does_not_swallow_the_line() {
        let code = "char c = '\\''; // after a char literal\n";
        assert_eq!(run(code, "c", "comments"), "after a char literal");
    }

    #[test]
    fn ruby_begin_end_block_only_at_line_start() {
        let code = "=begin\nnotes here\n=end\nx = 1 # inline\n";
        assert_eq!(run(code, "ruby", "comments"), "notes here\ninline");
    }

    #[test]
    fn javadoc_continuation_stars_are_removed() {
        let code = "/**\n * First line.\n * Second line.\n */\n";
        assert_eq!(run(code, "java", "comments"), "First line.\nSecond line.");
    }

    #[test]
    fn line_numbers_and_min_length_filters() {
        let code = "// a\n// a longer comment\n";
        assert_eq!(
            extract(code, "javascript", "comments", "all", true, true, 0, true).unwrap(),
            "[L1] a\n[L2] a longer comment"
        );
        assert_eq!(
            extract(code, "javascript", "comments", "all", true, false, 5, true).unwrap(),
            "a longer comment"
        );
    }

    #[test]
    fn markdown_table_escapes_pipes_and_newlines() {
        let code = "/* one | two\n   three */\n";
        assert_eq!(
            run(code, "javascript", "markdown"),
            "| Line | Kind | Comment |\n| --- | --- | --- |\n| 1 | block | one \\| two<br>three |"
        );
    }

    #[test]
    fn json_carries_positions_and_kind() {
        let code = "let a = 1; // note\n";
        let v: Value = serde_json::from_str(&run(code, "javascript", "json")).unwrap();
        assert_eq!(v[0]["line"], 1);
        assert_eq!(v[0]["column"], 12);
        assert_eq!(v[0]["end_line"], 1);
        assert_eq!(v[0]["kind"], "line");
        assert_eq!(v[0]["text"], "note");
    }

    #[test]
    fn stats_reports_counts_and_density() {
        let code = "// header\nlet a = 1;\n\nlet b = 2; // trailing\n";
        assert_eq!(
            run(code, "javascript", "stats"),
            "Language: javascript\nComments: 2 (2 line, 0 block, 0 doc)\nComment lines: 2\nCode lines: 2\nBlank lines: 1\nTotal lines: 4\nComment density: 66.7% (comment lines / non-blank lines)"
        );
    }

    #[test]
    fn auto_detects_python_html_and_generic() {
        assert_eq!(
            run("def f():\n    pass  # note\n", "auto", "comments"),
            "note"
        );
        assert_eq!(
            run("<div>\n<!-- note -->\n</div>\n", "auto", "comments"),
            "note"
        );
        assert_eq!(run("let a = 1; // note\n", "auto", "comments"), "note");
    }

    #[test]
    fn no_comments_reports_so_rather_than_empty_output() {
        assert_eq!(
            run("let a = 1;\n", "javascript", "comments"),
            "No comments found."
        );
    }

    #[test]
    fn unterminated_block_comment_runs_to_end_of_input() {
        assert_eq!(
            run("let a = 1;\n/* never closed\n", "javascript", "comments"),
            "never closed"
        );
    }

    #[test]
    fn empty_code_is_an_error() {
        let err = extract("   \n", "auto", "comments", "all", true, false, 0, true).unwrap_err();
        assert!(err.contains("code is empty"), "{err}");
    }

    #[test]
    fn unknown_language_is_an_error() {
        let err = extract("x", "klingon", "comments", "all", true, false, 0, true).unwrap_err();
        assert!(err.contains("unknown language 'klingon'"), "{err}");
    }

    #[test]
    fn unknown_output_and_kind_are_errors() {
        let err = extract("x", "auto", "yaml-table", "all", true, false, 0, true).unwrap_err();
        assert!(err.contains("unknown output"), "{err}");
        let err = extract("x", "auto", "comments", "inline", true, false, 0, true).unwrap_err();
        assert!(err.contains("unknown kind"), "{err}");
    }

    #[test]
    fn negative_min_length_is_an_error() {
        let err = extract("// x", "auto", "comments", "all", true, false, -1, true).unwrap_err();
        assert!(err.contains("min_length must be 0 or greater"), "{err}");
    }
}

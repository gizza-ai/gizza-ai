//! c-vuln-pattern-scanner core — pure, deterministic vulnerability-pattern
//! heuristics for pasted C/C++ source.
//!
//! Nothing is compiled, preprocessed, linked or executed: the snippet is scanned
//! as text. Comments and the bodies of string/char literals are blanked before
//! the rules run (the quotes are kept), so a dangerous function name inside a
//! comment or a string does not produce a finding.
//!
//! This is a lexical pass, not a parser. There is no control flow, data flow,
//! type or scope information — the same limitation every tool in this class has.
//! Findings mean "worth a human look", never "proven vulnerability".

use serde_json::json;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;

/// Largest snippet accepted, in bytes. Larger input is rejected rather than
/// silently truncated.
pub const MAX_INPUT_BYTES: usize = 200_000;

/// In-source marker that suppresses findings on its own line or the next one.
pub const SUPPRESS_MARKER: &str = "vuln-scan: ignore";

// ---------------------------------------------------------------------------
// Severity / language / profile / format
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

impl Severity {
    pub fn label(self) -> &'static str {
        match self {
            Severity::Low => "low",
            Severity::Medium => "medium",
            Severity::High => "high",
            Severity::Critical => "critical",
        }
    }

    fn parse_min(s: &str) -> Result<Severity, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "all" | "low" => Severity::Low,
            "medium" => Severity::Medium,
            "high" => Severity::High,
            "critical" => Severity::Critical,
            other => {
                return Err(format!(
                "unknown min_severity '{other}' (use all, low, medium, high, or critical)"
            ))
            }
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    C,
    Cpp,
}

impl Lang {
    fn label(self) -> &'static str {
        match self {
            Lang::C => "c",
            Lang::Cpp => "cpp",
        }
    }
}

fn parse_language(s: &str) -> Result<Option<Lang>, String> {
    Ok(match s.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => None,
        "c" => Some(Lang::C),
        "cpp" | "c++" => Some(Lang::Cpp),
        other => return Err(format!("unknown language '{other}' (use auto, c, or cpp)")),
    })
}

/// C++ markers looked for by `language = auto`.
const CPP_MARKERS: [&str; 7] = [
    "#include <iostream>",
    "std::",
    "class ",
    "namespace ",
    "template<",
    "template <",
    "using namespace",
];

fn detect_language(masked: &str) -> Lang {
    if CPP_MARKERS.iter().any(|m| masked.contains(m)) {
        Lang::Cpp
    } else {
        Lang::C
    }
}

fn parse_profile(s: &str) -> Result<&'static str, String> {
    Ok(match s.trim().to_ascii_lowercase().as_str() {
        "" | "all" => "all",
        "memory" => "memory",
        "injection" => "injection",
        "crypto" => "crypto",
        "banned" => "banned",
        other => {
            return Err(format!(
                "unknown profile '{other}' (use all, memory, injection, crypto, or banned)"
            ))
        }
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    Text,
    Json,
    Csv,
}

impl Format {
    fn parse(s: &str) -> Result<Format, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "text" | "report" => Format::Text,
            "json" => Format::Json,
            "csv" => Format::Csv,
            other => return Err(format!("unknown format '{other}' (use text, json, or csv)")),
        })
    }
}

// ---------------------------------------------------------------------------
// Rule table
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rule {
    pub code: &'static str,
    /// Common Weakness Enumeration id reported with every finding.
    pub cwe: u16,
    pub severity: Severity,
    /// Rule families the `profile` filter selects on.
    pub families: &'static [&'static str],
}

/// Every rule this scanner can emit. `profile` selects families; `ignore`
/// suppresses individual codes.
pub const RULES: &[Rule] = &[
    Rule { code: "GETS", cwe: 242, severity: Severity::Critical, families: &["memory", "banned"] },
    Rule { code: "BUFFER-OVERRUN", cwe: 787, severity: Severity::Critical, families: &["memory"] },
    Rule { code: "BANNED-COPY", cwe: 120, severity: Severity::High, families: &["memory", "banned"] },
    Rule { code: "SCANF-UNBOUNDED", cwe: 120, severity: Severity::High, families: &["memory", "banned"] },
    Rule { code: "FORMAT-STRING", cwe: 134, severity: Severity::High, families: &["injection"] },
    Rule { code: "COMMAND-EXEC", cwe: 78, severity: Severity::High, families: &["injection", "banned"] },
    Rule { code: "USE-AFTER-FREE", cwe: 416, severity: Severity::High, families: &["memory"] },
    Rule { code: "SIZEOF-POINTER", cwe: 467, severity: Severity::High, families: &["memory"] },
    Rule { code: "CPP-STREAM", cwe: 120, severity: Severity::High, families: &["memory"] },
    Rule { code: "OFF-BY-ONE", cwe: 193, severity: Severity::Medium, families: &["memory"] },
    Rule { code: "INT-OVERFLOW", cwe: 190, severity: Severity::Medium, families: &["memory"] },
    Rule { code: "SIGN-CONVERSION", cwe: 195, severity: Severity::Medium, families: &["memory"] },
    Rule { code: "UNBOUNDED-ALLOC", cwe: 770, severity: Severity::Medium, families: &["memory"] },
    Rule { code: "UNCHECKED-ALLOC", cwe: 476, severity: Severity::Medium, families: &["memory"] },
    Rule { code: "TEMP-FILE", cwe: 377, severity: Severity::Medium, families: &["injection", "banned"] },
    Rule { code: "TOCTOU", cwe: 367, severity: Severity::Medium, families: &["injection"] },
    Rule { code: "WEAK-RANDOM", cwe: 330, severity: Severity::Medium, families: &["crypto", "banned"] },
    Rule { code: "WEAK-CRYPTO", cwe: 327, severity: Severity::Medium, families: &["crypto", "banned"] },
    Rule { code: "BOUNDED-COPY", cwe: 120, severity: Severity::Low, families: &["memory", "banned"] },
    Rule { code: "MEM-LEAK", cwe: 401, severity: Severity::Low, families: &["memory"] },
];

fn rule_of(code: &str) -> &'static Rule {
    RULES
        .iter()
        .find(|r| r.code == code)
        .expect("finding code is always a declared rule")
}

fn parse_ignore(s: &str) -> BTreeSet<&'static str> {
    let mut set = BTreeSet::new();
    for part in s.split(|c: char| c == ',' || c.is_whitespace()) {
        let upper = part.trim().to_ascii_uppercase();
        if upper.is_empty() {
            continue;
        }
        if let Some(r) = RULES.iter().find(|r| r.code == upper) {
            set.insert(r.code);
        }
    }
    set
}

// ---------------------------------------------------------------------------
// Masking
// ---------------------------------------------------------------------------

/// Result of the masking pass. Both strings keep the input's line structure.
///
/// * `code` — comments and literal bodies blanked (quotes kept), so a flagged
///   name inside a comment or a string cannot fire a rule.
/// * `text` — comments blanked, literals intact. Rules that must read a format
///   string (`SCANF-UNBOUNDED`) use this one.
struct Masked {
    code: String,
    text: String,
}

fn mask(src: &str) -> Masked {
    #[derive(PartialEq)]
    enum St {
        Normal,
        Line,
        Block,
        Str,
        Chr,
    }
    let chars: Vec<char> = src.chars().collect();
    let mut code = String::with_capacity(src.len());
    let mut text = String::with_capacity(src.len());
    let mut st = St::Normal;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied().unwrap_or('\0');
        match st {
            St::Normal => {
                if c == '/' && next == '/' {
                    st = St::Line;
                    code.push_str("  ");
                    text.push_str("  ");
                    i += 2;
                    continue;
                }
                if c == '/' && next == '*' {
                    st = St::Block;
                    code.push_str("  ");
                    text.push_str("  ");
                    i += 2;
                    continue;
                }
                if c == '"' {
                    st = St::Str;
                } else if c == '\'' {
                    st = St::Chr;
                }
                code.push(c);
                text.push(c);
            }
            St::Line => {
                if c == '\n' {
                    st = St::Normal;
                    code.push('\n');
                    text.push('\n');
                } else {
                    code.push(' ');
                    text.push(' ');
                }
            }
            St::Block => {
                if c == '*' && next == '/' {
                    st = St::Normal;
                    code.push_str("  ");
                    text.push_str("  ");
                    i += 2;
                    continue;
                }
                let ch = if c == '\n' { '\n' } else { ' ' };
                code.push(ch);
                text.push(ch);
            }
            St::Str | St::Chr => {
                let closing = if st == St::Str { '"' } else { '\'' };
                if c == '\\' {
                    // Escape: the backslash and the escaped char are one unit.
                    code.push_str("  ");
                    text.push(c);
                    text.push(next);
                    i += 2;
                    continue;
                }
                if c == closing {
                    st = St::Normal;
                    code.push(c);
                    text.push(c);
                } else if c == '\n' {
                    // Unterminated literal — recover at the line break.
                    st = St::Normal;
                    code.push('\n');
                    text.push('\n');
                } else {
                    code.push(' ');
                    text.push(c);
                }
            }
        }
        i += 1;
    }
    Masked { code, text }
}

// ---------------------------------------------------------------------------
// Lexical helpers
// ---------------------------------------------------------------------------

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// `(name_start, paren_index)` for every call to `name` in `line`.
fn call_sites(line: &str, name: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(rel) = line[from..].find(name) {
        let start = from + rel;
        let after = start + name.len();
        from = after;
        if let Some(prev) = line[..start].chars().last() {
            if is_ident_char(prev) || prev == '.' || prev == '#' {
                continue;
            }
            if prev == '>' && line[..start].ends_with("->") {
                continue;
            }
        }
        let rest = &line[after..];
        let trimmed = rest.trim_start();
        if !trimmed.starts_with('(') {
            continue;
        }
        out.push((start, after + (rest.len() - trimmed.len())));
    }
    out
}

/// Top-level, comma-separated arguments of the call whose `(` is at `paren`.
/// A call that continues on the next line yields the arguments seen so far.
fn args_at(line: &str, paren: usize) -> Vec<String> {
    let mut depth = 0i32;
    let mut cur = String::new();
    let mut out = Vec::new();
    for c in line[paren..].chars() {
        match c {
            '(' => {
                depth += 1;
                if depth > 1 {
                    cur.push(c);
                }
            }
            ')' => {
                depth -= 1;
                if depth == 0 {
                    out.push(cur.trim().to_string());
                    return out;
                }
                cur.push(c);
            }
            '[' => {
                depth += 1;
                cur.push(c);
            }
            ']' => {
                depth -= 1;
                cur.push(c);
            }
            ',' if depth == 1 => {
                out.push(cur.trim().to_string());
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    let tail = cur.trim();
    if !tail.is_empty() {
        out.push(tail.to_string());
    }
    out
}

/// Identifier tokens of a line with their byte offsets.
fn words(line: &str) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if is_ident_char(bytes[i] as char) {
            let start = i;
            while i < bytes.len() && is_ident_char(bytes[i] as char) {
                i += 1;
            }
            out.push((line[start..i].to_string(), start));
        } else {
            i += 1;
        }
    }
    out
}

fn contains_word(line: &str, word: &str) -> bool {
    let mut from = 0;
    while let Some(rel) = line[from..].find(word) {
        let start = from + rel;
        let end = start + word.len();
        from = end;
        let left_ok = line[..start]
            .chars()
            .last()
            .map(|c| !is_ident_char(c))
            .unwrap_or(true);
        let right_ok = line[end..]
            .chars()
            .next()
            .map(|c| !is_ident_char(c))
            .unwrap_or(true);
        if left_ok && right_ok {
            return true;
        }
    }
    false
}

const TYPE_WORDS: [&str; 24] = [
    "char", "unsigned", "signed", "int", "short", "long", "float", "double", "void", "struct",
    "union", "const", "static", "wchar_t", "size_t", "uint8_t", "uint16_t", "uint32_t", "uint64_t",
    "int8_t", "int16_t", "int32_t", "int64_t", "FILE",
];

const NON_DECL_WORDS: [&str; 12] = [
    "return", "if", "while", "for", "switch", "case", "else", "sizeof", "new", "delete", "do",
    "goto",
];

// ---------------------------------------------------------------------------
// Declaration pre-pass
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Decls {
    /// Declared fixed-size arrays: name → element count.
    arrays: HashMap<String, usize>,
    /// `(line index, byte offset of the name)` of each array declaration, so the
    /// bound checks can skip the declaration itself.
    decl_sites: HashSet<(usize, usize)>,
    /// Variable-length arrays: `(line index, name, size token)`.
    vlas: Vec<(usize, String, String)>,
    pointers: HashSet<String>,
    /// Variables declared with a signed integer type.
    signed_ints: HashSet<String>,
}

fn collect_decls(masked: &[&str]) -> Decls {
    let mut d = Decls::default();
    for (idx, line) in masked.iter().enumerate() {
        let toks = words(line);

        // Pointer + signed-integer declarations: `char *p`, `int len`.
        for (w, off) in toks.iter() {
            if !TYPE_WORDS.contains(&w.as_str()) {
                continue;
            }
            let Some((next, next_off)) = toks.iter().find(|(_, o)| *o > *off) else {
                continue;
            };
            if TYPE_WORDS.contains(&next.as_str()) || NON_DECL_WORDS.contains(&next.as_str()) {
                continue;
            }
            let gap = &line[off + w.len()..*next_off];
            if !gap.chars().all(|c| c.is_whitespace() || c == '*') {
                continue;
            }
            if gap.contains('*') {
                d.pointers.insert(next.clone());
            } else if matches!(w.as_str(), "int" | "short" | "long" | "signed")
                && !toks
                    .iter()
                    .any(|(p, po)| p == "unsigned" && *po < *off && off - po <= 12)
            {
                d.signed_ints.insert(next.clone());
            }
        }

        // Array declarations: `char buf[16]`, `char path[len]`.
        for (open, _) in line.char_indices().filter(|(_, c)| *c == '[') {
            let before = &line[..open];
            let name_end = before.trim_end().len();
            let name_start = before[..name_end]
                .rfind(|c: char| !is_ident_char(c))
                .map(|i| i + 1)
                .unwrap_or(0);
            if name_start >= name_end {
                continue;
            }
            let name = &line[name_start..name_end];
            // The token before the name must look like a declaration head.
            let head = line[..name_start].trim_end();
            let head_word = head
                .rfind(|c: char| !is_ident_char(c) && c != '*')
                .map(|i| &head[i + 1..])
                .unwrap_or(head)
                .trim_start_matches('*');
            let head_is_decl = !head_word.is_empty()
                && head_word.chars().all(is_ident_char)
                && !NON_DECL_WORDS.contains(&head_word);
            if !head_is_decl {
                continue;
            }
            let Some(close_rel) = line[open..].find(']') else {
                continue;
            };
            let size = line[open + 1..open + close_rel].trim().to_string();
            d.decl_sites.insert((idx, name_start));
            if let Ok(n) = size.parse::<usize>() {
                d.arrays.insert(name.to_string(), n);
            } else if !size.is_empty()
                && size.chars().all(is_ident_char)
                && size.chars().any(|c| c.is_ascii_lowercase())
            {
                d.vlas.push((idx, name.to_string(), size));
            }
        }
    }
    d
}

// ---------------------------------------------------------------------------
// Rules
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Finding {
    line: usize,
    code: &'static str,
    message: String,
    source: String,
}

/// Format-string sinks: function name → index of the format argument.
const FORMAT_SINKS: [(&str, usize); 17] = [
    ("printf", 0),
    ("vprintf", 0),
    ("wprintf", 0),
    ("scanf", 0),
    ("vscanf", 0),
    ("fprintf", 1),
    ("vfprintf", 1),
    ("fwprintf", 1),
    ("sprintf", 1),
    ("vsprintf", 1),
    ("dprintf", 1),
    ("asprintf", 1),
    ("syslog", 1),
    ("fscanf", 1),
    ("sscanf", 1),
    ("snprintf", 2),
    ("vsnprintf", 2),
];

/// scanf-family readers: function name → index of the format argument.
const SCANF_SINKS: [(&str, usize); 6] = [
    ("scanf", 0),
    ("vscanf", 0),
    ("fscanf", 1),
    ("vfscanf", 1),
    ("sscanf", 1),
    ("vsscanf", 1),
];

const BANNED_COPIES: [&str; 7] = [
    "strcpy", "strcat", "wcscpy", "wcscat", "stpcpy", "sprintf", "vsprintf",
];

const BOUNDED_COPIES: [&str; 5] = ["strncpy", "strncat", "wcsncpy", "wcsncat", "strncat_s"];

const EXEC_CALLS: [&str; 10] = [
    "system", "popen", "_popen", "execl", "execlp", "execle", "execv", "execvp", "execvpe",
    "WinExec",
];

const TEMP_CALLS: [&str; 3] = ["tmpnam", "tempnam", "mktemp"];

const RANDOM_CALLS: [&str; 6] = ["rand", "srand", "random", "srandom", "drand48", "lrand48"];

const WEAK_CRYPTO_TOKENS: [&str; 9] = [
    "MD5", "MD4", "SHA1", "RC4", "md5", "sha1", "rc4", "DES_set_key", "DES_ecb_encrypt",
];

const ALLOCATORS: [(&str, usize); 4] = [
    ("malloc", 0),
    ("realloc", 1),
    ("alloca", 0),
    ("calloc", usize::MAX), // both arguments are size-ish
];

/// Calls whose length argument must not be a signed int: name → argument index.
const LENGTH_ARGS: [(&str, usize); 10] = [
    ("memcpy", 2),
    ("memmove", 2),
    ("memset", 2),
    ("strncpy", 2),
    ("strncat", 2),
    ("malloc", 0),
    ("alloca", 0),
    ("read", 2),
    ("recv", 2),
    ("snprintf", 1),
];

/// Calls that write `n` bytes into a destination buffer: name → (dst index, size index).
const SIZED_WRITES: [(&str, usize, usize); 8] = [
    ("memcpy", 0, 2),
    ("memmove", 0, 2),
    ("memset", 0, 2),
    ("strncpy", 0, 2),
    ("strlcpy", 0, 2),
    ("snprintf", 0, 1),
    ("fgets", 0, 1),
    ("strlcat", 0, 2),
];

fn first_ident(s: &str) -> Option<String> {
    let t = s.trim().trim_start_matches(|c| c == '&' || c == '*' || c == '(');
    let w = t
        .chars()
        .take_while(|c| is_ident_char(*c))
        .collect::<String>();
    if w.is_empty() {
        None
    } else {
        Some(w)
    }
}

/// True when `arg` is a string literal (possibly a concatenation) or a
/// gettext-style macro wrapping one.
fn is_literal_format(arg: &str) -> bool {
    let t = arg.trim();
    t.starts_with('"') || t.starts_with("_(\"") || t.starts_with("N_(\"") || t.starts_with("L\"")
}

/// `%s` / `%[` conversions with no field width in a scanf format literal.
fn scanf_lacks_width(fmt: &str) -> bool {
    let chars: Vec<char> = fmt.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '%' {
            i += 1;
            continue;
        }
        i += 1;
        if i < chars.len() && chars[i] == '%' {
            i += 1;
            continue;
        }
        if i < chars.len() && chars[i] == '*' {
            // Assignment-suppressing conversion: nothing is stored.
            i += 1;
            continue;
        }
        let mut width = false;
        while i < chars.len() && chars[i].is_ascii_digit() {
            width = true;
            i += 1;
        }
        if i < chars.len() && (chars[i] == 's' || chars[i] == '[') && !width {
            return true;
        }
    }
    false
}

fn arithmetic_size(arg: &str) -> bool {
    let has_op = arg.contains('*') || arg.contains('+') || arg.contains("<<");
    has_op && arg.chars().any(|c| c.is_ascii_alphabetic())
}

fn looks_like_null_check(line: &str, name: &str) -> bool {
    if !contains_word(line, name) {
        return false;
    }
    let has_guard = line.contains("if")
        || line.contains("assert")
        || line.contains("?")
        || line.contains("while");
    has_guard
        && (line.contains("NULL")
            || line.contains("nullptr")
            || line.contains(&format!("!{name}"))
            || line.contains("== 0")
            || line.contains("!= 0"))
}

#[allow(clippy::too_many_lines)]
fn scan_lines(raw: &[&str], masked: &[&str], text: &[&str], d: &Decls, lang: Lang) -> Vec<Finding> {
    let mut out: Vec<Finding> = Vec::new();
    let joined = masked.join("\n");
    // Variables freed on an earlier line, with the line they were freed on.
    let mut freed: HashMap<String, usize> = HashMap::new();

    for (idx, line) in masked.iter().enumerate() {
        let no = idx + 1;
        let src = raw[idx].trim();
        let mut push = |code: &'static str, message: String| {
            out.push(Finding {
                line: no,
                code,
                message,
                source: src.chars().take(160).collect(),
            });
        };

        if line.trim().is_empty() {
            continue;
        }

        // A closing brace at column 0 ends the function body the free-tracking
        // window applies to.
        if raw[idx].starts_with('}') {
            freed.clear();
        }

        // --- banned / bounded copies -------------------------------------
        for f in BANNED_COPIES {
            if !call_sites(line, f).is_empty() {
                push(
                    "BANNED-COPY",
                    format!(
                        "{f}() writes until the source ends with no destination bound; use a size-checked copy such as snprintf() or strlcpy()"
                    ),
                );
                break;
            }
        }
        for f in BOUNDED_COPIES {
            if !call_sites(line, f).is_empty() {
                push(
                    "BOUNDED-COPY",
                    format!(
                        "{f}() truncates silently and may leave the destination without a NUL terminator; check the return value and terminate explicitly"
                    ),
                );
                break;
            }
        }
        if !call_sites(line, "gets").is_empty() {
            push(
                "GETS",
                "gets() has no length argument and overflows on any input longer than the buffer; use fgets() with the destination size".to_string(),
            );
        }

        // --- format strings ----------------------------------------------
        for (f, idx_fmt) in FORMAT_SINKS {
            let Some((_, paren)) = call_sites(line, f).first().copied() else {
                continue;
            };
            let args = args_at(line, paren);
            if args.len() > idx_fmt && !is_literal_format(&args[idx_fmt]) {
                push(
                    "FORMAT-STRING",
                    format!(
                        "{f}() takes '{}' as its format string instead of a literal; a caller-controlled format can read or write memory — use {f}(\"%s\", ...)",
                        args[idx_fmt]
                    ),
                );
                break;
            }
        }

        // --- scanf field widths -------------------------------------------
        for (f, idx_fmt) in SCANF_SINKS {
            let Some((_, paren)) = call_sites(text[idx], f).first().copied() else {
                continue;
            };
            let args = args_at(text[idx], paren);
            if args.len() > idx_fmt && is_literal_format(&args[idx_fmt]) {
                let fmt = &args[idx_fmt];
                if scanf_lacks_width(fmt) {
                    push(
                        "SCANF-UNBOUNDED",
                        format!("{f}() uses a %s or %[ conversion with no field width; the destination can be overrun — write %31s for a 32-byte buffer"),
                    );
                    break;
                }
            }
        }

        // --- allocation size arithmetic ------------------------------------
        for (f, size_idx) in ALLOCATORS {
            let Some((_, paren)) = call_sites(line, f).first().copied() else {
                continue;
            };
            let args = args_at(line, paren);
            let hit = if size_idx == usize::MAX {
                args.iter().any(|a| arithmetic_size(a))
            } else {
                args.get(size_idx).map(|a| arithmetic_size(a)).unwrap_or(false)
            };
            if hit {
                push(
                    "INT-OVERFLOW",
                    format!("{f}() computes its size with arithmetic that can wrap; check the multiplication or addition against SIZE_MAX before allocating"),
                );
                break;
            }
        }

        // --- signed length arguments ---------------------------------------
        for (f, len_idx) in LENGTH_ARGS {
            let Some((_, paren)) = call_sites(line, f).first().copied() else {
                continue;
            };
            let args = args_at(line, paren);
            let Some(arg) = args.get(len_idx) else { continue };
            let Some(name) = first_ident(arg) else { continue };
            if arg.trim() == name && d.signed_ints.contains(&name) {
                push(
                    "SIGN-CONVERSION",
                    format!("length argument '{name}' is declared as a signed integer; a negative value converts to a huge size_t in {f}()"),
                );
                break;
            }
        }

        // --- provable bound violations --------------------------------------
        for (f, dst_idx, size_idx) in SIZED_WRITES {
            let Some((_, paren)) = call_sites(line, f).first().copied() else {
                continue;
            };
            let args = args_at(line, paren);
            let (Some(dst), Some(size)) = (args.get(dst_idx), args.get(size_idx)) else {
                continue;
            };
            let Some(name) = first_ident(dst) else { continue };
            let Some(cap) = d.arrays.get(&name).copied() else {
                continue;
            };
            let Ok(n) = size.trim().parse::<usize>() else {
                continue;
            };
            if n > cap {
                push(
                    "BUFFER-OVERRUN",
                    format!("{f}() writes up to {n} bytes into '{name}', which is declared with room for {cap}"),
                );
                break;
            }
        }

        // --- literal indexes past the declared bound -------------------------
        for (name, cap) in d.arrays.iter() {
            let mut from = 0;
            while let Some(rel) = line[from..].find(name.as_str()) {
                let start = from + rel;
                let end = start + name.len();
                from = end;
                if d.decl_sites.contains(&(idx, start)) {
                    continue;
                }
                let left_ok = line[..start]
                    .chars()
                    .last()
                    .map(|c| !is_ident_char(c) && c != '.')
                    .unwrap_or(true);
                if !left_ok || !line[end..].starts_with('[') {
                    continue;
                }
                let Some(close) = line[end..].find(']') else { break };
                let inside = line[end + 1..end + close].trim();
                let Ok(i) = inside.parse::<usize>() else {
                    continue;
                };
                if i > *cap {
                    push(
                        "BUFFER-OVERRUN",
                        format!("index {i} is past the end of '{name}', declared with {cap} elements (valid indexes are 0..{})", cap.saturating_sub(1)),
                    );
                } else if i == *cap {
                    push(
                        "OFF-BY-ONE",
                        format!("index {i} equals the length of '{name}'; the last valid index is {}", cap.saturating_sub(1)),
                    );
                }
            }
        }

        // --- other off-by-one shapes -----------------------------------------
        if line.contains("<= strlen(") || line.contains("<=strlen(") || line.contains("<= sizeof")
        {
            push(
                "OFF-BY-ONE",
                "loop bound compares with <= against strlen()/sizeof(); the final iteration reads or writes one element past the buffer".to_string(),
            );
        }
        for f in ["malloc", "alloca"] {
            let Some((_, paren)) = call_sites(line, f).first().copied() else {
                continue;
            };
            let args = args_at(line, paren);
            if let Some(a) = args.first() {
                if a.contains("strlen(") && !a.contains('+') {
                    push(
                        "OFF-BY-ONE",
                        format!("{f}(strlen(...)) leaves no room for the NUL terminator; allocate strlen(...) + 1"),
                    );
                }
            }
        }

        // --- unbounded stack allocation ---------------------------------------
        if !call_sites(line, "alloca").is_empty() {
            push(
                "UNBOUNDED-ALLOC",
                "alloca() takes stack space with no limit and cannot report failure; use a bounded heap allocation".to_string(),
            );
        }
        for (vla_line, name, size) in &d.vlas {
            if *vla_line == idx {
                push(
                    "UNBOUNDED-ALLOC",
                    format!("'{name}' is a variable-length array sized by '{size}'; an attacker-controlled length exhausts the stack — bound it or allocate on the heap"),
                );
            }
        }

        // --- allocation result handling ----------------------------------------
        for f in ["malloc", "calloc", "realloc", "strdup"] {
            let Some((name_start, paren)) = call_sites(line, f).first().copied() else {
                continue;
            };
            let lhs = &line[..name_start];
            let Some(eq) = lhs.rfind('=') else { continue };
            if eq > 0 && matches!(&lhs[eq - 1..eq], "=" | "!" | "<" | ">") {
                continue;
            }
            let target = lhs[..eq].trim_end();
            let Some(target) = target
                .rfind(|c: char| !is_ident_char(c))
                .map(|i| &target[i + 1..])
                .or(Some(target))
            else {
                continue;
            };
            if target.is_empty() {
                continue;
            }
            let checked = (idx + 1..=(idx + 5).min(masked.len().saturating_sub(1)))
                .any(|j| looks_like_null_check(masked[j], target))
                || looks_like_null_check(&line[paren..], target);
            if !checked {
                push(
                    "UNCHECKED-ALLOC",
                    format!("the result of {f}() assigned to '{target}' is not checked against NULL before use; a failed allocation dereferences a null pointer"),
                );
            }
            let freed_anywhere = call_sites(&joined, "free")
                .iter()
                .any(|(_, p)| args_at(&joined, *p).first().and_then(|a| first_ident(a)).as_deref() == Some(target));
            let returned = joined.lines().any(|l| {
                let t = l.trim();
                t.starts_with("return") && contains_word(t, target)
            });
            if !freed_anywhere && !returned {
                push(
                    "MEM-LEAK",
                    format!("'{target}' is allocated here but never freed or returned in this snippet; confirm every path releases it"),
                );
            }
            break;
        }

        // --- use after free / double free ----------------------------------------
        let free_targets: Vec<String> = call_sites(line, "free")
            .iter()
            .filter_map(|(_, p)| args_at(line, *p).first().and_then(|a| first_ident(a)))
            .collect();
        let mut reported_uaf = false;
        for (name, free_line) in freed.clone() {
            if idx.saturating_sub(free_line) > 20 {
                freed.remove(&name);
                continue;
            }
            if !contains_word(line, &name) {
                continue;
            }
            if free_targets.contains(&name) {
                push(
                    "USE-AFTER-FREE",
                    format!("'{name}' is freed again after an earlier free(); a double free corrupts the allocator"),
                );
                freed.remove(&name);
                reported_uaf = true;
                continue;
            }
            // A reassignment (including `= NULL`) makes the pointer live again.
            let reassigned = line
                .find(&name)
                .map(|p| {
                    let rest = line[p + name.len()..].trim_start();
                    rest.starts_with('=') && !rest.starts_with("==")
                })
                .unwrap_or(false);
            if reassigned {
                freed.remove(&name);
                continue;
            }
            if looks_like_null_check(line, &name) {
                continue;
            }
            push(
                "USE-AFTER-FREE",
                format!("'{name}' is used after free() on line {}; the memory may already be reallocated", free_line + 1),
            );
            freed.remove(&name);
            reported_uaf = true;
        }
        let _ = reported_uaf;
        for t in free_targets {
            freed.entry(t).or_insert(idx);
        }

        // --- sizeof on a pointer --------------------------------------------------
        for (_, paren) in call_sites(line, "sizeof") {
            let args = args_at(line, paren);
            let Some(name) = args.first().and_then(|a| first_ident(a)) else {
                continue;
            };
            if d.pointers.contains(&name) {
                push(
                    "SIZEOF-POINTER",
                    format!("sizeof({name}) is the size of a pointer, not of the buffer it points at; pass the real length instead"),
                );
                break;
            }
        }

        // --- C++ stream extraction -------------------------------------------------
        if lang == Lang::Cpp {
            if let Some(pos) = line.find("cin").map(|p| p + 3) {
                let rest = line[pos..].trim_start();
                if let Some(after) = rest.strip_prefix(">>") {
                    if let Some(name) = first_ident(after) {
                        if d.arrays.contains_key(&name) || d.pointers.contains(&name) {
                            push(
                                "CPP-STREAM",
                                format!("extracting into '{name}' with >> is unbounded and overflows on long input; use std::string or setw()"),
                            );
                        }
                    }
                }
            }
        }

        // --- command execution --------------------------------------------------
        for f in EXEC_CALLS {
            if !call_sites(line, f).is_empty() {
                push(
                    "COMMAND-EXEC",
                    format!("{f}() runs a command through a shell or an argument vector; never build the command from untrusted text"),
                );
                break;
            }
        }

        // --- temporary files and TOCTOU ------------------------------------------
        for f in TEMP_CALLS {
            if !call_sites(line, f).is_empty() {
                push(
                    "TEMP-FILE",
                    format!("{f}() returns a name, not an open file, so another process can win the race; use mkstemp() or open() with O_CREAT|O_EXCL"),
                );
                break;
            }
        }
        for probe in ["access", "stat", "lstat"] {
            if call_sites(line, probe).is_empty() {
                continue;
            }
            let opens = (idx + 1..=(idx + 5).min(masked.len().saturating_sub(1))).any(|j| {
                ["open", "fopen", "unlink", "chmod", "creat"]
                    .iter()
                    .any(|o| !call_sites(masked[j], o).is_empty())
            });
            if opens {
                push(
                    "TOCTOU",
                    format!("{probe}() checks the path and a later call acts on it; the file can change in between — operate on a file descriptor instead"),
                );
                break;
            }
        }

        // --- weak randomness and crypto -------------------------------------------
        for f in RANDOM_CALLS {
            if !call_sites(line, f).is_empty() {
                push(
                    "WEAK-RANDOM",
                    format!("{f}() is a predictable pseudo-random generator; use a cryptographic source such as getrandom() for keys, tokens or nonces"),
                );
                break;
            }
        }
        for t in WEAK_CRYPTO_TOKENS {
            if contains_word(line, t) {
                push(
                    "WEAK-CRYPTO",
                    format!("'{t}' names a broken or legacy algorithm; use SHA-256 or better and an AEAD cipher such as AES-GCM"),
                );
                break;
            }
        }
    }

    // One finding per rule per line keeps reports readable.
    let mut seen: HashSet<(usize, &'static str)> = HashSet::new();
    out.retain(|f| seen.insert((f.line, f.code)));
    out
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn counts(findings: &[Finding]) -> (usize, usize, usize, usize) {
    let mut c = [0usize; 4];
    for f in findings {
        match rule_of(f.code).severity {
            Severity::Critical => c[0] += 1,
            Severity::High => c[1] += 1,
            Severity::Medium => c[2] += 1,
            Severity::Low => c[3] += 1,
        }
    }
    (c[0], c[1], c[2], c[3])
}

fn render_text(lang: Lang, findings: &[Finding], include_context: bool) -> String {
    let (crit, high, med, low) = counts(findings);
    let mut out = format!(
        "C/C++ vulnerability scan ({}) · {} findings · {crit} critical · {high} high · {med} medium · {low} low",
        lang.label(),
        findings.len()
    );
    if findings.is_empty() {
        out.push_str(
            "\n\nNo matching patterns found. A clean report is not a proof of safety: this scan has no control flow, data flow or type information.",
        );
        return out;
    }
    for f in findings {
        let r = rule_of(f.code);
        let _ = write!(
            out,
            "\n\nL{} [{}] {} (CWE-{}): {}",
            f.line,
            r.severity.label(),
            f.code,
            r.cwe,
            f.message
        );
        if include_context && !f.source.is_empty() {
            let _ = write!(out, "\n  {}", f.source);
        }
    }
    out
}

fn render_json(
    lang: Lang,
    profile: &str,
    min_severity: &str,
    findings: &[Finding],
    include_context: bool,
) -> String {
    let (crit, high, med, low) = counts(findings);
    let items: Vec<_> = findings
        .iter()
        .map(|f| {
            let r = rule_of(f.code);
            json!({
                "line": f.line,
                "code": f.code,
                "cwe": format!("CWE-{}", r.cwe),
                "severity": r.severity.label(),
                "message": f.message,
                "source": if include_context { f.source.clone() } else { String::new() },
            })
        })
        .collect();
    serde_json::to_string_pretty(&json!({
        "language": lang.label(),
        "profile": profile,
        "min_severity": min_severity,
        "summary": {
            "findings": findings.len(),
            "critical": crit,
            "high": high,
            "medium": med,
            "low": low,
        },
        "findings": items,
    }))
    .unwrap()
}

fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.trim() != s {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn render_csv(findings: &[Finding], include_context: bool) -> String {
    let mut out = String::from("line,severity,code,cwe,message,source");
    for f in findings {
        let r = rule_of(f.code);
        let source = if include_context { f.source.as_str() } else { "" };
        let _ = write!(
            out,
            "\n{},{},{},CWE-{},{},{}",
            f.line,
            r.severity.label(),
            f.code,
            r.cwe,
            csv_field(&f.message),
            csv_field(source)
        );
    }
    out
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Scan a C/C++ snippet for vulnerability patterns.
///
/// * `code` — the source text (max [`MAX_INPUT_BYTES`] bytes). Never compiled or run.
/// * `language` — `auto` | `c` | `cpp`; `auto` looks for C++ markers.
/// * `profile` — `all` | `memory` | `injection` | `crypto` | `banned` rule family.
/// * `min_severity` — `all`/`low` | `medium` | `high` | `critical`.
/// * `ignore` — comma/space separated rule codes to suppress.
/// * `format` — `text` | `json` | `csv`.
/// * `include_context` — echo the matching source line with each finding.
pub fn scan_source(
    code: &str,
    language: &str,
    profile: &str,
    min_severity: &str,
    ignore: &str,
    format: &str,
    include_context: bool,
) -> Result<String, String> {
    if code.trim().is_empty() {
        return Err("paste C or C++ source code to scan".into());
    }
    if code.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "source is too large ({} bytes); the limit is {MAX_INPUT_BYTES} bytes",
            code.len()
        ));
    }
    let requested = parse_language(language)?;
    let profile = parse_profile(profile)?;
    let min = Severity::parse_min(min_severity)?;
    let format = Format::parse(format)?;
    let ignored = parse_ignore(ignore);

    let masked = mask(code);
    let lang = requested.unwrap_or_else(|| detect_language(&masked.code));
    let raw: Vec<&str> = code.lines().collect();
    let masked_lines: Vec<&str> = masked.code.lines().collect();
    let text_lines: Vec<&str> = masked.text.lines().collect();
    // `mask` preserves the line structure, but guard the indexing anyway.
    if masked_lines.len() != raw.len() || text_lines.len() != raw.len() {
        return Err("internal error: masking changed the line count".into());
    }

    let decls = collect_decls(&masked_lines);
    let suppressed: Vec<bool> = raw
        .iter()
        .map(|l| l.to_ascii_lowercase().contains(SUPPRESS_MARKER))
        .collect();

    let findings: Vec<Finding> = scan_lines(&raw, &masked_lines, &text_lines, &decls, lang)
        .into_iter()
        .filter(|f| {
            let r = rule_of(f.code);
            if profile != "all" && !r.families.contains(&profile) {
                return false;
            }
            if ignored.contains(r.code) {
                return false;
            }
            if r.severity < min {
                return false;
            }
            // `// vuln-scan: ignore` on the finding's line or the line above.
            let i = f.line - 1;
            if suppressed[i] || (i > 0 && suppressed[i - 1]) {
                return false;
            }
            true
        })
        .collect();

    Ok(match format {
        Format::Text => render_text(lang, &findings, include_context),
        Format::Json => render_json(lang, profile, min.label(), &findings, include_context),
        Format::Csv => render_csv(&findings, include_context),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const BAD: &str = r#"void log_user(char *s) {
  char buf[16];
  strcpy(buf, s);
  printf(s);
  gets(buf);
}"#;

    fn text(code: &str) -> String {
        scan_source(code, "auto", "all", "all", "", "text", true).unwrap()
    }

    #[test]
    fn happy_path_reports_codes_severities_and_cwes() {
        let out = text(BAD);
        assert!(
            out.starts_with(
                "C/C++ vulnerability scan (c) · 3 findings · 1 critical · 2 high · 0 medium · 0 low"
            ),
            "{out}"
        );
        assert!(out.contains("L3 [high] BANNED-COPY (CWE-120)"), "{out}");
        assert!(out.contains("L4 [high] FORMAT-STRING (CWE-134)"), "{out}");
        assert!(out.contains("L5 [critical] GETS (CWE-242)"), "{out}");
        // include_context echoes the offending source line.
        assert!(out.contains("\n  strcpy(buf, s);"), "{out}");
        let compact = scan_source(BAD, "auto", "all", "all", "", "text", false).unwrap();
        assert!(!compact.contains("\n  strcpy(buf, s);"), "{compact}");
    }

    #[test]
    fn rejects_empty_oversize_and_unknown_options() {
        assert!(scan_source("", "auto", "all", "all", "", "text", true)
            .unwrap_err()
            .contains("paste C or C++ source"));
        let big = "a".repeat(MAX_INPUT_BYTES + 1);
        assert!(scan_source(&big, "auto", "all", "all", "", "text", true)
            .unwrap_err()
            .contains("too large"));
        assert!(scan_source("int x;", "rust", "all", "all", "", "text", true)
            .unwrap_err()
            .contains("unknown language 'rust'"));
        assert!(scan_source("int x;", "auto", "nope", "all", "", "text", true)
            .unwrap_err()
            .contains("unknown profile 'nope'"));
        assert!(scan_source("int x;", "auto", "all", "urgent", "", "text", true)
            .unwrap_err()
            .contains("unknown min_severity 'urgent'"));
        assert!(scan_source("int x;", "auto", "all", "all", "", "sarif", true)
            .unwrap_err()
            .contains("unknown format 'sarif'"));
    }

    #[test]
    fn min_severity_filters_by_level() {
        let src = format!("{BAD}\nvoid g(char *d, char *s){{ strncpy(d, s, 8); }}");
        let all = scan_source(&src, "auto", "all", "all", "", "text", false).unwrap();
        assert!(all.contains("4 findings · 1 critical · 2 high · 0 medium · 1 low"), "{all}");
        assert!(all.contains("BOUNDED-COPY"), "{all}");
        let high = scan_source(&src, "auto", "all", "high", "", "text", false).unwrap();
        assert!(high.contains("3 findings"), "{high}");
        assert!(!high.contains("BOUNDED-COPY"), "{high}");
        let crit = scan_source(BAD, "auto", "all", "critical", "", "text", false).unwrap();
        assert!(crit.contains("1 findings · 1 critical"), "{crit}");
        assert!(crit.contains("GETS"), "{crit}");
    }

    #[test]
    fn profile_selects_a_rule_family() {
        let src = "int main(){ system(cmd); srand(1); char b[4]; strcpy(b, s); }";
        let injection = scan_source(src, "c", "injection", "all", "", "text", false).unwrap();
        assert!(injection.contains("COMMAND-EXEC"), "{injection}");
        assert!(!injection.contains("WEAK-RANDOM"), "{injection}");
        assert!(!injection.contains("BANNED-COPY"), "{injection}");
        let crypto = scan_source(src, "c", "crypto", "all", "", "text", false).unwrap();
        assert!(crypto.contains("WEAK-RANDOM"), "{crypto}");
        assert!(!crypto.contains("COMMAND-EXEC"), "{crypto}");
        let banned = scan_source(src, "c", "banned", "all", "", "text", false).unwrap();
        assert!(banned.contains("BANNED-COPY") && banned.contains("COMMAND-EXEC"), "{banned}");
    }

    #[test]
    fn ignore_list_suppresses_rule_families() {
        let out = scan_source(BAD, "auto", "all", "all", "BANNED-COPY, gets", "text", false).unwrap();
        assert!(!out.contains("BANNED-COPY"), "{out}");
        assert!(!out.contains("GETS"), "{out}");
        assert!(out.contains("FORMAT-STRING"), "{out}");
    }

    #[test]
    fn in_source_marker_suppresses_its_line_and_the_next() {
        let src = "void f(char *s){\n  strcpy(a, s); // vuln-scan: ignore\n  // vuln-scan: ignore\n  printf(s);\n  system(s);\n}";
        let out = text(src);
        assert!(!out.contains("BANNED-COPY"), "{out}");
        assert!(!out.contains("FORMAT-STRING"), "{out}");
        assert!(out.contains("COMMAND-EXEC"), "{out}");
    }

    #[test]
    fn comments_and_string_literals_are_masked() {
        let src = "int main(){\n  /* strcpy(a,b); gets(x); */\n  puts(\"call system() and strcpy() here\");\n  // system(cmd);\n  return 0;\n}";
        let out = text(src);
        assert!(out.contains("0 findings"), "{out}");
    }

    #[test]
    fn scanf_width_and_format_string_use_the_literal() {
        let unbounded = text("int main(){ char b[8]; scanf(\"%s\", b); }");
        assert!(unbounded.contains("SCANF-UNBOUNDED"), "{unbounded}");
        let bounded = text("int main(){ char b[8]; scanf(\"%7s\", b); }");
        assert!(!bounded.contains("SCANF-UNBOUNDED"), "{bounded}");
        let literal_fmt = text("int main(){ printf(\"hello %s\\n\", name); }");
        assert!(!literal_fmt.contains("FORMAT-STRING"), "{literal_fmt}");
        let variable_fmt = text("int main(){ fprintf(stderr, msg); }");
        assert!(variable_fmt.contains("FORMAT-STRING"), "{variable_fmt}");
    }

    #[test]
    fn bounds_rules_separate_provable_overruns_from_off_by_one() {
        let overrun = text("void f(void){\n  char buf[8];\n  buf[12] = 0;\n  memcpy(buf, src, 32);\n}");
        assert!(overrun.contains("BUFFER-OVERRUN"), "{overrun}");
        assert!(overrun.contains("index 12 is past the end of 'buf'"), "{overrun}");
        assert!(overrun.contains("writes up to 32 bytes into 'buf'"), "{overrun}");
        let off = text("void f(void){\n  char buf[8];\n  buf[8] = 0;\n}");
        assert!(off.contains("OFF-BY-ONE"), "{off}");
        assert!(!off.contains("BUFFER-OVERRUN"), "{off}");
        let ok = text("void f(void){\n  char buf[8];\n  buf[7] = 0;\n  memset(buf, 0, 8);\n}");
        assert!(ok.contains("0 findings"), "{ok}");
    }

    #[test]
    fn language_gates_the_cpp_stream_rule() {
        let src = "#include <iostream>\nint main(){ char name[16]; std::cin >> name; }";
        let cpp = scan_source(src, "auto", "all", "all", "", "text", false).unwrap();
        assert!(cpp.contains("(cpp)"), "{cpp}");
        assert!(cpp.contains("CPP-STREAM"), "{cpp}");
        let c = scan_source(src, "c", "all", "all", "", "text", false).unwrap();
        assert!(c.contains("(c)"), "{c}");
        assert!(!c.contains("CPP-STREAM"), "{c}");
    }

    #[test]
    fn memory_lifecycle_rules_fire() {
        let src = "void f(void){\n  char *p = malloc(n * 4);\n  memcpy(p, s, n);\n  free(p);\n  p[0] = 'x';\n}";
        let out = text(src);
        assert!(out.contains("UNCHECKED-ALLOC"), "{out}");
        assert!(out.contains("INT-OVERFLOW"), "{out}");
        assert!(out.contains("L5 [high] USE-AFTER-FREE (CWE-416)"), "{out}");
        let checked = text("void f(void){\n  char *p = malloc(16);\n  if (!p) return;\n  free(p);\n}");
        assert!(!checked.contains("UNCHECKED-ALLOC"), "{checked}");
        assert!(!checked.contains("MEM-LEAK"), "{checked}");
        let leak = text("void f(void){\n  char *p = malloc(16);\n  if (!p) return;\n  use(p);\n}");
        assert!(leak.contains("MEM-LEAK"), "{leak}");
    }

    #[test]
    fn json_output_carries_summary_and_cwes() {
        let out = scan_source(BAD, "auto", "all", "all", "", "json", true).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["language"], "c");
        assert_eq!(v["profile"], "all");
        assert_eq!(v["min_severity"], "low");
        assert_eq!(v["summary"]["findings"], 3);
        assert_eq!(v["summary"]["critical"], 1);
        assert_eq!(v["summary"]["high"], 2);
        assert_eq!(v["findings"][0]["line"], 3);
        assert_eq!(v["findings"][0]["code"], "BANNED-COPY");
        assert_eq!(v["findings"][0]["cwe"], "CWE-120");
        assert_eq!(v["findings"][0]["severity"], "high");
        assert_eq!(v["findings"][0]["source"], "strcpy(buf, s);");
        let no_ctx = scan_source(BAD, "auto", "all", "all", "", "json", false).unwrap();
        let v2: serde_json::Value = serde_json::from_str(&no_ctx).unwrap();
        assert_eq!(v2["findings"][0]["source"], "");
    }

    #[test]
    fn csv_output_is_rfc4180_quoted() {
        let out = scan_source(BAD, "auto", "all", "high", "", "csv", true).unwrap();
        let mut lines = out.lines();
        assert_eq!(lines.next().unwrap(), "line,severity,code,cwe,message,source");
        let first = lines.next().unwrap();
        assert!(first.starts_with("3,high,BANNED-COPY,CWE-120,"), "{first}");
        // The source line contains a comma, so it is quoted.
        assert!(first.ends_with("\"strcpy(buf, s);\""), "{first}");
        assert_eq!(out.lines().count(), 4);

        // A message containing a comma is quoted; an empty source column stays empty.
        let overrun = scan_source(
            "void f(void){\n  char buf[8];\n  buf[12] = 0;\n}",
            "c",
            "all",
            "all",
            "",
            "csv",
            false,
        )
        .unwrap();
        let row = overrun.lines().nth(1).unwrap();
        assert!(
            row.starts_with("3,critical,BUFFER-OVERRUN,CWE-787,\"index 12 is past the end of 'buf',"),
            "{row}"
        );
        assert!(row.ends_with("\","), "{row}");

        // Double quotes inside an echoed source line are doubled per RFC 4180.
        let quoted = scan_source(
            "void f(char *b){ puts(\"hi, there\"); strcpy(b, s); }",
            "c",
            "all",
            "all",
            "",
            "csv",
            true,
        )
        .unwrap();
        let row = quoted.lines().nth(1).unwrap();
        assert!(row.contains("\"\"hi, there\"\""), "{row}");
    }

    #[test]
    fn every_rule_is_reachable() {
        let corpus: &[(&str, &str)] = &[
            ("GETS", "void f(void){ char b[8]; gets(b); }"),
            ("BANNED-COPY", "void f(char*s){ char b[8]; strcpy(b, s); }"),
            ("BOUNDED-COPY", "void f(char*s){ char b[8]; strncpy(b, s, 7); }"),
            ("SCANF-UNBOUNDED", "void f(void){ char b[8]; scanf(\"%s\", b); }"),
            ("BUFFER-OVERRUN", "void f(void){ char b[8]; b[99] = 0; }"),
            ("OFF-BY-ONE", "void f(char*s){ for (i=0; i <= strlen(s); i++) t(i); }"),
            ("INT-OVERFLOW", "void f(int n){ p = malloc(n * 8); }"),
            ("SIGN-CONVERSION", "void f(char*s){ int len; memcpy(d, s, len); }"),
            ("UNBOUNDED-ALLOC", "void f(int n){ char *p = alloca(n); }"),
            ("UNCHECKED-ALLOC", "void f(void){ char *p = malloc(16); p[0] = 1; }"),
            ("MEM-LEAK", "void f(void){ char *p = malloc(16); if (!p) return; use(p); }"),
            ("USE-AFTER-FREE", "void f(char *p){ free(p);\n  p[0] = 1; }"),
            ("SIZEOF-POINTER", "void f(char *p){ memcpy(d, p, sizeof(p)); }"),
            ("CPP-STREAM", "#include <iostream>\nint main(){ char n[8]; std::cin >> n; }"),
            ("FORMAT-STRING", "void f(char *s){ printf(s); }"),
            ("COMMAND-EXEC", "void f(char *s){ system(s); }"),
            ("TEMP-FILE", "void f(void){ char *n = tmpnam(NULL); use(n); }"),
            ("TOCTOU", "void f(char*p){ if (access(p, W_OK) == 0) {\n  fopen(p, \"w\");\n } }"),
            ("WEAK-RANDOM", "void f(void){ int k = rand(); use(k); }"),
            ("WEAK-CRYPTO", "void f(void){ MD5(data, len, out); }"),
        ];
        for r in RULES {
            let sample = corpus
                .iter()
                .find(|(code, _)| *code == r.code)
                .unwrap_or_else(|| panic!("no corpus sample for rule {}", r.code));
            let out = text(sample.1);
            assert!(out.contains(r.code), "rule {} did not fire:\n{out}", r.code);
        }
    }
}

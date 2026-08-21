//! docstring-stub-generator core — turn a pasted function signature into a
//! ready-to-fill documentation stub (summary, parameters, return, raises).
//!
//! There is **no language grammar** here: parser-generator grammars are C
//! libraries that neither build nor instantiate in the wasm sandbox (the same
//! constraint `code-comment-extractor` hit). Instead a signature is recognised
//! structurally, with a string-literal-aware, depth-tracking scanner:
//!
//! - A signature is a *name token* followed by a balanced parenthesis group.
//!   Backward scanning skips generic groups (`<T>`, `[T any]`) and receiver
//!   groups (`func (r *T) Name(...)`) so the right group is chosen.
//! - Parameters are split on top-level commas, tracking `()`, `[]`, `{}` and —
//!   for the generic-using languages — `<>`, while skipping string and char
//!   literals (Rust lifetimes such as `&'a str` are NOT mistaken for chars).
//! - Each language has its own parameter shape: `name: T = d` (Python, TS,
//!   Rust), `T $name` (PHP), `T name` (Java, C#), `a, b string` (Go, with the
//!   type back-filled across grouped names), `...rest` / `*args` / `**kwargs`.
//!
//! Types come from annotations, or are inferred from default-value literals,
//! or fall back to the language's unknown-type placeholder. Nothing is
//! invented: descriptions are placeholders for the author to fill in.

use serde_json::{json, Value};

/// Largest input accepted, in bytes.
pub const MAX_INPUT_BYTES: usize = 200_000;
/// Largest number of signatures handled in one run.
pub const MAX_FUNCTIONS: usize = 200;
/// Largest number of parameters handled per signature.
pub const MAX_PARAMS: usize = 100;
/// Largest number of declared exception names accepted.
pub const MAX_RAISES: usize = 20;

/// Every language whose signature shape and documentation convention is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Python,
    JavaScript,
    TypeScript,
    Php,
    Java,
    CSharp,
    Go,
    Rust,
    Ruby,
}

impl Lang {
    fn parse(s: &str) -> Option<Lang> {
        Some(match s {
            "python" => Lang::Python,
            "javascript" => Lang::JavaScript,
            "typescript" => Lang::TypeScript,
            "php" => Lang::Php,
            "java" => Lang::Java,
            "csharp" => Lang::CSharp,
            "go" => Lang::Go,
            "rust" => Lang::Rust,
            "ruby" => Lang::Ruby,
            _ => return None,
        })
    }

    fn name(self) -> &'static str {
        match self {
            Lang::Python => "python",
            Lang::JavaScript => "javascript",
            Lang::TypeScript => "typescript",
            Lang::Php => "php",
            Lang::Java => "java",
            Lang::CSharp => "csharp",
            Lang::Go => "go",
            Lang::Rust => "rust",
            Lang::Ruby => "ruby",
        }
    }

    /// Languages whose types use `<...>` generics, so `<` `>` must be a depth level.
    fn uses_angles(self) -> bool {
        matches!(
            self,
            Lang::Java | Lang::CSharp | Lang::Rust | Lang::TypeScript
        )
    }

    /// Languages where a backtick opens a string (JS template, Go raw string).
    fn backtick_string(self) -> bool {
        matches!(self, Lang::JavaScript | Lang::TypeScript | Lang::Go)
    }

    /// Languages where `'` is a short CHAR literal, not a string — so `&'a str`
    /// (a Rust lifetime) is not read as an unterminated string.
    fn short_char_quote(self) -> bool {
        matches!(self, Lang::Rust | Lang::Go | Lang::Java | Lang::CSharp)
    }

    /// What to write where a type is required but unknown.
    fn unknown_type(self) -> &'static str {
        match self {
            Lang::Python => "_type_",
            Lang::JavaScript | Lang::TypeScript => "*",
            Lang::Php => "mixed",
            Lang::Java => "Object",
            Lang::CSharp => "object",
            Lang::Go => "any",
            Lang::Rust => "_",
            Lang::Ruby => "Object",
        }
    }

    /// Type names used when guessing from a default-value literal:
    /// (string, bool, int, float, list, map).
    fn literal_types(self) -> [&'static str; 6] {
        match self {
            Lang::Python => ["str", "bool", "int", "float", "list", "dict"],
            Lang::JavaScript | Lang::TypeScript => {
                ["string", "boolean", "number", "number", "Array", "Object"]
            }
            Lang::Php => ["string", "bool", "int", "float", "array", "array"],
            Lang::Java => ["String", "boolean", "int", "double", "List", "Map"],
            Lang::CSharp => ["string", "bool", "int", "double", "object[]", "object"],
            Lang::Go => ["string", "bool", "int", "float64", "[]any", "map[string]any"],
            Lang::Rust => ["&str", "bool", "i64", "f64", "Vec", "HashMap"],
            Lang::Ruby => ["String", "Boolean", "Integer", "Float", "Array", "Hash"],
        }
    }

    /// True when the doc block is written INSIDE the function (Python docstring).
    fn doc_inside(self) -> bool {
        self == Lang::Python
    }
}

/// How types are sourced for the generated stub.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TypeMode {
    /// Annotation, else a guess from the default literal, else a placeholder.
    Guess,
    /// Only explicitly declared annotations; nothing is written otherwise.
    Annotated,
    /// No type slots at all (the `-notypes` shape).
    None,
}

// ---------------------------------------------------------------------------
// Low-level scanning helpers (string-literal aware)
// ---------------------------------------------------------------------------

/// Byte index just past the literal opened at `start`, or `None` when that
/// quote character is not a literal opener in this language.
fn skip_quote(s: &str, start: usize, lang: Lang) -> Option<usize> {
    let b = s.as_bytes();
    let q = b[start];
    if q == b'`' && !lang.backtick_string() {
        return None;
    }
    if q == b'\'' && lang.short_char_quote() {
        // A char/rune literal is exactly `'X'` or an escape `'\n'`. Anything
        // else beginning with `'` is a Rust lifetime (`&'a [&'a str]`) and must
        // NOT be skipped as a literal — doing so swallowed the rest of the
        // parameter list and lost every bracket depth inside it.
        return match s[start + 1..].chars().next() {
            Some('\\') => {
                let lim = (start + 12).min(b.len());
                let mut i = start + 2;
                while i < lim {
                    if b[i] == b'\'' {
                        return Some(i + 1);
                    }
                    i += 1;
                }
                None
            }
            Some(c) => {
                let next = start + 1 + c.len_utf8();
                if b.get(next).copied() == Some(b'\'') {
                    Some(next + 1)
                } else {
                    None
                }
            }
            None => None,
        };
    }
    let mut i = start + 1;
    while i < b.len() {
        if b[i] == b'\\' {
            i += 2;
            continue;
        }
        if b[i] == q {
            return Some(i + 1);
        }
        i += 1;
    }
    Some(b.len())
}

/// Byte spans of every top-level `( … )` group in `s`.
fn paren_groups(s: &str, lang: Lang) -> Vec<(usize, usize)> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    let mut i = 0usize;
    while i < b.len() {
        match b[i] {
            b'"' | b'\'' | b'`' => {
                if let Some(n) = skip_quote(s, i, lang) {
                    i = n;
                    continue;
                }
                i += 1;
            }
            b'(' => {
                if depth == 0 {
                    start = i;
                }
                depth += 1;
                i += 1;
            }
            b'[' | b'{' => {
                depth += 1;
                i += 1;
            }
            b')' => {
                depth -= 1;
                if depth == 0 {
                    out.push((start, i));
                }
                if depth < 0 {
                    depth = 0;
                }
                i += 1;
            }
            b']' | b'}' => {
                depth -= 1;
                if depth < 0 {
                    depth = 0;
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    out
}

/// Net `(` minus `)` across a line, ignoring literals.
fn paren_delta(s: &str, lang: Lang) -> i32 {
    let b = s.as_bytes();
    let mut d = 0i32;
    let mut i = 0usize;
    while i < b.len() {
        match b[i] {
            b'"' | b'\'' | b'`' => {
                if let Some(n) = skip_quote(s, i, lang) {
                    i = n;
                    continue;
                }
                i += 1;
            }
            b'(' => {
                d += 1;
                i += 1;
            }
            b')' => {
                d -= 1;
                i += 1;
            }
            _ => i += 1,
        }
    }
    d
}

fn is_ident_byte(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'$'
}

/// The identifier immediately before byte `pos`, skipping whitespace and any
/// generic/receiver group (`<T>`, `[T any]`). Returns `(ident, ident_start)`.
fn ident_before(s: &str, pos: usize) -> (String, usize) {
    let b = s.as_bytes();
    let mut i = pos;
    loop {
        while i > 0 && (b[i - 1] as char).is_whitespace() {
            i -= 1;
        }
        if i > 0 && (b[i - 1] == b'>' || b[i - 1] == b']') {
            let close = b[i - 1];
            let open = if close == b'>' { b'<' } else { b'[' };
            let mut d = 0i32;
            let mut j = i - 1;
            loop {
                if b[j] == close {
                    d += 1;
                } else if b[j] == open {
                    d -= 1;
                    if d == 0 {
                        break;
                    }
                }
                if j == 0 {
                    return (String::new(), i);
                }
                j -= 1;
            }
            i = j;
            continue;
        }
        break;
    }
    let end = i;
    while i > 0 && is_ident_byte(b[i - 1]) {
        i -= 1;
    }
    (s[i..end].to_string(), i)
}

/// Split on top-level separators, tracking brackets, generics and literals.
fn split_top(s: &str, sep: u8, lang: Lang) -> Vec<String> {
    let b = s.as_bytes();
    let angles = lang.uses_angles();
    let mut out = Vec::new();
    let (mut depth, mut ang, mut last, mut i) = (0i32, 0i32, 0usize, 0usize);
    while i < b.len() {
        let c = b[i];
        match c {
            b'"' | b'\'' | b'`' => {
                if let Some(n) = skip_quote(s, i, lang) {
                    i = n;
                    continue;
                }
                i += 1;
            }
            b'(' | b'[' | b'{' => {
                depth += 1;
                i += 1;
            }
            b')' | b']' | b'}' => {
                depth -= 1;
                if depth < 0 {
                    depth = 0;
                }
                i += 1;
            }
            b'<' if angles && depth == 0 => {
                let prev = s[..i].trim_end().as_bytes().last().copied();
                if matches!(prev, Some(p) if is_ident_byte(p) || p == b'>' || p == b',' || p == b'?' || p == b']')
                {
                    ang += 1;
                }
                i += 1;
            }
            b'>' if angles && depth == 0 => {
                let prev = if i > 0 { Some(b[i - 1]) } else { None };
                if ang > 0 && !matches!(prev, Some(b'-') | Some(b'=')) {
                    ang -= 1;
                }
                i += 1;
            }
            _ if c == sep && depth == 0 && ang == 0 => {
                out.push(s[last..i].trim().to_string());
                last = i + 1;
                i += 1;
            }
            _ => i += 1,
        }
    }
    out.push(s[last..].trim().to_string());
    out.into_iter().filter(|p| !p.is_empty()).collect()
}

/// Whitespace-separated tokens at top level (generics stay one token).
fn split_ws_top(s: &str, lang: Lang) -> Vec<String> {
    let b = s.as_bytes();
    let angles = lang.uses_angles();
    let mut out = Vec::new();
    let (mut depth, mut ang, mut last, mut i) = (0i32, 0i32, 0usize, 0usize);
    while i < b.len() {
        let c = b[i];
        match c {
            b'"' | b'\'' | b'`' => {
                if let Some(n) = skip_quote(s, i, lang) {
                    i = n;
                    continue;
                }
                i += 1;
            }
            b'(' | b'[' | b'{' => {
                depth += 1;
                i += 1;
            }
            b')' | b']' | b'}' => {
                depth -= 1;
                if depth < 0 {
                    depth = 0;
                }
                i += 1;
            }
            b'<' if angles && depth == 0 => {
                let prev = s[..i].trim_end().as_bytes().last().copied();
                if matches!(prev, Some(p) if is_ident_byte(p) || p == b'>' || p == b',' || p == b'?')
                {
                    ang += 1;
                }
                i += 1;
            }
            b'>' if angles && depth == 0 => {
                if ang > 0 && !matches!(if i > 0 { Some(b[i - 1]) } else { None }, Some(b'-') | Some(b'=')) {
                    ang -= 1;
                }
                i += 1;
            }
            _ if (c as char).is_ascii_whitespace() && depth == 0 && ang == 0 => {
                if last < i {
                    out.push(s[last..i].trim().to_string());
                }
                last = i + 1;
                i += 1;
            }
            _ => i += 1,
        }
    }
    if last < b.len() {
        out.push(s[last..].trim().to_string());
    }
    out.into_iter().filter(|p| !p.is_empty()).collect()
}

/// Split `lhs = default` at the first top-level assignment `=`.
fn split_default(s: &str, lang: Lang) -> (String, Option<String>) {
    let b = s.as_bytes();
    let angles = lang.uses_angles();
    let (mut depth, mut ang, mut i) = (0i32, 0i32, 0usize);
    while i < b.len() {
        let c = b[i];
        match c {
            b'"' | b'\'' | b'`' => {
                if let Some(n) = skip_quote(s, i, lang) {
                    i = n;
                    continue;
                }
                i += 1;
            }
            b'(' | b'[' | b'{' => {
                depth += 1;
                i += 1;
            }
            b')' | b']' | b'}' => {
                depth -= 1;
                if depth < 0 {
                    depth = 0;
                }
                i += 1;
            }
            b'<' if angles && depth == 0 => {
                ang += 1;
                i += 1;
            }
            b'>' if angles && depth == 0 => {
                if ang > 0 {
                    ang -= 1;
                }
                i += 1;
            }
            b'=' if depth == 0 && ang == 0 => {
                let prev = if i > 0 { b[i - 1] } else { b' ' };
                let next = b.get(i + 1).copied().unwrap_or(b' ');
                if next == b'=' || matches!(prev, b'=' | b'!' | b'<' | b'>' | b'+' | b'-' | b'*' | b'/') {
                    i += 2;
                    continue;
                }
                return (
                    s[..i].trim().to_string(),
                    Some(s[i + 1..].trim().to_string()).filter(|d| !d.is_empty()),
                );
            }
            _ => i += 1,
        }
    }
    (s.trim().to_string(), None)
}

/// Split `name : type` at the first top-level colon (not `::`).
fn split_colon(s: &str, lang: Lang) -> Option<(String, String)> {
    let b = s.as_bytes();
    let angles = lang.uses_angles();
    let (mut depth, mut ang, mut i) = (0i32, 0i32, 0usize);
    while i < b.len() {
        let c = b[i];
        match c {
            b'"' | b'\'' | b'`' => {
                if let Some(n) = skip_quote(s, i, lang) {
                    i = n;
                    continue;
                }
                i += 1;
            }
            b'(' | b'[' | b'{' => {
                depth += 1;
                i += 1;
            }
            b')' | b']' | b'}' => {
                depth -= 1;
                if depth < 0 {
                    depth = 0;
                }
                i += 1;
            }
            b'<' if angles && depth == 0 => {
                ang += 1;
                i += 1;
            }
            b'>' if angles && depth == 0 => {
                if ang > 0 {
                    ang -= 1;
                }
                i += 1;
            }
            b':' if depth == 0 && ang == 0 => {
                if b.get(i + 1).copied() == Some(b':') {
                    i += 2;
                    continue;
                }
                return Some((s[..i].trim().to_string(), s[i + 1..].trim().to_string()));
            }
            _ => i += 1,
        }
    }
    None
}

fn has_kw(t: &str, kw: &str) -> bool {
    let (b, k) = (t.as_bytes(), kw.as_bytes());
    if k.is_empty() || b.len() < k.len() {
        return false;
    }
    for i in 0..=(b.len() - k.len()) {
        if &b[i..i + k.len()] == k {
            let before = i == 0 || !is_ident_byte(b[i - 1]);
            let j = i + k.len();
            let after = j >= b.len() || !is_ident_byte(b[j]);
            if before && after {
                return true;
            }
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Signature parsing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct PInfo {
    name: String,
    ty: Option<String>,
    default: Option<String>,
    optional: bool,
    variadic: Option<&'static str>,
}

#[derive(Debug, Clone)]
struct FnInfo {
    name: String,
    params: Vec<PInfo>,
    ret: Option<String>,
    is_async: bool,
    throws: Vec<String>,
}

/// Words that can never be a documented function name.
const NON_NAMES: [&str; 27] = [
    "func", "fn", "def", "function", "if", "while", "for", "switch", "catch", "return", "new",
    "async", "await", "match", "in", "of", "do", "else", "try", "using", "lock", "foreach",
    "typeof", "delete", "throw", "case", "yield",
];

const MODIFIERS: [&str; 26] = [
    "public",
    "private",
    "protected",
    "internal",
    "static",
    "final",
    "abstract",
    "synchronized",
    "native",
    "virtual",
    "override",
    "sealed",
    "async",
    "partial",
    "unsafe",
    "extern",
    "new",
    "readonly",
    "default",
    "const",
    "volatile",
    "transient",
    "strictfp",
    "implicit",
    "explicit",
    "operator",
];

/// Detect the language of a pasted snippet from its shape.
fn detect_lang(src: &str) -> Lang {
    for line in src.lines() {
        let t = line.trim();
        if t.starts_with("def ") || t.starts_with("async def ") {
            if t.ends_with(':') || t.contains("->") || t.contains("self") {
                return Lang::Python;
            }
            return Lang::Ruby;
        }
    }
    if src.lines().any(|l| l.trim().starts_with("func ")) {
        return Lang::Go;
    }
    if has_kw(src, "fn") {
        return Lang::Rust;
    }
    if has_kw(src, "function") && src.contains('$') {
        return Lang::Php;
    }
    let jsish = has_kw(src, "function")
        || src.contains("=>")
        || has_kw(src, "const")
        || has_kw(src, "let")
        || has_kw(src, "export");
    if jsish {
        let tsish = src.contains("?:")
            || src.contains(": string")
            || src.contains(": number")
            || src.contains(": boolean")
            || src.contains("): ")
            || src.contains(": Promise");
        return if tsish {
            Lang::TypeScript
        } else {
            Lang::JavaScript
        };
    }
    if has_kw(src, "throws")
        || src.contains("String ")
        || has_kw(src, "boolean")
        || src.contains("@Override")
    {
        return Lang::Java;
    }
    if src.contains("Task<") || src.contains("string ") || has_kw(src, "bool") || has_kw(src, "void")
    {
        return Lang::CSharp;
    }
    Lang::JavaScript
}

/// Is this trimmed line the start of a documentable signature?
fn is_sig_start(t: &str, lang: Lang) -> bool {
    if t.is_empty() {
        return false;
    }
    match lang {
        Lang::Python => t.starts_with("def ") || t.starts_with("async def "),
        Lang::Ruby => t == "def" || t.starts_with("def "),
        Lang::Go => t.starts_with("func ") || t.starts_with("func("),
        Lang::Rust => has_kw(t, "fn"),
        Lang::Php => has_kw(t, "function"),
        Lang::Java | Lang::CSharp => decl_sig(t, lang),
        Lang::JavaScript | Lang::TypeScript => js_sig(t, lang),
    }
}

fn leading_word(t: &str) -> String {
    t.chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '$')
        .collect()
}

fn js_sig(t: &str, lang: Lang) -> bool {
    // `function` is itself a NON_NAMES entry (it can never be the documented
    // name), so this check has to come BEFORE the leading-word rejection.
    if has_kw(t, "function") {
        return true;
    }
    if NON_NAMES.contains(&leading_word(t).as_str()) && !t.starts_with("async") {
        return false;
    }
    let (lhs, rhs) = split_default(t, lang);
    if let Some(rhs) = rhs {
        let _ = &lhs;
        if rhs.contains("=>") || rhs.starts_with("function") || rhs.starts_with("async") {
            return true;
        }
    }
    if (t.starts_with('(') || t.starts_with("async ")) && t.contains("=>") {
        return true;
    }
    let groups = paren_groups(t, lang);
    if let Some(&(s, e)) = groups.first() {
        let (ident, start) = ident_before(t, s);
        if !ident.is_empty()
            && !NON_NAMES.contains(&ident.as_str())
            && !(start > 0 && t.as_bytes()[start - 1] == b'@')
        {
            let tail = t[e + 1..].trim();
            if tail.is_empty()
                || tail.starts_with('{')
                || tail.starts_with(':')
                || tail.starts_with("=>")
            {
                return true;
            }
        }
    }
    false
}

/// Java / C# style: `<modifiers> <ReturnType> name(...)`.
fn decl_sig(t: &str, lang: Lang) -> bool {
    if NON_NAMES.contains(&leading_word(t).as_str()) {
        return false;
    }
    for &(s, _) in paren_groups(t, lang).iter() {
        let (ident, start) = ident_before(t, s);
        if ident.is_empty() || NON_NAMES.contains(&ident.as_str()) {
            continue;
        }
        if start > 0 && t.as_bytes()[start - 1] == b'@' {
            continue;
        }
        let head = t[..start].trim();
        let tokens = split_ws_top(head, lang);
        if tokens.is_empty() {
            return false;
        }
        if NON_NAMES.contains(&tokens[0].as_str()) {
            return false;
        }
        return true;
    }
    false
}

fn is_decorator(t: &str, lang: Lang) -> bool {
    match lang {
        Lang::CSharp => t.starts_with('[') && t.ends_with(']'),
        Lang::Python | Lang::Java | Lang::JavaScript | Lang::TypeScript => t.starts_with('@'),
        _ => false,
    }
}

/// Guess a type from a default-value literal.
fn guess_from_default(default: &str, lang: Lang) -> Option<String> {
    let d = default.trim();
    let [s_str, s_bool, s_int, s_float, s_list, s_map] = lang.literal_types();
    if d.is_empty() {
        return None;
    }
    let first = d.as_bytes()[0];
    if first == b'"' || first == b'\'' || first == b'`' {
        return Some(s_str.to_string());
    }
    match d {
        "true" | "false" | "True" | "False" | "TRUE" | "FALSE" => return Some(s_bool.to_string()),
        "None" | "null" | "nil" | "undefined" | "NULL" | "default" => return None,
        _ => {}
    }
    if first == b'[' {
        return Some(s_list.to_string());
    }
    if first == b'{' {
        return Some(s_map.to_string());
    }
    let num = d.strip_prefix('-').unwrap_or(d);
    if !num.is_empty() && num.as_bytes()[0].is_ascii_digit() {
        let numeric = num
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == '_' || c == 'e' || c == 'E' || c == '-');
        if numeric {
            return Some(if num.contains('.') || num.contains('e') || num.contains('E') {
                s_float.to_string()
            } else {
                s_int.to_string()
            });
        }
    }
    None
}

fn strip_type_noise(ty: &str) -> String {
    ty.trim().trim_end_matches('{').trim().to_string()
}

fn parse_params(inner: &str, lang: Lang) -> Vec<PInfo> {
    let chunks = split_top(inner, b',', lang);
    let mut out: Vec<PInfo> = Vec::new();
    for (idx, chunk) in chunks.iter().enumerate() {
        let c = chunk.trim();
        if c.is_empty() || c == "*" || c == "/" {
            continue;
        }
        let p = match lang {
            Lang::Python => parse_param_python(c, idx),
            Lang::Ruby => parse_param_ruby(c),
            Lang::JavaScript => parse_param_js(c, lang, false),
            Lang::TypeScript => parse_param_js(c, lang, true),
            Lang::Php => parse_param_php(c, lang),
            Lang::Java | Lang::CSharp => parse_param_decl(c, lang),
            Lang::Go => parse_param_go(c, lang),
            Lang::Rust => parse_param_rust(c, idx, lang),
        };
        if let Some(p) = p {
            out.push(p);
        }
    }
    if lang == Lang::Go {
        // `func f(a, b string)` — grouped names share the next declared type.
        for i in (0..out.len()).rev() {
            if out[i].ty.is_none() {
                if let Some(next) = out.get(i + 1).and_then(|p| p.ty.clone()) {
                    out[i].ty = Some(next);
                }
            }
        }
    }
    out.truncate(MAX_PARAMS);
    out
}

fn parse_param_python(c: &str, idx: usize) -> Option<PInfo> {
    let (variadic, rest) = if let Some(r) = c.strip_prefix("**") {
        (Some("kwargs"), r)
    } else if let Some(r) = c.strip_prefix('*') {
        (Some("args"), r)
    } else {
        (None, c)
    };
    let (lhs, default) = split_default(rest.trim(), Lang::Python);
    let (name, ty) = match split_colon(&lhs, Lang::Python) {
        Some((n, t)) => (n, Some(strip_type_noise(&t))),
        None => (lhs.clone(), None),
    };
    let name = name.trim().to_string();
    if name.is_empty() || (idx == 0 && (name == "self" || name == "cls")) {
        return None;
    }
    Some(PInfo {
        optional: default.is_some(),
        name,
        ty: ty.filter(|t| !t.is_empty()),
        default,
        variadic,
    })
}

fn parse_param_ruby(c: &str) -> Option<PInfo> {
    let (variadic, rest) = if let Some(r) = c.strip_prefix("**") {
        (Some("kwargs"), r)
    } else if let Some(r) = c.strip_prefix('*') {
        (Some("args"), r)
    } else if let Some(r) = c.strip_prefix('&') {
        (Some("block"), r)
    } else {
        (None, c)
    };
    let rest = rest.trim();
    // Keyword argument: `name:` or `name: default`.
    if variadic.is_none() {
        if let Some((n, d)) = split_colon(rest, Lang::Ruby) {
            if !n.is_empty() && n.chars().all(|ch| ch.is_alphanumeric() || ch == '_') {
                let default = Some(d.trim().to_string()).filter(|s| !s.is_empty());
                return Some(PInfo {
                    ty: default.as_deref().and_then(|v| guess_from_default(v, Lang::Ruby)),
                    optional: default.is_some(),
                    name: n,
                    default,
                    variadic: None,
                });
            }
        }
    }
    let (lhs, default) = split_default(rest, Lang::Ruby);
    if lhs.is_empty() {
        return None;
    }
    Some(PInfo {
        ty: default.as_deref().and_then(|v| guess_from_default(v, Lang::Ruby)),
        optional: default.is_some(),
        name: lhs,
        default,
        variadic,
    })
}

fn parse_param_js(c: &str, lang: Lang, typed: bool) -> Option<PInfo> {
    let (lhs, default) = split_default(c, lang);
    let mut lhs = lhs.trim().to_string();
    for m in ["public ", "private ", "protected ", "readonly "] {
        if let Some(r) = lhs.strip_prefix(m) {
            lhs = r.trim().to_string();
        }
    }
    let (variadic, mut lhs) = match lhs.strip_prefix("...") {
        Some(r) => (Some("rest"), r.trim().to_string()),
        None => (None, lhs),
    };
    let mut ty = None;
    if typed {
        if let Some((n, t)) = split_colon(&lhs, lang) {
            lhs = n;
            ty = Some(strip_type_noise(&t)).filter(|t| !t.is_empty());
        }
    }
    let mut optional = default.is_some();
    if let Some(stripped) = lhs.strip_suffix('?') {
        optional = true;
        lhs = stripped.trim().to_string();
    }
    if lhs.is_empty() {
        return None;
    }
    Some(PInfo {
        name: lhs,
        ty,
        default,
        optional,
        variadic,
    })
}

fn parse_param_php(c: &str, lang: Lang) -> Option<PInfo> {
    let (lhs, default) = split_default(c, lang);
    let mut tokens = split_ws_top(&lhs, lang);
    tokens.retain(|t| !matches!(t.as_str(), "public" | "private" | "protected" | "readonly"));
    let pos = tokens.iter().position(|t| t.contains('$'))?;
    let raw = tokens[pos].clone();
    let variadic = if raw.contains("...") {
        Some("rest")
    } else {
        None
    };
    let name = raw
        .trim_start_matches('&')
        .trim_start_matches("...")
        .trim_start_matches('&')
        .trim_start_matches('$')
        .to_string();
    if name.is_empty() {
        return None;
    }
    let ty = if pos > 0 {
        Some(tokens[..pos].join(" "))
    } else {
        None
    };
    Some(PInfo {
        optional: default.is_some(),
        name,
        ty,
        default,
        variadic,
    })
}

fn parse_param_decl(c: &str, lang: Lang) -> Option<PInfo> {
    let (lhs, default) = split_default(c, lang);
    let mut tokens = split_ws_top(&lhs, lang);
    let mut variadic = None;
    tokens.retain(|t| {
        if t == "params" {
            variadic = Some("rest");
            return false;
        }
        !t.starts_with('@')
            && !matches!(
                t.as_str(),
                "final" | "in" | "out" | "ref" | "this" | "readonly" | "scoped" | "const"
            )
    });
    if tokens.is_empty() {
        return None;
    }
    let name = tokens.last().unwrap().clone();
    let mut ty = if tokens.len() >= 2 {
        Some(tokens[..tokens.len() - 1].join(" "))
    } else {
        None
    };
    if let Some(t) = ty.clone() {
        if let Some(stripped) = t.strip_suffix("...") {
            variadic = Some("rest");
            ty = Some(stripped.trim().to_string());
        }
    }
    Some(PInfo {
        optional: default.is_some(),
        name,
        ty,
        default,
        variadic,
    })
}

fn parse_param_go(c: &str, lang: Lang) -> Option<PInfo> {
    let tokens = split_ws_top(c, lang);
    if tokens.is_empty() {
        return None;
    }
    if tokens.len() == 1 {
        return Some(PInfo {
            name: tokens[0].clone(),
            ty: None,
            default: None,
            optional: false,
            variadic: None,
        });
    }
    let ty = tokens[1..].join(" ");
    let (variadic, ty) = match ty.strip_prefix("...") {
        Some(r) => (Some("rest"), r.to_string()),
        None => (None, ty),
    };
    Some(PInfo {
        name: tokens[0].clone(),
        ty: Some(ty),
        default: None,
        optional: false,
        variadic,
    })
}

fn parse_param_rust(c: &str, idx: usize, lang: Lang) -> Option<PInfo> {
    let t = c.trim();
    if idx == 0 && matches!(t, "self" | "&self" | "&mut self" | "mut self") {
        return None;
    }
    let (name, ty) = match split_colon(t, lang) {
        Some((n, ty)) => (n, Some(strip_type_noise(&ty))),
        None => (t.to_string(), None),
    };
    let name = name.trim().trim_start_matches("mut ").trim().to_string();
    if name.is_empty() {
        return None;
    }
    Some(PInfo {
        name,
        ty: ty.filter(|t| !t.is_empty()),
        default: None,
        optional: false,
        variadic: None,
    })
}

/// Parse one joined signature into structured facts.
fn parse_signature(joined: &str, lang: Lang) -> Option<FnInfo> {
    let s = joined.trim();
    let groups = paren_groups(s, lang);
    let is_async = has_kw(s, "async");

    if groups.is_empty() {
        // Ruby `def greet` — a parameterless method with no parentheses.
        if lang == Lang::Ruby {
            let name = s.strip_prefix("def ")?.trim();
            let name = name.split_whitespace().next()?.trim_end_matches('=');
            if name.is_empty() {
                return None;
            }
            return Some(FnInfo {
                name: name.to_string(),
                params: Vec::new(),
                ret: None,
                is_async,
                throws: Vec::new(),
            });
        }
        return None;
    }

    let mut chosen: Option<(usize, usize, String, usize)> = None;
    for &(gs, ge) in &groups {
        let (ident, start) = ident_before(s, gs);
        if ident.is_empty() || NON_NAMES.contains(&ident.as_str()) {
            continue;
        }
        if start > 0 && s.as_bytes()[start - 1] == b'@' {
            continue;
        }
        chosen = Some((gs, ge, ident, start));
        break;
    }
    let (gs, ge, name, name_start) = match chosen {
        Some(v) => v,
        None => {
            let (gs, ge) = groups[0];
            let head = &s[..gs];
            let (lhs, _) = split_default(head, lang);
            let n = split_ws_top(&lhs, lang)
                .into_iter()
                .filter(|t| !matches!(t.as_str(), "export" | "default" | "const" | "let" | "var"))
                .next_back()
                .unwrap_or_default();
            let n = n.trim_end_matches(':').to_string();
            (gs, ge, n, gs)
        }
    };

    let params = parse_params(&s[gs + 1..ge], lang);
    let tail = s[ge + 1..].trim();
    let mut throws: Vec<String> = Vec::new();

    let ret = match lang {
        Lang::Python => tail
            .strip_prefix("->")
            .map(|r| r.trim().trim_end_matches(':').trim().to_string()),
        Lang::Rust => {
            let t = tail.split(" where ").next().unwrap_or(tail);
            t.trim()
                .strip_prefix("->")
                .map(|r| r.trim().trim_end_matches('{').trim().to_string())
        }
        Lang::TypeScript | Lang::Php => tail.strip_prefix(':').map(|r| {
            r.trim()
                .trim_end_matches('{')
                .trim_end_matches(';')
                .trim()
                .to_string()
        }),
        Lang::Go => {
            let t = tail.trim_end_matches('{').trim();
            if t.is_empty() {
                None
            } else if t.starts_with('(') {
                Some(t.trim_start_matches('(').trim_end_matches(')').trim().to_string())
            } else {
                Some(t.to_string())
            }
        }
        Lang::Java | Lang::CSharp => {
            if let Some(idx) = tail.find("throws ") {
                throws = split_top(&tail[idx + 7..], b',', lang)
                    .into_iter()
                    .map(|t| t.trim_end_matches('{').trim().to_string())
                    .filter(|t| !t.is_empty())
                    .collect();
            }
            let head = &s[..name_start];
            let mut tokens = split_ws_top(head, lang);
            tokens.retain(|t| !t.starts_with('@') && !MODIFIERS.contains(&t.as_str()));
            tokens
                .into_iter()
                .next_back()
                .filter(|t| !t.starts_with('<') && !t.is_empty())
        }
        Lang::JavaScript | Lang::Ruby => None,
    };

    let name = name.trim().to_string();
    if name.is_empty() && params.is_empty() {
        return None;
    }
    Some(FnInfo {
        name,
        params,
        ret: ret.filter(|r| !r.is_empty()),
        is_async,
        throws,
    })
}

// ---------------------------------------------------------------------------
// Segmentation: signatures vs passthrough lines
// ---------------------------------------------------------------------------

enum Seg {
    Pass(String),
    Sig {
        raw: Vec<String>,
        sig_start: usize,
        info: FnInfo,
    },
}

fn segment(src: &str, lang: Lang) -> Result<Vec<Seg>, String> {
    let lines: Vec<&str> = src.lines().collect();
    let mut segs: Vec<Seg> = Vec::new();
    let mut pending: Vec<String> = Vec::new();
    let mut i = 0usize;
    let mut found = 0usize;

    while i < lines.len() {
        let line = lines[i];
        let t = line.trim();
        if is_decorator(t, lang) && !is_sig_start(t, lang) {
            pending.push(line.to_string());
            i += 1;
            continue;
        }
        if is_sig_start(t, lang) {
            let mut raw = vec![line.to_string()];
            let mut depth = paren_delta(line, lang);
            let mut j = i;
            while depth > 0 && j + 1 < lines.len() {
                j += 1;
                raw.push(lines[j].to_string());
                depth += paren_delta(lines[j], lang);
            }
            let joined = raw
                .iter()
                .map(|l| l.trim())
                .collect::<Vec<_>>()
                .join(" ");
            if let Some(info) = parse_signature(&joined, lang) {
                found += 1;
                if found > MAX_FUNCTIONS {
                    return Err(format!(
                        "too many function signatures in one run (cap is {MAX_FUNCTIONS}) — split the input"
                    ));
                }
                let mut all: Vec<String> = pending.drain(..).collect();
                let sig_start = all.len();
                all.extend(raw);
                segs.push(Seg::Sig {
                    raw: all,
                    sig_start,
                    info,
                });
                i = j + 1;
                continue;
            }
        }
        for d in pending.drain(..) {
            segs.push(Seg::Pass(d));
        }
        segs.push(Seg::Pass(line.to_string()));
        i += 1;
    }
    for d in pending.drain(..) {
        segs.push(Seg::Pass(d));
    }
    if found == 0 {
        return Err(format!(
            "no function signature found for language '{}' — paste a signature such as \
             `def fetch(url: str, timeout: int = 30) -> dict:` (python), \
             `function fetch(url, timeout = 30) {{` (javascript) or \
             `public String fetch(String url)` (java), or set the language explicitly",
            lang.name()
        ));
    }
    Ok(segs)
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct RParam {
    name: String,
    ty: Option<String>,
    default: Option<String>,
    optional: bool,
    variadic: Option<&'static str>,
}

struct Doc {
    name: String,
    params: Vec<RParam>,
    ret: Option<String>,
    show_return: bool,
    raises: Vec<String>,
    ph: String,
    ext: bool,
    examples: bool,
    align: bool,
    ind: String,
    quote: &'static str,
}

fn void_like(t: &str) -> bool {
    matches!(
        t.trim(),
        "void" | "None" | "()" | "Unit" | "never" | "undefined" | "Void" | "NoReturn"
    )
}

fn resolve(info: &FnInfo, lang: Lang, mode: TypeMode) -> (Vec<RParam>, Option<String>, bool) {
    let params = info
        .params
        .iter()
        .map(|p| {
            let ty = match mode {
                TypeMode::None => None,
                TypeMode::Annotated => p.ty.clone(),
                TypeMode::Guess => p
                    .ty
                    .clone()
                    .or_else(|| p.default.as_deref().and_then(|d| guess_from_default(d, lang)))
                    .or_else(|| Some(lang.unknown_type().to_string())),
            };
            RParam {
                name: p.name.clone(),
                ty,
                default: p.default.clone(),
                optional: p.optional,
                variadic: p.variadic,
            }
        })
        .collect();

    let declared = info.ret.clone();
    let show_return = !declared.as_deref().map(void_like).unwrap_or(false);
    let ret = match mode {
        TypeMode::None => None,
        TypeMode::Annotated => declared.clone(),
        TypeMode::Guess => declared.clone().or_else(|| {
            Some(match (lang, info.is_async) {
                (Lang::JavaScript, true) | (Lang::TypeScript, true) => "Promise<*>".to_string(),
                _ => lang.unknown_type().to_string(),
            })
        }),
    };
    (params, ret.filter(|_| show_return), show_return)
}

fn disp_name(p: &RParam, lang: Lang) -> String {
    match (lang, p.variadic) {
        (Lang::Python, Some("args")) | (Lang::Ruby, Some("args")) => format!("*{}", p.name),
        (Lang::Python, Some("kwargs")) | (Lang::Ruby, Some("kwargs")) => format!("**{}", p.name),
        (Lang::Ruby, Some("block")) => format!("&{}", p.name),
        (Lang::Php, _) => format!("${}", p.name),
        _ => p.name.clone(),
    }
}

fn pad(s: &str, width: usize, on: bool) -> String {
    if !on || s.chars().count() >= width {
        s.to_string()
    } else {
        format!("{s}{}", " ".repeat(width - s.chars().count()))
    }
}

fn max_width(items: &[String]) -> usize {
    items.iter().map(|s| s.chars().count()).max().unwrap_or(0)
}

// --- Python conventions -----------------------------------------------------

fn python_body(d: &Doc, style: &str) -> Vec<String> {
    let ph = &d.ph;
    let ind = &d.ind;
    let mut b: Vec<String> = Vec::new();

    match style {
        "numpy" => {
            b.push(ph.clone());
            if d.ext {
                b.push(String::new());
                b.push(ph.clone());
            }
            if !d.params.is_empty() {
                b.push(String::new());
                b.push("Parameters".into());
                b.push("----------".into());
                for p in &d.params {
                    let n = disp_name(p, Lang::Python);
                    let head = match (&p.ty, p.optional) {
                        (Some(t), true) => format!("{n} : {t}, optional"),
                        (Some(t), false) => format!("{n} : {t}"),
                        (None, true) => format!("{n} : optional"),
                        (None, false) => n,
                    };
                    b.push(head);
                    let mut desc = format!("{ind}{ph}");
                    if let Some(dv) = &p.default {
                        desc.push_str(&format!(", by default {dv}"));
                    }
                    b.push(desc);
                }
            }
            if d.show_return {
                b.push(String::new());
                b.push("Returns".into());
                b.push("-------".into());
                if let Some(r) = &d.ret {
                    b.push(r.clone());
                }
                b.push(format!("{ind}{ph}"));
            }
            if !d.raises.is_empty() {
                b.push(String::new());
                b.push("Raises".into());
                b.push("------".into());
                for r in &d.raises {
                    b.push(r.clone());
                    b.push(format!("{ind}{ph}"));
                }
            }
            if d.examples {
                b.push(String::new());
                b.push("Examples".into());
                b.push("--------".into());
                b.push(format!(">>> {}({})", d.name, ph));
            }
        }
        "sphinx" => {
            b.push(ph.clone());
            if d.ext {
                b.push(String::new());
                b.push(ph.clone());
            }
            b.push(String::new());
            for p in &d.params {
                let n = disp_name(p, Lang::Python);
                let mut line = format!(":param {n}: {ph}");
                if let Some(dv) = &p.default {
                    line.push_str(&format!(", defaults to {dv}"));
                }
                b.push(line);
                if let Some(t) = &p.ty {
                    b.push(format!(
                        ":type {n}: {t}{}",
                        if p.optional { ", optional" } else { "" }
                    ));
                }
            }
            for r in &d.raises {
                b.push(format!(":raises {r}: {ph}"));
            }
            if d.show_return {
                b.push(format!(":return: {ph}"));
                if let Some(r) = &d.ret {
                    b.push(format!(":rtype: {r}"));
                }
            }
            if d.examples {
                b.push(String::new());
                b.push(format!(">>> {}({})", d.name, ph));
            }
        }
        "epytext" => {
            b.push(ph.clone());
            if d.ext {
                b.push(String::new());
                b.push(ph.clone());
            }
            b.push(String::new());
            for p in &d.params {
                let n = disp_name(p, Lang::Python);
                b.push(format!("@param {n}: {ph}"));
                if let Some(t) = &p.ty {
                    b.push(format!("@type {n}: {t}"));
                }
            }
            for r in &d.raises {
                b.push(format!("@raise {r}: {ph}"));
            }
            if d.show_return {
                b.push(format!("@return: {ph}"));
                if let Some(r) = &d.ret {
                    b.push(format!("@rtype: {r}"));
                }
            }
            if d.examples {
                b.push(String::new());
                b.push(format!(">>> {}({})", d.name, ph));
            }
        }
        "pep257" => {
            b.push(ph.clone());
            if d.ext {
                b.push(String::new());
                b.push(ph.clone());
            }
            if !d.params.is_empty() {
                b.push(String::new());
                b.push("Arguments:".into());
                for p in &d.params {
                    let mut line = format!("{} -- {ph}", disp_name(p, Lang::Python));
                    if let Some(dv) = &p.default {
                        line.push_str(&format!(" (default {dv})"));
                    }
                    b.push(line);
                }
            }
            if d.show_return {
                b.push(String::new());
                b.push("Returns:".into());
                b.push(match &d.ret {
                    Some(r) => format!("{r} -- {ph}"),
                    None => ph.clone(),
                });
            }
            if !d.raises.is_empty() {
                b.push(String::new());
                b.push("Raises:".into());
                for r in &d.raises {
                    b.push(format!("{r} -- {ph}"));
                }
            }
            if d.examples {
                b.push(String::new());
                b.push("Examples:".into());
                b.push(format!(">>> {}({})", d.name, ph));
            }
        }
        // "google" and anything unresolved
        _ => {
            b.push(ph.clone());
            if d.ext {
                b.push(String::new());
                b.push(ph.clone());
            }
            if !d.params.is_empty() {
                b.push(String::new());
                b.push("Args:".into());
                for p in &d.params {
                    let n = disp_name(p, Lang::Python);
                    let spec = match (&p.ty, p.optional) {
                        (Some(t), true) => format!(" ({t}, optional)"),
                        (Some(t), false) => format!(" ({t})"),
                        (None, true) => " (optional)".to_string(),
                        (None, false) => String::new(),
                    };
                    let mut line = format!("{ind}{n}{spec}: {ph}");
                    if let Some(dv) = &p.default {
                        line.push_str(&format!(". Defaults to {dv}."));
                    }
                    b.push(line);
                }
            }
            if d.show_return {
                b.push(String::new());
                b.push("Returns:".into());
                b.push(match &d.ret {
                    Some(r) => format!("{ind}{r}: {ph}"),
                    None => format!("{ind}{ph}"),
                });
            }
            if !d.raises.is_empty() {
                b.push(String::new());
                b.push("Raises:".into());
                for r in &d.raises {
                    b.push(format!("{ind}{r}: {ph}"));
                }
            }
            if d.examples {
                b.push(String::new());
                b.push("Examples:".into());
                b.push(format!("{ind}>>> {}({})", d.name, ph));
            }
        }
    }
    while b.last().map(|l| l.is_empty()).unwrap_or(false) {
        b.pop();
    }
    b
}

fn render_python(d: &Doc, style: &str) -> Vec<String> {
    let q = d.quote;
    let body = python_body(d, style);
    if body.len() == 1 {
        return vec![format!("{q}{}{q}", body[0])];
    }
    let mut out = vec![format!("{q}{}", body[0])];
    out.extend(body[1..].iter().cloned());
    out.push(q.to_string());
    out
}

// --- Tag-block conventions --------------------------------------------------

fn wrap_block(lines: Vec<String>, open: &str, prefix: &str, close: Option<&str>) -> Vec<String> {
    let mut out = vec![open.to_string()];
    for l in lines {
        if l.is_empty() {
            out.push(prefix.trim_end().to_string());
        } else {
            out.push(format!("{prefix}{l}"));
        }
    }
    if let Some(c) = close {
        out.push(c.to_string());
    }
    out
}

fn render_jsdoc(d: &Doc, lang: Lang) -> Vec<String> {
    let ph = &d.ph;
    let mut body = vec![ph.clone()];
    if d.ext {
        body.push(String::new());
        body.push(ph.clone());
    }
    let types: Vec<String> = d
        .params
        .iter()
        .map(|p| match &p.ty {
            Some(t) if p.variadic.is_some() => format!("{{...{t}}}"),
            Some(t) => format!("{{{t}}}"),
            None => String::new(),
        })
        .collect();
    let names: Vec<String> = d
        .params
        .iter()
        .map(|p| match (&p.default, p.optional) {
            (Some(dv), _) => format!("[{}={}]", p.name, dv),
            (None, true) => format!("[{}]", p.name),
            (None, false) => p.name.clone(),
        })
        .collect();
    let tw = max_width(&types);
    let nw = max_width(&names);
    if !d.params.is_empty() {
        body.push(String::new());
    }
    for i in 0..d.params.len() {
        let t = pad(&types[i], tw, d.align);
        let n = pad(&names[i], nw, d.align);
        let line = if types[i].is_empty() {
            format!("@param {n} - {ph}")
        } else {
            format!("@param {t} {n} - {ph}")
        };
        body.push(line.trim_end().to_string());
    }
    if d.show_return {
        body.push(match &d.ret {
            Some(r) => format!("@returns {{{r}}} {ph}"),
            None => format!("@returns {ph}"),
        });
    }
    for r in &d.raises {
        body.push(format!("@throws {{{r}}} {ph}"));
    }
    if d.examples {
        body.push("@example".into());
        body.push(format!("{}({})", d.name, ph));
    }
    let _ = lang;
    wrap_block(body, "/**", " * ", Some(" */"))
}

fn render_phpdoc(d: &Doc) -> Vec<String> {
    let ph = &d.ph;
    let mut body = vec![ph.clone()];
    if d.ext {
        body.push(String::new());
        body.push(ph.clone());
    }
    let types: Vec<String> = d
        .params
        .iter()
        .map(|p| match &p.ty {
            Some(t) if p.variadic.is_some() => format!("{t} ..."),
            Some(t) => t.clone(),
            None => String::new(),
        })
        .collect();
    let names: Vec<String> = d.params.iter().map(|p| format!("${}", p.name)).collect();
    let tw = max_width(&types);
    let nw = max_width(&names);
    if !d.params.is_empty() {
        body.push(String::new());
    }
    for i in 0..d.params.len() {
        let t = pad(&types[i], tw, d.align);
        let n = pad(&names[i], nw, d.align);
        let line = if types[i].is_empty() {
            format!("@param {n} {ph}")
        } else {
            format!("@param {t} {n} {ph}")
        };
        body.push(line.trim_end().to_string());
    }
    if d.show_return {
        body.push(match &d.ret {
            Some(r) => format!("@return {r} {ph}"),
            None => format!("@return {ph}"),
        });
    }
    for r in &d.raises {
        body.push(format!("@throws {r} {ph}"));
    }
    if d.examples {
        body.push(format!("@example {}({})", d.name, ph));
    }
    wrap_block(body, "/**", " * ", Some(" */"))
}

fn render_javadoc(d: &Doc) -> Vec<String> {
    let ph = &d.ph;
    let mut body = vec![ph.clone()];
    if d.ext {
        body.push(String::new());
        body.push(ph.clone());
    }
    let names: Vec<String> = d.params.iter().map(|p| p.name.clone()).collect();
    let nw = max_width(&names);
    if !d.params.is_empty() {
        body.push(String::new());
    }
    for (i, p) in d.params.iter().enumerate() {
        let n = pad(&names[i], nw, d.align);
        let ty = match &p.ty {
            Some(t) => format!(" the {t}"),
            None => String::new(),
        };
        body.push(format!("@param {n} {ph}{ty}").trim_end().to_string());
    }
    if d.show_return {
        body.push(match &d.ret {
            Some(r) => format!("@return {ph} ({r})"),
            None => format!("@return {ph}"),
        });
    }
    for r in &d.raises {
        body.push(format!("@throws {r} {ph}"));
    }
    if d.examples {
        body.push(format!("@see {}", d.name));
    }
    wrap_block(body, "/**", " * ", Some(" */"))
}

fn render_xmldoc(d: &Doc) -> Vec<String> {
    let ph = &d.ph;
    let mut body = vec!["<summary>".to_string(), ph.clone(), "</summary>".to_string()];
    if d.ext {
        body.push(format!("<remarks>{ph}</remarks>"));
    }
    for p in &d.params {
        body.push(format!("<param name=\"{}\">{ph}</param>", p.name));
    }
    if d.show_return {
        body.push(format!("<returns>{ph}</returns>"));
    }
    for r in &d.raises {
        body.push(format!("<exception cref=\"{r}\">{ph}</exception>"));
    }
    if d.examples {
        body.push(format!("<example>{}({})</example>", d.name, ph));
    }
    wrap_block(body, "/// <summary>", "/// ", None)
        .into_iter()
        .skip(1)
        .collect()
}

fn render_godoc(d: &Doc) -> Vec<String> {
    let ph = &d.ph;
    let mut body = vec![format!("{} {ph}", d.name)];
    if d.ext {
        body.push(String::new());
        body.push(ph.clone());
    }
    if !d.params.is_empty() {
        body.push(String::new());
        body.push("Parameters:".into());
        for p in &d.params {
            let ty = match &p.ty {
                Some(t) if p.variadic.is_some() => format!(" (...{t})"),
                Some(t) => format!(" ({t})"),
                None => String::new(),
            };
            body.push(format!("  - {}{ty}: {ph}", p.name));
        }
    }
    if d.show_return {
        body.push(String::new());
        body.push("Returns:".into());
        body.push(match &d.ret {
            Some(r) => format!("  - {r}: {ph}"),
            None => format!("  - {ph}"),
        });
    }
    if !d.raises.is_empty() {
        body.push(String::new());
        body.push("Errors:".into());
        for r in &d.raises {
            body.push(format!("  - {r}: {ph}"));
        }
    }
    if d.examples {
        body.push(String::new());
        body.push("Example:".into());
        body.push(format!("  {}({})", d.name, ph));
    }
    wrap_block(body, "//", "// ", None)
        .into_iter()
        .skip(1)
        .collect()
}

fn render_rustdoc(d: &Doc) -> Vec<String> {
    let ph = &d.ph;
    let mut body = vec![ph.clone()];
    if d.ext {
        body.push(String::new());
        body.push(ph.clone());
    }
    if !d.params.is_empty() {
        body.push(String::new());
        body.push("# Arguments".into());
        body.push(String::new());
        for p in &d.params {
            let ty = match &p.ty {
                Some(t) => format!(" (`{t}`)"),
                None => String::new(),
            };
            body.push(format!("* `{}`{ty} - {ph}", p.name));
        }
    }
    if d.show_return {
        body.push(String::new());
        body.push("# Returns".into());
        body.push(String::new());
        body.push(match &d.ret {
            Some(r) => format!("`{r}` - {ph}"),
            None => ph.clone(),
        });
    }
    if !d.raises.is_empty() {
        body.push(String::new());
        body.push("# Errors".into());
        body.push(String::new());
        for r in &d.raises {
            body.push(format!("Returns [`{r}`] when {ph}."));
        }
    }
    if d.examples {
        body.push(String::new());
        body.push("# Examples".into());
        body.push(String::new());
        body.push("```".into());
        body.push(format!("{}({});", d.name, ph));
        body.push("```".into());
    }
    wrap_block(body, "///", "/// ", None)
        .into_iter()
        .skip(1)
        .collect()
}

fn render_yard(d: &Doc) -> Vec<String> {
    let ph = &d.ph;
    let mut body = vec![ph.clone()];
    if d.ext {
        body.push(String::new());
        body.push(ph.clone());
    }
    if !d.params.is_empty() {
        body.push(String::new());
    }
    for p in &d.params {
        let n = disp_name(p, Lang::Ruby);
        let ty = match &p.ty {
            Some(t) => format!(" [{t}]"),
            None => String::new(),
        };
        let mut line = format!("@param {n}{ty} {ph}");
        if let Some(dv) = &p.default {
            line.push_str(&format!(" (default: {dv})"));
        }
        body.push(line);
    }
    if d.show_return {
        body.push(match &d.ret {
            Some(r) => format!("@return [{r}] {ph}"),
            None => format!("@return {ph}"),
        });
    }
    for r in &d.raises {
        body.push(format!("@raise [{r}] {ph}"));
    }
    if d.examples {
        body.push("@example".into());
        body.push(format!("  {}({})", d.name, ph));
    }
    wrap_block(body, "#", "# ", None)
        .into_iter()
        .skip(1)
        .collect()
}

fn render(d: &Doc, lang: Lang, style: &str) -> Vec<String> {
    match lang {
        Lang::Python => render_python(d, style),
        Lang::JavaScript | Lang::TypeScript => render_jsdoc(d, lang),
        Lang::Php => render_phpdoc(d),
        Lang::Java => render_javadoc(d),
        Lang::CSharp => render_xmldoc(d),
        Lang::Go => render_godoc(d),
        Lang::Rust => render_rustdoc(d),
        Lang::Ruby => render_yard(d),
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

const STYLES: [&str; 6] = ["auto", "google", "numpy", "sphinx", "epytext", "pep257"];
const OUTPUTS: [&str; 3] = ["annotated", "docstring", "json"];
const TYPE_MODES: [&str; 3] = ["guess", "annotated", "none"];

/// Generate documentation stubs for every function signature in `signature`.
#[allow(clippy::too_many_arguments)]
pub fn generate(
    signature: &str,
    language: &str,
    style: &str,
    output: &str,
    types: &str,
    placeholder: &str,
    raises: &str,
    quote_style: &str,
    extended_summary: bool,
    examples: bool,
    align_tags: bool,
    indent_size: i64,
) -> Result<String, String> {
    if signature.trim().is_empty() {
        return Err("signature is required — paste at least one function signature".to_string());
    }
    if signature.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes; the cap is {MAX_INPUT_BYTES} bytes",
            signature.len()
        ));
    }
    if !STYLES.contains(&style) {
        return Err(format!(
            "unknown style '{style}' — expected one of: {}",
            STYLES.join(", ")
        ));
    }
    if !OUTPUTS.contains(&output) {
        return Err(format!(
            "unknown output '{output}' — expected one of: {}",
            OUTPUTS.join(", ")
        ));
    }
    if !TYPE_MODES.contains(&types) {
        return Err(format!(
            "unknown types mode '{types}' — expected one of: {}",
            TYPE_MODES.join(", ")
        ));
    }
    let quote = match quote_style {
        "double" => "\"\"\"",
        "single" => "'''",
        other => {
            return Err(format!(
                "unknown quote_style '{other}' — expected 'double' or 'single'"
            ))
        }
    };
    if !(0..=8).contains(&indent_size) {
        return Err(format!(
            "indent_size must be between 0 and 8 spaces, got {indent_size}"
        ));
    }
    let lang = if language == "auto" {
        detect_lang(signature)
    } else {
        Lang::parse(language).ok_or_else(|| {
            format!(
                "unknown language '{language}' — expected one of: auto, python, javascript, \
                 typescript, php, java, csharp, go, rust, ruby"
            )
        })?
    };
    let mode = match types {
        "none" => TypeMode::None,
        "annotated" => TypeMode::Annotated,
        _ => TypeMode::Guess,
    };
    let resolved_style = if style == "auto" { "google" } else { style };
    let ph = if placeholder.trim().is_empty() {
        "_description_".to_string()
    } else {
        placeholder.trim().to_string()
    };
    let declared: Vec<String> = raises
        .split([',', ';'])
        .flat_map(|p| p.split_whitespace())
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect();
    if declared.len() > MAX_RAISES {
        return Err(format!(
            "too many exception names ({}); the cap is {MAX_RAISES}",
            declared.len()
        ));
    }
    let ind = " ".repeat(indent_size as usize);

    let segs = segment(signature, lang)?;

    if output == "json" {
        let mut funcs: Vec<Value> = Vec::new();
        for seg in &segs {
            if let Seg::Sig { info, .. } = seg {
                let (params, ret, show_return) = resolve(info, lang, mode);
                let mut exc = declared.clone();
                for t in &info.throws {
                    if !exc.contains(t) {
                        exc.push(t.clone());
                    }
                }
                funcs.push(json!({
                    "name": info.name,
                    "async": info.is_async,
                    "params": params.iter().map(|p| json!({
                        "name": p.name,
                        "type": p.ty,
                        "default": p.default,
                        "optional": p.optional,
                        "variadic": p.variadic,
                    })).collect::<Vec<_>>(),
                    "returns": ret,
                    "returns_documented": show_return,
                    "raises": exc,
                }));
            }
        }
        let doc = json!({
            "language": lang.name(),
            "style": if lang == Lang::Python { resolved_style } else { "native" },
            "functions": funcs,
        });
        return serde_json::to_string_pretty(&doc).map_err(|e| e.to_string());
    }

    let mut out: Vec<String> = Vec::new();
    let mut blocks: Vec<String> = Vec::new();

    for seg in &segs {
        match seg {
            Seg::Pass(l) => {
                if output == "annotated" {
                    out.push(l.clone());
                }
            }
            Seg::Sig {
                raw,
                sig_start,
                info,
            } => {
                let (params, ret, show_return) = resolve(info, lang, mode);
                let mut exc = declared.clone();
                for t in &info.throws {
                    if !exc.contains(t) {
                        exc.push(t.clone());
                    }
                }
                let doc = Doc {
                    name: info.name.clone(),
                    params,
                    ret,
                    show_return,
                    raises: exc,
                    ph: ph.clone(),
                    ext: extended_summary,
                    examples,
                    align: align_tags,
                    ind: ind.clone(),
                    quote,
                };
                let lines = render(&doc, lang, resolved_style);
                let sig_line = &raw[*sig_start];
                let base: String = sig_line
                    .chars()
                    .take_while(|c| c.is_whitespace())
                    .collect();
                let doc_indent = if lang.doc_inside() {
                    format!("{base}{ind}")
                } else {
                    base.clone()
                };
                let indented: Vec<String> = lines
                    .iter()
                    .map(|l| {
                        if l.is_empty() {
                            String::new()
                        } else {
                            format!("{doc_indent}{l}")
                        }
                    })
                    .collect();
                if output == "docstring" {
                    blocks.push(indented.join("\n"));
                } else if lang.doc_inside() {
                    out.extend(raw.iter().cloned());
                    out.extend(indented);
                } else {
                    out.extend(indented);
                    out.extend(raw.iter().cloned());
                }
            }
        }
    }

    if output == "docstring" {
        return Ok(blocks.join("\n\n"));
    }
    while out.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
        out.pop();
    }
    Ok(out.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gen(sig: &str, lang: &str, style: &str, output: &str) -> String {
        generate(
            sig, lang, style, output, "guess", "_description_", "", "double", false, false, false, 4,
        )
        .unwrap()
    }

    #[test]
    fn python_google_happy_path() {
        let out = gen(
            "def fetch(url: str, timeout: int = 30) -> dict:",
            "python",
            "google",
            "docstring",
        );
        assert_eq!(
            out,
            concat!(
                "    \"\"\"_description_\n",
                "\n",
                "    Args:\n",
                "        url (str): _description_\n",
                "        timeout (int, optional): _description_. Defaults to 30.\n",
                "\n",
                "    Returns:\n",
                "        dict: _description_\n",
                "    \"\"\""
            )
        );
    }

    #[test]
    fn empty_signature_is_an_error() {
        let err = generate(
            "   ", "auto", "auto", "annotated", "guess", "_description_", "", "double", false,
            false, false, 4,
        )
        .unwrap_err();
        assert!(err.contains("signature is required"), "{err}");
    }

    #[test]
    fn unparseable_input_is_an_error() {
        let err = generate(
            "just some prose, no signature here",
            "python",
            "auto",
            "annotated",
            "guess",
            "_description_",
            "",
            "double",
            false,
            false,
            false,
            4,
        )
        .unwrap_err();
        assert!(err.contains("no function signature found"), "{err}");
    }

    #[test]
    fn bad_enum_values_are_rejected() {
        for (l, s, o, t, q) in [
            ("klingon", "auto", "annotated", "guess", "double"),
            ("python", "haiku", "annotated", "guess", "double"),
            ("python", "auto", "poem", "guess", "double"),
            ("python", "auto", "annotated", "psychic", "double"),
            ("python", "auto", "annotated", "guess", "curly"),
        ] {
            assert!(
                generate("def f(a):", l, s, o, t, "_description_", "", q, false, false, false, 4)
                    .is_err(),
                "{l}/{s}/{o}/{t}/{q} should be rejected"
            );
        }
        assert!(generate(
            "def f(a):", "python", "auto", "annotated", "guess", "_d_", "", "double", false, false,
            false, 99
        )
        .is_err());
    }

    #[test]
    fn python_annotated_keeps_body_and_self_is_skipped() {
        let src = "class C:\n    def add(self, a, b=2):\n        return a + b";
        let out = gen(src, "python", "google", "annotated");
        assert_eq!(
            out,
            concat!(
                "class C:\n",
                "    def add(self, a, b=2):\n",
                "        \"\"\"_description_\n",
                "\n",
                "        Args:\n",
                "            a (_type_): _description_\n",
                "            b (int, optional): _description_. Defaults to 2.\n",
                "\n",
                "        Returns:\n",
                "            _type_: _description_\n",
                "        \"\"\"\n",
                "        return a + b"
            )
        );
    }

    #[test]
    fn python_varargs_and_decorators() {
        let src = "@app.route(\"/x\")\ndef handler(req, *args, **kwargs):";
        let out = gen(src, "python", "google", "annotated");
        assert!(out.starts_with("@app.route(\"/x\")\ndef handler("), "{out}");
        assert!(out.contains("*args (_type_): _description_"), "{out}");
        assert!(out.contains("**kwargs (_type_): _description_"), "{out}");
    }

    #[test]
    fn python_numpy_sphinx_epytext_pep257() {
        let sig = "def f(a: int, b: str = \"x\") -> bool:";
        let numpy = gen(sig, "python", "numpy", "docstring");
        assert!(numpy.contains("Parameters\n    ----------"), "{numpy}");
        assert!(numpy.contains("b : str, optional"), "{numpy}");
        assert!(numpy.contains(", by default \"x\""), "{numpy}");

        let sphinx = gen(sig, "python", "sphinx", "docstring");
        assert!(sphinx.contains(":param b: _description_, defaults to \"x\""), "{sphinx}");
        assert!(sphinx.contains(":type b: str, optional"), "{sphinx}");
        assert!(sphinx.contains(":rtype: bool"), "{sphinx}");

        let epy = gen(sig, "python", "epytext", "docstring");
        assert!(epy.contains("@type a: int"), "{epy}");
        assert!(epy.contains("@rtype: bool"), "{epy}");

        let pep = gen(sig, "python", "pep257", "docstring");
        assert!(pep.contains("b -- _description_ (default \"x\")"), "{pep}");
        assert!(pep.contains("bool -- _description_"), "{pep}");
    }

    #[test]
    fn python_one_liner_when_nothing_to_document() {
        let out = gen("def ping() -> None:", "python", "google", "docstring");
        assert_eq!(out, "    \"\"\"_description_\"\"\"");
    }

    #[test]
    fn quote_style_single() {
        let out = generate(
            "def ping() -> None:", "python", "auto", "docstring", "guess", "_description_", "",
            "single", false, false, false, 4,
        )
        .unwrap();
        assert_eq!(out, "    '''_description_'''");
    }

    #[test]
    fn jsdoc_from_arrow_and_defaults() {
        let out = gen(
            "const send = (to, subject = \"hi\", ...rest) => {}",
            "javascript",
            "auto",
            "docstring",
        );
        assert_eq!(
            out,
            concat!(
                "/**\n",
                " * _description_\n",
                " *\n",
                " * @param {*} to - _description_\n",
                " * @param {string} [subject=\"hi\"] - _description_\n",
                " * @param {...*} rest - _description_\n",
                " * @returns {*} _description_\n",
                " */"
            )
        );
    }

    #[test]
    fn jsdoc_align_tags_pads_columns() {
        let out = generate(
            "function f(a, bbbb = 1) {}",
            "javascript",
            "auto",
            "docstring",
            "guess",
            "_description_",
            "",
            "double",
            false,
            false,
            true,
            4,
        )
        .unwrap();
        let cols: Vec<usize> = out
            .lines()
            .filter(|l| l.contains("@param"))
            .map(|l| l.find(" - _description_").unwrap())
            .collect();
        assert_eq!(cols.len(), 2, "{out}");
        assert_eq!(cols[0], cols[1], "@param descriptions must line up:\n{out}");
        assert!(out.contains("{*}"), "{out}");
        assert!(out.contains("[bbbb=1]"), "{out}");

        // …and without align_tags the columns are ragged.
        let ragged = generate(
            "function f(a, bbbb = 1) {}",
            "javascript",
            "auto",
            "docstring",
            "guess",
            "_description_",
            "",
            "double",
            false,
            false,
            false,
            4,
        )
        .unwrap();
        assert!(ragged.contains(" * @param {*} a - _description_"), "{ragged}");
        assert!(ragged.contains(" * @param {number} [bbbb=1] - _description_"), "{ragged}");
    }

    #[test]
    fn typescript_optional_and_return_type() {
        let out = gen(
            "export async function load(id: string, opts?: LoadOptions): Promise<User> {",
            "typescript",
            "auto",
            "docstring",
        );
        assert!(out.contains("@param {string} id - _description_"), "{out}");
        assert!(out.contains("@param {LoadOptions} [opts] - _description_"), "{out}");
        assert!(out.contains("@returns {Promise<User>} _description_"), "{out}");
    }

    #[test]
    fn typescript_generics_do_not_break_the_comma_split() {
        let out = gen(
            "function pick<T>(map: Map<string, number>, keys: Array<string>): T {",
            "typescript",
            "auto",
            "json",
        );
        let v: Value = serde_json::from_str(&out).unwrap();
        let params = v["functions"][0]["params"].as_array().unwrap();
        assert_eq!(params.len(), 2, "{out}");
        assert_eq!(params[0]["type"], "Map<string, number>");
        assert_eq!(v["functions"][0]["name"], "pick");
    }

    #[test]
    fn php_phpdoc() {
        let out = gen(
            "public function fetch(string $url, int $timeout = 30): bool {",
            "php",
            "auto",
            "docstring",
        );
        assert!(out.contains("@param string $url _description_"), "{out}");
        assert!(out.contains("@param int $timeout _description_"), "{out}");
        assert!(out.contains("@return bool _description_"), "{out}");
    }

    #[test]
    fn java_javadoc_picks_up_throws() {
        let out = gen(
            "public static String read(File f, int limit) throws IOException, ParseException {",
            "java",
            "auto",
            "docstring",
        );
        assert!(out.contains("@param f _description_ the File"), "{out}");
        assert!(out.contains("@return _description_ (String)"), "{out}");
        assert!(out.contains("@throws IOException _description_"), "{out}");
        assert!(out.contains("@throws ParseException _description_"), "{out}");
    }

    #[test]
    fn csharp_xmldoc_and_void_return_is_omitted() {
        let out = gen(
            "public void Log(string message, params object[] args)",
            "csharp",
            "auto",
            "docstring",
        );
        assert!(out.starts_with("/// <summary>"), "{out}");
        assert!(out.contains("/// <param name=\"message\">_description_</param>"), "{out}");
        assert!(!out.contains("<returns>"), "void must not get a returns tag: {out}");
    }

    #[test]
    fn go_receiver_and_grouped_params() {
        let out = gen(
            "func (s *Store) Put(key, value string, ttl int) (bool, error) {",
            "go",
            "auto",
            "json",
        );
        let v: Value = serde_json::from_str(&out).unwrap();
        let f = &v["functions"][0];
        assert_eq!(f["name"], "Put");
        assert_eq!(f["params"][0]["name"], "key");
        assert_eq!(f["params"][0]["type"], "string");
        assert_eq!(f["params"][1]["type"], "string");
        assert_eq!(f["params"][2]["type"], "int");
        assert_eq!(f["returns"], "bool, error");
    }

    #[test]
    fn rust_lifetimes_generics_and_self() {
        let out = gen(
            "pub fn join<'a>(&self, parts: &'a [&'a str], sep: &str) -> Result<String, Error> {",
            "rust",
            "auto",
            "json",
        );
        let v: Value = serde_json::from_str(&out).unwrap();
        let f = &v["functions"][0];
        assert_eq!(f["name"], "join");
        assert_eq!(f["params"].as_array().unwrap().len(), 2, "{out}");
        assert_eq!(f["params"][0]["name"], "parts");
        assert_eq!(f["params"][0]["type"], "&'a [&'a str]");
        assert_eq!(f["returns"], "Result<String, Error>");
    }

    #[test]
    fn ruby_yard_keyword_and_block_args() {
        let out = gen("def notify(user, urgent: false, &block)", "ruby", "auto", "docstring");
        assert!(out.contains("# @param user [Object] _description_"), "{out}");
        assert!(
            out.contains("# @param urgent [Boolean] _description_ (default: false)"),
            "{out}"
        );
        assert!(out.contains("# @param &block [Object] _description_"), "{out}");
    }

    #[test]
    fn types_none_and_annotated_modes() {
        let sig = "def f(a: int, b=2):";
        let none = generate(
            sig, "python", "google", "docstring", "none", "_description_", "", "double", false,
            false, false, 4,
        )
        .unwrap();
        assert!(none.contains("        a: _description_"), "{none}");
        assert!(!none.contains("(int)"), "{none}");

        let ann = generate(
            sig, "python", "google", "docstring", "annotated", "_description_", "", "double",
            false, false, false, 4,
        )
        .unwrap();
        assert!(ann.contains("a (int): _description_"), "{ann}");
        assert!(ann.contains("        b (optional): _description_. Defaults to 2."), "{ann}");
    }

    #[test]
    fn raises_extended_summary_examples_and_placeholder() {
        let out = generate(
            "def f(a):",
            "python",
            "google",
            "docstring",
            "guess",
            "TODO",
            "ValueError, KeyError",
            "double",
            true,
            true,
            false,
            4,
        )
        .unwrap();
        assert!(out.contains("\"\"\"TODO\n\n    TODO\n"), "{out}");
        assert!(out.contains("        ValueError: TODO"), "{out}");
        assert!(out.contains("        KeyError: TODO"), "{out}");
        assert!(out.contains("Examples:"), "{out}");
        assert!(out.contains(">>> f(TODO)"), "{out}");
    }

    #[test]
    fn indent_size_two() {
        let out = generate(
            "def f(a):", "python", "google", "docstring", "guess", "_description_", "", "double",
            false, false, false, 2,
        )
        .unwrap();
        assert!(out.starts_with("  \"\"\"_description_"), "{out}");
        assert!(out.contains("\n    a (_type_): _description_"), "{out}");
    }

    #[test]
    fn auto_detection_across_languages() {
        for (src, want) in [
            ("def f(a: int) -> str:", "python"),
            ("def f(a)\nend", "ruby"),
            ("func Add(a, b int) int {", "go"),
            ("pub fn add(a: i64) -> i64 {", "rust"),
            ("public function add($a) {", "php"),
            ("function add(a, b) {", "javascript"),
            ("function add(a: number): number {", "typescript"),
            ("public String add(String a) throws IOException {", "java"),
            ("public async Task<int> AddAsync(string a)", "csharp"),
        ] {
            let out = gen(src, "auto", "auto", "json");
            let v: Value = serde_json::from_str(&out).unwrap();
            assert_eq!(v["language"], want, "detecting {src:?} -> {out}");
        }
    }

    #[test]
    fn multiple_signatures_and_multiline_signature() {
        let src = "def a(x):\n\ndef b(\n    y: int,\n    z: str = \"q\",\n) -> None:";
        let out = gen(src, "python", "google", "json");
        let v: Value = serde_json::from_str(&out).unwrap();
        let fs = v["functions"].as_array().unwrap();
        assert_eq!(fs.len(), 2, "{out}");
        assert_eq!(fs[1]["name"], "b");
        assert_eq!(fs[1]["params"].as_array().unwrap().len(), 2);
        assert_eq!(fs[1]["params"][1]["default"], "\"q\"");
        assert_eq!(fs[1]["returns_documented"], false);
    }

    #[test]
    fn body_lines_are_not_mistaken_for_signatures() {
        let src = "function total(items) {\n  return items.reduce((sum, i) => sum + i.price, 0);\n}";
        let out = gen(src, "javascript", "auto", "json");
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["functions"].as_array().unwrap().len(), 1, "{out}");
        assert_eq!(v["functions"][0]["name"], "total");
    }

    #[test]
    fn strings_containing_commas_stay_one_parameter() {
        let out = gen("def f(sep=\", \", other=1):", "python", "auto", "json");
        let v: Value = serde_json::from_str(&out).unwrap();
        let ps = v["functions"][0]["params"].as_array().unwrap();
        assert_eq!(ps.len(), 2, "{out}");
        assert_eq!(ps[0]["default"], "\", \"");
    }

    #[test]
    fn oversized_input_is_rejected() {
        let big = "x".repeat(MAX_INPUT_BYTES + 1);
        let err = generate(
            &big, "python", "auto", "annotated", "guess", "_d_", "", "double", false, false, false,
            4,
        )
        .unwrap_err();
        assert!(err.contains("the cap is"), "{err}");
    }
}

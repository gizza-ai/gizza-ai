//! code-chunker core — split source code into function- / class-aligned chunks
//! suitable for embedding or an LLM context window (RAG over code).
//!
//! Pure-Rust (only `serde_json` for output). No wafer/wasm-bindgen deps — shared
//! by the chat skill block and the web page.
//!
//! There is **no full parser** here: a parser-generator's per-language grammars
//! are C libraries that neither build nor instantiate in the wasm sandbox, so
//! boundaries come from a heuristic that is deterministic and dependency-free:
//!
//! - **Brace languages** (JS/TS/Rust/Go/Java/C/C++/C#/PHP/Swift): scan the code
//!   tracking bracket depth `()[]{}`, ignoring brackets inside strings, char
//!   literals, and `//` / `/* */` comments. A top-level *unit* is a maximal run
//!   of lines that returns bracket depth to zero — one balanced construct (a
//!   function/class/…) or one top-level statement. A leading comment block folds
//!   into the unit that follows it.
//! - **Python**: indentation-based. A top-level unit is an indent-0 line plus
//!   every following blank or indented line (its body). Leading decorators
//!   (`@…`) and comments fold into the def/class they precede.
//!
//! Consecutive units are then greedily **packed** into chunks up to `max_lines`;
//! a single unit larger than `max_lines` is emitted whole (never split
//! mid-definition) and flagged `oversize`. Each chunk records its 1-based
//! `start_line`/`end_line`, `line_count`, the leading construct's `kind`/`name`,
//! and its text. Chunks do not overlap.

use serde_json::json;

/// A chunk must be allowed at least one line.
pub const MIN_MAX_LINES: u32 = 1;
/// Guard against absurd inputs.
pub const MAX_MAX_LINES: u32 = 100_000;
/// Default target chunk size, in lines.
pub const DEFAULT_MAX_LINES: u32 = 60;

/// Which boundary strategy + tokenizer quirks apply.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Lang {
    /// Indentation-driven (Python).
    Python,
    /// Bracket-driven, `'` is a single-char/lifetime marker (Rust).
    BraceRust,
    /// Bracket-driven, `'…'` is a normal string/char literal (everything else).
    Brace,
}

/// Parse the public `language` value into a strategy, resolving `auto`.
fn resolve_lang(language: &str, code: &str) -> Result<Lang, String> {
    match language.trim().to_ascii_lowercase().as_str() {
        "auto" | "" => Ok(detect_lang(code)),
        "python" | "py" => Ok(Lang::Python),
        "rust" | "rs" => Ok(Lang::BraceRust),
        "javascript" | "js" | "jsx" | "typescript" | "ts" | "tsx" | "go" | "golang" | "java"
        | "c" | "cpp" | "c++" | "cxx" | "csharp" | "c#" | "cs" | "php" | "swift" => Ok(Lang::Brace),
        other => Err(format!(
            "language must be one of auto, python, javascript, typescript, rust, go, java, c, cpp, csharp, php, swift (got {other:?})"
        )),
    }
}

/// Best-effort language guess for `language = "auto"`.
fn detect_lang(code: &str) -> Lang {
    let head = code.trim_start();
    if head.starts_with("#!") && head.lines().next().unwrap_or("").contains("python") {
        return Lang::Python;
    }
    let braces = code.bytes().filter(|&b| b == b'{').count();
    // Python: colon-terminated def/class headers and (almost) no braces.
    let looks_python = code.lines().any(|l| {
        let t = l.trim_start();
        (t.starts_with("def ") || t.starts_with("class ") || t.starts_with("async def "))
            && l.trim_end().ends_with(':')
    });
    if looks_python && braces == 0 {
        return Lang::Python;
    }
    // Rust: `fn ` plus a Rust-flavoured token, so `'` is treated as a lifetime.
    let looks_rust = (code.contains("fn ") || code.starts_with("fn"))
        && (code.contains("->")
            || code.contains("impl ")
            || code.contains("let ")
            || code.contains("::")
            || code.contains("pub fn"));
    if looks_rust {
        return Lang::BraceRust;
    }
    Lang::Brace
}

/// Output serialization.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum OutFormat {
    Json,
    Jsonl,
    Text,
}

impl OutFormat {
    fn parse(s: &str) -> Result<OutFormat, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "json" => Ok(OutFormat::Json),
            "jsonl" | "ndjson" => Ok(OutFormat::Jsonl),
            "text" | "plain" => Ok(OutFormat::Text),
            other => Err(format!(
                "format must be \"json\", \"jsonl\", or \"text\" (got {other:?})"
            )),
        }
    }
}

/// One detected top-level construct, as inclusive 0-based line indices.
struct Unit {
    start: usize,
    end: usize,
    kind: &'static str,
    name: String,
}

/// One emitted chunk (one or more packed units).
struct Chunk {
    start: usize, // inclusive 0-based
    end: usize,   // inclusive 0-based
    kind: &'static str,
    name: String,
    text: String,
    oversize: bool,
}

/// Split `code` into function-/class-aligned chunks. See the module docs.
///
/// `language` is `auto` or one of the supported languages; `max_lines` is the
/// target maximum lines per chunk; `format` is `json`, `jsonl`, or `text`.
pub fn chunk(code: &str, language: &str, max_lines: u32, format: &str) -> Result<String, String> {
    let lang = resolve_lang(language, code)?;
    let format = OutFormat::parse(format)?;
    if !(MIN_MAX_LINES..=MAX_MAX_LINES).contains(&max_lines) {
        return Err(format!(
            "max_lines must be between {MIN_MAX_LINES} and {MAX_MAX_LINES}"
        ));
    }

    let lines: Vec<&str> = code
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect();

    let units = match lang {
        Lang::Python => segment_python(&lines),
        Lang::BraceRust => segment_brace(&lines, true),
        Lang::Brace => segment_brace(&lines, false),
    };

    let chunks = pack(&units, &lines, max_lines as usize);
    Ok(render(&chunks, format))
}

// ---------------------------------------------------------------------------
// Brace-language segmentation
// ---------------------------------------------------------------------------

/// Per-line bracket depth *after* the line, and whether the line has real code
/// (any non-whitespace outside comments). `rust_squote` selects Rust's
/// char-literal/lifetime handling for `'`.
fn scan_brace(lines: &[&str], rust_squote: bool) -> (Vec<usize>, Vec<bool>) {
    let mut depth: i64 = 0;
    let mut in_block = false; // inside /* ... */
    let mut in_template = false; // inside `...` (backtick / raw string)
    let mut depth_after = Vec::with_capacity(lines.len());
    let mut has_code = Vec::with_capacity(lines.len());

    for line in lines {
        let chars: Vec<char> = line.chars().collect();
        let n = chars.len();
        let mut i = 0;
        let mut code = false;
        while i < n {
            let c = chars[i];
            if in_block {
                if c == '*' && i + 1 < n && chars[i + 1] == '/' {
                    in_block = false;
                    i += 2;
                    continue;
                }
                i += 1;
                continue;
            }
            if in_template {
                if c == '\\' {
                    i += 2;
                    continue;
                }
                if c == '`' {
                    in_template = false;
                }
                i += 1;
                continue;
            }
            match c {
                '/' if i + 1 < n && chars[i + 1] == '/' => break, // line comment
                '/' if i + 1 < n && chars[i + 1] == '*' => {
                    in_block = true;
                    i += 2;
                    continue;
                }
                '#' => {
                    // Preprocessor / attribute-ish line — treat the rest as
                    // opaque so macro braces don't skew depth.
                    code = true;
                    break;
                }
                '"' => {
                    code = true;
                    i += 1;
                    while i < n && chars[i] != '"' {
                        if chars[i] == '\\' {
                            i += 1;
                        }
                        i += 1;
                    }
                    i += 1; // past closing quote (or EOL)
                    continue;
                }
                '\'' => {
                    code = true;
                    if rust_squote {
                        // Rust: a char literal is 'x' or '\n'; anything else is a
                        // lifetime — treat the quote as an ordinary code char.
                        if i + 1 < n && chars[i + 1] == '\\' {
                            i += 2;
                            while i < n && chars[i] != '\'' {
                                if chars[i] == '\\' {
                                    i += 1;
                                }
                                i += 1;
                            }
                            i += 1;
                            continue;
                        } else if i + 2 < n && chars[i + 2] == '\'' {
                            i += 3;
                            continue;
                        } else {
                            i += 1; // lifetime tick
                            continue;
                        }
                    } else {
                        // C/JS/Go/…: '…' is a normal single-quoted string/char.
                        i += 1;
                        while i < n && chars[i] != '\'' {
                            if chars[i] == '\\' {
                                i += 1;
                            }
                            i += 1;
                        }
                        i += 1;
                        continue;
                    }
                }
                '`' => {
                    code = true;
                    in_template = true;
                    i += 1;
                    continue;
                }
                '(' | '[' | '{' => {
                    depth += 1;
                    code = true;
                    i += 1;
                }
                ')' | ']' | '}' => {
                    depth = (depth - 1).max(0);
                    code = true;
                    i += 1;
                }
                _ => {
                    if !c.is_whitespace() {
                        code = true;
                    }
                    i += 1;
                }
            }
        }
        depth_after.push(depth.max(0) as usize);
        has_code.push(code);
    }
    (depth_after, has_code)
}

fn segment_brace(lines: &[&str], rust_squote: bool) -> Vec<Unit> {
    let (depth_after, has_code) = scan_brace(lines, rust_squote);
    let n = lines.len();
    let mut units = Vec::new();
    let mut i = 0;
    while i < n {
        while i < n && lines[i].trim().is_empty() {
            i += 1;
        }
        if i >= n {
            break;
        }
        let start = i;
        // Unit ends at the first following line that has real code AND leaves
        // bracket depth at zero (one balanced construct / statement). Leading
        // comments therefore fold into the construct they precede.
        let mut end = n - 1;
        let mut j = start;
        while j < n {
            if has_code[j] && depth_after[j] == 0 {
                end = j;
                break;
            }
            j += 1;
        }
        // First real-code line in the unit is its signature.
        let sig = (start..=end).find(|&k| has_code[k]).unwrap_or(start);
        let (kind, name) = classify_brace(lines[sig]);
        units.push(Unit {
            start,
            end,
            kind,
            name,
        });
        i = end + 1;
    }
    units
}

/// Best-effort (kind, name) from a brace-language signature line.
fn classify_brace(line: &str) -> (&'static str, String) {
    let words = ident_words(line);
    for (idx, w) in words.iter().enumerate() {
        let next = words.get(idx + 1).cloned().unwrap_or("").to_string();
        match *w {
            "fn" | "function" | "func" | "def" => return ("function", next),
            "class" => return ("class", next),
            "struct" => return ("struct", next),
            "enum" => return ("enum", next),
            "interface" => return ("interface", next),
            "trait" => return ("trait", next),
            "impl" => return ("impl", next),
            "type" => return ("type", next),
            "namespace" | "module" | "mod" | "package" => return ("module", next),
            "let" | "const" | "var" => {
                if line.contains("=>") {
                    return ("function", next);
                }
                return ("variable", next);
            }
            _ => {}
        }
    }
    // No keyword: a bare method signature `name(args) {` → the ident before `(`.
    if let Some(paren) = line.find('(') {
        if line[..paren].find('{').is_none() {
            if let Some(name) = ident_words(&line[..paren]).last() {
                return ("function", (*name).to_string());
            }
        }
    }
    ("code", String::new())
}

// ---------------------------------------------------------------------------
// Python segmentation
// ---------------------------------------------------------------------------

fn indent_of(line: &str) -> usize {
    let mut n = 0;
    for c in line.chars() {
        match c {
            ' ' => n += 1,
            '\t' => n += 4,
            _ => break,
        }
    }
    n
}

fn segment_python(lines: &[&str]) -> Vec<Unit> {
    let n = lines.len();
    let mut units = Vec::new();
    let mut i = 0;
    while i < n {
        while i < n && lines[i].trim().is_empty() {
            i += 1;
        }
        if i >= n {
            break;
        }
        let start = i;
        // Fold leading indent-0 decorators / comments into the following unit.
        while i < n {
            let t = lines[i].trim_start();
            if indent_of(lines[i]) == 0 && (t.starts_with('@') || t.starts_with('#')) {
                i += 1;
                while i < n && lines[i].trim().is_empty() {
                    i += 1;
                }
            } else {
                break;
            }
        }
        if i >= n {
            // Trailing comments/decorators with no following code.
            let (kind, name) = classify_python(lines[start]);
            units.push(Unit {
                start,
                end: n - 1,
                kind,
                name,
            });
            break;
        }
        let head = i;
        // The body is the following blank or more-indented lines.
        let mut j = head + 1;
        while j < n && (lines[j].trim().is_empty() || indent_of(lines[j]) > 0) {
            j += 1;
        }
        // Trim trailing blank lines back into the gap between units.
        let mut end = j - 1;
        while end > head && lines[end].trim().is_empty() {
            end -= 1;
        }
        let (kind, name) = classify_python(lines[head]);
        units.push(Unit {
            start,
            end,
            kind,
            name,
        });
        i = j;
    }
    units
}

fn classify_python(line: &str) -> (&'static str, String) {
    let words = ident_words(line);
    for (idx, w) in words.iter().enumerate() {
        let next = words.get(idx + 1).cloned().unwrap_or("").to_string();
        match *w {
            "def" => return ("function", next),
            "class" => return ("class", next),
            "async" => {} // followed by def → handled on next iteration
            _ => {}
        }
    }
    ("code", String::new())
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Split a line into identifier tokens (`[A-Za-z0-9_]+`), preserving order.
fn ident_words(line: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let bytes = line.as_bytes();
    let mut i = 0;
    let n = bytes.len();
    while i < n {
        let b = bytes[i];
        if b.is_ascii_alphanumeric() || b == b'_' {
            let s = i;
            while i < n && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            out.push(&line[s..i]);
        } else {
            i += 1;
        }
    }
    out
}

/// Greedily pack consecutive units into chunks up to `max_lines`. A unit larger
/// than `max_lines` becomes its own oversize chunk (never split mid-definition).
fn pack(units: &[Unit], lines: &[&str], max_lines: usize) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let mut cur_first: Option<usize> = None; // index into `units`
    let mut cur_last: usize = 0;

    let flush = |first: usize, last: usize, chunks: &mut Vec<Chunk>| {
        let s = units[first].start;
        let e = units[last].end;
        let text = lines[s..=e].join("\n");
        let line_count = e - s + 1;
        chunks.push(Chunk {
            start: s,
            end: e,
            kind: units[first].kind,
            name: units[first].name.clone(),
            text,
            oversize: line_count > max_lines,
        });
    };

    for (idx, u) in units.iter().enumerate() {
        match cur_first {
            None => {
                cur_first = Some(idx);
                cur_last = idx;
            }
            Some(first) => {
                let combined = u.end - units[first].start + 1;
                if combined <= max_lines {
                    cur_last = idx;
                } else {
                    flush(first, cur_last, &mut chunks);
                    cur_first = Some(idx);
                    cur_last = idx;
                }
            }
        }
    }
    if let Some(first) = cur_first {
        flush(first, cur_last, &mut chunks);
    }
    chunks
}

fn render(chunks: &[Chunk], format: OutFormat) -> String {
    match format {
        OutFormat::Json => {
            let arr: Vec<_> = chunks
                .iter()
                .enumerate()
                .map(|(i, c)| record(i, c))
                .collect();
            serde_json::to_string_pretty(&arr).unwrap_or_else(|_| "[]".into())
        }
        OutFormat::Jsonl => chunks
            .iter()
            .enumerate()
            .map(|(i, c)| record(i, c).to_string())
            .collect::<Vec<_>>()
            .join("\n"),
        OutFormat::Text => chunks
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let label = if c.name.is_empty() {
                    c.kind.to_string()
                } else {
                    format!("{} {}", c.kind, c.name)
                };
                format!(
                    "===== chunk {i} — {label} (lines {}-{}) =====\n{}",
                    c.start + 1,
                    c.end + 1,
                    c.text
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n"),
    }
}

fn record(index: usize, c: &Chunk) -> serde_json::Value {
    json!({
        "index": index,
        "start_line": c.start + 1,
        "end_line": c.end + 1,
        "line_count": c.end - c.start + 1,
        "kind": c.kind,
        "name": c.name,
        "oversize": c.oversize,
        "text": c.text,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> Vec<serde_json::Value> {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn rust_functions_each_own_chunk() {
        let code = "fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n\nfn sub(a: i32, b: i32) -> i32 {\n    a - b\n}\n";
        let out = chunk(code, "rust", 3, "json").unwrap();
        let recs = parse(&out);
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[0]["kind"], json!("function"));
        assert_eq!(recs[0]["name"], json!("add"));
        assert_eq!(recs[0]["start_line"], json!(1));
        assert_eq!(recs[0]["end_line"], json!(3));
        assert_eq!(recs[1]["name"], json!("sub"));
        assert_eq!(recs[1]["start_line"], json!(5));
    }

    #[test]
    fn rust_lifetime_does_not_break_bracket_depth() {
        // The `'a` lifetime must not be read as a string that eats the '(' .
        let code = "fn longest<'a>(x: &'a str, y: &'a str) -> &'a str {\n    if x.len() > y.len() { x } else { y }\n}\n";
        let out = chunk(code, "rust", 60, "json").unwrap();
        let recs = parse(&out);
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0]["name"], json!("longest"));
        assert_eq!(recs[0]["end_line"], json!(3));
    }

    #[test]
    fn csharp_single_line_class_returns() {
        let out = chunk("class F { void M() {} }", "csharp", 60, "json").unwrap();
        let recs = parse(&out);
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0]["kind"], json!("class"));
        assert_eq!(recs[0]["name"], json!("F"));
    }

    #[test]
    fn leading_doc_comment_folds_into_function() {
        let code = "// adds two numbers\nfunction add(a, b) {\n  return a + b;\n}\n";
        let out = chunk(code, "javascript", 60, "json").unwrap();
        let recs = parse(&out);
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0]["start_line"], json!(1)); // comment included
        assert_eq!(recs[0]["kind"], json!("function"));
        assert_eq!(recs[0]["name"], json!("add"));
        assert!(recs[0]["text"].as_str().unwrap().starts_with("// adds"));
    }

    #[test]
    fn small_units_pack_together_up_to_max_lines() {
        let code = "import a;\nimport b;\nimport c;\nimport d;\n";
        // 4 single-line imports, max 2 lines → two chunks of two imports.
        let out = chunk(code, "javascript", 2, "json").unwrap();
        let recs = parse(&out);
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[0]["line_count"], json!(2));
        assert_eq!(recs[0]["text"], json!("import a;\nimport b;"));
        assert_eq!(recs[1]["text"], json!("import c;\nimport d;"));
    }

    #[test]
    fn oversize_unit_kept_whole_and_flagged() {
        let code = "function big() {\n  a;\n  b;\n  c;\n  d;\n}\n";
        // 6-line function, max 3 → still one chunk, flagged oversize.
        let out = chunk(code, "javascript", 3, "json").unwrap();
        let recs = parse(&out);
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0]["line_count"], json!(6));
        assert_eq!(recs[0]["oversize"], json!(true));
    }

    #[test]
    fn python_class_with_methods_is_one_unit() {
        let code = "class Foo:\n    def a(self):\n        return 1\n\n    def b(self):\n        return 2\n\nx = 3\n";
        // max 6 keeps the 6-line class its own chunk, separate from `x = 3`.
        let out = chunk(code, "python", 6, "json").unwrap();
        let recs = parse(&out);
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[0]["kind"], json!("class"));
        assert_eq!(recs[0]["name"], json!("Foo"));
        assert_eq!(recs[0]["start_line"], json!(1));
        assert_eq!(recs[0]["end_line"], json!(6));
        assert_eq!(recs[1]["text"], json!("x = 3"));
    }

    #[test]
    fn python_decorator_folds_into_def() {
        let code = "@decorator\ndef greet(name):\n    return name\n";
        let out = chunk(code, "python", 60, "json").unwrap();
        let recs = parse(&out);
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0]["start_line"], json!(1));
        assert_eq!(recs[0]["kind"], json!("function"));
        assert_eq!(recs[0]["name"], json!("greet"));
    }

    #[test]
    fn auto_detects_python() {
        let code = "def f(x):\n    return x\n";
        let out = chunk(code, "auto", 60, "json").unwrap();
        let recs = parse(&out);
        assert_eq!(recs[0]["kind"], json!("function"));
        assert_eq!(recs[0]["name"], json!("f"));
    }

    #[test]
    fn jsonl_one_object_per_line() {
        let code = "fn a() {}\nfn b() {}\n";
        let out = chunk(code, "rust", 1, "jsonl").unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 2);
        let first: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(first["name"], json!("a"));
        assert_eq!(first["index"], json!(0));
    }

    #[test]
    fn text_format_has_headers() {
        let code = "fn a() {}\n";
        let out = chunk(code, "rust", 60, "text").unwrap();
        assert!(out.starts_with("===== chunk 0 — function a (lines 1-1) =====\n"));
        assert!(out.contains("fn a() {}"));
    }

    #[test]
    fn empty_input_yields_empty_output() {
        assert_eq!(chunk("", "auto", 60, "json").unwrap(), "[]");
        assert_eq!(chunk("", "auto", 60, "jsonl").unwrap(), "");
        assert_eq!(chunk("   \n  \n", "auto", 60, "json").unwrap(), "[]");
    }

    #[test]
    fn braces_in_strings_do_not_count() {
        let code = "fn a() {\n    let s = \"} not a real close {\";\n    s\n}\n";
        let out = chunk(code, "rust", 60, "json").unwrap();
        let recs = parse(&out);
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0]["end_line"], json!(4));
    }

    #[test]
    fn rejects_bad_format() {
        assert!(chunk("fn a() {}", "rust", 60, "banana").is_err());
    }

    #[test]
    fn rejects_bad_language() {
        assert!(chunk("x", "cobol", 60, "json").is_err());
    }

    #[test]
    fn rejects_max_lines_out_of_range() {
        assert!(chunk("x", "auto", 0, "json").is_err());
        assert!(chunk("x", "auto", MAX_MAX_LINES + 1, "json").is_err());
    }
}

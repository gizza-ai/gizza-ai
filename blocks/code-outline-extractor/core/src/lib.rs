//! code-outline-extractor core — produce a structured, nested outline of the
//! functions, classes, and methods in pasted source code. Pure-Rust (only
//! `serde_json` for the JSON format). No wafer/wasm-bindgen deps — shared by the
//! chat skill block and the web page.
//!
//! There is **no full language parser** here: a parser-generator's per-language
//! grammars are C libraries that neither build nor instantiate in the wasm
//! sandbox, so the outline comes from a deterministic, dependency-free heuristic:
//!
//! - **Brace languages** (JS/TS/Rust/Go/Java/C/C++/C#/PHP/Swift): scan the code
//!   tracking bracket depth `()[]{}`, ignoring brackets inside strings, char
//!   literals, and `//` / `/* */` comments. A *declaration* is a line that
//!   introduces a named construct via a keyword (`class`/`struct`/`enum`/
//!   `interface`/`trait`/`impl`/`fn`/`func`/`function`/`def`/`namespace`/…) or a
//!   bare `name(args) {` method signature that opens a block. Its bracket depth
//!   at the start of the line is its nesting level, so methods inside a class
//!   nest under the class.
//! - **Python**: indentation-based. A `def` / `async def` / `class` header nests
//!   by its indentation, so methods nest under their class.
//!
//! A `function` whose parent is a class-like construct is relabeled `method`.
//! Output is a tree (indented text), a Markdown nested list, or nested JSON.

use serde_json::{json, Value};

/// Which scanning strategy + tokenizer quirks apply to the input.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Lang {
    /// Indentation-driven (Python).
    Python,
    /// Bracket-driven, `'` is a char-literal / lifetime marker (Rust).
    BraceRust,
    /// Bracket-driven, `'…'` is a normal char/string literal (everything else).
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
    Tree,
    Markdown,
    Json,
}

impl OutFormat {
    fn parse(s: &str) -> Result<OutFormat, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "tree" => Ok(OutFormat::Tree),
            "markdown" | "md" => Ok(OutFormat::Markdown),
            "json" => Ok(OutFormat::Json),
            other => Err(format!(
                "format must be \"tree\", \"markdown\", or \"json\" (got {other:?})"
            )),
        }
    }
}

/// One declared construct plus its nested children.
struct Node {
    kind: &'static str,
    name: String,
    line: usize, // 1-based
    signature: String,
    children: Vec<Node>,
}

/// Extract a structured outline of the functions, classes, and methods in `code`.
///
/// - `language`: `auto` or one of python/javascript/typescript/rust/go/java/c/cpp/csharp/php/swift.
/// - `format`: `tree` (indented text), `markdown` (nested list), or `json` (nested objects).
/// - `signatures`: when true, show each entry's trimmed header line instead of just its name
///   (ignored by the `json` format, which always carries the signature field).
/// - `line_numbers`: when true, annotate each entry with its 1-based source line (ignored by `json`).
///
/// Returns `Err` only on an invalid `language` or `format`.
pub fn outline(
    code: &str,
    language: &str,
    format: &str,
    signatures: bool,
    line_numbers: bool,
) -> Result<String, String> {
    let lang = resolve_lang(language, code)?;
    let format = OutFormat::parse(format)?;

    let lines: Vec<&str> = code
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect();

    let decls = match lang {
        Lang::Python => collect_python(&lines),
        Lang::BraceRust => collect_brace(&lines, true),
        Lang::Brace => collect_brace(&lines, false),
    };
    let mut roots = build_tree(decls);
    relabel_methods(&mut roots, "");

    Ok(render(&roots, format, signatures, line_numbers))
}

// ---------------------------------------------------------------------------
// Flat declaration list (source order, with a nesting depth)
// ---------------------------------------------------------------------------

/// A detected declaration before it is assembled into a tree.
struct Decl {
    depth: usize, // brace depth (brace langs) or indent columns (python)
    kind: &'static str,
    name: String,
    line: usize, // 1-based
    signature: String,
}

// ---------------------------------------------------------------------------
// Brace-language scanning
// ---------------------------------------------------------------------------

/// Per-line bracket depth *after* the line, and whether the line has real code
/// (any non-whitespace outside comments). `rust_squote` selects Rust's
/// char-literal / lifetime handling for `'`. (Mirrors code-chunker's scanner.)
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

/// Collect brace-language declarations, tagged with the bracket depth at the
/// start of their line (their nesting level).
fn collect_brace(lines: &[&str], rust_squote: bool) -> Vec<Decl> {
    let (depth_after, has_code) = scan_brace(lines, rust_squote);
    let mut out = Vec::new();
    let mut prev_after = 0usize;
    for (i, raw) in lines.iter().enumerate() {
        let depth_before = prev_after;
        prev_after = depth_after[i];
        if !has_code[i] {
            continue;
        }
        // A line "opens a block" when its net brace effect is positive — the
        // signal that a bare `name(args)` is a method definition, not a call.
        let opens_block = depth_after[i] > depth_before;
        if let Some((kind, name, sig)) = classify_brace(raw, opens_block) {
            out.push(Decl {
                depth: depth_before,
                kind,
                name,
                line: i + 1,
                signature: sig,
            });
        }
    }
    out
}

/// Best-effort (kind, name, signature) from a brace-language line — or `None`
/// when the line is not a declaration. `opens_block` gates keyword-less method
/// signatures so a plain call `foo();` is not mistaken for a definition.
fn classify_brace(line: &str, opens_block: bool) -> Option<(&'static str, String, String)> {
    let words = ident_words(line);
    for (idx, w) in words.iter().enumerate() {
        let next = || words.get(idx + 1).cloned().unwrap_or("").to_string();
        match *w {
            "class" => return Some(("class", next(), clean_sig(line))),
            "struct" => return Some(("struct", next(), clean_sig(line))),
            "enum" => return Some(("enum", next(), clean_sig(line))),
            "interface" => return Some(("interface", next(), clean_sig(line))),
            "trait" => return Some(("trait", next(), clean_sig(line))),
            "impl" => return Some(("impl", impl_name(&words), clean_sig(line))),
            "namespace" | "module" | "mod" | "package" => {
                return Some(("module", next(), clean_sig(line)))
            }
            "fn" | "func" | "function" | "def" => {
                return Some(("function", next(), clean_sig(line)))
            }
            "let" | "const" | "var" if line.contains("=>") => {
                // Arrow function bound to a name: `const add = (a, b) => {`.
                return Some(("function", next(), clean_sig(line)));
            }
            _ => {}
        }
    }
    // No keyword: a bare method signature `name(args) {` that opens a block →
    // the identifier before `(`. Gated on `opens_block` so calls don't match.
    if opens_block {
        if let Some(paren) = line.find('(') {
            let before = &line[..paren];
            if !before.contains('{') {
                if let Some(name) = ident_words(before).last() {
                    // Reject control-flow keywords that also take `(...) { ... }`.
                    if !is_control_keyword(name) {
                        return Some(("function", (*name).to_string(), clean_sig(line)));
                    }
                }
            }
        }
    }
    None
}

/// `impl Foo`, `impl<'a> Trait for Foo<'a>` → the implementing type's name.
fn impl_name(words: &[&str]) -> String {
    if let Some(pos) = words.iter().position(|&w| w == "for") {
        // `impl Trait for Type` → the type after `for`.
        return words.get(pos + 1).cloned().unwrap_or("").to_string();
    }
    // `impl Type` → the first ident after `impl`.
    if let Some(pos) = words.iter().position(|&w| w == "impl") {
        return words.get(pos + 1).cloned().unwrap_or("").to_string();
    }
    String::new()
}

fn is_control_keyword(word: &str) -> bool {
    matches!(
        word,
        "if" | "else"
            | "for"
            | "while"
            | "do"
            | "switch"
            | "case"
            | "catch"
            | "try"
            | "return"
            | "foreach"
            | "using"
            | "lock"
            | "synchronized"
            | "match"
            | "when"
            | "with"
    )
}

// ---------------------------------------------------------------------------
// Python scanning
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

/// Collect Python `def` / `async def` / `class` declarations, tagged with their
/// indentation (their nesting level).
fn collect_python(lines: &[&str]) -> Vec<Decl> {
    let mut out = Vec::new();
    for (i, raw) in lines.iter().enumerate() {
        let t = raw.trim_start();
        let (kind, name) = if let Some(rest) = t.strip_prefix("class ") {
            ("class", first_ident(rest))
        } else if let Some(rest) = t.strip_prefix("def ") {
            ("function", first_ident(rest))
        } else if let Some(rest) = t.strip_prefix("async def ") {
            ("function", first_ident(rest))
        } else {
            continue;
        };
        out.push(Decl {
            depth: indent_of(raw),
            kind,
            name,
            line: i + 1,
            signature: clean_sig(raw),
        });
    }
    out
}

/// The leading identifier of `rest` (a Python `class`/`def` tail).
fn first_ident(rest: &str) -> String {
    rest.chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect()
}

// ---------------------------------------------------------------------------
// Tree assembly + rendering
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

/// Trim a declaration's header line down to a compact signature: drop the
/// trailing block opener (`{`), the Python `:`, and any leftover `=>`.
fn clean_sig(line: &str) -> String {
    let mut s = line.trim().to_string();
    loop {
        let before = s.len();
        for suffix in ["{", ":", "=>"] {
            if let Some(stripped) = s.strip_suffix(suffix) {
                s = stripped.trim_end().to_string();
            }
        }
        if s.len() == before {
            break;
        }
    }
    s
}

/// Assemble the flat, source-ordered declarations into a nesting tree: a
/// declaration at a deeper depth becomes a child of the nearest shallower open
/// declaration. Handles depth gaps (a construct nested inside a non-declaring
/// block still attaches to its nearest declared ancestor).
fn build_tree(decls: Vec<Decl>) -> Vec<Node> {
    let mut roots: Vec<Node> = Vec::new();
    let mut stack: Vec<Node> = Vec::new();
    let mut depths: Vec<usize> = Vec::new();

    let attach = |node: Node, stack: &mut Vec<Node>, roots: &mut Vec<Node>| {
        if let Some(parent) = stack.last_mut() {
            parent.children.push(node);
        } else {
            roots.push(node);
        }
    };

    for d in decls {
        // Close every open construct at the same or deeper depth.
        while let Some(&top) = depths.last() {
            if top >= d.depth {
                let closed = stack.pop().unwrap();
                depths.pop();
                attach(closed, &mut stack, &mut roots);
            } else {
                break;
            }
        }
        stack.push(Node {
            kind: d.kind,
            name: d.name,
            line: d.line,
            signature: d.signature,
            children: Vec::new(),
        });
        depths.push(d.depth);
    }
    // Unwind whatever is still open.
    while let Some(closed) = stack.pop() {
        depths.pop();
        attach(closed, &mut stack, &mut roots);
    }
    roots
}

/// A `function` nested directly inside a class-like construct is a `method`.
fn relabel_methods(nodes: &mut [Node], parent_kind: &str) {
    let parent_is_type = matches!(
        parent_kind,
        "class" | "struct" | "interface" | "trait" | "impl" | "enum"
    );
    for node in nodes.iter_mut() {
        if parent_is_type && node.kind == "function" {
            node.kind = "method";
        }
        let this_kind = node.kind;
        relabel_methods(&mut node.children, this_kind);
    }
}

fn render(roots: &[Node], format: OutFormat, signatures: bool, line_numbers: bool) -> String {
    match format {
        OutFormat::Json => {
            let arr: Vec<Value> = roots.iter().map(to_json).collect();
            serde_json::to_string_pretty(&arr).unwrap_or_else(|_| "[]".into())
        }
        OutFormat::Tree => {
            if roots.is_empty() {
                return EMPTY_MSG.into();
            }
            let mut buf = String::new();
            for node in roots {
                render_tree(node, 0, signatures, line_numbers, &mut buf);
            }
            buf.trim_end().to_string()
        }
        OutFormat::Markdown => {
            if roots.is_empty() {
                return EMPTY_MSG.into();
            }
            let mut buf = String::new();
            for node in roots {
                render_md(node, 0, signatures, line_numbers, &mut buf);
            }
            buf.trim_end().to_string()
        }
    }
}

const EMPTY_MSG: &str = "No functions, classes, or methods found.";

/// The display label for one node (`kind name` or `kind signature`).
fn label(node: &Node, signatures: bool) -> String {
    if signatures && !node.signature.is_empty() {
        format!("{} {}", node.kind, node.signature)
    } else if node.name.is_empty() {
        node.kind.to_string()
    } else {
        format!("{} {}", node.kind, node.name)
    }
}

fn render_tree(node: &Node, depth: usize, signatures: bool, line_numbers: bool, buf: &mut String) {
    let indent = "  ".repeat(depth);
    buf.push_str(&indent);
    buf.push_str(&label(node, signatures));
    if line_numbers {
        buf.push_str(&format!("  [L{}]", node.line));
    }
    buf.push('\n');
    for child in &node.children {
        render_tree(child, depth + 1, signatures, line_numbers, buf);
    }
}

fn render_md(node: &Node, depth: usize, signatures: bool, line_numbers: bool, buf: &mut String) {
    let indent = "  ".repeat(depth);
    let display = if signatures && !node.signature.is_empty() {
        format!("**{}** `{}`", node.kind, node.signature)
    } else if node.name.is_empty() {
        format!("**{}**", node.kind)
    } else {
        format!("**{}** `{}`", node.kind, node.name)
    };
    buf.push_str(&format!("{indent}- {display}"));
    if line_numbers {
        buf.push_str(&format!(" — line {}", node.line));
    }
    buf.push('\n');
    for child in &node.children {
        render_md(child, depth + 1, signatures, line_numbers, buf);
    }
}

fn to_json(node: &Node) -> Value {
    json!({
        "kind": node.kind,
        "name": node.name,
        "line": node.line,
        "signature": node.signature,
        "children": node.children.iter().map(to_json).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_class_with_methods_nests() {
        let code =
            "class Greeter:\n    def hi(self):\n        return 1\n\n    def bye(self):\n        return 2\n\ndef main():\n    pass\n";
        let out = outline(code, "python", "tree", false, true).unwrap();
        let expected =
            "class Greeter  [L1]\n  method hi  [L2]\n  method bye  [L5]\nfunction main  [L8]";
        assert_eq!(out, expected);
    }

    #[test]
    fn brace_class_methods_become_methods() {
        // A Java-ish class with two methods that open blocks on their own line.
        let code = "class Calc {\n    int add(int a, int b) {\n        return a + b;\n    }\n    int sub(int a, int b) {\n        return a - b;\n    }\n}\n";
        let out = outline(code, "java", "tree", false, true).unwrap();
        let expected = "class Calc  [L1]\n  method add  [L2]\n  method sub  [L5]";
        assert_eq!(out, expected);
    }

    #[test]
    fn rust_impl_with_methods() {
        let code = "struct Point { x: i32 }\n\nimpl Point {\n    fn new() -> Self {\n        Point { x: 0 }\n    }\n}\n";
        let out = outline(code, "rust", "tree", false, true).unwrap();
        assert!(out.contains("struct Point  [L1]"), "got: {out}");
        assert!(out.contains("impl Point  [L3]"), "got: {out}");
        assert!(out.contains("  method new  [L4]"), "got: {out}");
    }

    #[test]
    fn bare_call_is_not_a_function() {
        // `foo();` is a call, not a declaration — it must not appear.
        let code = "function run() {\n    foo();\n    bar();\n}\n";
        let out = outline(code, "javascript", "tree", false, true).unwrap();
        assert_eq!(out, "function run  [L1]");
    }

    #[test]
    fn control_flow_not_mistaken_for_method() {
        let code = "class C {\n    void go() {\n        if (x) {\n            work();\n        }\n        for (int i = 0; i < 3; i++) {\n        }\n    }\n}\n";
        let out = outline(code, "java", "tree", false, true).unwrap();
        // Only the class and its one method — not `if`/`for`.
        assert_eq!(out, "class C  [L1]\n  method go  [L2]");
    }

    #[test]
    fn signatures_flag_shows_header() {
        let code = "def add(a, b):\n    return a + b\n";
        let out = outline(code, "python", "tree", true, false).unwrap();
        assert_eq!(out, "function def add(a, b)");
    }

    #[test]
    fn line_numbers_off() {
        let code = "def add(a, b):\n    return a + b\n";
        let out = outline(code, "python", "tree", false, false).unwrap();
        assert_eq!(out, "function add");
    }

    #[test]
    fn markdown_format_nests() {
        let code = "class A:\n    def m(self):\n        pass\n";
        let out = outline(code, "python", "markdown", false, true).unwrap();
        let expected = "- **class** `A` — line 1\n  - **method** `m` — line 2";
        assert_eq!(out, expected);
    }

    #[test]
    fn json_format_is_nested() {
        let code = "class A:\n    def m(self):\n        pass\n";
        let out = outline(code, "python", "json", false, true).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["kind"], json!("class"));
        assert_eq!(v[0]["name"], json!("A"));
        assert_eq!(v[0]["line"], json!(1));
        assert_eq!(v[0]["children"][0]["kind"], json!("method"));
        assert_eq!(v[0]["children"][0]["name"], json!("m"));
        assert_eq!(v[0]["children"][0]["line"], json!(2));
    }

    #[test]
    fn auto_detects_python() {
        let code = "def f(x):\n    return x\n";
        let out = outline(code, "auto", "tree", false, true).unwrap();
        assert_eq!(out, "function f  [L1]");
    }

    #[test]
    fn js_arrow_function_named() {
        let code = "const add = (a, b) => {\n    return a + b;\n};\n";
        let out = outline(code, "javascript", "tree", false, true).unwrap();
        assert_eq!(out, "function add  [L1]");
    }

    #[test]
    fn braces_in_strings_do_not_break_nesting() {
        let code = "class C {\n    String s = \"} not a close {\";\n    void m() {\n    }\n}\n";
        let out = outline(code, "java", "tree", false, true).unwrap();
        assert_eq!(out, "class C  [L1]\n  method m  [L3]");
    }

    #[test]
    fn empty_input_yields_message() {
        assert_eq!(outline("", "auto", "tree", false, true).unwrap(), EMPTY_MSG);
        assert_eq!(
            outline("   \n\n", "auto", "markdown", false, true).unwrap(),
            EMPTY_MSG
        );
        assert_eq!(outline("", "auto", "json", false, true).unwrap(), "[]");
    }

    #[test]
    fn no_declarations_yields_message() {
        let code = "x = 1\ny = 2\nprint(x + y)\n";
        assert_eq!(
            outline(code, "python", "tree", false, true).unwrap(),
            EMPTY_MSG
        );
    }

    #[test]
    fn rejects_bad_language() {
        let err = outline("x", "cobol", "tree", false, true).unwrap_err();
        assert!(err.contains("language must be one of"), "got: {err}");
    }

    #[test]
    fn rejects_bad_format() {
        let err = outline("x", "auto", "banana", false, true).unwrap_err();
        assert!(err.contains("format must be"), "got: {err}");
    }
}

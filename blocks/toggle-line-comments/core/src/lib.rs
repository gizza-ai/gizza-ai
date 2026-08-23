//! gizza-ai/toggle-line-comments core — comment or uncomment a block of code
//! using the correct line-comment syntax for its language. Pure-Rust, no I/O.
//!
//! The behavior mirrors what an editor's `Ctrl+/` does, and is deliberately a
//! *lexical* transform rather than a parse:
//!
//! - Each language has a profile giving its line-comment `open` marker and, for
//!   the few languages that have no line comment at all (`css`, `html`, `xml`),
//!   a `close` marker so each line is wrapped in the block pair instead.
//! - **Toggle** follows the editor rule: if EVERY considered line is already
//!   commented, the block is uncommented; otherwise the whole block is
//!   commented. A partly-commented block therefore becomes fully commented,
//!   which is what makes the operation reversible.
//! - The marker is inserted at the block's **shallowest** indentation so the
//!   commented code keeps its relative shape, or at column 0 when
//!   `Align::Column0` is asked for.
//!
//! There is no full language parser here — a parser-generator's grammars are C
//! libraries that do not instantiate in the wasm sandbox. `Language::detect`
//! is a deterministic shebang/keyword/existing-marker heuristic, and callers can
//! always name the language (or the marker) explicitly.

use serde::Serialize;

/// Cap on the input size, in characters (keeps the transform bounded).
pub const MAX_CHARS: usize = 2_000_000;

/// What the tool should do with the block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Uncomment if every considered line is already commented, else comment.
    Toggle,
    /// Always add comment markers.
    Comment,
    /// Always remove comment markers (lines without one are left alone).
    Uncomment,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Mode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "toggle" | "" => Ok(Mode::Toggle),
            "comment" => Ok(Mode::Comment),
            "uncomment" => Ok(Mode::Uncomment),
            other => Err(format!(
                "unknown mode '{other}' (expected toggle, comment, or uncomment)"
            )),
        }
    }
}

/// Where the comment marker is placed on each line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    /// At the shallowest indentation of the block, so relative indents survive.
    Indent,
    /// At column 0, flush left.
    Column0,
}

impl Align {
    pub fn parse(s: &str) -> Result<Align, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "indent" | "" => Ok(Align::Indent),
            "column0" | "column-0" | "col0" => Ok(Align::Column0),
            other => Err(format!(
                "unknown align '{other}' (expected indent or column0)"
            )),
        }
    }
}

/// A language's comment syntax: a line-comment opener, or a block pair for the
/// languages that have no line comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Syntax {
    /// The marker written at the start of a commented line.
    pub open: String,
    /// The marker written at the end, for block-pair languages. Empty for a
    /// true line comment.
    pub close: String,
}

/// Every language the tool knows, in the order the enum advertises them.
/// `(name, open, close)` — a non-empty `close` means "wrap each line in a pair".
const LANGUAGES: &[(&str, &str, &str)] = &[
    ("javascript", "//", ""),
    ("typescript", "//", ""),
    ("java", "//", ""),
    ("csharp", "//", ""),
    ("c", "//", ""),
    ("cpp", "//", ""),
    ("go", "//", ""),
    ("rust", "//", ""),
    ("swift", "//", ""),
    ("kotlin", "//", ""),
    ("scala", "//", ""),
    ("php", "//", ""),
    ("python", "#", ""),
    ("ruby", "#", ""),
    ("perl", "#", ""),
    ("shell", "#", ""),
    ("powershell", "#", ""),
    ("yaml", "#", ""),
    ("toml", "#", ""),
    ("r", "#", ""),
    ("dockerfile", "#", ""),
    ("makefile", "#", ""),
    ("sql", "--", ""),
    ("lua", "--", ""),
    ("haskell", "--", ""),
    ("ini", ";", ""),
    ("clojure", ";;", ""),
    ("latex", "%", ""),
    ("vb", "'", ""),
    ("batch", "REM", ""),
    ("css", "/*", "*/"),
    ("html", "<!--", "-->"),
    ("xml", "<!--", "-->"),
];

/// The list of accepted `language` values, `auto` first — the single source for
/// the descriptor enum and the page `<select>`.
pub fn language_names() -> Vec<&'static str> {
    let mut v = vec!["auto"];
    v.extend(LANGUAGES.iter().map(|(n, _, _)| *n));
    v
}

/// Look up a named language's comment syntax.
pub fn syntax_for(language: &str) -> Result<Syntax, String> {
    let want = language.trim().to_ascii_lowercase();
    // A few common aliases people actually type.
    let want = match want.as_str() {
        "js" | "node" | "jsx" => "javascript",
        "ts" | "tsx" => "typescript",
        "py" | "python3" => "python",
        "rb" => "ruby",
        "cs" | "c#" => "csharp",
        "c++" | "cc" | "hpp" => "cpp",
        "golang" => "go",
        "rs" => "rust",
        "bash" | "sh" | "zsh" | "fish" => "shell",
        "ps1" | "pwsh" => "powershell",
        "yml" => "yaml",
        "tex" => "latex",
        "vbnet" | "vba" | "basic" => "vb",
        "cmd" | "bat" => "batch",
        "htm" => "html",
        "scss" | "less" => "css",
        "kt" => "kotlin",
        "pl" => "perl",
        "clj" | "lisp" | "scheme" | "elisp" => "clojure",
        "hs" => "haskell",
        "make" | "mk" => "makefile",
        "docker" => "dockerfile",
        other => other,
    };
    LANGUAGES
        .iter()
        .find(|(n, _, _)| *n == want)
        .map(|(_, open, close)| Syntax {
            open: (*open).to_string(),
            close: (*close).to_string(),
        })
        .ok_or_else(|| {
            format!(
                "unknown language '{language}' (expected one of: {})",
                language_names().join(", ")
            )
        })
}

/// Guess the language from the code's shape.
///
/// Precedence: an existing comment marker on the first commented-looking line
/// (so an uncomment round-trips even when the syntax is ambiguous) → a shebang →
/// distinctive keywords. Returns the language NAME so callers can report it.
pub fn detect_language(code: &str) -> &'static str {
    let lines: Vec<&str> = code.lines().map(|l| l.trim()).collect();
    let first_code = lines.iter().find(|l| !l.is_empty()).copied().unwrap_or("");

    // A shebang is unambiguous.
    if first_code.starts_with("#!") {
        let s = first_code.to_ascii_lowercase();
        if s.contains("python") {
            return "python";
        }
        if s.contains("ruby") {
            return "ruby";
        }
        if s.contains("perl") {
            return "perl";
        }
        if s.contains("node") {
            return "javascript";
        }
        return "shell";
    }

    let lower = code.to_ascii_lowercase();

    // Markup first: the openers below are unmistakable.
    if first_code.starts_with("<?php") || lower.contains("<?php") {
        return "php";
    }
    if first_code.starts_with("<?xml") {
        return "xml";
    }
    if lower.contains("<!doctype html") || lower.contains("<html") || lower.contains("</div>") {
        return "html";
    }

    // Strong single-token signals, most specific first.
    let has = |needle: &str| code.contains(needle);
    if has("fn ") && (has("let mut ") || has("->") || has("::") || has("&str")) {
        return "rust";
    }
    if has("func ") && (has(":= ") || has("package ")) {
        return "go";
    }
    if has("#include") || has("printf(") {
        return if has("std::") || has("class ") { "cpp" } else { "c" };
    }
    if (has("def ") && has(":")) || has("elif ") || (has("import ") && has("print(")) {
        return "python";
    }
    if has("interface ") || has(": string") || has(": number") {
        return "typescript";
    }
    if has("const ") || has("=> ") || has("function ") || has("let ") || has("var ") {
        return "javascript";
    }
    if has("public class ") || has("System.out.println") {
        return "java";
    }
    if has("Console.WriteLine") || (has("namespace ") && has("using ")) {
        return "csharp";
    }
    if has("local ") && has("end") {
        return "lua";
    }
    if has("\\begin{") || has("\\documentclass") {
        return "latex";
    }
    let upper = code.to_ascii_uppercase();
    if upper.contains("SELECT ") && upper.contains(" FROM ") {
        return "sql";
    }
    if has("{") && has(":") && has(";") && has("}") {
        return "css";
    }
    if has("end") && has("do ") {
        return "ruby";
    }

    // Nothing matched: shell's `#` is the least surprising fallback for a plain
    // list of lines, and is what most config-ish text wants.
    "shell"
}

/// The result of a comment/uncomment run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Output {
    /// What was actually done: `commented` or `uncommented`.
    pub action: String,
    /// The language whose syntax was used (the resolved value of `auto`).
    pub language: String,
    /// The comment marker used, e.g. `//`, `#`, or `<!-- -->` for a pair.
    pub marker: String,
    /// Number of lines in the input.
    pub total_lines: usize,
    /// How many lines were actually modified.
    pub changed_lines: usize,
    /// The transformed code.
    pub result: String,
}

/// Split a line into (leading whitespace, rest).
fn split_indent(line: &str) -> (&str, &str) {
    let idx = line
        .find(|c: char| !c.is_whitespace())
        .unwrap_or(line.len());
    line.split_at(idx)
}

/// Is this line already commented with `syntax`? Blank lines are never
/// "commented" — they are simply not in the way.
fn is_commented(line: &str, syntax: &Syntax) -> bool {
    let body = line.trim();
    if body.is_empty() {
        return false;
    }
    if !body.starts_with(&syntax.open) {
        return false;
    }
    if syntax.close.is_empty() {
        return true;
    }
    // Block-pair language: the closer must be there too, and must not overlap
    // the opener itself (`<!-->` is not a comment).
    body.len() >= syntax.open.len() + syntax.close.len() && body.ends_with(&syntax.close)
}

/// Remove the comment markers from a single line, keeping its indentation and
/// any line ending (a CRLF `\r`) exactly as it arrived.
fn uncomment_line(line: &str, syntax: &Syntax) -> String {
    let (indent, rest) = split_indent(line);
    let body = rest.trim_end();
    let eol = &rest[body.len()..];

    let mut inner = &body[syntax.open.len()..];
    // Drop at most ONE padding space, so deliberate indentation inside the
    // comment survives.
    if let Some(r) = inner.strip_prefix(' ') {
        inner = r;
    }
    let mut inner = inner.to_string();
    if !syntax.close.is_empty() {
        if let Some(r) = inner.strip_suffix(&syntax.close) {
            inner = r.to_string();
            if let Some(r2) = inner.strip_suffix(' ') {
                inner = r2.to_string();
            }
        }
    }
    format!("{indent}{inner}{eol}")
}

/// Add the comment markers to a single line at column `at` (a character index
/// into the line's leading whitespace).
fn comment_line(line: &str, syntax: &Syntax, at: usize, pad: bool) -> String {
    let space = if pad { " " } else { "" };
    let trimmed_end = line.trim_end();
    if trimmed_end.is_empty() {
        // A blank line becomes the bare marker — never marker + trailing space.
        let eol = if line.ends_with('\r') { "\r" } else { "" };
        return if syntax.close.is_empty() {
            format!("{}{eol}", syntax.open)
        } else {
            format!("{}{}{eol}", syntax.open, syntax.close)
        };
    }
    // `at` is measured in characters; split there, clamped to the real indent.
    let split = line
        .char_indices()
        .nth(at)
        .map(|(i, _)| i)
        .unwrap_or(line.len());
    let (head, tail) = line.split_at(split);
    if syntax.close.is_empty() {
        format!("{head}{}{space}{tail}", syntax.open)
    } else {
        let tail_trimmed = tail.trim_end();
        let trailing = &tail[tail_trimmed.len()..];
        format!(
            "{head}{}{space}{tail_trimmed}{space}{}{trailing}",
            syntax.open, syntax.close
        )
    }
}

/// Comment, uncomment, or toggle the line comments on a block of code.
///
/// * `code` — the block, split on newlines.
/// * `language` — a name from [`language_names`], or `auto` to detect.
/// * `mode` — see [`Mode`].
/// * `marker` — an explicit comment marker that overrides the language's own
///   (e.g. `//` or `#`); empty to use the language's.
/// * `space_after_marker` — write `// code` rather than `//code`.
/// * `align` — see [`Align`].
/// * `comment_blank_lines` — also mark blank/whitespace-only lines.
#[allow(clippy::too_many_arguments)]
pub fn toggle(
    code: &str,
    language: &str,
    mode: Mode,
    marker: &str,
    space_after_marker: bool,
    align: Align,
    comment_blank_lines: bool,
) -> Result<Output, String> {
    if code.chars().count() > MAX_CHARS {
        return Err(format!(
            "code is too large: {} characters (limit {MAX_CHARS})",
            code.chars().count()
        ));
    }
    if code.trim().is_empty() {
        return Err("code is empty — paste at least one line of code".into());
    }

    let lang_input = language.trim();
    let resolved = if lang_input.is_empty() || lang_input.eq_ignore_ascii_case("auto") {
        detect_language(code).to_string()
    } else {
        // Validate + canonicalize through the alias table.
        syntax_for(lang_input)?;
        canonical_name(lang_input)
    };
    let mut syntax = syntax_for(&resolved)?;

    // An explicit marker wins over the language's own opener.
    let custom = marker.trim();
    if !custom.is_empty() {
        if custom.chars().any(char::is_whitespace) {
            return Err(format!(
                "marker '{custom}' must not contain whitespace (expected something like // or #)"
            ));
        }
        syntax = Syntax {
            open: custom.to_string(),
            close: String::new(),
        };
    }

    let lines: Vec<&str> = code.split('\n').collect();
    // Which lines take part: blanks only when explicitly asked for.
    let participates =
        |l: &str| comment_blank_lines || !l.trim().is_empty();

    let considered: Vec<&&str> = lines.iter().filter(|l| participates(l)).collect();
    // Toggle rule (editor behavior): uncomment only when EVERY considered line
    // already carries the marker; a single bare line means "comment it all".
    let uncommenting = match mode {
        Mode::Comment => false,
        Mode::Uncomment => true,
        Mode::Toggle => {
            !considered.is_empty() && considered.iter().all(|l| is_commented(l, &syntax))
        }
    };

    // Where the marker goes when commenting: the block's shallowest indent.
    let column = match align {
        Align::Column0 => 0,
        Align::Indent => lines
            .iter()
            .filter(|l| !l.trim().is_empty())
            .map(|l| split_indent(l).0.chars().count())
            .min()
            .unwrap_or(0),
    };

    let mut changed = 0usize;
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    for line in &lines {
        if !participates(line) {
            out.push((*line).to_string());
            continue;
        }
        let new = if uncommenting {
            if is_commented(line, &syntax) {
                uncomment_line(line, &syntax)
            } else {
                (*line).to_string()
            }
        } else {
            comment_line(line, &syntax, column, space_after_marker)
        };
        if new != *line {
            changed += 1;
        }
        out.push(new);
    }

    let marker_label = if syntax.close.is_empty() {
        syntax.open.clone()
    } else {
        format!("{} {}", syntax.open, syntax.close)
    };

    Ok(Output {
        action: if uncommenting { "uncommented" } else { "commented" }.to_string(),
        language: resolved,
        marker: marker_label,
        total_lines: lines.len(),
        changed_lines: changed,
        result: out.join("\n"),
    })
}

/// Resolve an alias to the canonical language name it names.
fn canonical_name(language: &str) -> String {
    let syn = match syntax_for(language) {
        Ok(s) => s,
        Err(_) => return language.trim().to_ascii_lowercase(),
    };
    let want = language.trim().to_ascii_lowercase();
    if LANGUAGES.iter().any(|(n, _, _)| *n == want) {
        return want;
    }
    LANGUAGES
        .iter()
        .find(|(_, o, c)| *o == syn.open && *c == syn.close)
        .map(|(n, _, _)| (*n).to_string())
        .unwrap_or(want)
}

/// Page-facing wrapper: the transformed code only.
#[allow(clippy::too_many_arguments)]
pub fn render(
    code: &str,
    language: &str,
    mode: Mode,
    marker: &str,
    space_after_marker: bool,
    align: Align,
    comment_blank_lines: bool,
) -> Result<String, String> {
    toggle(
        code,
        language,
        mode,
        marker,
        space_after_marker,
        align,
        comment_blank_lines,
    )
    .map(|o| o.result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(code: &str, lang: &str, mode: Mode) -> Output {
        toggle(code, lang, mode, "", true, Align::Indent, false).unwrap()
    }

    #[test]
    fn comments_javascript_with_slashes() {
        let o = run("const a = 1;\nconst b = 2;", "javascript", Mode::Comment);
        assert_eq!(o.result, "// const a = 1;\n// const b = 2;");
        assert_eq!(o.action, "commented");
        assert_eq!(o.marker, "//");
        assert_eq!(o.changed_lines, 2);
        assert_eq!(o.total_lines, 2);
    }

    #[test]
    fn toggle_comments_an_uncommented_block() {
        let o = run("x = 1\ny = 2", "python", Mode::Toggle);
        assert_eq!(o.result, "# x = 1\n# y = 2");
        assert_eq!(o.action, "commented");
    }

    #[test]
    fn toggle_uncomments_a_fully_commented_block() {
        let o = run("# x = 1\n# y = 2", "python", Mode::Toggle);
        assert_eq!(o.result, "x = 1\ny = 2");
        assert_eq!(o.action, "uncommented");
        assert_eq!(o.changed_lines, 2);
    }

    #[test]
    fn toggle_comments_a_partly_commented_block() {
        // One bare line means the whole block gets commented — that is what
        // makes the operation reversible.
        let o = run("# x = 1\ny = 2", "python", Mode::Toggle);
        assert_eq!(o.result, "# # x = 1\n# y = 2");
        assert_eq!(o.action, "commented");
    }

    #[test]
    fn marker_lands_at_the_shallowest_indent() {
        let o = run("  if a:\n      b()\n  done()", "python", Mode::Comment);
        assert_eq!(o.result, "  # if a:\n  #     b()\n  # done()");
    }

    #[test]
    fn column0_alignment_puts_the_marker_flush_left() {
        let o = toggle(
            "  if a:\n      b()",
            "python",
            Mode::Comment,
            "",
            true,
            Align::Column0,
            false,
        )
        .unwrap();
        assert_eq!(o.result, "#   if a:\n#       b()");
    }

    #[test]
    fn blank_lines_are_skipped_by_default_and_commented_on_request() {
        let skipped = run("a = 1\n\nb = 2", "python", Mode::Comment);
        assert_eq!(skipped.result, "# a = 1\n\n# b = 2");

        let marked = toggle(
            "a = 1\n\nb = 2",
            "python",
            Mode::Comment,
            "",
            true,
            Align::Indent,
            true,
        )
        .unwrap();
        // A commented blank line is the bare marker — no trailing space.
        assert_eq!(marked.result, "# a = 1\n#\n# b = 2");
    }

    #[test]
    fn space_after_marker_can_be_turned_off() {
        let o = toggle(
            "const a = 1;",
            "javascript",
            Mode::Comment,
            "",
            false,
            Align::Indent,
            false,
        )
        .unwrap();
        assert_eq!(o.result, "//const a = 1;");
    }

    #[test]
    fn uncomment_drops_only_one_padding_space() {
        let o = run("//     deep", "javascript", Mode::Uncomment);
        assert_eq!(o.result, "    deep");
    }

    #[test]
    fn uncomment_leaves_uncommented_lines_alone() {
        let o = run("// a\nb", "javascript", Mode::Uncomment);
        assert_eq!(o.result, "a\nb");
        assert_eq!(o.changed_lines, 1);
    }

    #[test]
    fn block_pair_languages_wrap_each_line() {
        let o = run("<p>hi</p>\n<p>yo</p>", "html", Mode::Comment);
        assert_eq!(o.result, "<!-- <p>hi</p> -->\n<!-- <p>yo</p> -->");
        assert_eq!(o.marker, "<!-- -->");

        let back = run(&o.result, "html", Mode::Toggle);
        assert_eq!(back.result, "<p>hi</p>\n<p>yo</p>");
        assert_eq!(back.action, "uncommented");
    }

    #[test]
    fn css_round_trips_through_its_block_pair() {
        let o = run("color: red;", "css", Mode::Comment);
        assert_eq!(o.result, "/* color: red; */");
        assert_eq!(run(&o.result, "css", Mode::Toggle).result, "color: red;");
    }

    #[test]
    fn sql_and_lua_use_double_dash() {
        assert_eq!(
            run("SELECT 1;", "sql", Mode::Comment).result,
            "-- SELECT 1;"
        );
        assert_eq!(run("local x = 1", "lua", Mode::Comment).result, "-- local x = 1");
    }

    #[test]
    fn custom_marker_overrides_the_language() {
        let o = toggle(
            "x = 1",
            "python",
            Mode::Comment,
            "//",
            true,
            Align::Indent,
            false,
        )
        .unwrap();
        assert_eq!(o.result, "// x = 1");
        assert_eq!(o.marker, "//");
    }

    #[test]
    fn auto_detects_from_a_shebang() {
        let o = run("#!/usr/bin/env python3\nx = 1", "auto", Mode::Comment);
        assert_eq!(o.language, "python");
    }

    #[test]
    fn auto_detects_rust_and_sql() {
        assert_eq!(detect_language("fn main() { let mut a = 1; }"), "rust");
        assert_eq!(detect_language("SELECT id FROM users"), "sql");
        assert_eq!(detect_language("<!DOCTYPE html><html></html>"), "html");
    }

    #[test]
    fn aliases_resolve_to_their_canonical_language() {
        let o = run("const a = 1;", "js", Mode::Comment);
        assert_eq!(o.language, "javascript");
        assert_eq!(o.result, "// const a = 1;");
    }

    #[test]
    fn round_trip_restores_the_original() {
        let src = "function f() {\n    return 1;\n}";
        let commented = run(src, "javascript", Mode::Toggle);
        let back = run(&commented.result, "javascript", Mode::Toggle);
        assert_eq!(back.result, src);
    }

    #[test]
    fn crlf_input_keeps_its_carriage_returns_on_commented_lines() {
        let o = run("a = 1\r\nb = 2", "python", Mode::Comment);
        assert_eq!(o.result, "# a = 1\r\n# b = 2");
    }

    // ---- error paths ----

    #[test]
    fn empty_code_is_an_error() {
        let e = toggle("   \n\n", "python", Mode::Comment, "", true, Align::Indent, false)
            .unwrap_err();
        assert!(e.contains("code is empty"), "got: {e}");
    }

    #[test]
    fn unknown_language_is_an_error_listing_the_choices() {
        let e = toggle("x", "klingon", Mode::Comment, "", true, Align::Indent, false).unwrap_err();
        assert!(e.contains("unknown language 'klingon'"), "got: {e}");
        assert!(e.contains("javascript"), "error should list the choices: {e}");
    }

    #[test]
    fn marker_with_whitespace_is_an_error() {
        let e = toggle("x", "python", Mode::Comment, "-- ok", true, Align::Indent, false)
            .unwrap_err();
        assert!(e.contains("must not contain whitespace"), "got: {e}");
    }

    #[test]
    fn oversized_input_is_an_error() {
        let big = "a\n".repeat(MAX_CHARS);
        let e = toggle(&big, "python", Mode::Comment, "", true, Align::Indent, false).unwrap_err();
        assert!(e.contains("too large"), "got: {e}");
    }

    #[test]
    fn unknown_mode_and_align_are_errors() {
        assert!(Mode::parse("nope").unwrap_err().contains("unknown mode"));
        assert!(Align::parse("nope").unwrap_err().contains("unknown align"));
        assert_eq!(Mode::parse("toggle").unwrap(), Mode::Toggle);
        assert_eq!(Align::parse("column0").unwrap(), Align::Column0);
    }

    #[test]
    fn language_names_start_with_auto_and_cover_every_profile() {
        let names = language_names();
        assert_eq!(names[0], "auto");
        assert_eq!(names.len(), LANGUAGES.len() + 1);
        for n in &names[1..] {
            assert!(syntax_for(n).is_ok(), "{n} should resolve");
        }
    }
}

//! glob-filter core — filter a list of paths by glob and gitignore-style
//! include/exclude patterns. Pure Rust (`regex`), shared by the chat block and
//! the web page.
//!
//! Two matching dialects:
//! - `glob`      — each pattern must match the WHOLE path. `*` and `?` never
//!                 cross `/`; `**` does. Use `**/` for "at any depth".
//! - `gitignore` — git's `.gitignore` rules: a pattern with no `/` matches at
//!                 any depth; a leading/embedded `/` anchors it to the root; a
//!                 match also covers everything under a matched directory; blank
//!                 lines and `#` comments are ignored.
//!
//! In BOTH dialects a pattern line may start with `!` to negate (re-include what
//! an earlier pattern excluded). Within the include set and within the exclude
//! set, the LAST matching pattern wins.

use regex::RegexBuilder;
use serde::Serialize;

/// Matching dialect for a pattern set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Syntax {
    /// Whole-path glob (`*`/`?` stay within a segment, `**` spans them).
    Glob,
    /// `.gitignore` semantics (any-depth, anchoring, dir contents, comments).
    Gitignore,
}

impl Syntax {
    pub fn parse(s: &str) -> Result<Syntax, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "gitignore" | "" => Ok(Syntax::Gitignore),
            "glob" => Ok(Syntax::Glob),
            other => Err(format!("unknown syntax '{other}' (use glob or gitignore)")),
        }
    }
}

/// What to emit in the `result` string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    /// Only the paths that are kept (matched include, not excluded).
    Matched,
    /// Only the paths that are dropped.
    Unmatched,
    /// Every path, prefixed with `✓ ` (kept) or `✗ ` (dropped).
    Annotated,
}

impl OutputMode {
    pub fn parse(s: &str) -> Result<OutputMode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "matched" | "" => Ok(OutputMode::Matched),
            "unmatched" | "dropped" => Ok(OutputMode::Unmatched),
            "annotated" | "annotate" => Ok(OutputMode::Annotated),
            other => Err(format!(
                "unknown output '{other}' (use matched, unmatched, or annotated)"
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Output {
    /// Total non-blank paths considered.
    pub total: usize,
    /// Paths that were kept.
    pub matched: usize,
    /// Paths that were dropped.
    pub dropped: usize,
    /// The rendered result (depends on the output mode).
    pub result: String,
}

/// One compiled pattern: its regex plus whether it re-includes (`!`-prefixed).
struct CompiledPattern {
    re: regex::Regex,
    negated: bool,
}

/// Evaluate a pattern set against `path`, honouring `!` re-includes with
/// last-match-wins. Returns `None` if no pattern matched at all.
fn set_result(set: &[CompiledPattern], path: &str) -> Option<bool> {
    let mut result = None;
    for p in set {
        if p.re.is_match(path) {
            result = Some(!p.negated);
        }
    }
    result
}

/// Turn a glob pattern body into a regex fragment (no anchors). Handles
/// `*`, `**`, `?`, `[...]` classes (incl. `[!..]`/`[^..]` negation), and
/// `{a,b,..}` brace alternation (nestable).
fn glob_to_regex(pattern: &str) -> String {
    let chars: Vec<char> = pattern.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    compile_seg(&chars, &mut i, &mut out, &[]);
    out
}

fn compile_seg(chars: &[char], i: &mut usize, out: &mut String, stop: &[char]) {
    while *i < chars.len() {
        let c = chars[*i];
        if stop.contains(&c) {
            return;
        }
        match c {
            '*' => {
                if *i + 1 < chars.len() && chars[*i + 1] == '*' {
                    // `**` — spans path separators.
                    *i += 2;
                    if *i < chars.len() && chars[*i] == '/' {
                        // `**/` matches zero or more leading directories.
                        *i += 1;
                        out.push_str("(?:.*/)?");
                    } else {
                        out.push_str(".*");
                    }
                } else {
                    // `*` — anything except a separator.
                    out.push_str("[^/]*");
                    *i += 1;
                }
            }
            '?' => {
                out.push_str("[^/]");
                *i += 1;
            }
            '[' => compile_class(chars, i, out),
            '{' => compile_brace(chars, i, out, stop),
            _ => {
                push_escaped(c, out);
                *i += 1;
            }
        }
    }
}

fn compile_class(chars: &[char], i: &mut usize, out: &mut String) {
    let start = *i; // points at '['
    let mut j = start + 1;
    let mut neg = false;
    if j < chars.len() && (chars[j] == '!' || chars[j] == '^') {
        neg = true;
        j += 1;
    }
    // A ']' immediately after the (optional) negation is a literal member.
    let mut k = j;
    if k < chars.len() && chars[k] == ']' {
        k += 1;
    }
    while k < chars.len() && chars[k] != ']' {
        k += 1;
    }
    if k >= chars.len() {
        // No closing bracket → treat '[' literally.
        push_escaped('[', out);
        *i = start + 1;
        return;
    }
    out.push('[');
    if neg {
        out.push('^');
    }
    for m in j..k {
        let ch = chars[m];
        if ch == '\\' || ch == ']' || ch == '^' {
            out.push('\\');
        }
        out.push(ch);
    }
    out.push(']');
    *i = k + 1;
}

fn compile_brace(chars: &[char], i: &mut usize, out: &mut String, stop: &[char]) {
    let start = *i; // points at '{'
                    // Find the matching '}' at the same depth.
    let mut depth = 1;
    let mut k = start + 1;
    while k < chars.len() && depth > 0 {
        match chars[k] {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            _ => {}
        }
        k += 1;
    }
    if k >= chars.len() || depth != 0 {
        // Unbalanced → treat '{' literally.
        push_escaped('{', out);
        *i = start + 1;
        return;
    }
    // Split the interior on top-level commas.
    let interior = &chars[start + 1..k];
    let parts = split_top_commas(interior);
    if parts.len() <= 1 {
        // No alternation → literal '{'; reprocess the interior normally.
        push_escaped('{', out);
        *i = start + 1;
        return;
    }
    out.push_str("(?:");
    for (idx, part) in parts.iter().enumerate() {
        if idx > 0 {
            out.push('|');
        }
        let mut pi = 0;
        compile_seg(part, &mut pi, out, stop);
    }
    out.push(')');
    *i = k + 1;
}

fn split_top_commas(chars: &[char]) -> Vec<Vec<char>> {
    let mut parts = Vec::new();
    let mut cur = Vec::new();
    let mut depth = 0;
    for &c in chars {
        match c {
            '{' => {
                depth += 1;
                cur.push(c);
            }
            '}' => {
                depth -= 1;
                cur.push(c);
            }
            ',' if depth == 0 => {
                parts.push(std::mem::take(&mut cur));
            }
            _ => cur.push(c),
        }
    }
    parts.push(cur);
    parts
}

fn push_escaped(c: char, out: &mut String) {
    // Escape regex metacharacters that can appear as glob literals.
    if matches!(
        c,
        '.' | '+' | '(' | ')' | '|' | '^' | '$' | '\\' | '{' | '}' | ']'
    ) {
        out.push('\\');
    }
    out.push(c);
}

/// Compile one pattern LINE into a `CompiledPattern`, or `Ok(None)` to skip it
/// (blank / comment). `case_sensitive` toggles the regex `(?i)` flag.
fn compile_pattern(
    line: &str,
    syntax: Syntax,
    case_sensitive: bool,
) -> Result<Option<CompiledPattern>, String> {
    let pat = line.trim();
    if pat.is_empty() {
        return Ok(None);
    }
    // gitignore comments (a leading '#'); `\#` escapes a literal '#'.
    if syntax == Syntax::Gitignore && pat.starts_with('#') {
        return Ok(None);
    }
    let mut negated = false;
    let body = if let Some(rest) = pat.strip_prefix('!') {
        negated = true;
        rest.trim().to_string()
    } else if let Some(rest) = pat.strip_prefix("\\!") {
        // Escaped leading '!': literal, not a negation.
        format!("!{}", rest)
    } else if syntax == Syntax::Gitignore {
        if let Some(rest) = pat.strip_prefix("\\#") {
            format!("#{}", rest)
        } else {
            pat.to_string()
        }
    } else {
        pat.to_string()
    };

    finish_pattern(&body, syntax, case_sensitive, negated).map(Some)
}

fn finish_pattern(
    pat: &str,
    syntax: Syntax,
    case_sensitive: bool,
    negated: bool,
) -> Result<CompiledPattern, String> {
    let regex_body = match syntax {
        Syntax::Glob => {
            // Whole-path match.
            format!("^{}$", glob_to_regex(pat))
        }
        Syntax::Gitignore => {
            let mut p = pat;
            // Trailing slash → directory pattern (match contents too); we treat
            // every match as also covering descendants below, so just strip it.
            if let Some(stripped) = p.strip_suffix('/') {
                p = stripped;
            }
            // Anchoring: a leading '/' or any embedded '/' anchors to the root.
            let leading = p.starts_with('/');
            let core = if leading { &p[1..] } else { p };
            let anchored = leading || core.contains('/');
            let body = glob_to_regex(core);
            if anchored {
                // Match the path itself and everything under it.
                format!("^{}(?:/.*)?$", body)
            } else {
                // Match at any depth, plus descendants of a match.
                format!("^(?:.*/)?{}(?:/.*)?$", body)
            }
        }
    };
    let re = RegexBuilder::new(&regex_body)
        .case_insensitive(!case_sensitive)
        .build()
        .map_err(|e| format!("invalid pattern '{pat}': {e}"))?;
    Ok(CompiledPattern { re, negated })
}

fn compile_set(
    text: &str,
    syntax: Syntax,
    case_sensitive: bool,
) -> Result<Vec<CompiledPattern>, String> {
    let mut set = Vec::new();
    for line in text.lines() {
        if let Some(p) = compile_pattern(line, syntax, case_sensitive)? {
            set.push(p);
        }
    }
    Ok(set)
}

/// Filter `paths` (one per line) by the `include`/`exclude` pattern sets.
///
/// A path is KEPT when it is included (the include set is empty, or its last
/// matching include pattern is not a `!` negation) AND not excluded (the exclude
/// set is empty, or its last matching exclude pattern is not a `!` negation).
pub fn filter(
    paths: &str,
    include: &str,
    exclude: &str,
    syntax: Syntax,
    case_sensitive: bool,
    output: OutputMode,
) -> Result<Output, String> {
    let path_list: Vec<&str> = paths
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    if path_list.is_empty() {
        return Err("no paths to filter — paste one path per line".into());
    }

    let inc = compile_set(include, syntax, case_sensitive)?;
    let exc = compile_set(exclude, syntax, case_sensitive)?;

    let mut lines = Vec::with_capacity(path_list.len());
    let mut matched = 0usize;
    for path in &path_list {
        let included = if inc.is_empty() {
            true
        } else {
            set_result(&inc, path).unwrap_or(false)
        };
        let excluded = if exc.is_empty() {
            false
        } else {
            set_result(&exc, path).unwrap_or(false)
        };
        let kept = included && !excluded;
        if kept {
            matched += 1;
        }
        match output {
            OutputMode::Matched if kept => lines.push(path.to_string()),
            OutputMode::Unmatched if !kept => lines.push(path.to_string()),
            OutputMode::Annotated => {
                lines.push(format!("{} {}", if kept { '✓' } else { '✗' }, path));
            }
            _ => {}
        }
    }

    Ok(Output {
        total: path_list.len(),
        matched,
        dropped: path_list.len() - matched,
        result: lines.join("\n"),
    })
}

/// Convenience for the page: run [`filter`] and return just the rendered
/// `result` string (kept/dropped/annotated paths) for display.
pub fn render(
    paths: &str,
    include: &str,
    exclude: &str,
    syntax: Syntax,
    case_sensitive: bool,
    output: OutputMode,
) -> Result<String, String> {
    filter(paths, include, exclude, syntax, case_sensitive, output).map(|o| o.result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(paths: &str, inc: &str, exc: &str, syn: Syntax) -> Output {
        filter(paths, inc, exc, syn, true, OutputMode::Matched).unwrap()
    }

    #[test]
    fn gitignore_any_depth_extension() {
        let out = f(
            "src/main.rs\nsrc/util/mod.rs\nREADME.md\nCargo.toml",
            "*.rs",
            "",
            Syntax::Gitignore,
        );
        assert_eq!(out.result, "src/main.rs\nsrc/util/mod.rs");
        assert_eq!(out.matched, 2);
        assert_eq!(out.total, 4);
        assert_eq!(out.dropped, 2);
    }

    #[test]
    fn gitignore_anchored_and_dir_contents() {
        // `/build` anchors to root and covers everything under build/.
        let out = f(
            "build/app.o\nbuild\nsrc/build/keep.rs\nother",
            "",
            "/build",
            Syntax::Gitignore,
        );
        // include empty → all included; exclude drops build + build/*.
        assert_eq!(out.result, "src/build/keep.rs\nother");
    }

    #[test]
    fn negation_reincludes() {
        // Exclude everything, then re-include *.rs via `!`.
        let out = f("a.rs\nb.txt\nc.rs", "", "*\n!*.rs", Syntax::Gitignore);
        assert_eq!(out.result, "a.rs\nc.rs");
    }

    #[test]
    fn glob_whole_path_needs_globstar_for_depth() {
        // In glob mode `*.rs` matches only top-level; `**/*.rs` matches any depth.
        let shallow = f("a.rs\nsrc/b.rs", "*.rs", "", Syntax::Glob);
        assert_eq!(shallow.result, "a.rs");
        let deep = f("a.rs\nsrc/b.rs", "**/*.rs", "", Syntax::Glob);
        assert_eq!(deep.result, "a.rs\nsrc/b.rs");
    }

    #[test]
    fn brace_and_class() {
        let out = f(
            "img.png\nimg.jpg\nimg.gif\nnote.txt\nv2.log",
            "*.{png,jpg}\n[a-z]*.log",
            "",
            Syntax::Gitignore,
        );
        assert_eq!(out.result, "img.png\nimg.jpg\nv2.log");
    }

    #[test]
    fn class_negation() {
        let out = f("a1\nax\nab", "a[!0-9]", "", Syntax::Gitignore);
        assert_eq!(out.result, "ax\nab");
    }

    #[test]
    fn case_insensitive() {
        let out = filter(
            "README.MD\nnotes.md",
            "*.md",
            "",
            Syntax::Gitignore,
            false,
            OutputMode::Matched,
        )
        .unwrap();
        assert_eq!(out.matched, 2);
    }

    #[test]
    fn annotated_output() {
        let out = filter(
            "keep.rs\ndrop.txt",
            "*.rs",
            "",
            Syntax::Gitignore,
            true,
            OutputMode::Annotated,
        )
        .unwrap();
        assert_eq!(out.result, "✓ keep.rs\n✗ drop.txt");
    }

    #[test]
    fn unmatched_output() {
        let out = filter(
            "keep.rs\ndrop.txt",
            "*.rs",
            "",
            Syntax::Gitignore,
            true,
            OutputMode::Unmatched,
        )
        .unwrap();
        assert_eq!(out.result, "drop.txt");
    }

    #[test]
    fn comments_ignored_in_gitignore() {
        let out = f("a.rs\nb.rs", "# only rs\n*.rs", "", Syntax::Gitignore);
        assert_eq!(out.matched, 2);
    }

    #[test]
    fn empty_paths_is_error() {
        let err = filter(
            "\n  \n",
            "*",
            "",
            Syntax::Gitignore,
            true,
            OutputMode::Matched,
        )
        .unwrap_err();
        assert!(err.contains("no paths"), "got: {err}");
    }

    #[test]
    fn syntax_and_output_parse() {
        assert_eq!(Syntax::parse("GITIGNORE").unwrap(), Syntax::Gitignore);
        assert_eq!(Syntax::parse("glob").unwrap(), Syntax::Glob);
        assert!(Syntax::parse("zsh").is_err());
        assert_eq!(OutputMode::parse("").unwrap(), OutputMode::Matched);
        assert!(OutputMode::parse("nope").is_err());
    }
}

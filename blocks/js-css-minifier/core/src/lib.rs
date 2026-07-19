//! gizza-ai/js-css-minifier core — minify JavaScript OR CSS and report the
//! before/after byte sizes. Pure-Rust, no I/O.
//!
//! The JS path reuses the proven token-aware `js-minify` core (strings, regex
//! and template literals kept verbatim, ASI-preserving, no identifier renaming),
//! so JS behavior is identical to that tool. The CSS path is a self-contained,
//! deliberately *structural-only* minifier: it collapses whitespace, strips
//! comments, and drops redundant separators, while protecting strings,
//! `url(...)`, and parenthesized expressions (so `calc(100% - 10px)` is never
//! mangled). It never rewrites values, so the minified CSS is guaranteed
//! equivalent to the input.

use gizza_ai_js_minify_core::minify_with;

/// Which language the input is minified as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Js,
    Css,
    /// Detect from the source (see [`detect_language`]).
    Auto,
}

impl Language {
    /// Parse the `language` param. Accepts `js`/`javascript`, `css`, `auto`
    /// (case-insensitive); empty/unknown falls back to `auto`.
    pub fn parse(s: &str) -> Language {
        match s.trim().to_ascii_lowercase().as_str() {
            "js" | "javascript" => Language::Js,
            "css" => Language::Css,
            _ => Language::Auto,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Language::Js => "JavaScript",
            Language::Css => "CSS",
            Language::Auto => "auto",
        }
    }
}

/// Minify `code` as `language`, optionally prepending a one-line size-report
/// comment. When `remove_comments` is true, comments are stripped — but
/// `/*! … */` / `@license` / `@preserve` banner comments are always preserved.
///
/// Returns an error on empty input.
pub fn minify(
    code: &str,
    language: Language,
    remove_comments: bool,
    report: bool,
) -> Result<String, String> {
    if code.trim().is_empty() {
        return Err("no code to minify — paste some JavaScript or CSS".into());
    }
    let lang = match language {
        Language::Auto => detect_language(code),
        other => other,
    };
    let minified = match lang {
        Language::Css => minify_css(code, remove_comments),
        // js-minify keeps license banners when its third arg is true.
        _ => minify_with(code, remove_comments, true)?,
    };
    if report {
        let head = size_report(code.len(), minified.len(), lang.label());
        // A CSS/JS block comment is valid in both languages, so the reported
        // output stays directly usable.
        Ok(format!("{head}\n{minified}"))
    } else {
        Ok(minified)
    }
}

/// Guess whether `code` is CSS or JavaScript from keyword/structure signals.
/// Realistic CSS and JS are easily separable; ambiguous input leans to CSS only
/// when clear CSS markers dominate, otherwise JS.
pub fn detect_language(code: &str) -> Language {
    let mut js: i32 = 0;
    let mut css: i32 = 0;

    for kw in [
        "function", "=>", "var ", "let ", "const ", "return", "===", "!==",
        "typeof ", "console.", "document.", "window.", "import ", "export ",
        "null", "undefined",
    ] {
        if code.contains(kw) {
            js += 1;
        }
    }
    for kw in [
        "@media", "@import", "@font-face", "@keyframes", ":root", "!important",
        "px", "rem", "em;", "rgba(", "rgb(", "#", "-webkit-", "margin", "padding",
    ] {
        if code.contains(kw) {
            css += 1;
        }
    }
    // A run of `selector { prop: value; }` declarations is a strong CSS tell.
    if code.contains('{') && code.contains(':') && code.contains(';') && !code.contains("=>") {
        css += 2;
    }

    if css > js {
        Language::Css
    } else {
        Language::Js
    }
}

/// Format the size-report banner comment, e.g.
/// `/* CSS: 1234 B -> 567 B — 54.0% smaller (saved 667 B) */`.
fn size_report(orig: usize, min: usize, lang: &str) -> String {
    let delta = orig as i64 - min as i64;
    let pct = if orig == 0 {
        0.0
    } else {
        (delta as f64 / orig as f64) * 100.0
    };
    let direction = if delta >= 0 { "smaller" } else { "larger" };
    format!(
        "/* {lang}: {orig} B -> {min} B — {pct:.1}% {direction} (saved {saved} B) */",
        pct = pct.abs(),
        saved = delta.abs()
    )
}

/// Structural-only CSS minifier. Collapses whitespace, drops redundant spaces
/// around `{ } ; , :` and selector combinators (`> + ~`) at the top level, and
/// removes the last `;` before a `}`. Strings and comments are protected;
/// spaces inside `( … )` (e.g. `calc()`) are preserved except right after `(`,
/// after `,`, and before `)`. When `remove_comments` is true, comments are
/// dropped but `/*! … */` / `@license` / `@preserve` banners are kept.
pub fn minify_css(src: &str, remove_comments: bool) -> String {
    let b: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    let mut pending_space = false; // whitespace seen, not yet emitted
    let mut after_comment = false; // last emitted token was a kept comment
    let mut brace_depth: i32 = 0;
    let mut paren_depth: i32 = 0;

    while i < b.len() {
        let c = b[i];

        // Whitespace: collapse a run, defer the decision to the next real char.
        if c.is_whitespace() {
            pending_space = true;
            i += 1;
            continue;
        }

        // String literal — emit verbatim, keep its inner whitespace.
        if c == '"' || c == '\'' {
            // A kept comment's `*/` already separates tokens — no space needed.
            if after_comment {
                pending_space = false;
            }
            after_comment = false;
            flush_space(&mut out, pending_space, c, brace_depth, paren_depth);
            pending_space = false;
            let quote = c;
            out.push(c);
            i += 1;
            while i < b.len() {
                let ch = b[i];
                out.push(ch);
                i += 1;
                if ch == '\\' && i < b.len() {
                    out.push(b[i]);
                    i += 1;
                } else if ch == quote {
                    break;
                }
            }
            continue;
        }

        // Comment.
        if c == '/' && i + 1 < b.len() && b[i + 1] == '*' {
            // Find the end of the comment.
            let start = i;
            let mut j = i + 2;
            while j + 1 < b.len() && !(b[j] == '*' && b[j + 1] == '/') {
                j += 1;
            }
            let end = if j + 1 < b.len() { j + 2 } else { b.len() };
            let text: String = b[start..end].iter().collect();
            let keep = !remove_comments || is_banner(&text);
            if keep {
                flush_space(&mut out, pending_space, '/', brace_depth, paren_depth);
                out.push_str(&text);
                pending_space = false;
                after_comment = true;
            } else {
                // A dropped comment still separates tokens.
                pending_space = true;
            }
            i = end;
            continue;
        }

        // Ordinary char — decide whether the pending space is needed.
        // A kept comment's `*/` already separates tokens — no space needed.
        if after_comment {
            pending_space = false;
        }
        after_comment = false;
        flush_space(&mut out, pending_space, c, brace_depth, paren_depth);
        pending_space = false;

        match c {
            '{' => brace_depth += 1,
            '}' => {
                brace_depth = (brace_depth - 1).max(0);
                // Drop a redundant `;` right before the closing brace.
                if out.ends_with(';') {
                    out.pop();
                }
            }
            '(' => paren_depth += 1,
            ')' => paren_depth = (paren_depth - 1).max(0),
            _ => {}
        }
        out.push(c);
        i += 1;
    }

    out.trim().to_string()
}

/// Emit a single space before `next` iff it is actually needed to keep tokens
/// apart, given the previous emitted char and the current nesting depth.
fn flush_space(out: &mut String, pending: bool, next: char, brace_depth: i32, paren_depth: i32) {
    if !pending {
        return;
    }
    let prev = match out.chars().last() {
        Some(p) => p,
        None => return, // leading whitespace — drop it
    };
    if space_removable(prev, next, brace_depth, paren_depth) {
        return;
    }
    out.push(' ');
}

/// Whether a space between `prev` and `next` can be dropped without changing
/// meaning.
fn space_removable(prev: char, next: char, brace_depth: i32, paren_depth: i32) -> bool {
    // Always-safe: adjacent to structural punctuation.
    if matches!(prev, '{' | '}' | ';' | ',' | '(' | ':') {
        return true;
    }
    if matches!(next, '{' | '}' | ';' | ',' | ')') {
        return true;
    }
    // Selector combinators — only at the top level (not inside a value/paren),
    // where `> + ~` are combinators rather than calc operators.
    if brace_depth == 0 && paren_depth == 0 && (is_combinator(prev) || is_combinator(next)) {
        return true;
    }
    false
}

fn is_combinator(c: char) -> bool {
    matches!(c, '>' | '+' | '~')
}

/// Is this block comment an important/license banner that should survive
/// comment stripping?
fn is_banner(s: &str) -> bool {
    s.starts_with("/*!") || s.contains("@license") || s.contains("@preserve")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_happy_collapses_and_reports() {
        let css = "body {\n  margin: 0;\n  color: red;\n}\n";
        let out = minify_css(css, true);
        assert_eq!(out, "body{margin:0;color:red}");
    }

    #[test]
    fn css_protects_calc_and_strings() {
        let css = ".a { width: calc(100% - 10px); content: \"a  b\"; }";
        let out = minify_css(css, true);
        assert_eq!(out, ".a{width:calc(100% - 10px);content:\"a  b\"}");
    }

    #[test]
    fn css_combinators_stripped_at_top_level() {
        let css = "a > b + c ~ d { color: blue }";
        let out = minify_css(css, true);
        assert_eq!(out, "a>b+c~d{color:blue}");
    }

    #[test]
    fn css_keeps_license_banner_drops_normal_comment() {
        let css = "/*! keep me */\n/* drop me */\na{color:red}";
        let out = minify_css(css, true);
        assert_eq!(out, "/*! keep me */a{color:red}");
    }

    #[test]
    fn minify_css_via_top_level_with_report() {
        let out = minify(".a { color: red; }", Language::Css, true, true).unwrap();
        assert!(out.starts_with("/* CSS:"), "report banner missing: {out}");
        assert!(out.contains("smaller"));
        assert!(out.trim_end().ends_with(".a{color:red}"));
    }

    #[test]
    fn minify_js_reuses_js_core() {
        let out = minify("function  f ( ) {\n return 1 ;\n}", Language::Js, true, false).unwrap();
        assert_eq!(out, "function f(){return 1;}");
    }

    #[test]
    fn detect_picks_css_and_js() {
        assert_eq!(detect_language("a{color:red;margin:0}"), Language::Css);
        assert_eq!(
            detect_language("const x = () => { return 1; };"),
            Language::Js
        );
    }

    #[test]
    fn empty_input_errors() {
        assert!(minify("   ", Language::Auto, true, true).is_err());
    }
}

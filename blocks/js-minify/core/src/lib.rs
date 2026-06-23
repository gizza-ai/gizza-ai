//! gizza-ai/js-minify core — minify JavaScript by removing unnecessary
//! whitespace, line breaks and comments while preserving behavior. Pure-Rust,
//! dependency-free.
//!
//! This is a token-aware minifier (NOT a full parser / name-mangler): it scans
//! the source into tokens (strings, template literals, line/block comments,
//! regex literals, punctuators, and other "word" runs) and re-emits them with
//! the minimum whitespace needed to keep them distinct. It never renames
//! identifiers and never reorders or drops code (other than comments) — so the
//! output is semantically identical to the input, just smaller.
//!
//! It is conservative about newlines: a line break between two tokens that could
//! be the target of automatic semicolon insertion (ASI) is preserved as a real
//! `\n` so the meaning can't change. Everywhere else, line breaks become either
//! a single space (when needed to keep two tokens apart) or nothing.

/// Minify `src` JavaScript. When `remove_comments` is true (the default), line
/// and block comments are dropped; otherwise comments are kept verbatim (a line
/// comment forces a newline, since it runs to end-of-line). Returns an error
/// only on empty input.
///
/// Even with `remove_comments`, "important" block comments are PRESERVED so you
/// never strip a license/copyright banner: a `/*! … */` comment (the de-facto
/// convention) or one containing `@license` or `@preserve`. Use [`minify_with`]
/// to turn that off.
pub fn minify(src: &str, remove_comments: bool) -> Result<String, String> {
    minify_with(src, remove_comments, true)
}

/// Like [`minify`], but `keep_license` controls whether `/*! … */` /
/// `@license` / `@preserve` banner comments survive when `remove_comments` is
/// true. With `keep_license = false`, removing comments removes ALL of them.
pub fn minify_with(src: &str, remove_comments: bool, keep_license: bool) -> Result<String, String> {
    if src.trim().is_empty() {
        return Err("no JavaScript input".into());
    }
    let toks = tokenize(src);
    Ok(render(&toks, remove_comments, keep_license))
}

/// Is this block comment an "important"/license banner that should survive a
/// comment-stripping minify?
fn is_license_comment(s: &str) -> bool {
    s.starts_with("/*!") || s.contains("@license") || s.contains("@preserve")
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    /// String / template-literal / regex literal — emitted verbatim.
    Literal(String),
    /// Line comment `// ...` (text WITHOUT trailing newline).
    LineComment(String),
    /// Block comment `/* ... */`.
    BlockComment(String),
    /// A single punctuator character of interest.
    Punct(char),
    /// `=>` arrow.
    Arrow,
    /// Any other contiguous run (identifiers, numbers, operators, keywords).
    Word(String),
}

/// A token plus whether at least one newline appeared in the whitespace that
/// *preceded* it in the source (needed to decide ASI-relevant line breaks).
#[derive(Debug, Clone)]
struct Token {
    tok: Tok,
    nl_before: bool,
}

fn is_op_char(c: char) -> bool {
    matches!(
        c,
        '+' | '-' | '*' | '/' | '%' | '=' | '<' | '>' | '!' | '&' | '|' | '^' | '~' | '?' | ':' | ','
    )
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

/// Was the previous *significant* token one after which a `/` begins a regex
/// (rather than a division operator)?
fn regex_allowed(prev: Option<&Tok>) -> bool {
    match prev {
        None => true,
        Some(Tok::Punct(c)) => !matches!(c, ')' | ']'),
        Some(Tok::Word(w)) => {
            matches!(
                w.as_str(),
                "return" | "typeof" | "instanceof" | "in" | "of" | "new" | "delete"
                    | "void" | "throw" | "do" | "else" | "yield" | "await" | "case"
            ) || w.chars().last().map(is_op_char).unwrap_or(false)
        }
        Some(Tok::Arrow) => true,
        Some(Tok::Literal(_)) => false,
        Some(_) => true,
    }
}

fn tokenize(src: &str) -> Vec<Token> {
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0;
    let len = chars.len();
    let mut out: Vec<Token> = Vec::new();
    let mut last_sig: Option<Tok> = None;
    let mut pending_nl = false;

    macro_rules! push {
        ($t:expr) => {{
            let t = $t;
            out.push(Token { tok: t.clone(), nl_before: pending_nl });
            pending_nl = false;
            last_sig = Some(t);
        }};
    }

    while i < len {
        let c = chars[i];

        if c.is_whitespace() {
            if c == '\n' {
                pending_nl = true;
            }
            i += 1;
            continue;
        }

        // line comment
        if c == '/' && i + 1 < len && chars[i + 1] == '/' {
            let start = i;
            i += 2;
            while i < len && chars[i] != '\n' {
                i += 1;
            }
            out.push(Token {
                tok: Tok::LineComment(
                    chars[start..i].iter().collect::<String>().trim_end().to_string(),
                ),
                nl_before: pending_nl,
            });
            pending_nl = false;
            continue;
        }
        // block comment
        if c == '/' && i + 1 < len && chars[i + 1] == '*' {
            let start = i;
            i += 2;
            while i + 1 < len && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i = (i + 2).min(len);
            out.push(Token {
                tok: Tok::BlockComment(chars[start..i].iter().collect()),
                nl_before: pending_nl,
            });
            pending_nl = false;
            continue;
        }

        // string literals
        if c == '"' || c == '\'' {
            let quote = c;
            let start = i;
            i += 1;
            while i < len {
                if chars[i] == '\\' {
                    i += 2;
                    continue;
                }
                if chars[i] == quote {
                    i += 1;
                    break;
                }
                if chars[i] == '\n' {
                    break;
                }
                i += 1;
            }
            push!(Tok::Literal(chars[start..i.min(len)].iter().collect()));
            continue;
        }

        // template literals
        if c == '`' {
            let start = i;
            i += 1;
            let mut depth = 0usize;
            while i < len {
                let ch = chars[i];
                if ch == '\\' {
                    i += 2;
                    continue;
                }
                if ch == '$' && i + 1 < len && chars[i + 1] == '{' {
                    depth += 1;
                    i += 2;
                    continue;
                }
                if ch == '}' && depth > 0 {
                    depth -= 1;
                    i += 1;
                    continue;
                }
                if ch == '`' && depth == 0 {
                    i += 1;
                    break;
                }
                i += 1;
            }
            push!(Tok::Literal(chars[start..i.min(len)].iter().collect()));
            continue;
        }

        // regex literal
        if c == '/' && regex_allowed(last_sig.as_ref()) {
            let start = i;
            i += 1;
            let mut in_class = false;
            let mut ok = false;
            while i < len {
                let ch = chars[i];
                if ch == '\\' {
                    i += 2;
                    continue;
                }
                if ch == '\n' {
                    break;
                }
                if ch == '[' {
                    in_class = true;
                } else if ch == ']' {
                    in_class = false;
                } else if ch == '/' && !in_class {
                    i += 1;
                    ok = true;
                    break;
                }
                i += 1;
            }
            if ok {
                while i < len && is_word_char(chars[i]) {
                    i += 1;
                }
                push!(Tok::Literal(chars[start..i].iter().collect()));
                continue;
            } else {
                i = start;
            }
        }

        // arrow
        if c == '=' && i + 1 < len && chars[i + 1] == '>' {
            i += 2;
            push!(Tok::Arrow);
            continue;
        }

        // structural punctuators
        if matches!(c, '{' | '}' | '(' | ')' | '[' | ']' | ';' | ',' | ':') {
            i += 1;
            push!(Tok::Punct(c));
            continue;
        }

        // a "word" run: identifiers/keywords/numbers, or a run of operator chars.
        let start = i;
        if is_word_char(c) {
            while i < len && is_word_char(chars[i]) {
                i += 1;
            }
        } else {
            while i < len {
                let ch = chars[i];
                if ch.is_whitespace()
                    || is_word_char(ch)
                    || matches!(
                        ch,
                        '{' | '}' | '(' | ')' | '[' | ']' | ';' | ',' | ':' | '"' | '\'' | '`'
                    )
                {
                    break;
                }
                if ch == '/' {
                    break;
                }
                if ch == '=' && i + 1 < len && chars[i + 1] == '>' {
                    break;
                }
                i += 1;
            }
        }
        if i == start {
            i += 1;
        }
        push!(Tok::Word(chars[start..i].iter().collect()));
    }

    out
}

fn last_char(t: &Tok) -> Option<char> {
    match t {
        Tok::Literal(s) | Tok::LineComment(s) | Tok::BlockComment(s) | Tok::Word(s) => {
            s.chars().last()
        }
        Tok::Punct(c) => Some(*c),
        Tok::Arrow => Some('>'),
    }
}

fn first_char(t: &Tok) -> Option<char> {
    match t {
        Tok::Literal(s) | Tok::LineComment(s) | Tok::BlockComment(s) | Tok::Word(s) => {
            s.chars().next()
        }
        Tok::Punct(c) => Some(*c),
        Tok::Arrow => Some('='),
    }
}

/// Would emitting `b` directly after `a` with NO whitespace merge them into a
/// different token (or otherwise change meaning)? If so a single space is
/// needed. This is the heart of correctness: we drop all whitespace EXCEPT
/// where two tokens would weld together.
fn needs_space(a: &Tok, b: &Tok) -> bool {
    let la = match last_char(a) {
        Some(c) => c,
        None => return false,
    };
    let fb = match first_char(b) {
        Some(c) => c,
        None => return false,
    };

    // Two word-chars would weld (e.g. `return x`, `var a`, `1 in x`, `a instanceof b`).
    if is_word_char(la) && is_word_char(fb) {
        return true;
    }
    // Operator-char pairs that would weld into a DIFFERENT token if joined:
    //  - `+ +` → `++`, `- -` → `--` (and any same-sign run, e.g. `a - -b`),
    //  - `/ /` → `//` line comment, `/ *` → `/*` block comment.
    // Other operator pairs (`=/`, `=-`, `<!`, `*-`, …) tokenize the same joined
    // as separated, so they need no space — keeping the output as small as safe.
    match (la, fb) {
        ('+', '+') | ('-', '-') => return true,
        ('/', '/') | ('/', '*') => return true,
        _ => {}
    }
    false
}

/// True when a source newline before `b` (following `a`) is ASI-significant and
/// must be preserved as a real `\n`. Conservative: keeping an extra newline can
/// never change behavior, it just costs one byte; dropping a meaningful one can.
fn needs_newline(a: &Tok, b: &Tok) -> bool {
    // A newline right after a restricted-production keyword is significant
    // (`return\nx` ≠ `return x`).
    if let Tok::Word(w) = a {
        if matches!(w.as_str(), "return" | "throw" | "break" | "continue" | "yield")
            && !matches!(b, Tok::Punct(';') | Tok::Punct('}'))
        {
            return true;
        }
    }
    // Postfix `++`/`--` on a new line after a value: `a\n++b` ≠ `a++b`.
    if let Tok::Word(w) = b {
        if matches!(w.as_str(), "++" | "--") && is_value_end(a) {
            return true;
        }
    }
    false
}

/// Is `a` the end of a value/expression (so a following newline + `++`/`--`
/// could be ASI-relevant)?
fn is_value_end(a: &Tok) -> bool {
    match a {
        Tok::Literal(_) => true,
        Tok::Punct(c) => matches!(c, ')' | ']'),
        Tok::Word(w) => {
            w.chars().next().map(is_word_char).unwrap_or(false)
                && !matches!(
                    w.as_str(),
                    "return" | "typeof" | "instanceof" | "in" | "of" | "new" | "delete" | "void"
                        | "throw" | "do" | "else" | "yield" | "await" | "case"
                )
        }
        _ => false,
    }
}

fn render(toks: &[Token], remove_comments: bool, keep_license: bool) -> String {
    let mut out = String::with_capacity(toks.len() * 2);
    // text of the last *emitted* token (we may skip comments).
    let mut prev_emitted: Option<Tok> = None;

    for cur in toks {
        let t = &cur.tok;

        match t {
            Tok::LineComment(s) => {
                if !remove_comments {
                    out.push_str(s);
                    out.push('\n');
                    prev_emitted = None; // at line start — no merge risk
                }
                continue;
            }
            Tok::BlockComment(s) => {
                let keep = !remove_comments || (keep_license && is_license_comment(s));
                if keep {
                    if let Some(p) = &prev_emitted {
                        if needs_space(p, t) {
                            out.push(' ');
                        }
                    }
                    out.push_str(s);
                    prev_emitted = Some(t.clone());
                }
                continue;
            }
            _ => {}
        }

        if let Some(p) = &prev_emitted {
            if cur.nl_before && needs_newline(p, t) {
                out.push('\n');
            } else if needs_space(p, t) {
                out.push(' ');
            }
        }

        match t {
            Tok::Literal(s) | Tok::Word(s) => out.push_str(s),
            Tok::Punct(c) => out.push(*c),
            Tok::Arrow => out.push_str("=>"),
            _ => {}
        }
        prev_emitted = Some(t.clone());
    }

    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_errors() {
        assert!(minify("", true).is_err());
        assert!(minify("   \n ", true).is_err());
    }

    #[test]
    fn collapses_whitespace_in_block() {
        let out = minify("function f() {\n  return 1;\n}", true).unwrap();
        assert_eq!(out, "function f(){return 1;}");
    }

    #[test]
    fn keeps_word_separation() {
        let out = minify("var   a   =   1 ;", true).unwrap();
        assert_eq!(out, "var a=1;");
    }

    #[test]
    fn removes_line_comment() {
        let out = minify("a(); // hi\nb();", true).unwrap();
        assert_eq!(out, "a();b();");
    }

    #[test]
    fn removes_block_comment() {
        let out = minify("var /* x */ a = 1;", true).unwrap();
        assert_eq!(out, "var a=1;");
    }

    #[test]
    fn keeps_comments_when_disabled() {
        let out = minify("var a=1;// note", false).unwrap();
        assert_eq!(out, "var a=1;// note");
    }

    #[test]
    fn keeps_license_banner_even_when_removing() {
        // `/*! ... */` survives a comment-stripping minify.
        let out = minify("/*! (c) 2026 ACME */ var a = 1;", true).unwrap();
        assert_eq!(out, "/*! (c) 2026 ACME */var a=1;");
        // `@license` survives too.
        let out2 = minify("/* @license MIT */ var b = 2;", true).unwrap();
        assert_eq!(out2, "/* @license MIT */var b=2;");
    }

    #[test]
    fn drops_license_banner_when_keep_license_off() {
        let out = minify_with("/*! (c) 2026 */ var a = 1;", true, false).unwrap();
        assert_eq!(out, "var a=1;");
    }

    #[test]
    fn string_with_spaces_untouched() {
        let out = minify(r#"var s = "a  b  c" ;"#, true).unwrap();
        assert_eq!(out, r#"var s="a  b  c";"#);
    }

    #[test]
    fn template_literal_verbatim() {
        let out = minify("var t = `a ${ b } c`;", true).unwrap();
        assert_eq!(out, "var t=`a ${ b } c`;");
    }

    #[test]
    fn regex_not_division() {
        let out = minify(r#"var r = /a\/b/g; x();"#, true).unwrap();
        assert_eq!(out, r#"var r=/a\/b/g;x();"#);
    }

    #[test]
    fn division_kept() {
        let out = minify("var x = a / b;", true).unwrap();
        assert_eq!(out, "var x=a/b;");
    }

    #[test]
    fn adjacent_operators_keep_space() {
        // `a - -b` must NOT become `a--b`.
        let out = minify("var x = a - -b;", true).unwrap();
        assert_eq!(out, "var x=a- -b;");
        let out2 = minify("var y = a + +b;", true).unwrap();
        assert_eq!(out2, "var y=a+ +b;");
    }

    #[test]
    fn return_newline_preserved() {
        // ASI: `return\n a` must keep a newline (different from `return a`).
        let out = minify("function f(){\n return\n a\n}", true).unwrap();
        assert_eq!(out, "function f(){return\na}");
    }

    #[test]
    fn return_value_inline_no_newline() {
        let out = minify("function f(){ return a; }", true).unwrap();
        assert_eq!(out, "function f(){return a;}");
    }

    #[test]
    fn arrow_minified() {
        let out = minify("const f = ( x ) => x + 1 ;", true).unwrap();
        assert_eq!(out, "const f=(x)=>x+1;");
    }

    #[test]
    fn object_literal() {
        let out = minify("var o = { a : 1 , b : 2 } ;", true).unwrap();
        assert_eq!(out, "var o={a:1,b:2};");
    }

    #[test]
    fn keyword_operand_space() {
        let out = minify("var t = typeof x ;", true).unwrap();
        assert_eq!(out, "var t=typeof x;");
    }

    #[test]
    fn for_loop() {
        let out = minify("for ( var i = 0 ; i < 3 ; i++ ) { x(); }", true).unwrap();
        assert_eq!(out, "for(var i=0;i<3;i++){x();}");
    }

    #[test]
    fn idempotent() {
        let once = minify("function f(){ return a + b; }", true).unwrap();
        let twice = minify(&once, true).unwrap();
        assert_eq!(once, twice);
    }
}

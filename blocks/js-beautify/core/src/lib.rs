//! gizza-ai/js-beautify core — re-indent and format obfuscated or minified
//! JavaScript into readable source. Pure-Rust, dependency-free.
//!
//! This is a token-aware pretty-printer (NOT a full parser): it scans the source
//! into tokens (strings, template literals, line/block comments, regex literals,
//! punctuators, and other "word" runs) and re-emits them with one statement /
//! block member per line and `indent` spaces of nesting per `{ ( [` level. It is
//! resilient to malformed input and never reorders or drops code — it only
//! changes whitespace and line breaks, so the output is semantically identical.

/// One level of indentation.
#[derive(Clone, Copy, PartialEq)]
pub enum IndentChar {
    /// `n` spaces per level.
    Space,
    /// A single tab per level (the numeric width is ignored).
    Tab,
}

/// Format `src` JavaScript with `indent` spaces per level (clamped 1..=8).
/// Returns an error only on empty input. (Convenience wrapper over
/// [`beautify_with`] using space indentation.)
pub fn beautify(src: &str, indent: usize) -> Result<String, String> {
    beautify_with(src, indent, IndentChar::Space)
}

/// Format `src` JavaScript. With [`IndentChar::Space`], each level is `indent`
/// spaces (clamped 1..=8); with [`IndentChar::Tab`], each level is one tab and
/// `indent` is ignored. Returns an error only on empty input.
pub fn beautify_with(src: &str, indent: usize, ch: IndentChar) -> Result<String, String> {
    if src.trim().is_empty() {
        return Err("no JavaScript input".into());
    }
    let unit = match ch {
        IndentChar::Tab => "\t".to_string(),
        IndentChar::Space => " ".repeat(indent.clamp(1, 8)),
    };
    let toks = tokenize(src);
    Ok(render(&toks, &unit))
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

/// Was the previous *significant* token one after which a `/` begins a regex
/// (rather than a division operator)? After a value (identifier, number, `)`,
/// `]`, string) a `/` is division; otherwise it starts a regex.
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

fn is_op_char(c: char) -> bool {
    matches!(
        c,
        '+' | '-' | '*' | '/' | '%' | '=' | '<' | '>' | '!' | '&' | '|' | '^' | '~' | '?' | ':' | ','
    )
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

fn tokenize(src: &str) -> Vec<Tok> {
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0;
    let len = chars.len();
    let mut out: Vec<Tok> = Vec::new();
    let mut last_sig: Option<Tok> = None;

    while i < len {
        let c = chars[i];

        if c.is_whitespace() {
            i += 1;
            continue;
        }

        // comments
        if c == '/' && i + 1 < len && chars[i + 1] == '/' {
            let start = i;
            i += 2;
            while i < len && chars[i] != '\n' {
                i += 1;
            }
            out.push(Tok::LineComment(
                chars[start..i].iter().collect::<String>().trim_end().to_string(),
            ));
            continue;
        }
        if c == '/' && i + 1 < len && chars[i + 1] == '*' {
            let start = i;
            i += 2;
            while i + 1 < len && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i = (i + 2).min(len);
            out.push(Tok::BlockComment(chars[start..i].iter().collect()));
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
            let t = Tok::Literal(chars[start..i.min(len)].iter().collect());
            last_sig = Some(t.clone());
            out.push(t);
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
            let t = Tok::Literal(chars[start..i.min(len)].iter().collect());
            last_sig = Some(t.clone());
            out.push(t);
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
                let t = Tok::Literal(chars[start..i].iter().collect());
                last_sig = Some(t.clone());
                out.push(t);
                continue;
            } else {
                i = start;
            }
        }

        // arrow
        if c == '=' && i + 1 < len && chars[i + 1] == '>' {
            i += 2;
            let t = Tok::Arrow;
            last_sig = Some(t.clone());
            out.push(t);
            continue;
        }

        // structural punctuators
        if matches!(c, '{' | '}' | '(' | ')' | '[' | ']' | ';' | ',' | ':') {
            i += 1;
            let t = Tok::Punct(c);
            last_sig = Some(t.clone());
            out.push(t);
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
                // always stop at `/` — it is either a comment or a regex/division
                // that the outer loop decides on the next pass.
                if ch == '/' {
                    break;
                }
                // `=>` is its own token — stop before an `=` that would start one.
                if ch == '=' && i + 1 < len && chars[i + 1] == '>' {
                    break;
                }
                i += 1;
            }
        }
        if i == start {
            i += 1;
        }
        let t = Tok::Word(chars[start..i].iter().collect());
        last_sig = Some(t.clone());
        out.push(t);
    }

    out
}

/// Tokens that, when adjacent, should NOT have a space before them.
fn no_space_before(t: &Tok) -> bool {
    matches!(
        t,
        Tok::Punct(';') | Tok::Punct(',') | Tok::Punct(')') | Tok::Punct(']') | Tok::Punct(':')
    )
}

/// Bracket-stack frame kind — distinguishes a `{}` *block* (statements, members
/// on their own lines) from a `()` or `[]` group (kept on one line).
#[derive(Clone, Copy, PartialEq)]
enum Brk {
    Block, // {
    Paren, // (
    Brack, // [
}

fn render(toks: &[Tok], unit: &str) -> String {
    let mut out = String::new();
    let mut at_line_start = true;
    let mut prev: Option<&Tok> = None;
    let mut stack: Vec<Brk> = Vec::new();
    // Count of open ternary `?` awaiting their `:` (per the current bracket
    // frame depth). A `:` is a ternary colon iff this is > 0; otherwise it's an
    // object/label colon. Reset on `{`/`}`/`;` so a `?` can't leak across.
    let mut ternary: usize = 0;

    // True when we are at "statement level" — directly inside a block or at the
    // top, not inside any (...) or [...] group. Commas + semicolons break lines
    // only at this level.
    let block_level = |stack: &[Brk]| -> bool { stack.iter().all(|b| *b == Brk::Block) };
    let depth_of = |stack: &[Brk]| -> usize { stack.iter().filter(|b| **b == Brk::Block).count() };

    fn push_newline(out: &mut String, depth: usize, unit: &str, at_line_start: &mut bool) {
        while out.ends_with(' ') {
            out.pop();
        }
        out.push('\n');
        out.push_str(&unit.repeat(depth));
        *at_line_start = true;
    }

    let mut idx = 0;
    while idx < toks.len() {
        let t = &toks[idx];
        match t {
            Tok::Punct('{') => {
                if !at_line_start && !out.ends_with(' ') && needs_space(prev, t) {
                    out.push(' ');
                }
                out.push('{');
                at_line_start = false;
                if toks.get(idx + 1) == Some(&Tok::Punct('}')) {
                    out.push('}');
                    prev = Some(&Tok::Punct('}'));
                    idx += 2;
                    continue;
                }
                stack.push(Brk::Block);
                ternary = 0;
                push_newline(&mut out, depth_of(&stack), unit, &mut at_line_start);
            }
            Tok::Punct('}') => {
                stack.pop();
                ternary = 0;
                let d = depth_of(&stack);
                if !at_line_start {
                    push_newline(&mut out, d, unit, &mut at_line_start);
                } else {
                    while out.ends_with(' ') {
                        out.pop();
                    }
                    out.push_str(&unit.repeat(d));
                }
                out.push('}');
                at_line_start = false;
                // A new statement after `}` goes on its own line; but keep
                // continuation keywords/punctuation (else, catch, while, ),],
                // ;, ,, ., a member call) on the same line.
                if block_level(&stack) {
                    let cont = match toks.get(idx + 1) {
                        Some(Tok::Word(w)) => matches!(
                            w.as_str(),
                            "else" | "catch" | "finally" | "while"
                        ) || w.starts_with('.'),
                        Some(Tok::Punct(c)) => matches!(c, ')' | ']' | ';' | ',' | '}'),
                        Some(Tok::LineComment(_)) => true,
                        _ => false,
                    };
                    if !cont && idx + 1 < toks.len() {
                        push_newline(&mut out, d, unit, &mut at_line_start);
                    }
                }
            }
            Tok::Punct('(') => {
                if !at_line_start && needs_space(prev, t) && !out.ends_with(' ') {
                    out.push(' ');
                }
                out.push('(');
                stack.push(Brk::Paren);
                at_line_start = false;
            }
            Tok::Punct(')') => {
                if stack.last() == Some(&Brk::Paren) {
                    stack.pop();
                }
                while out.ends_with(' ') {
                    out.pop();
                }
                out.push(')');
                at_line_start = false;
            }
            Tok::Punct('[') => {
                if !at_line_start && needs_space(prev, t) && !out.ends_with(' ') {
                    out.push(' ');
                }
                out.push('[');
                stack.push(Brk::Brack);
                at_line_start = false;
            }
            Tok::Punct(']') => {
                if stack.last() == Some(&Brk::Brack) {
                    stack.pop();
                }
                while out.ends_with(' ') {
                    out.pop();
                }
                out.push(']');
                at_line_start = false;
            }
            Tok::Punct(';') => {
                while out.ends_with(' ') {
                    out.pop();
                }
                out.push(';');
                at_line_start = false;
                ternary = 0;
                if block_level(&stack) {
                    let nxt = toks.get(idx + 1);
                    // keep a trailing line comment on this line
                    if matches!(nxt, Some(Tok::LineComment(_))) {
                        out.push(' ');
                    } else if nxt.is_some() && nxt != Some(&Tok::Punct('}')) {
                        push_newline(&mut out, depth_of(&stack), unit, &mut at_line_start);
                    }
                } else {
                    out.push(' ');
                }
            }
            Tok::Punct(',') => {
                while out.ends_with(' ') {
                    out.pop();
                }
                out.push(',');
                at_line_start = false;
                // object members (comma directly inside a `{}` block) go on their
                // own lines; commas inside (...) or [...] stay inline.
                if block_level(&stack) && !stack.is_empty() {
                    let nxt = toks.get(idx + 1);
                    if nxt.is_some() && nxt != Some(&Tok::Punct('}')) {
                        push_newline(&mut out, depth_of(&stack), unit, &mut at_line_start);
                    }
                } else {
                    out.push(' ');
                }
            }
            Tok::Punct(':') => {
                while out.ends_with(' ') {
                    out.pop();
                }
                if ternary > 0 {
                    // ternary colon: ` : `
                    ternary -= 1;
                    out.push(' ');
                    out.push(':');
                    out.push(' ');
                } else {
                    // object key / label colon: `: `
                    out.push(':');
                    out.push(' ');
                }
                at_line_start = false;
            }
            Tok::Punct(c) => {
                out.push(*c);
                at_line_start = false;
            }
            Tok::Arrow => {
                if !at_line_start && !out.ends_with(' ') {
                    out.push(' ');
                }
                out.push_str("=>");
                out.push(' ');
                at_line_start = false;
            }
            Tok::LineComment(s) => {
                if !at_line_start && !out.ends_with(' ') {
                    out.push(' ');
                }
                out.push_str(s);
                push_newline(&mut out, depth_of(&stack), unit, &mut at_line_start);
            }
            Tok::BlockComment(s) => {
                if !at_line_start && needs_space(prev, t) && !out.ends_with(' ') {
                    out.push(' ');
                }
                out.push_str(s);
                at_line_start = false;
            }
            Tok::Literal(s) | Tok::Word(s) => {
                if !at_line_start && needs_space(prev, t) && !out.ends_with(' ') {
                    out.push(' ');
                }
                out.push_str(s);
                at_line_start = false;
                // A lone `?` opens a ternary (its `:` pairs with it). Exclude
                // optional chaining `?.` and nullish `??`.
                if let Tok::Word(w) = t {
                    if w == "?" {
                        ternary += 1;
                    }
                }
            }
        }
        prev = Some(t);
        idx += 1;
    }

    while out.ends_with(['\n', ' ', '\t']) {
        out.pop();
    }
    out
}

/// Whether a space should separate `prev` and `cur`.
fn needs_space(prev: Option<&Tok>, cur: &Tok) -> bool {
    let p = match prev {
        None => return false,
        Some(p) => p,
    };
    if no_space_before(cur) {
        return false;
    }
    match (p, cur) {
        (Tok::Punct('(') | Tok::Punct('['), _) => false,
        (Tok::Word(w), Tok::Punct('(')) => {
            // keyword ( → space; operator-ending word (=, &&, return-like) → space;
            // a plain identifier/number ( → call, no space.
            matches!(
                w.as_str(),
                "if" | "for" | "while" | "switch" | "catch" | "return" | "do" | "function" | "in"
                    | "of" | "else" | "typeof" | "new" | "delete" | "void" | "yield" | "await" | "case"
            ) || w.chars().last().map(is_op_char).unwrap_or(false)
        }
        (Tok::Word(w), Tok::Punct('[')) => matches!(
            w.as_str(),
            "return" | "typeof" | "in" | "of" | "new" | "delete" | "void" | "yield" | "await" | "case"
        ),
        (Tok::Literal(_), Tok::Punct('(')) | (Tok::Literal(_), Tok::Punct('[')) => false,
        (Tok::Punct(')'), Tok::Punct('(')) | (Tok::Punct(']'), Tok::Punct('(')) => false,
        (Tok::Punct(')'), Tok::Punct('[')) | (Tok::Punct(']'), Tok::Punct('[')) => false,
        (_, Tok::Word(w)) if w.starts_with('.') => false,
        (Tok::Word(w), _) if w.ends_with('.') => false,
        // `++` / `--` bind tight to their operand (i++, --i).
        (Tok::Word(w), _) if matches!(w.as_str(), "++" | "--") => false,
        (_, Tok::Word(w)) if matches!(w.as_str(), "++" | "--") => false,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_errors() {
        assert!(beautify("", 2).is_err());
        assert!(beautify("   \n ", 2).is_err());
    }

    #[test]
    fn simple_block_indents() {
        let out = beautify("function f(){return 1;}", 2).unwrap();
        assert_eq!(out, "function f() {\n  return 1;\n}");
    }

    #[test]
    fn nested_blocks() {
        let out = beautify("if(a){if(b){c();}}", 2).unwrap();
        assert_eq!(out, "if (a) {\n  if (b) {\n    c();\n  }\n}");
    }

    #[test]
    fn indent_width_respected() {
        let out = beautify("function f(){return 1;}", 4).unwrap();
        assert_eq!(out, "function f() {\n    return 1;\n}");
    }

    #[test]
    fn string_with_braces_untouched() {
        let out = beautify(r#"var s="{a:1}";f();"#, 2).unwrap();
        assert_eq!(out, "var s = \"{a:1}\";\nf();");
    }

    #[test]
    fn for_loop_semicolons_inline() {
        let out = beautify("for(var i=0;i<3;i++){x();}", 2).unwrap();
        assert_eq!(out, "for (var i = 0; i < 3; i++) {\n  x();\n}");
    }

    #[test]
    fn empty_block_stays_inline() {
        let out = beautify("function f(){}", 2).unwrap();
        assert_eq!(out, "function f() {}");
    }

    #[test]
    fn line_comment_breaks() {
        let out = beautify("a();//hi\nb();", 2).unwrap();
        assert_eq!(out, "a(); //hi\nb();");
    }

    #[test]
    fn regex_not_split_as_division() {
        let out = beautify(r#"var r=/a\/b/g;x();"#, 2).unwrap();
        assert_eq!(out, "var r = /a\\/b/g;\nx();");
    }

    #[test]
    fn template_literal_verbatim() {
        let out = beautify("var t=`a${b+1}c`;", 2).unwrap();
        assert_eq!(out, "var t = `a${b+1}c`;");
    }

    #[test]
    fn object_literal_keys() {
        let out = beautify("var o={a:1,b:2};", 2).unwrap();
        assert_eq!(out, "var o = {\n  a: 1,\n  b: 2\n};");
    }

    #[test]
    fn arrow_function() {
        let out = beautify("const f=(x)=>x+1;", 2).unwrap();
        assert_eq!(out, "const f = (x) => x + 1;");
    }

    #[test]
    fn member_access_tight() {
        let out = beautify("a.b.c();", 2).unwrap();
        assert_eq!(out, "a.b.c();");
    }

    #[test]
    fn division_not_regex() {
        let out = beautify("var x=a/b;", 2).unwrap();
        assert_eq!(out, "var x = a / b;");
    }

    #[test]
    fn ternary_colon_spaced() {
        let out = beautify("var x=a?1:2;", 2).unwrap();
        assert_eq!(out, "var x = a ? 1 : 2;");
    }

    #[test]
    fn object_colon_not_ternary() {
        let out = beautify("var o={k:1};", 2).unwrap();
        assert_eq!(out, "var o = {\n  k: 1\n};");
    }

    #[test]
    fn statement_after_brace_breaks() {
        let out = beautify("if(a){b();}c();", 2).unwrap();
        assert_eq!(out, "if (a) {\n  b();\n}\nc();");
    }

    #[test]
    fn tab_indentation() {
        let out = beautify_with("function f(){return 1;}", 2, IndentChar::Tab).unwrap();
        assert_eq!(out, "function f() {\n\treturn 1;\n}");
    }

    #[test]
    fn else_stays_with_brace() {
        let out = beautify("if(a){b();}else{c();}", 2).unwrap();
        assert_eq!(out, "if (a) {\n  b();\n} else {\n  c();\n}");
    }
}

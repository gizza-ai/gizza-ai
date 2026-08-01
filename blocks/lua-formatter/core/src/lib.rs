//! lua-formatter core — a dependency-free Lua source re-indenter / beautifier.
//!
//! A forgiving Lua tokenizer + line-based reformatter. It splits the source into
//! tokens (short/long strings, line/long comments, names, numbers, operators and
//! punctuation), then rebuilds the source line by line: leading indentation is
//! recomputed from Lua block structure (`function`/`then`/`do`/`repeat` and the
//! bracket family open a level; `end`/`until` and closers close one), intra-line
//! whitespace is collapsed to single spaces where the author had a space, commas
//! are normalized (no space before, one space after), runs of blank lines are
//! collapsed, and — optionally — string quotes are normalized to a single style.
//!
//! It is deliberately conservative: it never wraps long lines, never reorders or
//! renames anything, and never touches the interior of a long string / long
//! comment. That keeps it safe on any Lua dialect (5.1–5.4, LuaJIT, Luau); a full
//! AST reprint (line-wrapping at a column width, call-parenthesis rules) needs a
//! real parser and is out of scope.

/// Indentation style: a fixed number of spaces per level, or one tab per level.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Indent {
    Spaces(usize),
    Tab,
}

impl Indent {
    fn unit(&self) -> String {
        match *self {
            Indent::Spaces(n) => " ".repeat(n.min(8)),
            Indent::Tab => "\t".to_string(),
        }
    }
}

/// How string literals should be quoted in the output.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum QuoteStyle {
    /// Leave each short string's quote character untouched.
    Preserve,
    /// Normalize short strings to double quotes (`"…"`), re-escaping as needed.
    Double,
    /// Normalize short strings to single quotes (`'…'`), re-escaping as needed.
    Single,
}

impl QuoteStyle {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "preserve" | "keep" | "original" => Ok(QuoteStyle::Preserve),
            "double" | "double_quotes" => Ok(QuoteStyle::Double),
            "single" | "single_quotes" => Ok(QuoteStyle::Single),
            other => Err(format!(
                "invalid quote_style '{other}' (expected preserve, double, or single)"
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Kind {
    Newline,
    Name,          // identifier or keyword
    ShortString,   // "…" or '…' incl. delimiters
    LongString,    // [[ … ]] / [=[ … ]=] incl. delimiters (may span lines)
    LineComment,   // -- … (to end of line)
    LongComment,   // --[[ … ]] (may span lines)
    Open,          // ( [ {
    Close,         // ) ] }
    Other,         // number, operator, punctuation char
}

#[derive(Debug, Clone)]
struct Tok {
    kind: Kind,
    text: String,
    space_before: bool,
}

fn is_name_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}
fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// If `b[i]` begins a Lua long bracket (`[`, then zero+ `=`, then `[`), return
/// its level (count of `=`); otherwise `None`.
fn long_bracket_level(b: &[char], i: usize) -> Option<usize> {
    if b.get(i) != Some(&'[') {
        return None;
    }
    let mut j = i + 1;
    let mut level = 0;
    while b.get(j) == Some(&'=') {
        level += 1;
        j += 1;
    }
    if b.get(j) == Some(&'[') {
        Some(level)
    } else {
        None
    }
}

/// Consume a long bracket body starting at `i` (which points at the opening `[`)
/// with the given `level`, returning the end index (exclusive). Forgiving:
/// an unterminated bracket runs to EOF.
fn scan_long_bracket(b: &[char], i: usize, level: usize) -> usize {
    let n = b.len();
    let mut j = i + 2 + level; // skip [ =… [
    let close: String = std::iter::once(']')
        .chain(std::iter::repeat('=').take(level))
        .chain(std::iter::once(']'))
        .collect();
    let close: Vec<char> = close.chars().collect();
    while j < n {
        if b[j] == ']' && b[j..].starts_with(close.as_slice()) {
            return j + close.len();
        }
        j += 1;
    }
    n
}

/// Tokenize Lua source. Forgiving: unterminated strings/comments run to EOF.
fn tokenize(src: &str) -> Vec<Tok> {
    let b: Vec<char> = src.chars().collect();
    let n = b.len();
    let mut i = 0;
    let mut out: Vec<Tok> = Vec::new();
    let mut pending_space = false;

    macro_rules! push {
        ($kind:expr, $text:expr) => {{
            out.push(Tok {
                kind: $kind,
                text: $text,
                space_before: pending_space,
            });
            pending_space = false;
        }};
    }

    while i < n {
        let c = b[i];
        // newline (normalize \r\n / \r to a single Newline token)
        if c == '\n' || c == '\r' {
            if c == '\r' && b.get(i + 1) == Some(&'\n') {
                i += 1;
            }
            push!(Kind::Newline, "\n".to_string());
            i += 1;
            continue;
        }
        // other whitespace
        if c == ' ' || c == '\t' {
            pending_space = true;
            i += 1;
            continue;
        }
        // comments: -- … or --[[ … ]]
        if c == '-' && b.get(i + 1) == Some(&'-') {
            if let Some(level) = long_bracket_level(&b, i + 2) {
                let end = scan_long_bracket(&b, i + 2, level);
                push!(Kind::LongComment, b[i..end].iter().collect());
                i = end;
                continue;
            }
            let start = i;
            while i < n && b[i] != '\n' {
                i += 1;
            }
            push!(Kind::LineComment, b[start..i].iter().collect());
            continue;
        }
        // long string [[ … ]] / [=[ … ]=]
        if c == '[' {
            if let Some(level) = long_bracket_level(&b, i) {
                let end = scan_long_bracket(&b, i, level);
                push!(Kind::LongString, b[i..end].iter().collect());
                i = end;
                continue;
            }
        }
        // short string "…" / '…'
        if c == '"' || c == '\'' {
            let quote = c;
            let start = i;
            i += 1;
            while i < n {
                if b[i] == '\\' {
                    i += 2;
                    continue;
                }
                if b[i] == quote {
                    i += 1;
                    break;
                }
                if b[i] == '\n' {
                    break; // unterminated — stop at line end, stay forgiving
                }
                i += 1;
            }
            push!(Kind::ShortString, b[start..i.min(n)].iter().collect());
            continue;
        }
        // brackets
        if c == '(' || c == '[' || c == '{' {
            push!(Kind::Open, c.to_string());
            i += 1;
            continue;
        }
        if c == ')' || c == ']' || c == '}' {
            push!(Kind::Close, c.to_string());
            i += 1;
            continue;
        }
        // name / keyword
        if is_name_start(c) {
            let start = i;
            while i < n && is_name_char(b[i]) {
                i += 1;
            }
            push!(Kind::Name, b[start..i].iter().collect());
            continue;
        }
        // anything else — a single char (number digit, operator, punctuation).
        push!(Kind::Other, c.to_string());
        i += 1;
    }
    out
}

const OPENER_WORDS: &[&str] = &["function", "then", "do", "repeat"];
const CLOSER_WORDS: &[&str] = &["end", "until"];

fn is_opener(t: &Tok) -> bool {
    matches!(t.kind, Kind::Open) || (t.kind == Kind::Name && OPENER_WORDS.contains(&t.text.as_str()))
}
fn is_closer(t: &Tok) -> bool {
    matches!(t.kind, Kind::Close)
        || (t.kind == Kind::Name && CLOSER_WORDS.contains(&t.text.as_str()))
}

/// Convert one short-string token to the target quote style. Returns the token
/// text unchanged when it already uses the target quote or the style is Preserve.
fn requote(text: &str, style: QuoteStyle) -> String {
    let target = match style {
        QuoteStyle::Preserve => return text.to_string(),
        QuoteStyle::Double => '"',
        QuoteStyle::Single => '\'',
    };
    let chars: Vec<char> = text.chars().collect();
    if chars.len() < 2 {
        return text.to_string();
    }
    let src_q = chars[0];
    if src_q != '"' && src_q != '\'' {
        return text.to_string();
    }
    // Only convert when the string is properly closed with the same quote.
    if *chars.last().unwrap() != src_q || chars.len() < 2 {
        return text.to_string();
    }
    if src_q == target {
        return text.to_string();
    }
    let inner = &chars[1..chars.len() - 1];
    let mut out = String::new();
    out.push(target);
    let mut k = 0;
    while k < inner.len() {
        let ch = inner[k];
        if ch == '\\' {
            if let Some(&next) = inner.get(k + 1) {
                if next == src_q {
                    // `\'`→`'` when leaving single, `\"`→`"` when leaving double.
                    out.push(next);
                } else {
                    out.push('\\');
                    out.push(next);
                }
                k += 2;
                continue;
            }
            out.push('\\');
            k += 1;
            continue;
        }
        if ch == target {
            out.push('\\'); // escape the new delimiter
        }
        out.push(ch);
        k += 1;
    }
    out.push(target);
    out
}

/// Rebuild one logical line's text (no leading indentation) from its tokens.
fn render_line(toks: &[Tok], style: QuoteStyle) -> String {
    let mut out = String::new();
    let mut prev_comma = false;
    for (idx, t) in toks.iter().enumerate() {
        let text = if t.kind == Kind::ShortString {
            requote(&t.text, style)
        } else {
            t.text.clone()
        };
        let is_comma_semi = t.kind == Kind::Other && (t.text == "," || t.text == ";");
        let sep = if idx == 0 {
            false
        } else if is_comma_semi {
            false // never a space before , or ;
        } else if prev_comma && !matches!(t.kind, Kind::Close) {
            true // always a space after , or ; (except before a closer)
        } else {
            t.space_before
        };
        if sep {
            out.push(' ');
        }
        out.push_str(&text);
        prev_comma = is_comma_semi;
    }
    out
}

/// Reformat Lua `src` with the given `indent` and quote `style`.
pub fn format(src: &str, indent: Indent, style: QuoteStyle) -> Result<String, String> {
    if src.trim().is_empty() {
        return Err("empty Lua input".to_string());
    }
    let unit = indent.unit();
    let toks = tokenize(src);

    // Split into logical lines on Newline tokens (newlines *inside* long
    // strings/comments stay embedded in that single token and are left verbatim).
    let mut lines: Vec<Vec<Tok>> = Vec::new();
    let mut cur: Vec<Tok> = Vec::new();
    for t in toks {
        if t.kind == Kind::Newline {
            lines.push(std::mem::take(&mut cur));
        } else {
            cur.push(t);
        }
    }
    lines.push(cur);

    let mut out_lines: Vec<String> = Vec::new();
    let mut depth: usize = 0;
    for line in &lines {
        if line.is_empty() {
            out_lines.push(String::new()); // blank line placeholder
            continue;
        }
        // else / elseif: dedent this line one level, leave running depth alone.
        let first_word = if line[0].kind == Kind::Name {
            line[0].text.as_str()
        } else {
            ""
        };
        if first_word == "else" || first_word == "elseif" {
            let ind = depth.saturating_sub(1);
            out_lines.push(format!("{}{}", unit.repeat(ind), render_line(line, style)));
            continue;
        }
        // A line that begins with closers dedents itself by one level (a leading
        // run — `end)` — still dedents just one, matching the one-level bump the
        // matching opener line added).
        let starts_with_closer = is_closer(&line[0]);
        let this_depth = if starts_with_closer {
            depth.saturating_sub(1)
        } else {
            depth
        };
        out_lines.push(format!(
            "{}{}",
            unit.repeat(this_depth),
            render_line(line, style)
        ));

        // Running depth change, capped to one level per line so that a line
        // opening several blocks at once (`run(function()`) and the line that
        // closes them (`end)`) stay symmetric.
        let mut opens = 0i64;
        let mut closes = 0i64;
        for t in line {
            if is_opener(t) {
                opens += 1;
            } else if is_closer(t) {
                closes += 1;
            }
        }
        let net = (opens - closes).clamp(-1, 1);
        if net >= 0 {
            depth += net as usize;
        } else {
            depth = depth.saturating_sub(1);
        }
    }

    // Collapse runs of blank lines to at most one; trim leading/trailing blanks.
    let mut result: Vec<String> = Vec::new();
    let mut blank_run = 0;
    for l in out_lines {
        if l.is_empty() {
            blank_run += 1;
            if blank_run <= 1 {
                result.push(String::new());
            }
        } else {
            blank_run = 0;
            result.push(l);
        }
    }
    while result.first().map(|s| s.is_empty()).unwrap_or(false) {
        result.remove(0);
    }
    while result.last().map(|s| s.is_empty()).unwrap_or(false) {
        result.pop();
    }
    if result.is_empty() {
        return Err("empty Lua input".to_string());
    }
    Ok(result.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(src: &str) -> String {
        format(src, Indent::Spaces(2), QuoteStyle::Preserve).unwrap()
    }

    #[test]
    fn reindents_if_block() {
        let out = fmt("if x then\nprint(1)\nend");
        assert_eq!(out, "if x then\n  print(1)\nend");
    }

    #[test]
    fn nested_function_and_loop() {
        let src = "local function f(t)\nfor i=1,#t do\nif t[i] then\nreturn i\nend\nend\nend";
        let out = fmt(src);
        let expected = "local function f(t)\n  for i=1, #t do\n    if t[i] then\n      return i\n    end\n  end\nend";
        assert_eq!(out, expected, "got:\n{out}");
    }

    #[test]
    fn else_and_elseif_dedent() {
        let src = "if a then\nx()\nelseif b then\ny()\nelse\nz()\nend";
        let out = fmt(src);
        let expected = "if a then\n  x()\nelseif b then\n  y()\nelse\n  z()\nend";
        assert_eq!(out, expected, "got:\n{out}");
    }

    #[test]
    fn repeat_until_block() {
        let out = fmt("repeat\ntick()\nuntil done");
        assert_eq!(out, "repeat\n  tick()\nuntil done");
    }

    #[test]
    fn multiline_table_indents() {
        let out = fmt("local t = {\na = 1,\nb = 2,\n}");
        assert_eq!(out, "local t = {\n  a = 1,\n  b = 2,\n}");
    }

    #[test]
    fn tab_indent() {
        let out = format("do\nx()\nend", Indent::Tab, QuoteStyle::Preserve).unwrap();
        assert_eq!(out, "do\n\tx()\nend");
    }

    #[test]
    fn indent_width_four() {
        let out = format("if x then\ny()\nend", Indent::Spaces(4), QuoteStyle::Preserve).unwrap();
        assert_eq!(out, "if x then\n    y()\nend");
    }

    #[test]
    fn comma_spacing_normalized() {
        let out = fmt("f(a ,b,  c)");
        assert_eq!(out, "f(a, b, c)");
    }

    #[test]
    fn collapses_blank_lines() {
        let out = fmt("a()\n\n\n\nb()");
        assert_eq!(out, "a()\n\nb()");
    }

    #[test]
    fn long_string_interior_untouched() {
        let out = fmt("local s = [[\n  keep\n    me\n]]\nprint(s)");
        assert_eq!(out, "local s = [[\n  keep\n    me\n]]\nprint(s)");
    }

    #[test]
    fn line_comment_preserved() {
        let out = fmt("do\n-- a note\nx()\nend");
        assert_eq!(out, "do\n  -- a note\n  x()\nend");
    }

    #[test]
    fn keyword_inside_string_not_counted() {
        let out = fmt("print(\"end of file\")\nreturn 1");
        assert_eq!(out, "print(\"end of file\")\nreturn 1");
    }

    #[test]
    fn requote_to_double() {
        let out = format("local s = 'hi'", Indent::Spaces(2), QuoteStyle::Double).unwrap();
        assert_eq!(out, "local s = \"hi\"");
    }

    #[test]
    fn requote_reescapes() {
        // 'say "hi"' → "say \"hi\""
        let out = format("x = 'say \"hi\"'", Indent::Spaces(2), QuoteStyle::Double).unwrap();
        assert_eq!(out, "x = \"say \\\"hi\\\"\"");
    }

    #[test]
    fn requote_unescapes_old_quote() {
        // "it\'s" → 'it\'s' would be wrong; leaving to single: content `it's` needs escape.
        // Convert double→single: "don't" → 'don\'t'
        let out = format("x = \"don't\"", Indent::Spaces(2), QuoteStyle::Single).unwrap();
        assert_eq!(out, "x = 'don\\'t'");
    }

    #[test]
    fn requote_preserve_leaves_alone() {
        let out = format("a = 'x'\nb = \"y\"", Indent::Spaces(2), QuoteStyle::Preserve).unwrap();
        assert_eq!(out, "a = 'x'\nb = \"y\"");
    }

    #[test]
    fn empty_is_error() {
        assert!(format("   \n\n", Indent::Spaces(2), QuoteStyle::Preserve).is_err());
    }

    #[test]
    fn quote_style_parse() {
        assert!(QuoteStyle::parse("preserve").is_ok());
        assert!(QuoteStyle::parse("DOUBLE").is_ok());
        assert!(QuoteStyle::parse("single").is_ok());
        assert!(QuoteStyle::parse("smart").is_err());
    }

    #[test]
    fn closing_paren_and_end_dedent_together() {
        // call taking a function literal
        let src = "run(function()\ndo_it()\nend)";
        let out = fmt(src);
        assert_eq!(out, "run(function()\n  do_it()\nend)", "got:\n{out}");
    }
}

//! gizza-ai/indent-block-text core — add a configurable number of spaces or
//! tabs, or a fixed custom prefix, to the start of every line; also remove a
//! fixed amount of indentation (outdent) or strip the common leading
//! indentation shared by all lines (dedent). Pure-Rust, no I/O.

use serde::Serialize;

/// The maximum number of indent units (or custom-prefix repeats) per line.
pub const MAX_COUNT: i64 = 200;
/// The maximum length, in characters, of a custom prefix string.
pub const MAX_PREFIX_CHARS: usize = 100;

/// What to do with each line's leading indentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Add the indent unit to the start of the line.
    Indent,
    /// Remove up to `count` leading copies of the indent unit.
    Outdent,
    /// Remove the longest leading whitespace shared by every non-blank line.
    Dedent,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Mode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "indent" | "add" | "" => Ok(Mode::Indent),
            "outdent" | "remove" | "unindent" => Ok(Mode::Outdent),
            "dedent" | "auto" => Ok(Mode::Dedent),
            other => Err(format!(
                "unknown mode '{other}' (use indent, outdent, or dedent)"
            )),
        }
    }
}

/// Which character (or string) makes up one unit of indentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    /// One space per unit.
    Spaces,
    /// One tab per unit.
    Tabs,
    /// One copy of the caller-supplied `prefix` per unit.
    Custom,
}

impl Style {
    pub fn parse(s: &str) -> Result<Style, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "spaces" | "space" | "" => Ok(Style::Spaces),
            "tabs" | "tab" => Ok(Style::Tabs),
            "custom" | "prefix" => Ok(Style::Custom),
            other => Err(format!(
                "unknown style '{other}' (use spaces, tabs, or custom)"
            )),
        }
    }
}

/// Which lines the change applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Every line.
    All,
    /// Only the very first line of the text.
    FirstLine,
    /// Every line except the first — a hanging indent.
    Hanging,
    /// The first line of each paragraph (blocks separated by blank lines).
    ParagraphStarts,
}

impl Scope {
    pub fn parse(s: &str) -> Result<Scope, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "all" | "" => Ok(Scope::All),
            "first-line" | "first_line" | "first" => Ok(Scope::FirstLine),
            "hanging" => Ok(Scope::Hanging),
            "paragraph-starts" | "paragraph_starts" | "paragraph" | "paragraphs" => {
                Ok(Scope::ParagraphStarts)
            }
            other => Err(format!(
                "unknown lines '{other}' (use all, first-line, hanging, or paragraph-starts)"
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Output {
    /// Total number of lines in the input.
    pub total: usize,
    /// How many lines were actually changed.
    pub changed: usize,
    /// The one indent unit that was applied (empty for outdent/dedent).
    pub unit: String,
    /// The re-indented text.
    pub result: String,
}

/// True for lines that hold nothing but whitespace (`\r` included, so CRLF
/// input is treated the same as LF).
fn is_blank(line: &str) -> bool {
    line.trim().is_empty()
}

/// Split into lines, keeping track of a trailing newline so it survives the
/// round trip (important for code blocks, which conventionally end with one).
fn split_lines(text: &str) -> (Vec<&str>, bool) {
    if text.is_empty() {
        return (Vec::new(), false);
    }
    let mut segs: Vec<&str> = text.split('\n').collect();
    let trailing = segs.len() > 1 && segs.last() == Some(&"");
    if trailing {
        segs.pop();
    }
    (segs, trailing)
}

/// Decide, per line index, whether the scope selects it.
fn selected(scope: Scope, lines: &[&str], i: usize) -> bool {
    match scope {
        Scope::All => true,
        Scope::FirstLine => i == 0,
        Scope::Hanging => i > 0,
        // A paragraph start is the first line of the text, or any line whose
        // predecessor is blank.
        Scope::ParagraphStarts => i == 0 || is_blank(lines[i - 1]),
    }
}

/// Strip up to `count` leading copies of `unit` from `line`.
fn strip_units(line: &str, unit: &str, count: i64) -> String {
    let mut rest = line;
    let mut removed = 0;
    while removed < count {
        match rest.strip_prefix(unit) {
            Some(r) => {
                rest = r;
                removed += 1;
            }
            None => break,
        }
    }
    rest.to_string()
}

/// The longest leading run of whitespace shared by every selected non-blank
/// line — the classic `textwrap.dedent`.
fn common_indent(lines: &[&str], scope: Scope) -> String {
    let mut common: Option<&str> = None;
    for (i, line) in lines.iter().enumerate() {
        if !selected(scope, lines, i) || is_blank(line) {
            continue;
        }
        let lead = &line[..line.len() - line.trim_start().len()];
        common = Some(match common {
            None => lead,
            Some(prev) => {
                let n = prev
                    .bytes()
                    .zip(lead.bytes())
                    .take_while(|(a, b)| a == b)
                    .count();
                &prev[..n]
            }
        });
        if common == Some("") {
            break;
        }
    }
    common.unwrap_or("").to_string()
}

/// Re-indent `text`.
///
/// * `mode` — add an indent, remove a fixed one, or auto-dedent.
/// * `style` — spaces, tabs, or repeats of `prefix`.
/// * `count` — how many units (ignored by `dedent`).
/// * `prefix` — the unit used when `style` is `custom`.
/// * `scope` — which lines are touched.
/// * `skip_blank_lines` — leave whitespace-only lines exactly as they are
///   (avoids adding trailing whitespace to empty lines).
pub fn indent(
    text: &str,
    mode: Mode,
    style: Style,
    count: i64,
    prefix: &str,
    scope: Scope,
    skip_blank_lines: bool,
) -> Result<Output, String> {
    if count < 0 {
        return Err("count must be zero or more".into());
    }
    if count > MAX_COUNT {
        return Err(format!("count must be {MAX_COUNT} or less"));
    }
    if prefix.chars().count() > MAX_PREFIX_CHARS {
        return Err(format!(
            "prefix must be {MAX_PREFIX_CHARS} characters or fewer"
        ));
    }
    if style == Style::Custom && prefix.is_empty() && mode != Mode::Dedent {
        return Err("style=custom needs a prefix (e.g. \"> \" or \"# \")".into());
    }

    let unit_char = match style {
        Style::Spaces => " ".to_string(),
        Style::Tabs => "\t".to_string(),
        Style::Custom => prefix.to_string(),
    };
    let (lines, trailing_newline) = split_lines(text);

    // The full per-line unit for indent mode: `count` copies of one unit.
    let full_unit = unit_char.repeat(count.max(0) as usize);
    let dedent_unit = if mode == Mode::Dedent {
        common_indent(&lines, scope)
    } else {
        String::new()
    };

    let mut changed = 0usize;
    let mut out = String::with_capacity(text.len() + lines.len() * full_unit.len());
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let touch = selected(scope, &lines, i) && !(skip_blank_lines && is_blank(line));
        if !touch {
            out.push_str(line);
            continue;
        }
        let new_line = match mode {
            Mode::Indent => {
                if full_unit.is_empty() {
                    line.to_string()
                } else {
                    format!("{full_unit}{line}")
                }
            }
            Mode::Outdent => strip_units(line, &unit_char, count),
            Mode::Dedent => match line.strip_prefix(dedent_unit.as_str()) {
                Some(r) => r.to_string(),
                None => line.trim_start().to_string(),
            },
        };
        if new_line != *line {
            changed += 1;
        }
        out.push_str(&new_line);
    }
    if trailing_newline {
        out.push('\n');
    }

    Ok(Output {
        total: lines.len(),
        changed,
        unit: match mode {
            Mode::Indent => full_unit,
            Mode::Dedent => dedent_unit,
            Mode::Outdent => String::new(),
        },
        result: out,
    })
}

/// Human-readable rendering (used by the page) — just the re-indented text.
#[allow(clippy::too_many_arguments)]
pub fn render(
    text: &str,
    mode: Mode,
    style: Style,
    count: i64,
    prefix: &str,
    scope: Scope,
    skip_blank_lines: bool,
) -> Result<String, String> {
    Ok(indent(text, mode, style, count, prefix, scope, skip_blank_lines)?.result)
}

/// Full surface entry point: parse string enum options, apply defaults, and return
/// the transformed text only.
#[allow(clippy::too_many_arguments)]
pub fn run_with_options(
    text: &str,
    mode: &str,
    style: &str,
    count: i64,
    prefix: &str,
    lines: &str,
    skip_blank_lines: bool,
) -> Result<String, String> {
    render(
        text,
        Mode::parse(mode)?,
        Style::parse(style)?,
        count,
        prefix,
        Scope::parse(lines)?,
        skip_blank_lines,
    )
}

/// Backwards-compatible default: indent every non-blank line by four spaces.
pub fn run(input: &str) -> Result<String, String> {
    run_with_options(input, "indent", "spaces", 4, "", "all", true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spaces(text: &str, n: i64) -> String {
        render(text, Mode::Indent, Style::Spaces, n, "", Scope::All, true).unwrap()
    }

    #[test]
    fn indents_every_line_by_four_spaces() {
        let o = indent(
            "a\nb\nc",
            Mode::Indent,
            Style::Spaces,
            4,
            "",
            Scope::All,
            true,
        )
        .unwrap();
        assert_eq!(o.result, "    a\n    b\n    c");
        assert_eq!(o.total, 3);
        assert_eq!(o.changed, 3);
        assert_eq!(o.unit, "    ");
    }

    #[test]
    fn tabs_style() {
        let o = indent("a\nb", Mode::Indent, Style::Tabs, 2, "", Scope::All, true).unwrap();
        assert_eq!(o.result, "\t\ta\n\t\tb");
    }

    #[test]
    fn custom_prefix_applies_verbatim() {
        let o = indent(
            "hello\nworld",
            Mode::Indent,
            Style::Custom,
            1,
            "> ",
            Scope::All,
            true,
        )
        .unwrap();
        assert_eq!(o.result, "> hello\n> world");
    }

    #[test]
    fn custom_prefix_repeats_with_count() {
        assert_eq!(
            render("x", Mode::Indent, Style::Custom, 3, "-", Scope::All, true).unwrap(),
            "---x"
        );
    }

    #[test]
    fn blank_lines_are_skipped_by_default() {
        // No trailing whitespace is introduced on the empty line.
        let o = indent(
            "a\n\n  \nb",
            Mode::Indent,
            Style::Spaces,
            2,
            "",
            Scope::All,
            true,
        )
        .unwrap();
        assert_eq!(o.result, "  a\n\n  \n  b");
        assert_eq!(o.changed, 2);
    }

    #[test]
    fn blank_lines_can_be_indented_too() {
        let o = indent(
            "a\n\nb",
            Mode::Indent,
            Style::Spaces,
            2,
            "",
            Scope::All,
            false,
        )
        .unwrap();
        assert_eq!(o.result, "  a\n  \n  b");
        assert_eq!(o.changed, 3);
    }

    #[test]
    fn hanging_indent_leaves_the_first_line_flush() {
        let o = indent(
            "Author, A. (2020). Title.\ncontinued\nmore",
            Mode::Indent,
            Style::Spaces,
            4,
            "",
            Scope::Hanging,
            true,
        )
        .unwrap();
        assert_eq!(
            o.result,
            "Author, A. (2020). Title.\n    continued\n    more"
        );
        assert_eq!(o.changed, 2);
    }

    #[test]
    fn first_line_only() {
        assert_eq!(
            render(
                "a\nb",
                Mode::Indent,
                Style::Spaces,
                4,
                "",
                Scope::FirstLine,
                true
            )
            .unwrap(),
            "    a\nb"
        );
    }

    #[test]
    fn paragraph_starts_only() {
        let o = render(
            "one\ntwo\n\nthree\nfour",
            Mode::Indent,
            Style::Spaces,
            4,
            "",
            Scope::ParagraphStarts,
            true,
        )
        .unwrap();
        assert_eq!(o, "    one\ntwo\n\n    three\nfour");
    }

    #[test]
    fn outdent_removes_up_to_count_units() {
        let o = indent(
            "      deep\n  shallow\nflush",
            Mode::Outdent,
            Style::Spaces,
            4,
            "",
            Scope::All,
            true,
        )
        .unwrap();
        // 4 removed, then only the 2 that exist, then nothing to remove.
        assert_eq!(o.result, "  deep\nshallow\nflush");
        assert_eq!(o.changed, 2);
    }

    #[test]
    fn outdent_strips_a_custom_prefix() {
        assert_eq!(
            render(
                "> quoted\n> lines",
                Mode::Outdent,
                Style::Custom,
                1,
                "> ",
                Scope::All,
                true
            )
            .unwrap(),
            "quoted\nlines"
        );
    }

    #[test]
    fn dedent_removes_the_common_indent_only() {
        let o = indent(
            "    fn main() {\n        println!(\"hi\");\n    }",
            Mode::Dedent,
            Style::Spaces,
            0,
            "",
            Scope::All,
            true,
        )
        .unwrap();
        assert_eq!(o.result, "fn main() {\n    println!(\"hi\");\n}");
        assert_eq!(o.unit, "    ");
    }

    #[test]
    fn dedent_ignores_blank_lines_when_measuring() {
        let o = render(
            "  a\n\n  b",
            Mode::Dedent,
            Style::Spaces,
            0,
            "",
            Scope::All,
            true,
        )
        .unwrap();
        assert_eq!(o, "a\n\nb");
    }

    #[test]
    fn mixed_tabs_and_spaces_dedent_to_the_common_prefix() {
        // Common prefix is a single tab; the extra space on line 2 survives.
        let o = render(
            "\ta\n\t b",
            Mode::Dedent,
            Style::Spaces,
            0,
            "",
            Scope::All,
            true,
        )
        .unwrap();
        assert_eq!(o, "a\n b");
    }

    #[test]
    fn trailing_newline_is_preserved() {
        assert_eq!(spaces("a\nb\n", 2), "  a\n  b\n");
        assert_eq!(spaces("a\n\n", 2), "  a\n\n");
    }

    #[test]
    fn crlf_line_endings_survive() {
        assert_eq!(spaces("a\r\nb", 2), "  a\r\n  b");
    }

    #[test]
    fn count_zero_is_a_no_op() {
        let o = indent("a\nb", Mode::Indent, Style::Spaces, 0, "", Scope::All, true).unwrap();
        assert_eq!(o.result, "a\nb");
        assert_eq!(o.changed, 0);
    }

    #[test]
    fn empty_input() {
        let o = indent("", Mode::Indent, Style::Spaces, 4, "", Scope::All, true).unwrap();
        assert_eq!(o.result, "");
        assert_eq!(o.total, 0);
        assert_eq!(o.changed, 0);
    }

    #[test]
    fn unicode_prefix_is_counted_in_characters() {
        assert_eq!(
            render("x", Mode::Indent, Style::Custom, 2, "→", Scope::All, true).unwrap(),
            "→→x"
        );
    }

    #[test]
    fn count_boundary() {
        assert!(indent(
            "x",
            Mode::Indent,
            Style::Spaces,
            MAX_COUNT,
            "",
            Scope::All,
            true
        )
        .is_ok());
        assert!(indent(
            "x",
            Mode::Indent,
            Style::Spaces,
            MAX_COUNT + 1,
            "",
            Scope::All,
            true
        )
        .is_err());
    }

    #[test]
    fn errors() {
        // Negative count.
        assert!(indent("x", Mode::Indent, Style::Spaces, -1, "", Scope::All, true).is_err());
        // custom style with no prefix.
        assert!(indent("x", Mode::Indent, Style::Custom, 1, "", Scope::All, true).is_err());
        // Over-long prefix.
        let long = "x".repeat(MAX_PREFIX_CHARS + 1);
        assert!(indent("x", Mode::Indent, Style::Custom, 1, &long, Scope::All, true).is_err());
        // Bad enum spellings.
        assert!(Mode::parse("nope").is_err());
        assert!(Style::parse("nope").is_err());
        assert!(Scope::parse("nope").is_err());
    }

    #[test]
    fn enum_aliases_parse() {
        assert_eq!(Mode::parse("indent").unwrap(), Mode::Indent);
        assert_eq!(Mode::parse("outdent").unwrap(), Mode::Outdent);
        assert_eq!(Mode::parse("dedent").unwrap(), Mode::Dedent);
        assert_eq!(Style::parse("spaces").unwrap(), Style::Spaces);
        assert_eq!(Style::parse("tabs").unwrap(), Style::Tabs);
        assert_eq!(Style::parse("custom").unwrap(), Style::Custom);
        assert_eq!(Scope::parse("all").unwrap(), Scope::All);
        assert_eq!(Scope::parse("first-line").unwrap(), Scope::FirstLine);
        assert_eq!(Scope::parse("hanging").unwrap(), Scope::Hanging);
        assert_eq!(
            Scope::parse("paragraph-starts").unwrap(),
            Scope::ParagraphStarts
        );
    }
}

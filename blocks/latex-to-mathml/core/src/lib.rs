//! latex-to-mathml core — convert a LaTeX math expression into MathML.
//!
//! Pure Rust (the `latex2mathml` crate, no external deps, no I/O), so it runs on
//! every backend including the chat Service Worker. Produces a single
//! `<math xmlns="http://www.w3.org/1998/Math/MathML">…</math>` element ready to
//! embed in HTML or reuse in DocBook/EPUB/Office documents.
//!
//! The converter covers the common math subset: numbers, ASCII/Greek letters,
//! symbols, relations/operators, `\frac`/`\sqrt`/`\binom`, sub/superscripts,
//! integrals, big operators (`\sum`/`\prod`), `\left…\right` delimiters, font
//! styles (`\mathbb`/`\mathbf`/…), spacing, and matrix/align environments.

use latex2mathml::{latex_to_mathml, DisplayStyle};

/// Whether the equation renders as a centered block or inline with the text.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Display {
    Block,
    Inline,
}

impl Display {
    pub fn parse(s: &str) -> Result<Display, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "block" | "" => Ok(Display::Block),
            "inline" => Ok(Display::Inline),
            other => Err(format!("unknown display '{other}' (use 'block' or 'inline')")),
        }
    }
    fn to_style(self) -> DisplayStyle {
        match self {
            Display::Block => DisplayStyle::Block,
            Display::Inline => DisplayStyle::Inline,
        }
    }
}

/// Conversion options.
pub struct Options {
    pub display: Display,
    /// Pretty-print the MathML with one element per indented line.
    pub pretty: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options { display: Display::Block, pretty: false }
    }
}

/// Surface entry point: parse the string params (display name, pretty flag) and
/// convert. Shared by the chat handler, the CLI, and the web page so every
/// surface has identical behaviour. `display` accepts "block"|"inline" (blank →
/// block); `pretty` indents one element per line.
pub fn run(latex: &str, display: &str, pretty: bool) -> Result<String, String> {
    let opts = Options { display: Display::parse(display)?, pretty };
    to_mathml(latex, &opts)
}

/// Convert a LaTeX math expression to a MathML `<math>` element.
pub fn to_mathml(latex: &str, opts: &Options) -> Result<String, String> {
    if latex.trim().is_empty() {
        return Err("no LaTeX provided — enter a math expression".into());
    }
    let mathml = latex_to_mathml(latex, opts.display.to_style())
        .map_err(|e| format!("could not parse LaTeX: {e}"))?;
    if opts.pretty {
        Ok(pretty_print(&mathml))
    } else {
        Ok(mathml)
    }
}

/// Indent a well-formed MathML string (the converter emits no whitespace) so it
/// reads one element per line. A forgiving XML tag walker: closing tags dedent
/// before printing, opening tags indent after; self-closing tags and text runs
/// stay on their own line at the current depth.
fn pretty_print(xml: &str) -> String {
    let bytes = xml.as_bytes();
    let mut out = String::with_capacity(xml.len() * 2);
    let mut depth: usize = 0;
    let mut i = 0;
    let mut first = true;
    while i < bytes.len() {
        if bytes[i] == b'<' {
            // find the end of this tag
            let end = match xml[i..].find('>') {
                Some(rel) => i + rel + 1,
                None => xml.len(),
            };
            let tag = &xml[i..end];
            let is_close = tag.starts_with("</");
            let is_self = tag.ends_with("/>");
            if is_close {
                depth = depth.saturating_sub(1);
            }
            if !first {
                out.push('\n');
            }
            first = false;
            for _ in 0..depth {
                out.push_str("  ");
            }
            out.push_str(tag);
            if !is_close && !is_self {
                depth += 1;
            }
            i = end;
        } else {
            // text run up to the next '<'
            let end = match xml[i..].find('<') {
                Some(rel) => i + rel,
                None => xml.len(),
            };
            let text = xml[i..end].trim();
            if !text.is_empty() {
                if !first {
                    out.push('\n');
                }
                first = false;
                for _ in 0..depth {
                    out.push_str("  ");
                }
                out.push_str(text);
            }
            i = end;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_fraction() {
        let out = to_mathml("\\frac{1}{2}", &Options::default()).unwrap();
        assert!(out.starts_with("<math"));
        assert!(out.contains("display=\"block\""));
        assert!(out.contains("<mfrac>"));
        assert!(out.ends_with("</math>"));
    }

    #[test]
    fn inline_display_attribute() {
        let out = to_mathml(
            "E = mc^2",
            &Options { display: Display::Inline, pretty: false },
        )
        .unwrap();
        assert!(out.contains("display=\"inline\""));
        assert!(out.contains("<msup>"));
    }

    #[test]
    fn quadratic_formula_superscript_and_sqrt() {
        let out =
            to_mathml("x = \\frac{-b \\pm \\sqrt{b^2 - 4ac}}{2a}", &Options::default()).unwrap();
        assert!(out.contains("<msqrt>"));
        assert!(out.contains("<mfrac>"));
    }

    #[test]
    fn greek_and_symbols() {
        let out = to_mathml("\\alpha + \\beta = \\gamma", &Options::default()).unwrap();
        assert!(out.contains("<mi>α</mi>") || out.contains("α"));
    }

    #[test]
    fn pretty_indents_each_element() {
        let out = to_mathml("\\frac{1}{2}", &Options { display: Display::Block, pretty: true })
            .unwrap();
        assert!(out.contains('\n'), "pretty output should have newlines");
        // nested element indented relative to <math>
        assert!(out.contains("\n  <mfrac>"));
        // round-trips back to the same compact form
        let compact: String = out.lines().map(|l| l.trim()).collect();
        let plain = to_mathml("\\frac{1}{2}", &Options::default()).unwrap();
        assert_eq!(compact, plain);
    }

    #[test]
    fn run_parses_string_params() {
        let out = run("a^2 + b^2", "inline", false).unwrap();
        assert!(out.contains("display=\"inline\""));
        assert!(out.contains("<msup>"));
        // blank display defaults to block
        let blk = run("x", "", false).unwrap();
        assert!(blk.contains("display=\"block\""));
        // pretty path
        let pretty = run("\\frac{1}{2}", "block", true).unwrap();
        assert!(pretty.contains('\n'));
        // invalid display surfaces the parse error
        assert!(run("x", "sideways", false).unwrap_err().contains("unknown display"));
    }

    #[test]
    fn empty_input_errors() {
        let err = to_mathml("   ", &Options::default()).unwrap_err();
        assert!(err.contains("no LaTeX"));
    }

    #[test]
    fn bad_display_errors() {
        assert!(Display::parse("sideways").is_err());
        assert_eq!(Display::parse("").unwrap(), Display::Block);
    }
}

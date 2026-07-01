//! gizza-ai/add-line-numbers core — prefix every line with a sequential line
//! number (like `nl` / `cat -n`), with a configurable start, step, separator,
//! number alignment, and an optional skip-blank-lines mode. Pure-Rust, no I/O.

use serde::Serialize;

/// How the line numbers are aligned to a common width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pad {
    /// No padding — numbers are written as-is (`1`, `2`, … `10`).
    None,
    /// Right-align with leading spaces so every number is the same width.
    Spaces,
    /// Left-pad with zeros so every number is the same width (`01`, `02`).
    Zeros,
}

impl Pad {
    pub fn parse(s: &str) -> Result<Pad, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "spaces" | "space" | "" => Ok(Pad::Spaces),
            "zeros" | "zero" | "zeroes" => Ok(Pad::Zeros),
            "none" => Ok(Pad::None),
            other => Err(format!("unknown pad '{other}' (use spaces, zeros, or none)")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Output {
    /// Total number of lines in the input.
    pub total: usize,
    /// How many lines actually received a number.
    pub numbered: usize,
    /// The numbered text.
    pub result: String,
}

/// Prefix every line of `text` with a sequential number.
///
/// * `start` — the first line's number.
/// * `step` — the increment between numbered lines (must be >= 1).
/// * `separator` — the string inserted between the number and the line content.
/// * `pad` — how numbers are aligned to a common width.
/// * `number_nonblank` — when true, blank (whitespace-only) lines are output
///   unchanged and do not consume a number (like `cat -b`).
pub fn add_numbers(
    text: &str,
    start: i64,
    step: i64,
    separator: &str,
    pad: Pad,
    number_nonblank: bool,
) -> Result<Output, String> {
    if step < 1 {
        return Err("step must be at least 1".into());
    }

    let lines: Vec<&str> = text.lines().collect();

    // First pass: assign each line a number (or None when skipped).
    let mut numbers: Vec<Option<i64>> = Vec::with_capacity(lines.len());
    let mut n = start;
    for line in &lines {
        if number_nonblank && line.trim().is_empty() {
            numbers.push(None);
        } else {
            numbers.push(Some(n));
            n = n.checked_add(step).ok_or("line number overflow")?;
        }
    }

    let numbered = numbers.iter().filter(|x| x.is_some()).count();
    // Column width = widest formatted number (counts a leading minus sign).
    let width = numbers
        .iter()
        .flatten()
        .map(|v| v.to_string().len())
        .max()
        .unwrap_or(0);

    let mut out = String::new();
    for (i, (line, num)) in lines.iter().zip(numbers.iter()).enumerate() {
        if i > 0 {
            out.push('\n');
        }
        match num {
            Some(v) => {
                let num_str = match pad {
                    Pad::None => v.to_string(),
                    Pad::Spaces => format!("{v:>width$}"),
                    Pad::Zeros => {
                        let s = v.to_string();
                        if let Some(stripped) = s.strip_prefix('-') {
                            format!("-{:0>w$}", stripped, w = width.saturating_sub(1))
                        } else {
                            format!("{s:0>width$}")
                        }
                    }
                };
                out.push_str(&num_str);
                out.push_str(separator);
                out.push_str(line);
            }
            None => out.push_str(line),
        }
    }

    Ok(Output {
        total: lines.len(),
        numbered,
        result: out,
    })
}

/// Human-readable rendering (used by the page) — just the numbered text.
pub fn render(
    text: &str,
    start: i64,
    step: i64,
    separator: &str,
    pad: Pad,
    number_nonblank: bool,
) -> Result<String, String> {
    Ok(add_numbers(text, start, step, separator, pad, number_nonblank)?.result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_tab_default() {
        let o = add_numbers("foo\nbar\nbaz", 1, 1, "\t", Pad::Spaces, false).unwrap();
        assert_eq!(o.result, "1\tfoo\n2\tbar\n3\tbaz");
        assert_eq!(o.total, 3);
        assert_eq!(o.numbered, 3);
    }

    #[test]
    fn start_and_step() {
        let o = add_numbers("a\nb\nc", 10, 5, ": ", Pad::None, false).unwrap();
        assert_eq!(o.result, "10: a\n15: b\n20: c");
    }

    #[test]
    fn space_padding_aligns() {
        // 10 lines → widest number is "10" (width 2); "1" right-aligns to " 1".
        let text = "a\nb\nc\nd\ne\nf\ng\nh\ni\nj";
        let o = add_numbers(text, 1, 1, ". ", Pad::Spaces, false).unwrap();
        assert!(o.result.starts_with(" 1. a\n 2. b\n"));
        assert!(o.result.ends_with("10. j"));
    }

    #[test]
    fn zero_padding() {
        let text = "a\nb\nc\nd\ne\nf\ng\nh\ni\nj\nk";
        let o = add_numbers(text, 1, 1, " ", Pad::Zeros, false).unwrap();
        assert!(o.result.starts_with("01 a\n02 b\n"));
        assert!(o.result.contains("\n10 j\n11 k"));
    }

    #[test]
    fn number_nonblank_skips_blanks() {
        let o = add_numbers("foo\n\n  \nbar", 1, 1, "\t", Pad::None, true).unwrap();
        // blank + whitespace-only lines keep no number and don't consume one
        assert_eq!(o.result, "1\tfoo\n\n  \n2\tbar");
        assert_eq!(o.total, 4);
        assert_eq!(o.numbered, 2);
    }

    #[test]
    fn number_all_lines_by_default() {
        let o = add_numbers("foo\n\nbar", 1, 1, "\t", Pad::None, false).unwrap();
        assert_eq!(o.result, "1\tfoo\n2\t\n3\tbar");
        assert_eq!(o.numbered, 3);
    }

    #[test]
    fn empty_input() {
        let o = add_numbers("", 1, 1, "\t", Pad::Spaces, false).unwrap();
        assert_eq!(o.result, "");
        assert_eq!(o.total, 0);
        assert_eq!(o.numbered, 0);
    }

    #[test]
    fn errors() {
        assert!(add_numbers("x", 1, 0, "\t", Pad::None, false).is_err());
        assert!(add_numbers("x", 1, -1, "\t", Pad::None, false).is_err());
        assert!(Pad::parse("nope").is_err());
    }
}

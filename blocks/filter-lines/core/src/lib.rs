//! gizza-ai/filter-lines core — keep or drop lines matching a substring or
//! regular expression, like a friendly `grep`. Pure-Rust (`regex`).

use regex::RegexBuilder;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Keep,
    Drop,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Mode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "keep" | "" => Ok(Mode::Keep),
            "drop" | "remove" | "exclude" => Ok(Mode::Drop),
            other => Err(format!("unknown mode '{other}' (use keep or drop)")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Output {
    pub total: usize,
    /// Lines that matched the pattern.
    pub matched: usize,
    /// Lines kept in the result.
    pub kept: usize,
    pub result: String,
}

/// Filter `text` line by line.
pub fn filter(
    text: &str,
    pattern: &str,
    mode: Mode,
    use_regex: bool,
    ignore_case: bool,
) -> Result<Output, String> {
    if pattern.is_empty() {
        return Err("pattern is empty — provide a substring or regular expression".into());
    }

    // A matcher closure: substring (optionally case-insensitive) or regex.
    let re = if use_regex {
        Some(
            RegexBuilder::new(pattern)
                .case_insensitive(ignore_case)
                .build()
                .map_err(|e| format!("invalid regular expression: {e}"))?,
        )
    } else {
        None
    };
    let needle_lc = pattern.to_lowercase();

    let matches = |line: &str| -> bool {
        match &re {
            Some(r) => r.is_match(line),
            None => {
                if ignore_case {
                    line.to_lowercase().contains(&needle_lc)
                } else {
                    line.contains(pattern)
                }
            }
        }
    };

    let mut total = 0;
    let mut matched = 0;
    let mut out_lines: Vec<&str> = Vec::new();
    for line in text.lines() {
        total += 1;
        let m = matches(line);
        if m {
            matched += 1;
        }
        let keep = match mode {
            Mode::Keep => m,
            Mode::Drop => !m,
        };
        if keep {
            out_lines.push(line);
        }
    }

    Ok(Output {
        total,
        matched,
        kept: out_lines.len(),
        result: out_lines.join("\n"),
    })
}

/// Human-readable rendering (used by the page) — just the filtered text.
pub fn render(
    text: &str,
    pattern: &str,
    mode: Mode,
    use_regex: bool,
    ignore_case: bool,
) -> Result<String, String> {
    Ok(filter(text, pattern, mode, use_regex, ignore_case)?.result)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "apple\nbanana\nApricot\ncherry\navocado";

    #[test]
    fn keep_substring() {
        let o = filter(SAMPLE, "a", Mode::Keep, false, false).unwrap();
        // lines containing lowercase 'a': apple, banana, avocado (Apricot has capital A)
        assert_eq!(o.result, "apple\nbanana\navocado");
        assert_eq!(o.matched, 3);
        assert_eq!(o.total, 5);
        assert_eq!(o.kept, 3);
    }

    #[test]
    fn drop_substring() {
        let o = filter(SAMPLE, "a", Mode::Drop, false, false).unwrap();
        assert_eq!(o.result, "Apricot\ncherry");
        assert_eq!(o.kept, 2);
    }

    #[test]
    fn ignore_case() {
        let o = filter(SAMPLE, "ap", Mode::Keep, false, true).unwrap();
        // apple, Apricot
        assert_eq!(o.result, "apple\nApricot");
    }

    #[test]
    fn regex_keep() {
        let o = filter(SAMPLE, "^a", Mode::Keep, true, false).unwrap();
        // apple, avocado (start with lowercase a)
        assert_eq!(o.result, "apple\navocado");
    }

    #[test]
    fn regex_ignore_case() {
        let o = filter(SAMPLE, "^a", Mode::Keep, true, true).unwrap();
        assert_eq!(o.result, "apple\nApricot\navocado");
    }

    #[test]
    fn errors() {
        assert!(filter("x", "", Mode::Keep, false, false).is_err());
        assert!(filter("x", "(", Mode::Keep, true, false).is_err()); // bad regex
        assert!(Mode::parse("nope").is_err());
    }
}

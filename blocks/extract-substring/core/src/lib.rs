//! gizza-ai/extract-substring core — pull out a portion of text either by
//! start/end character index (Python-style, negatives count from the end) or
//! everything between two delimiters (all non-overlapping occurrences).
//! Pure-Rust, dependency-free (besides serde).

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Index,
    Delimiters,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Mode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "index" | "" => Ok(Mode::Index),
            "delimiters" | "delimiter" | "between" => Ok(Mode::Delimiters),
            other => Err(format!("unknown mode '{other}' (use index or delimiters)")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Output {
    pub mode: String,
    pub count: usize,
    /// The extracted substring(s). Index mode returns exactly one.
    pub results: Vec<String>,
}

/// Resolve a possibly-negative char index to a clamped `0..=n` position.
fn resolve(i: i64, n: i64) -> i64 {
    if i < 0 {
        (n + i).max(0)
    } else {
        i.min(n)
    }
}

fn by_index(text: &str, start: i64, end: Option<i64>) -> String {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len() as i64;
    let s = resolve(start, n);
    let e = end.map(|e| resolve(e, n)).unwrap_or(n);
    if s >= e {
        return String::new();
    }
    chars[s as usize..e as usize].iter().collect()
}

fn by_delimiters(text: &str, start_delim: &str, end_delim: &str) -> Result<Vec<String>, String> {
    if start_delim.is_empty() || end_delim.is_empty() {
        return Err("both start and end delimiters are required in delimiters mode".into());
    }
    let mut out = Vec::new();
    let mut pos = 0;
    while pos <= text.len() {
        let Some(s_rel) = text[pos..].find(start_delim) else { break };
        let s = pos + s_rel + start_delim.len();
        let Some(e_rel) = text[s..].find(end_delim) else { break };
        let e = s + e_rel;
        out.push(text[s..e].to_string());
        pos = e + end_delim.len();
    }
    Ok(out)
}

/// Extract substring(s) from `text`.
pub fn extract(
    text: &str,
    mode: Mode,
    start: i64,
    end: Option<i64>,
    start_delim: &str,
    end_delim: &str,
) -> Result<Output, String> {
    let results = match mode {
        Mode::Index => vec![by_index(text, start, end)],
        Mode::Delimiters => by_delimiters(text, start_delim, end_delim)?,
    };
    let mode_name = match mode {
        Mode::Index => "index",
        Mode::Delimiters => "delimiters",
    };
    Ok(Output { mode: mode_name.into(), count: results.len(), results })
}

/// Human-readable rendering (used by the page).
pub fn render(
    text: &str,
    mode: Mode,
    start: i64,
    end: Option<i64>,
    start_delim: &str,
    end_delim: &str,
) -> Result<String, String> {
    let o = extract(text, mode, start, end, start_delim, end_delim)?;
    match mode {
        Mode::Index => Ok(o.results.into_iter().next().unwrap_or_default()),
        Mode::Delimiters => {
            if o.count == 0 {
                return Ok("No matches between the delimiters.".to_string());
            }
            let mut s = format!("{} match(es):\n", o.count);
            for (i, r) in o.results.iter().enumerate() {
                s.push_str(&format!("[{}] {}\n", i + 1, r));
            }
            Ok(s.trim_end().to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(text: &str, start: i64, end: Option<i64>) -> String {
        extract(text, Mode::Index, start, end, "", "").unwrap().results.remove(0)
    }

    #[test]
    fn index_basic() {
        assert_eq!(one("hello world", 0, Some(5)), "hello");
        assert_eq!(one("hello world", 6, None), "world");
        assert_eq!(one("hello world", 6, Some(11)), "world");
    }

    #[test]
    fn index_negative_and_clamp() {
        assert_eq!(one("hello", -2, None), "lo");
        assert_eq!(one("hello", 0, Some(-1)), "hell");
        assert_eq!(one("hello", 2, Some(100)), "llo"); // end clamps
        assert_eq!(one("hello", 4, Some(2)), ""); // start>=end
    }

    #[test]
    fn index_unicode() {
        // char-based, not byte-based
        assert_eq!(one("héllo", 0, Some(2)), "hé");
        assert_eq!(one("café", -1, None), "é");
    }

    #[test]
    fn delimiters_all_occurrences() {
        let o = extract("a[1]b[2]c[3]", Mode::Delimiters, 0, None, "[", "]").unwrap();
        assert_eq!(o.results, vec!["1", "2", "3"]);
        assert_eq!(o.count, 3);
    }

    #[test]
    fn delimiters_multichar() {
        let o = extract("<b>x</b> and <b>y</b>", Mode::Delimiters, 0, None, "<b>", "</b>").unwrap();
        assert_eq!(o.results, vec!["x", "y"]);
    }

    #[test]
    fn delimiters_none() {
        let o = extract("no markers here", Mode::Delimiters, 0, None, "[", "]").unwrap();
        assert!(o.results.is_empty());
    }

    #[test]
    fn errors() {
        assert!(extract("x", Mode::Delimiters, 0, None, "", "]").is_err());
        assert!(Mode::parse("nope").is_err());
    }

    #[test]
    fn render_modes() {
        assert_eq!(render("hello", Mode::Index, 0, Some(2), "", "").unwrap(), "he");
        assert!(render("a[1]b[2]", Mode::Delimiters, 0, None, "[", "]").unwrap().contains("2 match"));
    }
}

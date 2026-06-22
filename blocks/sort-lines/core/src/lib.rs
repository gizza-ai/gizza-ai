//! gizza-ai/sort-lines core — sort the lines of a block of text.
//! Pure-Rust, no deps beyond serde. Shared by the chat skill block and the page.
//!
//! Supports alphabetical or numeric ordering, ascending or descending, optional
//! case-insensitive comparison, optional whitespace trimming for the comparison
//! key, optional removal of duplicate lines, and an option to drop blank lines.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    Asc,
    Desc,
}

impl Order {
    pub fn parse(s: &str) -> Result<Order, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "asc" | "ascending" | "" => Ok(Order::Asc),
            "desc" | "descending" => Ok(Order::Desc),
            other => Err(format!("unknown order '{other}' (use asc or desc)")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// Lexicographic, by Unicode scalar value.
    Alpha,
    /// Parse a leading number from each line; non-numeric lines sort last.
    Numeric,
    /// Natural order: digit runs compared as numbers (e.g. file2 < file10).
    Natural,
    /// Sort by line length (shortest first under asc).
    Length,
}

impl Method {
    pub fn parse(s: &str) -> Result<Method, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "alpha" | "alphabetical" | "" => Ok(Method::Alpha),
            "numeric" | "number" => Ok(Method::Numeric),
            "natural" => Ok(Method::Natural),
            "length" => Ok(Method::Length),
            other => Err(format!(
                "unknown method '{other}' (use alpha, numeric, natural or length)"
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Output {
    /// Number of input lines.
    pub total: usize,
    /// Number of output lines (after optional dedup / blank removal).
    pub lines: usize,
    /// Number of duplicate lines removed (0 if dedup off).
    pub removed_duplicates: usize,
    /// The sorted text.
    pub result: String,
}

/// Parse a leading (possibly signed/decimal) number from the start of `s`,
/// skipping leading whitespace. Returns None when no number is present.
fn leading_number(s: &str) -> Option<f64> {
    let t = s.trim_start();
    let bytes = t.as_bytes();
    let mut end = 0;
    let mut seen_digit = false;
    let mut seen_dot = false;
    while end < bytes.len() {
        let c = bytes[end] as char;
        if end == 0 && (c == '-' || c == '+') {
            end += 1;
            continue;
        }
        if c.is_ascii_digit() {
            seen_digit = true;
            end += 1;
        } else if c == '.' && !seen_dot {
            seen_dot = true;
            end += 1;
        } else {
            break;
        }
    }
    if !seen_digit {
        return None;
    }
    t[..end].parse::<f64>().ok()
}

/// Compare two digit-runs / non-digit-runs for a natural ordering.
fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let mut ai = a.chars().peekable();
    let mut bi = b.chars().peekable();
    loop {
        match (ai.peek().copied(), bi.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(ca), Some(cb)) => {
                if ca.is_ascii_digit() && cb.is_ascii_digit() {
                    // Compare the full digit runs as numbers.
                    let mut na = String::new();
                    while let Some(&c) = ai.peek() {
                        if c.is_ascii_digit() {
                            na.push(c);
                            ai.next();
                        } else {
                            break;
                        }
                    }
                    let mut nb = String::new();
                    while let Some(&c) = bi.peek() {
                        if c.is_ascii_digit() {
                            nb.push(c);
                            bi.next();
                        } else {
                            break;
                        }
                    }
                    // Strip leading zeros for the numeric comparison, then fall
                    // back to length, then to the original strings for ties.
                    let ta = na.trim_start_matches('0');
                    let tb = nb.trim_start_matches('0');
                    let ord = ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb));
                    if ord != Ordering::Equal {
                        return ord;
                    }
                    // Shorter zero-prefix sorts first for stable, predictable output.
                    let ord = na.len().cmp(&nb.len());
                    if ord != Ordering::Equal {
                        return ord;
                    }
                } else {
                    let ord = ca.cmp(&cb);
                    if ord != Ordering::Equal {
                        return ord;
                    }
                    ai.next();
                    bi.next();
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn sort(
    text: &str,
    method: Method,
    order: Order,
    ignore_case: bool,
    trim: bool,
    unique: bool,
    remove_blank: bool,
) -> Result<Output, String> {
    use std::cmp::Ordering;

    let mut lines: Vec<&str> = text.lines().collect();
    let total = lines.len();

    if remove_blank {
        lines.retain(|l| !l.trim().is_empty());
    }

    // Comparison key: optionally trimmed and/or lowercased.
    let key = |l: &str| -> String {
        let s = if trim { l.trim() } else { l };
        if ignore_case {
            s.to_lowercase()
        } else {
            s.to_string()
        }
    };

    lines.sort_by(|a, b| {
        let ord = match method {
            Method::Alpha => key(a).cmp(&key(b)),
            Method::Length => {
                let la = if trim { a.trim() } else { a };
                let lb = if trim { b.trim() } else { b };
                la.chars()
                    .count()
                    .cmp(&lb.chars().count())
                    .then_with(|| key(a).cmp(&key(b)))
            }
            Method::Natural => natural_cmp(&key(a), &key(b)),
            Method::Numeric => {
                let na = leading_number(a);
                let nb = leading_number(b);
                match (na, nb) {
                    (Some(x), Some(y)) => {
                        x.partial_cmp(&y).unwrap_or(Ordering::Equal).then_with(|| key(a).cmp(&key(b)))
                    }
                    // Numeric lines sort before non-numeric lines (ascending).
                    (Some(_), None) => Ordering::Less,
                    (None, Some(_)) => Ordering::Greater,
                    (None, None) => key(a).cmp(&key(b)),
                }
            }
        };
        ord
    });

    if order == Order::Desc {
        lines.reverse();
    }

    let mut removed_duplicates = 0;
    if unique {
        let mut seen = std::collections::HashSet::new();
        let mut out: Vec<&str> = Vec::with_capacity(lines.len());
        for l in lines {
            if seen.insert(key(l)) {
                out.push(l);
            } else {
                removed_duplicates += 1;
            }
        }
        lines = out;
    }

    Ok(Output {
        total,
        lines: lines.len(),
        removed_duplicates,
        result: lines.join("\n"),
    })
}

/// Human-readable rendering used by the page — just the sorted text.
#[allow(clippy::too_many_arguments)]
pub fn render(
    text: &str,
    method: Method,
    order: Order,
    ignore_case: bool,
    trim: bool,
    unique: bool,
    remove_blank: bool,
) -> Result<String, String> {
    Ok(sort(text, method, order, ignore_case, trim, unique, remove_blank)?.result)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRUITS: &str = "banana\nApple\ncherry\napple\nDate";

    fn s(
        text: &str,
        method: Method,
        order: Order,
        ic: bool,
        trim: bool,
        uniq: bool,
        rb: bool,
    ) -> String {
        sort(text, method, order, ic, trim, uniq, rb).unwrap().result
    }

    #[test]
    fn alpha_asc_case_sensitive() {
        // Uppercase letters sort before lowercase in Unicode order.
        let o = s(FRUITS, Method::Alpha, Order::Asc, false, false, false, false);
        assert_eq!(o, "Apple\nDate\napple\nbanana\ncherry");
    }

    #[test]
    fn alpha_desc() {
        let o = s(FRUITS, Method::Alpha, Order::Desc, false, false, false, false);
        assert_eq!(o, "cherry\nbanana\napple\nDate\nApple");
    }

    #[test]
    fn alpha_ignore_case() {
        let o = s(FRUITS, Method::Alpha, Order::Asc, true, false, false, false);
        // apple/Apple tie (stable -> input order: Apple then apple), banana, cherry, Date
        assert_eq!(o, "Apple\napple\nbanana\ncherry\nDate");
    }

    #[test]
    fn numeric_sort() {
        let t = "10 apples\n2 pears\n1 plum\n30 figs";
        let o = s(t, Method::Numeric, Order::Asc, false, false, false, false);
        assert_eq!(o, "1 plum\n2 pears\n10 apples\n30 figs");
    }

    #[test]
    fn numeric_non_numeric_last() {
        let t = "5\nhello\n2\nworld";
        let o = s(t, Method::Numeric, Order::Asc, false, false, false, false);
        assert_eq!(o, "2\n5\nhello\nworld");
    }

    #[test]
    fn natural_sort() {
        let t = "file10\nfile2\nfile1\nfile20";
        let o = s(t, Method::Natural, Order::Asc, false, false, false, false);
        assert_eq!(o, "file1\nfile2\nfile10\nfile20");
    }

    #[test]
    fn length_sort() {
        let t = "ccc\na\nbb\ndddd";
        let o = s(t, Method::Length, Order::Asc, false, false, false, false);
        assert_eq!(o, "a\nbb\nccc\ndddd");
    }

    #[test]
    fn unique_removes_dups() {
        let t = "b\na\nb\na\nc";
        let out = sort(t, Method::Alpha, Order::Asc, false, false, true, false).unwrap();
        assert_eq!(out.result, "a\nb\nc");
        assert_eq!(out.removed_duplicates, 2);
        assert_eq!(out.lines, 3);
        assert_eq!(out.total, 5);
    }

    #[test]
    fn trim_key_and_remove_blank() {
        let t = "  b\n\na \n   \n  a";
        let out = sort(t, Method::Alpha, Order::Asc, false, true, true, true).unwrap();
        // blanks removed; "a " and "  a" both trim to "a" -> dedup to 1
        assert_eq!(out.result, "a \n  b");
        assert_eq!(out.removed_duplicates, 1);
    }

    #[test]
    fn parse_helpers() {
        assert_eq!(Order::parse("DESC").unwrap(), Order::Desc);
        assert!(Order::parse("sideways").is_err());
        assert_eq!(Method::parse("natural").unwrap(), Method::Natural);
        assert!(Method::parse("bogus").is_err());
    }

    #[test]
    fn leading_number_parsing() {
        assert_eq!(leading_number("  -3.5 foo"), Some(-3.5));
        assert_eq!(leading_number("42"), Some(42.0));
        assert_eq!(leading_number("foo"), None);
        assert_eq!(leading_number(""), None);
    }
}

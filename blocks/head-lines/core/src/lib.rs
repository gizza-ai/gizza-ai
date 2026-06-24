//! head-lines core — output the first N lines of text. Pure compute, shared by
//! the chat skill block and the web page. No wafer/wasm-bindgen deps.

/// Maximum number of lines `count` may request. Guards against absurd inputs and
/// keeps the LLM-facing schema bounded; the core clamps to `1..=MAX_COUNT`.
pub const MAX_COUNT: u32 = 1_000_000;

/// Default number of lines to keep when `count` is `0` (i.e. omitted). Matches
/// the conventional `head` default and the descriptor's `default(10)`, so an
/// omitted `count` behaves the same on every surface (chat/CLI use serde's `0`
/// default; this maps it to 10 rather than 1).
pub const DEFAULT_COUNT: u32 = 10;

/// Return the first `count` lines of `text` (the `head -n` / "top N rows"
/// operation).
///
/// - `count`: how many leading lines to keep. `0` means "omitted" and uses
///   `DEFAULT_COUNT` (10); otherwise clamped to `1..=MAX_COUNT`.
/// - `skip`: drop this many lines from the start *before* taking `count`
///   (like `tail -n +K`); `0` keeps the very first line.
/// - `number`: when `true`, prefix each kept line with its 1-based line number
///   (counting from the original text, after `skip`) and a tab — like
///   `cat -n` / `nl`.
///
/// Line splitting is on `'\n'`; a trailing `'\r'` (Windows CRLF) is preserved on
/// each line as-is. A final newline is appended only if the original input had a
/// trailing newline, so taking the head of a file keeps its line structure.
/// Lines are rejoined with `'\n'`.
pub fn head(text: &str, count: u32, skip: u32, number: bool) -> Result<String, String> {
    let count = if count == 0 { DEFAULT_COUNT } else { count };
    let count = count.clamp(1, MAX_COUNT) as usize;
    let skip = skip as usize;

    // Split into lines without losing whether there was a trailing newline.
    // `split('\n')` yields a trailing empty element iff the text ends with '\n'
    // (or is empty), which we drop and remember.
    let mut parts: Vec<&str> = text.split('\n').collect();
    let had_trailing_newline = matches!(parts.last(), Some(&"")) && !text.is_empty();
    if had_trailing_newline {
        parts.pop();
    }

    // 1-based original line numbers start at skip+1.
    let kept: Vec<String> = parts
        .into_iter()
        .skip(skip)
        .take(count)
        .enumerate()
        .map(|(i, line)| {
            if number {
                format!("{}\t{}", skip + i + 1, line)
            } else {
                line.to_string()
            }
        })
        .collect();

    let mut out = kept.join("\n");
    // Re-add a trailing newline only when the original had one AND we actually
    // kept at least one line (the result is non-empty).
    if had_trailing_newline && !out.is_empty() {
        out.push('\n');
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_n_lines() {
        let t = "a\nb\nc\nd\ne";
        assert_eq!(head(t, 3, 0, false).unwrap(), "a\nb\nc");
    }

    #[test]
    fn count_zero_uses_default_ten() {
        // 0 == "omitted" -> DEFAULT_COUNT (10), so all 3 lines are returned.
        assert_eq!(head("a\nb\nc", 0, 0, false).unwrap(), "a\nb\nc");
        // And it really is capped at 10 when there are more.
        let many: String = (1..=15).map(|n| n.to_string()).collect::<Vec<_>>().join("\n");
        assert_eq!(head(&many, 0, 0, false).unwrap().lines().count(), 10);
    }

    #[test]
    fn count_one_keeps_single_line() {
        assert_eq!(head("a\nb\nc", 1, 0, false).unwrap(), "a");
    }

    #[test]
    fn count_exceeds_lines_returns_all() {
        assert_eq!(head("a\nb", 100, 0, false).unwrap(), "a\nb");
    }

    #[test]
    fn skip_then_take() {
        let t = "1\n2\n3\n4\n5";
        assert_eq!(head(t, 2, 2, false).unwrap(), "3\n4");
    }

    #[test]
    fn number_prefix_counts_original_lines() {
        let t = "x\ny\nz";
        assert_eq!(head(t, 2, 1, true).unwrap(), "2\ty\n3\tz");
    }

    #[test]
    fn preserves_trailing_newline() {
        assert_eq!(head("a\nb\nc\n", 2, 0, false).unwrap(), "a\nb\n");
    }

    #[test]
    fn no_trailing_newline_when_absent() {
        assert_eq!(head("a\nb\nc", 2, 0, false).unwrap(), "a\nb");
    }

    #[test]
    fn preserves_crlf() {
        // Trailing \r on each line is kept verbatim.
        assert_eq!(head("a\r\nb\r\nc", 2, 0, false).unwrap(), "a\r\nb\r");
    }

    #[test]
    fn empty_input() {
        assert_eq!(head("", 5, 0, false).unwrap(), "");
    }

    #[test]
    fn skip_past_end_is_empty() {
        assert_eq!(head("a\nb", 3, 10, false).unwrap(), "");
    }

    #[test]
    fn single_line_no_newline() {
        assert_eq!(head("hello", 10, 0, false).unwrap(), "hello");
    }
}

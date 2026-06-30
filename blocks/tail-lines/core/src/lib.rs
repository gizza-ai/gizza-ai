//! tail-lines core — output the last N lines of text. Pure compute, shared by
//! the chat skill block and the web page. No wafer/wasm-bindgen deps.

/// Maximum number of lines `count` may request. Guards against absurd inputs and
/// keeps the LLM-facing schema bounded; the core clamps to `1..=MAX_COUNT`.
pub const MAX_COUNT: u32 = 1_000_000;

/// Default number of lines to keep when `count` is `0` (i.e. omitted). Matches
/// the conventional `tail` default and the descriptor's `default(10)`, so an
/// omitted `count` behaves the same on every surface (chat/CLI use serde's `0`
/// default; this maps it to 10 rather than 1).
pub const DEFAULT_COUNT: u32 = 10;

/// Return the last `count` lines of `text` (the `tail -n` / "bottom N rows"
/// operation).
///
/// - `count`: how many trailing lines to keep. `0` means "omitted" and uses
///   `DEFAULT_COUNT` (10); otherwise clamped to `1..=MAX_COUNT`.
/// - `skip`: drop this many lines from the *end* before taking the last `count`
///   (the symmetric mirror of `head`'s leading skip — e.g. `skip=1` ignores a
///   trailing footer line); `0` keeps through the very last line.
/// - `number`: when `true`, prefix each kept line with its 1-based line number
///   (counting from the original text) and a tab — like `cat -n` / `nl`, so the
///   numbers reflect each line's real position near the end of the input.
///
/// Line splitting is on `'\n'`; a trailing `'\r'` (Windows CRLF) is preserved on
/// each line as-is. A final newline is appended only if the original input had a
/// trailing newline, so taking the tail of a file keeps its line structure.
/// Lines are rejoined with `'\n'`.
pub fn tail(text: &str, count: u32, skip: u32, number: bool) -> Result<String, String> {
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

    // Drop `skip` lines from the END, then keep the last `count` of what remains.
    let end = parts.len().saturating_sub(skip);
    let start = end.saturating_sub(count);

    // 1-based original line numbers: the first kept line is at index `start`.
    let kept: Vec<String> = parts[start..end]
        .iter()
        .enumerate()
        .map(|(i, line)| {
            if number {
                format!("{}\t{}", start + i + 1, line)
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
    fn last_n_lines() {
        let t = "a\nb\nc\nd\ne";
        assert_eq!(tail(t, 3, 0, false).unwrap(), "c\nd\ne");
    }

    #[test]
    fn count_zero_uses_default_ten() {
        // 0 == "omitted" -> DEFAULT_COUNT (10), so all 3 lines are returned.
        assert_eq!(tail("a\nb\nc", 0, 0, false).unwrap(), "a\nb\nc");
        // And it really is capped at 10 when there are more — keeps the LAST 10.
        let many: String = (1..=15).map(|n| n.to_string()).collect::<Vec<_>>().join("\n");
        let out = tail(&many, 0, 0, false).unwrap();
        assert_eq!(out.lines().count(), 10);
        assert!(out.starts_with("6\n"));
        assert!(out.ends_with("15"));
    }

    #[test]
    fn count_one_keeps_last_line() {
        assert_eq!(tail("a\nb\nc", 1, 0, false).unwrap(), "c");
    }

    #[test]
    fn count_exceeds_lines_returns_all() {
        assert_eq!(tail("a\nb", 100, 0, false).unwrap(), "a\nb");
    }

    #[test]
    fn skip_from_end_then_take() {
        // Drop the last 2 lines (4,5), then take the last 2 of {1,2,3} -> 2,3.
        let t = "1\n2\n3\n4\n5";
        assert_eq!(tail(t, 2, 2, false).unwrap(), "2\n3");
    }

    #[test]
    fn number_prefix_counts_original_lines() {
        // Last 2 lines of {x,y,z} are y(2) and z(3).
        let t = "x\ny\nz";
        assert_eq!(tail(t, 2, 0, true).unwrap(), "2\ty\n3\tz");
    }

    #[test]
    fn number_prefix_with_skip() {
        // Drop last line (z), take last 2 of {w,x,y} -> x(2),y(3).
        let t = "w\nx\ny\nz";
        assert_eq!(tail(t, 2, 1, true).unwrap(), "2\tx\n3\ty");
    }

    #[test]
    fn preserves_trailing_newline() {
        assert_eq!(tail("a\nb\nc\n", 2, 0, false).unwrap(), "b\nc\n");
    }

    #[test]
    fn no_trailing_newline_when_absent() {
        assert_eq!(tail("a\nb\nc", 2, 0, false).unwrap(), "b\nc");
    }

    #[test]
    fn preserves_crlf() {
        // Trailing \r on each line is kept verbatim.
        assert_eq!(tail("a\r\nb\r\nc", 2, 0, false).unwrap(), "b\r\nc");
    }

    #[test]
    fn empty_input() {
        assert_eq!(tail("", 5, 0, false).unwrap(), "");
    }

    #[test]
    fn skip_past_end_is_empty() {
        assert_eq!(tail("a\nb", 3, 10, false).unwrap(), "");
    }

    #[test]
    fn single_line_no_newline() {
        assert_eq!(tail("hello", 10, 0, false).unwrap(), "hello");
    }
}

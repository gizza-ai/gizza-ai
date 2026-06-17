//! word-count core — counts words, characters, and lines in a text string.
//! No wafer/wasm-bindgen deps. Shared by the chat skill block and the web page.

/// Count words, characters, and lines in `text`.
///
/// - **words**: whitespace-split non-empty tokens (same as `split_whitespace`)
/// - **characters**: total Unicode scalar values (`chars().count()`)
/// - **lines**: `lines().count()` — an empty string yields 0 lines
///
/// Always succeeds; returns `Err` only in shape (so callers can use `?`).
pub fn count(text: &str) -> Result<String, String> {
    let words = text.split_whitespace().count();
    let chars = text.chars().count();
    let lines = text.lines().count();
    Ok(format!("{words} words, {chars} characters, {lines} lines"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_simple_text() {
        let r = count("a b c").unwrap();
        assert!(r.contains("3 words"), "expected '3 words' in {r:?}");
        assert!(r.contains("5 characters"), "expected '5 characters' in {r:?}");
        assert!(r.contains("1 lines"), "expected '1 lines' in {r:?}");
    }

    #[test]
    fn empty_string_returns_zeros() {
        let r = count("").unwrap();
        assert_eq!(r, "0 words, 0 characters, 0 lines");
    }

    #[test]
    fn counts_multiline() {
        let r = count("hello world\nfoo bar baz").unwrap();
        assert!(r.contains("5 words"), "expected '5 words' in {r:?}");
        assert!(r.contains("2 lines"), "expected '2 lines' in {r:?}");
    }
}

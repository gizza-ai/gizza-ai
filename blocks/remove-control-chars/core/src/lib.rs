//! gizza-ai/remove-control-chars core — strip null bytes and non-printable
//! control characters from text. Pure-Rust, dependency-free.
//!
//! A "control character" is any char in Unicode category Cc — the C0 range
//! U+0000–U+001F (which includes the NUL byte U+0000) and the C1/DEL range
//! U+007F–U+009F. This matches Rust's `char::is_control`.
//!
//! Tab (U+0009), line feed (U+000A) and carriage return (U+000D) are technically
//! control characters but are usually meaningful whitespace, so they can be kept
//! via `keep_tabs` / `keep_newlines`. Each removed character is replaced by the
//! `replacement` string ("" by default → the character is simply deleted).

/// Return true if `c` is a control character that should be removed given the
/// keep flags.
fn is_removable(c: char, keep_tabs: bool, keep_newlines: bool) -> bool {
    if !c.is_control() {
        return false;
    }
    if keep_tabs && c == '\t' {
        return false;
    }
    if keep_newlines && (c == '\n' || c == '\r') {
        return false;
    }
    true
}

/// Strip control characters from `text`.
///
/// * `keep_tabs` — keep horizontal tab (`\t`).
/// * `keep_newlines` — keep line feed (`\n`) and carriage return (`\r`).
/// * `replacement` — string substituted for each removed character ("" deletes it).
pub fn remove_control_chars(
    text: &str,
    keep_tabs: bool,
    keep_newlines: bool,
    replacement: &str,
) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if is_removable(c, keep_tabs, keep_newlines) {
            out.push_str(replacement);
        } else {
            out.push(c);
        }
    }
    out
}

/// Count how many control characters in `text` would be removed given the flags.
pub fn count_removable(text: &str, keep_tabs: bool, keep_newlines: bool) -> usize {
    text.chars()
        .filter(|&c| is_removable(c, keep_tabs, keep_newlines))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_null_bytes() {
        assert_eq!(
            remove_control_chars("a\u{0000}b\u{0000}c", true, true, ""),
            "abc"
        );
    }

    #[test]
    fn removes_c0_and_del_and_c1() {
        // BEL (0x07), DEL (0x7F), and a C1 control (0x85 NEL) all stripped.
        let t = "x\u{0007}y\u{007F}z\u{0085}w";
        assert_eq!(remove_control_chars(t, true, true, ""), "xyzw");
    }

    #[test]
    fn keeps_tabs_and_newlines_by_default() {
        let t = "line1\tcol\nline2\r\nline3";
        assert_eq!(remove_control_chars(t, true, true, ""), t);
    }

    #[test]
    fn can_drop_tabs_and_newlines() {
        let t = "a\tb\nc\rd";
        assert_eq!(remove_control_chars(t, false, false, ""), "abcd");
    }

    #[test]
    fn replacement_string_substitutes() {
        let t = "a\u{0000}b\u{0007}c";
        assert_eq!(remove_control_chars(t, true, true, " "), "a b c");
        assert_eq!(remove_control_chars(t, true, true, "?"), "a?b?c");
    }

    #[test]
    fn keeps_printable_unicode() {
        // Emoji and accented letters are not control chars and must survive.
        let t = "café 🚀 — ok";
        assert_eq!(remove_control_chars(t, true, true, ""), t);
    }

    #[test]
    fn count_matches() {
        let t = "a\u{0000}\u{0007}b\tc\n";
        assert_eq!(count_removable(t, true, true), 2); // NUL + BEL; tab/newline kept
        assert_eq!(count_removable(t, false, false), 4); // + tab + newline
    }

    #[test]
    fn empty_input() {
        assert_eq!(remove_control_chars("", true, true, ""), "");
        assert_eq!(count_removable("", true, true), 0);
    }
}

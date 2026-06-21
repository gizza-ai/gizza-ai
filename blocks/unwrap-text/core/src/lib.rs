//! gizza-ai/unwrap-text core — remove hard line breaks inside paragraphs so
//! wrapped text becomes continuous lines, while keeping paragraph breaks (blank
//! lines). Pure-Rust, dependency-free.
//!
//! A "paragraph" is a run of consecutive non-blank lines; its lines are rejoined
//! with a single space. Blank lines separate paragraphs and are preserved (as a
//! single blank line). With `keep_list_breaks` (default true), lines that look
//! like list items / quotes (starting with -, *, +, >, or "N." ) keep their own
//! line so lists and quotes aren't mangled.

fn looks_like_list_item(line: &str) -> bool {
    let t = line.trim_start();
    if t.starts_with("- ") || t.starts_with("* ") || t.starts_with("+ ") || t.starts_with("> ") {
        return true;
    }
    // "1." / "12)" style ordered list markers.
    let mut chars = t.chars();
    let mut saw_digit = false;
    for c in chars.by_ref() {
        if c.is_ascii_digit() {
            saw_digit = true;
        } else if (c == '.' || c == ')') && saw_digit {
            return matches!(chars.next(), Some(' ') | None);
        } else {
            break;
        }
    }
    false
}

/// Rejoin wrapped lines. `keep_list_breaks` keeps list/quote lines on their own.
pub fn unwrap(text: &str, keep_list_breaks: bool) -> String {
    let norm = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut out: Vec<String> = Vec::new();
    let mut para: Vec<String> = Vec::new();

    let flush = |para: &mut Vec<String>, out: &mut Vec<String>| {
        if !para.is_empty() {
            out.push(para.join(" "));
            para.clear();
        }
    };

    for raw in norm.split('\n') {
        let line = raw.trim();
        if line.is_empty() {
            // Blank line: end the current paragraph, emit a paragraph break.
            flush(&mut para, &mut out);
            // Avoid piling up multiple blank separators.
            if out.last().map(|s| !s.is_empty()).unwrap_or(false) {
                out.push(String::new());
            }
        } else if keep_list_breaks && looks_like_list_item(line) {
            // Keep list/quote items on their own line.
            flush(&mut para, &mut out);
            out.push(line.to_string());
        } else {
            para.push(line.to_string());
        }
    }
    flush(&mut para, &mut out);

    // Trim a trailing empty separator.
    while out.last().map(|s| s.is_empty()).unwrap_or(false) {
        out.pop();
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_wrapped_paragraph() {
        let t = "This is a long line\nthat was hard-wrapped\ninto three pieces.";
        assert_eq!(unwrap(t, true), "This is a long line that was hard-wrapped into three pieces.");
    }

    #[test]
    fn preserves_paragraph_breaks() {
        let t = "First paragraph line one\nline two.\n\nSecond paragraph\nhere.";
        assert_eq!(unwrap(t, true), "First paragraph line one line two.\n\nSecond paragraph here.");
    }

    #[test]
    fn collapses_multiple_blank_lines() {
        let t = "A\n\n\n\nB";
        assert_eq!(unwrap(t, true), "A\n\nB");
    }

    #[test]
    fn keeps_list_items_separate() {
        let t = "Shopping:\n- milk\n- eggs\n- bread";
        assert_eq!(unwrap(t, true), "Shopping:\n- milk\n- eggs\n- bread");
        // ordered list too
        let o = "Steps:\n1. wake up\n2. code";
        assert_eq!(unwrap(o, true), "Steps:\n1. wake up\n2. code");
    }

    #[test]
    fn list_breaks_disabled_joins_everything() {
        let t = "intro\n- a\n- b";
        assert_eq!(unwrap(t, false), "intro - a - b");
    }

    #[test]
    fn single_line_unchanged() {
        assert_eq!(unwrap("already one line", true), "already one line");
        assert_eq!(unwrap("", true), "");
    }
}

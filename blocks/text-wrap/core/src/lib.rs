//! gizza-ai/text-wrap core — hard-wrap text to a given column width.
//!
//! Pure-Rust, dependency-free. Reflows each line so no output line exceeds
//! `width` display columns (measured in Unicode scalar values, which is exact
//! for plain prose). The wrap operates per input line: existing hard line breaks
//! and blank lines are preserved, so paragraph structure survives.
//!
//! Options:
//! - `preserve_indent` (default true): the leading whitespace of a source line is
//!   detected and re-applied to every continuation line it produces, so indented
//!   blocks and list items stay aligned.
//! - `break_long_words` (default true): a single word longer than the usable
//!   width is hard-split at the column. When false, such a word is left on its own
//!   over-length line rather than being broken mid-word.
//!
//! Wrapping is greedy (the classic word-wrap): words are packed onto a line until
//! the next word would overflow, then a new line is started. Runs of whitespace
//! between words collapse to a single space on wrapped lines; the leading indent
//! is kept verbatim.

/// Minimum accepted wrap width.
pub const MIN_WIDTH: u32 = 1;
/// Maximum accepted wrap width (guards against absurd inputs).
pub const MAX_WIDTH: u32 = 10_000;

fn char_len(s: &str) -> usize {
    s.chars().count()
}

/// Split a long word into pieces of at most `width` chars each.
fn hard_split(word: &str, width: usize) -> Vec<String> {
    let mut pieces = Vec::new();
    let mut cur = String::new();
    let mut n = 0usize;
    for c in word.chars() {
        if n == width {
            pieces.push(std::mem::take(&mut cur));
            n = 0;
        }
        cur.push(c);
        n += 1;
    }
    if !cur.is_empty() {
        pieces.push(cur);
    }
    pieces
}

/// Wrap a single (already newline-free) source line.
fn wrap_line(line: &str, width: usize, preserve_indent: bool, break_long_words: bool) -> Vec<String> {
    // Pull off the leading whitespace as the indent.
    let trimmed = line.trim_start();
    let indent_chars = line.len() - trimmed.len();
    let indent: String = if preserve_indent {
        line[..indent_chars].to_string()
    } else {
        String::new()
    };
    let indent_w = char_len(&indent);

    // If the line is empty or only whitespace, keep it as-is (blank line).
    if trimmed.is_empty() {
        return vec![line.to_string()];
    }

    // Usable width for content after the indent; always leave room for >=1 char.
    let usable = width.saturating_sub(indent_w).max(1);

    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut cur_w = 0usize;

    let mut push_line = |cur: &mut String, cur_w: &mut usize| {
        out.push(format!("{indent}{cur}"));
        cur.clear();
        *cur_w = 0;
    };

    for word in trimmed.split_whitespace() {
        let word_w = char_len(word);

        // A word that cannot fit even on its own line.
        if word_w > usable {
            // Flush the current line first.
            if !cur.is_empty() {
                push_line(&mut cur, &mut cur_w);
            }
            if break_long_words {
                for piece in hard_split(word, usable) {
                    cur = piece;
                    cur_w = char_len(&cur);
                    push_line(&mut cur, &mut cur_w);
                }
            } else {
                cur = word.to_string();
                cur_w = word_w;
                push_line(&mut cur, &mut cur_w);
            }
            continue;
        }

        // Does the word fit on the current line?
        let needed = if cur.is_empty() { word_w } else { cur_w + 1 + word_w };
        if needed > usable && !cur.is_empty() {
            push_line(&mut cur, &mut cur_w);
        }
        if cur.is_empty() {
            cur.push_str(word);
            cur_w = word_w;
        } else {
            cur.push(' ');
            cur.push_str(word);
            cur_w += 1 + word_w;
        }
    }
    if !cur.is_empty() {
        push_line(&mut cur, &mut cur_w);
    }
    if out.is_empty() {
        out.push(indent);
    }
    out
}

/// Hard-wrap `text` to `width` columns.
///
/// Errors if `width` is outside `MIN_WIDTH..=MAX_WIDTH`.
pub fn wrap(
    text: &str,
    width: u32,
    preserve_indent: bool,
    break_long_words: bool,
) -> Result<String, String> {
    if !(MIN_WIDTH..=MAX_WIDTH).contains(&width) {
        return Err(format!(
            "width must be between {MIN_WIDTH} and {MAX_WIDTH} (got {width})"
        ));
    }
    let width = width as usize;
    let norm = text.replace("\r\n", "\n").replace('\r', "\n");

    let mut out: Vec<String> = Vec::new();
    for line in norm.split('\n') {
        out.extend(wrap_line(line, width, preserve_indent, break_long_words));
    }
    Ok(out.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_to_width() {
        let t = "the quick brown fox jumps over the lazy dog";
        let got = wrap(t, 15, true, true).unwrap();
        assert_eq!(got, "the quick brown\nfox jumps over\nthe lazy dog");
        for line in got.lines() {
            assert!(line.chars().count() <= 15);
        }
    }

    #[test]
    fn preserves_blank_lines_and_paragraphs() {
        let t = "para one here\n\npara two here";
        let got = wrap(t, 80, true, true).unwrap();
        assert_eq!(got, "para one here\n\npara two here");
    }

    #[test]
    fn preserves_indent() {
        let t = "    an indented sentence that should wrap and keep its indent";
        let got = wrap(t, 20, true, true).unwrap();
        for line in got.lines() {
            assert!(line.starts_with("    "), "line lost indent: {line:?}");
            assert!(line.chars().count() <= 20);
        }
    }

    #[test]
    fn drop_indent_when_disabled() {
        let t = "    indented words here please";
        let got = wrap(t, 10, false, true).unwrap();
        assert!(!got.starts_with(' '));
    }

    #[test]
    fn breaks_long_word_when_enabled() {
        let t = "supercalifragilisticexpialidocious";
        let got = wrap(t, 10, true, true).unwrap();
        for line in got.lines() {
            assert!(line.chars().count() <= 10);
        }
        // round-trips back to the original word (no spaces inserted)
        assert_eq!(got.replace('\n', ""), t);
    }

    #[test]
    fn keeps_long_word_intact_when_disabled() {
        let t = "supercalifragilisticexpialidocious";
        let got = wrap(t, 10, true, false).unwrap();
        assert_eq!(got, t);
    }

    #[test]
    fn collapses_inner_whitespace_runs() {
        let t = "a    b\tc";
        let got = wrap(t, 80, true, true).unwrap();
        assert_eq!(got, "a b c");
    }

    #[test]
    fn unicode_counted_by_chars() {
        // 5 accented chars + space + 5 chars -> wraps at width 5
        let t = "ééééé ààààà";
        let got = wrap(t, 5, true, true).unwrap();
        assert_eq!(got, "ééééé\nààààà");
    }

    #[test]
    fn rejects_bad_width() {
        assert!(wrap("x", 0, true, true).is_err());
        assert!(wrap("x", MAX_WIDTH + 1, true, true).is_err());
    }

    #[test]
    fn empty_input() {
        assert_eq!(wrap("", 80, true, true).unwrap(), "");
    }
}

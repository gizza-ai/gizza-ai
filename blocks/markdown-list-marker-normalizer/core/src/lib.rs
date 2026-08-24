//! gizza-ai/markdown-list-marker-normalizer core — rewrite the *list scaffolding*
//! of a Markdown document without touching anything else.
//!
//! Three things are normalized:
//!   1. **Unordered markers** — every `-` / `*` / `+` bullet becomes one chosen
//!      character (or alternates per nesting depth in `sublist` mode).
//!   2. **Indentation steps** — sloppy 1/3/5-space nesting is snapped to an exact
//!      N-spaces-per-level ladder (tabs expanded at 4 columns, output is spaces).
//!   3. **Ordered numbering + marker spacing** — `1. 2. 3.`, all-`1.`, all-`0.`,
//!      or left alone, and a fixed number of spaces between marker and content.
//!
//! Everything that is not list scaffolding is preserved byte-for-byte: item text,
//! headings, tables, blockquotes, thematic breaks (`---`, `***`), emphasis at the
//! start of a line (`*bold*` has no space after the `*`, so it is not a bullet),
//! and — deliberately — every line inside a fenced code block.
//!
//! Pure string processing, no dependencies, no I/O.

/// Maximum input size, in characters.
pub const MAX_CHARS: usize = 500_000;
/// Widest indent step (spaces per nesting level) we accept.
pub const MAX_INDENT: i64 = 8;
/// Widest gap we accept between a list marker and its content.
pub const MAX_MARKER_SPACE: i64 = 4;
/// A tab advances to the next multiple of this many columns (CommonMark).
const TAB_WIDTH: usize = 4;
/// A line must be indented at least this much deeper than its parent item to
/// count as a new nesting level; anything less is treated as sloppy alignment
/// of a sibling and snapped back.
const NEST_TOLERANCE: usize = 2;
/// Marker cycle used by `marker = "sublist"`, one per nesting depth.
const SUBLIST_CYCLE: [char; 3] = ['-', '*', '+'];

/// Which character unordered bullets should end up as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Marker {
    /// Every bullet becomes `-`.
    Dash,
    /// Every bullet becomes `*`.
    Asterisk,
    /// Every bullet becomes `+`.
    Plus,
    /// Every bullet becomes whichever character the document used first.
    Consistent,
    /// Depth 0 uses `-`, depth 1 `*`, depth 2 `+`, then the cycle repeats.
    Sublist,
}

impl Marker {
    pub fn parse(s: &str) -> Result<Marker, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "dash" | "hyphen" | "-" => Ok(Marker::Dash),
            "asterisk" | "star" | "*" => Ok(Marker::Asterisk),
            "plus" | "+" => Ok(Marker::Plus),
            "consistent" | "first" => Ok(Marker::Consistent),
            "sublist" | "alternate" => Ok(Marker::Sublist),
            other => Err(format!(
                "unknown marker '{other}' (use dash, asterisk, plus, consistent, or sublist)"
            )),
        }
    }
}

/// What to do with the numbers on ordered (`1.`) list items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ordered {
    /// Leave the numbers exactly as written.
    Keep,
    /// Renumber each list sequentially from its own first number.
    Sequential,
    /// Every item becomes `1.` (lazy numbering — diff-friendly).
    One,
    /// Every item becomes `0.`.
    Zero,
}

impl Ordered {
    pub fn parse(s: &str) -> Result<Ordered, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "keep" | "as-is" => Ok(Ordered::Keep),
            "ordered" | "sequential" | "increment" => Ok(Ordered::Sequential),
            "one" => Ok(Ordered::One),
            "zero" => Ok(Ordered::Zero),
            other => Err(format!(
                "unknown ordered '{other}' (use keep, ordered, one, or zero)"
            )),
        }
    }
}

/// The marker that introduces a list item.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Kind {
    /// An unordered bullet (`-`, `*`, `+`).
    Ul,
    /// An ordered item: the number as written, plus its `.` or `)` delimiter.
    Ol { number: u64, delim: char },
}

/// A parsed list-item line.
struct Item<'a> {
    /// Visual column the marker starts at (tabs expanded).
    indent: usize,
    kind: Kind,
    /// Visual column the item text starts at, before rewriting.
    content_col: usize,
    /// Item text after the marker and the whitespace following it.
    content: &'a str,
}

/// Leading-whitespace width in columns plus its length in bytes.
fn leading_ws(line: &str) -> (usize, usize) {
    let (mut width, mut bytes) = (0usize, 0usize);
    for c in line.chars() {
        match c {
            ' ' => {
                width += 1;
                bytes += 1;
            }
            '\t' => {
                width += TAB_WIDTH - (width % TAB_WIDTH);
                bytes += 1;
            }
            _ => break,
        }
    }
    (width, bytes)
}

/// `---`, `***`, `___`, `- - -` … — a thematic break, never a list item.
fn is_thematic_break(rest: &str) -> bool {
    let mut seen: Option<char> = None;
    let mut count = 0usize;
    for c in rest.chars() {
        match c {
            ' ' | '\t' => {}
            '-' | '*' | '_' => {
                match seen {
                    Some(prev) if prev != c => return false,
                    _ => seen = Some(c),
                }
                count += 1;
            }
            _ => return false,
        }
    }
    seen.is_some() && count >= 3
}

/// A ``` or ~~~ fence opener/closer → the fence character.
fn fence_char(rest: &str) -> Option<char> {
    if rest.starts_with("```") {
        Some('`')
    } else if rest.starts_with("~~~") {
        Some('~')
    } else {
        None
    }
}

/// Consume leading spaces/tabs from `s`, returning the column landed on and the
/// remaining text. `col` is the column `s` itself starts at, so tabs expand right.
fn skip_ws_col(mut col: usize, s: &str) -> (usize, &str) {
    for c in s.chars() {
        match c {
            ' ' => col += 1,
            '\t' => col += TAB_WIDTH - (col % TAB_WIDTH),
            _ => break,
        }
    }
    (col, s.trim_start_matches([' ', '\t']))
}

/// Parse `line` as a list item, or `None` if it is anything else.
fn parse_item(line: &str) -> Option<Item<'_>> {
    let (indent, off) = leading_ws(line);
    let rest = &line[off..];
    if rest.is_empty() || is_thematic_break(rest) {
        return None;
    }
    let first = rest.chars().next()?;

    // Unordered: one bullet char, then a space/tab or end of line.
    if matches!(first, '-' | '*' | '+') {
        let after = &rest[1..];
        if !after.is_empty() && !after.starts_with(' ') && !after.starts_with('\t') {
            return None;
        }
        let (content_col, content) = skip_ws_col(indent + 1, after);
        return Some(Item {
            indent,
            kind: Kind::Ul,
            content_col,
            content,
        });
    }

    // Ordered: up to 9 digits, then `.` or `)`, then a space/tab or end of line.
    if !first.is_ascii_digit() {
        return None;
    }
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.len() > 9 {
        return None;
    }
    let after_digits = &rest[digits.len()..];
    let delim = after_digits.chars().next()?;
    if delim != '.' && delim != ')' {
        return None;
    }
    let after = &after_digits[1..];
    if !after.is_empty() && !after.starts_with(' ') && !after.starts_with('\t') {
        return None;
    }
    let (content_col, content) = skip_ws_col(indent + digits.len() + 1, after);
    Some(Item {
        indent,
        kind: Kind::Ol {
            number: digits.parse().ok()?,
            delim,
        },
        content_col,
        content,
    })
}

/// One open nesting level: the original indent column its items sat at, and the
/// running counter used to renumber ordered items at that level.
struct Level {
    orig_indent: usize,
    counter: u64,
    seeded: bool,
}

/// Normalize the list markers, nesting indentation, and ordered numbering of a
/// Markdown document.
///
/// * `marker` — `dash` | `asterisk` | `plus` | `consistent` | `sublist`
/// * `indent` — spaces per nesting level (1–8), applied when `normalize_indent`
/// * `normalize_indent` — `false` keeps each item's original indentation
/// * `ordered` — `keep` | `ordered` | `one` | `zero`
/// * `marker_space` — spaces between the marker and the item text (1–4)
pub fn normalize(
    markdown: &str,
    marker: &str,
    indent: i64,
    normalize_indent: bool,
    ordered: &str,
    marker_space: i64,
) -> Result<String, String> {
    if markdown.trim().is_empty() {
        return Err("markdown is empty — paste the Markdown text to normalize".into());
    }
    let char_count = markdown.chars().count();
    if char_count > MAX_CHARS {
        return Err(format!(
            "markdown is {char_count} characters, over the {MAX_CHARS} character limit"
        ));
    }
    let marker = Marker::parse(marker)?;
    let ordered = Ordered::parse(ordered)?;
    if !(1..=MAX_INDENT).contains(&indent) {
        return Err(format!(
            "indent must be between 1 and {MAX_INDENT} spaces, got {indent}"
        ));
    }
    if !(1..=MAX_MARKER_SPACE).contains(&marker_space) {
        return Err(format!(
            "marker_space must be between 1 and {MAX_MARKER_SPACE} spaces, got {marker_space}"
        ));
    }
    let indent = indent as usize;
    let marker_space = marker_space as usize;

    let had_trailing_newline = markdown.ends_with('\n');
    let mut out: Vec<String> = Vec::new();
    let mut levels: Vec<Level> = Vec::new();
    let mut fence: Option<char> = None;
    let mut first_ul: Option<char> = None;
    // Column shift applied to the most recent item, reused for its wrapped lines.
    let mut shift: i64 = 0;

    for raw in markdown.split('\n') {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        let carriage = line.len() != raw.len();
        let (width, off) = leading_ws(line);
        let rest = &line[off..];

        // Inside a fenced code block nothing is rewritten — code stays verbatim.
        if let Some(open) = fence {
            if fence_char(rest) == Some(open) && rest.trim_end().chars().all(|c| c == open) {
                fence = None;
            }
            out.push(raw.to_string());
            continue;
        }
        if let Some(open) = fence_char(rest) {
            fence = Some(open);
            out.push(raw.to_string());
            continue;
        }

        // Blank lines never end a list (loose lists have them between items).
        if rest.is_empty() {
            out.push(raw.to_string());
            continue;
        }

        let item = parse_item(line);
        if item.is_none() {
            if width == 0 {
                // Unindented prose/heading/table closes any open list.
                levels.clear();
                shift = 0;
            } else if !levels.is_empty() && normalize_indent && shift != 0 {
                // A wrapped continuation line of the current item: move it by the
                // same amount its marker moved so it stays attached.
                let new_width = (width as i64 + shift).max(0) as usize;
                let mut s = " ".repeat(new_width);
                s.push_str(rest);
                if carriage {
                    s.push('\r');
                }
                out.push(s);
                continue;
            }
            out.push(raw.to_string());
            continue;
        }
        let item = item.unwrap();

        // Snap this item's indent column onto the nesting ladder.
        while levels.len() > 1 && item.indent + NEST_TOLERANCE <= levels[levels.len() - 1].orig_indent
        {
            levels.pop();
        }
        let deeper = levels
            .last()
            .is_some_and(|l| item.indent >= l.orig_indent + NEST_TOLERANCE);
        if levels.is_empty() || deeper {
            levels.push(Level {
                orig_indent: item.indent,
                counter: 0,
                seeded: false,
            });
        }
        let depth = levels.len() - 1;

        let marker_text = match &item.kind {
            Kind::Ul => {
                let ch = match marker {
                    Marker::Dash => '-',
                    Marker::Asterisk => '*',
                    Marker::Plus => '+',
                    Marker::Sublist => SUBLIST_CYCLE[depth % SUBLIST_CYCLE.len()],
                    Marker::Consistent => {
                        let seen = *first_ul.get_or_insert_with(|| {
                            line[off..].chars().next().unwrap_or('-')
                        });
                        seen
                    }
                };
                // Remember the document's first bullet even outside `consistent`,
                // so the choice is stable if the mode is switched.
                first_ul.get_or_insert(ch);
                ch.to_string()
            }
            Kind::Ol { number, delim } => {
                let level = levels.last_mut().expect("level pushed above");
                if !level.seeded {
                    level.counter = *number;
                    level.seeded = true;
                } else {
                    level.counter = level.counter.saturating_add(1);
                }
                let n = match ordered {
                    Ordered::Keep => *number,
                    Ordered::Sequential => level.counter,
                    Ordered::One => 1,
                    Ordered::Zero => 0,
                };
                format!("{n}{delim}")
            }
        };

        let new_indent = if normalize_indent {
            depth * indent
        } else {
            item.indent
        };
        let mut s = " ".repeat(new_indent);
        s.push_str(&marker_text);
        if !item.content.is_empty() {
            s.push_str(&" ".repeat(marker_space));
            s.push_str(item.content);
        }
        // Where the item text used to start vs. where it starts now — wrapped
        // continuation lines of this item get shifted by the same amount.
        let new_content_col = new_indent + marker_text.chars().count() + marker_space;
        shift = new_content_col as i64 - item.content_col as i64;
        if carriage {
            s.push('\r');
        }
        out.push(s);
    }

    let mut result = out.join("\n");
    if had_trailing_newline && !result.ends_with('\n') {
        result.push('\n');
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(md: &str) -> String {
        normalize(md, "dash", 2, true, "keep", 1).unwrap()
    }

    #[test]
    fn mixed_markers_become_one_character() {
        let out = run("* alpha\n+ beta\n- gamma\n");
        assert_eq!(out, "- alpha\n- beta\n- gamma\n");
    }

    #[test]
    fn sloppy_nesting_snaps_to_an_exact_ladder() {
        let out = run("* alpha\n   + beta\n      - gamma\n* delta\n");
        assert_eq!(out, "- alpha\n  - beta\n    - gamma\n- delta\n");
    }

    #[test]
    fn one_space_drift_is_treated_as_a_sibling() {
        let out = run("- alpha\n - beta\n- gamma\n");
        assert_eq!(out, "- alpha\n- beta\n- gamma\n");
    }

    #[test]
    fn tabs_are_expanded_then_replaced_with_spaces() {
        let out = run("* alpha\n\t* beta\n");
        assert_eq!(out, "- alpha\n  - beta\n");
    }

    #[test]
    fn asterisk_plus_and_sublist_modes() {
        assert_eq!(
            normalize("- a\n  - b\n", "asterisk", 2, true, "keep", 1).unwrap(),
            "* a\n  * b\n"
        );
        assert_eq!(
            normalize("- a\n  - b\n", "plus", 2, true, "keep", 1).unwrap(),
            "+ a\n  + b\n"
        );
        assert_eq!(
            normalize("* a\n  * b\n    * c\n", "sublist", 2, true, "keep", 1).unwrap(),
            "- a\n  * b\n    + c\n"
        );
    }

    #[test]
    fn consistent_mode_follows_the_first_bullet_seen() {
        let out = normalize("+ alpha\n- beta\n* gamma\n", "consistent", 2, true, "keep", 1).unwrap();
        assert_eq!(out, "+ alpha\n+ beta\n+ gamma\n");
    }

    #[test]
    fn ordered_modes_renumber_or_leave_alone() {
        let src = "3. alpha\n7. beta\n9. gamma\n";
        assert_eq!(
            normalize(src, "dash", 2, true, "keep", 1).unwrap(),
            "3. alpha\n7. beta\n9. gamma\n"
        );
        assert_eq!(
            normalize(src, "dash", 2, true, "ordered", 1).unwrap(),
            "3. alpha\n4. beta\n5. gamma\n"
        );
        assert_eq!(
            normalize(src, "dash", 2, true, "one", 1).unwrap(),
            "1. alpha\n1. beta\n1. gamma\n"
        );
        assert_eq!(
            normalize(src, "dash", 2, true, "zero", 1).unwrap(),
            "0. alpha\n0. beta\n0. gamma\n"
        );
    }

    #[test]
    fn ordered_paren_delimiter_is_preserved() {
        let out = normalize("1) a\n1) b\n", "dash", 2, true, "ordered", 1).unwrap();
        assert_eq!(out, "1) a\n2) b\n");
    }

    #[test]
    fn nested_ordered_lists_number_independently() {
        let out = normalize(
            "1. alpha\n    1. one\n    5. two\n2. beta\n",
            "dash",
            3,
            true,
            "ordered",
            1,
        )
        .unwrap();
        assert_eq!(out, "1. alpha\n   1. one\n   2. two\n2. beta\n");
    }

    #[test]
    fn marker_space_widens_the_gap() {
        let out = normalize("-   alpha\n- beta\n", "dash", 2, true, "keep", 2).unwrap();
        assert_eq!(out, "-  alpha\n-  beta\n");
    }

    #[test]
    fn indent_step_is_configurable() {
        let out = normalize("- a\n  - b\n", "dash", 4, true, "keep", 1).unwrap();
        assert_eq!(out, "- a\n    - b\n");
    }

    #[test]
    fn normalize_indent_off_keeps_original_columns() {
        let out = normalize("* a\n     * b\n", "dash", 2, false, "keep", 1).unwrap();
        assert_eq!(out, "- a\n     - b\n");
    }

    #[test]
    fn fenced_code_is_never_touched() {
        let src = "- alpha\n\n```sh\n* not a bullet\n   + neither\n```\n\n+ beta\n";
        let out = run(src);
        assert_eq!(
            out,
            "- alpha\n\n```sh\n* not a bullet\n   + neither\n```\n\n- beta\n"
        );
    }

    #[test]
    fn thematic_breaks_and_emphasis_survive() {
        let src = "* alpha\n\n---\n\n***\n\n*emphasis* at line start\n";
        let out = run(src);
        assert_eq!(
            out,
            "- alpha\n\n---\n\n***\n\n*emphasis* at line start\n"
        );
    }

    #[test]
    fn task_list_checkboxes_are_preserved() {
        let out = run("* [ ] todo\n+ [x] done\n");
        assert_eq!(out, "- [ ] todo\n- [x] done\n");
    }

    #[test]
    fn headings_close_the_list_and_reset_nesting() {
        let out = run("  * alpha\n\n## Heading\n\n     + beta\n");
        assert_eq!(out, "- alpha\n\n## Heading\n\n- beta\n");
    }

    #[test]
    fn wrapped_continuation_lines_follow_their_item() {
        let out = normalize(
            "*    alpha\n     wrapped line\n",
            "dash",
            2,
            true,
            "keep",
            1,
        )
        .unwrap();
        assert_eq!(out, "- alpha\n  wrapped line\n");
    }

    #[test]
    fn crlf_and_missing_trailing_newline_round_trip() {
        assert_eq!(normalize("* a\r\n* b\r\n", "dash", 2, true, "keep", 1).unwrap(), "- a\r\n- b\r\n");
        assert_eq!(normalize("* a\n* b", "dash", 2, true, "keep", 1).unwrap(), "- a\n- b");
    }

    #[test]
    fn bare_marker_lines_get_no_trailing_space() {
        let out = run("*\n* alpha\n");
        assert_eq!(out, "-\n- alpha\n");
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = normalize("   \n", "dash", 2, true, "keep", 1).unwrap_err();
        assert!(err.contains("empty"), "{err}");
    }

    #[test]
    fn unknown_marker_is_an_error() {
        let err = normalize("- a", "bullet", 2, true, "keep", 1).unwrap_err();
        assert!(err.contains("unknown marker"), "{err}");
    }

    #[test]
    fn unknown_ordered_style_is_an_error() {
        let err = normalize("- a", "dash", 2, true, "roman", 1).unwrap_err();
        assert!(err.contains("unknown ordered"), "{err}");
    }

    #[test]
    fn out_of_range_indent_and_spacing_are_errors() {
        assert!(normalize("- a", "dash", 0, true, "keep", 1)
            .unwrap_err()
            .contains("indent must be"));
        assert!(normalize("- a", "dash", 9, true, "keep", 1)
            .unwrap_err()
            .contains("indent must be"));
        assert!(normalize("- a", "dash", 2, true, "keep", 5)
            .unwrap_err()
            .contains("marker_space must be"));
    }

    #[test]
    fn oversized_input_is_rejected_at_the_boundary() {
        let ok = "- a\n".repeat(MAX_CHARS / 4);
        assert_eq!(ok.chars().count(), MAX_CHARS);
        assert!(normalize(&ok, "dash", 2, true, "keep", 1).is_ok());
        let too_big = format!("{ok}- b");
        assert!(normalize(&too_big, "dash", 2, true, "keep", 1)
            .unwrap_err()
            .contains("character limit"));
    }
}

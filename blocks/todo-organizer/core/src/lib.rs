//! todo-organizer core — pure compute, shared by the chat skill block and the
//! web page. No deps. Parses a freeform brain-dump (one task per line) into a
//! prioritized Markdown checklist, grouping by urgency keywords and extracting
//! lightweight due-date hints.

/// Priority bucket, highest first.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Priority {
    High,
    Medium,
    Low,
}

impl Priority {
    fn label(self) -> &'static str {
        match self {
            Priority::High => "High priority",
            Priority::Medium => "Medium priority",
            Priority::Low => "Low priority",
        }
    }
}

// Keyword → priority. Matched case-insensitively as whole words against each
// line. Order within a list is irrelevant; High wins over Medium wins over Low
// when a line carries more than one signal.
const HIGH_WORDS: &[&str] = &[
    "urgent", "asap", "critical", "important", "now", "immediately", "today",
    "deadline", "overdue", "must", "blocker", "blocking", "p0", "p1", "high",
];
const MEDIUM_WORDS: &[&str] = &[
    "soon", "tomorrow", "this week", "next", "p2", "medium", "mid",
];
const LOW_WORDS: &[&str] = &[
    "later", "someday", "eventually", "whenever", "maybe", "optional",
    "low", "backlog", "nice to have", "p3", "p4", "if time",
];

// Due-hint phrases surfaced inline after the task text. Kept short and explicit
// so we never invent a date the user did not write.
const DUE_HINTS: &[&str] = &[
    "today", "tonight", "tomorrow", "this week", "next week", "this weekend",
    "monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday",
    "eod", "eow", "morning", "afternoon", "evening",
];

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Whole-phrase, case-insensitive containment check. `needle` is already
/// lowercase; `haystack_lower` is the lowercased line. A match must be bounded
/// by non-word characters (or string edges) so `now` does not match `known`.
fn contains_phrase(haystack_lower: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    let hb = haystack_lower.as_bytes();
    let nb = needle.as_bytes();
    let n = nb.len();
    if n > hb.len() {
        return false;
    }
    let mut i = 0;
    while i + n <= hb.len() {
        if &hb[i..i + n] == nb {
            let before_ok = i == 0 || !is_word_char(hb[i - 1] as char);
            let after_ok = i + n == hb.len() || !is_word_char(hb[i + n] as char);
            if before_ok && after_ok {
                return true;
            }
        }
        i += 1;
    }
    false
}

/// Keyword stem match: the needle must begin at a word boundary but may be
/// followed by more word characters, so the stem `urgent` matches `urgently`
/// and `block` matches `blocking`/`blocker`. (Due hints use the stricter
/// whole-word `contains_phrase` so `Friday` stays exact.)
fn contains_stem(haystack_lower: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    let hb = haystack_lower.as_bytes();
    let nb = needle.as_bytes();
    let n = nb.len();
    if n > hb.len() {
        return false;
    }
    let mut i = 0;
    while i + n <= hb.len() {
        if &hb[i..i + n] == nb {
            let before_ok = i == 0 || !is_word_char(hb[i - 1] as char);
            if before_ok {
                return true;
            }
        }
        i += 1;
    }
    false
}

fn classify(line_lower: &str) -> Priority {
    if HIGH_WORDS.iter().any(|w| contains_stem(line_lower, w)) {
        Priority::High
    } else if MEDIUM_WORDS.iter().any(|w| contains_stem(line_lower, w)) {
        Priority::Medium
    } else if LOW_WORDS.iter().any(|w| contains_stem(line_lower, w)) {
        Priority::Low
    } else {
        Priority::Medium
    }
}

/// Find the first due-hint phrase present in the (lowercased) line, returning
/// the canonical phrase as written in `DUE_HINTS`.
fn due_hint(line_lower: &str) -> Option<&'static str> {
    DUE_HINTS
        .iter()
        .find(|h| contains_phrase(line_lower, h))
        .copied()
}

/// Strip a leading bullet / number / checkbox marker so a brain-dump that is
/// already lightly formatted does not get double-bulleted.
fn strip_marker(s: &str) -> &str {
    let t = s.trim();
    let t = t
        .strip_prefix("- [ ]")
        .or_else(|| t.strip_prefix("- [x]"))
        .or_else(|| t.strip_prefix("- [X]"))
        .or_else(|| t.strip_prefix('-'))
        .or_else(|| t.strip_prefix('*'))
        .or_else(|| t.strip_prefix('+'))
        .or_else(|| t.strip_prefix('•'))
        .unwrap_or(t)
        .trim_start();
    // strip a leading "12." or "12)" numbering
    let cb = t.as_bytes();
    let mut idx = 0;
    while idx < cb.len() && cb[idx].is_ascii_digit() {
        idx += 1;
    }
    if idx > 0 && idx < cb.len() && (cb[idx] == b'.' || cb[idx] == b')') {
        return t[idx + 1..].trim();
    }
    t.trim()
}

fn title_case_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

struct Item {
    text: String,
    priority: Priority,
    due: Option<&'static str>,
}

/// Organize a freeform `text` brain-dump (one task per line; blank lines
/// ignored) into a Markdown checklist.
///
/// * `group_by` — `"priority"` (default) groups under High/Medium/Low headings;
///   `"none"` keeps the original order in one flat list.
/// * `numbered` — when true, emit a numbered list instead of checkbox bullets.
/// * `show_due` — when true (default), append any detected due hint as `(due: …)`.
pub fn organize(
    text: &str,
    group_by: &str,
    numbered: bool,
    show_due: bool,
) -> Result<String, String> {
    let mut items: Vec<Item> = Vec::new();
    for raw in text.lines() {
        let stripped = strip_marker(raw);
        if stripped.is_empty() {
            continue;
        }
        let lower = stripped.to_lowercase();
        let priority = classify(&lower);
        let due = if show_due { due_hint(&lower) } else { None };
        items.push(Item {
            text: stripped.to_string(),
            priority,
            due,
        });
    }

    if items.is_empty() {
        return Err("no tasks found — give one task per line".into());
    }

    let render_item = |idx: usize, it: &Item| -> String {
        let prefix = if numbered {
            format!("{}. ", idx)
        } else {
            "- [ ] ".to_string()
        };
        let mut line = format!("{}{}", prefix, it.text);
        if let Some(d) = it.due {
            line.push_str(&format!(" _(due: {})_", title_case_first(d)));
        }
        line
    };

    let mut out = String::from("# To-do\n\n");

    match group_by {
        "none" => {
            for (i, it) in items.iter().enumerate() {
                out.push_str(&render_item(i + 1, it));
                out.push('\n');
            }
        }
        _ => {
            for &p in &[Priority::High, Priority::Medium, Priority::Low] {
                let group: Vec<&Item> = items.iter().filter(|it| it.priority == p).collect();
                if group.is_empty() {
                    continue;
                }
                out.push_str(&format!("## {}\n\n", p.label()));
                for (i, it) in group.iter().enumerate() {
                    out.push_str(&render_item(i + 1, it));
                    out.push('\n');
                }
                out.push('\n');
            }
        }
    }

    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_by_priority_with_default_medium() {
        let out = organize(
            "Call the bank urgently\nBuy milk\nClean garage someday",
            "priority",
            false,
            true,
        )
        .unwrap();
        let high = out.find("High priority").unwrap();
        let med = out.find("Medium priority").unwrap();
        let low = out.find("Low priority").unwrap();
        assert!(high < med && med < low, "headings ordered High>Medium>Low");
        assert!(out.contains("- [ ] Call the bank urgently"));
        assert!(out.contains("- [ ] Buy milk")); // default medium
        assert!(out.contains("- [ ] Clean garage someday"));
    }

    #[test]
    fn high_wins_over_medium_and_low() {
        // line carries both an "urgent" (High) and "later" (Low) signal
        let out = organize("ship the urgent fix later", "priority", false, false).unwrap();
        let high = out.find("High priority").unwrap();
        let pos = out.find("ship the urgent fix later").unwrap();
        assert!(pos > high);
        assert!(!out.contains("Low priority"));
    }

    #[test]
    fn detects_due_hint() {
        let out = organize("Submit report by Friday", "priority", false, true).unwrap();
        assert!(out.contains("_(due: Friday)_"), "got: {out}");
    }

    #[test]
    fn show_due_false_hides_hints() {
        let out = organize("Submit report by Friday", "priority", false, false).unwrap();
        assert!(!out.contains("due:"));
    }

    #[test]
    fn whole_word_boundary_avoids_false_positive() {
        // "known" must NOT trigger the "now" High keyword
        let out = organize("the known issue", "priority", false, false).unwrap();
        assert!(!out.contains("High priority"), "got: {out}");
    }

    #[test]
    fn numbered_mode() {
        let out = organize("urgent thing\nanother urgent thing", "priority", true, false).unwrap();
        assert!(out.contains("1. urgent thing"));
        assert!(out.contains("2. another urgent thing"));
    }

    #[test]
    fn group_by_none_keeps_order_flat() {
        let out = organize("urgent a\nlater b\nmid c", "none", false, false).unwrap();
        assert!(!out.contains("## "));
        let a = out.find("urgent a").unwrap();
        let b = out.find("later b").unwrap();
        let c = out.find("mid c").unwrap();
        assert!(a < b && b < c, "order preserved");
    }

    #[test]
    fn strips_existing_markers() {
        let out = organize("- [ ] do thing\n* star item\n3. numbered item", "none", false, false)
            .unwrap();
        assert!(out.contains("- [ ] do thing"));
        assert!(out.contains("- [ ] star item"));
        assert!(out.contains("- [ ] numbered item"));
        // ensure no double bullet
        assert!(!out.contains("- [ ] - "));
        assert!(!out.contains("- [ ] *"));
    }

    #[test]
    fn blank_and_whitespace_lines_ignored() {
        let out = organize("real task\n\n   \n", "none", false, false).unwrap();
        assert_eq!(out.matches("- [ ]").count(), 1);
    }

    #[test]
    fn empty_input_errors() {
        assert!(organize("   \n  \n", "priority", false, false).is_err());
    }

    #[test]
    fn multiword_keyword_this_week_is_medium() {
        let out = organize("finish slides this week", "priority", false, false).unwrap();
        let med = out.find("Medium priority").unwrap();
        let pos = out.find("finish slides this week").unwrap();
        assert!(pos > med);
    }
}

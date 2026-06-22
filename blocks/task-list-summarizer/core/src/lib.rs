//! task-list-summarizer core — parse Markdown task lists (GFM checklists) into
//! done/pending counts and filtered views. No wafer/wasm-bindgen deps; shared by
//! the chat skill block and the web page.
//!
//! A "task" is a Markdown list item whose marker is followed by a GFM checkbox:
//! `- [ ] todo`, `* [x] done`, `+ [X] done`, or an ordered item `1. [ ] todo`.
//! The checkbox must be `[ ]` (pending) or `[x]`/`[X]` (done). Leading
//! indentation (nesting) is allowed; plain list items without a checkbox and all
//! other lines are ignored.

/// Output mode for [`summarize`].
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Text summary: counts + completion percentage (default).
    Summary,
    /// The completed tasks, one per line.
    Done,
    /// The not-yet-completed tasks, one per line.
    Pending,
    /// Machine-readable JSON: `{total, done, pending, percent, done_items, pending_items}`.
    Json,
}

impl Mode {
    fn parse(mode: &str) -> Result<Self, String> {
        match mode {
            "" | "summary" => Ok(Mode::Summary),
            "done" => Ok(Mode::Done),
            "pending" => Ok(Mode::Pending),
            "json" => Ok(Mode::Json),
            other => Err(format!(
                "invalid mode {other:?}: expected \"summary\", \"done\", \"pending\", or \"json\""
            )),
        }
    }
}

/// A single parsed checklist task.
struct Task {
    done: bool,
    /// The task text after the checkbox (trimmed of trailing whitespace).
    text: String,
}

/// Parse one line into a [`Task`], or `None` if it is not a GFM checkbox item.
///
/// Accepts the unordered markers `-`, `*`, `+` and an ordered marker
/// (`<digits>.` or `<digits>)`), each followed by exactly one space, then a
/// `[ ]`/`[x]`/`[X]` checkbox and another space (or end of line).
fn parse_task(line: &str) -> Option<Task> {
    let trimmed = line.trim_start();
    // Strip the list marker.
    let after_marker = if let Some(rest) = trimmed
        .strip_prefix("- ")
        .or_else(|| trimmed.strip_prefix("* "))
        .or_else(|| trimmed.strip_prefix("+ "))
    {
        rest
    } else {
        // Ordered marker: leading digits then '.' or ')' then a space.
        let digits: String = trimmed.chars().take_while(|c| c.is_ascii_digit()).collect();
        if digits.is_empty() {
            return None;
        }
        let rest = &trimmed[digits.len()..];
        let rest = rest.strip_prefix('.').or_else(|| rest.strip_prefix(')'))?;
        rest.strip_prefix(' ')?
    };

    // Now expect the checkbox: "[ ]", "[x]" or "[X]".
    let (done, after_box) = if let Some(rest) = after_marker.strip_prefix("[ ]") {
        (false, rest)
    } else if let Some(rest) = after_marker.strip_prefix("[x]") {
        (true, rest)
    } else if let Some(rest) = after_marker.strip_prefix("[X]") {
        (true, rest)
    } else {
        return None;
    };

    // The checkbox must be followed by a space (or be the entire item).
    let text = match after_box.strip_prefix(' ') {
        Some(t) => t,
        None if after_box.is_empty() => "",
        None => return None, // e.g. "[ ]x" is not a checkbox item
    };

    Some(Task {
        done,
        text: text.trim_end().to_string(),
    })
}

/// Round `done/total` to a whole-number percentage (0 when `total == 0`).
fn percent(done: usize, total: usize) -> u32 {
    if total == 0 {
        0
    } else {
        ((done as f64 / total as f64) * 100.0).round() as u32
    }
}

/// JSON-escape a string for embedding in the `json` mode output.
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn json_array(items: &[&str]) -> String {
    let inner: Vec<String> = items.iter().map(|s| format!("\"{}\"", json_escape(s))).collect();
    format!("[{}]", inner.join(","))
}

/// Parse a Markdown task list and summarize it according to `mode`.
///
/// - `mode` (`"summary"` | `"done"` | `"pending"` | `"json"`, blank → `"summary"`):
///     - `summary`: a human-readable line — total, done, pending, and percent complete.
///     - `done`: the completed task texts, one per line (empty string if none).
///     - `pending`: the not-yet-completed task texts, one per line (empty if none).
///     - `json`: a compact JSON object with the counts, percent, and the two item lists.
///
/// Returns `Err` only on an invalid `mode`. An input with no checklist items is
/// valid and reports zero totals.
pub fn summarize(input: &str, mode: &str) -> Result<String, String> {
    let m = Mode::parse(mode)?;

    let tasks: Vec<Task> = input.lines().filter_map(parse_task).collect();
    let total = tasks.len();
    let done_items: Vec<&str> = tasks.iter().filter(|t| t.done).map(|t| t.text.as_str()).collect();
    let pending_items: Vec<&str> =
        tasks.iter().filter(|t| !t.done).map(|t| t.text.as_str()).collect();
    let done = done_items.len();
    let pending = pending_items.len();
    let pct = percent(done, total);

    let out = match m {
        Mode::Summary => {
            if total == 0 {
                "No tasks found. Add Markdown checklist items like \"- [ ] task\" or \"- [x] done\".".to_string()
            } else {
                let task_word = if total == 1 { "task" } else { "tasks" };
                format!("{total} {task_word}: {done} done, {pending} pending ({pct}% complete).")
            }
        }
        Mode::Done => done_items.join("\n"),
        Mode::Pending => pending_items.join("\n"),
        Mode::Json => format!(
            "{{\"total\":{total},\"done\":{done},\"pending\":{pending},\"percent\":{pct},\"done_items\":{},\"pending_items\":{}}}",
            json_array(&done_items),
            json_array(&pending_items),
        ),
    };
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "# My list\n- [x] write code\n- [ ] write tests\n* [X] ship it\n+ [ ] celebrate\nnot a task\n- plain item";

    #[test]
    fn summary_counts_done_and_pending() {
        assert_eq!(
            summarize(SAMPLE, "summary").unwrap(),
            "4 tasks: 2 done, 2 pending (50% complete)."
        );
    }

    #[test]
    fn summary_defaults_when_blank() {
        assert_eq!(
            summarize(SAMPLE, "").unwrap(),
            "4 tasks: 2 done, 2 pending (50% complete)."
        );
    }

    #[test]
    fn done_lists_only_completed() {
        assert_eq!(summarize(SAMPLE, "done").unwrap(), "write code\nship it");
    }

    #[test]
    fn pending_lists_only_remaining() {
        assert_eq!(summarize(SAMPLE, "pending").unwrap(), "write tests\ncelebrate");
    }

    #[test]
    fn json_mode_is_machine_readable() {
        let out = summarize("- [x] a\n- [ ] b", "json").unwrap();
        assert_eq!(
            out,
            r#"{"total":2,"done":1,"pending":1,"percent":50,"done_items":["a"],"pending_items":["b"]}"#
        );
    }

    #[test]
    fn ordered_list_markers_are_tasks() {
        let out = summarize("1. [x] one\n2. [ ] two\n3) [x] three", "summary").unwrap();
        assert_eq!(out, "3 tasks: 2 done, 1 pending (67% complete).");
    }

    #[test]
    fn indented_nested_tasks_are_counted() {
        let out =
            summarize("- [ ] parent\n    - [x] child\n      - [ ] grandchild", "summary").unwrap();
        assert_eq!(out, "3 tasks: 1 done, 2 pending (33% complete).");
    }

    #[test]
    fn empty_input_reports_no_tasks() {
        assert_eq!(
            summarize("just some prose\n# heading", "summary").unwrap(),
            "No tasks found. Add Markdown checklist items like \"- [ ] task\" or \"- [x] done\"."
        );
        assert_eq!(summarize("", "done").unwrap(), "");
        assert_eq!(
            summarize("", "json").unwrap(),
            r#"{"total":0,"done":0,"pending":0,"percent":0,"done_items":[],"pending_items":[]}"#
        );
    }

    #[test]
    fn single_task_uses_singular_word() {
        assert_eq!(
            summarize("- [ ] only one", "summary").unwrap(),
            "1 task: 0 done, 1 pending (0% complete)."
        );
    }

    #[test]
    fn checkbox_without_following_space_is_not_a_task() {
        // "[ ]x" — no space after the box — is treated as plain text, not a task.
        assert_eq!(
            summarize("- [ ]x glued", "summary").unwrap(),
            "No tasks found. Add Markdown checklist items like \"- [ ] task\" or \"- [x] done\"."
        );
        // A bare checkbox with no text is a valid (empty) task.
        assert_eq!(
            summarize("- [x]", "summary").unwrap(),
            "1 task: 1 done, 0 pending (100% complete)."
        );
    }

    #[test]
    fn json_escapes_special_characters() {
        let out = summarize("- [ ] a \"quoted\" task\\path", "json").unwrap();
        assert!(out.contains(r#"["a \"quoted\" task\\path"]"#), "got: {out}");
    }

    #[test]
    fn rejects_unknown_mode() {
        let err = summarize("- [ ] x", "stats").unwrap_err();
        assert!(err.contains("invalid mode"), "got: {err}");
    }
}

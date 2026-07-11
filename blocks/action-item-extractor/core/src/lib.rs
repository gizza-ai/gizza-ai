//! action-item-extractor core — pure compute, shared by the chat skill block and
//! the web page. No deps. Parses freeform meeting or daily notes (one item per
//! line) into a structured task list: action items (with detected owners) and
//! decisions. Extraction is fully deterministic — owners, action verbs, and
//! decision markers come from explicit signals in the text; nothing is invented.

/// What a single non-empty line resolves to after classification.
enum Kind {
    /// A task someone needs to do, with an optional detected owner.
    Action { owner: Option<String>, task: String },
    /// A resolution / conclusion reached in the meeting.
    Decision(String),
    /// Neither — a plain note, skipped from the structured output.
    Note,
}

// Decision markers. A line carrying one of these (and no stronger action
// signal) is filed under Decisions. Matched case-insensitively at a word stem.
const DECISION_WORDS: &[&str] = &[
    "decided", "decision", "agreed", "resolved", "conclusion", "consensus",
    "concluded", "we will go with", "going with", "final call", "sign off",
    "signed off", "approved",
];

// Explicit action markers — a line prefixed with any of these is an action item
// regardless of verb. Checkbox markers are handled by strip_marker separately.
const ACTION_MARKERS: &[&str] = &[
    "action item", "action:", "action -", "todo:", "to-do:", "to do:",
    "task:", "follow up", "follow-up", "next step", "next steps", "ai:",
];

// Imperative action verbs that start a bare task line (no owner, no marker).
// Curated like todo-organizer's keyword lists: deterministic and in-model.
const ACTION_VERBS: &[&str] = &[
    "send", "email", "call", "book", "schedule", "update", "review", "write",
    "draft", "prepare", "share", "finalize", "finalise", "confirm", "check",
    "investigate", "ship", "deploy", "contact", "ask", "set up", "setup",
    "finish", "complete", "fix", "add", "remove", "create", "build", "test",
    "research", "plan", "organize", "organise", "reach out", "circle back",
    "sync", "coordinate", "arrange", "submit", "publish", "collect", "gather",
    "define", "document", "notify", "reply", "respond", "escalate", "verify",
];

// Pronouns / collective subjects that must NOT be treated as an owner name in
// the "Name will …" assignment pattern.
const NON_OWNER_SUBJECTS: &[&str] = &[
    "we", "i", "you", "they", "he", "she", "it", "this", "that", "there",
    "everyone", "everybody", "someone", "somebody", "nobody", "all",
];

// Modals / verbs that mark an assignment when they follow a leading name:
// "Alice will …", "Bob to …", "Carol should …".
const ASSIGN_CONNECTORS: &[&str] = &[
    "will", "to", "should", "must", "needs to", "need to", "is going to",
    "are going to", "'ll", "can", "shall", "would", "going to", "has to",
    "have to", "gonna", "agreed to", "volunteered to",
];

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Whole-phrase, case-insensitive containment bounded by non-word characters, so
/// `ai` does not match `email`. `needle` is already lowercase.
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

/// Stem match: needle begins at a word boundary but may be followed by more word
/// characters (so `decide` matches `decided`/`decision`).
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

/// Strip a leading bullet / number / checkbox marker so already-formatted notes
/// don't get double-bulleted.
fn strip_marker(s: &str) -> &str {
    let t = s.trim();
    let t = t
        .strip_prefix("- [ ]")
        .or_else(|| t.strip_prefix("- [x]"))
        .or_else(|| t.strip_prefix("- [X]"))
        .or_else(|| t.strip_prefix("* [ ]"))
        .or_else(|| t.strip_prefix("[ ]"))
        .or_else(|| t.strip_prefix("[x]"))
        .or_else(|| t.strip_prefix('-'))
        .or_else(|| t.strip_prefix('*'))
        .or_else(|| t.strip_prefix('+'))
        .or_else(|| t.strip_prefix('•'))
        .or_else(|| t.strip_prefix('◦'))
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

/// Trim surrounding punctuation/quotes and normalize whitespace of a task body.
fn clean_task(s: &str) -> String {
    let trimmed = s.trim().trim_matches(|c: char| {
        matches!(c, '-' | '–' | '—' | ':' | ',' | ';' | '.' | ' ' | '"' | '\'' | '(' | ')')
    });
    // collapse internal whitespace runs
    let mut out = String::with_capacity(trimmed.len());
    let mut prev_ws = false;
    for c in trimmed.chars() {
        if c.is_whitespace() {
            if !prev_ws && !out.is_empty() {
                out.push(' ');
            }
            prev_ws = true;
        } else {
            out.push(c);
            prev_ws = false;
        }
    }
    out.trim_end().to_string()
}

/// True if `tok` looks like a person/handle name (a capitalized word or two),
/// not a pronoun/collective subject.
fn looks_like_name(tok: &str) -> bool {
    let t = tok.trim();
    if t.is_empty() || t.len() > 40 {
        return false;
    }
    if NON_OWNER_SUBJECTS.contains(&t.to_lowercase().as_str()) {
        return false;
    }
    let first = t.chars().next().unwrap();
    first.is_uppercase()
        && t.chars()
            .all(|c| c.is_alphabetic() || matches!(c, '-' | '.' | '\'' | ' '))
}

/// Extract an `@handle` owner and strip the token from the line.
/// Returns (owner, line_without_handle).
fn extract_at_handle(line: &str) -> Option<(String, String)> {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'@' {
            let start = i + 1;
            let mut j = start;
            while j < bytes.len() {
                let c = bytes[j] as char;
                if c.is_alphanumeric() || matches!(c, '_' | '-' | '.') {
                    j += 1;
                } else {
                    break;
                }
            }
            if j > start {
                // strip a trailing dot from the handle (sentence period)
                let mut end = j;
                while end > start && bytes[end - 1] == b'.' {
                    end -= 1;
                }
                let owner = line[start..end].to_string();
                let mut rest = String::with_capacity(line.len());
                rest.push_str(&line[..i]);
                rest.push_str(&line[end..]);
                return Some((owner, rest));
            }
        }
        i += 1;
    }
    None
}

/// Detect a leading "Name will/to/should … <task>" assignment.
/// Returns (owner, task) when the pattern matches.
fn extract_leading_assignment(line: &str) -> Option<(String, String)> {
    let words: Vec<&str> = line.split_whitespace().collect();
    if words.len() < 3 {
        return None;
    }
    // Consider a 1- or 2-word leading name (2 first, so "Mary Jane will" wins).
    for name_len in [2usize, 1usize] {
        if words.len() < name_len + 2 {
            continue;
        }
        let name = words[..name_len].join(" ");
        if !looks_like_name(&name) {
            continue;
        }
        // Try connectors of 2 or 1 words right after the name.
        for conn_len in [2usize, 1usize] {
            if words.len() < name_len + conn_len + 1 {
                continue;
            }
            let conn = words[name_len..name_len + conn_len].join(" ").to_lowercase();
            if ASSIGN_CONNECTORS.contains(&conn.as_str()) {
                let task = words[name_len + conn_len..].join(" ");
                let task = clean_task(&task);
                if !task.is_empty() {
                    return Some((name, task));
                }
            }
        }
    }
    None
}

/// Detect an explicit `owner:`/`assignee:`/`assigned to` marker.
fn extract_owner_prefix(line: &str) -> Option<(String, String)> {
    let low = line.to_lowercase();
    for marker in ["owner:", "assignee:", "assigned to:", "assigned to"] {
        if let Some(pos) = low.find(marker) {
            let after = &line[pos + marker.len()..];
            let after_t = after.trim_start();
            let mut parts = after_t.splitn(2, |c: char| {
                matches!(c, ',' | '-' | '–' | '—' | ':')
            });
            let owner_part = parts.next().unwrap_or("").trim();
            let ow: Vec<&str> = owner_part.split_whitespace().take(2).collect();
            let owner = ow.join(" ");
            if !owner.is_empty() {
                let task_part = parts.next().unwrap_or("");
                let before = line[..pos].trim();
                let task = if !task_part.trim().is_empty() {
                    clean_task(task_part)
                } else {
                    clean_task(before)
                };
                if !task.is_empty() {
                    return Some((owner, task));
                }
            }
        }
    }
    None
}

/// Detect a trailing owner tag: `— Alice`, `(Alice)`, `[Alice]`, `-- Bob`.
fn extract_trailing_owner(line: &str) -> Option<(String, String)> {
    let t = line.trim_end();
    // bracketed forms
    for (open, close) in [('(', ')'), ('[', ']')] {
        if t.ends_with(close) {
            if let Some(op) = t.rfind(open) {
                let inner = t[op + 1..t.len() - close.len_utf8()].trim();
                let ow: Vec<&str> = inner.split_whitespace().take(2).collect();
                let owner = ow.join(" ");
                if looks_like_name(&owner) {
                    let task = clean_task(&t[..op]);
                    if !task.is_empty() {
                        return Some((owner, task));
                    }
                }
            }
        }
    }
    // dash-suffixed forms: "task — Alice" / "task -- Bob" / "task – Carol"
    for sep in [" — ", " -- ", " – ", " - "] {
        if let Some(pos) = t.rfind(sep) {
            let tail = t[pos + sep.len()..].trim();
            let ow: Vec<&str> = tail.split_whitespace().take(2).collect();
            let owner = ow.join(" ");
            // only treat as owner when the tail is a short name, not a clause
            if tail.split_whitespace().count() <= 2 && looks_like_name(&owner) {
                let task = clean_task(&t[..pos]);
                if !task.is_empty() {
                    return Some((owner, task));
                }
            }
        }
    }
    None
}

/// Given a (marker-stripped) line, find an owner and return the cleaned task.
fn detect_owner(line: &str) -> (Option<String>, String) {
    if let Some((owner, rest)) = extract_at_handle(line) {
        return (Some(title_case_first(&owner)), clean_task(&rest));
    }
    if let Some((owner, task)) = extract_owner_prefix(line) {
        return (Some(owner), task);
    }
    if let Some((owner, task)) = extract_leading_assignment(line) {
        return (Some(owner), task);
    }
    if let Some((owner, task)) = extract_trailing_owner(line) {
        return (Some(owner), task);
    }
    (None, clean_task(line))
}

/// Strip a leading explicit action marker (`Action:`, `TODO:`, …) if present,
/// returning the remaining task text.
fn strip_action_marker(line: &str) -> Option<String> {
    let low = line.to_lowercase();
    for m in ACTION_MARKERS {
        if low.starts_with(m) {
            return Some(line[m.len()..].to_string());
        }
    }
    None
}

/// If `text` begins with a decision word/phrase at a word boundary, return the
/// remainder after it (longest phrase wins, so `signed off` beats `sign off`).
fn strip_leading_decision(text: &str) -> Option<&str> {
    let t = text.trim_start();
    let low = t.to_lowercase();
    let mut words: Vec<&&str> = DECISION_WORDS.iter().collect();
    words.sort_by(|a, b| b.len().cmp(&a.len()));
    for w in words {
        if low.starts_with(*w) {
            let after = &t[w.len()..];
            let boundary = after.chars().next().map_or(true, |c| !is_word_char(c));
            if boundary {
                return Some(after);
            }
        }
    }
    None
}

/// Drop the connective punctuation / small words that trail a decision marker
/// ("decided **:** ship", "agreed **to** drop", "decided **we will** ship").
fn finalize_decision(body: &str, original: &str) -> String {
    let mut b = body.trim();
    loop {
        let bl = b.to_lowercase();
        let mut advanced = false;
        for conn in [
            ":", "-", "–", "—", "that ", "to ", "on ", "we'll ", "we will ", "we ", "will ",
        ] {
            if bl.starts_with(conn) {
                b = b[conn.len()..].trim_start();
                advanced = true;
                break;
            }
        }
        if !advanced {
            break;
        }
    }
    let cleaned = clean_task(b);
    if cleaned.is_empty() {
        clean_task(original)
    } else {
        cleaned
    }
}

/// Turn a decision line into clean decision text by removing the leading marker.
/// `Decided: ship Friday` → `ship Friday`; `We agreed to drop X` → `drop X`.
fn strip_decision_prefix(line: &str) -> String {
    let trimmed = line.trim();
    // A marker at the very start (covers phrase markers like "we will go with").
    if let Some(body) = strip_leading_decision(trimmed) {
        return finalize_decision(body, line);
    }
    // Otherwise skip a leading collective subject and try once more.
    let low = trimmed.to_lowercase();
    for lead in ["we've ", "we have ", "we ", "i ", "team ", "the team "] {
        if low.starts_with(lead) {
            if let Some(body) = strip_leading_decision(&trimmed[lead.len()..]) {
                return finalize_decision(body, line);
            }
        }
    }
    clean_task(line)
}

fn starts_with_action_verb(line_lower: &str) -> bool {
    ACTION_VERBS.iter().any(|v| {
        line_lower == *v
            || line_lower.starts_with(&format!("{v} "))
            || line_lower.starts_with(&format!("{v}s "))
            || line_lower.starts_with(&format!("{v}: "))
    })
}

/// Classify one already-marker-stripped, non-empty line.
fn classify(line: &str) -> Kind {
    let low = line.to_lowercase();
    let has_handle = line.contains('@') && extract_at_handle(line).is_some();

    // 1. Explicit action marker or @handle → action item.
    if let Some(rest) = strip_action_marker(line) {
        let (owner, task) = detect_owner(rest.trim());
        if !task.is_empty() {
            return Kind::Action { owner, task };
        }
    }
    if has_handle {
        let (owner, task) = detect_owner(line);
        if !task.is_empty() {
            return Kind::Action { owner, task };
        }
    }

    // 2. Decision markers (only when there's no explicit action signal above).
    if DECISION_WORDS
        .iter()
        .any(|w| contains_stem(&low, w) || contains_phrase(&low, w))
    {
        let text = strip_decision_prefix(line);
        if !text.is_empty() {
            return Kind::Decision(text);
        }
    }

    // 3. Leading-name assignment ("Alice will …").
    if let Some((owner, task)) = extract_leading_assignment(line) {
        return Kind::Action {
            owner: Some(owner),
            task,
        };
    }

    // 4. Trailing owner tag.
    if let Some((owner, task)) = extract_trailing_owner(line) {
        return Kind::Action {
            owner: Some(owner),
            task,
        };
    }

    // 5. Bare imperative starting with an action verb.
    if starts_with_action_verb(&low) {
        return Kind::Action {
            owner: None,
            task: clean_task(line),
        };
    }

    Kind::Note
}

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

struct ActionItem {
    task: String,
    owner: Option<String>,
}

/// The single-input entry point used by the chat skill, CLI and web page.
///
/// Takes freeform meeting or daily `notes` (one item per line) and returns a
/// structured Markdown task list: an **Action Items** checklist (each item with
/// its detected owner, or _Unassigned_) followed by a **Decisions** list. Every
/// item comes from an explicit signal in the text — an owner name, `@handle`,
/// action marker/verb, or decision word — so nothing is invented. Errors when
/// no action item or decision can be found.
pub fn run(notes: &str) -> Result<String, String> {
    extract(notes, "markdown", "type", true)
}

/// Extract action items + decisions from freeform meeting/daily `notes`.
///
/// * `format` — `"markdown"` (default) renders a checklist + decisions list;
///   `"json"` emits a compact machine-readable object.
/// * `group_by` — `"type"` (default) keeps a single Action Items section;
///   `"owner"` groups action items under a heading per owner (Unassigned last).
/// * `include_decisions` — when true (default), include the Decisions section.
pub fn extract(
    notes: &str,
    format: &str,
    group_by: &str,
    include_decisions: bool,
) -> Result<String, String> {
    let mut actions: Vec<ActionItem> = Vec::new();
    let mut decisions: Vec<String> = Vec::new();

    for raw in notes.lines() {
        let stripped = strip_marker(raw);
        if stripped.is_empty() {
            continue;
        }
        match classify(stripped) {
            Kind::Action { owner, task } => {
                if !task.is_empty() {
                    actions.push(ActionItem { task, owner });
                }
            }
            Kind::Decision(text) => {
                if include_decisions {
                    decisions.push(text);
                }
            }
            Kind::Note => {}
        }
    }

    if actions.is_empty() && decisions.is_empty() {
        return Err(
            "no action items or decisions found — give one note per line (e.g. \"Alice will send the deck\", \"Decided: ship on Friday\")".into(),
        );
    }

    match format {
        "json" => Ok(render_json(&actions, &decisions, include_decisions)),
        "markdown" | "" => Ok(render_markdown(&actions, &decisions, group_by, include_decisions)),
        other => Err(format!(
            "invalid format {other:?}: expected \"markdown\" or \"json\""
        )),
    }
}

fn render_json(actions: &[ActionItem], decisions: &[String], include_decisions: bool) -> String {
    let items: Vec<String> = actions
        .iter()
        .map(|a| {
            let owner = match &a.owner {
                Some(o) => format!("\"{}\"", json_escape(o)),
                None => "null".to_string(),
            };
            format!(
                "{{\"task\":\"{}\",\"owner\":{}}}",
                json_escape(&a.task),
                owner
            )
        })
        .collect();
    if include_decisions {
        let decs: Vec<String> = decisions
            .iter()
            .map(|d| format!("\"{}\"", json_escape(d)))
            .collect();
        format!(
            "{{\"action_items\":[{}],\"decisions\":[{}]}}",
            items.join(","),
            decs.join(",")
        )
    } else {
        format!("{{\"action_items\":[{}]}}", items.join(","))
    }
}

fn render_markdown(
    actions: &[ActionItem],
    decisions: &[String],
    group_by: &str,
    include_decisions: bool,
) -> String {
    let mut out = String::new();

    if !actions.is_empty() {
        out.push_str("## Action Items\n\n");
        if group_by == "owner" {
            // Preserve first-seen owner order; Unassigned last.
            let mut order: Vec<String> = Vec::new();
            for a in actions {
                let key = a.owner.clone().unwrap_or_default();
                if !order.contains(&key) {
                    order.push(key);
                }
            }
            let named: Vec<&String> = order.iter().filter(|k| !k.is_empty()).collect();
            let has_unassigned = order.iter().any(|k| k.is_empty());
            for owner in named {
                out.push_str(&format!("### {owner}\n\n"));
                for a in actions
                    .iter()
                    .filter(|a| a.owner.as_deref() == Some(owner.as_str()))
                {
                    out.push_str(&format!("- [ ] {}\n", a.task));
                }
                out.push('\n');
            }
            if has_unassigned {
                out.push_str("### Unassigned\n\n");
                for a in actions.iter().filter(|a| a.owner.is_none()) {
                    out.push_str(&format!("- [ ] {}\n", a.task));
                }
                out.push('\n');
            }
        } else {
            for a in actions {
                match &a.owner {
                    Some(o) => out.push_str(&format!("- [ ] {} — **@{}**\n", a.task, o)),
                    None => out.push_str(&format!("- [ ] {} — _Unassigned_\n", a.task)),
                }
            }
            out.push('\n');
        }
    }

    if include_decisions && !decisions.is_empty() {
        out.push_str("## Decisions\n\n");
        for d in decisions {
            out.push_str(&format!("- {d}\n"));
        }
        out.push('\n');
    }

    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_owner_from_leading_assignment() {
        let out = extract("Alice will send the budget draft", "markdown", "type", true).unwrap();
        assert!(out.contains("## Action Items"));
        assert!(
            out.contains("- [ ] send the budget draft — **@Alice**"),
            "got: {out}"
        );
    }

    #[test]
    fn extracts_at_handle_owner() {
        let out = extract("Book the venue @bob", "markdown", "type", true).unwrap();
        assert!(out.contains("- [ ] Book the venue — **@Bob**"), "got: {out}");
    }

    #[test]
    fn action_marker_prefix() {
        let out = extract("ACTION: update the roadmap", "markdown", "type", true).unwrap();
        assert!(
            out.contains("- [ ] update the roadmap — _Unassigned_"),
            "got: {out}"
        );
    }

    #[test]
    fn detects_decision() {
        let out = extract("Decided: ship v2 on Friday", "markdown", "type", true).unwrap();
        assert!(out.contains("## Decisions"), "got: {out}");
        assert!(out.contains("- ship v2 on Friday"), "got: {out}");
    }

    #[test]
    fn agreed_is_a_decision() {
        let out = extract("We agreed to drop the legacy API", "markdown", "type", true).unwrap();
        assert!(out.contains("## Decisions"));
        assert!(out.contains("drop the legacy API"), "got: {out}");
    }

    #[test]
    fn bare_imperative_verb_is_action() {
        let out = extract("Schedule the retro for next week", "markdown", "type", true).unwrap();
        assert!(
            out.contains("- [ ] Schedule the retro for next week — _Unassigned_"),
            "got: {out}"
        );
    }

    #[test]
    fn trailing_owner_tag() {
        let out = extract("Prepare the slides (Carol)", "markdown", "type", true).unwrap();
        assert!(
            out.contains("- [ ] Prepare the slides — **@Carol**"),
            "got: {out}"
        );
    }

    #[test]
    fn plain_note_is_ignored() {
        let out = extract(
            "The weather was nice today\nAlice will book the room",
            "markdown",
            "type",
            true,
        )
        .unwrap();
        assert!(!out.contains("weather"), "got: {out}");
        assert!(out.contains("book the room"));
    }

    #[test]
    fn group_by_owner() {
        let out = extract(
            "Alice will send the deck\nBob to book the venue\nUpdate the wiki",
            "markdown",
            "owner",
            true,
        )
        .unwrap();
        let alice = out.find("### Alice").unwrap();
        let bob = out.find("### Bob").unwrap();
        let un = out.find("### Unassigned").unwrap();
        assert!(alice < bob && bob < un, "owners then unassigned: {out}");
        assert!(out.contains("- [ ] send the deck"));
    }

    #[test]
    fn json_format_is_machine_readable() {
        let out =
            extract("Alice will send the deck\nDecided: ship Friday", "json", "type", true).unwrap();
        assert!(
            out.contains("\"action_items\":[{\"task\":\"send the deck\",\"owner\":\"Alice\"}]"),
            "got: {out}"
        );
        assert!(out.contains("\"decisions\":[\"ship Friday\"]"), "got: {out}");
    }

    #[test]
    fn json_escapes_special_chars() {
        let out = extract("Send the \"final\" report @dan", "json", "type", true).unwrap();
        assert!(out.contains("\\\"final\\\""), "got: {out}");
    }

    #[test]
    fn include_decisions_false_hides_them() {
        let out =
            extract("Decided: ship Friday\nAlice will test", "markdown", "type", false).unwrap();
        assert!(!out.contains("Decisions"), "got: {out}");
        assert!(out.contains("test"));
    }

    #[test]
    fn strips_existing_checkbox_markers() {
        let out = extract("- [ ] Email the client @sam", "markdown", "type", true).unwrap();
        assert!(
            out.contains("- [ ] Email the client — **@Sam**"),
            "got: {out}"
        );
        assert!(!out.contains("[ ] - [ ]"));
    }

    #[test]
    fn we_is_not_an_owner() {
        // "We will ..." must not treat "We" as a person owner.
        let out = extract("We will revisit pricing", "markdown", "type", true);
        // "revisit" isn't a listed verb and "We" isn't an owner, so nothing
        // actionable is found → error, and "We" is never surfaced as an owner.
        assert!(out.is_err() || !out.unwrap().contains("**@We**"));
    }

    #[test]
    fn empty_input_errors() {
        assert!(extract("   \n  \n", "markdown", "type", true).is_err());
        assert!(extract(
            "just some prose with nothing actionable here",
            "markdown",
            "type",
            true
        )
        .is_err());
    }

    #[test]
    fn invalid_format_errors() {
        assert!(extract("Alice will test", "xml", "type", true).is_err());
    }

    #[test]
    fn run_is_markdown_with_owners_and_decisions() {
        let out = run("TODO: update the roadmap\nAlice will send the deck\nBook the venue @bob\nDecided: ship on Friday").unwrap();
        assert_eq!(
            out,
            "## Action Items\n\n\
             - [ ] update the roadmap — _Unassigned_\n\
             - [ ] send the deck — **@Alice**\n\
             - [ ] Book the venue — **@Bob**\n\n\
             ## Decisions\n\n\
             - ship on Friday",
            "got: {out}"
        );
    }

    #[test]
    fn run_errors_on_nothing_actionable() {
        assert!(run("just some prose here").is_err());
    }
}

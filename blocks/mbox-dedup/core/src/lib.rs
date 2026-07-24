//! gizza-ai/mbox-dedup core — remove duplicate messages from an mbox archive by
//! their `Message-ID` header, keeping the first or last occurrence and preserving
//! the surviving messages verbatim (postmark lines and all). Pure-Rust, shared by
//! the chat skill block and the web page. Depends only on serde/serde_json.

use std::collections::HashSet;

use serde::Serialize;

/// Which occurrence of a duplicated Message-ID to keep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keep {
    /// Keep the first occurrence, drop later duplicates (default).
    First,
    /// Keep the last occurrence, drop earlier duplicates.
    Last,
}

impl Keep {
    pub fn parse(s: &str) -> Result<Keep, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "first" | "" => Ok(Keep::First),
            "last" => Ok(Keep::Last),
            other => Err(format!("keep must be \"first\" or \"last\", got \"{other}\"")),
        }
    }
}

/// What to do with a message that has no `Message-ID` header (drafts, some
/// mailing-list output). These can't be de-duplicated by ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoId {
    /// Keep every message that lacks a Message-ID (default). Distinct ID-less
    /// messages are never collapsed into one.
    Keep,
    /// Drop every message that lacks a Message-ID.
    Drop,
}

impl NoId {
    pub fn parse(s: &str) -> Result<NoId, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "keep" | "" => Ok(NoId::Keep),
            "drop" => Ok(NoId::Drop),
            other => Err(format!("no_message_id must be \"keep\" or \"drop\", got \"{other}\"")),
        }
    }
}

/// Options controlling how duplicate messages are detected and removed.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Which occurrence of a duplicated Message-ID to keep.
    pub keep: Keep,
    /// Compare Message-IDs case-insensitively. RFC 5322 Message-IDs are
    /// case-sensitive, so this is off by default.
    pub ignore_case: bool,
    /// How to handle messages that have no Message-ID header.
    pub no_id: NoId,
}

impl Default for Options {
    fn default() -> Self {
        Options { keep: Keep::First, ignore_case: false, no_id: NoId::Keep }
    }
}

/// Result of a de-duplication run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Output {
    /// The de-duplicated mbox text (surviving messages, verbatim, in order).
    pub text: String,
    /// Number of messages found in the input.
    pub total_messages: usize,
    /// Number of messages kept in the output.
    pub kept_messages: usize,
    /// Number of messages removed (duplicate Message-IDs + any dropped ID-less).
    pub removed_messages: usize,
    /// How many input messages had no Message-ID header.
    pub messages_without_id: usize,
    /// How many distinct Message-IDs appeared more than once.
    pub duplicate_ids: usize,
}

/// Split a raw mbox blob into individual message blocks, VERBATIM.
///
/// mbox delimits messages with a `From ` "postmark" line at column 0 (the space
/// after `From` distinguishes it from a `From:` header). Each returned slice runs
/// from one postmark line up to (but not including) the next — so it carries the
/// postmark, the RFC 5322 message, and the trailing blank-line separator exactly
/// as written. Concatenating a subset of these slices therefore reproduces a
/// valid mbox with no byte fiddling. A blob with no postmark is treated as a
/// single message so a lone `.eml` still works; any non-blank text before the
/// first postmark becomes its own leading message.
pub fn split_blocks(raw: &str) -> Vec<&str> {
    // Byte offsets of every `From ` postmark line (start-of-line only).
    let mut starts: Vec<usize> = Vec::new();
    let mut offset = 0usize;
    for line in raw.split_inclusive('\n') {
        if line.starts_with("From ") {
            starts.push(offset);
        }
        offset += line.len();
    }

    let mut blocks: Vec<&str> = Vec::new();
    if starts.is_empty() {
        if !raw.trim().is_empty() {
            blocks.push(raw);
        }
        return blocks;
    }

    // Text before the first postmark (a lone message pasted without a postmark).
    let first = starts[0];
    if first > 0 && !raw[..first].trim().is_empty() {
        blocks.push(&raw[..first]);
    }

    for (i, &s) in starts.iter().enumerate() {
        let end = starts.get(i + 1).copied().unwrap_or(raw.len());
        blocks.push(&raw[s..end]);
    }
    blocks
}

/// Extract and normalize the `Message-ID` header of one message block.
///
/// Scans the header section (up to the first blank line), unfolding RFC 5322
/// continuation lines, matching the header name case-insensitively. The value is
/// trimmed and one surrounding `<…>` pair is stripped so `<id@host>` and
/// ` id@host ` compare equal; `ignore_case` additionally lowercases it. A postmark
/// line at the top of the block never matches (`From ` has no colon-name form of
/// `message-id`). Returns `None` when the message has no Message-ID.
pub fn message_id(block: &str, ignore_case: bool) -> Option<String> {
    let end = block
        .find("\r\n\r\n")
        .or_else(|| block.find("\n\n"))
        .unwrap_or(block.len());
    let headers = &block[..end];

    let mut acc: Option<String> = None;
    for line in headers.lines() {
        let is_continuation = line.starts_with(' ') || line.starts_with('\t');
        if is_continuation {
            if let Some(a) = acc.as_mut() {
                a.push(' ');
                a.push_str(line.trim());
            }
            continue;
        }
        // A new header line: if we were collecting Message-ID, we're done.
        if acc.is_some() {
            break;
        }
        if let Some(idx) = line.find(':') {
            if line[..idx].trim().eq_ignore_ascii_case("message-id") {
                acc = Some(line[idx + 1..].trim().to_string());
            }
        }
    }

    let raw = acc?;
    let t = raw.trim();
    let core = if t.starts_with('<') && t.ends_with('>') && t.len() >= 2 {
        t[1..t.len() - 1].trim()
    } else {
        t
    };
    if core.is_empty() {
        return None;
    }
    Some(if ignore_case { core.to_ascii_lowercase() } else { core.to_string() })
}

/// Remove duplicate messages from `data` (an mbox blob) by Message-ID.
pub fn dedupe(data: &str, opts: &Options) -> Output {
    let blocks = split_blocks(data);
    let total_messages = blocks.len();

    let ids: Vec<Option<String>> = blocks.iter().map(|b| message_id(b, opts.ignore_case)).collect();
    let messages_without_id = ids.iter().filter(|o| o.is_none()).count();

    let mut keep_flags = vec![false; blocks.len()];

    // ID-less messages: kept or dropped wholesale, never collapsed together.
    for (i, id) in ids.iter().enumerate() {
        if id.is_none() {
            keep_flags[i] = matches!(opts.no_id, NoId::Keep);
        }
    }

    // Count how many IDs appear more than once (for the summary).
    let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for id in ids.iter().flatten() {
        *counts.entry(id.as_str()).or_insert(0) += 1;
    }
    let duplicate_ids = counts.values().filter(|&&c| c > 1).count();

    // Messages with an ID: keep only the first (or last) occurrence of each ID.
    let mut seen: HashSet<&str> = HashSet::new();
    let order: Vec<usize> = if opts.keep == Keep::Last {
        (0..blocks.len()).rev().collect()
    } else {
        (0..blocks.len()).collect()
    };
    for &i in &order {
        if let Some(id) = &ids[i] {
            if seen.insert(id.as_str()) {
                keep_flags[i] = true;
            }
        }
    }

    // Emit surviving blocks in original order — verbatim reconstruction.
    let mut text = String::new();
    let mut kept_messages = 0usize;
    for (i, block) in blocks.iter().enumerate() {
        if keep_flags[i] {
            text.push_str(block);
            kept_messages += 1;
        }
    }

    Output {
        text,
        total_messages,
        kept_messages,
        removed_messages: total_messages - kept_messages,
        messages_without_id,
        duplicate_ids,
    }
}

/// Web/page entry: build [`Options`] from raw string fields (order matches the
/// descriptor / meta.toml) and return the de-duplicated mbox text.
pub fn run(data: &str, keep: &str, ignore_case: bool, no_message_id: &str) -> Result<String, String> {
    let opts = Options {
        keep: Keep::parse(keep)?,
        ignore_case,
        no_id: NoId::parse(no_message_id)?,
    };
    Ok(dedupe(data, &opts).text)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Two messages with the SAME Message-ID (k1) plus a distinct one (k2).
    const DUP: &str = "From a@x Mon Sep 03 10:00:00 2018\r\nFrom: Alice <alice@example.com>\r\nSubject: Hi\r\nMessage-ID: <k1@example.com>\r\n\r\nfirst copy\r\n\r\nFrom b@x Mon Sep 03 11:00:00 2018\r\nFrom: Bob <bob@example.org>\r\nSubject: Yo\r\nMessage-ID: <k2@example.com>\r\n\r\nsecond message\r\n\r\nFrom c@x Mon Sep 03 12:00:00 2018\r\nFrom: Alice <alice@example.com>\r\nSubject: Hi again\r\nMessage-ID: <k1@example.com>\r\n\r\nduplicate of first\r\n";

    #[test]
    fn dedupe_keep_first() {
        let o = dedupe(DUP, &Options::default());
        assert_eq!(o.total_messages, 3);
        assert_eq!(o.kept_messages, 2);
        assert_eq!(o.removed_messages, 1);
        assert_eq!(o.duplicate_ids, 1);
        assert_eq!(o.messages_without_id, 0);
        // first copy of k1 kept, third message (dup) removed
        assert!(o.text.contains("first copy"));
        assert!(o.text.contains("second message"));
        assert!(!o.text.contains("duplicate of first"));
    }

    #[test]
    fn dedupe_keep_last() {
        let mut opts = Options::default();
        opts.keep = Keep::Last;
        let o = dedupe(DUP, &opts);
        assert_eq!(o.kept_messages, 2);
        // last copy of k1 kept in its final position
        assert!(o.text.contains("duplicate of first"));
        assert!(!o.text.contains("first copy"));
        assert!(o.text.contains("second message"));
        // order preserved: k2 message comes before the surviving k1
        let ci = o.text.find("second message").unwrap();
        let ki = o.text.find("duplicate of first").unwrap();
        assert!(ci < ki);
    }

    #[test]
    fn output_reconstructs_valid_mbox() {
        // Kept output is the exact concatenation of surviving verbatim blocks.
        let o = dedupe(DUP, &Options::default());
        let expected = "From a@x Mon Sep 03 10:00:00 2018\r\nFrom: Alice <alice@example.com>\r\nSubject: Hi\r\nMessage-ID: <k1@example.com>\r\n\r\nfirst copy\r\n\r\nFrom b@x Mon Sep 03 11:00:00 2018\r\nFrom: Bob <bob@example.org>\r\nSubject: Yo\r\nMessage-ID: <k2@example.com>\r\n\r\nsecond message\r\n\r\n";
        assert_eq!(o.text, expected);
    }

    #[test]
    fn message_id_normalizes_brackets_and_whitespace() {
        assert_eq!(message_id("Message-ID: <abc@h>\n\nbody", false).as_deref(), Some("abc@h"));
        assert_eq!(message_id("Message-ID:   abc@h  \n\nbody", false).as_deref(), Some("abc@h"));
    }

    #[test]
    fn message_id_case_sensitive_by_default() {
        let data = "From a\nMessage-ID: <A@H>\n\none\n\nFrom b\nMessage-ID: <a@h>\n\ntwo\n";
        let o = dedupe(data, &Options::default());
        // different case → distinct IDs → both kept
        assert_eq!(o.kept_messages, 2);
        assert_eq!(o.duplicate_ids, 0);
    }

    #[test]
    fn ignore_case_collapses_id_case() {
        let data = "From a\nMessage-ID: <A@H>\n\none\n\nFrom b\nMessage-ID: <a@h>\n\ntwo\n";
        let mut opts = Options::default();
        opts.ignore_case = true;
        let o = dedupe(data, &opts);
        assert_eq!(o.kept_messages, 1);
        assert_eq!(o.duplicate_ids, 1);
    }

    #[test]
    fn folded_message_id_header() {
        // Message-ID value continued on a folded line.
        let block = "Subject: x\r\nMessage-ID:\r\n <folded@example.com>\r\n\r\nbody";
        assert_eq!(message_id(block, false).as_deref(), Some("folded@example.com"));
    }

    #[test]
    fn no_id_kept_by_default() {
        // Two ID-less messages plus one with an ID: all kept, none collapsed.
        let data = "From a\nSubject: draft one\n\nno id here\n\nFrom b\nSubject: draft two\n\nalso no id\n\nFrom c\nMessage-ID: <z@h>\n\nhas id\n";
        let o = dedupe(data, &Options::default());
        assert_eq!(o.total_messages, 3);
        assert_eq!(o.messages_without_id, 2);
        assert_eq!(o.kept_messages, 3);
    }

    #[test]
    fn no_id_dropped_when_requested() {
        let data = "From a\nSubject: draft\n\nno id here\n\nFrom c\nMessage-ID: <z@h>\n\nhas id\n";
        let mut opts = Options::default();
        opts.no_id = NoId::Drop;
        let o = dedupe(data, &opts);
        assert_eq!(o.messages_without_id, 1);
        assert_eq!(o.kept_messages, 1);
        assert!(o.text.contains("has id"));
        assert!(!o.text.contains("no id here"));
    }

    #[test]
    fn lone_eml_without_postmark() {
        let data = "Subject: solo\r\nMessage-ID: <solo@h>\r\n\r\njust one message\r\n";
        let o = dedupe(data, &Options::default());
        assert_eq!(o.total_messages, 1);
        assert_eq!(o.kept_messages, 1);
        assert_eq!(o.text, data);
    }

    #[test]
    fn empty_input() {
        let o = dedupe("", &Options::default());
        assert_eq!(o.total_messages, 0);
        assert_eq!(o.kept_messages, 0);
        assert_eq!(o.removed_messages, 0);
        assert_eq!(o.text, "");
    }

    #[test]
    fn run_wires_fields() {
        let out = run(DUP, "first", false, "keep").unwrap();
        assert!(out.contains("first copy"));
        assert!(!out.contains("duplicate of first"));
    }

    #[test]
    fn keep_parse() {
        assert_eq!(Keep::parse("first").unwrap(), Keep::First);
        assert_eq!(Keep::parse("LAST").unwrap(), Keep::Last);
        assert_eq!(Keep::parse("").unwrap(), Keep::First);
        assert!(Keep::parse("middle").is_err());
    }

    #[test]
    fn no_id_parse() {
        assert_eq!(NoId::parse("keep").unwrap(), NoId::Keep);
        assert_eq!(NoId::parse("DROP").unwrap(), NoId::Drop);
        assert_eq!(NoId::parse("").unwrap(), NoId::Keep);
        assert!(NoId::parse("bogus").is_err());
    }
}

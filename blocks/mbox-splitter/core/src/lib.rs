//! gizza-ai/mbox-splitter core — split a multi-message mbox archive into the
//! individual RFC 5322 messages it contains (one `.eml` each), with a suggested
//! filename per message. Pure-Rust (`mail-parser` for decoded subjects/dates),
//! wasm-safe, no wafer/wasm-bindgen deps.
//!
//! mbox delimits messages with a `From ` "postmark" line at column 0 — the space
//! after `From` is what distinguishes it from a `From:` header. Message bytes are
//! sliced out verbatim (headers, MIME structure and base64 attachments are never
//! re-serialized) so each piece is a faithful `.eml`.

use std::collections::HashMap;

use mail_parser::MessageParser;
use serde::Serialize;

/// Hard cap on how many messages one run will split out. Guards the page/chat
/// surfaces against a pasted multi-gigabyte export.
pub const MAX_MESSAGES: usize = 2000;

/// What to emit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    /// Every message, each under a `===== NNN-name.eml (N bytes) =====` header.
    Files,
    /// A numbered index table: filename, date, from, subject, size.
    List,
    /// JSON array of `{ filename, subject, from, date, bytes, eml }`.
    Json,
    /// One raw `.eml`, nothing else (use with `message`).
    Eml,
}

impl Output {
    pub fn parse(s: &str) -> Result<Output, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "files" | "" => Ok(Output::Files),
            "list" => Ok(Output::List),
            "json" => Ok(Output::Json),
            "eml" => Ok(Output::Eml),
            other => Err(format!(
                "output must be \"files\", \"list\", \"json\" or \"eml\", got \"{other}\""
            )),
        }
    }
}

/// How each message's suggested filename is built. Every scheme keeps a 1-based
/// index prefix so the original archive order and uniqueness survive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Naming {
    /// `001.eml`
    Index,
    /// `001-quarterly-report.eml`
    Subject,
    /// `001-2018-09-03-1000.eml`
    Date,
    /// `001-a1-example-com.eml`
    MessageId,
}

impl Naming {
    pub fn parse(s: &str) -> Result<Naming, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "index" | "" => Ok(Naming::Index),
            "subject" => Ok(Naming::Subject),
            "date" => Ok(Naming::Date),
            "message-id" | "message_id" => Ok(Naming::MessageId),
            other => Err(format!(
                "naming must be \"index\", \"subject\", \"date\" or \"message-id\", got \"{other}\""
            )),
        }
    }
}

/// Options controlling the split.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    pub output: Output,
    pub naming: Naming,
    /// 1-based message to isolate; `0` = every message.
    pub message: usize,
    /// Undo mbox `>From ` body quoting (mboxo/mboxrd) so bodies match the
    /// original message.
    pub unescape_from: bool,
    /// Keep the `From ` postmark line at the top of each piece. Off by default —
    /// a `.eml` is a bare RFC 5322 message.
    pub keep_postmark: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            output: Output::Files,
            naming: Naming::Index,
            message: 0,
            unescape_from: true,
            keep_postmark: false,
        }
    }
}

/// One split-out message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Message {
    /// 1-based position in the archive.
    pub index: usize,
    pub filename: String,
    pub subject: String,
    pub from: String,
    pub date: String,
    /// Byte length of `eml`.
    pub bytes: usize,
    /// The message itself — a ready-to-save `.eml`.
    pub eml: String,
}

/// A raw message block as it appears in the archive: its postmark line (if any)
/// and everything after it, up to the next postmark.
struct Block {
    postmark: Option<String>,
    body: String,
}

/// True for a line that opens a new mbox message: `From ` at column 0. A
/// `From:` header line is NOT a postmark (no space after `From`).
fn is_postmark(line: &str) -> bool {
    line.starts_with("From ")
}

/// Split a raw mbox blob into per-message blocks. A blob with no postmark at all
/// is treated as a single message, so a lone `.eml` still works.
fn split_blocks(raw: &str) -> Vec<Block> {
    let mut blocks: Vec<Block> = Vec::new();
    let mut cur = String::new();
    let mut postmark: Option<String> = None;
    let mut started = false;

    for line in raw.split_inclusive('\n') {
        if is_postmark(line) {
            if started && !cur.trim().is_empty() {
                blocks.push(Block { postmark: postmark.take(), body: std::mem::take(&mut cur) });
            } else {
                cur.clear();
            }
            postmark = Some(line.to_string());
            started = true;
            continue;
        }
        if started {
            cur.push_str(line);
        }
    }
    if started && !cur.trim().is_empty() {
        blocks.push(Block { postmark, body: cur });
    }
    if blocks.is_empty() && !raw.trim().is_empty() {
        blocks.push(Block { postmark: None, body: raw.to_string() });
    }
    blocks
}

/// Undo mbox body quoting: `>From ` → `From `, `>>From ` → `>From `, … Only
/// lines that are one or more `>` followed by `From ` are touched, which is
/// exactly what mboxo/mboxrd writers escape.
fn unescape_from_lines(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let gts = line.chars().take_while(|&c| c == '>').count();
        if gts >= 1 && line[gts..].starts_with("From ") {
            out.push_str(&line[1..]);
        } else {
            out.push_str(line);
        }
    }
    out
}

/// Filename-safe slug: lowercase alphanumerics, everything else collapsed to a
/// single `-`. Empty input yields `message`.
fn slug(text: &str, max: usize) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for c in text.trim().chars() {
        if c.is_alphanumeric() && c.is_ascii() {
            out.extend(c.to_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
        if out.trim_matches('-').len() >= max {
            break;
        }
    }
    let s = out.trim_matches('-').to_string();
    if s.is_empty() {
        "message".to_string()
    } else {
        s
    }
}

/// Collapse whitespace runs to single spaces and trim.
fn tidy(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Header fields we surface, decoded via `mail-parser` (RFC 2047 encoded words
/// in subjects become readable text).
struct Headers {
    subject: String,
    from: String,
    date: String,
    message_id: String,
}

fn headers_of(eml: &str) -> Headers {
    let parsed = MessageParser::default().parse(eml.as_bytes());
    let subject = parsed
        .as_ref()
        .and_then(|m| m.subject())
        .map(tidy)
        .unwrap_or_default();
    let from = parsed
        .as_ref()
        .and_then(|m| m.from())
        .and_then(|a| {
            a.iter()
                .find_map(|m| m.address().map(|s| s.to_string()).or_else(|| m.name().map(|s| s.to_string())))
        })
        .unwrap_or_default();
    let date = parsed
        .as_ref()
        .and_then(|m| m.date())
        .map(|d| d.to_rfc3339())
        .unwrap_or_default();
    let message_id = parsed
        .as_ref()
        .and_then(|m| m.message_id())
        .map(|s| s.trim().trim_matches(|c| c == '<' || c == '>').to_string())
        .unwrap_or_default();
    Headers { subject, from, date, message_id }
}

/// Compact `YYYY-MM-DD-HHMM` stamp from an RFC 3339 date, for `naming = date`.
fn date_stamp(rfc3339: &str) -> String {
    // 2018-09-03T10:00:00+00:00 → 2018-09-03-1000
    if rfc3339.len() < 16 {
        return String::new();
    }
    let (d, rest) = rfc3339.split_at(10);
    let time: String = rest.chars().skip(1).take(5).filter(|c| c.is_ascii_digit()).collect();
    if time.len() == 4 {
        format!("{d}-{time}")
    } else {
        d.to_string()
    }
}

/// Split `mbox` into individual messages.
pub fn split(mbox: &str, opts: &Options) -> Result<Vec<Message>, String> {
    if mbox.trim().is_empty() {
        return Err("input is empty — paste an mbox archive".into());
    }
    let blocks = split_blocks(mbox);
    if blocks.is_empty() {
        return Err("no messages found — an mbox separates messages with a `From ` line".into());
    }
    if blocks.len() > MAX_MESSAGES {
        return Err(format!(
            "archive holds {} messages, over the {MAX_MESSAGES}-message cap — split the file first",
            blocks.len()
        ));
    }
    if opts.message > blocks.len() {
        return Err(format!(
            "message {} does not exist — the archive holds {} message(s)",
            opts.message,
            blocks.len()
        ));
    }

    let mut used: HashMap<String, usize> = HashMap::new();
    let mut out = Vec::new();
    for (i, block) in blocks.into_iter().enumerate() {
        let index = i + 1;
        if opts.message != 0 && opts.message != index {
            continue;
        }
        let body = if opts.unescape_from { unescape_from_lines(&block.body) } else { block.body };
        let eml = match (&block.postmark, opts.keep_postmark) {
            (Some(p), true) => format!("{p}{body}"),
            _ => body,
        };
        let eml = eml.trim_end_matches('\n').to_string() + "\n";
        let h = headers_of(&eml);

        let stem = match opts.naming {
            Naming::Index => String::new(),
            Naming::Subject => slug(&h.subject, 40),
            Naming::Date => {
                let s = date_stamp(&h.date);
                if s.is_empty() { "undated".to_string() } else { s }
            }
            Naming::MessageId => slug(&h.message_id, 40),
        };
        let base = if stem.is_empty() {
            format!("{index:03}")
        } else {
            format!("{index:03}-{stem}")
        };
        // Index-prefixed names are already unique; the counter is belt-and-braces
        // for pathological inputs.
        let n = used.entry(base.clone()).or_insert(0);
        let filename = if *n == 0 { format!("{base}.eml") } else { format!("{base}-{n}.eml") };
        *n += 1;

        out.push(Message {
            index,
            filename,
            subject: h.subject,
            from: h.from,
            date: h.date,
            bytes: eml.len(),
            eml,
        });
    }

    if out.is_empty() {
        return Err("no messages found — an mbox separates messages with a `From ` line".into());
    }
    Ok(out)
}

/// Render the split for a text surface (the page and the CLI's readable modes).
pub fn render(mbox: &str, opts: &Options) -> Result<String, String> {
    let msgs = split(mbox, opts)?;
    match opts.output {
        Output::Eml => Ok(msgs[0].eml.trim_end().to_string()),
        Output::Json => serde_json::to_string_pretty(&msgs).map_err(|e| e.to_string()),
        Output::List => {
            let mut out = format!("{} message(s)\n", msgs.len());
            for m in &msgs {
                out.push_str(&format!(
                    "\n{:>3}. {}\n     date:    {}\n     from:    {}\n     subject: {}\n     size:    {} bytes\n",
                    m.index,
                    m.filename,
                    if m.date.is_empty() { "(none)" } else { &m.date },
                    if m.from.is_empty() { "(none)" } else { &m.from },
                    if m.subject.is_empty() { "(no subject)" } else { &m.subject },
                    m.bytes
                ));
            }
            Ok(out.trim_end().to_string())
        }
        Output::Files => {
            let mut out = format!("{} message(s)\n", msgs.len());
            for m in &msgs {
                out.push_str(&format!(
                    "\n===== {} ({} bytes) =====\n{}\n",
                    m.filename,
                    m.bytes,
                    m.eml.trim_end()
                ));
            }
            Ok(out.trim_end().to_string())
        }
    }
}

/// One-call entry point shared by the chat block, the CLI and the browser
/// wrapper: string/bool arguments in, rendered text out.
pub fn run(
    mbox: &str,
    output: &str,
    naming: &str,
    message: i64,
    unescape_from: bool,
    keep_postmark: bool,
) -> Result<String, String> {
    if message < 0 {
        return Err("message must be 0 (split every message) or a 1-based message number".into());
    }
    let opts = Options {
        output: Output::parse(output)?,
        naming: Naming::parse(naming)?,
        message: message as usize,
        unescape_from,
        keep_postmark,
    };
    render(mbox, &opts)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TWO: &str = "From alice@example.com Mon Sep 03 10:00:00 2018\nFrom: Alice <alice@example.com>\nSubject: Quarterly report\nMessage-ID: <a1@example.com>\nDate: Mon, 3 Sep 2018 10:00:00 +0000\n\nfirst body\n\nFrom bob@example.com Mon Sep 03 11:00:00 2018\nFrom: Bob <bob@example.com>\nSubject: Lunch?\nMessage-ID: <b2@example.com>\nDate: Mon, 3 Sep 2018 11:30:00 +0000\n\nsecond body\n";

    #[test]
    fn splits_two_messages_and_drops_postmarks() {
        let msgs = split(TWO, &Options::default()).unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].filename, "001.eml");
        assert_eq!(msgs[1].filename, "002.eml");
        assert!(msgs[0].eml.starts_with("From: Alice"), "postmark dropped: {:?}", msgs[0].eml);
        assert!(msgs[0].eml.contains("first body"));
        assert!(!msgs[0].eml.contains("second body"));
        assert_eq!(msgs[1].subject, "Lunch?");
        assert_eq!(msgs[1].from, "bob@example.com");
        assert!(msgs[1].date.starts_with("2018-09-03T11:30:00"));
    }

    #[test]
    fn keep_postmark_round_trips_the_separator() {
        let opts = Options { keep_postmark: true, ..Options::default() };
        let msgs = split(TWO, &opts).unwrap();
        assert!(msgs[0].eml.starts_with("From alice@example.com Mon Sep 03 10:00:00 2018\n"));
    }

    #[test]
    fn naming_schemes() {
        let by_subject = split(TWO, &Options { naming: Naming::Subject, ..Options::default() }).unwrap();
        assert_eq!(by_subject[0].filename, "001-quarterly-report.eml");
        let by_date = split(TWO, &Options { naming: Naming::Date, ..Options::default() }).unwrap();
        assert_eq!(by_date[1].filename, "002-2018-09-03-1130.eml");
        let by_id = split(TWO, &Options { naming: Naming::MessageId, ..Options::default() }).unwrap();
        assert_eq!(by_id[0].filename, "001-a1-example-com.eml");
    }

    #[test]
    fn single_message_selection() {
        let opts = Options { output: Output::Eml, message: 2, ..Options::default() };
        let out = render(TWO, &opts).unwrap();
        assert!(out.starts_with("From: Bob"));
        assert!(out.contains("second body"));
        assert!(!out.contains("first body"));
    }

    #[test]
    fn encoded_word_subject_is_decoded_for_the_filename() {
        let raw = "From x@example.com Mon Sep 03 10:00:00 2018\nSubject: =?utf-8?q?Caf=C3=A9_menu?=\n\nbody\n";
        let msgs = split(raw, &Options { naming: Naming::Subject, ..Options::default() }).unwrap();
        assert_eq!(msgs[0].subject, "Café menu");
        // Non-ASCII is dropped from the filename slug, so it stays portable.
        assert_eq!(msgs[0].filename, "001-caf-menu.eml");
    }

    #[test]
    fn from_quoting_is_undone_by_default_and_kept_when_off() {
        let raw = "From x@example.com Mon Sep 03 10:00:00 2018\nSubject: Quoting\n\n>From the desk of X\n>>From deeper\n";
        let on = split(raw, &Options::default()).unwrap();
        assert!(on[0].eml.contains("\nFrom the desk of X\n"));
        assert!(on[0].eml.contains("\n>From deeper\n"));
        let off = split(raw, &Options { unescape_from: false, ..Options::default() }).unwrap();
        assert!(off[0].eml.contains("\n>From the desk of X\n"));
    }

    #[test]
    fn a_lone_eml_without_a_postmark_is_one_message() {
        let raw = "From: Solo <solo@example.com>\nSubject: No postmark\n\nbody\n";
        let msgs = split(raw, &Options::default()).unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].subject, "No postmark");
    }

    #[test]
    fn from_header_is_not_treated_as_a_separator() {
        let msgs = split(TWO, &Options::default()).unwrap();
        assert_eq!(msgs.len(), 2, "`From:` header lines must not split messages");
    }

    #[test]
    fn renders_list_and_files_and_json() {
        let list = render(TWO, &Options { output: Output::List, ..Options::default() }).unwrap();
        assert!(list.starts_with("2 message(s)"));
        assert!(list.contains("001.eml"));
        assert!(list.contains("Quarterly report"));

        let files = render(TWO, &Options::default()).unwrap();
        assert!(files.contains("===== 001.eml ("));
        assert!(files.contains("second body"));

        let json = render(TWO, &Options { output: Output::Json, ..Options::default() }).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v.as_array().unwrap().len(), 2);
        assert_eq!(v[0]["filename"], "001.eml");
    }

    #[test]
    fn errors() {
        assert!(split("", &Options::default()).is_err());
        assert!(split("   \n ", &Options::default()).is_err());
        let missing = split(TWO, &Options { message: 9, ..Options::default() }).unwrap_err();
        assert!(missing.contains("does not exist"), "{missing}");
        assert!(Output::parse("zip").is_err());
        assert!(Naming::parse("sender").is_err());
    }

    #[test]
    fn run_wires_string_arguments_through() {
        let out = run(TWO, "list", "subject", 0, true, false).unwrap();
        assert!(out.starts_with("2 message(s)"));
        assert!(out.contains("001-quarterly-report.eml"));

        let one = run(TWO, "eml", "index", 2, true, false).unwrap();
        assert!(one.starts_with("From: Bob"));

        assert!(run(TWO, "zip", "index", 0, true, false).is_err());
        assert!(run(TWO, "files", "sender", 0, true, false).is_err());
        let negative = run(TWO, "files", "index", -1, true, false).unwrap_err();
        assert!(negative.contains("1-based"), "{negative}");
    }

    #[test]
    fn message_cap_boundary() {
        let one = "From x@example.com Mon Sep 03 10:00:00 2018\nSubject: s\n\nbody\n";
        let at_cap: String = one.repeat(MAX_MESSAGES);
        assert_eq!(split(&at_cap, &Options::default()).unwrap().len(), MAX_MESSAGES);
        let over = one.repeat(MAX_MESSAGES + 1);
        let err = split(&over, &Options::default()).unwrap_err();
        assert!(err.contains("over the"), "{err}");
    }
}

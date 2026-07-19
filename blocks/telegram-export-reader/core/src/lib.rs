//! telegram-export-reader core — turn a Telegram Desktop JSON export
//! (`result.json`) into a clean readable transcript and/or per-sender message +
//! word statistics. Pure Rust (`serde_json` only), no wafer/wasm-bindgen deps, so
//! it runs on every backend including the chat Service Worker.
//!
//! Accepted input shapes (Telegram Desktop "Machine-readable JSON" export):
//!   * a single-chat export: `{ "name": …, "type": …, "messages": [ … ] }`
//!   * a full account export: `{ "chats": { "list": [ { …, "messages": [ … ] } ] } }`
//!   * a bare array of message objects: `[ { … }, … ]`
//!
//! Each message is either `type: "message"` (a real message with `from` +
//! `text`) or `type: "service"` (a group action with `actor` + `action`). The
//! `text` field is a string OR an array mixing plain strings and formatting
//! entities (`{ "type": "bold", "text": "…" }`); both are flattened to plain text.

use std::collections::BTreeMap;

use serde_json::Value;

/// Which sections to render.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    Transcript,
    Stats,
    Both,
}

impl Output {
    pub fn parse(s: &str) -> Output {
        match s.trim().to_ascii_lowercase().as_str() {
            "transcript" | "text" => Output::Transcript,
            "stats" | "statistics" => Output::Stats,
            _ => Output::Both,
        }
    }
}

/// Rendering options (mirror the block descriptor params).
#[derive(Debug, Clone)]
pub struct Options {
    pub output: Output,
    /// Include Telegram service/system messages (group created, member joined, …).
    pub include_service: bool,
    /// Case-insensitive sender name; when set, only that person's messages are kept.
    pub sender_filter: Option<String>,
    /// Cap on how many messages to include (0 = no cap).
    pub max_messages: usize,
}

impl Default for Options {
    fn default() -> Options {
        Options { output: Output::Both, include_service: false, sender_filter: None, max_messages: 0 }
    }
}

/// How many entries the "Top words" / "Top emoji" rankings list.
const TOP_N: usize = 10;
/// Minimum length (chars) for a word to enter the "Top words" ranking.
const MIN_WORD_LEN: usize = 3;

/// Parse the pasted export and render the requested sections.
pub fn render(input: &str, opts: &Options) -> Result<String, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("No input: paste the contents of a Telegram export result.json.".into());
    }
    let root: Value = serde_json::from_str(trimmed).map_err(|e| {
        format!(
            "Could not parse the export as JSON: {e}. Paste the full contents of result.json \
             from a Telegram Desktop export."
        )
    })?;

    let (chat_name, chat_kind, raw_messages) = extract_messages(&root)?;

    let filter = opts.sender_filter.as_ref().map(|s| s.trim().to_lowercase());
    let filter = filter.filter(|s| !s.is_empty());

    // Filter + cap into the set of messages we actually render/count.
    let mut kept: Vec<&Value> = Vec::new();
    for m in &raw_messages {
        let is_service = m.get("type").and_then(Value::as_str) == Some("service");
        if is_service {
            if !opts.include_service {
                continue;
            }
            if let Some(f) = &filter {
                // A sender filter narrows to one person; keep a service line only
                // when that person is the actor.
                let actor = m.get("actor").and_then(Value::as_str).unwrap_or("").to_lowercase();
                if &actor != f {
                    continue;
                }
            }
        } else if let Some(f) = &filter {
            let from = m.get("from").and_then(Value::as_str).unwrap_or("").to_lowercase();
            if &from != f {
                continue;
            }
        }
        kept.push(m);
        if opts.max_messages != 0 && kept.len() >= opts.max_messages {
            break;
        }
    }

    if kept.is_empty() {
        return Err(match &filter {
            Some(f) => format!(
                "No messages matched sender '{f}'. Check the exact display name in the export."
            ),
            None => "No messages found in this export. Make sure you pasted the Machine-readable \
                     JSON (result.json), not the HTML export."
                .into(),
        });
    }

    let mut out = String::new();
    let header = chat_header(&chat_name, &chat_kind);

    match opts.output {
        Output::Transcript => {
            out.push_str(&header);
            out.push('\n');
            out.push_str(&render_transcript(&kept));
        }
        Output::Stats => {
            out.push_str(&render_stats(&header, &kept));
        }
        Output::Both => {
            out.push_str(&render_stats(&header, &kept));
            out.push_str("\n\n=== Transcript ===\n\n");
            out.push_str(&render_transcript(&kept));
        }
    }
    Ok(out.trim_end().to_string())
}

/// Pull the message array (and chat name/kind if present) out of any of the
/// three accepted top-level shapes.
fn extract_messages(root: &Value) -> Result<(Option<String>, Option<String>, Vec<Value>), String> {
    // Bare array of messages.
    if let Some(arr) = root.as_array() {
        return Ok((None, None, arr.clone()));
    }
    let obj = root.as_object().ok_or_else(|| {
        "Unexpected JSON shape: expected a Telegram export object or a messages array.".to_string()
    })?;

    // Single-chat export: top-level `messages`.
    if let Some(msgs) = obj.get("messages").and_then(Value::as_array) {
        let name = obj.get("name").and_then(Value::as_str).map(str::to_string);
        let kind = obj.get("type").and_then(Value::as_str).map(str::to_string);
        return Ok((name, kind, msgs.clone()));
    }

    // Full account export: `chats.list[].messages` — flatten every chat.
    if let Some(list) = obj.get("chats").and_then(|c| c.get("list")).and_then(Value::as_array) {
        let mut all = Vec::new();
        let mut only_name = None;
        let mut only_kind = None;
        let with_msgs: Vec<&Value> = list.iter().filter(|c| c.get("messages").is_some()).collect();
        for (i, chat) in with_msgs.iter().enumerate() {
            if let Some(msgs) = chat.get("messages").and_then(Value::as_array) {
                if i == 0 {
                    only_name = chat.get("name").and_then(Value::as_str).map(str::to_string);
                    only_kind = chat.get("type").and_then(Value::as_str).map(str::to_string);
                }
                all.extend(msgs.iter().cloned());
            }
        }
        if all.is_empty() {
            return Err("This looks like a full account export but no chat contained any messages."
                .into());
        }
        // Only surface a single chat's name when the export holds exactly one chat.
        let single = with_msgs.len() == 1;
        return Ok((
            if single { only_name } else { None },
            if single { only_kind } else { None },
            all,
        ));
    }

    Err("Could not find a `messages` array. Paste the Machine-readable JSON (result.json) from \
         Telegram Desktop's \"Export chat history\"."
        .into())
}

fn chat_header(name: &Option<String>, kind: &Option<String>) -> String {
    match (name, kind) {
        (Some(n), Some(k)) => format!("Chat: {} ({})", n, pretty_kind(k)),
        (Some(n), None) => format!("Chat: {}", n),
        (None, Some(k)) => format!("Chat: ({})", pretty_kind(k)),
        (None, None) => "Chat".to_string(),
    }
}

fn pretty_kind(k: &str) -> String {
    k.replace('_', " ")
}

/// Flatten a message `text` field (string OR array of strings/entities) into
/// plain text.
fn flatten_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(parts) => {
            let mut s = String::new();
            for p in parts {
                match p {
                    Value::String(t) => s.push_str(t),
                    Value::Object(o) => {
                        if let Some(t) = o.get("text").and_then(Value::as_str) {
                            s.push_str(t);
                        }
                    }
                    _ => {}
                }
            }
            s
        }
        _ => String::new(),
    }
}

/// The visible content of a real message: its text, or a `[media]` placeholder
/// when the message is media-only.
fn message_content(m: &Value) -> String {
    let text = m.get("text").map(flatten_text).unwrap_or_default();
    let mut body = if text.trim().is_empty() { media_placeholder(m) } else { text };
    if let Some(src) = m.get("forwarded_from").and_then(Value::as_str) {
        body = format!("[forwarded from {src}] {body}");
    }
    if m.get("edited").is_some() || m.get("edited_unixtime").is_some() {
        body.push_str(" (edited)");
    }
    body
}

/// A readable `[media]` tag for a message with no text body.
fn media_placeholder(m: &Value) -> String {
    if let Some(mt) = m.get("media_type").and_then(Value::as_str) {
        return match mt {
            "sticker" => match m.get("sticker_emoji").and_then(Value::as_str) {
                Some(e) => format!("[sticker {e}]"),
                None => "[sticker]".to_string(),
            },
            "voice_message" => "[voice message]".to_string(),
            "video_message" => "[video message]".to_string(),
            "video_file" => "[video]".to_string(),
            "animation" => "[GIF]".to_string(),
            "audio_file" => "[audio]".to_string(),
            other => match m.get("file_name").and_then(Value::as_str) {
                Some(f) => format!("[{}: {}]", other.replace('_', " "), f),
                None => format!("[{}]", other.replace('_', " ")),
            },
        };
    }
    if m.get("photo").is_some() {
        return "[photo]".to_string();
    }
    if m.get("poll").is_some() {
        let q = m.get("poll").and_then(|p| p.get("question")).and_then(Value::as_str).unwrap_or("");
        return if q.is_empty() { "[poll]".to_string() } else { format!("[poll: {q}]") };
    }
    if m.get("location_information").is_some() {
        return "[location]".to_string();
    }
    if m.get("contact_information").is_some() {
        return "[contact]".to_string();
    }
    if let Some(f) = m.get("file_name").and_then(Value::as_str) {
        return format!("[file: {f}]");
    }
    if m.get("file").is_some() {
        return "[file]".to_string();
    }
    "[no text]".to_string()
}

/// Reformat Telegram's ISO `date` (`2021-03-27T14:44:24`) as `2021-03-27 14:44:24`.
fn fmt_datetime(m: &Value) -> String {
    match m.get("date").and_then(Value::as_str) {
        Some(d) => d.replacen('T', " ", 1),
        None => "(no date)".to_string(),
    }
}

/// A one-line description of a service message action.
fn service_line(m: &Value) -> String {
    let actor = m.get("actor").and_then(Value::as_str).unwrap_or("Someone");
    let action = m.get("action").and_then(Value::as_str).unwrap_or("");
    let title = m.get("title").and_then(Value::as_str).unwrap_or("");
    let members: Vec<&str> = m
        .get("members")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let desc = match action {
        "create_group" => format!("created the group \"{title}\""),
        "create_channel" => format!("created the channel \"{title}\""),
        "edit_group_title" => format!("changed the group name to \"{title}\""),
        "edit_group_photo" => "changed the group photo".to_string(),
        "delete_group_photo" => "removed the group photo".to_string(),
        "invite_members" => format!("invited {}", join_names(&members)),
        "remove_members" => format!("removed {}", join_names(&members)),
        "join_group_by_link" => "joined the group via invite link".to_string(),
        "pin_message" => "pinned a message".to_string(),
        "clear_history" => "cleared the history".to_string(),
        "phone_call" => "made a call".to_string(),
        "migrate_to_supergroup" => "converted the group to a supergroup".to_string(),
        "migrate_from_group" => "was migrated from a basic group".to_string(),
        "" => "performed a service action".to_string(),
        other => other.replace('_', " "),
    };
    format!("[{}] * {} {}", fmt_datetime(m), actor, desc).trim_end().to_string()
}

fn join_names(names: &[&str]) -> String {
    match names.len() {
        0 => "members".to_string(),
        1 => names[0].to_string(),
        _ => format!("{} and {}", names[..names.len() - 1].join(", "), names[names.len() - 1]),
    }
}

fn render_transcript(kept: &[&Value]) -> String {
    let mut lines = Vec::with_capacity(kept.len());
    for m in kept {
        if m.get("type").and_then(Value::as_str) == Some("service") {
            lines.push(service_line(m));
        } else {
            let from = m.get("from").and_then(Value::as_str).unwrap_or("Unknown");
            lines.push(format!("[{}] {}: {}", fmt_datetime(m), from, message_content(m)));
        }
    }
    lines.join("\n")
}

fn count_words(text: &str) -> usize {
    text.split_whitespace().filter(|w| w.chars().any(char::is_alphanumeric)).count()
}

/// Is `c` an emoji-range code point (rough, dependency-free)?
fn is_emoji(c: char) -> bool {
    let u = c as u32;
    matches!(u,
        0x1F300..=0x1FAFF   // symbols & pictographs, supplemental, extended-A
        | 0x2600..=0x27BF   // misc symbols + dingbats
        | 0x1F000..=0x1F0FF // mahjong/dominoes/cards
        | 0x2B00..=0x2BFF   // misc symbols & arrows (incl. ⭐)
        | 0x1F1E6..=0x1F1FF // regional indicators (flags)
    )
}

fn render_stats(header: &str, kept: &[&Value]) -> String {
    // Per-sender tallies + global word/emoji frequencies (real messages only).
    let mut per_sender: BTreeMap<String, (usize, usize)> = BTreeMap::new(); // name -> (msgs, words)
    let mut word_freq: BTreeMap<String, usize> = BTreeMap::new();
    let mut emoji_freq: BTreeMap<String, usize> = BTreeMap::new();
    let mut total_msgs = 0usize;
    let mut total_words = 0usize;
    let mut media_msgs = 0usize;
    let mut service_msgs = 0usize;
    let mut first_date: Option<String> = None;
    let mut last_date: Option<String> = None;

    for m in kept {
        if let Some(d) = m.get("date").and_then(Value::as_str) {
            let day = d.get(0..10).unwrap_or(d).to_string();
            if first_date.as_ref().map(|f| &day < f).unwrap_or(true) {
                first_date = Some(day.clone());
            }
            if last_date.as_ref().map(|l| &day > l).unwrap_or(true) {
                last_date = Some(day);
            }
        }
        if m.get("type").and_then(Value::as_str) == Some("service") {
            service_msgs += 1;
            continue;
        }
        total_msgs += 1;
        let from = m.get("from").and_then(Value::as_str).unwrap_or("Unknown").to_string();
        let text = m.get("text").map(flatten_text).unwrap_or_default();
        if text.trim().is_empty() {
            media_msgs += 1;
        }
        let words = count_words(&text);
        total_words += words;
        let entry = per_sender.entry(from).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += words;
        for w in text.split(|c: char| !c.is_alphanumeric()) {
            let w = w.to_lowercase();
            if w.chars().count() >= MIN_WORD_LEN {
                *word_freq.entry(w).or_insert(0) += 1;
            }
        }
        for c in text.chars() {
            if is_emoji(c) {
                *emoji_freq.entry(c.to_string()).or_insert(0) += 1;
            }
        }
    }

    let mut s = String::new();
    s.push_str(header);
    s.push('\n');
    s.push_str(&format!("Messages: {total_msgs}\n"));
    s.push_str(&format!("Participants: {}\n", per_sender.len()));
    s.push_str(&format!("Words: {total_words}\n"));
    if media_msgs > 0 {
        s.push_str(&format!("Media messages: {media_msgs}\n"));
    }
    if service_msgs > 0 {
        s.push_str(&format!("Service messages: {service_msgs}\n"));
    }
    if let (Some(f), Some(l)) = (&first_date, &last_date) {
        if f == l {
            s.push_str(&format!("Date: {f}\n"));
        } else {
            s.push_str(&format!("Date range: {f} to {l}\n"));
        }
    }

    // Per-sender leaderboard: most messages first, ties broken by name.
    let mut senders: Vec<(&String, &(usize, usize))> = per_sender.iter().collect();
    senders.sort_by(|a, b| b.1 .0.cmp(&a.1 .0).then_with(|| a.0.cmp(b.0)));
    if !senders.is_empty() {
        s.push_str("\nMessages per sender:\n");
        let name_w = senders.iter().map(|(n, _)| n.chars().count()).max().unwrap_or(0);
        for (name, (msgs, words)) in senders {
            let share =
                if total_msgs > 0 { *msgs as f64 * 100.0 / total_msgs as f64 } else { 0.0 };
            s.push_str(&format!(
                "  {:>5}  {:>6.2}%  {:<width$}  ({} words)\n",
                msgs,
                share,
                name,
                words,
                width = name_w
            ));
        }
    }

    push_ranking(&mut s, &format!("\nTop words (min length {MIN_WORD_LEN}):\n"), &word_freq);
    push_ranking(&mut s, "\nTop emoji:\n", &emoji_freq);
    s
}

/// Append a "count  token" ranking (desc by count, ties by token), top TOP_N.
fn push_ranking(s: &mut String, heading: &str, freq: &BTreeMap<String, usize>) {
    if freq.is_empty() {
        return;
    }
    let mut items: Vec<(&String, &usize)> = freq.iter().collect();
    items.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
    s.push_str(heading);
    for (tok, count) in items.into_iter().take(TOP_N) {
        s.push_str(&format!("  {:>5}  {}\n", count, tok));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "name": "Weekend Trip",
        "type": "private_group",
        "id": 100,
        "messages": [
            {"id": 1, "type": "service", "date": "2021-03-27T14:44:24", "actor": "Alice", "actor_id": "user1", "action": "create_group", "title": "Weekend Trip"},
            {"id": 2, "type": "message", "date": "2021-03-27T14:45:00", "from": "Alice", "from_id": "user1", "text": "Hey everyone ready for the trip"},
            {"id": 3, "type": "message", "date": "2021-03-28T09:46:10", "from": "Bob", "from_id": "user2", "text": ["Yes ", {"type":"bold","text":"so"}, " excited 🎉"]},
            {"id": 4, "type": "message", "date": "2021-03-28T09:47:00", "from": "Bob", "from_id": "user2", "text": "", "photo": "photos/photo_1.jpg", "width": 1280, "height": 720}
        ]
    }"#;

    #[test]
    fn transcript_excludes_service_by_default() {
        let out =
            render(SAMPLE, &Options { output: Output::Transcript, ..Default::default() }).unwrap();
        assert!(out.contains("Chat: Weekend Trip (private group)"));
        assert!(out.contains("[2021-03-27 14:45:00] Alice: Hey everyone ready for the trip"));
        assert!(out.contains("[2021-03-28 09:46:10] Bob: Yes so excited 🎉"));
        assert!(out.contains("[2021-03-28 09:47:00] Bob: [photo]"));
        assert!(!out.contains("created the group"));
    }

    #[test]
    fn transcript_includes_service_when_asked() {
        let out = render(
            SAMPLE,
            &Options { output: Output::Transcript, include_service: true, ..Default::default() },
        )
        .unwrap();
        assert!(out.contains("* Alice created the group \"Weekend Trip\""));
    }

    #[test]
    fn stats_counts_messages_and_words() {
        let out = render(SAMPLE, &Options { output: Output::Stats, ..Default::default() }).unwrap();
        assert!(out.contains("Messages: 3"));
        assert!(out.contains("Participants: 2"));
        assert!(out.contains("Words: 9")); // Alice 6 + Bob "Yes so excited" 3
        assert!(out.contains("Media messages: 1"));
        assert!(out.contains("Date range: 2021-03-27 to 2021-03-28"));
    }

    #[test]
    fn sender_filter_narrows() {
        let out = render(
            SAMPLE,
            &Options {
                output: Output::Stats,
                sender_filter: Some("bob".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(out.contains("Messages: 2"));
        assert!(out.contains("Participants: 1"));
    }

    #[test]
    fn max_messages_caps() {
        let out = render(
            SAMPLE,
            &Options { output: Output::Transcript, max_messages: 1, ..Default::default() },
        )
        .unwrap();
        // Only the first non-service message survives the cap (service excluded first).
        assert_eq!(out.lines().filter(|l| l.starts_with('[')).count(), 1);
    }

    #[test]
    fn bare_array_shape_supported() {
        let arr = r#"[{"type":"message","date":"2020-01-01T00:00:00","from":"X","text":"hi there"}]"#;
        let out = render(arr, &Options { output: Output::Stats, ..Default::default() }).unwrap();
        assert!(out.contains("Messages: 1"));
        assert!(out.contains("Participants: 1"));
    }

    #[test]
    fn invalid_json_errors() {
        let err = render("not json at all", &Options::default()).unwrap_err();
        assert!(err.contains("Could not parse"));
    }

    #[test]
    fn empty_input_errors() {
        let err = render("   ", &Options::default()).unwrap_err();
        assert!(err.contains("No input"));
    }
}

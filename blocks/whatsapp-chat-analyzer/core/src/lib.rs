//! gizza-ai/whatsapp-chat-analyzer core — parse an exported WhatsApp chat and
//! report per-person message counts, busiest hours and days, and word + emoji
//! frequency. Pure-Rust, dependency-free (no regex, no clock): the whole report
//! is a deterministic function of the input text and the options.
//!
//! Two export dialects are recognised without regex:
//!   iOS:     `[2024-01-05, 21:07:33] Alice: hey` (brackets, optional seconds,
//!            optional AM/PM, optional invisible LRM/RTL marks)
//!   Android: `05/01/2024, 21:07 - Alice: hey`     (date, time - sender: text)
//! Continuation lines (no parseable header) are folded into the previous
//! message. Lines with no `Sender:` are system/notification lines and are
//! excluded from every per-person and frequency total.

use std::collections::HashMap;

/// How to read the day/month/year order of ambiguous numeric dates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateFormat {
    /// Detect from the data: if any date has a first field > 12 it is day-first,
    /// if any has a second field > 12 it is month-first, else default day-first.
    Auto,
    /// Day/Month/Year (e.g. 05/01/2024 = 5 January).
    Dmy,
    /// Month/Day/Year (e.g. 01/05/2024 = 5 January).
    Mdy,
    /// Year/Month/Day (ISO, e.g. 2024-01-05).
    Ymd,
}

impl DateFormat {
    pub fn parse(s: &str) -> DateFormat {
        match s.trim().to_ascii_lowercase().as_str() {
            "dmy" | "dd/mm/yyyy" | "day-first" => DateFormat::Dmy,
            "mdy" | "mm/dd/yyyy" | "month-first" => DateFormat::Mdy,
            "ymd" | "yyyy-mm-dd" | "iso" => DateFormat::Ymd,
            _ => DateFormat::Auto,
        }
    }
}

/// A single parsed chat message.
#[derive(Debug, Clone, PartialEq)]
struct Msg {
    /// `None` for system/notification lines (no explicit sender).
    sender: Option<String>,
    hour: u32,
    /// Raw numeric date fields exactly as they appear, plus whether the first
    /// field was a 4-digit year (ISO). Resolved to a weekday in a second pass.
    date: Option<RawDate>,
    text: String,
    is_media: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct RawDate {
    n0: i64,
    n1: i64,
    n2: i64,
    iso: bool,
}

const WEEKDAYS: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];

/// Common English stop words (lowercase, sorted for binary search).
const STOP_WORDS: &[&str] = &[
    "a", "about", "after", "again", "all", "am", "an", "and", "any", "are", "as", "at", "be",
    "been", "but", "by", "can", "did", "do", "for", "from", "get", "go", "got", "had", "has",
    "have", "he", "her", "here", "him", "his", "how", "i", "if", "in", "is", "it", "its", "just",
    "know", "like", "lol", "me", "my", "no", "not", "now", "of", "off", "oh", "ok", "okay", "on",
    "one", "or", "our", "out", "so", "that", "the", "their", "them", "then", "there", "they",
    "this", "to", "too", "up", "was", "we", "were", "what", "when", "where", "which", "who",
    "will", "with", "would", "yeah", "yes", "you", "your",
];

fn is_stop_word(lower: &str) -> bool {
    STOP_WORDS.binary_search(&lower).is_ok()
}

/// Strip the invisible directionality marks (LRM/RLM/LRE…/pop) WhatsApp sprinkles
/// into iOS exports so they don't corrupt sender/media detection.
fn strip_marks(s: &str) -> String {
    s.chars()
        .filter(|c| {
            !matches!(*c,
                '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
        })
        .collect()
}

/// Parse the `HH:MM[:SS] [AM|PM]` time chunk into a 0–23 hour.
fn parse_hour(time: &str) -> Option<u32> {
    let time = strip_marks(time);
    let time = time.trim();
    // Split off an AM/PM suffix (may be separated by a normal or narrow space).
    let mut hhmm = time;
    let mut pm = None;
    let lower = time.to_ascii_lowercase();
    if let Some(idx) = lower.find(|c| c == 'a' || c == 'p') {
        let suffix = lower[idx..].replace(['.', ' ', '\u{202F}', '\u{00A0}'], "");
        if suffix.starts_with("am") {
            pm = Some(false);
            hhmm = time[..idx].trim();
        } else if suffix.starts_with("pm") {
            pm = Some(true);
            hhmm = time[..idx].trim();
        }
    }
    let mut hour: u32 = hhmm.split(':').next()?.trim().parse().ok()?;
    match pm {
        Some(true) => {
            if hour != 12 {
                hour += 12;
            }
        }
        Some(false) => {
            if hour == 12 {
                hour = 0;
            }
        }
        None => {}
    }
    if hour > 23 {
        return None;
    }
    Some(hour)
}

/// Parse a `DATE, TIME` header chunk (the part inside `[...]` for iOS, or before
/// ` - ` for Android) into a raw date + hour.
fn parse_datetime(chunk: &str) -> Option<(RawDate, u32)> {
    let chunk = strip_marks(chunk);
    let (date_part, time_part) = chunk.split_once(',')?;
    let hour = parse_hour(time_part)?;
    let nums: Vec<i64> = date_part
        .trim()
        .split(|c: char| c == '/' || c == '.' || c == '-')
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .map(|p| p.parse::<i64>())
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    if nums.len() != 3 {
        return None;
    }
    let iso = date_part
        .trim()
        .split(|c: char| c == '/' || c == '.' || c == '-')
        .next()?
        .trim()
        .len()
        == 4;
    Some((RawDate { n0: nums[0], n1: nums[1], n2: nums[2], iso }, hour))
}

/// Split the message body into `(sender, text)`. A `Sender: text` split only
/// counts when the colon comes before the first newline and the sender is short
/// enough to be a name (not a whole system sentence).
fn split_sender(rest: &str) -> (Option<String>, String) {
    let rest = strip_marks(rest);
    let first_line = rest.split('\n').next().unwrap_or("");
    if let Some(idx) = first_line.find(": ") {
        let sender = first_line[..idx].trim();
        // A real sender is a short-ish name, not a system sentence like
        // "Alice changed the group description to". Guard on length + word count.
        if !sender.is_empty()
            && sender.chars().count() <= 60
            && sender.split_whitespace().count() <= 6
        {
            let text = rest[idx + 2..].to_string();
            return (Some(sender.to_string()), text);
        }
    }
    (None, rest)
}

fn looks_like_media(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    if lower.contains("<media omitted>") || lower.contains("<attached:") {
        return true;
    }
    lower.contains("omitted")
        && ["image", "video", "audio", "sticker", "gif", "document", "contact card"]
            .iter()
            .any(|k| lower.contains(k))
}

/// Try to parse one line as a NEW message header. Returns `None` for a
/// continuation line (which the caller folds into the previous message).
fn parse_header(line: &str) -> Option<Msg> {
    let trimmed = strip_marks(line);
    let trimmed = trimmed.trim_start();
    // iOS: [DATE, TIME] rest
    if let Some(after_open) = trimmed.strip_prefix('[') {
        if let Some(close) = after_open.find(']') {
            if let Some((date, hour)) = parse_datetime(&after_open[..close]) {
                let rest = after_open[close + 1..].trim_start();
                let (sender, text) = split_sender(rest);
                let is_media = looks_like_media(&text);
                return Some(Msg { sender, hour, date: Some(date), text, is_media });
            }
        }
    }
    // Android: DATE, TIME - rest
    if let Some(pos) = trimmed.find(" - ") {
        let (left, right) = trimmed.split_at(pos);
        if let Some((date, hour)) = parse_datetime(left) {
            let rest = &right[3..];
            let (sender, text) = split_sender(rest);
            let is_media = looks_like_media(&text);
            return Some(Msg { sender, hour, date: Some(date), text, is_media });
        }
    }
    None
}

/// Zeller's congruence → weekday index 0=Monday … 6=Sunday.
fn weekday_index(day: i64, mut month: i64, mut year: i64) -> Option<usize> {
    if !(1..=31).contains(&day) || !(1..=12).contains(&month) {
        return None;
    }
    if year < 100 {
        year += 2000;
    }
    if month < 3 {
        month += 12;
        year -= 1;
    }
    let k = year % 100;
    let j = year / 100;
    // h: 0=Saturday, 1=Sunday, 2=Monday, … 6=Friday
    let h = (day + (13 * (month + 1)) / 5 + k + k / 4 + j / 4 + 5 * j).rem_euclid(7);
    // Map to 0=Monday … 6=Sunday.
    Some(((h + 5) % 7) as usize)
}

/// Resolve a raw date into `(day, month, year)` given the chosen order.
fn resolve_date(d: &RawDate, day_first: bool) -> (i64, i64, i64) {
    if d.iso {
        // YYYY, MM, DD
        (d.n2, d.n1, d.n0)
    } else if day_first {
        (d.n0, d.n1, d.n2)
    } else {
        (d.n1, d.n0, d.n2)
    }
}

/// Tokenise a message into words (letters/digits, interior apostrophes kept).
/// Everything else separates.
fn words<'a>(text: &'a str, out: &mut Vec<&'a str>) {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut start: Option<usize> = None;
    let mut end = 0usize;
    for (idx, (i, c)) in chars.iter().enumerate() {
        let is_apos = *c == '\'' || *c == '\u{2019}';
        let next_word = chars.get(idx + 1).map(|(_, nc)| nc.is_alphanumeric()).unwrap_or(false);
        let in_word = c.is_alphanumeric() || (is_apos && start.is_some() && next_word);
        if in_word {
            if start.is_none() {
                start = Some(*i);
            }
            end = i + c.len_utf8();
        } else if let Some(s) = start.take() {
            out.push(&text[s..end]);
        }
    }
    if let Some(s) = start {
        out.push(&text[s..end]);
    }
}

fn is_regional(c: char) -> bool {
    ('\u{1F1E6}'..='\u{1F1FF}').contains(&c)
}

fn is_emoji_base(c: char) -> bool {
    matches!(c as u32,
        0x1F300..=0x1F5FF | // symbols & pictographs
        0x1F600..=0x1F64F | // emoticons
        0x1F680..=0x1F6FF | // transport & map
        0x1F900..=0x1F9FF | // supplemental symbols
        0x1FA00..=0x1FAFF | // extended-A + chess/symbols
        0x2600..=0x26FF   | // misc symbols
        0x2700..=0x27BF   | // dingbats
        0x2B00..=0x2BFF   | // misc symbols & arrows (⭐ etc.)
        0x2194..=0x21AA   | // arrows used as emoji
        0x231A..=0x231B   | // watch/hourglass
        0x23E9..=0x23FA   | // media controls
        0x25AA..=0x25FE   | // geometric
        0x2934..=0x2935     // arrows
    )
}

fn is_emoji_modifier(c: char) -> bool {
    matches!(c,
        '\u{1F3FB}'..='\u{1F3FF}' | // skin tones
        '\u{FE0F}' | '\u{FE0E}'   | // variation selectors
        '\u{20E3}'                  // combining enclosing keycap
    )
}

/// Split emoji clusters out of text, grouping ZWJ sequences, skin-tone
/// modifiers, variation selectors, and regional-indicator flag pairs into one
/// token each so 👨‍👩‍👧 and 🇬🇧 count once.
fn emojis(text: &str, out: &mut Vec<String>) {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if is_regional(c) {
            let mut cluster = String::new();
            cluster.push(c);
            i += 1;
            if i < chars.len() && is_regional(chars[i]) {
                cluster.push(chars[i]);
                i += 1;
            }
            out.push(cluster);
            continue;
        }
        if is_emoji_base(c) {
            let mut cluster = String::new();
            cluster.push(c);
            i += 1;
            loop {
                while i < chars.len() && is_emoji_modifier(chars[i]) {
                    cluster.push(chars[i]);
                    i += 1;
                }
                if i + 1 < chars.len()
                    && chars[i] == '\u{200D}'
                    && (is_emoji_base(chars[i + 1]) || is_regional(chars[i + 1]))
                {
                    cluster.push(chars[i]); // ZWJ
                    cluster.push(chars[i + 1]);
                    i += 2;
                    continue;
                }
                break;
            }
            out.push(cluster);
            continue;
        }
        i += 1;
    }
}

fn has_link(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("http://") || lower.contains("https://") || lower.contains("www.")
}

/// Rank a first-seen-ordered count map most→least, ties broken by first-seen.
fn rank(counts: &HashMap<String, usize>, order: &[String], top: usize) -> Vec<(String, usize)> {
    let mut idx: Vec<(usize, &String)> = order.iter().enumerate().collect();
    idx.sort_by(|a, b| counts[b.1].cmp(&counts[a.1]).then(a.0.cmp(&b.0)));
    let mut out: Vec<(String, usize)> =
        idx.into_iter().map(|(_, k)| (k.clone(), counts[k])).collect();
    if top > 0 && out.len() > top {
        out.truncate(top);
    }
    out
}

fn pct(n: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        ((n as f64 / total as f64) * 10_000.0).round() / 100.0
    }
}

/// Analyse an exported WhatsApp chat and return a plain-text report.
///
/// - `date_format` — how to read ambiguous DD/MM vs MM/DD dates (auto-detected
///   by default; only affects the "busiest days" weekday breakdown).
/// - `top` — how many entries to list for words and emoji (0 = all).
/// - `min_word_length` — skip words shorter than this in the word ranking.
/// - `ignore_stopwords` — drop common English filler words from the word ranking.
pub fn analyze(
    chat: &str,
    date_format: DateFormat,
    top: usize,
    min_word_length: usize,
    ignore_stopwords: bool,
) -> Result<String, String> {
    // --- Pass 1: parse lines into messages, folding continuations. ---
    let mut msgs: Vec<Msg> = Vec::new();
    for line in chat.lines() {
        match parse_header(line) {
            Some(m) => msgs.push(m),
            None => {
                if let Some(last) = msgs.last_mut() {
                    last.text.push('\n');
                    last.text.push_str(&strip_marks(line));
                    if !last.is_media {
                        last.is_media = looks_like_media(line);
                    }
                }
                // A leading continuation with no prior message is ignored.
            }
        }
    }

    // --- Detect day-first vs month-first for the whole chat. ---
    let day_first = match date_format {
        DateFormat::Dmy => true,
        DateFormat::Mdy => false,
        DateFormat::Ymd => true, // unused for ISO dates
        DateFormat::Auto => {
            let mut first_gt12 = false;
            let mut second_gt12 = false;
            for m in &msgs {
                if let Some(d) = &m.date {
                    if !d.iso {
                        if d.n0 > 12 {
                            first_gt12 = true;
                        }
                        if d.n1 > 12 {
                            second_gt12 = true;
                        }
                    }
                }
            }
            // If the second field ever exceeds 12 it must be the day → month-first.
            // Otherwise assume day-first (the international / iOS default).
            !(second_gt12 && !first_gt12)
        }
    };

    // --- Aggregate. ---
    let mut per_person: HashMap<String, usize> = HashMap::new();
    let mut person_order: Vec<String> = Vec::new();
    let mut hours = [0usize; 24];
    let mut weekdays = [0usize; 7];
    let mut word_counts: HashMap<String, usize> = HashMap::new();
    let mut word_order: Vec<String> = Vec::new();
    let mut emoji_counts: HashMap<String, usize> = HashMap::new();
    let mut emoji_order: Vec<String> = Vec::new();

    let mut total_msgs = 0usize;
    let mut media_msgs = 0usize;
    let mut link_msgs = 0usize;
    let mut total_words = 0usize;
    let min_word_length = min_word_length.max(1);

    for m in &msgs {
        let sender = match &m.sender {
            Some(s) => s,
            None => continue, // system/notification line
        };
        total_msgs += 1;
        let e = per_person.entry(sender.clone()).or_insert_with(|| {
            person_order.push(sender.clone());
            0
        });
        *e += 1;

        hours[m.hour as usize] += 1;
        if let Some(d) = &m.date {
            let (day, month, year) = resolve_date(d, day_first);
            if let Some(wd) = weekday_index(day, month, year) {
                weekdays[wd] += 1;
            }
        }

        if m.is_media {
            media_msgs += 1;
            continue; // placeholder text — not real words/emoji
        }
        if has_link(&m.text) {
            link_msgs += 1;
        }

        let mut toks: Vec<&str> = Vec::new();
        words(&m.text, &mut toks);
        for w in toks {
            total_words += 1;
            if w.chars().count() < min_word_length {
                continue;
            }
            let lower = w.to_lowercase();
            if ignore_stopwords && is_stop_word(&lower) {
                continue;
            }
            let we = word_counts.entry(lower.clone()).or_insert_with(|| {
                word_order.push(lower.clone());
                0
            });
            *we += 1;
        }

        let mut es: Vec<String> = Vec::new();
        emojis(&m.text, &mut es);
        for e in es {
            let ee = emoji_counts.entry(e.clone()).or_insert_with(|| {
                emoji_order.push(e.clone());
                0
            });
            *ee += 1;
        }
    }

    if total_msgs == 0 {
        return Err(
            "No WhatsApp messages found. Paste an exported chat — each message begins with a \
             timestamp like `[2024-01-05, 21:07:33] Alice: …` (iOS) or `05/01/2024, 21:07 - \
             Alice: …` (Android)."
                .into(),
        );
    }

    // --- Build the report. ---
    let mut r = String::new();
    r.push_str("WhatsApp chat summary\n");
    r.push_str("=====================\n");
    r.push_str(&format!("Messages: {}\n", total_msgs));
    r.push_str(&format!("Participants: {}\n", per_person.len()));
    r.push_str(&format!(
        "Words: {}  ·  Media messages: {}  ·  Links: {}\n",
        total_words, media_msgs, link_msgs
    ));

    r.push_str("\nMessages per person\n");
    r.push_str("-------------------\n");
    for (name, count) in rank(&per_person, &person_order, 0) {
        r.push_str(&format!("{:>6}  {:>6.2}%  {}\n", count, pct(count, total_msgs), name));
    }

    r.push_str("\nBusiest hours\n");
    r.push_str("-------------\n");
    let mut hour_rows: Vec<(usize, usize)> =
        hours.iter().copied().enumerate().filter(|&(_, c)| c > 0).collect();
    hour_rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    for (h, c) in hour_rows {
        r.push_str(&format!("{:02}:00  {:>6}  {:>6.2}%\n", h, c, pct(c, total_msgs)));
    }

    r.push_str("\nBusiest days\n");
    r.push_str("------------\n");
    let wd_total: usize = weekdays.iter().sum();
    if wd_total == 0 {
        r.push_str("(no parseable dates)\n");
    } else {
        let mut wd_rows: Vec<(usize, usize)> =
            weekdays.iter().copied().enumerate().filter(|&(_, c)| c > 0).collect();
        wd_rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        for (d, c) in wd_rows {
            r.push_str(&format!("{:<9}  {:>6}  {:>6.2}%\n", WEEKDAYS[d], c, pct(c, wd_total)));
        }
    }

    r.push_str(&format!("\nTop words (min length {}", min_word_length));
    if ignore_stopwords {
        r.push_str(", stop words removed");
    }
    r.push_str(")\n");
    r.push_str("---------------------------------\n");
    let word_rows = rank(&word_counts, &word_order, top);
    if word_rows.is_empty() {
        r.push_str("(none)\n");
    } else {
        for (w, c) in word_rows {
            r.push_str(&format!("{:>6}  {}\n", c, w));
        }
    }

    r.push_str("\nTop emoji\n");
    r.push_str("---------\n");
    let emoji_rows = rank(&emoji_counts, &emoji_order, top);
    if emoji_rows.is_empty() {
        r.push_str("(none)\n");
    } else {
        for (e, c) in emoji_rows {
            r.push_str(&format!("{:>6}  {}\n", c, e));
        }
    }

    Ok(r.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
[2024-01-05, 21:07:33] Alice: Hey Bob 😂😂 how are you?
[2024-01-05, 21:08:01] Bob: good thanks! www.example.com
[2024-01-06, 09:15:00] Alice: ‎image omitted
[2024-01-06, 09:16:10] Bob: nice 😂";

    #[test]
    fn parses_ios_export_and_counts_people() {
        let out = analyze(SAMPLE, DateFormat::Auto, 10, 1, false).unwrap();
        assert!(out.contains("Messages: 4"));
        assert!(out.contains("Participants: 2"));
        // Alice + Bob each sent 2.
        assert!(out.contains("     2   50.00%  Alice"));
        assert!(out.contains("     2   50.00%  Bob"));
        // media + link overview
        assert!(out.contains("Media messages: 1"));
        assert!(out.contains("Links: 1"));
    }

    #[test]
    fn ranks_top_emoji() {
        let out = analyze(SAMPLE, DateFormat::Auto, 10, 1, false).unwrap();
        let emoji_section = out.split("Top emoji").nth(1).unwrap();
        // 😂 appears 3 times (twice from Alice, once from Bob).
        assert!(emoji_section.contains("     3  😂"), "got: {emoji_section}");
    }

    #[test]
    fn busiest_hour_is_21() {
        // 21:00 has 3 messages, more than any other hour, so it ranks first.
        let chat = "\
[2024-01-05, 21:07:33] Alice: hi
[2024-01-05, 21:08:01] Bob: hey
[2024-01-05, 21:30:00] Alice: still up?
[2024-01-06, 09:15:00] Bob: morning";
        let out = analyze(chat, DateFormat::Auto, 10, 1, false).unwrap();
        let hours = out.split("Busiest hours").nth(1).unwrap();
        let first_line = hours.lines().nth(2).unwrap();
        assert!(first_line.starts_with("21:00"), "got: {first_line}");
    }

    #[test]
    fn parses_android_dash_format() {
        let chat = "05/01/2024, 21:07 - Alice: hello\n06/01/2024, 08:00 - Bob: morning";
        let out = analyze(chat, DateFormat::Auto, 10, 1, false).unwrap();
        assert!(out.contains("Messages: 2"));
        assert!(out.contains("Alice"));
        assert!(out.contains("Bob"));
    }

    #[test]
    fn am_pm_hours_convert() {
        // 12:30 AM -> hour 0; 1:00 PM -> hour 13.
        let chat = "1/5/2024, 12:30 AM - A: x\n1/5/2024, 1:00 PM - A: y";
        let out = analyze(chat, DateFormat::Auto, 10, 1, false).unwrap();
        let hours = out.split("Busiest hours").nth(1).unwrap();
        assert!(hours.contains("00:00"));
        assert!(hours.contains("13:00"));
    }

    #[test]
    fn weekday_known_date() {
        // 2024-01-05 was a Friday.
        let chat = "[2024-01-05, 10:00:00] A: hi";
        let out = analyze(chat, DateFormat::Auto, 10, 1, false).unwrap();
        assert!(out.contains("Friday"), "got: {out}");
    }

    #[test]
    fn dmy_vs_mdy_detection() {
        // 13 can only be a day -> day-first; 25/12/2024 is a Wednesday.
        let chat = "13/01/2024, 10:00 - A: x\n25/12/2024, 11:00 - A: y";
        let out = analyze(chat, DateFormat::Auto, 10, 1, false).unwrap();
        assert!(out.contains("Wednesday"), "25 Dec 2024 is a Wednesday; got: {out}");
    }

    #[test]
    fn forced_mdy() {
        // 01/05/2024 as month-first = 5 January = Friday.
        let chat = "01/05/2024, 10:00 - A: hi";
        let out = analyze(chat, DateFormat::Mdy, 10, 1, false).unwrap();
        assert!(out.contains("Friday"), "got: {out}");
    }

    #[test]
    fn multiline_messages_fold_in() {
        let chat =
            "[2024-01-05, 10:00:00] A: line one\nline two continues\n[2024-01-05, 10:01:00] B: ok";
        let out = analyze(chat, DateFormat::Auto, 10, 1, false).unwrap();
        // Only two messages despite three lines.
        assert!(out.contains("Messages: 2"));
        // "continues" from the folded line is counted as a word.
        assert!(out.to_lowercase().contains("continues"));
    }

    #[test]
    fn system_lines_excluded() {
        let chat = "[2024-01-05, 10:00:00] Messages and calls are end-to-end encrypted.\n[2024-01-05, 10:01:00] Alice: hi";
        let out = analyze(chat, DateFormat::Auto, 10, 1, false).unwrap();
        assert!(out.contains("Messages: 1"));
        assert!(out.contains("Participants: 1"));
    }

    #[test]
    fn stopwords_and_min_length_filter_words() {
        let chat = "[2024-01-05, 10:00:00] A: the the the pizza pizza hi";
        let out = analyze(chat, DateFormat::Auto, 10, 3, true).unwrap();
        let words = out.split("Top words").nth(1).unwrap();
        assert!(words.contains("pizza"));
        assert!(!words.contains("the")); // stop word
        assert!(!words.contains(" hi")); // too short
    }

    #[test]
    fn empty_input_errors() {
        let err = analyze("", DateFormat::Auto, 10, 1, false).unwrap_err();
        assert!(err.contains("No WhatsApp messages"));
    }

    #[test]
    fn non_chat_text_errors() {
        let err =
            analyze("just some random text\nwith no timestamps", DateFormat::Auto, 10, 1, false)
                .unwrap_err();
        assert!(err.contains("No WhatsApp messages"));
    }

    #[test]
    fn flag_emoji_counts_once() {
        let mut out = Vec::new();
        emojis("🇬🇧 hi 🇬🇧🇺🇸", &mut out);
        assert_eq!(out, vec!["🇬🇧", "🇬🇧", "🇺🇸"]);
    }

    #[test]
    fn zwj_family_counts_once() {
        let mut out = Vec::new();
        emojis("👨‍👩‍👧 done", &mut out);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn adjacent_emoji_split() {
        let mut out = Vec::new();
        emojis("😀😁", &mut out);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn stop_words_sorted() {
        let mut s = STOP_WORDS.to_vec();
        s.sort_unstable();
        assert_eq!(s.as_slice(), STOP_WORDS, "STOP_WORDS must stay sorted");
    }
}

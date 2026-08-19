//! gizza-ai/chat-log-analyzer core — parse an IRC or generic chat log and report
//! who talked most, activity by hour and weekday, word frequency, links shared,
//! and channel events. Pure-Rust, dependency-free (no regex, no clock): the whole
//! report is a deterministic function of the input text and the options.
//!
//! Dialects recognised without regex (timestamps are optional everywhere):
//!   irssi / HexChat / mIRC  `21:07 <alice> hey`, `[21:07:33] <@alice> hey`
//!   dated logs             `2024-01-05 21:07:33 <alice> hey`, `2024-01-05T21:07 <alice> hey`
//!   WeeChat columns        `2024-01-05 21:07:33\talice\they`
//!   plain transcripts      `21:07 alice: hey`, `alice: hey`
//!   actions                `* alice waves`, ` *\talice waves`
//!   events                 `-!- alice [~a@h] has joined #chan`, `--> alice ...`,
//!                          `*** Joins: alice (u@h)`, `<-- alice has quit`
//! Event lines never count towards message/word/link stats; `/me` actions do
//! (they are also tallied separately), matching how IRC stats tools count lines.

use std::collections::{HashMap, HashSet};

/// Longest input accepted, in bytes. Roughly a 5 MB log.
pub const MAX_INPUT_BYTES: usize = 5_000_000;

const BAR_WIDTH: usize = 24;

const WEEKDAYS: [&str; 7] = [
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
    "Sunday",
];

/// Common English filler words plus chat-specific noise, dropped from the word
/// ranking when `ignore_stopwords` is on.
const STOPWORDS: &[&str] = &[
    "a",
    "about",
    "after",
    "again",
    "all",
    "also",
    "am",
    "an",
    "and",
    "any",
    "are",
    "aren't",
    "as",
    "at",
    "back",
    "be",
    "because",
    "been",
    "before",
    "being",
    "but",
    "by",
    "can",
    "can't",
    "cause",
    "could",
    "did",
    "didn't",
    "do",
    "does",
    "doesn't",
    "doing",
    "don't",
    "down",
    "even",
    "ever",
    "every",
    "for",
    "from",
    "get",
    "gets",
    "getting",
    "give",
    "go",
    "going",
    "good",
    "got",
    "had",
    "has",
    "have",
    "haven't",
    "having",
    "he",
    "her",
    "here",
    "hers",
    "him",
    "his",
    "how",
    "i",
    "i'm",
    "i've",
    "if",
    "in",
    "into",
    "is",
    "isn't",
    "it",
    "it's",
    "its",
    "just",
    "know",
    "let",
    "like",
    "little",
    "look",
    "made",
    "make",
    "many",
    "may",
    "me",
    "mean",
    "might",
    "more",
    "most",
    "much",
    "must",
    "my",
    "need",
    "never",
    "new",
    "no",
    "not",
    "now",
    "of",
    "off",
    "on",
    "once",
    "one",
    "only",
    "or",
    "other",
    "our",
    "out",
    "over",
    "own",
    "really",
    "right",
    "said",
    "same",
    "say",
    "see",
    "she",
    "should",
    "since",
    "so",
    "some",
    "something",
    "still",
    "such",
    "sure",
    "take",
    "than",
    "that",
    "that's",
    "the",
    "their",
    "them",
    "then",
    "there",
    "there's",
    "these",
    "they",
    "thing",
    "things",
    "think",
    "this",
    "those",
    "though",
    "through",
    "time",
    "to",
    "too",
    "try",
    "two",
    "up",
    "us",
    "use",
    "used",
    "very",
    "want",
    "was",
    "wasn't",
    "way",
    "we",
    "well",
    "went",
    "were",
    "what",
    "when",
    "where",
    "which",
    "while",
    "who",
    "why",
    "will",
    "with",
    "won't",
    "would",
    "yeah",
    "yep",
    "yes",
    "yet",
    "you",
    "you're",
    "you've",
    "your",
    "yours",
    // chat noise
    "afk",
    "brb",
    "btw",
    "hey",
    "hi",
    "hello",
    "haha",
    "hmm",
    "imo",
    "lol",
    "nope",
    "ok",
    "okay",
    "omg",
    "thanks",
    "thx",
    "wtf",
];

/// Shape of the returned report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// Human-readable plain-text report (default).
    Summary,
    /// Machine-readable JSON object with the same numbers.
    Json,
}

impl OutputFormat {
    pub fn parse(s: &str) -> OutputFormat {
        match s.trim().to_ascii_lowercase().as_str() {
            "json" => OutputFormat::Json,
            _ => OutputFormat::Summary,
        }
    }
}

/// A date exactly as written, before the field order is resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RawDate {
    n0: i64,
    n1: i64,
    n2: i64,
    /// First field was a 4-digit year (ISO order, unambiguous).
    iso: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Order {
    Dmy,
    Mdy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EventKind {
    Join,
    Part,
    Quit,
    NickChange,
    Kick,
    Mode,
    Topic,
    Other,
}

#[derive(Debug, Clone, PartialEq)]
enum Kind {
    Message,
    Action,
    Event(EventKind),
}

#[derive(Debug, Clone)]
struct Entry {
    kind: Kind,
    nick: Option<String>,
    time: Option<(u32, u32)>,
    date: Option<RawDate>,
    text: String,
}

#[derive(Debug, Clone, Default)]
struct Person {
    name: String,
    msgs: usize,
    words: usize,
    chars: usize,
}

// ---------------------------------------------------------------- entry point

/// Analyze a chat/IRC log and render the report.
///
/// * `top` — how many people / words / link domains to list (0 = all).
/// * `min_word_length` — words shorter than this are skipped in the ranking.
/// * `ignore_stopwords` — drop common English filler words from the ranking.
/// * `exclude_nicks` — comma-separated nicks to drop entirely (bots); a trailing
///   `*` matches by prefix, e.g. `travis*`.
pub fn analyze(
    log: &str,
    output: OutputFormat,
    top: usize,
    min_word_length: usize,
    ignore_stopwords: bool,
    exclude_nicks: &str,
) -> Result<String, String> {
    if log.trim().is_empty() {
        return Err("chat log is empty — paste at least one line of a chat or IRC log".into());
    }
    if log.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "chat log is too large: {} bytes (maximum {} bytes)",
            log.len(),
            MAX_INPUT_BYTES
        ));
    }
    let min_word_length = min_word_length.max(1);
    let excludes = parse_excludes(exclude_nicks);

    let mut entries: Vec<Entry> = Vec::new();
    let mut total_lines = 0usize;
    let mut unparsed = 0usize;
    for raw in log.lines() {
        let line = raw.trim_end_matches(['\r', '\n']);
        if line.trim().is_empty() {
            continue;
        }
        total_lines += 1;
        match parse_line(line) {
            Some(e) => entries.push(e),
            None => unparsed += 1,
        }
    }

    // Resolve ambiguous numeric dates once, globally, from every date seen.
    let order = detect_order(&entries);

    let mut people: HashMap<String, Person> = HashMap::new();
    let mut order_seen: Vec<String> = Vec::new();
    let mut hours = [0usize; 24];
    let mut weekdays = [0usize; 7];
    let mut days: HashSet<(i64, i64, i64)> = HashSet::new();
    let mut words: HashMap<String, usize> = HashMap::new();
    let mut domains: HashMap<String, usize> = HashMap::new();
    let mut links_total = 0usize;
    let mut ev_counts = [0usize; 8];
    let mut actions = 0usize;
    let mut event_lines = 0usize;
    let mut excluded_lines = 0usize;
    let mut total_msgs = 0usize;
    let mut total_words = 0usize;
    let mut total_chars = 0usize;
    let mut first_stamp: Option<String> = None;
    let mut last_stamp: Option<String> = None;

    for e in &entries {
        let nick_excluded = e
            .nick
            .as_deref()
            .map(|n| is_excluded(n, &excludes))
            .unwrap_or(false);
        if nick_excluded {
            excluded_lines += 1;
            continue;
        }

        if let Kind::Event(kind) = e.kind {
            event_lines += 1;
            ev_counts[event_index(kind)] += 1;
            continue;
        }
        if e.kind == Kind::Action {
            actions += 1;
        }

        let nick = match &e.nick {
            Some(n) if !n.is_empty() => n.clone(),
            _ => continue,
        };
        total_msgs += 1;

        let stamp = format_stamp(e, order);
        if let Some(s) = stamp {
            if first_stamp.is_none() {
                first_stamp = Some(s.clone());
            }
            last_stamp = Some(s);
        }
        if let Some((h, _)) = e.time {
            hours[h as usize % 24] += 1;
        }
        if let Some(d) = e.date {
            if let Some((y, m, day)) = resolve_date(d, order) {
                weekdays[weekday_index(y, m, day)] += 1;
                days.insert((y, m, day));
            }
        }

        let msg_words = e.text.split_whitespace().count();
        let msg_chars = e.text.chars().count();
        total_words += msg_words;
        total_chars += msg_chars;

        let key = nick.to_lowercase();
        if !people.contains_key(&key) {
            order_seen.push(key.clone());
        }
        let p = people.entry(key).or_insert_with(|| Person {
            name: nick.clone(),
            ..Person::default()
        });
        p.msgs += 1;
        p.words += msg_words;
        p.chars += msg_chars;

        for tok in e.text.split_whitespace() {
            if let Some(dom) = link_domain(tok) {
                links_total += 1;
                *domains.entry(dom).or_insert(0) += 1;
                continue;
            }
            if let Some(w) = normalize_word(tok) {
                if w.chars().count() < min_word_length {
                    continue;
                }
                if ignore_stopwords && STOPWORDS.contains(&w.as_str()) {
                    continue;
                }
                *words.entry(w).or_insert(0) += 1;
            }
        }
    }

    if total_msgs == 0 {
        return Err(format!(
            "no chat messages found in {} non-empty line(s). Expected lines like \
             '21:07 <alice> hey', '2024-01-05 21:07:33 <alice> hey', 'alice: hey', or a \
             tab-separated WeeChat log. Event-only logs (joins/parts) have no messages to count.",
            total_lines
        ));
    }

    // Rankings — ties broken by name so output is fully deterministic.
    let mut ranked: Vec<&Person> = order_seen.iter().filter_map(|k| people.get(k)).collect();
    ranked.sort_by(|a, b| b.msgs.cmp(&a.msgs).then_with(|| a.name.cmp(&b.name)));
    let top_words = rank_map(&words, top);
    let top_domains = rank_map(&domains, top);
    let shown_n = if top == 0 {
        ranked.len()
    } else {
        top.min(ranked.len())
    };
    let people_shown: &[&Person] = &ranked[..shown_n];

    let busiest_hour = hours
        .iter()
        .enumerate()
        .max_by_key(|(i, c)| (**c, usize::MAX - *i))
        .filter(|(_, c)| **c > 0)
        .map(|(i, c)| (i as u32, *c));
    let busiest_day = weekdays
        .iter()
        .enumerate()
        .max_by_key(|(i, c)| (**c, usize::MAX - *i))
        .filter(|(_, c)| **c > 0)
        .map(|(i, c)| (i, *c));

    let stats = Stats {
        total_msgs,
        participants: ranked.len(),
        total_words,
        total_chars,
        total_lines,
        event_lines,
        unparsed,
        excluded_lines,
        actions,
        first_stamp,
        last_stamp,
        days: days.len(),
        hours,
        weekdays,
        busiest_hour,
        busiest_day,
        ev_counts,
        links_total,
        min_word_length,
        ignore_stopwords,
    };

    Ok(match output {
        OutputFormat::Summary => render_summary(&stats, people_shown, &top_words, &top_domains),
        OutputFormat::Json => render_json(&stats, people_shown, &top_words, &top_domains),
    })
}

struct Stats {
    total_msgs: usize,
    participants: usize,
    total_words: usize,
    total_chars: usize,
    total_lines: usize,
    event_lines: usize,
    unparsed: usize,
    excluded_lines: usize,
    actions: usize,
    first_stamp: Option<String>,
    last_stamp: Option<String>,
    days: usize,
    hours: [usize; 24],
    weekdays: [usize; 7],
    busiest_hour: Option<(u32, usize)>,
    busiest_day: Option<(usize, usize)>,
    ev_counts: [usize; 8],
    links_total: usize,
    min_word_length: usize,
    ignore_stopwords: bool,
}

// -------------------------------------------------------------------- parsing

fn parse_line(line: &str) -> Option<Entry> {
    let (date, time, rest) = strip_timestamp(line);
    let (kind, nick, text) = classify(rest)?;
    Some(Entry {
        kind,
        nick,
        time,
        date,
        text,
    })
}

/// Consume a leading timestamp (bracketed or bare) and return what is left.
fn strip_timestamp(line: &str) -> (Option<RawDate>, Option<(u32, u32)>, &str) {
    let s = line.trim_start_matches([' ', '\u{feff}']);
    if let Some(stripped) = s.strip_prefix('[') {
        if let Some(close) = stripped.find(']') {
            let inside = &stripped[..close];
            if let Some((d, t)) = parse_stamp_fields(inside) {
                if d.is_some() || t.is_some() {
                    let rest = stripped[close + 1..].trim_start_matches(' ');
                    return (d, t, rest);
                }
            }
        }
    }
    strip_bare_timestamp(s)
}

/// Parse the whole of a bracketed timestamp body, e.g. `2024-01-05, 21:07:33`.
fn parse_stamp_fields(inside: &str) -> Option<(Option<RawDate>, Option<(u32, u32)>)> {
    let mut date = None;
    let mut time = None;
    let body = inside.replace(',', " ");
    for tok in body.split_whitespace() {
        if let Some((d, t)) = parse_iso_combined(tok) {
            if date.is_some() || time.is_some() {
                return None;
            }
            date = Some(d);
            time = Some(t);
            continue;
        }
        if date.is_none() && time.is_none() {
            if let Some(d) = parse_date(tok) {
                date = Some(d);
                continue;
            }
        }
        if time.is_none() {
            if let Some(t) = parse_time(tok) {
                time = Some(t);
                continue;
            }
        }
        if let Some(t) = time.as_mut() {
            if let Some(adj) = apply_meridiem(tok, t.0) {
                t.0 = adj;
                continue;
            }
        }
        return None;
    }
    if date.is_none() && time.is_none() {
        return None;
    }
    Some((date, time))
}

/// Consume an unbracketed leading `date? time? (am|pm)?` prefix.
fn strip_bare_timestamp(s: &str) -> (Option<RawDate>, Option<(u32, u32)>, &str) {
    let mut date: Option<RawDate> = None;
    let mut time: Option<(u32, u32)> = None;
    let mut pos = 0usize;
    loop {
        // Only spaces separate timestamp fields; a tab starts the WeeChat body.
        let start = pos + s[pos..].len() - s[pos..].trim_start_matches(' ').len();
        if start >= s.len() {
            break;
        }
        let tok_end = s[start..]
            .find([' ', '\t'])
            .map(|i| start + i)
            .unwrap_or(s.len());
        let tok = &s[start..tok_end];
        if tok.is_empty() {
            break;
        }
        if date.is_none() && time.is_none() {
            if let Some((d, t)) = parse_iso_combined(tok) {
                date = Some(d);
                time = Some(t);
                pos = tok_end;
                continue;
            }
            if let Some(d) = parse_date(tok) {
                date = Some(d);
                pos = tok_end;
                continue;
            }
        }
        if time.is_none() {
            if let Some(t) = parse_time(tok) {
                time = Some(t);
                pos = tok_end;
                continue;
            }
        } else if let Some(t) = time.as_mut() {
            if let Some(adj) = apply_meridiem(tok, t.0) {
                t.0 = adj;
                pos = tok_end;
                continue;
            }
        }
        break;
    }
    if date.is_none() && time.is_none() {
        return (None, None, s);
    }
    (date, time, s[pos..].trim_start_matches([' ', '\t']))
}

fn parse_iso_combined(tok: &str) -> Option<(RawDate, (u32, u32))> {
    let (d, t) = tok.split_once('T')?;
    Some((parse_date(d)?, parse_time(t)?))
}

/// `2024-01-05`, `05/01/2024`, `05.01.24` — order is resolved later.
fn parse_date(tok: &str) -> Option<RawDate> {
    let sep = ['-', '/', '.']
        .into_iter()
        .find(|c| tok.matches(*c).count() == 2)?;
    let parts: Vec<&str> = tok.split(sep).collect();
    if parts.len() != 3 {
        return None;
    }
    let mut nums = [0i64; 3];
    for (i, p) in parts.iter().enumerate() {
        if p.is_empty() || p.len() > 4 || !p.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        nums[i] = p.parse::<i64>().ok()?;
    }
    let iso = parts[0].len() == 4;
    if iso {
        if !(1..=12).contains(&nums[1]) || !(1..=31).contains(&nums[2]) {
            return None;
        }
    } else {
        if !(1..=31).contains(&nums[0]) || !(1..=31).contains(&nums[1]) {
            return None;
        }
        if parts[2].len() != 2 && parts[2].len() != 4 {
            return None;
        }
    }
    Some(RawDate {
        n0: nums[0],
        n1: nums[1],
        n2: nums[2],
        iso,
    })
}

/// `21:07`, `21:07:33`, `9:07pm` — returns (hour, minute) on a 24-hour clock.
fn parse_time(tok: &str) -> Option<(u32, u32)> {
    let lower = tok.to_ascii_lowercase();
    let (body, meridiem) = if let Some(b) = lower.strip_suffix("am") {
        (b.trim_end(), Some(false))
    } else if let Some(b) = lower.strip_suffix("pm") {
        (b.trim_end(), Some(true))
    } else {
        (lower.as_str(), None)
    };
    let parts: Vec<&str> = body.split(':').collect();
    if parts.len() < 2 || parts.len() > 3 {
        return None;
    }
    for p in &parts {
        if p.is_empty() || p.len() > 2 || !p.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
    }
    let mut h: u32 = parts[0].parse().ok()?;
    let m: u32 = parts[1].parse().ok()?;
    if parts.len() == 3 && parts[2].parse::<u32>().ok()? > 59 {
        return None;
    }
    if m > 59 {
        return None;
    }
    match meridiem {
        Some(pm) => {
            if !(1..=12).contains(&h) {
                return None;
            }
            h = match (h, pm) {
                (12, false) => 0,
                (12, true) => 12,
                (h, false) => h,
                (h, true) => h + 12,
            };
        }
        None => {
            if h > 23 {
                return None;
            }
        }
    }
    Some((h, m))
}

/// A standalone `AM`/`PM` token following an already-parsed time.
fn apply_meridiem(tok: &str, hour: u32) -> Option<u32> {
    match tok.to_ascii_lowercase().as_str() {
        "am" | "a.m." => Some(if hour == 12 { 0 } else { hour }),
        "pm" | "p.m." => Some(if hour == 12 { 12 } else { hour + 12 }),
        _ => None,
    }
}

/// Turn the post-timestamp remainder into a message, action, or event.
fn classify(rest: &str) -> Option<(Kind, Option<String>, String)> {
    if let Some(t) = rest.find('\t') {
        let col1 = rest[..t].trim();
        let col2 = rest[t + 1..].trim_start();
        if !col1.contains(' ') {
            match col1 {
                "--" | "-->" | "<--" | "=!=" | "-!-" | "***" | "" => {
                    let hint = marker_hint(col1);
                    return Some((
                        Kind::Event(event_kind_of(col2).unwrap_or(hint)),
                        None,
                        col2.to_string(),
                    ));
                }
                "*" | "* " => return action_from(col2),
                _ => {
                    if let Some(nick) = strip_modes(col1) {
                        return Some((Kind::Message, Some(nick), col2.to_string()));
                    }
                }
            }
        }
    }

    let line = rest.trim_start();
    if line.is_empty() {
        return None;
    }

    for marker in ["-->", "<--", "-!-", "***", "=!=", "<->"] {
        if let Some(body) = line.strip_prefix(marker) {
            let body = body.trim_start();
            let kind = event_kind_of(body).unwrap_or_else(|| marker_hint(marker));
            return Some((Kind::Event(kind), None, body.to_string()));
        }
    }

    if let Some(after) = line.strip_prefix('<') {
        if let Some(gt) = after.find('>') {
            let nick_raw = &after[..gt];
            if !nick_raw.is_empty() && !nick_raw.contains(' ') && nick_raw.len() <= 64 {
                if let Some(nick) = strip_modes(nick_raw) {
                    let text = after[gt + 1..].trim_start();
                    return Some((Kind::Message, Some(nick), text.to_string()));
                }
            }
        }
    }

    if let Some(body) = line.strip_prefix('*') {
        let body = body.trim_start();
        if !body.is_empty() {
            if let Some(kind) = event_kind_of(body) {
                return Some((Kind::Event(kind), None, body.to_string()));
            }
            return action_from(body);
        }
    }

    // `alice: hey` — a colon-terminated first token with no spaces in it.
    if let Some(i) = line.find(':') {
        let prefix = &line[..i];
        let after = &line[i + 1..];
        let follows_ok = after.is_empty() || after.starts_with(' ') || after.starts_with('\t');
        if follows_ok && !prefix.is_empty() && prefix.len() <= 64 && !prefix.contains(' ') {
            if let Some(nick) = strip_modes(prefix) {
                return Some((Kind::Message, Some(nick), after.trim_start().to_string()));
            }
        }
    }

    None
}

fn action_from(body: &str) -> Option<(Kind, Option<String>, String)> {
    let mut it = body.splitn(2, char::is_whitespace);
    let nick_raw = it.next()?;
    let text = it.next().unwrap_or("").trim_start();
    let nick = strip_modes(nick_raw)?;
    Some((Kind::Action, Some(nick), text.to_string()))
}

/// What an event marker means when the wording itself is unrecognised.
fn marker_hint(marker: &str) -> EventKind {
    match marker {
        "-->" => EventKind::Join,
        "<--" => EventKind::Part,
        _ => EventKind::Other,
    }
}

fn event_kind_of(body: &str) -> Option<EventKind> {
    let l = body.to_ascii_lowercase();
    let has = |needle: &str| l.contains(needle);
    if has("has joined") || l.starts_with("joins:") || has(" joined ") || l.ends_with(" joined") {
        return Some(EventKind::Join);
    }
    if has("has left") || has("has parted") || l.starts_with("parts:") || has(" left the ") {
        return Some(EventKind::Part);
    }
    if has("has quit") || l.starts_with("quits:") || has(" quit ") || l.ends_with(" quit") {
        return Some(EventKind::Quit);
    }
    if has("now known as") || l.starts_with("nick:") || has("changed nick") {
        return Some(EventKind::NickChange);
    }
    if has("was kicked") || has("kicked by") || l.starts_with("kicks:") {
        return Some(EventKind::Kick);
    }
    if has("sets mode") || has("set mode") || l.starts_with("mode:") {
        return Some(EventKind::Mode);
    }
    if has("changed the topic") || has("changed topic") || l.starts_with("topic:") {
        return Some(EventKind::Topic);
    }
    None
}

fn event_index(k: EventKind) -> usize {
    match k {
        EventKind::Join => 0,
        EventKind::Part => 1,
        EventKind::Quit => 2,
        EventKind::NickChange => 3,
        EventKind::Kick => 4,
        EventKind::Mode => 5,
        EventKind::Topic => 6,
        EventKind::Other => 7,
    }
}

/// Drop IRC mode/status prefixes (`@op`, `+voice`) and stray punctuation.
fn strip_modes(nick: &str) -> Option<String> {
    let n = nick
        .trim()
        .trim_start_matches(['@', '+', '%', '~', '&', '!'])
        .trim_end_matches([':', '>'])
        .trim();
    if n.is_empty() || n.len() > 64 {
        return None;
    }
    if !n.chars().any(|c| c.is_alphanumeric()) {
        return None;
    }
    Some(n.to_string())
}

// ------------------------------------------------------------------ nick rules

fn parse_excludes(list: &str) -> Vec<String> {
    list.split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

fn is_excluded(nick: &str, excludes: &[String]) -> bool {
    let n = nick.to_lowercase();
    excludes.iter().any(|p| match p.strip_suffix('*') {
        Some(prefix) => n.starts_with(prefix),
        None => n == *p,
    })
}

// ------------------------------------------------------------------ text rules

/// `https://example.com/x` / `www.example.com` → `example.com`.
fn link_domain(tok: &str) -> Option<String> {
    let t = tok.trim_matches(|c: char| "<>()[]{}\"'`,;!?".contains(c));
    let lower = t.to_lowercase();
    let host = if let Some(r) = lower.strip_prefix("https://") {
        r
    } else if let Some(r) = lower.strip_prefix("http://") {
        r
    } else if lower.starts_with("www.") {
        lower.as_str()
    } else {
        return None;
    };
    let host = host
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .split('@')
        .next_back()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
        .trim_end_matches('.');
    let host = host.strip_prefix("www.").unwrap_or(host);
    if host.is_empty() || !host.contains('.') {
        return None;
    }
    Some(host.to_string())
}

/// Lowercase a token and strip surrounding punctuation; `None` if nothing is left.
fn normalize_word(tok: &str) -> Option<String> {
    let w: String = tok
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '\'' || *c == '-' || *c == '_')
        .collect::<String>()
        .to_lowercase();
    let w = w.trim_matches(|c: char| c == '\'' || c == '-' || c == '_');
    if w.is_empty() || !w.chars().any(|c| c.is_alphabetic()) {
        return None;
    }
    Some(w.to_string())
}

// ------------------------------------------------------------------ date logic

fn detect_order(entries: &[Entry]) -> Order {
    let mut saw_high_first = false;
    let mut saw_high_second = false;
    for e in entries {
        if let Some(d) = e.date {
            if d.iso {
                continue;
            }
            if d.n0 > 12 {
                saw_high_first = true;
            }
            if d.n1 > 12 {
                saw_high_second = true;
            }
        }
    }
    if saw_high_first {
        Order::Dmy
    } else if saw_high_second {
        Order::Mdy
    } else {
        Order::Dmy
    }
}

fn resolve_date(d: RawDate, order: Order) -> Option<(i64, i64, i64)> {
    let (y, m, day) = if d.iso {
        (d.n0, d.n1, d.n2)
    } else {
        match order {
            Order::Dmy => (d.n2, d.n1, d.n0),
            Order::Mdy => (d.n2, d.n0, d.n1),
        }
    };
    let y = if y < 100 {
        if y < 70 {
            y + 2000
        } else {
            y + 1900
        }
    } else {
        y
    };
    if !(1..=12).contains(&m) || !(1..=31).contains(&day) {
        return None;
    }
    Some((y, m, day))
}

/// 0 = Monday. Days-from-civil, valid for any proleptic Gregorian date.
fn weekday_index(y: i64, m: i64, d: i64) -> usize {
    let y2 = if m <= 2 { y - 1 } else { y };
    let era = if y2 >= 0 { y2 } else { y2 - 399 } / 400;
    let yoe = y2 - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468; // days since 1970-01-01, a Thursday
    (((days % 7) + 7 + 3) % 7) as usize
}

fn format_stamp(e: &Entry, order: Order) -> Option<String> {
    let time = e.time.map(|(h, m)| format!("{:02}:{:02}", h, m));
    let date = e
        .date
        .and_then(|d| resolve_date(d, order))
        .map(|(y, m, day)| format!("{:04}-{:02}-{:02}", y, m, day));
    match (date, time) {
        (Some(d), Some(t)) => Some(format!("{} {}", d, t)),
        (Some(d), None) => Some(d),
        (None, Some(t)) => Some(t),
        (None, None) => None,
    }
}

// ------------------------------------------------------------------- rendering

fn rank_map(map: &HashMap<String, usize>, top: usize) -> Vec<(String, usize)> {
    let mut v: Vec<(String, usize)> = map.iter().map(|(k, c)| (k.clone(), *c)).collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    if top > 0 {
        v.truncate(top);
    }
    v
}

fn pct(n: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        n as f64 * 100.0 / total as f64
    }
}

/// Per-message average (words or characters), 0 when a person has no messages.
fn per_msg(n: usize, msgs: usize) -> f64 {
    if msgs == 0 {
        0.0
    } else {
        n as f64 / msgs as f64
    }
}

fn bar(n: usize, max: usize) -> String {
    if n == 0 || max == 0 {
        return String::new();
    }
    let len = ((n * BAR_WIDTH) / max).max(1);
    "#".repeat(len)
}

fn render_summary(
    s: &Stats,
    people: &[&Person],
    top_words: &[(String, usize)],
    top_domains: &[(String, usize)],
) -> String {
    let mut r = String::new();
    r.push_str("Chat log analysis\n");
    r.push_str("=================\n");
    r.push_str(&format!("Messages: {}\n", s.total_msgs));
    r.push_str(&format!("Participants: {}\n", s.participants));
    r.push_str(&format!("Words: {}\n", s.total_words));
    r.push_str(&format!("Characters: {}\n", s.total_chars));
    match (&s.first_stamp, &s.last_stamp) {
        (Some(a), Some(b)) => {
            r.push_str(&format!("Time span: {} -> {}", a, b));
            if s.days > 0 {
                r.push_str(&format!(" ({} day(s) with messages)", s.days));
            }
            r.push('\n');
        }
        _ => r.push_str("Time span: (no timestamps in the log)\n"),
    }
    if let Some((h, c)) = s.busiest_hour {
        r.push_str(&format!(
            "Busiest hour: {:02}:00 ({} messages, {:.2}%)\n",
            h,
            c,
            pct(c, s.total_msgs)
        ));
    }
    if let Some((d, c)) = s.busiest_day {
        r.push_str(&format!(
            "Busiest weekday: {} ({} messages)\n",
            WEEKDAYS[d], c
        ));
    }
    r.push_str(&format!(
        "Lines read: {} ({} messages, {} events, {} excluded, {} unrecognized)\n",
        s.total_lines, s.total_msgs, s.event_lines, s.excluded_lines, s.unparsed
    ));

    r.push_str("\nWho talked most\n");
    r.push_str("---------------\n");
    if people.len() < s.participants {
        r.push_str(&format!(
            "(top {} of {} participants)\n",
            people.len(),
            s.participants
        ));
    }
    r.push_str("  MSGS   SHARE   WORDS   CHARS   W/MSG   C/MSG  NICK\n");
    for p in people {
        r.push_str(&format!(
            "{:>6}  {:>6.2}%  {:>6}  {:>6}  {:>6.1}  {:>6.1}  {}\n",
            p.msgs,
            pct(p.msgs, s.total_msgs),
            p.words,
            p.chars,
            per_msg(p.words, p.msgs),
            per_msg(p.chars, p.msgs),
            p.name
        ));
    }

    r.push_str("\nActivity by hour\n");
    r.push_str("----------------\n");
    let hmax = *s.hours.iter().max().unwrap_or(&0);
    if hmax == 0 {
        r.push_str("(no timestamps in the log)\n");
    } else {
        for (h, c) in s.hours.iter().enumerate() {
            if *c == 0 {
                continue;
            }
            r.push_str(&format!(
                "{:02}:00  {:>6}  {:>6.2}%  {}\n",
                h,
                c,
                pct(*c, s.total_msgs),
                bar(*c, hmax)
            ));
        }
    }

    r.push_str("\nActivity by weekday\n");
    r.push_str("-------------------\n");
    let wtotal: usize = s.weekdays.iter().sum();
    if wtotal == 0 {
        r.push_str("(no dated lines in the log)\n");
    } else {
        let wmax = *s.weekdays.iter().max().unwrap_or(&0);
        for (d, c) in s.weekdays.iter().enumerate() {
            if *c == 0 {
                continue;
            }
            r.push_str(&format!(
                "{:<10}  {:>6}  {:>6.2}%  {}\n",
                WEEKDAYS[d],
                c,
                pct(*c, wtotal),
                bar(*c, wmax)
            ));
        }
    }

    r.push_str(&format!("\nTop words (min length {}", s.min_word_length));
    if s.ignore_stopwords {
        r.push_str(", stop words removed");
    }
    r.push_str(")\n");
    r.push_str("---------------------------------\n");
    if top_words.is_empty() {
        r.push_str("(none)\n");
    } else {
        for (w, c) in top_words {
            r.push_str(&format!("{:>6}  {}\n", c, w));
        }
    }

    r.push_str("\nLinks shared\n");
    r.push_str("------------\n");
    if s.links_total == 0 {
        r.push_str("(none)\n");
    } else {
        r.push_str(&format!(
            "{} link(s) from {} domain(s)\n",
            s.links_total,
            top_domains.len()
        ));
        for (d, c) in top_domains {
            r.push_str(&format!("{:>6}  {}\n", c, d));
        }
    }

    r.push_str("\nChannel events\n");
    r.push_str("--------------\n");
    let labels = [
        "Joins",
        "Parts",
        "Quits",
        "Nick changes",
        "Kicks",
        "Mode changes",
        "Topic changes",
        "Other",
    ];
    let mut any = false;
    for (i, c) in s.ev_counts.iter().enumerate() {
        if *c == 0 {
            continue;
        }
        any = true;
        r.push_str(&format!("{}: {}\n", labels[i], c));
    }
    if s.actions > 0 {
        any = true;
        r.push_str(&format!(
            "Actions (/me): {} (counted as messages)\n",
            s.actions
        ));
    }
    if !any {
        r.push_str("(none)\n");
    }
    r
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

fn json_str(v: &Option<String>) -> String {
    match v {
        Some(s) => format!("\"{}\"", json_escape(s)),
        None => "null".to_string(),
    }
}

fn render_json(
    s: &Stats,
    people: &[&Person],
    top_words: &[(String, usize)],
    top_domains: &[(String, usize)],
) -> String {
    let mut r = String::new();
    r.push_str("{\n");
    r.push_str(&format!("  \"messages\": {},\n", s.total_msgs));
    r.push_str(&format!("  \"participants\": {},\n", s.participants));
    r.push_str(&format!("  \"words\": {},\n", s.total_words));
    r.push_str(&format!("  \"characters\": {},\n", s.total_chars));
    r.push_str(&format!("  \"lines_read\": {},\n", s.total_lines));
    r.push_str(&format!("  \"event_lines\": {},\n", s.event_lines));
    r.push_str(&format!("  \"excluded_lines\": {},\n", s.excluded_lines));
    r.push_str(&format!("  \"unrecognized_lines\": {},\n", s.unparsed));
    r.push_str(&format!(
        "  \"first_timestamp\": {},\n",
        json_str(&s.first_stamp)
    ));
    r.push_str(&format!(
        "  \"last_timestamp\": {},\n",
        json_str(&s.last_stamp)
    ));
    r.push_str(&format!("  \"days_with_messages\": {},\n", s.days));
    r.push_str(&format!(
        "  \"busiest_hour\": {},\n",
        match s.busiest_hour {
            Some((h, _)) => h.to_string(),
            None => "null".into(),
        }
    ));
    r.push_str(&format!(
        "  \"busiest_weekday\": {},\n",
        match s.busiest_day {
            Some((d, _)) => format!("\"{}\"", WEEKDAYS[d]),
            None => "null".into(),
        }
    ));

    r.push_str("  \"people\": [");
    for (i, p) in people.iter().enumerate() {
        r.push_str(if i == 0 { "\n" } else { ",\n" });
        r.push_str(&format!(
            "    {{ \"nick\": \"{}\", \"messages\": {}, \"share_percent\": {:.2}, \"words\": {}, \"characters\": {}, \"avg_words_per_message\": {:.2}, \"avg_characters_per_message\": {:.2} }}",
            json_escape(&p.name),
            p.msgs,
            pct(p.msgs, s.total_msgs),
            p.words,
            p.chars,
            per_msg(p.words, p.msgs),
            per_msg(p.chars, p.msgs)
        ));
    }
    r.push_str(if people.is_empty() {
        "],\n"
    } else {
        "\n  ],\n"
    });

    r.push_str("  \"activity_by_hour\": [");
    let mut first = true;
    for (h, c) in s.hours.iter().enumerate() {
        if *c == 0 {
            continue;
        }
        r.push_str(if first { "\n" } else { ",\n" });
        first = false;
        r.push_str(&format!("    {{ \"hour\": {}, \"messages\": {} }}", h, c));
    }
    r.push_str(if first { "],\n" } else { "\n  ],\n" });

    r.push_str("  \"activity_by_weekday\": [");
    let mut first = true;
    for (d, c) in s.weekdays.iter().enumerate() {
        if *c == 0 {
            continue;
        }
        r.push_str(if first { "\n" } else { ",\n" });
        first = false;
        r.push_str(&format!(
            "    {{ \"weekday\": \"{}\", \"messages\": {} }}",
            WEEKDAYS[d], c
        ));
    }
    r.push_str(if first { "],\n" } else { "\n  ],\n" });

    r.push_str("  \"top_words\": [");
    for (i, (w, c)) in top_words.iter().enumerate() {
        r.push_str(if i == 0 { "\n" } else { ",\n" });
        r.push_str(&format!(
            "    {{ \"word\": \"{}\", \"count\": {} }}",
            json_escape(w),
            c
        ));
    }
    r.push_str(if top_words.is_empty() {
        "],\n"
    } else {
        "\n  ],\n"
    });

    r.push_str("  \"links\": {\n");
    r.push_str(&format!("    \"total\": {},\n", s.links_total));
    r.push_str("    \"domains\": [");
    for (i, (d, c)) in top_domains.iter().enumerate() {
        r.push_str(if i == 0 { "\n" } else { ",\n" });
        r.push_str(&format!(
            "      {{ \"domain\": \"{}\", \"count\": {} }}",
            json_escape(d),
            c
        ));
    }
    r.push_str(if top_domains.is_empty() {
        "]\n  },\n"
    } else {
        "\n    ]\n  },\n"
    });

    r.push_str("  \"events\": {\n");
    let keys = [
        "joins",
        "parts",
        "quits",
        "nick_changes",
        "kicks",
        "mode_changes",
        "topic_changes",
        "other",
    ];
    for (i, k) in keys.iter().enumerate() {
        r.push_str(&format!("    \"{}\": {},\n", k, s.ev_counts[i]));
    }
    r.push_str(&format!("    \"actions\": {}\n", s.actions));
    r.push_str("  }\n");
    r.push_str("}\n");
    r
}

// ----------------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;

    const IRC: &str = "\
2024-01-05 21:07:33 <alice> pizza tonight? https://example.com/menu
2024-01-05 21:08:01 <@bob> pizza sounds great
2024-01-05 21:09:00 -!- carol [~c@host] has joined #food
2024-01-06 09:15:00 <bob> morning
2024-01-06 09:16:10 * alice waves";

    fn run(log: &str) -> String {
        analyze(log, OutputFormat::Summary, 10, 3, true, "").unwrap()
    }

    #[test]
    fn irc_log_happy_path() {
        let out = run(IRC);
        assert!(out.contains("Messages: 4"), "{out}");
        assert!(out.contains("Participants: 2"), "{out}");
        assert!(
            out.contains("Time span: 2024-01-05 21:07 -> 2024-01-06 09:16"),
            "{out}"
        );
        assert!(out.contains("Busiest hour: 09:00"), "{out}");
        // alice: 2 messages (one is an action), bob: 2 — alice first on the name tie-break.
        assert!(out.contains("     2   50.00%"), "{out}");
        assert!(out.contains("Joins: 1"), "{out}");
        assert!(out.contains("Actions (/me): 1"), "{out}");
        assert!(out.contains("     2  pizza"), "{out}");
        assert!(out.contains("1 link(s) from 1 domain(s)"), "{out}");
        assert!(out.contains("     1  example.com"), "{out}");
        assert!(out.contains("Friday"), "{out}");
        assert!(out.contains("Saturday"), "{out}");
    }

    #[test]
    fn empty_log_is_an_error() {
        let err = analyze("   \n\n ", OutputFormat::Summary, 10, 3, true, "").unwrap_err();
        assert!(err.contains("empty"), "{err}");
    }

    #[test]
    fn event_only_log_is_an_error() {
        let err = analyze(
            "-!- alice has joined #chan\n-!- alice has quit [Ping timeout]",
            OutputFormat::Summary,
            10,
            3,
            true,
            "",
        )
        .unwrap_err();
        assert!(err.contains("no chat messages found"), "{err}");
    }

    #[test]
    fn oversized_log_is_an_error() {
        let big = "x".repeat(MAX_INPUT_BYTES + 1);
        let err = analyze(&big, OutputFormat::Summary, 10, 3, true, "").unwrap_err();
        assert!(err.contains("too large"), "{err}");
    }

    #[test]
    fn bracketed_and_bare_times_both_parse() {
        let out = run("[21:07] <alice> hey there friend\n21:08 <bob> hello friend");
        assert!(out.contains("Messages: 2"), "{out}");
        assert!(out.contains("21:00"), "{out}");
        assert!(out.contains("(no dated lines in the log)"), "{out}");
    }

    #[test]
    fn plain_nick_colon_lines_parse_without_timestamps() {
        let out = run("alice: deploy is green\nbob: deploy looks good\nalice: shipping");
        assert!(out.contains("Messages: 3"), "{out}");
        assert!(
            out.contains("Time span: (no timestamps in the log)"),
            "{out}"
        );
        assert!(out.contains("(no timestamps in the log)"), "{out}");
        assert!(out.contains("     2  deploy"), "{out}");
    }

    #[test]
    fn weechat_tab_columns_parse() {
        let out = run("2024-01-05 21:07:33\talice\thello there everyone\n2024-01-05 21:08:00\t--\tbob has joined");
        assert!(out.contains("Messages: 1"), "{out}");
        assert!(out.contains("Joins: 1"), "{out}");
    }

    #[test]
    fn url_at_line_start_is_not_a_nick() {
        let out = run("<alice> https://rust-lang.org is the site");
        assert!(out.contains("Participants: 1"), "{out}");
        assert!(out.contains("rust-lang.org"), "{out}");
    }

    #[test]
    fn excluded_nicks_are_dropped_including_wildcards() {
        let log = "<alice> real message here\n<gizzabot> automated notice one\n<gizzabot2> automated notice two";
        let all = run(log);
        assert!(all.contains("Messages: 3"), "{all}");
        let filtered = analyze(log, OutputFormat::Summary, 10, 3, true, "gizzabot*").unwrap();
        assert!(filtered.contains("Messages: 1"), "{filtered}");
        assert!(filtered.contains("Participants: 1"), "{filtered}");
        assert!(!filtered.contains("automated"), "{filtered}");
    }

    #[test]
    fn stopwords_and_min_length_are_respected() {
        let log = "<alice> the cat and the hat\n<bob> the cat";
        let kept = analyze(log, OutputFormat::Summary, 10, 1, false, "").unwrap();
        assert!(kept.contains("     3  the"), "{kept}");
        let dropped = analyze(log, OutputFormat::Summary, 10, 3, true, "").unwrap();
        assert!(!dropped.contains("  the\n"), "{dropped}");
        assert!(dropped.contains("     2  cat"), "{dropped}");
    }

    #[test]
    fn top_caps_the_rankings_and_zero_means_all() {
        let log = "<alice> alpha bravo charlie delta\n<bob> alpha bravo charlie\n<carol> alpha bravo\n<dave> alpha";
        let capped = analyze(log, OutputFormat::Summary, 2, 3, true, "").unwrap();
        assert!(capped.contains("(top 2 of 4 participants)"), "{capped}");
        assert!(!capped.contains("delta"), "{capped}");
        let all = analyze(log, OutputFormat::Summary, 0, 3, true, "").unwrap();
        assert!(all.contains("Participants: 4"), "{all}");
        assert!(all.contains("delta"), "{all}");
    }

    #[test]
    fn json_output_is_valid_and_carries_the_same_numbers() {
        let out = analyze(IRC, OutputFormat::Json, 10, 3, true, "").unwrap();
        assert!(out.contains("\"messages\": 4"), "{out}");
        assert!(out.contains("\"busiest_hour\": 9"), "{out}");
        assert!(out.contains("\"busiest_weekday\": \"Friday\""), "{out}");
        assert!(out.contains("\"joins\": 1"), "{out}");
        assert!(out.contains("\"actions\": 1"), "{out}");
        assert!(out.contains("\"domain\": \"example.com\""), "{out}");
        // Braces balance — a hand-built JSON string that doesn't is malformed.
        assert_eq!(
            out.matches('{').count(),
            out.matches('}').count(),
            "unbalanced braces: {out}"
        );
        assert_eq!(out.matches('[').count(), out.matches(']').count(), "{out}");
    }

    #[test]
    fn ambiguous_dates_resolve_day_first_unless_proven_otherwise() {
        // A second field of 31 can only be a day → month-first.
        let out = run("01/31/2024 10:00 <alice> hello there\n02/01/2024 10:00 <alice> again now");
        assert!(out.contains("Wednesday"), "{out}"); // 2024-01-31
                                                     // A first field of 31 can only be a day → day-first.
        let out2 = run("31/01/2024 10:00 <alice> hello there\n01/02/2024 10:00 <alice> again now");
        assert!(out2.contains("Wednesday"), "{out2}");
    }

    #[test]
    fn twelve_hour_times_convert() {
        let out = run("[9:07 PM] <alice> evening message here\n[9:07 AM] <bob> morning message");
        assert!(out.contains("21:00"), "{out}");
        assert!(out.contains("09:00"), "{out}");
    }

    #[test]
    fn weekday_index_matches_known_dates() {
        assert_eq!(weekday_index(2024, 1, 5), 4); // Friday
        assert_eq!(weekday_index(2024, 1, 6), 5); // Saturday
        assert_eq!(weekday_index(2000, 2, 29), 1); // Tuesday
        assert_eq!(weekday_index(1970, 1, 1), 3); // Thursday
    }

    #[test]
    fn mode_prefixes_do_not_split_a_nick_into_two_people() {
        let out =
            run("<@alice> first message here\n<+alice> second message here\n<alice> third one");
        assert!(out.contains("Participants: 1"), "{out}");
        assert!(out.contains("Messages: 3"), "{out}");
    }

    #[test]
    fn per_message_averages_are_reported_for_words_and_characters() {
        // alice: 2 messages, 6 words, 26 chars → 3.0 w/msg, 13.0 c/msg.
        let log = "<alice> one two three\n<alice> four five six";
        let out = run(log);
        assert!(out.contains("W/MSG   C/MSG  NICK"), "{out}");
        assert!(out.contains("   3.0    13.0  alice"), "{out}");
        let json = analyze(log, OutputFormat::Json, 10, 3, true, "").unwrap();
        assert!(json.contains("\"avg_words_per_message\": 3.00"), "{json}");
        assert!(
            json.contains("\"avg_characters_per_message\": 13.00"),
            "{json}"
        );
    }

    #[test]
    fn unrecognized_lines_are_counted_not_guessed() {
        let out = run("<alice> real message here\nthis line has no nick marker at all");
        assert!(out.contains("1 unrecognized"), "{out}");
        assert!(out.contains("Messages: 1"), "{out}");
    }
}

//! gizza-ai/frequent-contacts-ranker core — rank the people in a mailbox export
//! by how often you exchange mail with them and how recently, so the top of the
//! list can be pasted straight into an address book / autocomplete list.
//!
//! Input is an mbox archive (Gmail Takeout, Thunderbird, Apple Mail) or one or
//! more raw RFC 5322 messages. Messages are split on the classic `From `
//! postmark lines, parsed with the pure-Rust `mail-parser`, and every address on
//! `From`/`To`/`Cc`/`Bcc` is folded to one row per person.
//!
//! Scoring combines both halves of the question:
//!   * **frequency** — how many messages that person appears in, and
//!   * **recency** — each message is weighted `0.5^(age_days / half_life_days)`,
//!     so old traffic fades instead of dominating forever.
//! The recency clock is the newest message in the paste (not the wall clock), so
//! the same archive always produces the same ranking, offline and repeatably.
//!
//! Pure-Rust, wasm-safe, no wafer/wasm-bindgen deps.

use mail_parser::{Address, MessageParser};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// Hard cap on messages parsed from one paste.
pub const MAX_MESSAGES: usize = 5000;

/// Seconds in a day, as an `f64` for the decay maths.
const DAY: f64 = 86_400.0;

/// Which side of the correspondence is ranked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Count {
    /// Only addresses you sent to (`To`, plus `Cc`/`Bcc` when enabled).
    Recipients,
    /// Only addresses that sent to you (`From`).
    Senders,
    /// Both directions folded into one ranking.
    Both,
}

impl Count {
    pub fn parse(s: &str) -> Result<Count, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "both" | "all" => Ok(Count::Both),
            "recipients" | "recipient" | "to" | "sent" => Ok(Count::Recipients),
            "senders" | "sender" | "from" | "received" => Ok(Count::Senders),
            other => Err(format!(
                "expected count to be \"recipients\", \"senders\" or \"both\", got \"{other}\""
            )),
        }
    }
    fn wants_recipients(self) -> bool {
        matches!(self, Count::Recipients | Count::Both)
    }
    fn wants_senders(self) -> bool {
        matches!(self, Count::Senders | Count::Both)
    }
}

/// Ordering of the ranked list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sort {
    /// Recency-weighted score, highest first (the default).
    Score,
    /// Raw message count, highest first.
    Messages,
    /// Most recently seen first.
    Recent,
    /// Display name (or address) A→Z.
    Name,
}

impl Sort {
    pub fn parse(s: &str) -> Result<Sort, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "score" => Ok(Sort::Score),
            "messages" | "count" | "frequency" => Ok(Sort::Messages),
            "recent" | "recency" | "last" => Ok(Sort::Recent),
            "name" | "alpha" | "alphabetical" => Ok(Sort::Name),
            other => Err(format!(
                "expected sort to be \"score\", \"messages\", \"recent\" or \"name\", got \"{other}\""
            )),
        }
    }
}

/// Output serialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Aligned rank table with a summary header line.
    Report,
    /// One `Name <address>` per line, ready to paste into an address book.
    List,
    /// RFC 4180 table.
    Csv,
    /// Structured object with the ranking plus archive metadata.
    Json,
}

impl Format {
    pub fn parse(s: &str) -> Result<Format, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "report" | "table" => Ok(Format::Report),
            "list" | "contacts" | "addresses" => Ok(Format::List),
            "csv" => Ok(Format::Csv),
            "json" => Ok(Format::Json),
            other => Err(format!(
                "expected format to be \"report\", \"list\", \"csv\" or \"json\", got \"{other}\""
            )),
        }
    }
}

/// Everything that steers the ranking.
#[derive(Debug, Clone)]
pub struct Options {
    pub count: Count,
    /// Count `Cc`/`Bcc` recipients as well as `To`.
    pub include_cc: bool,
    /// Addresses (`me@example.com`) and domains (`@example.com`) to leave out.
    pub exclude: Vec<String>,
    /// Drop no-reply / mailer-daemon style automated addresses.
    pub skip_automated: bool,
    /// Recency half-life in days; `0` disables decay (pure frequency).
    pub half_life_days: f64,
    /// Minimum message count for a contact to appear.
    pub min_messages: usize,
    /// Keep at most this many rows; `0` keeps every contact.
    pub limit: usize,
    pub sort: Sort,
    pub format: Format,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            count: Count::Both,
            include_cc: true,
            exclude: Vec::new(),
            skip_automated: true,
            half_life_days: 180.0,
            min_messages: 1,
            limit: 25,
            sort: Sort::Score,
            format: Format::Report,
        }
    }
}

/// One ranked person.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Contact {
    pub rank: usize,
    /// Most frequently used display name for this address (empty if never named).
    pub name: String,
    pub email: String,
    /// Messages this person appears in (a message counts once per person).
    pub messages: usize,
    /// Messages where they were a recipient — mail you sent them.
    pub to: usize,
    /// Messages where they were the sender — mail they sent you.
    pub from: usize,
    /// `YYYY-MM-DD` of the earliest dated message, empty when none were dated.
    pub first_seen: String,
    /// `YYYY-MM-DD` of the latest dated message, empty when none were dated.
    pub last_seen: String,
    /// Recency-weighted score, normalized so the top contact is `100.0`.
    pub score: f64,
}

/// The ranking plus the archive metadata the report header shows.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Ranking {
    /// Messages parsed out of the paste.
    pub messages: usize,
    /// Distinct contacts left after filtering.
    pub contacts_found: usize,
    /// Rows actually returned (after `limit`).
    pub contacts_returned: usize,
    /// `YYYY-MM-DD` of the oldest dated message, empty when none were dated.
    pub oldest: String,
    /// `YYYY-MM-DD` of the newest dated message — this is the recency clock.
    pub newest: String,
    pub half_life_days: f64,
    pub contacts: Vec<Contact>,
}

/// Split a raw mbox blob into individual message blocks.
///
/// mbox delimits messages with a `From ` postmark at column 0 (the space after
/// `From` is what distinguishes it from a `From:` header). The postmark line is
/// dropped. A blob with no postmark at all is treated as a single message so a
/// lone `.eml` paste still works.
pub fn split_messages(raw: &str) -> Vec<String> {
    let mut msgs: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut started = false;
    for line in raw.split_inclusive('\n') {
        if line.starts_with("From ") {
            if started && !cur.trim().is_empty() {
                msgs.push(std::mem::take(&mut cur));
            } else {
                cur.clear();
            }
            started = true;
            continue;
        }
        if started {
            cur.push_str(line);
        }
    }
    if started && !cur.trim().is_empty() {
        msgs.push(cur);
    }
    if msgs.is_empty() && !raw.trim().is_empty() {
        msgs.push(raw.to_string());
    }
    msgs
}

/// Local parts (and prefixes) that mark an address as machine-generated.
const AUTOMATED: [&str; 10] = [
    "noreply",
    "no-reply",
    "no_reply",
    "donotreply",
    "do-not-reply",
    "do_not_reply",
    "mailer-daemon",
    "postmaster",
    "bounce",
    "bounces",
];

/// True when the address looks like a machine sender rather than a person.
fn is_automated(email: &str) -> bool {
    let local = email.split('@').next().unwrap_or("");
    AUTOMATED
        .iter()
        .any(|p| local == *p || local.starts_with(&format!("{p}-")) || local.starts_with(&format!("{p}+")))
        || local.contains("noreply")
        || local.contains("no-reply")
}

/// Parse the `exclude` field: comma/semicolon/whitespace separated addresses and
/// `@domain` patterns, case-folded. A bare `example.com` is read as a domain too.
pub fn parse_exclude(s: &str) -> Vec<String> {
    s.split(|c: char| c == ',' || c == ';' || c.is_whitespace())
        .map(|t| t.trim().trim_matches(|c| c == '<' || c == '>').to_ascii_lowercase())
        .filter(|t| !t.is_empty())
        .map(|t| {
            if t.contains('@') || t.starts_with('@') {
                t
            } else {
                format!("@{t}")
            }
        })
        .collect()
}

/// True when `email` matches any exclude pattern (exact address or `@domain`).
fn is_excluded(email: &str, patterns: &[String]) -> bool {
    patterns.iter().any(|p| {
        if let Some(dom) = p.strip_prefix('@') {
            email.ends_with(&format!("@{dom}"))
        } else {
            email == p
        }
    })
}

/// Per-address accumulator, kept alongside an insertion-ordered key list so the
/// output never depends on `HashMap` iteration order.
#[derive(Debug, Default)]
struct Acc {
    email: String,
    name_counts: HashMap<String, usize>,
    name_order: Vec<String>,
    messages: usize,
    to: usize,
    from: usize,
    /// One entry per message: its timestamp, or `None` when undated.
    stamps: Vec<Option<i64>>,
    raw_score: f64,
}

impl Acc {
    /// The display name used most often for this address (first-seen wins ties).
    fn best_name(&self) -> String {
        let mut best: Option<(&String, usize)> = None;
        for n in &self.name_order {
            let c = self.name_counts.get(n).copied().unwrap_or(0);
            if best.map(|(_, bc)| c > bc).unwrap_or(true) {
                best = Some((n, c));
            }
        }
        best.map(|(n, _)| n.clone()).unwrap_or_default()
    }
}

/// Pull `(name, address)` pairs out of one header's address list.
fn addresses(addr: Option<&Address>) -> Vec<(String, String)> {
    match addr {
        None => Vec::new(),
        Some(a) => a
            .iter()
            .filter_map(|m| {
                let email = m.address()?.trim().to_ascii_lowercase();
                if !email.contains('@') || email.starts_with('@') || email.ends_with('@') {
                    return None;
                }
                let name = m
                    .name()
                    .map(|n| n.split_whitespace().collect::<Vec<_>>().join(" "))
                    .unwrap_or_default()
                    .trim_matches('"')
                    .trim()
                    .to_string();
                Some((name, email))
            })
            .collect(),
    }
}

/// `YYYY-MM-DD` from an RFC 3339 stamp.
fn day_of(rfc3339: &str) -> String {
    rfc3339.chars().take(10).collect()
}

/// Parse + rank a mailbox paste. This is the whole engine; `render` only formats.
pub fn rank(raw: &str, opts: &Options) -> Result<Ranking, String> {
    if raw.trim().is_empty() {
        return Err(
            "input is empty — paste an mbox export (Gmail Takeout, Thunderbird, Apple Mail) or one or more raw email messages"
                .into(),
        );
    }
    if opts.half_life_days < 0.0 {
        return Err(format!(
            "half_life_days must be 0 or more (0 turns recency off), got {}",
            opts.half_life_days
        ));
    }

    let blocks = split_messages(raw);
    if blocks.len() > MAX_MESSAGES {
        return Err(format!(
            "archive holds {} messages, more than the {MAX_MESSAGES}-message limit — split it up first (for example with the mbox-splitter tool)",
            blocks.len()
        ));
    }

    let mut accs: Vec<Acc> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut parsed = 0usize;
    let mut oldest_ts: Option<i64> = None;
    let mut newest_ts: Option<i64> = None;
    let mut oldest_day = String::new();
    let mut newest_day = String::new();

    for block in &blocks {
        let msg = match MessageParser::default().parse(block.as_bytes()) {
            Some(m) => m,
            None => continue,
        };
        let stamp = msg.date().map(|d| (d.to_timestamp(), day_of(&d.to_rfc3339())));
        if let Some((ts, ref day)) = stamp {
            if oldest_ts.map(|o| ts < o).unwrap_or(true) {
                oldest_ts = Some(ts);
                oldest_day = day.clone();
            }
            if newest_ts.map(|n| ts > n).unwrap_or(true) {
                newest_ts = Some(ts);
                newest_day = day.clone();
            }
        }

        // Collect this message's people, de-duplicated per role so a person
        // repeated across To and Cc still only counts once.
        let mut people: Vec<(String, String, bool)> = Vec::new(); // (name, email, is_sender)
        if opts.count.wants_senders() {
            for (n, e) in addresses(msg.from()) {
                people.push((n, e, true));
            }
        }
        if opts.count.wants_recipients() {
            for (n, e) in addresses(msg.to()) {
                people.push((n, e, false));
            }
            if opts.include_cc {
                for (n, e) in addresses(msg.cc()) {
                    people.push((n, e, false));
                }
                for (n, e) in addresses(msg.bcc()) {
                    people.push((n, e, false));
                }
            }
        }

        let mut seen_role: HashSet<(String, bool)> = HashSet::new();
        let mut seen_msg: HashSet<String> = HashSet::new();
        let mut touched = false;
        for (name, email, is_sender) in people {
            if opts.skip_automated && is_automated(&email) {
                continue;
            }
            if is_excluded(&email, &opts.exclude) {
                continue;
            }
            if !seen_role.insert((email.clone(), is_sender)) {
                continue;
            }
            touched = true;
            let idx = match index.get(&email) {
                Some(i) => *i,
                None => {
                    accs.push(Acc { email: email.clone(), ..Acc::default() });
                    index.insert(email.clone(), accs.len() - 1);
                    accs.len() - 1
                }
            };
            let acc = &mut accs[idx];
            if !name.is_empty() {
                if !acc.name_counts.contains_key(&name) {
                    acc.name_order.push(name.clone());
                }
                *acc.name_counts.entry(name).or_insert(0) += 1;
            }
            if is_sender {
                acc.from += 1;
            } else {
                acc.to += 1;
            }
            if seen_msg.insert(email.clone()) {
                acc.messages += 1;
                acc.stamps.push(stamp.as_ref().map(|(ts, _)| *ts));
            }
        }
        if touched || msg.from().is_some() || msg.to().is_some() {
            parsed += 1;
        }
    }

    if parsed == 0 {
        return Err(
            "no parseable email messages found — expected mbox text with `From ` postmark lines, or raw messages with From:/To:/Date: headers"
                .into(),
        );
    }

    // Recency weighting, clocked off the newest message in the archive.
    let reference = newest_ts;
    let fallback = oldest_ts;
    for acc in accs.iter_mut() {
        let mut raw_score = 0.0f64;
        for stamp in &acc.stamps {
            let w = match (reference, opts.half_life_days) {
                (Some(_), hl) if hl <= 0.0 => 1.0,
                (Some(r), hl) => {
                    let ts = stamp.or(fallback).unwrap_or(r);
                    let age_days = ((r - ts) as f64 / DAY).max(0.0);
                    0.5f64.powf(age_days / hl)
                }
                (None, _) => 1.0,
            };
            raw_score += w;
        }
        acc.raw_score = raw_score;
    }

    let min_messages = opts.min_messages.max(1);
    let kept: Vec<&Acc> = accs.iter().filter(|a| a.messages >= min_messages).collect();
    if kept.is_empty() {
        return Err(format!(
            "no contacts left after filtering — {} address(es) were found but none reached min_messages={min_messages}; lower it, widen `count`, or clear `exclude`",
            accs.len()
        ));
    }

    let max_raw = kept.iter().fold(0.0f64, |m, a| m.max(a.raw_score));
    let mut contacts: Vec<Contact> = kept
        .iter()
        .map(|a| {
            let dated: Vec<i64> = a.stamps.iter().filter_map(|s| *s).collect();
            let first = dated.iter().copied().min();
            let last = dated.iter().copied().max();
            Contact {
                rank: 0,
                name: a.best_name(),
                email: a.email.clone(),
                messages: a.messages,
                to: a.to,
                from: a.from,
                first_seen: first.map(ts_day).unwrap_or_default(),
                last_seen: last.map(ts_day).unwrap_or_default(),
                score: if max_raw > 0.0 {
                    (a.raw_score / max_raw * 1000.0).round() / 10.0
                } else {
                    0.0
                },
            }
        })
        .collect();

    // Deterministic ordering: the chosen key first, then stable tie-breakers.
    match opts.sort {
        Sort::Score => contacts.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(b.messages.cmp(&a.messages))
                .then(a.email.cmp(&b.email))
        }),
        Sort::Messages => contacts.sort_by(|a, b| {
            b.messages
                .cmp(&a.messages)
                .then(b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal))
                .then(a.email.cmp(&b.email))
        }),
        Sort::Recent => contacts.sort_by(|a, b| {
            b.last_seen
                .cmp(&a.last_seen)
                .then(b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal))
                .then(a.email.cmp(&b.email))
        }),
        Sort::Name => contacts.sort_by(|a, b| {
            let ka = if a.name.is_empty() { &a.email } else { &a.name };
            let kb = if b.name.is_empty() { &b.email } else { &b.name };
            ka.to_lowercase().cmp(&kb.to_lowercase()).then(a.email.cmp(&b.email))
        }),
    }

    let contacts_found = contacts.len();
    if opts.limit > 0 && contacts.len() > opts.limit {
        contacts.truncate(opts.limit);
    }
    for (i, c) in contacts.iter_mut().enumerate() {
        c.rank = i + 1;
    }

    Ok(Ranking {
        messages: parsed,
        contacts_found,
        contacts_returned: contacts.len(),
        oldest: oldest_day,
        newest: newest_day,
        half_life_days: opts.half_life_days,
        contacts,
    })
}

/// `YYYY-MM-DD` for a unix timestamp (UTC), without pulling in a date crate.
fn ts_day(ts: i64) -> String {
    // Days since the 1970-01-01 epoch, floored so pre-epoch stamps still work.
    let days = (ts as f64 / DAY).floor() as i64;
    let mut year = 1970i64;
    let mut d = days;
    loop {
        let len = if is_leap(year) { 366 } else { 365 };
        if d >= len {
            d -= len;
            year += 1;
        } else if d < 0 {
            year -= 1;
            d += if is_leap(year) { 366 } else { 365 };
        } else {
            break;
        }
    }
    let months = [
        31,
        if is_leap(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1;
    for len in months {
        if d < len {
            break;
        }
        d -= len;
        month += 1;
    }
    format!("{year:04}-{month:02}-{:02}", d + 1)
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

/// `Name <addr>`, or the bare address when no display name was ever used.
fn contact_label(c: &Contact) -> String {
    if c.name.is_empty() {
        c.email.clone()
    } else {
        format!("{} <{}>", c.name, c.email)
    }
}

/// Escape one field for RFC 4180 CSV.
fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Render a table whose columns are padded to their widest cell. `right` marks
/// the columns that are right-aligned; the last column is never padded.
fn table(header: &[&str], rows: &[Vec<String>], right: &[bool]) -> String {
    let cols = header.len();
    let mut widths: Vec<usize> = header.iter().map(|h| h.chars().count()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(cell.chars().count());
        }
    }
    let mut out = String::new();
    let render = |cells: &[String], out: &mut String| {
        let mut line = String::new();
        for (i, cell) in cells.iter().enumerate() {
            let pad = widths[i].saturating_sub(cell.chars().count());
            if right[i] {
                line.push_str(&" ".repeat(pad));
                line.push_str(cell);
            } else {
                line.push_str(cell);
                if i + 1 < cols {
                    line.push_str(&" ".repeat(pad));
                }
            }
            if i + 1 < cols {
                line.push_str("  ");
            }
        }
        out.push_str(line.trim_end());
        out.push('\n');
    };
    render(&header.iter().map(|h| h.to_string()).collect::<Vec<_>>(), &mut out);
    for row in rows {
        render(row, &mut out);
    }
    out.trim_end().to_string()
}

/// Rank a mailbox paste and format the result.
pub fn render(raw: &str, opts: &Options) -> Result<String, String> {
    let r = rank(raw, opts)?;
    match opts.format {
        Format::Json => {
            serde_json::to_string_pretty(&r).map_err(|e| format!("failed to serialize JSON: {e}"))
        }
        Format::List => Ok(r
            .contacts
            .iter()
            .map(contact_label)
            .collect::<Vec<_>>()
            .join("\n")),
        Format::Csv => {
            let mut out = String::from("rank,name,email,messages,to,from,first_seen,last_seen,score\r\n");
            for c in &r.contacts {
                let cells = [
                    c.rank.to_string(),
                    csv_field(&c.name),
                    csv_field(&c.email),
                    c.messages.to_string(),
                    c.to.to_string(),
                    c.from.to_string(),
                    c.first_seen.clone(),
                    c.last_seen.clone(),
                    format!("{:.1}", c.score),
                ];
                out.push_str(&cells.join(","));
                out.push_str("\r\n");
            }
            Ok(out.trim_end_matches("\r\n").to_string())
        }
        Format::Report => {
            let mut summary = format!(
                "Top {} of {} contact{} · {} message{}",
                r.contacts_returned,
                r.contacts_found,
                if r.contacts_found == 1 { "" } else { "s" },
                r.messages,
                if r.messages == 1 { "" } else { "s" },
            );
            if !r.oldest.is_empty() {
                summary.push_str(&format!(" · {} → {}", r.oldest, r.newest));
            } else {
                summary.push_str(" · no dated messages");
            }
            if r.half_life_days <= 0.0 || r.newest.is_empty() {
                summary.push_str(" · ranked by message count (recency off)");
            } else {
                summary.push_str(&format!(
                    " · half-life {} days, clocked from {}",
                    trim_num(r.half_life_days),
                    r.newest
                ));
            }

            let mut header: Vec<&str> = vec!["#", "contact", "msgs"];
            let mut right: Vec<bool> = vec![true, false, true];
            let show_to = opts.count.wants_recipients();
            let show_from = opts.count.wants_senders();
            if show_to {
                header.push("to");
                right.push(true);
            }
            if show_from {
                header.push("from");
                right.push(true);
            }
            header.push("last seen");
            right.push(false);
            header.push("score");
            right.push(true);

            let rows: Vec<Vec<String>> = r
                .contacts
                .iter()
                .map(|c| {
                    let mut row = vec![
                        c.rank.to_string(),
                        contact_label(c),
                        c.messages.to_string(),
                    ];
                    if show_to {
                        row.push(c.to.to_string());
                    }
                    if show_from {
                        row.push(c.from.to_string());
                    }
                    row.push(if c.last_seen.is_empty() {
                        "—".to_string()
                    } else {
                        c.last_seen.clone()
                    });
                    row.push(format!("{:.1}", c.score));
                    row
                })
                .collect();

            Ok(format!("{summary}\n\n{}", table(&header, &rows, &right)))
        }
    }
}

/// `180` rather than `180.0`, but `7.5` stays `7.5`.
fn trim_num(v: f64) -> String {
    if (v - v.round()).abs() < f64::EPSILON {
        format!("{}", v.round() as i64)
    } else {
        format!("{v}")
    }
}

/// One-call entry shared by the chat/CLI block and the web wrapper.
#[allow(clippy::too_many_arguments)]
pub fn run(
    mbox: &str,
    count: &str,
    include_cc: bool,
    exclude: &str,
    skip_automated: bool,
    half_life_days: f64,
    min_messages: i64,
    limit: i64,
    sort: &str,
    format: &str,
) -> Result<String, String> {
    if min_messages < 1 {
        return Err(format!("min_messages must be 1 or more, got {min_messages}"));
    }
    if limit < 0 {
        return Err(format!("limit must be 0 or more (0 = every contact), got {limit}"));
    }
    let opts = Options {
        count: Count::parse(count)?,
        include_cc,
        exclude: parse_exclude(exclude),
        skip_automated,
        half_life_days,
        min_messages: min_messages as usize,
        limit: limit as usize,
        sort: Sort::parse(sort)?,
        format: Format::parse(format)?,
    };
    render(mbox, &opts)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Four-message archive: Bob is the busiest and most recent contact, Carol is
    // older, Dave appears once on Cc, and a no-reply robot writes once.
    const MBOX: &str = concat!(
        "From 1@x Mon Sep 03 10:00:00 +0000 2018\r\n",
        "From: Alice Example <alice@example.com>\r\n",
        "To: Bob <bob@example.org>, Carol <carol@example.net>\r\n",
        "Subject: Project kickoff\r\n",
        "Date: Mon, 3 Sep 2018 10:00:00 +0000\r\n",
        "\r\n",
        "Hi both.\r\n",
        "From 2@x Tue Sep 04 09:30:00 +0000 2018\r\n",
        "From: Bob <bob@example.org>\r\n",
        "To: alice@example.com\r\n",
        "Cc: Dave <dave@example.com>\r\n",
        "Subject: Re: Project kickoff\r\n",
        "Date: Tue, 4 Sep 2018 09:30:00 +0000\r\n",
        "\r\n",
        "Sounds good.\r\n",
        "From 3@x Wed Sep 05 08:00:00 +0000 2018\r\n",
        "From: Alice Example <alice@example.com>\r\n",
        "To: Bob <bob@example.org>\r\n",
        "Subject: Slides\r\n",
        "Date: Wed, 5 Sep 2018 08:00:00 +0000\r\n",
        "\r\n",
        "Attached.\r\n",
        "From 4@x Wed Sep 05 09:00:00 +0000 2018\r\n",
        "From: Notifications <no-reply@example.io>\r\n",
        "To: alice@example.com\r\n",
        "Subject: Your receipt\r\n",
        "Date: Wed, 5 Sep 2018 09:00:00 +0000\r\n",
        "\r\n",
        "Receipt.\r\n",
    );

    fn opts() -> Options {
        Options { exclude: parse_exclude("alice@example.com"), ..Options::default() }
    }

    #[test]
    fn splits_four_messages() {
        assert_eq!(split_messages(MBOX).len(), 4);
    }

    #[test]
    fn ranks_bob_first_with_frequency_and_recency() {
        let r = rank(MBOX, &opts()).unwrap();
        assert_eq!(r.messages, 4);
        assert_eq!(r.oldest, "2018-09-03");
        assert_eq!(r.newest, "2018-09-05");
        let bob = &r.contacts[0];
        assert_eq!(bob.rank, 1);
        assert_eq!(bob.email, "bob@example.org");
        assert_eq!(bob.name, "Bob");
        assert_eq!(bob.messages, 3);
        assert_eq!(bob.to, 2); // messages 1 and 3 were addressed to him
        assert_eq!(bob.from, 1); // message 2 came from him
        assert_eq!(bob.first_seen, "2018-09-03");
        assert_eq!(bob.last_seen, "2018-09-05");
        assert_eq!(bob.score, 100.0);
        // Carol appears once and older than Dave, so recency puts Dave ahead.
        let order: Vec<&str> = r.contacts.iter().map(|c| c.email.as_str()).collect();
        assert_eq!(order, vec!["bob@example.org", "dave@example.com", "carol@example.net"]);
    }

    #[test]
    fn automated_sender_is_skipped_by_default_and_kept_when_asked() {
        let r = rank(MBOX, &opts()).unwrap();
        assert!(!r.contacts.iter().any(|c| c.email == "no-reply@example.io"));
        let keep = Options { skip_automated: false, ..opts() };
        let r2 = rank(MBOX, &keep).unwrap();
        assert!(r2.contacts.iter().any(|c| c.email == "no-reply@example.io"));
    }

    #[test]
    fn cc_recipients_can_be_dropped() {
        let no_cc = Options { include_cc: false, ..opts() };
        let r = rank(MBOX, &no_cc).unwrap();
        assert!(!r.contacts.iter().any(|c| c.email == "dave@example.com"));
    }

    #[test]
    fn count_senders_only_ignores_recipients() {
        let senders = Options { count: Count::Senders, ..opts() };
        let r = rank(MBOX, &senders).unwrap();
        let order: Vec<&str> = r.contacts.iter().map(|c| c.email.as_str()).collect();
        assert_eq!(order, vec!["bob@example.org"]); // alice excluded, no-reply skipped
        assert_eq!(r.contacts[0].to, 0);
        assert_eq!(r.contacts[0].from, 1);
    }

    #[test]
    fn exclude_accepts_a_domain_pattern() {
        let o = Options { exclude: parse_exclude("@example.com"), ..Options::default() };
        let r = rank(MBOX, &o).unwrap();
        assert!(!r.contacts.iter().any(|c| c.email.ends_with("@example.com")));
        // A bare domain (no @) is read as a domain too.
        assert_eq!(parse_exclude("example.com, me@x.org"), vec!["@example.com", "me@x.org"]);
    }

    #[test]
    fn half_life_zero_ranks_by_raw_frequency() {
        let o = Options { half_life_days: 0.0, sort: Sort::Score, ..opts() };
        let r = rank(MBOX, &o).unwrap();
        // Carol and Dave both have one message, so with decay off they tie.
        let carol = r.contacts.iter().find(|c| c.email == "carol@example.net").unwrap();
        let dave = r.contacts.iter().find(|c| c.email == "dave@example.com").unwrap();
        assert_eq!(carol.score, dave.score);
        // A short half-life instead makes the older contact score strictly lower.
        let fast = Options { half_life_days: 1.0, ..opts() };
        let r2 = rank(MBOX, &fast).unwrap();
        let carol2 = r2.contacts.iter().find(|c| c.email == "carol@example.net").unwrap();
        let dave2 = r2.contacts.iter().find(|c| c.email == "dave@example.com").unwrap();
        assert!(carol2.score < dave2.score);
    }

    #[test]
    fn min_messages_and_limit_trim_the_list() {
        let o = Options { min_messages: 2, ..opts() };
        let r = rank(MBOX, &o).unwrap();
        assert_eq!(r.contacts_found, 1);
        assert_eq!(r.contacts[0].email, "bob@example.org");

        let l = Options { limit: 2, ..opts() };
        let r2 = rank(MBOX, &l).unwrap();
        assert_eq!(r2.contacts_found, 3);
        assert_eq!(r2.contacts_returned, 2);
    }

    #[test]
    fn sort_modes_reorder_deterministically() {
        let by_name = Options { sort: Sort::Name, limit: 0, ..opts() };
        let names: Vec<String> = rank(MBOX, &by_name)
            .unwrap()
            .contacts
            .iter()
            .map(|c| c.name.clone())
            .collect();
        assert_eq!(names, vec!["Bob", "Carol", "Dave"]);

        let by_recent = Options { sort: Sort::Recent, limit: 0, ..opts() };
        let recent: Vec<String> = rank(MBOX, &by_recent)
            .unwrap()
            .contacts
            .iter()
            .map(|c| c.last_seen.clone())
            .collect();
        assert_eq!(recent, vec!["2018-09-05", "2018-09-04", "2018-09-03"]);
    }

    #[test]
    fn report_is_an_aligned_table_with_a_summary_line() {
        let out = render(MBOX, &opts()).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(
            lines[0],
            "Top 3 of 3 contacts · 4 messages · 2018-09-03 → 2018-09-05 · half-life 180 days, clocked from 2018-09-05"
        );
        assert_eq!(lines[1], "");
        assert_eq!(lines[2], "#  contact                    msgs  to  from  last seen   score");
        assert_eq!(lines[3], "1  Bob <bob@example.org>         3   2     1  2018-09-05  100.0");
        assert_eq!(lines.len(), 6);
    }

    #[test]
    fn list_format_is_paste_ready() {
        let o = Options { format: Format::List, ..opts() };
        assert_eq!(
            render(MBOX, &o).unwrap(),
            "Bob <bob@example.org>\nDave <dave@example.com>\nCarol <carol@example.net>"
        );
    }

    #[test]
    fn csv_has_a_header_and_quotes_commas() {
        let o = Options { format: Format::Csv, ..opts() };
        let out = render(MBOX, &o).unwrap();
        let lines: Vec<&str> = out.split("\r\n").collect();
        assert_eq!(lines[0], "rank,name,email,messages,to,from,first_seen,last_seen,score");
        assert_eq!(lines[1], "1,Bob,bob@example.org,3,2,1,2018-09-03,2018-09-05,100.0");
        // A display name containing a comma must be quoted.
        let comma = "From: \"Doe, Jane\" <jane@example.org>\r\nTo: x@y.com\r\nDate: Mon, 3 Sep 2018 10:00:00 +0000\r\n\r\nhi\r\n";
        let out2 = render(comma, &o).unwrap();
        assert!(out2.contains("\"Doe, Jane\""));
    }

    #[test]
    fn json_carries_the_archive_metadata() {
        let o = Options { format: Format::Json, ..opts() };
        let v: serde_json::Value = serde_json::from_str(&render(MBOX, &o).unwrap()).unwrap();
        assert_eq!(v["messages"], 4);
        assert_eq!(v["contacts_found"], 3);
        assert_eq!(v["newest"], "2018-09-05");
        assert_eq!(v["half_life_days"], 180.0);
        assert_eq!(v["contacts"][0]["email"], "bob@example.org");
        assert_eq!(v["contacts"][0]["score"], 100.0);
    }

    #[test]
    fn single_message_without_a_postmark_still_ranks() {
        let eml = "From: Solo <solo@example.com>\r\nTo: me@example.org\r\nSubject: Hi\r\n\r\nbody\r\n";
        let r = rank(eml, &Options::default()).unwrap();
        assert_eq!(r.messages, 1);
        assert_eq!(r.contacts.len(), 2);
        // No Date header anywhere → recency is off and both score the same.
        assert_eq!(r.newest, "");
        assert_eq!(r.contacts[0].score, 100.0);
        assert_eq!(r.contacts[1].score, 100.0);
        assert!(render(eml, &Options::default()).unwrap().contains("recency off"));
    }

    #[test]
    fn empty_input_errors() {
        assert!(rank("", &Options::default()).is_err());
        assert!(rank("   \n\t ", &Options::default()).is_err());
    }

    #[test]
    fn unparseable_input_errors_with_guidance() {
        let err = rank("just some prose, no headers at all", &Options::default()).unwrap_err();
        assert!(err.contains("no parseable email messages"), "got: {err}");
    }

    #[test]
    fn filtering_everything_out_errors() {
        let o = Options { min_messages: 99, ..opts() };
        let err = rank(MBOX, &o).unwrap_err();
        assert!(err.contains("min_messages=99"), "got: {err}");
    }

    #[test]
    fn over_the_message_cap_errors() {
        let one = "From x@y Mon Sep 03 10:00:00 +0000 2018\r\nFrom: a@b.com\r\nTo: c@d.com\r\n\r\nhi\r\n";
        let big = one.repeat(MAX_MESSAGES + 1);
        let err = rank(&big, &Options::default()).unwrap_err();
        assert!(err.contains("5000-message limit"), "got: {err}");
        // Exactly at the cap is fine.
        let at_cap = one.repeat(MAX_MESSAGES);
        assert!(rank(&at_cap, &Options::default()).is_ok());
    }

    #[test]
    fn enum_parsers_reject_garbage() {
        assert!(Count::parse("nobody").is_err());
        assert!(Sort::parse("random").is_err());
        assert!(Format::parse("xml").is_err());
        assert_eq!(Count::parse("").unwrap(), Count::Both);
        assert_eq!(Sort::parse("SCORE").unwrap(), Sort::Score);
        assert_eq!(Format::parse("JSON").unwrap(), Format::Json);
    }

    #[test]
    fn run_entry_validates_numbers_and_defaults() {
        assert!(run(MBOX, "", true, "", true, -1.0, 1, 25, "", "").is_err());
        assert!(run(MBOX, "", true, "", true, 180.0, 0, 25, "", "").is_err());
        assert!(run(MBOX, "", true, "", true, 180.0, 1, -1, "", "").is_err());
        let out = run(MBOX, "", true, "alice@example.com", true, 180.0, 1, 25, "", "list").unwrap();
        assert_eq!(out.lines().next().unwrap(), "Bob <bob@example.org>");
    }

    #[test]
    fn ts_day_converts_known_dates() {
        assert_eq!(ts_day(0), "1970-01-01");
        assert_eq!(ts_day(1_535_968_800), "2018-09-03"); // 2018-09-03T10:00:00Z
        assert_eq!(ts_day(1_583_020_800), "2020-03-01"); // leap-year boundary
    }
}

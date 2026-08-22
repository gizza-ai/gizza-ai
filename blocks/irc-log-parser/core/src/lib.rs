//! gizza-ai/irc-log-parser core — turn a raw IRC client log into structured,
//! typed records and render them as a readable timeline, JSON, NDJSON, CSV or a
//! Markdown table. Pure Rust, no regex, no clock, no network: the output is a
//! deterministic function of the pasted text and the options.
//!
//! Two independent layers:
//!
//! 1. **Timestamp grammar** (`Dialect`) — how the client prefixes each line:
//!    `weechat` `2024-01-05 21:07:33<TAB>nick<TAB>text`, `irssi` `21:07 …`,
//!    `bracket` `[21:07:33] …` (mIRC / ZNC / EnergyMech), `hexchat`
//!    `Jan 05 21:07:33 …`, `iso` `2024-01-05 21:07:33 …`, or `plain` (none).
//!    `Dialect::Auto` scores every grammar over the first lines and picks the
//!    best.
//! 2. **Body grammar** — the event wording, which is read the same way in every
//!    dialect because clients borrow each other's phrasing: `-!- nick [u@h] has
//!    joined #chan` (irssi), `*** Joins: nick (u@h)` (EnergyMech/ZNC),
//!    `* Parts: nick (u@h) (bye)` (mIRC), `nick (u@h) has quit (…)` (WeeChat /
//!    HexChat), plus mode, topic, kick and nick-change phrasings.
//!
//! Every record carries the same eight fields, so the JSON/NDJSON/CSV shapes are
//! stable regardless of which client wrote the log:
//! `line`, `time`, `type`, `nick`, `host`, `channel`, `arg`, `text`
//! (`arg` = the second actor or payload: the new nick, the kicker, or the mode
//! string). `raw` is appended when `include_raw` is on.
//!
//! Deliberately NOT here: statistics (blocks/chat-log-analyzer), multi-file
//! merging (blocks/log-merger), and `nick: message` transcripts
//! (blocks/chat-transcript-formatter) — a bare `alice: hi` line is not IRC
//! syntax and stays an `unknown` record rather than being guessed at.

use serde_json::{Map, Value};
use std::fmt::Write as _;

/// Longest log accepted, in bytes (~5 MB, a few hundred thousand lines).
pub const MAX_INPUT_BYTES: usize = 5_000_000;
/// Largest accepted `limit` (0 means "no limit" and is always allowed).
pub const MAX_LIMIT: i64 = 200_000;
/// How many non-blank lines auto-detection scores before deciding.
const DETECT_SAMPLE: usize = 200;
/// Longest run of characters accepted between `<`/`>` (or `-`/`-`) as a nick.
const MAX_NICK_LEN: usize = 64;

// ---------------------------------------------------------------- options ---

/// The timestamp grammar the log was written with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    Auto,
    /// `2024-01-05 21:07:33<TAB>nick<TAB>message`
    Weechat,
    /// `21:07 <nick> message` / `21:07:33 -!- …`
    Irssi,
    /// `[21:07:33] <nick> message` (mIRC, ZNC, EnergyMech, Textual)
    Bracket,
    /// `Jan 05 21:07:33 <nick> message` (HexChat, XChat)
    Hexchat,
    /// `2024-01-05 21:07:33 <nick> message`
    Iso,
    /// No timestamps at all.
    Plain,
}

impl Dialect {
    pub fn parse(s: &str) -> Result<Dialect, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Dialect::Auto,
            "weechat" => Dialect::Weechat,
            "irssi" => Dialect::Irssi,
            "bracket" | "mirc" | "znc" | "energymech" => Dialect::Bracket,
            "hexchat" | "xchat" => Dialect::Hexchat,
            "iso" => Dialect::Iso,
            "plain" | "none" => Dialect::Plain,
            other => {
                return Err(format!(
                    "unknown format '{other}' (use auto, weechat, irssi, bracket, hexchat, iso, or plain)"
                ))
            }
        })
    }

    pub fn label(self) -> &'static str {
        match self {
            Dialect::Auto => "auto",
            Dialect::Weechat => "weechat",
            Dialect::Irssi => "irssi",
            Dialect::Bracket => "bracket",
            Dialect::Hexchat => "hexchat",
            Dialect::Iso => "iso",
            Dialect::Plain => "plain",
        }
    }
}

/// How the parsed records are rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    Timeline,
    Json,
    Ndjson,
    Csv,
    Markdown,
}

impl Output {
    pub fn parse(s: &str) -> Result<Output, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "timeline" => Output::Timeline,
            "json" => Output::Json,
            "ndjson" | "jsonl" => Output::Ndjson,
            "csv" => Output::Csv,
            "markdown" | "md" => Output::Markdown,
            other => {
                return Err(format!(
                    "unknown output '{other}' (use timeline, json, ndjson, csv, or markdown)"
                ))
            }
        })
    }
}

/// How each record's timestamp is written out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeFormat {
    Iso,
    H24,
    H12,
    Original,
    None,
}

impl TimeFormat {
    pub fn parse(s: &str) -> Result<TimeFormat, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "iso" => TimeFormat::Iso,
            "24h" => TimeFormat::H24,
            "12h" => TimeFormat::H12,
            "original" | "keep" => TimeFormat::Original,
            "none" => TimeFormat::None,
            other => {
                return Err(format!(
                    "unknown time_format '{other}' (use iso, 24h, 12h, original, or none)"
                ))
            }
        })
    }
}

/// Which kinds of line survive into the output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Include {
    All,
    Messages,
    Events,
}

impl Include {
    pub fn parse(s: &str) -> Result<Include, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "all" => Include::All,
            "messages" => Include::Messages,
            "events" => Include::Events,
            other => {
                return Err(format!(
                    "unknown include '{other}' (use all, messages, or events)"
                ))
            }
        })
    }

    fn keeps(self, k: Kind) -> bool {
        match self {
            Include::All => true,
            Include::Messages => matches!(k, Kind::Message | Kind::Action | Kind::Notice),
            Include::Events => matches!(
                k,
                Kind::Join | Kind::Part | Kind::Quit | Kind::Kick | Kind::Nick | Kind::Mode | Kind::Topic
            ),
        }
    }
}

/// The type of a parsed line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Message,
    Action,
    Notice,
    Join,
    Part,
    Quit,
    Kick,
    Nick,
    Mode,
    Topic,
    /// Log open/close markers, day changes, server banners.
    Meta,
    /// Recognised as a log line but not as any known IRC shape.
    Unknown,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Message => "message",
            Kind::Action => "action",
            Kind::Notice => "notice",
            Kind::Join => "join",
            Kind::Part => "part",
            Kind::Quit => "quit",
            Kind::Kick => "kick",
            Kind::Nick => "nick",
            Kind::Mode => "mode",
            Kind::Topic => "topic",
            Kind::Meta => "meta",
            Kind::Unknown => "unknown",
        }
    }
}

// ----------------------------------------------------------------- record ---

/// One parsed log line.
#[derive(Debug, Clone, Default)]
pub struct Record {
    pub line: usize,
    pub date: Option<(i32, u32, u32)>,
    pub time: Option<(u32, u32, u32)>,
    /// The timestamp exactly as it appeared in the source line.
    pub raw_time: String,
    pub kind: Kind,
    pub nick: String,
    pub host: String,
    pub channel: String,
    pub arg: String,
    pub text: String,
    pub raw: String,
}

impl Default for Kind {
    fn default() -> Self {
        Kind::Unknown
    }
}

// ---------------------------------------------------------- small helpers ---

/// Strip mIRC formatting codes (bold/italic/underline/reverse/reset/monospace,
/// `^C` colour pairs) and ANSI CSI escape sequences. IRC colour codes are
/// `\x03` followed by up to two digits, optionally `,` plus up to two more.
fn strip_codes(s: &str) -> String {
    let b: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            '\u{02}' | '\u{1D}' | '\u{1F}' | '\u{16}' | '\u{0F}' | '\u{11}' | '\u{1E}' => i += 1,
            '\u{03}' => {
                i += 1;
                let mut digits = 0;
                while i < b.len() && digits < 2 && b[i].is_ascii_digit() {
                    i += 1;
                    digits += 1;
                }
                if digits > 0 && i < b.len() && b[i] == ',' {
                    let save = i;
                    i += 1;
                    let mut d2 = 0;
                    while i < b.len() && d2 < 2 && b[i].is_ascii_digit() {
                        i += 1;
                        d2 += 1;
                    }
                    // A comma not followed by digits belongs to the message.
                    if d2 == 0 {
                        i = save;
                    }
                }
            }
            '\u{04}' => {
                // `^D` hex colour: up to 6 hex digits, optionally a second pair.
                i += 1;
                let mut hexd = 0;
                while i < b.len() && hexd < 6 && b[i].is_ascii_hexdigit() {
                    i += 1;
                    hexd += 1;
                }
            }
            '\u{1B}' => {
                i += 1;
                if i < b.len() && b[i] == '[' {
                    i += 1;
                    while i < b.len() && !b[i].is_ascii_alphabetic() {
                        i += 1;
                    }
                    if i < b.len() {
                        i += 1;
                    }
                }
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

/// Case-insensitive prefix match; returns the remainder with leading blanks gone.
fn ci_prefix<'a>(s: &'a str, p: &str) -> Option<&'a str> {
    if s.len() >= p.len() && s[..p.len()].eq_ignore_ascii_case(p) {
        Some(s[p.len()..].trim_start())
    } else {
        None
    }
}

fn first_token(s: &str) -> (&str, &str) {
    let t = s.trim_start();
    match t.find(char::is_whitespace) {
        Some(i) => (&t[..i], t[i..].trim_start()),
        None => (t, ""),
    }
}

/// `+` is deliberately NOT a channel prefix here: modeless `+chan` channels are
/// extinct, while `+m` is an extremely common mode string, and treating the
/// latter as a channel breaks every `alice sets mode: +m` line.
fn is_channel(tok: &str) -> bool {
    let t = tok.trim_end_matches([',', ':', '.']);
    t.len() > 1 && matches!(t.as_bytes()[0], b'#' | b'&' | b'!')
}

/// IRC nick prefixes (`@op`, `+voice`, …) plus WeeChat's away marker.
fn strip_nick_prefix(nick: &str) -> &str {
    nick.trim_start_matches(['@', '+', '%', '~', '&', ' '])
}

/// Everything inside the FIRST `(...)` or `[...]` group, if any.
fn bracket_inner(s: &str) -> Option<(&str, usize)> {
    let bytes = s.as_bytes();
    for (i, c) in bytes.iter().enumerate() {
        let close = match c {
            b'(' => b')',
            b'[' => b']',
            _ => continue,
        };
        if let Some(j) = s[i + 1..].find(close as char) {
            return Some((&s[i + 1..i + 1 + j], i + 1 + j + 1));
        }
    }
    None
}

/// The trailing `(reason)` / `[reason]` / `: reason` of a part/quit/kick line.
fn trailing_reason(s: &str) -> String {
    let t = s.trim();
    if t.is_empty() {
        return String::new();
    }
    let bytes = t.as_bytes();
    let last = bytes[bytes.len() - 1];
    let open = match last {
        b')' => b'(',
        b']' => b'[',
        _ => {
            return match t.strip_prefix(':') {
                Some(r) => r.trim().to_string(),
                None => t.to_string(),
            }
        }
    };
    match t.rfind(open as char) {
        Some(i) => t[i + 1..t.len() - 1].trim().to_string(),
        None => t.to_string(),
    }
}

fn month_from_name(tok: &str) -> Option<u32> {
    let t = tok.trim_matches(|c: char| !c.is_ascii_alphabetic());
    if t.len() < 3 {
        return None;
    }
    Some(match t[..3].to_ascii_lowercase().as_str() {
        "jan" => 1,
        "feb" => 2,
        "mar" => 3,
        "apr" => 4,
        "may" => 5,
        "jun" => 6,
        "jul" => 7,
        "aug" => 8,
        "sep" => 9,
        "oct" => 10,
        "nov" => 11,
        "dec" => 12,
        _ => return None,
    })
}

fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        _ => 0,
    }
}

/// `YYYY-MM-DD` (also accepts `/` and `.` separators).
fn parse_iso_date(tok: &str) -> Option<(i32, u32, u32)> {
    let t = tok.trim();
    if t.len() != 10 {
        return None;
    }
    let sep = t.as_bytes()[4] as char;
    if !matches!(sep, '-' | '/' | '.') || t.as_bytes()[7] as char != sep {
        return None;
    }
    let y: i32 = t[..4].parse().ok()?;
    let m: u32 = t[5..7].parse().ok()?;
    let d: u32 = t[8..10].parse().ok()?;
    if m == 0 || m > 12 || d == 0 || d > days_in_month(y, m) {
        return None;
    }
    Some((y, m, d))
}

/// `HH:MM` or `HH:MM:SS`.
fn parse_clock(tok: &str) -> Option<(u32, u32, u32)> {
    let t = tok.trim();
    let mut it = t.split(':');
    let h: u32 = it.next()?.parse().ok()?;
    let mi: u32 = it.next()?.parse().ok()?;
    let s: u32 = match it.next() {
        Some(v) => v.parse().ok()?,
        None => 0,
    };
    if it.next().is_some() || h > 23 || mi > 59 || s > 60 {
        return None;
    }
    Some((h, mi, s.min(59)))
}

/// Pull a date out of free text such as `Log opened Fri Jan 05 20:00:00 2024`,
/// `Day changed Sat Jan 06 2024` or `Day changed to 06 Jan 2024`.
fn sniff_date(s: &str) -> Option<(i32, u32, u32)> {
    for tok in s.split_whitespace() {
        if let Some(d) = parse_iso_date(tok.trim_matches(|c: char| c == ',' || c == '.')) {
            return Some(d);
        }
    }
    let (mut year, mut month, mut day) = (None, None, None);
    for tok in s.split_whitespace() {
        let tok = tok.trim_matches(|c: char| c == ',');
        if tok.len() == 4 {
            if let Ok(y) = tok.parse::<i32>() {
                if (1900..=2999).contains(&y) {
                    year.get_or_insert(y);
                    continue;
                }
            }
        }
        if tok.len() >= 3 && tok.chars().all(|c| c.is_ascii_alphabetic()) {
            if let Some(m) = month_from_name(tok) {
                month.get_or_insert(m);
                continue;
            }
        }
        if tok.len() <= 2 {
            if let Ok(d) = tok.parse::<u32>() {
                if (1..=31).contains(&d) {
                    day.get_or_insert(d);
                }
            }
        }
    }
    match (year, month, day) {
        (Some(y), Some(m), Some(d)) if d <= days_in_month(y, m) => Some((y, m, d)),
        _ => None,
    }
}

// -------------------------------------------------------- timestamp layer ---

struct Stamp<'a> {
    date: Option<(i32, u32, u32)>,
    time: Option<(u32, u32, u32)>,
    raw: &'a str,
    rest: &'a str,
}

/// Try one dialect's timestamp grammar against a line.
fn take_stamp<'a>(line: &'a str, d: Dialect) -> Option<Stamp<'a>> {
    match d {
        Dialect::Plain | Dialect::Auto => None,
        Dialect::Weechat => {
            let tab = line.find('\t')?;
            let head = &line[..tab];
            let (dtok, ttok) = head.split_once(' ')?;
            let date = parse_iso_date(dtok)?;
            let time = parse_clock(ttok)?;
            Some(Stamp {
                date: Some(date),
                time: Some(time),
                raw: head,
                rest: &line[tab + 1..],
            })
        }
        Dialect::Iso => {
            if line.len() < 16 {
                return None;
            }
            let date = parse_iso_date(&line[..10])?;
            let sep = line.as_bytes()[10];
            if sep != b' ' && sep != b'T' && sep != b'\t' {
                return None;
            }
            let after = &line[11..];
            let end = after
                .find(|c: char| c != ':' && !c.is_ascii_digit() && c != '.')
                .unwrap_or(after.len());
            let time = parse_clock(after[..end].split('.').next()?)?;
            Some(Stamp {
                date: Some(date),
                time: Some(time),
                raw: &line[..11 + end],
                rest: after[end..].trim_start(),
            })
        }
        Dialect::Bracket => {
            if !line.starts_with('[') {
                return None;
            }
            let close = line.find(']')?;
            let inner = line[1..close].trim();
            let (date, time) = if inner.len() > 8 {
                let (dtok, ttok) = inner.split_once(|c| c == ' ' || c == 'T')?;
                (Some(parse_iso_date(dtok)?), parse_clock(ttok)?)
            } else {
                (None, parse_clock(inner)?)
            };
            Some(Stamp {
                date,
                time: Some(time),
                raw: &line[..close + 1],
                rest: line[close + 1..].trim_start(),
            })
        }
        Dialect::Hexchat => {
            // `Jan 05 21:07:33 …` — the day may be space-padded (`Jan  5`).
            let month = month_from_name(line.get(..3)?)?;
            let after = line.get(3..)?.trim_start();
            let (dtok, rest) = first_token(after);
            let day: u32 = dtok.parse().ok()?;
            if day == 0 || day > 31 {
                return None;
            }
            let (ttok, rest2) = first_token(rest);
            let time = parse_clock(ttok)?;
            let consumed = line.len() - rest2.len();
            Some(Stamp {
                date: Some((0, month, day)),
                time: Some(time),
                raw: line[..consumed].trim_end(),
                rest: rest2,
            })
        }
        Dialect::Irssi => {
            let (ttok, rest) = first_token(line);
            if !ttok.contains(':') {
                return None;
            }
            let time = parse_clock(ttok)?;
            Some(Stamp {
                date: None,
                time: Some(time),
                raw: ttok,
                rest,
            })
        }
    }
}

const DETECT_ORDER: [Dialect; 5] = [
    Dialect::Weechat,
    Dialect::Iso,
    Dialect::Bracket,
    Dialect::Hexchat,
    Dialect::Irssi,
];

fn detect_dialect(lines: &[&str]) -> Dialect {
    let mut best = Dialect::Plain;
    let mut best_score = 0usize;
    for cand in DETECT_ORDER {
        let mut score = 0;
        let mut seen = 0;
        for raw in lines.iter() {
            let l = raw.trim_end();
            if l.trim().is_empty() {
                continue;
            }
            seen += 1;
            if take_stamp(l, cand).is_some() {
                score += 1;
            }
            if seen >= DETECT_SAMPLE {
                break;
            }
        }
        if score > best_score {
            best_score = score;
            best = cand;
        }
    }
    best
}

// ------------------------------------------------------------- body layer ---

#[derive(Default)]
struct Ev {
    kind: Kind,
    nick: String,
    host: String,
    channel: String,
    arg: String,
    text: String,
}

fn ev(kind: Kind) -> Ev {
    Ev {
        kind,
        ..Default::default()
    }
}

/// Split `alice [~a@host]` / `alice (~a@host)` / `alice` into nick + host.
fn actor(head: &str) -> (String, String) {
    let (nick, rest) = first_token(head.trim());
    let host = bracket_inner(rest)
        .map(|(h, _)| h.trim().to_string())
        .filter(|h| h.contains('@'))
        .unwrap_or_default();
    (strip_nick_prefix(nick).to_string(), host)
}

/// Every `(...)` group in order, used by the `Joins:`/`Parts:` phrasings.
fn paren_groups(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'(' {
            if let Some(j) = s[i + 1..].find(')') {
                out.push(&s[i + 1..i + 1 + j]);
                i = i + 1 + j + 1;
                continue;
            }
        }
        i += 1;
    }
    out
}

fn channel_in(s: &str) -> String {
    s.split_whitespace()
        .find(|t| is_channel(t))
        .map(|t| t.trim_end_matches([',', ':', '.']).to_string())
        .unwrap_or_default()
}

/// Read one IRC event phrasing. Returns `None` when the text is not an event.
fn parse_event(s: &str) -> Option<Ev> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    let low = t.to_ascii_lowercase(); // ASCII-only mapping keeps byte offsets

    // --- EnergyMech / ZNC / mIRC: `Joins: nick (u@h)` -----------------------
    for (kw, kind) in [
        ("joins:", Kind::Join),
        ("parts:", Kind::Part),
        ("quits:", Kind::Quit),
    ] {
        if let Some(rest) = ci_prefix(t, kw) {
            let (nick, after) = first_token(rest);
            let groups = paren_groups(after);
            let mut e = ev(kind);
            e.nick = strip_nick_prefix(nick).to_string();
            let mut idx = 0;
            if let Some(g) = groups.first() {
                if g.contains('@') {
                    e.host = g.trim().to_string();
                    idx = 1;
                }
            }
            if kind != Kind::Join {
                e.text = groups.get(idx).map(|g| g.trim().to_string()).unwrap_or_default();
            }
            e.channel = channel_in(after);
            return Some(e);
        }
    }

    // --- join ---------------------------------------------------------------
    if let Some(p) = low.find(" has joined") {
        let (nick, host) = actor(&t[..p]);
        let tail = &t[p + " has joined".len()..];
        let mut e = ev(Kind::Join);
        e.nick = nick;
        e.host = host;
        e.channel = channel_in(tail);
        return Some(e);
    }

    // --- part ---------------------------------------------------------------
    for kw in [" has left", " has parted"] {
        if let Some(p) = low.find(kw) {
            let (nick, host) = actor(&t[..p]);
            let tail = t[p + kw.len()..].trim();
            let mut e = ev(Kind::Part);
            e.nick = nick;
            e.host = host;
            e.channel = channel_in(tail);
            let after_chan = match tail.split_once(' ') {
                Some((first, r)) if is_channel(first) => r,
                _ if is_channel(tail) => "",
                _ => tail,
            };
            e.text = trailing_reason(after_chan);
            return Some(e);
        }
    }

    // --- quit ---------------------------------------------------------------
    if let Some(p) = low.find(" has quit") {
        let (nick, host) = actor(&t[..p]);
        let mut e = ev(Kind::Quit);
        e.nick = nick;
        e.host = host;
        e.text = trailing_reason(&t[p + " has quit".len()..]);
        return Some(e);
    }

    // --- kick ---------------------------------------------------------------
    if let Some(p) = low.find(" was kicked") {
        let (nick, _) = actor(&t[..p]);
        let tail = t[p + " was kicked".len()..].trim();
        let mut e = ev(Kind::Kick);
        e.nick = nick;
        e.channel = channel_in(tail);
        let mut toks = tail.split_whitespace();
        while let Some(tok) = toks.next() {
            if tok.eq_ignore_ascii_case("by") {
                if let Some(by) = toks.next() {
                    e.arg = strip_nick_prefix(by).to_string();
                }
                break;
            }
        }
        e.text = trailing_reason(tail);
        if e.text.eq_ignore_ascii_case(tail) {
            e.text = String::new();
        }
        return Some(e);
    }

    // --- nick change --------------------------------------------------------
    if let Some(p) = low.find(" is now known as ") {
        let (nick, _) = actor(&t[..p]);
        let (new, _) = first_token(&t[p + " is now known as ".len()..]);
        let mut e = ev(Kind::Nick);
        e.nick = nick;
        e.arg = strip_nick_prefix(new.trim_end_matches(['.', ','])).to_string();
        return Some(e);
    }
    if let Some(rest) = ci_prefix(t, "nick change:") {
        if let Some((a, b)) = rest.split_once("->") {
            let mut e = ev(Kind::Nick);
            e.nick = strip_nick_prefix(a.trim()).to_string();
            e.arg = strip_nick_prefix(b.trim()).to_string();
            return Some(e);
        }
    }

    // --- mode ---------------------------------------------------------------
    for kw in ["mode/", "servermode/"] {
        if let Some(rest) = ci_prefix(t, kw) {
            let (chan, after) = first_token(rest);
            let mut e = ev(Kind::Mode);
            e.channel = chan.to_string();
            e.arg = bracket_inner(after)
                .map(|(a, _)| a.trim().to_string())
                .unwrap_or_else(|| after.trim().to_string());
            if let Some(p) = after.to_ascii_lowercase().find(" by ") {
                e.nick = strip_nick_prefix(first_token(&after[p + 4..]).0).to_string();
            }
            return Some(e);
        }
    }
    if let Some(rest) = ci_prefix(t, "mode ") {
        let (chan, after) = first_token(rest);
        if is_channel(chan) {
            let mut e = ev(Kind::Mode);
            e.channel = chan.to_string();
            e.arg = bracket_inner(after)
                .map(|(a, _)| a.trim().to_string())
                .unwrap_or_else(|| after.trim().to_string());
            if let Some(p) = after.to_ascii_lowercase().find(" by ") {
                e.nick = strip_nick_prefix(first_token(&after[p + 4..]).0).to_string();
            }
            return Some(e);
        }
    }
    if let Some(p) = low.find(" sets mode") {
        let (nick, _) = actor(&t[..p]);
        let rest = t[p + " sets mode".len()..].trim_start_matches([':', ' ']);
        let mut e = ev(Kind::Mode);
        e.nick = nick;
        e.channel = channel_in(rest);
        let payload = match rest.split_once(' ') {
            Some((first, r)) if is_channel(first) => r,
            _ => rest,
        };
        e.arg = payload.trim().to_string();
        return Some(e);
    }

    // --- topic --------------------------------------------------------------
    if let Some(p) = low.find("changed the topic of ") {
        let (nick, _) = actor(&t[..p]);
        let rest = &t[p + "changed the topic of ".len()..];
        let (chan, after) = first_token(rest);
        let mut e = ev(Kind::Topic);
        e.nick = nick;
        e.channel = chan.to_string();
        e.text = strip_quotes(after.trim_start_matches("to").trim_start_matches([':', ' ']));
        return Some(e);
    }
    if let Some(p) = low.find("has changed topic for ") {
        let (nick, _) = actor(&t[..p]);
        let rest = &t[p + "has changed topic for ".len()..];
        let (chan, after) = first_token(rest);
        let mut e = ev(Kind::Topic);
        e.nick = nick;
        e.channel = chan.to_string();
        let tail = match after.to_ascii_lowercase().rfind(" to ") {
            Some(i) => &after[i + 4..],
            None => after,
        };
        e.text = strip_quotes(tail);
        return Some(e);
    }
    for kw in ["has changed the topic to", "changes topic to", "sets topic to"] {
        if let Some(p) = low.find(kw) {
            let (nick, _) = actor(&t[..p]);
            let mut e = ev(Kind::Topic);
            e.nick = nick;
            e.text = strip_quotes(t[p + kw.len()..].trim_start_matches([':', ' ']));
            return Some(e);
        }
    }
    if let Some(rest) = ci_prefix(t, "topic for ") {
        let (chan, after) = first_token(rest);
        let mut e = ev(Kind::Topic);
        e.channel = chan.trim_end_matches(':').to_string();
        let body = match after.to_ascii_lowercase().strip_prefix("is") {
            Some(_) => after["is".len()..].trim_start_matches([':', ' ']),
            None => after.trim_start_matches([':', ' ']),
        };
        e.text = strip_quotes(body);
        return Some(e);
    }

    None
}

fn strip_quotes(s: &str) -> String {
    let t = s.trim();
    let t = t.strip_suffix('.').unwrap_or(t);
    for q in ['"', '\'', '\u{201C}'] {
        if let Some(inner) = t.strip_prefix(q) {
            let close = if q == '\u{201C}' { '\u{201D}' } else { q };
            if let Some(inner) = inner.strip_suffix(close) {
                return inner.to_string();
            }
        }
    }
    t.to_string()
}

/// Read the part of a line that follows the timestamp.
fn parse_body(rest: &str) -> Ev {
    let s = rest.trim_end();
    let trimmed = s.trim_start();

    // irssi meta: `--- Log opened …`, `--- Day changed …`
    if let Some(body) = trimmed.strip_prefix("---") {
        let body = body.trim();
        if let Some(e) = parse_event(body) {
            return e;
        }
        let mut e = ev(Kind::Meta);
        e.text = body.to_string();
        return e;
    }
    // HexChat banners: `**** BEGIN LOGGING AT …` (before the `***` event form).
    if let Some(body) = trimmed.strip_prefix("****") {
        let mut e = ev(Kind::Meta);
        e.text = body.trim().to_string();
        return e;
    }
    for marker in ["-!-", "***", "-->", "<--", "==>", "<==", "*!*"] {
        if let Some(body) = trimmed.strip_prefix(marker) {
            let body = body.trim();
            if let Some(e) = parse_event(body) {
                return e;
            }
            let mut e = ev(Kind::Meta);
            e.text = body.to_string();
            return e;
        }
    }
    // `<nick> message` (a nick never contains whitespace).
    if let Some(close) = trimmed.find('>') {
        if trimmed.starts_with('<') && close > 1 {
            let inner = &trimmed[1..close];
            if inner.len() <= MAX_NICK_LEN && !inner.chars().any(char::is_whitespace) {
                let mut e = ev(Kind::Message);
                e.nick = strip_nick_prefix(inner).to_string();
                e.text = trimmed[close + 1..].trim_start().to_string();
                return e;
            }
        }
    }
    // `* nick does something` — or a mIRC `* Joins: …` event.
    if let Some(body) = trimmed.strip_prefix('*') {
        if body.starts_with(' ') || body.starts_with('\t') {
            let body = body.trim();
            if let Some(e) = parse_event(body) {
                return e;
            }
            let (nick, text) = first_token(body);
            let mut e = ev(Kind::Action);
            e.nick = strip_nick_prefix(nick).to_string();
            e.text = text.to_string();
            return e;
        }
    }
    // `-nick- text` / `-nick:#chan- text` notices.
    if trimmed.starts_with('-') {
        if let Some(close) = trimmed[1..].find('-') {
            let inner = &trimmed[1..1 + close];
            if !inner.is_empty()
                && inner.len() <= MAX_NICK_LEN
                && !inner.chars().any(char::is_whitespace)
            {
                let mut e = ev(Kind::Notice);
                match inner.split_once(':') {
                    Some((n, c)) => {
                        e.nick = strip_nick_prefix(n).to_string();
                        e.channel = c.to_string();
                    }
                    None => e.nick = strip_nick_prefix(inner).to_string(),
                }
                e.text = trimmed[close + 2..].trim_start().to_string();
                return e;
            }
        }
    }
    // WeeChat's bare `--` prefix.
    if let Some(body) = trimmed.strip_prefix("--") {
        if body.starts_with(' ') || body.starts_with('\t') {
            let body = body.trim();
            if let Some(e) = parse_event(body) {
                return e;
            }
            let mut e = ev(Kind::Meta);
            e.text = body.to_string();
            return e;
        }
    }
    // Unmarked event wording, but only on a line that carries a strong marker —
    // a plain sentence must never be guessed into an event.
    let low = trimmed.to_ascii_lowercase();
    if [
        " has joined",
        " has left",
        " has parted",
        " has quit",
        " was kicked",
        " is now known as ",
        " sets mode",
    ]
    .iter()
    .any(|m| low.contains(m))
    {
        if let Some(e) = parse_event(trimmed) {
            return e;
        }
    }
    if low.starts_with("session start") || low.starts_with("session close") || low.starts_with("session ident") {
        let mut e = ev(Kind::Meta);
        e.text = trimmed.to_string();
        return e;
    }

    let mut e = ev(Kind::Unknown);
    e.text = trimmed.to_string();
    e
}

/// WeeChat splits the nick into its own tab-delimited column.
fn parse_weechat_body(rest: &str) -> Ev {
    let (col, msg) = match rest.split_once('\t') {
        Some((c, m)) => (c.trim(), m.trim_end()),
        None => return parse_body(rest),
    };
    match col {
        "-->" | "==>" => parse_event(msg.trim()).unwrap_or_else(|| {
            let mut e = ev(Kind::Join);
            let (nick, host) = actor(msg);
            e.nick = nick;
            e.host = host;
            e
        }),
        "<--" | "<==" => parse_event(msg.trim()).unwrap_or_else(|| {
            let mut e = ev(Kind::Quit);
            let (nick, host) = actor(msg);
            e.nick = nick;
            e.host = host;
            e
        }),
        "--" | "=!=" | "" => parse_event(msg.trim()).unwrap_or_else(|| {
            let mut e = ev(Kind::Meta);
            e.text = msg.trim().to_string();
            e
        }),
        "*" => parse_event(msg.trim()).unwrap_or_else(|| {
            let (nick, text) = first_token(msg);
            let mut e = ev(Kind::Action);
            e.nick = strip_nick_prefix(nick).to_string();
            e.text = text.to_string();
            e
        }),
        other => {
            let mut e = ev(Kind::Message);
            e.nick = strip_nick_prefix(other).to_string();
            e.text = msg.trim_start().to_string();
            e
        }
    }
}

// ------------------------------------------------------------- rendering ---

fn fmt_time(r: &Record, tf: TimeFormat) -> String {
    match tf {
        TimeFormat::None => String::new(),
        TimeFormat::Original => r.raw_time.clone(),
        _ => {
            let (h, mi, s) = match r.time {
                Some(t) => t,
                None => return String::new(),
            };
            match tf {
                TimeFormat::H24 => format!("{h:02}:{mi:02}:{s:02}"),
                TimeFormat::H12 => {
                    let ap = if h < 12 { "AM" } else { "PM" };
                    let h12 = match h % 12 {
                        0 => 12,
                        v => v,
                    };
                    format!("{h12}:{mi:02}:{s:02} {ap}")
                }
                _ => match r.date {
                    Some((y, m, d)) => format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}"),
                    None => format!("{h:02}:{mi:02}:{s:02}"),
                },
            }
        }
    }
}

/// The one-line rendering used by the `timeline` output.
fn timeline_body(r: &Record) -> String {
    let chan = if r.channel.is_empty() {
        String::new()
    } else {
        format!(" {}", r.channel)
    };
    let reason = if r.text.is_empty() {
        String::new()
    } else {
        format!(" ({})", r.text)
    };
    match r.kind {
        Kind::Message => format!("<{}> {}", r.nick, r.text),
        Kind::Action => format!("* {} {}", r.nick, r.text).trim_end().to_string(),
        Kind::Notice => format!("-{}- {}", r.nick, r.text),
        Kind::Join => {
            let host = if r.host.is_empty() {
                String::new()
            } else {
                format!(" ({})", r.host)
            };
            format!("--> {}{} joined{}", r.nick, host, chan)
        }
        Kind::Part => format!("<-- {} left{}{}", r.nick, chan, reason),
        Kind::Quit => format!("<-- {} quit{}", r.nick, reason),
        Kind::Kick => {
            let by = if r.arg.is_empty() {
                String::new()
            } else {
                format!(" by {}", r.arg)
            };
            format!("<-- {} was kicked from{}{}{}", r.nick, chan, by, reason)
        }
        Kind::Nick => format!("--  {} is now known as {}", r.nick, r.arg),
        Kind::Mode => {
            let by = if r.nick.is_empty() {
                String::new()
            } else {
                format!(" by {}", r.nick)
            };
            format!("--  mode{} {}{}", chan, r.arg, by)
        }
        Kind::Topic => {
            if r.nick.is_empty() {
                format!("--  topic{}: {}", chan, r.text)
            } else {
                format!("--  {} set the topic{} to: {}", r.nick, chan, r.text)
            }
        }
        Kind::Meta => format!("--- {}", r.text),
        Kind::Unknown => format!("??  {}", r.text),
    }
}

/// A compact human description used by the Markdown table's last column.
fn detail(r: &Record) -> String {
    let chan = if r.channel.is_empty() {
        String::new()
    } else {
        format!(" {}", r.channel)
    };
    let reason = if r.text.is_empty() {
        String::new()
    } else {
        format!(" ({})", r.text)
    };
    match r.kind {
        Kind::Message | Kind::Action | Kind::Notice | Kind::Meta | Kind::Unknown => r.text.clone(),
        Kind::Join => format!("joined{}", chan).trim().to_string(),
        Kind::Part => format!("left{}{}", chan, reason).trim().to_string(),
        Kind::Quit => format!("quit{}", reason).trim().to_string(),
        Kind::Kick => {
            let by = if r.arg.is_empty() {
                String::new()
            } else {
                format!(" by {}", r.arg)
            };
            format!("kicked from{}{}{}", chan, by, reason).trim().to_string()
        }
        Kind::Nick => format!("now known as {}", r.arg),
        Kind::Mode => format!("mode {}", r.arg).trim().to_string(),
        Kind::Topic => format!("topic: {}", r.text),
    }
}

fn csv_cell(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn md_cell(s: &str) -> String {
    s.replace('|', "\\|").replace(['\n', '\r'], " ")
}

fn record_object(r: &Record, tf: TimeFormat, include_raw: bool) -> Value {
    let mut m = Map::new();
    m.insert("line".into(), Value::from(r.line));
    m.insert("time".into(), Value::from(fmt_time(r, tf)));
    m.insert("type".into(), Value::from(r.kind.label()));
    m.insert("nick".into(), Value::from(r.nick.clone()));
    m.insert("host".into(), Value::from(r.host.clone()));
    m.insert("channel".into(), Value::from(r.channel.clone()));
    m.insert("arg".into(), Value::from(r.arg.clone()));
    m.insert("text".into(), Value::from(r.text.clone()));
    if include_raw {
        m.insert("raw".into(), Value::from(r.raw.clone()));
    }
    Value::Object(m)
}

// ------------------------------------------------------------------- API ---

/// Parse `log` and render it. See the descriptor in `../../src/lib.rs` for the
/// user-facing description of every argument.
#[allow(clippy::too_many_arguments)]
pub fn run(
    log: &str,
    format: &str,
    output: &str,
    date: &str,
    time_format: &str,
    include: &str,
    nicks: &str,
    channel: &str,
    strip_formatting: bool,
    include_raw: bool,
    limit: i64,
) -> Result<String, String> {
    if log.trim().is_empty() {
        return Err("log is empty — paste the raw lines from an IRC client log".into());
    }
    if log.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "log is {} bytes, over the {MAX_INPUT_BYTES} byte limit",
            log.len()
        ));
    }
    if !(0..=MAX_LIMIT).contains(&limit) {
        return Err(format!(
            "limit must be between 0 (no limit) and {MAX_LIMIT}, got {limit}"
        ));
    }
    let dialect_opt = Dialect::parse(format)?;
    let out = Output::parse(output)?;
    let tf = TimeFormat::parse(time_format)?;
    let inc = Include::parse(include)?;

    let base_date = if date.trim().is_empty() {
        None
    } else {
        Some(parse_iso_date(date.trim()).ok_or_else(|| {
            format!(
                "date must be YYYY-MM-DD (a real calendar date), got '{}'",
                date.trim()
            )
        })?)
    };
    let default_channel = channel.trim();
    if !default_channel.is_empty() && !is_channel(default_channel) {
        return Err(format!(
            "channel must start with # & or ! (e.g. #gizza), got '{default_channel}'"
        ));
    }

    let nick_filters: Vec<String> = nicks
        .split(',')
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect();

    let lines: Vec<&str> = log.split('\n').collect();
    let dialect = if dialect_opt == Dialect::Auto {
        detect_dialect(&lines)
    } else {
        dialect_opt
    };

    // ---- parse ----
    let mut records: Vec<Record> = Vec::new();
    let mut current_date = base_date;
    let mut recognised = 0usize;
    for (i, raw_line) in lines.iter().enumerate() {
        let raw_line = raw_line.trim_end_matches(['\r', '\n']);
        if raw_line.trim().is_empty() {
            continue;
        }
        let cleaned = if strip_formatting {
            strip_codes(raw_line)
        } else {
            raw_line.to_string()
        };
        let stamp = take_stamp(&cleaned, dialect);
        let (mut rec_date, rec_time, raw_time, body) = match &stamp {
            Some(s) => (s.date, s.time, s.raw.to_string(), s.rest.to_string()),
            None => (None, None, String::new(), cleaned.trim_start().to_string()),
        };
        // HexChat records a month/day but no year: borrow the running year.
        if let Some((0, m, d)) = rec_date {
            rec_date = current_date.map(|(y, _, _)| (y, m, d));
        }
        let e = if dialect == Dialect::Weechat && stamp.is_some() {
            parse_weechat_body(&body)
        } else {
            parse_body(&body)
        };

        // Day-change / log-open markers move the running date forward.
        if e.kind == Kind::Meta {
            let low = e.text.to_ascii_lowercase();
            if low.starts_with("day changed")
                || low.starts_with("log opened")
                || low.starts_with("begin logging")
                || low.starts_with("session start")
            {
                if let Some(d) = sniff_date(&e.text) {
                    current_date = Some(d);
                }
            }
        }
        if let Some(d) = rec_date {
            current_date = Some(d);
        }
        if e.kind != Kind::Unknown {
            recognised += 1;
        }
        let mut r = Record {
            line: i + 1,
            date: rec_date.or(current_date),
            time: rec_time,
            raw_time,
            kind: e.kind,
            nick: e.nick,
            host: e.host,
            channel: e.channel,
            arg: e.arg,
            text: e.text,
            raw: raw_line.to_string(),
        };
        if r.channel.is_empty() && !default_channel.is_empty() {
            r.channel = default_channel.to_string();
        }
        records.push(r);
    }

    if recognised == 0 {
        return Err(format!(
            "no IRC log lines were recognised (read as '{}' format) — check the format option, \
             or paste lines such as '21:07 <alice> hi' or '[21:07:33] *** Joins: alice (~a@host)'",
            dialect.label()
        ));
    }

    // ---- filter ----
    let mut kept: Vec<&Record> = records
        .iter()
        .filter(|r| inc.keeps(r.kind))
        .filter(|r| {
            if nick_filters.is_empty() {
                return true;
            }
            let n = r.nick.to_ascii_lowercase();
            if n.is_empty() {
                return false;
            }
            nick_filters.iter().any(|f| match f.strip_suffix('*') {
                Some(pre) => n.starts_with(pre),
                None => n == *f,
            })
        })
        .collect();
    if limit > 0 && kept.len() > limit as usize {
        kept.truncate(limit as usize);
    }
    let kept = kept;

    // ---- render ----
    let mut s = String::new();
    match out {
        Output::Timeline => {
            for r in &kept {
                let t = fmt_time(r, tf);
                if t.is_empty() {
                    let _ = writeln!(s, "{}", timeline_body(r));
                } else {
                    let _ = writeln!(s, "{t}  {}", timeline_body(r));
                }
            }
        }
        Output::Json => {
            let arr: Vec<Value> = kept
                .iter()
                .map(|r| record_object(r, tf, include_raw))
                .collect();
            s = serde_json::to_string_pretty(&Value::Array(arr))
                .map_err(|e| format!("could not encode JSON: {e}"))?;
            s.push('\n');
        }
        Output::Ndjson => {
            for r in &kept {
                let v = record_object(r, tf, include_raw);
                let _ = writeln!(
                    s,
                    "{}",
                    serde_json::to_string(&v).map_err(|e| format!("could not encode JSON: {e}"))?
                );
            }
        }
        Output::Csv => {
            s.push_str("line,time,type,nick,host,channel,arg,text");
            if include_raw {
                s.push_str(",raw");
            }
            s.push('\n');
            for r in &kept {
                let mut cells = vec![
                    r.line.to_string(),
                    fmt_time(r, tf),
                    r.kind.label().to_string(),
                    r.nick.clone(),
                    r.host.clone(),
                    r.channel.clone(),
                    r.arg.clone(),
                    r.text.clone(),
                ];
                if include_raw {
                    cells.push(r.raw.clone());
                }
                let row: Vec<String> = cells.iter().map(|c| csv_cell(c)).collect();
                let _ = writeln!(s, "{}", row.join(","));
            }
        }
        Output::Markdown => {
            s.push_str("| Time | Type | Nick | Channel | Detail |\n");
            s.push_str("| --- | --- | --- | --- | --- |\n");
            for r in &kept {
                let _ = writeln!(
                    s,
                    "| {} | {} | {} | {} | {} |",
                    md_cell(&fmt_time(r, tf)),
                    r.kind.label(),
                    md_cell(&r.nick),
                    md_cell(&r.channel),
                    md_cell(&detail(r))
                );
            }
        }
    }
    if s.is_empty() {
        return Err(format!(
            "no lines matched the filters ({} lines parsed) — widen 'include' or clear 'nicks'",
            records.len()
        ));
    }
    Ok(s)
}

// ----------------------------------------------------------------- tests ---

#[cfg(test)]
mod tests {
    use super::*;

    const IRSSI: &str = "--- Log opened Fri Jan 05 20:00:00 2024\n\
21:07 <alice> hey everyone\n\
21:07 -!- bob [~bob@example.net] has joined #gizza\n\
21:08  * alice waves\n\
21:09 -!- bob is now known as bobby\n\
21:10 -!- mode/#gizza [+o bobby] by alice\n\
21:11 -!- alice [~a@example.net] has quit [Ping timeout: 240 seconds]\n";

    fn parse_default(log: &str, output: &str) -> String {
        run(log, "auto", output, "", "iso", "all", "", "", true, false, 0).unwrap()
    }

    #[test]
    fn irssi_timeline_happy_path() {
        let out = parse_default(IRSSI, "timeline");
        assert_eq!(
            out,
            "--- Log opened Fri Jan 05 20:00:00 2024\n\
2024-01-05T21:07:00  <alice> hey everyone\n\
2024-01-05T21:07:00  --> bob (~bob@example.net) joined #gizza\n\
2024-01-05T21:08:00  * alice waves\n\
2024-01-05T21:09:00  --  bob is now known as bobby\n\
2024-01-05T21:10:00  --  mode #gizza +o bobby by alice\n\
2024-01-05T21:11:00  <-- alice quit (Ping timeout: 240 seconds)\n"
        );
    }

    #[test]
    fn irssi_csv_columns() {
        let out = parse_default(IRSSI, "csv");
        assert!(out.starts_with("line,time,type,nick,host,channel,arg,text\n"));
        assert!(out.contains("3,2024-01-05T21:07:00,join,bob,~bob@example.net,#gizza,,\n"));
        assert!(out.contains(
            "7,2024-01-05T21:11:00,quit,alice,~a@example.net,,,Ping timeout: 240 seconds\n"
        ));
    }

    #[test]
    fn weechat_tab_columns_parse() {
        let log = "2024-01-05 21:07:33\talice\they there\n\
2024-01-05 21:07:40\t-->\tbob (~bob@example.net) has joined #gizza\n\
2024-01-05 21:08:00\t<--\tbob (~bob@example.net) has quit (Client Quit)\n\
2024-01-05 21:08:30\t *\talice waves\n";
        let out = run(log, "weechat", "timeline", "", "iso", "all", "", "", true, false, 0).unwrap();
        assert_eq!(
            out,
            "2024-01-05T21:07:33  <alice> hey there\n\
2024-01-05T21:07:40  --> bob (~bob@example.net) joined #gizza\n\
2024-01-05T21:08:00  <-- bob quit (Client Quit)\n\
2024-01-05T21:08:30  * alice waves\n"
        );
    }

    #[test]
    fn bracket_mirc_and_znc_wording() {
        let log = "[21:07:33] <alice> hi\n\
[21:07:40] *** Joins: bob (~bob@example.net)\n\
[21:08:00] * Parts: bob (~bob@example.net) (Leaving)\n\
[21:08:30] * carol was kicked by alice (spam)\n\
[21:09:00] * alice sets mode: +m\n";
        let out = run(log, "bracket", "timeline", "2024-01-05", "24h", "all", "", "#gizza", true, false, 0)
            .unwrap();
        assert_eq!(
            out,
            "21:07:33  <alice> hi\n\
21:07:40  --> bob (~bob@example.net) joined #gizza\n\
21:08:00  <-- bob left #gizza (Leaving)\n\
21:08:30  <-- carol was kicked from #gizza by alice (spam)\n\
21:09:00  --  mode #gizza +m by alice\n"
        );
    }

    #[test]
    fn hexchat_month_day_stamp() {
        let log = "**** BEGIN LOGGING AT Fri Jan  5 20:00:00 2024\n\
Jan 05 21:07:33 <alice>\thello\n\
Jan 05 21:07:40 *\tbob has joined #gizza\n";
        let out = run(log, "hexchat", "csv", "", "iso", "messages", "", "", true, false, 0).unwrap();
        assert!(out.contains("2,2024-01-05T21:07:33,message,alice,,,,hello\n"), "{out}");
    }

    #[test]
    fn day_change_marker_rolls_the_date_forward() {
        let log = "--- Log opened Fri Jan 05 20:00:00 2024\n\
23:59 <alice> almost midnight\n\
--- Day changed Sat Jan 06 2024\n\
00:01 <alice> happy new day\n";
        let out = run(log, "irssi", "timeline", "", "iso", "messages", "", "", true, false, 0).unwrap();
        assert_eq!(
            out,
            "2024-01-05T23:59:00  <alice> almost midnight\n\
2024-01-06T00:01:00  <alice> happy new day\n"
        );
    }

    #[test]
    fn json_record_shape_and_raw_flag() {
        let log = "21:07 <alice> hey\n";
        let out = run(log, "irssi", "json", "2024-01-05", "iso", "all", "", "#gizza", true, true, 0)
            .unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        let r = &v[0];
        assert_eq!(r["line"], 1);
        assert_eq!(r["time"], "2024-01-05T21:07:00");
        assert_eq!(r["type"], "message");
        assert_eq!(r["nick"], "alice");
        assert_eq!(r["channel"], "#gizza");
        assert_eq!(r["text"], "hey");
        assert_eq!(r["raw"], "21:07 <alice> hey");
    }

    #[test]
    fn ndjson_is_one_object_per_line() {
        let out = parse_default(IRSSI, "ndjson");
        assert_eq!(out.lines().count(), 7);
        for l in out.lines() {
            let _: Value = serde_json::from_str(l).unwrap();
        }
    }

    #[test]
    fn markdown_table_has_header_and_rows() {
        let out = parse_default(IRSSI, "markdown");
        assert!(out.starts_with("| Time | Type | Nick | Channel | Detail |\n| --- |"));
        assert!(out.contains("| 2024-01-05T21:07:00 | message | alice |  | hey everyone |\n"));
        assert!(out.contains("| 2024-01-05T21:09:00 | nick | bob |  | now known as bobby |\n"));
    }

    #[test]
    fn include_filters_split_messages_and_events() {
        let msgs = run(IRSSI, "irssi", "timeline", "", "none", "messages", "", "", true, false, 0)
            .unwrap();
        assert_eq!(msgs, "<alice> hey everyone\n* alice waves\n");
        let events =
            run(IRSSI, "irssi", "timeline", "", "none", "events", "", "", true, false, 0).unwrap();
        assert_eq!(events.lines().count(), 4);
        assert!(!events.contains("hey everyone"));
    }

    #[test]
    fn nick_filter_accepts_exact_and_prefix_globs() {
        let exact = run(IRSSI, "irssi", "timeline", "", "none", "all", "alice", "", true, false, 0)
            .unwrap();
        // The mode line's actor is alice too, so it survives the filter.
        assert_eq!(
            exact,
            "<alice> hey everyone\n\
* alice waves\n\
--  mode #gizza +o bobby by alice\n\
<-- alice quit (Ping timeout: 240 seconds)\n"
        );
        let glob =
            run(IRSSI, "irssi", "timeline", "", "none", "all", "bob*", "", true, false, 0).unwrap();
        assert_eq!(glob.lines().count(), 2);
    }

    #[test]
    fn limit_truncates_after_filtering() {
        let out = run(IRSSI, "irssi", "timeline", "", "none", "all", "", "", true, false, 2).unwrap();
        assert_eq!(out.lines().count(), 2);
    }

    #[test]
    fn time_formats_render_the_same_instant() {
        let log = "21:07 <alice> hi\n";
        for (tf, want) in [
            ("iso", "2024-01-05T21:07:00  <alice> hi\n"),
            ("24h", "21:07:00  <alice> hi\n"),
            ("12h", "9:07:00 PM  <alice> hi\n"),
            ("original", "21:07  <alice> hi\n"),
            ("none", "<alice> hi\n"),
        ] {
            let out =
                run(log, "irssi", "timeline", "2024-01-05", tf, "all", "", "", true, false, 0).unwrap();
            assert_eq!(out, want, "time_format={tf}");
        }
    }

    #[test]
    fn formatting_codes_are_stripped_unless_disabled() {
        let log = "21:07 <alice> \u{3}04red\u{3} and \u{2}bold\u{2}\n";
        let on = run(log, "irssi", "timeline", "", "none", "all", "", "", true, false, 0).unwrap();
        assert_eq!(on, "<alice> red and bold\n");
        let off = run(log, "irssi", "timeline", "", "none", "all", "", "", false, false, 0).unwrap();
        assert!(off.contains('\u{3}') && off.contains('\u{2}'));
    }

    #[test]
    fn auto_detect_picks_each_grammar() {
        for (log, want) in [
            ("2024-01-05 21:07:33\talice\thi\n", Dialect::Weechat),
            ("[21:07:33] <alice> hi\n", Dialect::Bracket),
            ("Jan 05 21:07:33 <alice> hi\n", Dialect::Hexchat),
            ("2024-01-05 21:07:33 <alice> hi\n", Dialect::Iso),
            ("21:07 <alice> hi\n", Dialect::Irssi),
            ("<alice> hi\n", Dialect::Plain),
        ] {
            let lines: Vec<&str> = log.split('\n').collect();
            assert_eq!(detect_dialect(&lines), want, "log={log:?}");
        }
    }

    #[test]
    fn topic_wordings_are_recognised() {
        let log = "21:07 -!- alice changed the topic of #gizza to: release day\n\
21:08 -!- Topic for #gizza is: release day\n\
21:09 * carol changes topic to 'quiet hours'\n";
        let out = run(log, "irssi", "csv", "", "none", "events", "", "", true, false, 0).unwrap();
        assert!(out.contains(",topic,alice,,#gizza,,release day\n"), "{out}");
        assert!(out.contains(",topic,,,#gizza,,release day\n"), "{out}");
        assert!(out.contains(",topic,carol,,,,quiet hours\n"), "{out}");
    }

    #[test]
    fn notices_and_unknown_lines_are_kept_apart() {
        let log = "21:07 -alice- server going down\n21:08 total gibberish here\n";
        let out = run(log, "irssi", "csv", "", "none", "all", "", "", true, false, 0).unwrap();
        assert!(out.contains(",notice,alice,,,,server going down\n"), "{out}");
        assert!(out.contains(",unknown,,,,,total gibberish here\n"), "{out}");
    }

    #[test]
    fn err_on_empty_log() {
        let e = run("", "auto", "timeline", "", "iso", "all", "", "", true, false, 0).unwrap_err();
        assert!(e.contains("log is empty"), "{e}");
    }

    #[test]
    fn err_on_limit_over_cap() {
        let e = run("21:07 <a> hi\n", "auto", "timeline", "", "iso", "all", "", "", true, false, 200_001)
            .unwrap_err();
        assert_eq!(
            e,
            "limit must be between 0 (no limit) and 200000, got 200001"
        );
    }

    #[test]
    fn err_on_unknown_enum_values() {
        for (f, o, tf, inc, needle) in [
            ("nope", "timeline", "iso", "all", "unknown format 'nope'"),
            ("auto", "nope", "iso", "all", "unknown output 'nope'"),
            ("auto", "timeline", "nope", "all", "unknown time_format 'nope'"),
            ("auto", "timeline", "iso", "nope", "unknown include 'nope'"),
        ] {
            let e = run("21:07 <a> hi\n", f, o, "", tf, inc, "", "", true, false, 0).unwrap_err();
            assert!(e.contains(needle), "{e}");
        }
    }

    #[test]
    fn err_on_bad_date_and_channel() {
        let e = run("21:07 <a> hi\n", "auto", "timeline", "2024-13-01", "iso", "all", "", "", true, false, 0)
            .unwrap_err();
        assert!(e.contains("date must be YYYY-MM-DD"), "{e}");
        let e = run("21:07 <a> hi\n", "auto", "timeline", "", "iso", "all", "", "gizza", true, false, 0)
            .unwrap_err();
        assert!(e.contains("channel must start with"), "{e}");
    }

    #[test]
    fn err_when_nothing_looks_like_irc() {
        let e = run(
            "the quick brown fox\njumped over the lazy dog\n",
            "auto",
            "timeline",
            "",
            "iso",
            "all",
            "",
            "",
            true,
            false,
            0,
        )
        .unwrap_err();
        assert!(e.contains("no IRC log lines were recognised"), "{e}");
    }

    #[test]
    fn err_when_filters_remove_everything() {
        let e = run(IRSSI, "irssi", "timeline", "", "iso", "all", "nobody", "", true, false, 0)
            .unwrap_err();
        assert!(e.contains("no lines matched the filters"), "{e}");
    }

    #[test]
    fn err_on_oversized_log() {
        let big = "x".repeat(MAX_INPUT_BYTES + 1);
        let e = run(&big, "auto", "timeline", "", "iso", "all", "", "", true, false, 0).unwrap_err();
        assert!(e.contains("over the 5000000 byte limit"), "{e}");
    }
}

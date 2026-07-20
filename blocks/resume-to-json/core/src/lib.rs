//! gizza-ai/resume-to-json core — pure compute, shared by the chat skill block
//! and the web page. No wafer/wasm-bindgen deps.
//!
//! Two jobs, one box:
//! - **extract**: heuristically parse a pasted plain-text resume into the
//!   standard JSON Resume schema (jsonresume.org, v1.0.0) — canonical field
//!   names only, ISO-8601 partial dates (`YYYY`, `YYYY-MM`, `YYYY-MM-DD`),
//!   empty sections omitted. Output is schema-valid by construction (a unit
//!   test feeds extractor output back through the validator).
//! - **validate**: check a pasted resume.json document against the v1.0.0
//!   schema shape. Type mismatches and bad date patterns are ERRORS (the
//!   schema's `type`/`pattern` constraints); malformed email/URL formats and
//!   unknown keys are WARNINGS (the schema declares `additionalProperties:
//!   true`, and JSON-Schema `format` is an annotation).
//!
//! `mode=auto` routes input that parses as a JSON object to validate,
//! anything else to extract.

use regex::Regex;
use serde_json::{json, Map, Value};
use std::sync::OnceLock;

pub const MAX_INPUT_BYTES: usize = 1_048_576; // 1 MiB of pasted text
const SCHEMA_URL: &str =
    "https://raw.githubusercontent.com/jsonresume/resume-schema/v1.0.0/schema.json";

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Mode {
    Auto,
    Extract,
    Validate,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(Mode::Auto),
            "extract" => Ok(Mode::Extract),
            "validate" => Ok(Mode::Validate),
            other => Err(format!(
                "unknown mode '{other}' — expected 'auto', 'extract', or 'validate'"
            )),
        }
    }
}

/// Entry point shared by chat/CLI/page.
pub fn run(data: &str, mode: Mode, schema_ref: bool, pretty: bool) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err(
            "input is empty — paste the resume's plain text (or a resume.json document to validate)"
                .into(),
        );
    }
    if data.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is too large ({} bytes; max {} bytes = 1 MiB)",
            data.len(),
            MAX_INPUT_BYTES
        ));
    }
    let trimmed = data.trim_start();
    let value = match mode {
        Mode::Validate => {
            let v: Value = serde_json::from_str(data).map_err(|e| {
                format!("mode=validate expects a resume.json document, but the input is not valid JSON: {e}")
            })?;
            validate(&v)
        }
        Mode::Auto if trimmed.starts_with('{') => {
            let v: Value = serde_json::from_str(data).map_err(|e| {
                format!(
                    "input looks like JSON but failed to parse: {e}. Fix the JSON, or set mode=extract to treat it as plain resume text"
                )
            })?;
            validate(&v)
        }
        Mode::Auto | Mode::Extract => extract(data, schema_ref)?,
    };
    if pretty {
        serde_json::to_string_pretty(&value)
    } else {
        serde_json::to_string(&value)
    }
    .map_err(|e| format!("failed to serialize output: {e}"))
}

// ---------------------------------------------------------------------------
// Regexes (compiled once)
// ---------------------------------------------------------------------------

fn email_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}").unwrap())
}

fn url_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // Absolute URLs, www.-prefixed hosts, or a bare domain WITH a path
    // (linkedin.com/in/jane). A bare domain alone is too ambiguous.
    RE.get_or_init(|| {
        Regex::new(r"(?i)(?:https?://\S+|www\.\S+|\b(?:[a-z0-9-]+\.)+[a-z]{2,}/[^\s|,;]*)")
            .unwrap()
    })
}

fn date_token_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?ix)\b(?:
              (?:jan|feb|mar|apr|may|jun|jul|aug|sep|oct|nov|dec)[a-z]*\.?\s+\d{1,2},\s+\d{4}
             |(?:jan|feb|mar|apr|may|jun|jul|aug|sep|oct|nov|dec)[a-z]*\.?,?\s+\d{4}
             |(?:0?[1-9]|1[0-2])/(?:19|20)\d{2}
             |(?:19|20)\d{2}-(?:0[1-9]|1[0-2])(?:-(?:[0-2]\d|3[01]))?
             |(?:19|20)\d{2}
            )\b",
        )
        .unwrap()
    })
}

fn range_sep_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)^\s*(?:[\u{2010}-\u{2015}~\u{2192}-]+|to\b|until\b|through\b|thru\b)\s*")
            .unwrap()
    })
}

fn present_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)^(?:present|current(?:ly)?|now|ongoing|today|date)\b").unwrap()
    })
}

/// The EXACT date pattern from the published v1.0.0 schema.
fn schema_date_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^([1-2][0-9]{3}-[0-1][0-9]-[0-3][0-9]|[1-2][0-9]{3}-[0-1][0-9]|[1-2][0-9]{3})$")
            .unwrap()
    })
}

// ---------------------------------------------------------------------------
// Date parsing
// ---------------------------------------------------------------------------

fn month_num(name: &str) -> Option<u32> {
    let lower = name.to_ascii_lowercase();
    let key = lower.get(..3)?;
    Some(match key {
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

/// Normalize one matched date token to a schema date (YYYY / YYYY-MM / YYYY-MM-DD).
fn parse_date_token(tok: &str) -> Option<String> {
    let t = tok.trim();
    // YYYY-MM or YYYY-MM-DD (already ISO).
    if t.len() >= 7 && t.as_bytes().get(4) == Some(&b'-') {
        return Some(t.to_string());
    }
    // MM/YYYY
    if let Some((m, y)) = t.split_once('/') {
        if let (Ok(m), Ok(y)) = (m.trim().parse::<u32>(), y.trim().parse::<u32>()) {
            return Some(format!("{y:04}-{m:02}"));
        }
    }
    // Bare year.
    if t.len() == 4 && t.chars().all(|c| c.is_ascii_digit()) {
        return Some(t.to_string());
    }
    // "Month YYYY" or "Month DD, YYYY".
    let cleaned = t.replace(['.', ','], " ");
    let words: Vec<&str> = cleaned.split_whitespace().collect();
    if let Some(m) = words.first().and_then(|w| month_num(w)) {
        match words.len() {
            2 => {
                if let Ok(y) = words[1].parse::<u32>() {
                    return Some(format!("{y:04}-{m:02}"));
                }
            }
            3 => {
                if let (Ok(d), Ok(y)) = (words[1].parse::<u32>(), words[2].parse::<u32>()) {
                    return Some(format!("{y:04}-{m:02}-{d:02}"));
                }
            }
            _ => {}
        }
    }
    None
}

/// Find a date range ("Jan 2020 – Present", "2016 - 2019") or a single date in
/// a line. Returns (startDate, endDate, byte-span of the whole match).
fn find_date_range(line: &str) -> Option<(String, Option<String>, (usize, usize))> {
    let m = date_token_re().find(line)?;
    let start = parse_date_token(m.as_str())?;
    let rest = &line[m.end()..];
    if let Some(sep) = range_sep_re().find(rest) {
        let after = &rest[sep.end()..];
        if let Some(p) = present_re().find(after) {
            return Some((start, None, (m.start(), m.end() + sep.end() + p.end())));
        }
        if let Some(e) = date_token_re().find(after) {
            if e.start() == 0 {
                if let Some(end) = parse_date_token(e.as_str()) {
                    return Some((start, Some(end), (m.start(), m.end() + sep.end() + e.end())));
                }
            }
        }
    }
    Some((start, None, (m.start(), m.end())))
}

/// Find a single date (for awards/certificates/publications lines).
fn find_single_date(line: &str) -> Option<(String, (usize, usize))> {
    let m = date_token_re().find(line)?;
    parse_date_token(m.as_str()).map(|d| (d, (m.start(), m.end())))
}

// ---------------------------------------------------------------------------
// Section detection
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Section {
    Summary,
    Work,
    Education,
    Skills,
    Projects,
    Languages,
    Awards,
    Certificates,
    Publications,
    Volunteer,
    Interests,
    References,
}

fn heading_section(line: &str) -> Option<Section> {
    let mut t = line.trim();
    t = t.trim_start_matches('#').trim();
    let t = t.trim_matches(|c| c == '*' || c == '_' || c == ':').trim();
    if t.len() > 40 {
        return None;
    }
    let norm = t.to_ascii_lowercase();
    let norm = norm.split_whitespace().collect::<Vec<_>>().join(" ");
    let norm = norm.replace(" and ", " & ");
    let s = norm.as_str();
    use Section::*;
    let sec = match s {
        "summary" | "professional summary" | "profile" | "professional profile" | "about"
        | "about me" | "objective" | "career objective" | "career summary" => Summary,
        "experience" | "work experience" | "professional experience" | "employment"
        | "employment history" | "work history" | "career history" | "relevant experience" => Work,
        "education" | "academic background" | "education & training" | "academics" => Education,
        "skills" | "technical skills" | "core competencies" | "key skills" | "technologies"
        | "skills & tools" | "core skills" | "skills & technologies" => Skills,
        "projects" | "personal projects" | "selected projects" | "side projects"
        | "notable projects" => Projects,
        "languages" => Languages,
        "awards" | "honors" | "honours" | "achievements" | "awards & honors"
        | "honors & awards" | "awards & achievements" => Awards,
        "certificates" | "certifications" | "licenses & certifications"
        | "certifications & licenses" => Certificates,
        "publications" => Publications,
        "volunteer" | "volunteering" | "volunteer experience" | "volunteer work"
        | "community involvement" | "community service" => Volunteer,
        "interests" | "hobbies" | "hobbies & interests" => Interests,
        "references" | "referees" => References,
        _ => return None,
    };
    Some(sec)
}

/// Lines made only of decoration characters (heading underlines, dividers).
fn is_decoration(line: &str) -> bool {
    let t = line.trim();
    !t.is_empty()
        && t.chars()
            .all(|c| matches!(c, '-' | '=' | '_' | '*' | '~' | '#' | '—' | '–' | '·' | '•' | '|'))
}

const BULLETS: [char; 10] = ['-', '–', '—', '*', '•', '·', '▪', '◦', '‣', '>'];

fn strip_bullet(line: &str) -> Option<&str> {
    let t = line.trim_start();
    let mut chars = t.chars();
    let first = chars.next()?;
    if BULLETS.contains(&first) {
        let rest = chars.as_str();
        if rest.starts_with(' ') || rest.starts_with('\t') {
            return Some(rest.trim());
        }
        // "•Led x" without a space, but NOT "-45%" (negative number) or "--".
        if first != '-' && !rest.is_empty() && !rest.starts_with(first) {
            return Some(rest.trim());
        }
    }
    None
}

fn clean_leftover(s: &str) -> String {
    s.trim()
        .trim_matches(|c: char| {
            matches!(c, '|' | '•' | '·' | ',' | ';' | '–' | '—' | '-' | '(' | ')' | ' ' | '\t')
        })
        .trim()
        .to_string()
}

// ---------------------------------------------------------------------------
// Header (basics) extraction
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Basics {
    name: String,
    label: String,
    email: String,
    phone: String,
    url: String,
    summary: String,
    city: String,
    region: String,
    country: String,
    postal: String,
    profiles: Vec<(String, String, String)>, // network, username, url
}

fn split_tokens(line: &str) -> Vec<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"\s*[|•·●◦▪‣]\s*|\s{3,}|\t+").unwrap());
    re.split(line.trim()).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
}

fn strip_label_prefix(t: &str) -> &str {
    let lower = t.to_ascii_lowercase();
    for p in [
        "email:", "e-mail:", "mail:", "phone:", "tel:", "mobile:", "cell:", "web:", "website:",
        "site:", "portfolio:", "location:", "address:", "linkedin:", "github:",
    ] {
        if lower.starts_with(p) {
            return t[p.len()..].trim();
        }
    }
    t
}

fn is_phone(t: &str) -> bool {
    let digits = t.chars().filter(char::is_ascii_digit).count();
    (7..=16).contains(&digits)
        && t.chars().all(|c| c.is_ascii_digit() || " +-().extEXT/".contains(c))
}

fn profile_for(url: &str) -> Option<(&'static str, String)> {
    let lower = url.to_ascii_lowercase();
    let hosts: [(&str, &str); 9] = [
        ("linkedin.com", "LinkedIn"),
        ("github.com", "GitHub"),
        ("gitlab.com", "GitLab"),
        ("twitter.com", "Twitter"),
        ("x.com", "Twitter"),
        ("stackoverflow.com", "Stack Overflow"),
        ("medium.com", "Medium"),
        ("dribbble.com", "Dribbble"),
        ("behance.net", "Behance"),
    ];
    for (host, network) in hosts {
        let Some(pos) = lower.find(host) else { continue };
        // Host must start the token or follow a scheme/'.'/'/' boundary.
        if pos > 0 {
            let prev = lower.as_bytes()[pos - 1];
            if prev != b'/' && prev != b'.' {
                continue;
            }
        }
        let after = &url[pos + host.len()..];
        let username = after
            .split('/')
            .map(|s| s.trim().trim_start_matches('@'))
            .filter(|s| {
                !s.is_empty() && !matches!(s.to_ascii_lowercase().as_str(), "in" | "pub" | "company")
            })
            .next_back()
            .unwrap_or("")
            .trim_end_matches(['.', ',', ';', ')'])
            .to_string();
        return Some((network, username));
    }
    None
}

fn absolutize(url: &str) -> String {
    if url.to_ascii_lowercase().starts_with("http") {
        url.to_string()
    } else {
        format!("https://{url}")
    }
}

fn parse_location(t: &str, b: &mut Basics) {
    let parts: Vec<&str> = t.split(',').map(str::trim).filter(|s| !s.is_empty()).collect();
    if parts.len() < 2 {
        return;
    }
    b.city = parts[0].to_string();
    // Region may carry a postal code: "CA 94105".
    let mut region_words: Vec<&str> = parts[1].split_whitespace().collect();
    if let Some(last) = region_words.last() {
        if last.len() >= 4 && last.chars().all(|c| c.is_ascii_digit()) {
            b.postal = last.to_string();
            region_words.pop();
        }
    }
    b.region = region_words.join(" ");
    if let Some(third) = parts.get(2) {
        if third.len() <= 3 && third.chars().all(|c| c.is_ascii_alphabetic()) {
            b.country = third.to_uppercase();
        } else if b.region.is_empty() {
            b.region = third.to_string();
        }
    }
}

fn word_count(s: &str) -> usize {
    s.split_whitespace().count()
}

fn parse_header(lines: &[String]) -> Basics {
    let mut b = Basics::default();
    let mut prose: Vec<String> = Vec::new();
    for line in lines {
        if is_decoration(line) {
            continue;
        }
        for raw in split_tokens(line) {
            let t = strip_label_prefix(&raw);
            if t.is_empty() {
                continue;
            }
            if let Some(m) = email_re().find(t) {
                if b.email.is_empty() {
                    b.email = m.as_str().to_string();
                }
                continue;
            }
            if is_phone(t) {
                if b.phone.is_empty() {
                    b.phone = t.to_string();
                }
                continue;
            }
            if let Some(m) = url_re().find(t) {
                let url = m.as_str().trim_end_matches(['.', ',', ';', ')']);
                match profile_for(url) {
                    Some((network, username)) => {
                        b.profiles.push((network.to_string(), username, absolutize(url)))
                    }
                    None if b.url.is_empty() => b.url = absolutize(url),
                    None => {}
                }
                continue;
            }
            if t.contains(',') && word_count(t) <= 6 && b.city.is_empty() {
                parse_location(t, &mut b);
                if !b.city.is_empty() {
                    continue;
                }
            }
            // Plain text: name → label → summary prose.
            if b.name.is_empty() && word_count(t) <= 6 && t.len() <= 60 && !t.contains(':') {
                b.name = t.to_string();
            } else if b.label.is_empty() && word_count(t) <= 8 && t.len() <= 70 && !t.ends_with('.')
            {
                b.label = t.to_string();
            } else {
                prose.push(t.to_string());
            }
        }
    }
    b.summary = prose.join(" ");
    b
}

// ---------------------------------------------------------------------------
// Entry-based sections (work / volunteer / education / projects)
// ---------------------------------------------------------------------------

/// Split section lines into blocks on blank lines, then split each block into
/// entries whenever a non-bullet line follows bullet lines (tight format).
fn entry_blocks(lines: &[String]) -> Vec<Vec<String>> {
    let mut out: Vec<Vec<String>> = Vec::new();
    let mut cur: Vec<String> = Vec::new();
    let mut seen_bullet = false;
    for line in lines {
        let t = line.trim();
        if t.is_empty() {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
                seen_bullet = false;
            }
            continue;
        }
        if is_decoration(t) {
            continue;
        }
        let is_bullet = strip_bullet(t).is_some();
        if !is_bullet && seen_bullet {
            out.push(std::mem::take(&mut cur));
            seen_bullet = false;
        }
        seen_bullet |= is_bullet;
        cur.push(t.to_string());
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

const ROLE_KEYWORDS: [&str; 40] = [
    "engineer", "developer", "manager", "designer", "analyst", "director", "lead", "intern",
    "consultant", "architect", "specialist", "officer", "head", "founder", "scientist",
    "administrator", "coordinator", "president", "vp", "cto", "ceo", "cfo", "coo", "recruiter",
    "teacher", "professor", "assistant", "associate", "principal", "staff", "programmer",
    "writer", "editor", "marketer", "accountant", "attorney", "nurse", "technician",
    "supervisor", "researcher",
];

fn has_role_keyword(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    lower.split(|c: char| !c.is_ascii_alphanumeric()).any(|w| ROLE_KEYWORDS.contains(&w))
}

/// Split "left SEP right" on the first known separator.
fn split_pair(line: &str) -> Option<(String, String)> {
    for sep in [" — ", " – ", " | ", " @ ", " at ", ", ", " - "] {
        if let Some((a, c)) = line.split_once(sep) {
            let (a, c) = (a.trim(), c.trim());
            if !a.is_empty() && !c.is_empty() {
                return Some((a.to_string(), c.to_string()));
            }
        }
    }
    None
}

#[derive(Default)]
struct WorkEntry {
    org: String,
    position: String,
    location: String,
    url: String,
    start: Option<String>,
    end: Option<String>,
    summary_parts: Vec<String>,
    highlights: Vec<String>,
}

fn parse_work_entry(block: &[String]) -> WorkEntry {
    let mut e = WorkEntry::default();
    let mut leftovers: Vec<String> = Vec::new();
    for line in block {
        if let Some(text) = strip_bullet(line) {
            if !text.is_empty() {
                e.highlights.push(text.to_string());
            }
            continue;
        }
        let mut rest = line.clone();
        if e.start.is_none() {
            if let Some((start, end, (a, z))) = find_date_range(&rest) {
                e.start = Some(start);
                e.end = end;
                rest = format!("{} {}", &rest[..a], &rest[z..]);
            }
        }
        if e.url.is_empty() {
            if let Some(m) = url_re().find(&rest.clone()) {
                e.url = absolutize(m.as_str().trim_end_matches(['.', ',', ';', ')']));
                rest = format!("{} {}", &rest[..m.start()], &rest[m.end()..]);
            }
        }
        let rest = clean_leftover(&rest);
        if !rest.is_empty() {
            leftovers.push(rest);
        }
    }
    for text in leftovers {
        if e.position.is_empty() && e.org.is_empty() {
            match split_pair(&text) {
                Some((a, b)) => {
                    if has_role_keyword(&b) && !has_role_keyword(&a) {
                        e.org = a;
                        e.position = b;
                    } else {
                        e.position = a;
                        e.org = b;
                    }
                }
                None => {
                    if has_role_keyword(&text) {
                        e.position = text;
                    } else {
                        e.org = text;
                    }
                }
            }
        } else if e.org.is_empty() && word_count(&text) <= 8 && !text.ends_with('.') {
            match split_pair(&text) {
                Some((a, b)) if b.contains(',') && word_count(&b) <= 6 => {
                    e.org = a;
                    e.location = b;
                }
                _ => e.org = text,
            }
        } else if e.location.is_empty() && text.contains(',') && word_count(&text) <= 6 {
            e.location = text;
        } else {
            e.summary_parts.push(text);
        }
    }
    e
}

fn work_value(e: &WorkEntry, org_key: &str) -> Value {
    let mut m = Map::new();
    if !e.org.is_empty() {
        m.insert(org_key.into(), json!(e.org));
    }
    if !e.location.is_empty() && org_key == "name" {
        m.insert("location".into(), json!(e.location));
    }
    if !e.position.is_empty() {
        m.insert("position".into(), json!(e.position));
    }
    if !e.url.is_empty() {
        m.insert("url".into(), json!(e.url));
    }
    if let Some(s) = &e.start {
        m.insert("startDate".into(), json!(s));
    }
    if let Some(s) = &e.end {
        m.insert("endDate".into(), json!(s));
    }
    if !e.summary_parts.is_empty() {
        m.insert("summary".into(), json!(e.summary_parts.join(" ")));
    }
    if !e.highlights.is_empty() {
        m.insert("highlights".into(), json!(e.highlights));
    }
    Value::Object(m)
}

// -- education ---------------------------------------------------------------

const DEGREE_WORDS: [&str; 30] = [
    "bachelor", "bachelors", "master", "masters", "doctor", "doctorate", "phd", "ph.d", "mba",
    "b.s", "bs", "bsc", "b.sc", "ba", "b.a", "ms", "m.s", "msc", "m.sc", "ma", "m.a", "meng",
    "beng", "btech", "b.tech", "mtech", "m.tech", "associate", "associates", "diploma",
];

fn has_degree_word(s: &str) -> bool {
    s.to_ascii_lowercase()
        .split(|c: char| c.is_whitespace() || c == ',')
        .map(|w| w.trim_end_matches('.'))
        .any(|w| DEGREE_WORDS.contains(&w))
}

/// "Bachelor of Science in Computer Science" → ("Bachelor of Science", "Computer Science");
/// "B.S. Computer Science" → ("B.S.", "Computer Science").
fn parse_degree(s: &str) -> (String, String) {
    let lower = s.to_ascii_lowercase();
    if let Some(pos) = lower.find(" in ") {
        return (s[..pos].trim().to_string(), s[pos + 4..].trim().to_string());
    }
    let mut words = s.split_whitespace();
    if let Some(first) = words.next() {
        if DEGREE_WORDS.contains(&first.to_ascii_lowercase().trim_end_matches('.')) {
            let rest: Vec<&str> = words.collect();
            let rest = rest.join(" ");
            let rest = rest.trim_start_matches([',', ':']).trim();
            if !rest.is_empty()
                && !matches!(
                    rest.split_whitespace().next().unwrap_or("").to_ascii_lowercase().as_str(),
                    "of" | "degree"
                )
            {
                return (first.to_string(), rest.to_string());
            }
        }
    }
    (s.trim().to_string(), String::new())
}

fn parse_education_entry(block: &[String]) -> Value {
    let mut m = Map::new();
    let mut institution = String::new();
    let mut study = String::new();
    let mut area = String::new();
    let mut score = String::new();
    let mut courses: Vec<String> = Vec::new();
    let mut start: Option<String> = None;
    let mut end: Option<String> = None;
    let mut url = String::new();
    for line in block {
        let bullet = strip_bullet(line);
        let content = bullet.unwrap_or(line).to_string();
        let lower = content.to_ascii_lowercase();
        if let Some(pos) = lower.find("gpa") {
            let after = content[pos + 3..].trim_start_matches([':', ' ', '=']);
            let val: String = after
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '/')
                .collect();
            if !val.is_empty() {
                score = val;
                continue;
            }
        }
        if lower.starts_with("relevant coursework") || lower.starts_with("coursework") {
            if let Some((_, list)) = content.split_once(':') {
                courses.extend(
                    list.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
                );
            }
            continue;
        }
        if bullet.is_some() {
            courses.push(content);
            continue;
        }
        let mut rest = content;
        if start.is_none() {
            if let Some((s, e, (a, z))) = find_date_range(&rest) {
                start = Some(s);
                end = e;
                rest = format!("{} {}", &rest[..a], &rest[z..]);
            }
        }
        if url.is_empty() {
            if let Some(mch) = url_re().find(&rest.clone()) {
                url = absolutize(mch.as_str().trim_end_matches(['.', ',', ';', ')']));
                rest = format!("{} {}", &rest[..mch.start()], &rest[mch.end()..]);
            }
        }
        let rest = clean_leftover(&rest);
        if rest.is_empty() {
            continue;
        }
        match split_pair(&rest) {
            Some((a, b)) => {
                let (deg, inst) = if has_degree_word(&a) && !has_degree_word(&b) {
                    (Some(a), Some(b))
                } else if has_degree_word(&b) && !has_degree_word(&a) {
                    (Some(b), Some(a))
                } else if institution.is_empty() {
                    (None, Some(rest.clone()))
                } else {
                    (Some(rest.clone()), None)
                };
                if let Some(d) = deg {
                    if study.is_empty() {
                        let (s, ar) = parse_degree(&d);
                        study = s;
                        area = ar;
                    }
                }
                if let Some(i) = inst {
                    if institution.is_empty() {
                        institution = i;
                    }
                }
            }
            None => {
                if has_degree_word(&rest) && study.is_empty() {
                    let (s, ar) = parse_degree(&rest);
                    study = s;
                    area = ar;
                } else if institution.is_empty() {
                    institution = rest;
                }
            }
        }
    }
    if !institution.is_empty() {
        m.insert("institution".into(), json!(institution));
    }
    if !url.is_empty() {
        m.insert("url".into(), json!(url));
    }
    if !area.is_empty() {
        m.insert("area".into(), json!(area));
    }
    if !study.is_empty() {
        m.insert("studyType".into(), json!(study));
    }
    if let Some(s) = start {
        m.insert("startDate".into(), json!(s));
    }
    if let Some(e) = end {
        m.insert("endDate".into(), json!(e));
    }
    if !score.is_empty() {
        m.insert("score".into(), json!(score));
    }
    if !courses.is_empty() {
        m.insert("courses".into(), json!(courses));
    }
    Value::Object(m)
}

// -- projects ----------------------------------------------------------------

fn parse_project_entry(block: &[String]) -> Value {
    let mut m = Map::new();
    let mut name = String::new();
    let mut url = String::new();
    let mut start: Option<String> = None;
    let mut end: Option<String> = None;
    let mut desc_parts: Vec<String> = Vec::new();
    let mut highlights: Vec<String> = Vec::new();
    for line in block {
        if let Some(text) = strip_bullet(line) {
            if !text.is_empty() {
                highlights.push(text.to_string());
            }
            continue;
        }
        let mut rest = line.clone();
        if start.is_none() {
            if let Some((s, e, (a, z))) = find_date_range(&rest) {
                start = Some(s);
                end = e;
                rest = format!("{} {}", &rest[..a], &rest[z..]);
            }
        }
        if url.is_empty() {
            if let Some(mch) = url_re().find(&rest.clone()) {
                url = absolutize(mch.as_str().trim_end_matches(['.', ',', ';', ')']));
                rest = format!("{} {}", &rest[..mch.start()], &rest[mch.end()..]);
            }
        }
        let rest = clean_leftover(&rest);
        if rest.is_empty() {
            continue;
        }
        if name.is_empty() {
            match split_pair(&rest) {
                Some((a, b)) => {
                    name = a;
                    desc_parts.push(b);
                }
                None => name = rest,
            }
        } else {
            desc_parts.push(rest);
        }
    }
    if !name.is_empty() {
        m.insert("name".into(), json!(name));
    }
    if !desc_parts.is_empty() {
        m.insert("description".into(), json!(desc_parts.join(" ")));
    }
    if !highlights.is_empty() {
        m.insert("highlights".into(), json!(highlights));
    }
    if let Some(s) = start {
        m.insert("startDate".into(), json!(s));
    }
    if let Some(e) = end {
        m.insert("endDate".into(), json!(e));
    }
    if !url.is_empty() {
        m.insert("url".into(), json!(url));
    }
    Value::Object(m)
}

// -- line-based sections -----------------------------------------------------

fn split_list(s: &str) -> Vec<String> {
    s.split([',', ';', '•', '·', '|'])
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .collect()
}

const LEVEL_WORDS: [&str; 13] = [
    "advanced", "intermediate", "beginner", "expert", "proficient", "familiar", "basic",
    "native", "fluent", "professional", "conversational", "elementary", "working",
];

/// "Python (Expert)" → ("Python", Some("Expert")).
fn split_level(s: &str) -> (String, Option<String>) {
    if let Some(open) = s.rfind('(') {
        if let Some(close) = s[open..].find(')') {
            let inner = s[open + 1..open + close].trim();
            let first = inner.split_whitespace().next().unwrap_or("").to_ascii_lowercase();
            if LEVEL_WORDS.contains(&first.as_str()) {
                let name = s[..open].trim().to_string();
                return (name, Some(inner.to_string()));
            }
        }
    }
    (s.trim().to_string(), None)
}

fn parse_skills(lines: &[String], group_key: &str) -> Vec<Value> {
    let mut out = Vec::new();
    for line in lines {
        let t = strip_bullet(line).unwrap_or(line).trim();
        if t.is_empty() || is_decoration(t) {
            continue;
        }
        if let Some((name, list)) = t.split_once(':') {
            let keywords = split_list(list);
            if !keywords.is_empty() {
                let mut m = Map::new();
                m.insert(group_key.into(), json!(name.trim()));
                m.insert("keywords".into(), json!(keywords));
                out.push(Value::Object(m));
                continue;
            }
        }
        let items = split_list(t);
        for item in items {
            let (name, level) = split_level(&item);
            if name.is_empty() {
                continue;
            }
            let mut m = Map::new();
            m.insert(group_key.into(), json!(name));
            if let Some(l) = level {
                if group_key == "name" {
                    m.insert("level".into(), json!(l));
                }
            }
            out.push(Value::Object(m));
        }
    }
    out
}

fn parse_languages(lines: &[String]) -> Vec<Value> {
    let mut out = Vec::new();
    for line in lines {
        let t = strip_bullet(line).unwrap_or(line).trim();
        if t.is_empty() || is_decoration(t) {
            continue;
        }
        for item in t.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            let (lang, fluency) = split_level(item);
            let (lang, fluency) = if fluency.is_some() {
                (lang, fluency)
            } else if let Some((l, f)) = item
                .split_once(" – ")
                .or_else(|| item.split_once(" — "))
                .or_else(|| item.split_once(": "))
                .or_else(|| item.split_once(" - "))
            {
                (l.trim().to_string(), Some(f.trim().to_string()))
            } else {
                (item.to_string(), None)
            };
            if lang.is_empty() {
                continue;
            }
            let mut m = Map::new();
            m.insert("language".into(), json!(lang));
            if let Some(f) = fluency {
                m.insert("fluency".into(), json!(f));
            }
            out.push(Value::Object(m));
        }
    }
    out
}

/// Awards / certificates / publications share a "Name — Issuer, Date" line shape.
fn parse_dated_lines(lines: &[String], name_key: &str, by_key: &str, date_key: &str) -> Vec<Value> {
    let mut out = Vec::new();
    for line in lines {
        let t = strip_bullet(line).unwrap_or(line).trim().to_string();
        if t.is_empty() || is_decoration(&t) {
            continue;
        }
        let mut rest = t;
        let mut date: Option<String> = None;
        if let Some((d, (a, z))) = find_single_date(&rest) {
            date = Some(d);
            rest = format!("{} {}", &rest[..a], &rest[z..]);
        }
        let mut url = String::new();
        if let Some(mch) = url_re().find(&rest.clone()) {
            url = absolutize(mch.as_str().trim_end_matches(['.', ',', ';', ')']));
            rest = format!("{} {}", &rest[..mch.start()], &rest[mch.end()..]);
        }
        let rest = clean_leftover(&rest);
        if rest.is_empty() && date.is_none() {
            continue;
        }
        let (name, by) = match split_pair(&rest) {
            Some((a, b)) => (a, Some(b)),
            None => (rest, None),
        };
        let mut m = Map::new();
        if !name.is_empty() {
            m.insert(name_key.into(), json!(name.trim_matches(['"', '“', '”']).trim()));
        }
        if let Some(d) = date {
            m.insert(date_key.into(), json!(d));
        }
        if let Some(b) = by {
            let b = clean_leftover(&b);
            if !b.is_empty() {
                m.insert(by_key.into(), json!(b));
            }
        }
        if !url.is_empty() && name_key == "name" {
            m.insert("url".into(), json!(url));
        }
        if !m.is_empty() {
            out.push(Value::Object(m));
        }
    }
    out
}

fn parse_references(lines: &[String]) -> Vec<Value> {
    let mut out = Vec::new();
    for block in entry_blocks(lines) {
        let cleaned: Vec<String> = block
            .iter()
            .map(|l| strip_bullet(l).unwrap_or(l).trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();
        if cleaned.is_empty() {
            continue;
        }
        let lower = cleaned.join(" ").to_ascii_lowercase();
        if lower.contains("available") && lower.contains("request") {
            continue; // "References available upon request" is not a reference.
        }
        let (name, mut reference) = match split_pair(&cleaned[0]) {
            Some((a, b)) => (a, vec![b]),
            None => (cleaned[0].clone(), Vec::new()),
        };
        reference.extend(cleaned[1..].iter().cloned());
        let mut m = Map::new();
        m.insert("name".into(), json!(name));
        if !reference.is_empty() {
            m.insert("reference".into(), json!(reference.join(" ")));
        }
        out.push(Value::Object(m));
    }
    out
}

// ---------------------------------------------------------------------------
// extract — plain text → JSON Resume
// ---------------------------------------------------------------------------

fn extract(text: &str, schema_ref: bool) -> Result<Value, String> {
    let mut header: Vec<String> = Vec::new();
    let mut sections: Vec<(Section, Vec<String>)> = Vec::new();
    for raw in text.lines() {
        let line = raw.trim_end();
        if let Some(sec) = heading_section(line) {
            match sections.iter().position(|(s, _)| *s == sec) {
                Some(idx) => {
                    // Repeated heading: merge, and make it current again.
                    let mut entry = sections.remove(idx);
                    entry.1.push(String::new());
                    sections.push(entry);
                }
                None => sections.push((sec, Vec::new())),
            }
            continue;
        }
        match sections.last_mut() {
            Some((_, lines)) => lines.push(line.to_string()),
            None => header.push(line.to_string()),
        }
    }

    let basics = parse_header(&header);
    let mut summary = basics.summary.clone();
    let mut root = Map::new();
    if schema_ref {
        root.insert("$schema".into(), json!(SCHEMA_URL));
    }

    let mut work: Vec<Value> = Vec::new();
    let mut volunteer: Vec<Value> = Vec::new();
    let mut education: Vec<Value> = Vec::new();
    let mut awards: Vec<Value> = Vec::new();
    let mut certificates: Vec<Value> = Vec::new();
    let mut publications: Vec<Value> = Vec::new();
    let mut skills: Vec<Value> = Vec::new();
    let mut languages: Vec<Value> = Vec::new();
    let mut interests: Vec<Value> = Vec::new();
    let mut references: Vec<Value> = Vec::new();
    let mut projects: Vec<Value> = Vec::new();

    for (sec, lines) in &sections {
        match sec {
            Section::Summary => {
                let text = lines
                    .iter()
                    .map(|l| l.trim())
                    .filter(|l| !l.is_empty() && !is_decoration(l))
                    .collect::<Vec<_>>()
                    .join(" ");
                if !text.is_empty() {
                    summary = text;
                }
            }
            Section::Work => work.extend(
                entry_blocks(lines).iter().map(|b| work_value(&parse_work_entry(b), "name")),
            ),
            Section::Volunteer => volunteer.extend(
                entry_blocks(lines)
                    .iter()
                    .map(|b| work_value(&parse_work_entry(b), "organization")),
            ),
            Section::Education => {
                education.extend(entry_blocks(lines).iter().map(|b| parse_education_entry(b)))
            }
            Section::Projects => {
                projects.extend(entry_blocks(lines).iter().map(|b| parse_project_entry(b)))
            }
            Section::Skills => skills.extend(parse_skills(lines, "name")),
            Section::Languages => languages.extend(parse_languages(lines)),
            Section::Interests => interests.extend(parse_skills(lines, "name")),
            Section::Awards => awards.extend(parse_dated_lines(lines, "title", "awarder", "date")),
            Section::Certificates => {
                certificates.extend(parse_dated_lines(lines, "name", "issuer", "date"))
            }
            Section::Publications => {
                publications.extend(parse_dated_lines(lines, "name", "publisher", "releaseDate"))
            }
            Section::References => references.extend(parse_references(lines)),
        }
    }

    // basics (canonical field order).
    let mut bm = Map::new();
    if !basics.name.is_empty() {
        bm.insert("name".into(), json!(basics.name));
    }
    if !basics.label.is_empty() {
        bm.insert("label".into(), json!(basics.label));
    }
    if !basics.email.is_empty() {
        bm.insert("email".into(), json!(basics.email));
    }
    if !basics.phone.is_empty() {
        bm.insert("phone".into(), json!(basics.phone));
    }
    if !basics.url.is_empty() {
        bm.insert("url".into(), json!(basics.url));
    }
    if !summary.is_empty() {
        bm.insert("summary".into(), json!(summary));
    }
    let mut loc = Map::new();
    if !basics.postal.is_empty() {
        loc.insert("postalCode".into(), json!(basics.postal));
    }
    if !basics.city.is_empty() {
        loc.insert("city".into(), json!(basics.city));
    }
    if !basics.country.is_empty() {
        loc.insert("countryCode".into(), json!(basics.country));
    }
    if !basics.region.is_empty() {
        loc.insert("region".into(), json!(basics.region));
    }
    if !loc.is_empty() {
        bm.insert("location".into(), Value::Object(loc));
    }
    if !basics.profiles.is_empty() {
        let profiles: Vec<Value> = basics
            .profiles
            .iter()
            .map(|(network, username, url)| {
                let mut p = Map::new();
                p.insert("network".into(), json!(network));
                if !username.is_empty() {
                    p.insert("username".into(), json!(username));
                }
                p.insert("url".into(), json!(url));
                Value::Object(p)
            })
            .collect();
        bm.insert("profiles".into(), json!(profiles));
    }
    if !bm.is_empty() {
        root.insert("basics".into(), Value::Object(bm));
    }

    for (key, vals) in [
        ("work", work),
        ("volunteer", volunteer),
        ("education", education),
        ("awards", awards),
        ("certificates", certificates),
        ("publications", publications),
        ("skills", skills),
        ("languages", languages),
        ("interests", interests),
        ("references", references),
        ("projects", projects),
    ] {
        let vals: Vec<Value> =
            vals.into_iter().filter(|v| v.as_object().is_some_and(|m| !m.is_empty())).collect();
        if !vals.is_empty() {
            root.insert(key.into(), json!(vals));
        }
    }
    if schema_ref {
        root.insert("meta".into(), json!({ "version": "v1.0.0" }));
    }

    let only_schema_keys = root.keys().all(|k| k == "$schema" || k == "meta");
    if only_schema_keys {
        return Err(
            "no recognizable resume content found — paste the resume as plain text (name and contact lines first, then sections like Experience, Education, Skills)"
                .into(),
        );
    }
    Ok(Value::Object(root))
}

// ---------------------------------------------------------------------------
// validate — resume.json → report
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum Kind {
    Str,
    Date,
    Uri,
    Email,
    StrArr,
}

struct Report {
    errors: Vec<String>,
    warnings: Vec<String>,
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

fn check_field(path: &str, v: &Value, kind: Kind, rep: &mut Report) {
    if let Kind::StrArr = kind {
        match v {
            Value::Array(items) => {
                for (i, item) in items.iter().enumerate() {
                    if !item.is_string() {
                        rep.errors.push(format!(
                            "{path}[{i}]: expected a string, got {}",
                            type_name(item)
                        ));
                    }
                }
            }
            other => rep.errors.push(format!(
                "{path}: expected an array of strings, got {}",
                type_name(other)
            )),
        }
        return;
    }
    let Some(s) = v.as_str() else {
        rep.errors.push(format!("{path}: expected a string, got {}", type_name(v)));
        return;
    };
    match kind {
        Kind::Str | Kind::StrArr => {}
        Kind::Date => {
            if !schema_date_re().is_match(s) {
                rep.errors.push(format!(
                    "{path}: \"{s}\" is not an ISO-8601 date (expected YYYY, YYYY-MM, or YYYY-MM-DD)"
                ));
            }
        }
        Kind::Email => {
            let ok = s.split_once('@').is_some_and(|(local, domain)| {
                !local.is_empty() && domain.contains('.') && !domain.contains('@')
            }) && !s.contains(char::is_whitespace);
            if !ok {
                rep.warnings.push(format!("{path}: \"{s}\" does not look like an email address"));
            }
        }
        Kind::Uri => {
            if !s.contains("://") {
                rep.warnings.push(format!(
                    "{path}: \"{s}\" is not an absolute URL (missing a scheme like https://)"
                ));
            }
        }
    }
}

fn check_object(path: &str, v: &Value, specs: &[(&str, Kind)], rep: &mut Report) {
    let Some(obj) = v.as_object() else {
        rep.errors.push(format!("{path}: expected an object, got {}", type_name(v)));
        return;
    };
    for (key, val) in obj {
        match specs.iter().find(|(k, _)| k == key) {
            Some((_, kind)) => check_field(&format!("{path}.{key}"), val, *kind, rep),
            None => rep.warnings.push(format!(
                "{path}.{key}: unknown field (allowed by the schema, but ignored by most tools)"
            )),
        }
    }
}

fn check_section_array(name: &str, v: &Value, specs: &[(&str, Kind)], rep: &mut Report) {
    let Some(items) = v.as_array() else {
        rep.errors.push(format!("{name}: expected an array, got {}", type_name(v)));
        return;
    };
    for (i, item) in items.iter().enumerate() {
        check_object(&format!("{name}[{i}]"), item, specs, rep);
    }
}

pub fn validate(v: &Value) -> Value {
    use Kind::*;
    let mut rep = Report { errors: Vec::new(), warnings: Vec::new() };

    let Some(root) = v.as_object() else {
        return json!({
            "valid": false,
            "errors": [format!("root: expected a JSON object (a resume.json document), got {}", type_name(v))],
            "warnings": [],
            "summary": { "sections": [], "counts": {} }
        });
    };

    const WORK: &[(&str, Kind)] = &[
        ("name", Str),
        ("location", Str),
        ("description", Str),
        ("position", Str),
        ("url", Uri),
        ("startDate", Date),
        ("endDate", Date),
        ("summary", Str),
        ("highlights", StrArr),
    ];
    const VOLUNTEER: &[(&str, Kind)] = &[
        ("organization", Str),
        ("position", Str),
        ("url", Uri),
        ("startDate", Date),
        ("endDate", Date),
        ("summary", Str),
        ("highlights", StrArr),
    ];
    const EDUCATION: &[(&str, Kind)] = &[
        ("institution", Str),
        ("url", Uri),
        ("area", Str),
        ("studyType", Str),
        ("startDate", Date),
        ("endDate", Date),
        ("score", Str),
        ("courses", StrArr),
    ];
    const AWARDS: &[(&str, Kind)] =
        &[("title", Str), ("date", Date), ("awarder", Str), ("summary", Str)];
    const CERTIFICATES: &[(&str, Kind)] =
        &[("name", Str), ("date", Date), ("url", Uri), ("issuer", Str)];
    const PUBLICATIONS: &[(&str, Kind)] = &[
        ("name", Str),
        ("publisher", Str),
        ("releaseDate", Date),
        ("url", Uri),
        ("summary", Str),
    ];
    const SKILLS: &[(&str, Kind)] = &[("name", Str), ("level", Str), ("keywords", StrArr)];
    const LANGUAGES: &[(&str, Kind)] = &[("language", Str), ("fluency", Str)];
    const INTERESTS: &[(&str, Kind)] = &[("name", Str), ("keywords", StrArr)];
    const REFERENCES: &[(&str, Kind)] = &[("name", Str), ("reference", Str)];
    const PROJECTS: &[(&str, Kind)] = &[
        ("name", Str),
        ("description", Str),
        ("highlights", StrArr),
        ("keywords", StrArr),
        ("startDate", Date),
        ("endDate", Date),
        ("url", Uri),
        ("roles", StrArr),
        ("entity", Str),
        ("type", Str),
    ];
    const BASICS: &[(&str, Kind)] = &[
        ("name", Str),
        ("label", Str),
        ("image", Uri),
        ("email", Email),
        ("phone", Str),
        ("url", Uri),
        ("summary", Str),
    ];
    const LOCATION: &[(&str, Kind)] = &[
        ("address", Str),
        ("postalCode", Str),
        ("city", Str),
        ("countryCode", Str),
        ("region", Str),
    ];
    const PROFILE: &[(&str, Kind)] = &[("network", Str), ("username", Str), ("url", Uri)];

    const KNOWN: &[&str] = &[
        "basics",
        "work",
        "volunteer",
        "education",
        "awards",
        "certificates",
        "publications",
        "skills",
        "languages",
        "interests",
        "references",
        "projects",
        "meta",
    ];

    for (key, val) in root {
        match key.as_str() {
            "$schema" => check_field("$schema", val, Uri, &mut rep),
            "basics" => {
                let Some(basics) = val.as_object() else {
                    rep.errors.push(format!("basics: expected an object, got {}", type_name(val)));
                    continue;
                };
                for (bk, bv) in basics {
                    match bk.as_str() {
                        "location" => check_object("basics.location", bv, LOCATION, &mut rep),
                        "profiles" => check_section_array("basics.profiles", bv, PROFILE, &mut rep),
                        _ => match BASICS.iter().find(|(k, _)| k == bk) {
                            Some((_, kind)) => {
                                check_field(&format!("basics.{bk}"), bv, *kind, &mut rep)
                            }
                            None => rep.warnings.push(format!(
                                "basics.{bk}: unknown field (allowed by the schema, but ignored by most tools)"
                            )),
                        },
                    }
                }
            }
            "work" => check_section_array("work", val, WORK, &mut rep),
            "volunteer" => check_section_array("volunteer", val, VOLUNTEER, &mut rep),
            "education" => check_section_array("education", val, EDUCATION, &mut rep),
            "awards" => check_section_array("awards", val, AWARDS, &mut rep),
            "certificates" => check_section_array("certificates", val, CERTIFICATES, &mut rep),
            "publications" => check_section_array("publications", val, PUBLICATIONS, &mut rep),
            "skills" => check_section_array("skills", val, SKILLS, &mut rep),
            "languages" => check_section_array("languages", val, LANGUAGES, &mut rep),
            "interests" => check_section_array("interests", val, INTERESTS, &mut rep),
            "references" => check_section_array("references", val, REFERENCES, &mut rep),
            "projects" => check_section_array("projects", val, PROJECTS, &mut rep),
            "meta" => {
                check_object(
                    "meta",
                    val,
                    &[("canonical", Uri), ("version", Str), ("lastModified", Str)],
                    &mut rep,
                );
            }
            other => rep.warnings.push(format!(
                "{other}: unknown top-level section (allowed by the schema, but ignored by most tools)"
            )),
        }
    }

    let sections: Vec<&str> = KNOWN.iter().filter(|k| root.contains_key(**k)).copied().collect();
    let mut counts = Map::new();
    for key in &sections {
        if let Some(arr) = root.get(*key).and_then(Value::as_array) {
            counts.insert((*key).to_string(), json!(arr.len()));
        }
    }

    json!({
        "valid": rep.errors.is_empty(),
        "errors": rep.errors,
        "warnings": rep.warnings,
        "summary": { "sections": sections, "counts": counts }
    })
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "Jane Doe\nSenior Software Engineer\nSan Francisco, CA | jane.doe@example.com | (555) 123-4567 | linkedin.com/in/janedoe\n\nSummary\nEngineer with 8 years of experience building distributed systems.\n\nExperience\n\nSenior Software Engineer — Acme Corp\nJan 2020 – Present | San Francisco, CA\n- Led migration to a service mesh across 40 services\n- Cut p99 latency by 45%\n\nSoftware Engineer — Beta Labs\nJun 2016 – Dec 2019\n- Built the billing pipeline in Rust\n\nEducation\n\nB.S. in Computer Science — State University\n2012 – 2016\nGPA: 3.8/4.0\n\nSkills\nLanguages: Rust, Python, TypeScript\nInfrastructure: Kubernetes, Terraform\n\nLanguages\nEnglish (Native), Spanish (Professional)\n";

    fn extract_sample() -> Value {
        let out = run(SAMPLE, Mode::Auto, false, false).unwrap();
        serde_json::from_str(&out).unwrap()
    }

    #[test]
    fn extracts_basics_from_header() {
        let v = extract_sample();
        let b = &v["basics"];
        assert_eq!(b["name"], "Jane Doe");
        assert_eq!(b["label"], "Senior Software Engineer");
        assert_eq!(b["email"], "jane.doe@example.com");
        assert_eq!(b["phone"], "(555) 123-4567");
        assert_eq!(b["location"]["city"], "San Francisco");
        assert_eq!(b["location"]["region"], "CA");
        assert_eq!(b["profiles"][0]["network"], "LinkedIn");
        assert_eq!(b["profiles"][0]["username"], "janedoe");
        assert_eq!(b["profiles"][0]["url"], "https://linkedin.com/in/janedoe");
        assert_eq!(b["summary"], "Engineer with 8 years of experience building distributed systems.");
    }

    #[test]
    fn extracts_work_entries_with_iso_dates() {
        let v = extract_sample();
        let w = v["work"].as_array().unwrap();
        assert_eq!(w.len(), 2);
        assert_eq!(w[0]["position"], "Senior Software Engineer");
        assert_eq!(w[0]["name"], "Acme Corp");
        assert_eq!(w[0]["startDate"], "2020-01");
        assert!(w[0].get("endDate").is_none(), "Present must omit endDate");
        assert_eq!(w[0]["location"], "San Francisco, CA");
        assert_eq!(w[0]["highlights"].as_array().unwrap().len(), 2);
        assert_eq!(w[1]["name"], "Beta Labs");
        assert_eq!(w[1]["startDate"], "2016-06");
        assert_eq!(w[1]["endDate"], "2019-12");
    }

    #[test]
    fn extracts_education_skills_languages() {
        let v = extract_sample();
        let e = &v["education"][0];
        assert_eq!(e["institution"], "State University");
        assert_eq!(e["studyType"], "B.S.");
        assert_eq!(e["area"], "Computer Science");
        assert_eq!(e["startDate"], "2012");
        assert_eq!(e["endDate"], "2016");
        assert_eq!(e["score"], "3.8/4.0");
        let s = v["skills"].as_array().unwrap();
        assert_eq!(s[0]["name"], "Languages");
        assert_eq!(s[0]["keywords"], json!(["Rust", "Python", "TypeScript"]));
        let l = v["languages"].as_array().unwrap();
        assert_eq!(l[0], json!({ "language": "English", "fluency": "Native" }));
        assert_eq!(l[1], json!({ "language": "Spanish", "fluency": "Professional" }));
    }

    #[test]
    fn extracted_output_revalidates_clean() {
        let v = extract_sample();
        let report = validate(&v);
        assert_eq!(report["errors"], json!([]), "extractor must emit schema-valid JSON");
        assert_eq!(report["warnings"], json!([]));
        assert_eq!(report["valid"], true);
    }

    #[test]
    fn schema_ref_adds_schema_and_meta() {
        let out = run(SAMPLE, Mode::Extract, true, false).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["$schema"], SCHEMA_URL);
        assert_eq!(v["meta"]["version"], "v1.0.0");
        // And the first key really is $schema (canonical placement).
        let first = v.as_object().unwrap().keys().next().unwrap().clone();
        assert_eq!(first, "$schema");
    }

    #[test]
    fn auto_mode_validates_json_input() {
        let doc = r#"{ "basics": { "name": "Jane Doe" }, "work": [ { "name": "Acme", "startDate": "Jan 2020" } ], "hobbies": [] }"#;
        let out = run(doc, Mode::Auto, false, false).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["valid"], false);
        let errors = v["errors"].as_array().unwrap();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].as_str().unwrap().contains("work[0].startDate"), "{errors:?}");
        let warnings = v["warnings"].as_array().unwrap();
        assert!(warnings[0].as_str().unwrap().contains("hobbies"), "{warnings:?}");
        assert_eq!(v["summary"]["counts"]["work"], 1);
    }

    #[test]
    fn validate_flags_type_errors_and_email_warnings() {
        let doc = json!({
            "basics": { "name": 42, "email": "not-an-email" },
            "skills": [{ "keywords": "Rust" }]
        });
        let v = validate(&doc);
        let errors: Vec<String> =
            v["errors"].as_array().unwrap().iter().map(|e| e.as_str().unwrap().into()).collect();
        assert!(errors.iter().any(|e| e.contains("basics.name") && e.contains("string")));
        assert!(errors.iter().any(|e| e.contains("skills[0].keywords")));
        let warnings = v["warnings"].as_array().unwrap();
        assert!(warnings.iter().any(|w| w.as_str().unwrap().contains("basics.email")));
    }

    #[test]
    fn valid_document_passes() {
        let doc = json!({
            "basics": {
                "name": "Jane Doe",
                "email": "jane@example.com",
                "url": "https://example.com",
                "location": { "city": "Berlin", "countryCode": "DE" },
                "profiles": [{ "network": "GitHub", "username": "jane", "url": "https://github.com/jane" }]
            },
            "work": [{ "name": "Acme", "position": "Engineer", "startDate": "2020-01", "highlights": ["Shipped it"] }],
            "education": [{ "institution": "State University", "studyType": "Bachelor", "startDate": "2012", "endDate": "2016" }]
        });
        let v = validate(&doc);
        assert_eq!(v["valid"], true);
        assert_eq!(v["errors"], json!([]));
        assert_eq!(v["warnings"], json!([]));
        assert_eq!(v["summary"]["sections"], json!(["basics", "work", "education"]));
    }

    #[test]
    fn validate_mode_rejects_non_json() {
        let err = run("just some text", Mode::Validate, false, false).unwrap_err();
        assert!(err.contains("not valid JSON"), "{err}");
    }

    #[test]
    fn auto_mode_rejects_broken_json() {
        let err = run("{ \"basics\": ", Mode::Auto, false, false).unwrap_err();
        assert!(err.contains("looks like JSON"), "{err}");
    }

    #[test]
    fn empty_input_errors() {
        let err = run("  \n ", Mode::Auto, false, true).unwrap_err();
        assert!(err.contains("input is empty"), "{err}");
    }

    #[test]
    fn size_cap_boundary() {
        // Exactly at the cap: fine (a name line padded to 1 MiB).
        let mut at = String::from("Jane Doe\n");
        at.push_str(&"x".repeat(MAX_INPUT_BYTES - at.len()));
        assert_eq!(at.len(), MAX_INPUT_BYTES);
        assert!(run(&at, Mode::Extract, false, false).is_ok());
        // One byte over: rejected.
        let mut over = at;
        over.push('x');
        let err = run(&over, Mode::Extract, false, false).unwrap_err();
        assert!(err.contains("too large"), "{err}");
    }

    #[test]
    fn pretty_and_compact_shapes() {
        let compact = run(SAMPLE, Mode::Extract, false, false).unwrap();
        assert!(!compact.contains('\n'));
        let pretty = run(SAMPLE, Mode::Extract, false, true).unwrap();
        assert!(pretty.contains("\n  "));
    }

    #[test]
    fn date_token_forms() {
        assert_eq!(parse_date_token("Jan 2020").unwrap(), "2020-01");
        assert_eq!(parse_date_token("September 2019").unwrap(), "2019-09");
        assert_eq!(parse_date_token("Sept. 2019").unwrap(), "2019-09");
        assert_eq!(parse_date_token("May 15, 2020").unwrap(), "2020-05-15");
        assert_eq!(parse_date_token("03/2021").unwrap(), "2021-03");
        assert_eq!(parse_date_token("2020-07").unwrap(), "2020-07");
        assert_eq!(parse_date_token("2020").unwrap(), "2020");
        let (s, e, _) = find_date_range("2016 – 2019").unwrap();
        assert_eq!((s.as_str(), e.as_deref()), ("2016", Some("2019")));
        let (s, e, _) = find_date_range("Mar 2021 to Present").unwrap();
        assert_eq!((s.as_str(), e), ("2021-03", None));
        let (s, e, _) = find_date_range("2020-2023").unwrap();
        assert_eq!((s.as_str(), e.as_deref()), ("2020", Some("2023")));
    }

    #[test]
    fn no_content_errors() {
        let err = run("|||", Mode::Extract, false, false).unwrap_err();
        assert!(err.contains("no recognizable resume content"), "{err}");
    }
}

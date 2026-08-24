//! password-vault-audit core — pure compute, shared by the chat skill block and the web page.
//!
//! Audits a whole vault at once: paste a plain list of passwords or a password-manager export
//! (Bitwarden JSON, or any CSV whose header names a password column — Bitwarden, LastPass,
//! KeePass/KeePassXC, Chrome, 1Password, generic) and get every reused, duplicated, empty,
//! common, short, weak, stale and insecurely-stored credential in one report.
//!
//! Everything is derived from the text handed in — no I/O, no network, no randomness — so the
//! same export always produces the same report. The only ambient input is `now_unix`, supplied
//! by the caller so each surface brings its own clock and tests stay deterministic.
//!
//! Passwords are **fingerprinted, not echoed**, by default: a reuse group is identified by a
//! short non-reversible hash plus a length, which is enough to correlate entries without putting
//! plaintext into a report you might paste somewhere else.

use serde::Serialize;

/// Hard cap on entries per run, so a pasted mega-vault can't wedge a browser tab.
pub const MAX_ENTRIES: usize = 5000;

/// Minimum stem length before two passwords are considered "variants of each other".
const MIN_STEM_LEN: usize = 4;

// -------------------------------------------------------------------------------------------
// Options
// -------------------------------------------------------------------------------------------

/// How to read the pasted text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceFormat {
    /// Sniff the shape from the bytes.
    Auto,
    /// One password per line, nothing else.
    List,
    /// Delimited export with a header row naming a password column.
    Csv,
    /// Bitwarden's `.json` export (an object with `items`, or a bare array of items).
    BitwardenJson,
}

impl SourceFormat {
    pub fn parse(s: &str) -> Result<SourceFormat, String> {
        match s.trim().to_ascii_lowercase().replace('_', "-").as_str() {
            "" | "auto" => Ok(SourceFormat::Auto),
            "list" | "plain" | "lines" => Ok(SourceFormat::List),
            "csv" => Ok(SourceFormat::Csv),
            "bitwarden-json" | "json" | "bitwarden" => Ok(SourceFormat::BitwardenJson),
            other => Err(format!(
                "unknown format \u{201c}{other}\u{201d} — expected one of: auto, list, csv, \
                 bitwarden-json"
            )),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SourceFormat::Auto => "auto",
            SourceFormat::List => "list",
            SourceFormat::Csv => "csv",
            SourceFormat::BitwardenJson => "bitwarden-json",
        }
    }
}

/// How to render the audit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputForm {
    Report,
    Json,
    Csv,
}

impl OutputForm {
    pub fn parse(s: &str) -> Result<OutputForm, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "report" => Ok(OutputForm::Report),
            "json" => Ok(OutputForm::Json),
            "csv" => Ok(OutputForm::Csv),
            other => Err(format!(
                "unknown output \u{201c}{other}\u{201d} — expected one of: report, json, csv"
            )),
        }
    }
}

/// Every knob the audit exposes. `Default` matches the descriptor defaults.
#[derive(Clone, Debug)]
pub struct Options {
    pub format: SourceFormat,
    /// Passwords shorter than this are flagged.
    pub min_length: usize,
    /// Passwords scoring below this (0–100) are flagged as weak.
    pub min_score: u32,
    /// Flag passwords last changed more than this many days ago. 0 disables the check.
    pub max_age_days: u32,
    pub check_common: bool,
    pub check_reuse: bool,
    pub check_similar: bool,
    pub check_insecure_urls: bool,
    pub check_missing_2fa: bool,
    /// Replace every password in the output with a short fingerprint + length.
    pub mask_passwords: bool,
    pub output: OutputForm,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            format: SourceFormat::Auto,
            min_length: 12,
            min_score: 40,
            max_age_days: 365,
            check_common: true,
            check_reuse: true,
            check_similar: true,
            check_insecure_urls: true,
            check_missing_2fa: false,
            mask_passwords: true,
            output: OutputForm::Report,
        }
    }
}

// -------------------------------------------------------------------------------------------
// Parsed data
// -------------------------------------------------------------------------------------------

/// One vault entry, in the shape every supported input can be projected onto.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Entry {
    /// Item title, or a synthetic `row 4` / `line 4` when the input has no name column.
    pub name: String,
    pub username: String,
    pub password: String,
    /// All URIs for the item, already joined with `, ` when there is more than one.
    pub url: String,
    /// TOTP / authenticator secret, when the export carries one.
    pub totp: String,
    /// Last-modified time as a Unix timestamp in seconds, when the export carries one.
    pub revision: Option<i64>,
}

/// A single audit finding.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Finding {
    /// `error` or `warning`.
    pub severity: &'static str,
    /// Stable machine-readable rule slug (`reused-password`, `weak-password`, …).
    pub rule: &'static str,
    /// Which entry (or entries) the finding is about.
    pub entry: String,
    /// What is wrong and what was expected.
    pub detail: String,
    /// Input order of the (first) entry involved — report ordering only.
    #[serde(skip)]
    order: usize,
}

/// Count of entries per strength band.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Bands {
    pub strong: usize,
    pub medium: usize,
    pub fair: usize,
    pub weak: usize,
}

/// The structured audit result (what `output = "json"` serializes).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Report {
    /// True when there are zero `error`-severity findings.
    pub ok: bool,
    /// Which reader actually parsed the input.
    pub format: &'static str,
    /// Entries parsed.
    pub entries: usize,
    /// Entries that carry a non-empty password.
    pub with_password: usize,
    /// Distinct passwords among those entries.
    pub unique_passwords: usize,
    /// Vault health, 0–100.
    pub vault_score: u32,
    /// Band for `vault_score`: weak / fair / medium / strong.
    pub vault_band: &'static str,
    pub error_count: usize,
    pub warning_count: usize,
    pub strength: Bands,
    pub findings: Vec<Finding>,
}

// -------------------------------------------------------------------------------------------
// Entry point
// -------------------------------------------------------------------------------------------

/// Audit `data` and render it in `opts.output`.
///
/// `now_unix` is "now" in seconds since the Unix epoch; it is used only by the stale-password
/// check, so passing `0.0` with `max_age_days = 0` is a fully deterministic run.
pub fn audit(data: &str, opts: &Options, now_unix: f64) -> Result<String, String> {
    let report = analyze(data, opts, now_unix)?;
    match opts.output {
        OutputForm::Report => Ok(render_report(&report)),
        OutputForm::Json => {
            serde_json::to_string_pretty(&report).map_err(|e| format!("could not encode JSON: {e}"))
        }
        OutputForm::Csv => render_csv(&report),
    }
}

/// Audit `data` and return the structured result.
pub fn analyze(data: &str, opts: &Options, now_unix: f64) -> Result<Report, String> {
    let data = data.trim_start_matches('\u{feff}');
    if data.trim().is_empty() {
        return Err(
            "paste a password list or a password-manager export first — this box is empty".into(),
        );
    }

    let source = match opts.format {
        SourceFormat::Auto => detect(data),
        explicit => explicit,
    };
    let entries = match source {
        SourceFormat::BitwardenJson => read_bitwarden_json(data)?,
        SourceFormat::Csv => read_csv(data)?,
        _ => read_list(data),
    };

    if entries.len() > MAX_ENTRIES {
        return Err(format!(
            "up to {MAX_ENTRIES} entries per run, this input has {}; split it and audit in batches",
            entries.len()
        ));
    }
    if entries.is_empty() {
        return Err(format!(
            "no entries found — the input was read as {} but produced no rows",
            source.label()
        ));
    }

    Ok(build_report(&entries, source, opts, now_unix))
}

/// Sniff the input shape: JSON by its opening bracket, CSV by a header row that names a password
/// column, everything else as a plain one-per-line list.
pub fn detect(data: &str) -> SourceFormat {
    let trimmed = data.trim_start();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return SourceFormat::BitwardenJson;
    }
    // A single-column file is read as a list even when its first line says "password": the two
    // readings are otherwise identical, and the list reader does not swallow that first line.
    match csv_headers(data) {
        Ok(headers) if headers.len() >= 2 && password_column(&headers).is_some() => {
            SourceFormat::Csv
        }
        _ => SourceFormat::List,
    }
}

// -------------------------------------------------------------------------------------------
// Readers
// -------------------------------------------------------------------------------------------

/// Normalise a header cell: lowercase, letters and digits only, so `Login Name`, `login_name`
/// and `LOGINNAME` are the same column.
fn norm(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

fn csv_headers(data: &str) -> Result<Vec<String>, String> {
    let mut rdr = csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(data.as_bytes());
    let headers = rdr
        .headers()
        .map_err(|e| format!("could not read the CSV header row: {e}"))?;
    Ok(headers.iter().map(|h| h.to_string()).collect())
}

/// Column-name synonyms, normalised. First match in the header row wins.
const PASSWORD_COLS: &[&str] = &["password", "loginpassword", "pass", "pwd", "secret"];
const USERNAME_COLS: &[&str] = &[
    "username",
    "loginusername",
    "user",
    "loginname",
    "email",
    "login",
    "account",
    "usernamevalue",
];
const NAME_COLS: &[&str] = &["name", "title", "item", "site", "displayname", "accountname"];
const URL_COLS: &[&str] = &[
    "url", "uri", "loginuri", "website", "link", "originurl", "hostname", "webaddress",
];
const TOTP_COLS: &[&str] = &["totp", "logintotp", "otpauth", "otpsecret", "authenticatorkey"];
const DATE_COLS: &[&str] = &[
    "revisiondate",
    "lastmodified",
    "datemodified",
    "modified",
    "passwordmodified",
    "passwordchanged",
    "updated",
    "lastchanged",
    "datepasswordchanged",
];

fn find_column(headers: &[String], candidates: &[&str]) -> Option<usize> {
    let normed: Vec<String> = headers.iter().map(|h| norm(h)).collect();
    candidates
        .iter()
        .find_map(|c| normed.iter().position(|h| h == c))
}

fn password_column(headers: &[String]) -> Option<usize> {
    find_column(headers, PASSWORD_COLS)
}

fn read_csv(data: &str) -> Result<Vec<Entry>, String> {
    let headers = csv_headers(data)?;
    let pw_col = password_column(&headers).ok_or_else(|| {
        format!(
            "no password column in the CSV header — expected one named {}, got: {}",
            PASSWORD_COLS.join(" / "),
            if headers.is_empty() {
                "(no header row)".to_string()
            } else {
                headers.join(", ")
            }
        )
    })?;
    let user_col = find_column(&headers, USERNAME_COLS);
    let name_col = find_column(&headers, NAME_COLS);
    let url_col = find_column(&headers, URL_COLS);
    let totp_col = find_column(&headers, TOTP_COLS);
    let date_col = find_column(&headers, DATE_COLS);

    let mut rdr = csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(data.as_bytes());
    let cell = |rec: &csv::StringRecord, idx: Option<usize>| -> String {
        idx.and_then(|i| rec.get(i)).unwrap_or("").trim().to_string()
    };

    let mut out = Vec::new();
    for (i, rec) in rdr.records().enumerate() {
        let rec = rec.map_err(|e| format!("CSV row {} could not be read: {e}", i + 2))?;
        if rec.iter().all(|c| c.trim().is_empty()) {
            continue;
        }
        let name = cell(&rec, name_col);
        out.push(Entry {
            name: if name.is_empty() {
                format!("row {}", i + 2)
            } else {
                name
            },
            username: cell(&rec, user_col),
            // A password may legitimately start or end with a space, so this one is not trimmed.
            password: rec.get(pw_col).unwrap_or("").to_string(),
            url: cell(&rec, url_col),
            totp: cell(&rec, totp_col),
            revision: parse_timestamp(&cell(&rec, date_col)),
        });
    }
    Ok(out)
}

fn read_list(data: &str) -> Vec<Entry> {
    data.lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
        .map(|(i, l)| Entry {
            name: format!("line {}", i + 1),
            password: l.trim_end_matches(['\r', '\n']).to_string(),
            ..Entry::default()
        })
        .collect()
}

fn read_bitwarden_json(data: &str) -> Result<Vec<Entry>, String> {
    let doc: serde_json::Value = serde_json::from_str(data)
        .map_err(|e| format!("this does not parse as JSON: {e} — expected a Bitwarden export"))?;
    let items = match &doc {
        serde_json::Value::Array(a) => a.clone(),
        serde_json::Value::Object(o) => match o.get("items") {
            Some(serde_json::Value::Array(a)) => a.clone(),
            _ => {
                return Err(
                    "JSON parsed but has no \"items\" array — expected a Bitwarden export object \
                     like {\"items\": [ … ]} or a bare array of items"
                        .into(),
                )
            }
        },
        _ => return Err("expected a JSON object or array of vault items".into()),
    };

    let s = |v: Option<&serde_json::Value>| -> String {
        v.and_then(|v| v.as_str()).unwrap_or("").trim().to_string()
    };

    let mut out = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let login = item.get("login");
        let uris = login
            .and_then(|l| l.get("uris"))
            .and_then(|u| u.as_array())
            .map(|a| {
                a.iter()
                    .map(|u| s(u.get("uri")))
                    .filter(|u| !u.is_empty())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        let name = s(item.get("name"));
        out.push(Entry {
            name: if name.is_empty() {
                format!("item {}", i + 1)
            } else {
                name
            },
            username: s(login.and_then(|l| l.get("username"))),
            password: login
                .and_then(|l| l.get("password"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            url: uris,
            totp: s(login.and_then(|l| l.get("totp"))),
            revision: parse_timestamp(&s(item.get("revisionDate"))),
        });
    }
    Ok(out)
}

// -------------------------------------------------------------------------------------------
// Timestamps
// -------------------------------------------------------------------------------------------

/// Days from 1970-01-01 for a civil date (Howard Hinnant's `days_from_civil`).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Parse the date forms password managers actually export: ISO-8601
/// (`2023-04-01T09:15:00.000Z`, `2023-04-01 09:15:00`, `2023-04-01`), the same with `/`
/// separators, and bare Unix epoch seconds or milliseconds. Returns Unix seconds.
fn parse_timestamp(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if s.chars().all(|c| c.is_ascii_digit()) {
        let n: i64 = s.parse().ok()?;
        // 13+ digits is milliseconds (any second-precision stamp this century is 10 digits).
        return Some(if s.len() >= 12 { n / 1000 } else { n });
    }

    let bytes = s.as_bytes();
    if bytes.len() < 10 {
        return None;
    }
    let num = |a: usize, b: usize| -> Option<i64> { s.get(a..b)?.parse::<i64>().ok() };
    let sep = bytes[4] as char;
    if (sep != '-' && sep != '/') || bytes[7] as char != sep {
        return None;
    }
    let (y, m, d) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let mut secs = days_from_civil(y, m, d) * 86_400;
    if bytes.len() >= 19 {
        let t = bytes[10] as char;
        if (t == 'T' || t == ' ') && bytes[13] as char == ':' && bytes[16] as char == ':' {
            let (hh, mm, ss) = (num(11, 13)?, num(14, 16)?, num(17, 19)?);
            if hh < 24 && mm < 60 && ss < 62 {
                secs += hh * 3600 + mm * 60 + ss;
            }
        }
    }
    Some(secs)
}

// -------------------------------------------------------------------------------------------
// Strength
// -------------------------------------------------------------------------------------------

/// A single password's strength estimate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Strength {
    /// 0–100 health score (the number the bands and the vault score are built from).
    pub score: u32,
    /// Shannon-style entropy estimate: length × log2(character-pool size).
    pub bits: f64,
    /// Distinct character classes used (lower, upper, digit, symbol, non-ASCII).
    pub classes: u32,
    pub band: &'static str,
}

/// Industry-standard four-band split used by vault health reports.
pub fn band_for(score: u32) -> &'static str {
    match score {
        0..=39 => "weak",
        40..=59 => "fair",
        60..=79 => "medium",
        _ => "strong",
    }
}

/// True when the whole password is one repeated character, or one unbroken ascending or
/// descending run (`aaaaaa`, `123456`, `abcdef`, `987654`).
fn is_trivial_run(pw: &str) -> bool {
    let ch: Vec<char> = pw.chars().collect();
    if ch.len() < 3 {
        return true;
    }
    let deltas: Vec<i32> = ch
        .windows(2)
        .map(|w| w[1] as i32 - w[0] as i32)
        .collect::<Vec<_>>();
    let first = deltas[0];
    matches!(first, -1 | 0 | 1) && deltas.iter().all(|d| *d == first)
}

/// Score a password 0–100. `username` (when known) and `common_rank` (its position on the
/// bundled common-password list, when it is on it) sharpen the estimate.
pub fn score_password(pw: &str, username: &str, common_rank: Option<usize>) -> Strength {
    let len = pw.chars().count();
    if len == 0 {
        return Strength {
            score: 0,
            bits: 0.0,
            classes: 0,
            band: "weak",
        };
    }

    let (mut lower, mut upper, mut digit, mut symbol, mut other) = (false, false, false, false, false);
    for c in pw.chars() {
        if !c.is_ascii() {
            other = true;
        } else if c.is_ascii_lowercase() {
            lower = true;
        } else if c.is_ascii_uppercase() {
            upper = true;
        } else if c.is_ascii_digit() {
            digit = true;
        } else {
            symbol = true;
        }
    }
    let classes =
        [lower, upper, digit, symbol, other].iter().filter(|b| **b).count() as u32;
    let pool = (if lower { 26 } else { 0 })
        + (if upper { 26 } else { 0 })
        + (if digit { 10 } else { 0 })
        + (if symbol { 33 } else { 0 })
        + (if other { 100 } else { 0 });
    let bits = len as f64 * (pool.max(2) as f64).log2();

    // 80 bits of estimated entropy is a full score; scale linearly below that.
    let mut score = (bits * 1.25).min(100.0);
    if classes == 1 {
        score *= 0.6;
    }
    if digit && classes == 1 {
        score *= 0.6;
    }
    if len < 8 {
        score = score.min(25.0);
    }
    if is_trivial_run(pw) {
        score = score.min(8.0);
    }
    let u = username.trim().to_lowercase();
    let stem = u.split('@').next().unwrap_or("").to_string();
    if stem.chars().count() >= 3 && pw.to_lowercase().contains(&stem) {
        score = score.min(15.0);
    }
    if let Some(rank) = common_rank {
        // The best-known passwords score essentially zero regardless of shape.
        score = score.min(if rank <= 100 { 1.0 } else { 5.0 });
    }

    let score = score.round().clamp(0.0, 100.0) as u32;
    Strength {
        score,
        bits,
        classes,
        band: band_for(score),
    }
}

// -------------------------------------------------------------------------------------------
// Masking
// -------------------------------------------------------------------------------------------

/// FNV-1a/32 folded to 16 bits — a short, stable, non-reversible correlation handle. It is a
/// display aid for grouping, deliberately NOT a password hash.
fn fingerprint(pw: &str) -> String {
    let mut h: u32 = 0x811c_9dc5;
    for b in pw.as_bytes() {
        h ^= *b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    format!("{:04x}", (h ^ (h >> 16)) as u16)
}

fn label_password(pw: &str, mask: bool) -> String {
    if pw.is_empty() {
        return "(empty)".to_string();
    }
    let n = pw.chars().count();
    if mask {
        format!("#{} ({} chars)", fingerprint(pw), n)
    } else {
        format!("\u{201c}{pw}\u{201d} ({n} chars)")
    }
}

// -------------------------------------------------------------------------------------------
// The audit
// -------------------------------------------------------------------------------------------

fn severity_of(rule: &str) -> &'static str {
    match rule {
        "reused-password" | "common-password" | "empty-password" | "weak-password"
        | "password-contains-username" => "error",
        _ => "warning",
    }
}

/// Report ordering: most actionable rule first, not alphabetical.
fn rule_order(rule: &str) -> u8 {
    match rule {
        "reused-password" => 0,
        "common-password" => 1,
        "empty-password" => 2,
        "weak-password" => 3,
        "password-contains-username" => 4,
        "short-password" => 5,
        "duplicate-entry" => 6,
        "similar-password" => 7,
        "low-character-variety" => 8,
        "stale-password" => 9,
        "insecure-url" => 10,
        _ => 11,
    }
}

fn finding(rule: &'static str, entry: String, detail: String, order: usize) -> Finding {
    Finding {
        severity: severity_of(rule),
        rule,
        entry,
        detail,
        order,
    }
}

/// Strip a trailing counter/suffix so `Summer2024!` and `Summer2025?` share a stem.
fn stem_of(pw: &str) -> String {
    pw.trim_end_matches(|c: char| c.is_ascii_digit() || c.is_ascii_punctuation())
        .to_lowercase()
}

/// Join up to 4 names, then "+N more", so a 300-entry reuse group stays one readable line.
fn join_names(names: &[String]) -> String {
    if names.len() <= 4 {
        return names.join(", ");
    }
    format!("{}, +{} more", names[..4].join(", "), names.len() - 4)
}

fn build_report(entries: &[Entry], source: SourceFormat, opts: &Options, now_unix: f64) -> Report {
    let mut findings: Vec<Finding> = Vec::new();
    let mut bands = Bands::default();
    let mut score_total: u64 = 0;
    let mut with_password = 0usize;

    // ---- per-entry checks -------------------------------------------------------------
    for (i, e) in entries.iter().enumerate() {
        if e.password.is_empty() {
            findings.push(finding(
                "empty-password",
                e.name.clone(),
                "no password stored on this entry".to_string(),
                i,
            ));
            continue;
        }
        with_password += 1;

        let common = if opts.check_common {
            gizza_ai_weak_password_detector_core::detect(&e.password, false, true)
                .ok()
                .and_then(|d| d.rank)
        } else {
            None
        };
        let st = score_password(&e.password, &e.username, common);
        score_total += st.score as u64;
        match st.band {
            "strong" => bands.strong += 1,
            "medium" => bands.medium += 1,
            "fair" => bands.fair += 1,
            _ => bands.weak += 1,
        }

        let shown = label_password(&e.password, opts.mask_passwords);
        if let Some(rank) = common {
            findings.push(finding(
                "common-password",
                e.name.clone(),
                format!(
                    "{shown} is #{rank} on the bundled common-password list — attackers try these \
                     first"
                ),
                i,
            ));
        }
        if st.score < opts.min_score {
            findings.push(finding(
                "weak-password",
                e.name.clone(),
                format!(
                    "{shown} scores {}/100 ({}), below the {} minimum — about {:.0} bits of \
                     estimated entropy across {} character class{}",
                    st.score,
                    st.band,
                    opts.min_score,
                    st.bits,
                    st.classes,
                    if st.classes == 1 { "" } else { "es" }
                ),
                i,
            ));
        }
        let len = e.password.chars().count();
        if len < opts.min_length {
            findings.push(finding(
                "short-password",
                e.name.clone(),
                format!("{len} characters, minimum {}", opts.min_length),
                i,
            ));
        }
        let user_stem = e
            .username
            .trim()
            .to_lowercase()
            .split('@')
            .next()
            .unwrap_or("")
            .to_string();
        if user_stem.chars().count() >= 3 && e.password.to_lowercase().contains(&user_stem) {
            findings.push(finding(
                "password-contains-username",
                e.name.clone(),
                format!(
                    "the password contains the username \u{201c}{user_stem}\u{201d}, which is the \
                     first thing an attacker tries"
                ),
                i,
            ));
        }
        if st.classes == 1 && len >= opts.min_length {
            findings.push(finding(
                "low-character-variety",
                e.name.clone(),
                "uses a single character class — mix upper case, lower case, digits and symbols"
                    .to_string(),
                i,
            ));
        }
        if opts.check_insecure_urls {
            for u in e.url.split(',').map(str::trim).filter(|u| !u.is_empty()) {
                if u.len() >= 7 && u[..7].eq_ignore_ascii_case("http://") {
                    findings.push(finding(
                        "insecure-url",
                        e.name.clone(),
                        format!("{u} is unencrypted HTTP — the password travels in clear text"),
                        i,
                    ));
                }
            }
        }
        if opts.check_missing_2fa && e.totp.is_empty() && !e.url.is_empty() {
            findings.push(finding(
                "missing-2fa",
                e.name.clone(),
                "no authenticator (TOTP) secret stored for this login".to_string(),
                i,
            ));
        }
        if opts.max_age_days > 0 {
            if let Some(rev) = e.revision {
                let age = (now_unix - rev as f64) / 86_400.0;
                if age > opts.max_age_days as f64 {
                    findings.push(finding(
                        "stale-password",
                        e.name.clone(),
                        format!(
                            "last changed about {:.0} days ago, older than the {}-day maximum",
                            age, opts.max_age_days
                        ),
                        i,
                    ));
                }
            }
        }
    }

    // ---- cross-entry checks -----------------------------------------------------------
    let mut groups: Vec<(String, Vec<usize>)> = Vec::new();
    for (i, e) in entries.iter().enumerate() {
        if e.password.is_empty() {
            continue;
        }
        match groups.iter_mut().find(|(p, _)| *p == e.password) {
            Some((_, v)) => v.push(i),
            None => groups.push((e.password.clone(), vec![i])),
        }
    }
    let unique_passwords = groups.len();
    let mut reused_entries = 0usize;

    for (pw, idxs) in &groups {
        if idxs.len() < 2 {
            continue;
        }
        reused_entries += idxs.len();
        let names: Vec<String> = idxs.iter().map(|i| entries[*i].name.clone()).collect();
        if opts.check_reuse {
            findings.push(finding(
                "reused-password",
                join_names(&names),
                format!(
                    "{} entries share one password {} — one breach exposes all of them",
                    idxs.len(),
                    label_password(pw, opts.mask_passwords)
                ),
                idxs[0],
            ));
        }
        // Exact duplicate items: same title AND username AND password.
        let mut dup: Vec<(String, Vec<usize>)> = Vec::new();
        for i in idxs {
            let key = format!(
                "{}\u{1}{}",
                entries[*i].name.to_lowercase(),
                entries[*i].username.to_lowercase()
            );
            match dup.iter_mut().find(|(k, _)| *k == key) {
                Some((_, v)) => v.push(*i),
                None => dup.push((key, vec![*i])),
            }
        }
        for (_, same) in dup.iter().filter(|(_, v)| v.len() > 1) {
            findings.push(finding(
                "duplicate-entry",
                entries[same[0]].name.clone(),
                format!(
                    "{} identical items (same name, username and password) — delete the extras",
                    same.len()
                ),
                same[0],
            ));
        }
    }

    if opts.check_similar {
        let mut stems: Vec<(String, Vec<usize>)> = Vec::new();
        for (i, e) in entries.iter().enumerate() {
            if e.password.is_empty() {
                continue;
            }
            let s = stem_of(&e.password);
            if s.chars().count() < MIN_STEM_LEN {
                continue;
            }
            match stems.iter_mut().find(|(k, _)| *k == s) {
                Some((_, v)) => v.push(i),
                None => stems.push((s, vec![i])),
            }
        }
        for (s, idxs) in &stems {
            let distinct: Vec<&String> = {
                let mut v: Vec<&String> = idxs.iter().map(|i| &entries[*i].password).collect();
                v.sort();
                v.dedup();
                v
            };
            if distinct.len() < 2 {
                continue; // exact reuse, already reported above
            }
            let names: Vec<String> = idxs.iter().map(|i| entries[*i].name.clone()).collect();
            findings.push(finding(
                "similar-password",
                join_names(&names),
                format!(
                    "{} entries use variants of the same base \u{201c}{}\u{201d} — a counter on \
                     the end does not make a new password",
                    distinct.len(),
                    if opts.mask_passwords {
                        format!("#{}\u{2026}", fingerprint(s))
                    } else {
                        s.clone()
                    }
                ),
                idxs[0],
            ));
        }
    }

    findings.sort_by(|a, b| {
        (a.severity, rule_order(a.rule), a.order, &a.entry).cmp(&(
            b.severity,
            rule_order(b.rule),
            b.order,
            &b.entry,
        ))
    });

    let error_count = findings.iter().filter(|f| f.severity == "error").count();
    let warning_count = findings.len() - error_count;

    // Vault score: mean entry score, discounted by how much of the vault is reused.
    let mean = if with_password == 0 {
        0.0
    } else {
        score_total as f64 / with_password as f64
    };
    let reuse_fraction = if with_password == 0 {
        0.0
    } else {
        reused_entries as f64 / with_password as f64
    };
    let vault_score = (mean * (1.0 - 0.5 * reuse_fraction)).round().clamp(0.0, 100.0) as u32;

    Report {
        ok: error_count == 0,
        format: source.label(),
        entries: entries.len(),
        with_password,
        unique_passwords,
        vault_score,
        vault_band: band_for(vault_score),
        error_count,
        warning_count,
        strength: bands,
        findings,
    }
}

// -------------------------------------------------------------------------------------------
// Rendering
// -------------------------------------------------------------------------------------------

fn render_report(r: &Report) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Vault audit \u{2014} {} entries read as {}, {} with a password, {} unique\n",
        r.entries, r.format, r.with_password, r.unique_passwords
    ));
    out.push_str(&format!(
        "Vault score: {}/100 ({})\n",
        r.vault_score, r.vault_band
    ));
    out.push_str(&format!(
        "Strength: {} strong, {} medium, {} fair, {} weak\n",
        r.strength.strong, r.strength.medium, r.strength.fair, r.strength.weak
    ));
    out.push_str(&format!(
        "Findings: {} errors, {} warnings\n",
        r.error_count, r.warning_count
    ));

    if r.findings.is_empty() {
        out.push_str("\nNo issues found \u{2014} every entry is unique and passes each enabled check.\n");
        return out;
    }
    for (sev, title) in [("error", "ERRORS"), ("warning", "WARNINGS")] {
        let group: Vec<&Finding> = r.findings.iter().filter(|f| f.severity == sev).collect();
        if group.is_empty() {
            continue;
        }
        out.push_str(&format!("\n{title}\n"));
        for f in group {
            out.push_str(&format!("  [{}] {} \u{2014} {}\n", f.rule, f.entry, f.detail));
        }
    }
    out
}

fn render_csv(r: &Report) -> Result<String, String> {
    let mut w = csv::WriterBuilder::new()
        .quote_style(csv::QuoteStyle::Necessary)
        .from_writer(Vec::new());
    w.write_record(["severity", "rule", "entry", "detail"])
        .map_err(|e| format!("could not write CSV: {e}"))?;
    for f in &r.findings {
        w.write_record([f.severity, f.rule, &f.entry, &f.detail])
            .map_err(|e| format!("could not write CSV: {e}"))?;
    }
    let bytes = w
        .into_inner()
        .map_err(|e| format!("could not finish the CSV: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("CSV was not valid UTF-8: {e}"))
}

// -------------------------------------------------------------------------------------------
// Tests
// -------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options::default()
    }

    #[test]
    fn plain_list_flags_reuse_and_common_passwords() {
        let r = analyze("hunter2\nhunter2\nCorrect-Horse-Battery-9!\n", &opts(), 0.0).unwrap();
        assert_eq!(r.entries, 3);
        assert_eq!(r.with_password, 3);
        assert_eq!(r.unique_passwords, 2);
        assert!(!r.ok);
        assert!(r.findings.iter().any(|f| f.rule == "reused-password"));
        assert_eq!(r.format, "list");
        // The strong passphrase is not flagged as weak.
        assert!(!r
            .findings
            .iter()
            .any(|f| f.rule == "weak-password" && f.entry == "line 3"));
    }

    #[test]
    fn masked_output_never_echoes_the_password() {
        let out = audit("letmein\nletmein\n", &opts(), 0.0).unwrap();
        assert!(!out.contains("letmein"), "masked report leaked a password: {out}");
        assert!(out.contains("reused-password"));
    }

    #[test]
    fn unmasked_output_shows_the_password() {
        let o = Options {
            mask_passwords: false,
            ..opts()
        };
        let out = audit("letmein\nletmein\n", &o, 0.0).unwrap();
        assert!(out.contains("letmein"));
    }

    #[test]
    fn detects_bitwarden_json_and_its_login_fields() {
        let data = r#"{"items":[
            {"name":"Router","login":{"username":"admin","password":"admin",
             "uris":[{"uri":"http://192.168.1.1"}]},"revisionDate":"2019-01-02T03:04:05.000Z"},
            {"name":"Mail","login":{"username":"ada@example.com","password":"T7#vq!Lm2zRp8w"}}
        ]}"#;
        let o = Options {
            check_missing_2fa: true,
            ..opts()
        };
        let r = analyze(data, &o, 1_700_000_000.0).unwrap();
        assert_eq!(r.format, "bitwarden-json");
        assert_eq!(r.entries, 2);
        let rules: Vec<&str> = r.findings.iter().map(|f| f.rule).collect();
        assert!(rules.contains(&"common-password"));
        assert!(rules.contains(&"insecure-url"));
        assert!(rules.contains(&"missing-2fa"));
        assert!(rules.contains(&"stale-password"));
        assert!(rules.contains(&"password-contains-username"));
    }

    #[test]
    fn csv_header_synonyms_map_onto_entries() {
        let data = "name,login_username,login_password,login_uri,login_totp\n\
                    GitHub,ada,S3cret-Passphrase-42!,https://github.com,\n\
                    GitLab,ada,S3cret-Passphrase-42!,https://gitlab.com,JBSWY3DPEHPK3PXP\n";
        let r = analyze(data, &opts(), 0.0).unwrap();
        assert_eq!(r.format, "csv");
        assert_eq!(r.entries, 2);
        assert_eq!(r.unique_passwords, 1);
        let reuse = r
            .findings
            .iter()
            .find(|f| f.rule == "reused-password")
            .expect("reuse group");
        assert_eq!(reuse.entry, "GitHub, GitLab");
    }

    #[test]
    fn similar_passwords_are_grouped_by_stem() {
        let data = "Summer2023!\nSummer2024!\nSummer2025!\n";
        let r = analyze(data, &opts(), 0.0).unwrap();
        let f = r
            .findings
            .iter()
            .find(|f| f.rule == "similar-password")
            .expect("similar group");
        assert_eq!(f.entry, "line 1, line 2, line 3");
        assert!(!f.detail.contains("summer"), "stem leaked while masked");
    }

    #[test]
    fn duplicate_items_are_reported_separately_from_reuse() {
        let data = "name,username,password\n\
                    Mail,ada,T7#vq!Lm2zRp8w\n\
                    Mail,ada,T7#vq!Lm2zRp8w\n";
        let r = analyze(data, &opts(), 0.0).unwrap();
        let rules: Vec<&str> = r.findings.iter().map(|f| f.rule).collect();
        assert!(rules.contains(&"duplicate-entry"));
        assert!(rules.contains(&"reused-password"));
    }

    #[test]
    fn empty_passwords_are_flagged_and_excluded_from_scoring() {
        let data = "name,username,password\nOld note,,\nMail,ada,T7#vq!Lm2zRp8w\n";
        let r = analyze(data, &opts(), 0.0).unwrap();
        assert_eq!(r.entries, 2);
        assert_eq!(r.with_password, 1);
        assert!(r.findings.iter().any(|f| f.rule == "empty-password"));
    }

    #[test]
    fn checks_can_be_switched_off() {
        let o = Options {
            check_reuse: false,
            check_similar: false,
            check_common: false,
            min_score: 0,
            min_length: 1,
            ..opts()
        };
        let r = analyze("aB3!xY9@zQ1#\naB3!xY9@zQ1#\n", &o, 0.0).unwrap();
        assert!(r.findings.is_empty(), "unexpected findings: {:?}", r.findings);
        assert!(r.ok);
        // …and the same input with the checks on does report the reuse.
        assert!(!analyze("aB3!xY9@zQ1#\naB3!xY9@zQ1#\n", &opts(), 0.0)
            .unwrap()
            .findings
            .is_empty());
    }

    #[test]
    fn json_output_is_structured_and_parses() {
        let o = Options {
            output: OutputForm::Json,
            ..opts()
        };
        let out = audit("hunter2\nhunter2\n", &o, 0.0).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["entries"], 2);
        assert_eq!(v["unique_passwords"], 1);
        assert_eq!(v["ok"], false);
        assert!(v["findings"].as_array().unwrap().iter().any(|f| f["rule"]
            == "reused-password"));
    }

    #[test]
    fn csv_output_has_a_header_and_one_row_per_finding() {
        let o = Options {
            output: OutputForm::Csv,
            ..opts()
        };
        let out = audit("hunter2\nhunter2\n", &o, 0.0).unwrap();
        assert!(out.starts_with("severity,rule,entry,detail\n"));
        // 1 reuse group + weak + short for each of the two entries.
        assert_eq!(out.lines().count() - 1, 5);
    }

    #[test]
    fn scoring_bands_match_the_documented_thresholds() {
        assert_eq!(band_for(0), "weak");
        assert_eq!(band_for(39), "weak");
        assert_eq!(band_for(40), "fair");
        assert_eq!(band_for(59), "fair");
        assert_eq!(band_for(60), "medium");
        assert_eq!(band_for(79), "medium");
        assert_eq!(band_for(80), "strong");
        assert_eq!(score_password("123456", "", None).band, "weak");
        assert_eq!(score_password("Tr0ub4dor&3xKcd-Horse!", "", None).band, "strong");
        assert!(score_password("aaaaaaaaaaaaaaaa", "", None).score <= 8);
        assert!(score_password("ada-lovelace-1815", "ada@example.com", None).score <= 15);
    }

    #[test]
    fn timestamps_parse_in_every_exported_form() {
        assert_eq!(parse_timestamp("1970-01-02"), Some(86_400));
        assert_eq!(parse_timestamp("1970-01-02T00:00:01"), Some(86_401));
        assert_eq!(parse_timestamp("1970/01/02 00:00:01Z"), Some(86_401));
        assert_eq!(parse_timestamp("86400"), Some(86_400));
        // 10-digit stamps are seconds, 13-digit stamps are milliseconds.
        assert_eq!(parse_timestamp("1700000000"), Some(1_700_000_000));
        assert_eq!(parse_timestamp("1700000000000"), Some(1_700_000_000));
        assert_eq!(parse_timestamp(""), None);
        assert_eq!(parse_timestamp("never"), None);
        assert_eq!(parse_timestamp("2023-13-01"), None);
    }

    #[test]
    fn empty_input_is_an_actionable_error() {
        let e = analyze("   \n\t\n", &opts(), 0.0).unwrap_err();
        assert!(e.contains("empty"), "{e}");
    }

    #[test]
    fn csv_without_a_password_column_says_what_was_expected() {
        let e = analyze("name,url\nGitHub,https://github.com\n", &Options {
            format: SourceFormat::Csv,
            ..opts()
        }, 0.0)
        .unwrap_err();
        assert!(e.contains("no password column"), "{e}");
        assert!(e.contains("name, url"), "{e}");
    }

    #[test]
    fn malformed_json_is_an_actionable_error() {
        let e = analyze("{\"items\": [", &opts(), 0.0).unwrap_err();
        assert!(e.contains("does not parse as JSON"), "{e}");
        let e = analyze("{\"vault\": []}", &opts(), 0.0).unwrap_err();
        assert!(e.contains("items"), "{e}");
    }

    #[test]
    fn unknown_format_and_output_names_are_rejected() {
        assert!(SourceFormat::parse("keepass-xml").is_err());
        assert!(OutputForm::parse("yaml").is_err());
        assert_eq!(SourceFormat::parse("AUTO").unwrap(), SourceFormat::Auto);
        assert_eq!(OutputForm::parse("JSON").unwrap(), OutputForm::Json);
    }

    #[test]
    fn over_the_cap_is_rejected_at_the_boundary() {
        let ok = "aB3!xY9@zQ1#\n".repeat(MAX_ENTRIES);
        assert_eq!(analyze(&ok, &opts(), 0.0).unwrap().entries, MAX_ENTRIES);
        let over = "aB3!xY9@zQ1#\n".repeat(MAX_ENTRIES + 1);
        let e = analyze(&over, &opts(), 0.0).unwrap_err();
        assert!(e.contains("up to 5000 entries"), "{e}");
    }

    #[test]
    fn a_clean_vault_reports_no_issues() {
        let data = "name,username,password\n\
                    Mail,ada,T7#vq!Lm2zRp8w\n\
                    Bank,ada,Qx4$rnZb6*Kd1e\n";
        let r = analyze(data, &opts(), 0.0).unwrap();
        assert!(r.ok, "{:?}", r.findings);
        assert!(r.findings.is_empty(), "{:?}", r.findings);
        assert_eq!(r.vault_band, "strong");
        assert!(audit(data, &opts(), 0.0).unwrap().contains("No issues found"));
    }
}

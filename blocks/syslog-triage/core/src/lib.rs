//! syslog-triage core — pure compute, shared by the chat skill block and the web
//! page. No wafer/wasm-bindgen deps — deterministic pure Rust.
//!
//! Parses Linux syslog / auth.log text (BSD RFC 3164 and rsyslog RFC 3339/ISO
//! timestamps, with an optional `<PRI>` prefix) into structured events, then
//! classifies each into a security-relevant **category** — sudo, ssh, cron,
//! session, account, or other — with a derived **status** (success / failure /
//! info) and the fields an intrusion review cares about (user, source IP,
//! command). Renders an intrusion-review **summary** (the default), a Markdown
//! **table**, or a **json** array; filter by category and by status.

use regex::Regex;
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// Default row / item cap when `limit` is 0/unset.
const DEFAULT_LIMIT: u32 = 500;
/// Hard upper bound the `limit` param is clamped to.
pub const MAX_LIMIT: u32 = 5000;

// ---------------------------------------------------------------------------
// Enums parsed from user strings.
// ---------------------------------------------------------------------------

/// The security category an event falls into (derived from the service tag +
/// message, never chosen by the user).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Sudo,
    Ssh,
    Cron,
    Session,
    Account,
    Other,
}

impl Category {
    fn label(self) -> &'static str {
        match self {
            Category::Sudo => "sudo",
            Category::Ssh => "ssh",
            Category::Cron => "cron",
            Category::Session => "session",
            Category::Account => "account",
            Category::Other => "other",
        }
    }
}

/// The `category` filter the user selects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatFilter {
    All,
    One(Category),
}

impl CatFilter {
    pub fn parse(s: &str) -> Result<CatFilter, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "all" => CatFilter::All,
            "sudo" => CatFilter::One(Category::Sudo),
            "ssh" | "sshd" => CatFilter::One(Category::Ssh),
            "cron" => CatFilter::One(Category::Cron),
            "session" => CatFilter::One(Category::Session),
            "account" => CatFilter::One(Category::Account),
            "other" => CatFilter::One(Category::Other),
            other => {
                return Err(format!(
                    "unknown category '{other}' (use all, sudo, ssh, cron, session, account, or other)"
                ))
            }
        })
    }

    fn keeps(self, c: Category) -> bool {
        match self {
            CatFilter::All => true,
            CatFilter::One(want) => c == want,
        }
    }
}

/// Whether an event succeeded, failed, or is neutral/informational.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Success,
    Failure,
    Info,
}

impl Status {
    fn label(self) -> &'static str {
        match self {
            Status::Success => "success",
            Status::Failure => "failure",
            Status::Info => "info",
        }
    }
}

/// The `only` status filter the user selects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusFilter {
    All,
    Failed,
    Success,
}

impl StatusFilter {
    pub fn parse(s: &str) -> Result<StatusFilter, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "all" => StatusFilter::All,
            "failed" | "failures" | "failure" => StatusFilter::Failed,
            "success" | "ok" | "accepted" => StatusFilter::Success,
            other => {
                return Err(format!(
                    "unknown status filter '{other}' (use all, failed, or success)"
                ))
            }
        })
    }

    fn keeps(self, s: Status) -> bool {
        match self {
            StatusFilter::All => true,
            StatusFilter::Failed => s == Status::Failure,
            StatusFilter::Success => s == Status::Success,
        }
    }
}

/// How to render the parsed events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    Summary,
    Table,
    Json,
}

impl Output {
    pub fn parse(s: &str) -> Result<Output, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "summary" | "review" => Output::Summary,
            "table" | "markdown" | "md" => Output::Table,
            "json" => Output::Json,
            other => {
                return Err(format!("unknown output '{other}' (use summary, table, or json)"))
            }
        })
    }
}

// ---------------------------------------------------------------------------
// Parsed event.
// ---------------------------------------------------------------------------

/// One parsed syslog event with the fields an intrusion review needs.
#[derive(Debug, Clone)]
pub struct Event {
    pub time: String,
    pub host: String,
    pub service: String,
    pub pid: String,
    pub category: Category,
    pub status: Status,
    pub user: String,
    pub source_ip: String,
    /// Human-readable one-line description of the action.
    pub detail: String,
}

// ---------------------------------------------------------------------------
// Compiled matchers (built once per call).
// ---------------------------------------------------------------------------

struct Matchers {
    header: Regex,
    ssh_accepted: Regex,
    ssh_failed: Regex,
    ssh_invalid: Regex,
    ssh_conn_closed: Regex,
    sudo_command: Regex,
    cron_cmd: Regex,
    pam_session: Regex,
    kv_user: Regex,
    kv_rhost: Regex,
    acct_name: Regex,
}

impl Matchers {
    fn new() -> Matchers {
        // [<PRI>] TIMESTAMP HOST TAG[pid]: MESSAGE
        // TIMESTAMP is BSD (`Mmm dd HH:MM:SS`) or ISO/RFC3339
        // (`2024-01-02T03:04:05[.ffff][Z|+00:00]`, `T` or space separator).
        let header = Regex::new(
            r"(?x)
            ^(?:<\d{1,3}>)?
            (?P<ts>
                [A-Z][a-z]{2}\s+\d{1,2}\s+\d{2}:\d{2}:\d{2}
              | \d{4}-\d{2}-\d{2}[T\ ]\d{2}:\d{2}:\d{2}(?:[.,]\d+)?(?:Z|[+-]\d{2}:?\d{2})?
            )
            \s+(?P<host>\S+)
            \s+(?P<tag>[A-Za-z0-9._/-]+)(?:\[(?P<pid>\d+)\])?:\s?
            (?P<msg>.*)$",
        )
        .unwrap();

        // sshd messages.
        let ssh_accepted =
            Regex::new(r"^Accepted (\S+) for (\S+) from (\S+) port (\d+)").unwrap();
        let ssh_failed =
            Regex::new(r"^Failed (\S+) for (?:invalid user )?(\S+) from (\S+) port (\d+)").unwrap();
        let ssh_invalid = Regex::new(r"^Invalid user (\S+) from (\S+)").unwrap();
        let ssh_conn_closed =
            Regex::new(r"^Connection closed by (?:invalid user \S+ )?(\S+) port \d+").unwrap();

        // sudo: `user : TTY=… ; PWD=… ; USER=target ; COMMAND=cmd`.
        let sudo_command =
            Regex::new(r"^(\S+) : .*?(?:USER=(\S+) ;\s*)?COMMAND=(.*)$").unwrap();

        // cron: `(user) CMD (command)`.
        let cron_cmd = Regex::new(r"^\((\S+)\) CMD \((.*)\)\s*$").unwrap();

        // PAM: `pam_unix(<svc>:session): session opened/closed for user <name>`.
        let pam_session =
            Regex::new(r"pam_unix\([^)]*:session\): session (opened|closed) for user (\S+?)(?:\(|,|\s|$)")
                .unwrap();

        // Generic key=value fields common in PAM auth-failure lines.
        let kv_user = Regex::new(r"\buser=(\S+)").unwrap();
        let kv_rhost = Regex::new(r"\brhost=(\S+)").unwrap();

        // Account management: `new user: name=x` / `new group: name=x`.
        let acct_name = Regex::new(r"name=([A-Za-z0-9._-]+)").unwrap();

        Matchers {
            header,
            ssh_accepted,
            ssh_failed,
            ssh_invalid,
            ssh_conn_closed,
            sudo_command,
            cron_cmd,
            pam_session,
            kv_user,
            kv_rhost,
            acct_name,
        }
    }
}

/// Classify a service tag into a category (before looking at the message).
fn category_for_tag(tag: &str) -> Category {
    let t = tag.to_ascii_lowercase();
    // Strip a trailing `.service`/path-style suffix for matching.
    let base = t.split(['.', '/']).next().unwrap_or(&t);
    if base.starts_with("sshd") {
        return Category::Ssh;
    }
    if base == "sudo" {
        return Category::Sudo;
    }
    if base.starts_with("cron") || base == "crond" || base == "anacron" {
        return Category::Cron;
    }
    if base == "su" || base == "login" || base == "systemd-logind" {
        return Category::Session;
    }
    if matches!(
        base,
        "useradd" | "userdel" | "usermod" | "groupadd" | "groupdel" | "gpasswd" | "passwd"
            | "chpasswd" | "chage"
    ) {
        return Category::Account;
    }
    Category::Other
}

/// Fill in `user` / `source_ip` / `status` / `detail` from the message.
fn classify_message(m: &Matchers, cat: Category, tag: &str, msg: &str) -> Event {
    let mut ev = Event {
        time: String::new(),
        host: String::new(),
        service: tag.to_string(),
        pid: String::new(),
        category: cat,
        status: Status::Info,
        user: String::new(),
        source_ip: String::new(),
        detail: msg.trim().to_string(),
    };

    // Common PAM auth-failure signature across services.
    let auth_failure = msg.contains("authentication failure")
        || msg.contains("auth could not identify password")
        || msg.contains("incorrect password");

    match cat {
        Category::Ssh => {
            if let Some(c) = m.ssh_accepted.captures(msg) {
                ev.status = Status::Success;
                ev.user = c[2].to_string();
                ev.source_ip = c[3].to_string();
                ev.detail = format!("accepted {} for {} from {}", &c[1], &c[2], &c[3]);
            } else if let Some(c) = m.ssh_failed.captures(msg) {
                ev.status = Status::Failure;
                ev.user = c[2].to_string();
                ev.source_ip = c[3].to_string();
                ev.detail = format!("failed {} for {} from {}", &c[1], &c[2], &c[3]);
            } else if let Some(c) = m.ssh_invalid.captures(msg) {
                ev.status = Status::Failure;
                ev.user = c[1].to_string();
                ev.source_ip = c[2].to_string();
                ev.detail = format!("invalid user {} from {}", &c[1], &c[2]);
            } else if let Some(c) = m.ssh_conn_closed.captures(msg) {
                ev.status = if msg.contains("[preauth]") {
                    Status::Failure
                } else {
                    Status::Info
                };
                ev.source_ip = c[1].to_string();
                ev.detail = format!("connection closed by {}", &c[1]);
            } else if auth_failure {
                ev.status = Status::Failure;
            }
        }
        Category::Sudo => {
            if auth_failure || msg.contains("NOT in sudoers") {
                ev.status = Status::Failure;
                if let Some(c) = m.kv_user.captures(msg) {
                    ev.user = c[1].to_string();
                }
                // `user : ... ` prefix names the invoking user on non-pam lines.
                if ev.user.is_empty() {
                    if let Some((u, _)) = msg.split_once(" : ") {
                        ev.user = u.trim().to_string();
                    }
                }
            } else if let Some(c) = m.sudo_command.captures(msg) {
                ev.status = Status::Success;
                ev.user = c[1].to_string();
                let target = c.get(2).map(|g| g.as_str()).unwrap_or("root");
                let cmd = c[3].trim();
                ev.detail = format!("{} ran (as {}) {}", &c[1], target, cmd);
            }
        }
        Category::Cron => {
            if let Some(c) = m.cron_cmd.captures(msg) {
                ev.user = c[1].to_string();
                ev.detail = format!("({}) ran {}", &c[1], c[2].trim());
            } else if let Some(c) = m.pam_session.captures(msg) {
                ev.user = c[2].to_string();
                ev.detail = format!("session {} for {}", &c[1], &c[2]);
            }
        }
        Category::Session => {
            if msg.starts_with("FAILED su") || auth_failure {
                ev.status = Status::Failure;
            } else if let Some(c) = m.pam_session.captures(msg) {
                if &c[1] == "opened" {
                    ev.status = Status::Success;
                }
                ev.user = c[2].to_string();
                ev.detail = format!("session {} for {}", &c[1], &c[2]);
            } else if msg.contains("session opened") {
                ev.status = Status::Success;
            }
            if ev.user.is_empty() {
                if let Some(c) = m.kv_user.captures(msg) {
                    ev.user = c[1].to_string();
                }
            }
        }
        Category::Account => {
            ev.status = Status::Success;
            if let Some(c) = m.acct_name.captures(msg) {
                ev.user = c[1].to_string();
            }
        }
        Category::Other => {
            if auth_failure {
                ev.status = Status::Failure;
            }
        }
    }

    // Best-effort source IP from an `rhost=` field if none captured yet.
    if ev.source_ip.is_empty() {
        if let Some(c) = m.kv_rhost.captures(msg) {
            ev.source_ip = c[1].to_string();
        }
    }
    ev
}

// ---------------------------------------------------------------------------
// Public entry point.
// ---------------------------------------------------------------------------

/// Parse `logs`, classify + filter, and render.
///
/// - `category`: `all` (default) | `sudo` | `ssh` | `cron` | `session` | `account` | `other`.
/// - `only`:     `all` (default) | `failed` | `success` — status filter.
/// - `output`:   `summary` (default) | `table` | `json`.
/// - `limit`:    cap the number of events rendered (0 → default 500; hard max 5000).
pub fn triage(
    logs: &str,
    category: &str,
    only: &str,
    output: &str,
    limit: u32,
) -> Result<String, String> {
    if logs.trim().is_empty() {
        return Err("input is empty — paste some syslog or auth.log lines".into());
    }
    let cat_filter = CatFilter::parse(category)?;
    let status_filter = StatusFilter::parse(only)?;
    let out = Output::parse(output)?;
    let limit = if limit == 0 {
        DEFAULT_LIMIT
    } else {
        limit.clamp(1, MAX_LIMIT)
    } as usize;

    let m = Matchers::new();

    // Parse every non-empty line into an Event. Lines that don't match the
    // syslog header become an `other`/info event carrying the whole line, so
    // nothing is silently dropped.
    let mut events: Vec<Event> = Vec::new();
    for line in logs.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let ev = if let Some(h) = m.header.captures(line) {
            let tag = &h["tag"];
            let cat = category_for_tag(tag);
            let mut ev = classify_message(&m, cat, tag, h["msg"].trim());
            ev.time = h["ts"].to_string();
            ev.host = h["host"].to_string();
            ev.pid = h.name("pid").map(|g| g.as_str()).unwrap_or("").to_string();
            ev
        } else {
            Event {
                time: String::new(),
                host: String::new(),
                service: String::new(),
                pid: String::new(),
                category: Category::Other,
                status: Status::Info,
                user: String::new(),
                source_ip: String::new(),
                detail: line.trim().to_string(),
            }
        };
        events.push(ev);
    }
    if events.is_empty() {
        return Err("no log lines found".into());
    }

    let shown: Vec<&Event> = events
        .iter()
        .filter(|e| cat_filter.keeps(e.category) && status_filter.keeps(e.status))
        .take(limit)
        .collect();

    match out {
        Output::Summary => Ok(render_summary(&events, &shown)),
        Output::Table => Ok(render_table(&shown)),
        Output::Json => render_json(&shown),
    }
}

// ---------------------------------------------------------------------------
// Rendering.
// ---------------------------------------------------------------------------

const COLS: [&str; 9] = [
    "time", "host", "service", "pid", "category", "status", "user", "source_ip", "detail",
];

fn col_value<'a>(e: &'a Event, col: &str) -> &'a str {
    match col {
        "time" => &e.time,
        "host" => &e.host,
        "service" => &e.service,
        "pid" => &e.pid,
        "category" => e.category.label(),
        "status" => e.status.label(),
        "user" => &e.user,
        "source_ip" => &e.source_ip,
        "detail" => &e.detail,
        _ => "",
    }
}

fn caption(shown: &[&Event]) -> String {
    let failed = shown.iter().filter(|e| e.status == Status::Failure).count();
    format!("Syslog triage · {} events · {} failed", shown.len(), failed)
}

fn render_table(shown: &[&Event]) -> String {
    let cap = caption(shown);
    if shown.is_empty() {
        return format!("{cap}\n\n(no events match the current filter)");
    }
    let mut out = String::new();
    out.push_str(&cap);
    out.push_str("\n\n| ");
    out.push_str(&COLS.join(" | "));
    out.push_str(" |\n| ");
    out.push_str(&vec!["---"; COLS.len()].join(" | "));
    out.push_str(" |\n");
    for e in shown {
        out.push_str("| ");
        out.push_str(
            &COLS
                .iter()
                .map(|c| md_escape(col_value(e, c)))
                .collect::<Vec<_>>()
                .join(" | "),
        );
        out.push_str(" |\n");
    }
    out.pop();
    out
}

fn md_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('|', "\\|").replace('\n', " ")
}

fn render_json(shown: &[&Event]) -> Result<String, String> {
    let mut arr = Vec::with_capacity(shown.len());
    for e in shown {
        let mut obj = serde_json::Map::new();
        for c in COLS {
            obj.insert(
                c.to_string(),
                serde_json::Value::String(col_value(e, c).to_string()),
            );
        }
        arr.push(serde_json::Value::Object(obj));
    }
    serde_json::to_string_pretty(&arr).map_err(|e| format!("failed to serialize JSON: {e}"))
}

/// Intrusion-review summary: header + category counts + ranked failed logins by
/// source IP + sudo activity + cron jobs. `all_events` is reserved for future
/// whole-file stats; `shown` (post-filter) drives every rendered section.
fn render_summary(all_events: &[Event], shown: &[&Event]) -> String {
    let mut out = String::new();
    out.push_str(&caption(shown));

    // Category tally over the shown (post-filter) events, nonzero only.
    let order = [
        Category::Sudo,
        Category::Ssh,
        Category::Cron,
        Category::Session,
        Category::Account,
        Category::Other,
    ];
    let cats: Vec<String> = order
        .iter()
        .filter_map(|&c| {
            let n = shown.iter().filter(|e| e.category == c).count();
            (n > 0).then(|| format!("{} {}", c.label(), n))
        })
        .collect();
    if !cats.is_empty() {
        let _ = write!(out, "\n\nCategories: {}", cats.join(" · "));
    }

    // Failed logins by source IP (ssh + session failures with an IP), ranked.
    let mut by_ip: BTreeMap<String, (usize, Vec<String>)> = BTreeMap::new();
    for e in shown {
        if e.status == Status::Failure
            && !e.source_ip.is_empty()
            && matches!(e.category, Category::Ssh | Category::Session)
        {
            let slot = by_ip.entry(e.source_ip.clone()).or_default();
            slot.0 += 1;
            if !e.user.is_empty() && !slot.1.iter().any(|u| u == &e.user) && slot.1.len() < 6 {
                slot.1.push(e.user.clone());
            }
        }
    }
    if !by_ip.is_empty() {
        let mut ranked: Vec<(&String, &(usize, Vec<String>))> = by_ip.iter().collect();
        // Most attempts first, then IP ascending for a stable order.
        ranked.sort_by(|a, b| b.1 .0.cmp(&a.1 .0).then(a.0.cmp(b.0)));
        out.push_str("\n\nFailed logins by source IP:");
        for (ip, (count, users)) in ranked {
            let users = if users.is_empty() {
                String::new()
            } else {
                format!(" (users: {})", users.join(", "))
            };
            let _ = write!(out, "\n  {ip} ×{count}{users}");
        }
    }

    // Sudo activity.
    let sudo: Vec<&&Event> = shown
        .iter()
        .filter(|e| e.category == Category::Sudo)
        .collect();
    if !sudo.is_empty() {
        out.push_str("\n\nSudo activity:");
        for e in sudo {
            let flag = if e.status == Status::Failure { " [FAILED]" } else { "" };
            let _ = write!(out, "\n  {}{}", e.detail, flag);
        }
    }

    // Cron jobs.
    let cron: Vec<&&Event> = shown
        .iter()
        .filter(|e| e.category == Category::Cron)
        .collect();
    if !cron.is_empty() {
        out.push_str("\n\nCron:");
        for e in cron {
            let _ = write!(out, "\n  {}", e.detail);
        }
    }

    let _ = all_events; // reserved for future whole-file stats
    out
}

// ---------------------------------------------------------------------------
// Tests.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // A realistic auth.log slice: two failed SSH from one IP, one accepted, a
    // sudo command, a cron job, and a session line.
    const AUTH: &str = r#"May  3 18:20:45 web1 sshd[2001]: Failed password for root from 203.0.113.5 port 44001 ssh2
May  3 18:20:47 web1 sshd[2002]: Failed password for invalid user admin from 203.0.113.5 port 44002 ssh2
May  3 18:21:10 web1 sshd[2010]: Accepted publickey for bob from 192.168.1.10 port 51000 ssh2
May  3 18:22:00 web1 sudo:    alice : TTY=pts/0 ; PWD=/home/alice ; USER=root ; COMMAND=/usr/bin/apt-get update
May  3 18:25:01 web1 CRON[3001]: (root) CMD (/usr/local/bin/backup.sh)
May  3 18:26:00 web1 su[3100]: pam_unix(su:session): session opened for user root by alice(uid=1000)"#;

    #[test]
    fn table_parses_and_classifies_events() {
        let out = triage(AUTH, "all", "all", "table", 0).unwrap();
        assert!(out.starts_with("Syslog triage · 6 events · 2 failed"), "caption: {out}");
        assert!(out.contains(
            "| time | host | service | pid | category | status | user | source_ip | detail |"
        ));
        // Failed SSH row with the source IP + user.
        assert!(out.contains("| ssh | failure | root | 203.0.113.5 |"), "{out}");
        // Accepted publickey classified as success.
        assert!(out.contains("| ssh | success | bob | 192.168.1.10 |"), "{out}");
        // Sudo command captured.
        assert!(out.contains("| sudo | success | alice |"), "{out}");
        assert!(out.contains("apt-get update"), "{out}");
        // Cron job.
        assert!(out.contains("| cron | info | root |"), "{out}");
    }

    #[test]
    fn summary_ranks_failed_ssh_by_source_ip() {
        let out = triage(AUTH, "all", "all", "summary", 0).unwrap();
        assert!(out.starts_with("Syslog triage · 6 events · 2 failed"), "{out}");
        assert!(out.contains("Categories: sudo 1 · ssh 3 · cron 1 · session 1"), "{out}");
        assert!(out.contains("Failed logins by source IP:"), "{out}");
        assert!(out.contains("203.0.113.5 ×2"), "{out}");
        assert!(out.contains("root") && out.contains("admin"), "{out}");
        assert!(out.contains("Sudo activity:"), "{out}");
        assert!(out.contains("Cron:"), "{out}");
    }

    #[test]
    fn category_filter_keeps_only_ssh() {
        let out = triage(AUTH, "ssh", "all", "table", 0).unwrap();
        assert!(out.starts_with("Syslog triage · 3 events"), "{out}");
        assert!(!out.contains("apt-get"), "{out}");
        assert!(!out.contains("| cron |"), "{out}");
    }

    #[test]
    fn status_filter_keeps_only_failures() {
        let out = triage(AUTH, "all", "failed", "table", 0).unwrap();
        assert!(out.starts_with("Syslog triage · 2 events · 2 failed"), "{out}");
        assert!(out.contains("| failure |"));
        assert!(!out.contains("| success |"), "{out}");
        assert!(!out.contains("| info |"), "{out}");
    }

    #[test]
    fn json_output_is_valid_array() {
        let out = triage(AUTH, "all", "all", "json", 0).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert!(v.is_array());
        assert_eq!(v.as_array().unwrap().len(), 6);
        assert_eq!(v[0]["category"], "ssh");
        assert_eq!(v[0]["status"], "failure");
        assert_eq!(v[0]["source_ip"], "203.0.113.5");
    }

    #[test]
    fn iso_timestamp_and_invalid_user_line() {
        let logs = "2024-01-02T03:04:05.123456+00:00 host sshd[42]: Invalid user oracle from 198.51.100.7";
        let out = triage(logs, "all", "all", "table", 0).unwrap();
        assert!(out.contains("2024-01-02T03:04:05.123456+00:00"), "{out}");
        assert!(out.contains("| ssh | failure | oracle | 198.51.100.7 |"), "{out}");
    }

    #[test]
    fn sudo_auth_failure_is_flagged() {
        let logs = "Jul 31 10:00:00 h sudo: pam_unix(sudo:auth): authentication failure; logname=eve uid=1001 euid=0 tty=/dev/pts/1 ruser=eve rhost= user=eve";
        let out = triage(logs, "sudo", "all", "summary", 0).unwrap();
        assert!(out.contains("Sudo activity:"), "{out}");
        assert!(out.contains("[FAILED]"), "{out}");
    }

    #[test]
    fn limit_caps_events() {
        let out = triage(AUTH, "all", "all", "table", 2).unwrap();
        assert!(out.starts_with("Syslog triage · 2 events"), "{out}");
    }

    #[test]
    fn empty_input_errors() {
        let err = triage("   \n  ", "all", "all", "summary", 0).unwrap_err();
        assert!(err.contains("empty"), "{err}");
    }

    #[test]
    fn bad_category_errors() {
        let err = triage("x", "kernel", "all", "table", 0).unwrap_err();
        assert!(err.contains("unknown category"), "{err}");
    }

    #[test]
    fn bad_status_filter_errors() {
        let err = triage("x", "all", "maybe", "table", 0).unwrap_err();
        assert!(err.contains("unknown status filter"), "{err}");
    }

    #[test]
    fn bad_output_errors() {
        let err = triage("x", "all", "all", "xml", 0).unwrap_err();
        assert!(err.contains("unknown output"), "{err}");
    }
}

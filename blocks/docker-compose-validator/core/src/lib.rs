//! docker-compose-validator core — validate a Compose file's *semantics*, not just
//! its YAML.
//!
//! A `docker-compose.yml` that parses cleanly can still fail the moment
//! `docker compose up` runs: a service depending on a name nobody defined, a
//! named volume that was never declared at the top level, a port mapping with a
//! typo'd range, two services fighting over one host port, a dependency cycle.
//! Those are all valid YAML. This checker parses the document with a *marked*
//! event parser (so every finding carries a real 1-based line and column) and
//! then walks the resulting tree against the Compose specification.
//!
//! Pure compute, no I/O: the same entry point backs the chat/CLI block and the
//! browser page.
//!
//! Two things make the tree pass possible:
//!
//!   * **Markers.** `yaml_rust2`'s event parser reports a source position for
//!     every scalar and collection start, so `undefined-volume` can point at the
//!     exact line the volume was referenced on rather than at the service.
//!   * **Scalar style.** The parser also reports whether a scalar was written
//!     plain or quoted, which is what lets `quote-ports` fire on `- 8080:80`
//!     while staying quiet on `- "8080:80"`.
//!
//! Anchors, aliases and `<<` merge keys are resolved while the tree is built, so
//! the very common `x-defaults: &defaults` / `<<: *defaults` pattern does not
//! produce phantom "service has neither image nor build" errors.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde_json::json;
use yaml_rust2::parser::{Event, Parser};
use yaml_rust2::scanner::{Marker, TScalarStyle};

/// Largest document accepted. A Compose file is configuration, not a data set;
/// anything bigger is rejected with a clear message instead of locking up the
/// browser tab.
pub const MAX_INPUT_BYTES: usize = 1_048_576;

/// At most this many problems are reported; the rest are summarised.
pub const MAX_PROBLEMS: usize = 500;

/// Host-port ranges wider than this are not expanded for duplicate detection.
const MAX_RANGE_EXPANSION: u32 = 4096;

// ---------------------------------------------------------------------------
// Severities, rules and presets
// ---------------------------------------------------------------------------

/// How serious a finding is. Ordered so filtering is a comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Style or hardening advice; the file works without acting on it.
    Hint,
    /// Works today, but is deprecated, risky or surprising.
    Warning,
    /// `docker compose up` will reject or misbehave on this.
    Error,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Hint => "hint",
            Severity::Warning => "warning",
            Severity::Error => "error",
        }
    }

    pub fn parse(s: &str) -> Result<Severity, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "hint" | "hints" | "info" | "all" => Ok(Severity::Hint),
            "warning" | "warn" | "warnings" => Ok(Severity::Warning),
            "error" | "errors" => Ok(Severity::Error),
            other => Err(format!(
                "min_severity must be hint, warning or error (got '{other}')"
            )),
        }
    }
}

/// A check this validator can emit: its stable id, the lowest preset level that
/// enables it (0 = essential, 1 = default, 2 = strict) and its severity.
pub struct Rule {
    pub id: &'static str,
    pub level: u8,
    pub severity: Severity,
}

const fn rule(id: &'static str, level: u8, severity: Severity) -> Rule {
    Rule { id, level, severity }
}

/// Every rule, in report order. Ids are stable and are what `disable` accepts.
pub const RULES: &[Rule] = &[
    // --- structural: the file will not come up -----------------------------
    rule("syntax", 0, Severity::Error),
    rule("top-level-type", 0, Severity::Error),
    rule("services-missing", 0, Severity::Error),
    rule("service-type", 0, Severity::Error),
    rule("image-or-build", 0, Severity::Error),
    rule("port-syntax", 0, Severity::Error),
    rule("volume-syntax", 0, Severity::Error),
    rule("environment-syntax", 0, Severity::Error),
    rule("depends-on-syntax", 0, Severity::Error),
    rule("undefined-depends-on", 0, Severity::Error),
    rule("circular-depends-on", 0, Severity::Error),
    rule("undefined-network", 0, Severity::Error),
    rule("undefined-volume", 0, Severity::Error),
    rule("undefined-config-secret", 0, Severity::Error),
    rule("duplicate-container-name", 0, Severity::Error),
    rule("duplicate-host-port", 0, Severity::Error),
    rule("restart-policy", 0, Severity::Error),
    rule("network-mode-conflict", 0, Severity::Error),
    // --- best practice: valid today, but you probably meant otherwise ------
    rule("version-field", 1, Severity::Warning),
    rule("image-tag", 1, Severity::Warning),
    rule("build-and-image", 1, Severity::Warning),
    rule("deprecated-links", 1, Severity::Warning),
    rule("privileged", 1, Severity::Warning),
    rule("host-network", 1, Severity::Warning),
    rule("env-secrets", 1, Severity::Warning),
    rule("quote-ports", 1, Severity::Warning),
    rule("unknown-top-level-key", 1, Severity::Warning),
    // --- strict: hardening and house style ---------------------------------
    rule("unknown-service-key", 2, Severity::Warning),
    rule("unbound-port-interface", 2, Severity::Hint),
    rule("missing-restart", 2, Severity::Hint),
    rule("missing-healthcheck", 2, Severity::Hint),
    rule("resource-limits", 2, Severity::Hint),
    rule("logging-options", 2, Severity::Hint),
    rule("project-name", 2, Severity::Hint),
];

fn rule_for(id: &str) -> &'static Rule {
    RULES
        .iter()
        .find(|r| r.id == id)
        .expect("every emitted rule id must be declared in RULES")
}

/// How aggressive the rule set is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    /// Only what actually breaks `docker compose up`.
    Essential,
    /// Everything in `essential` plus deprecation, security and pinning warnings.
    Default,
    /// Everything, including hardening and house-style hints.
    Strict,
}

impl Preset {
    pub fn parse(s: &str) -> Result<Preset, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "default" => Ok(Preset::Default),
            "essential" | "errors" | "relaxed" => Ok(Preset::Essential),
            "strict" => Ok(Preset::Strict),
            other => Err(format!(
                "preset must be essential, default or strict (got '{other}')"
            )),
        }
    }

    fn level(self) -> u8 {
        match self {
            Preset::Essential => 0,
            Preset::Default => 1,
            Preset::Strict => 2,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Preset::Essential => "essential",
            Preset::Default => "default",
            Preset::Strict => "strict",
        }
    }
}

/// Everything the checker needs beyond the document itself.
#[derive(Debug, Clone)]
pub struct Options {
    pub preset: Preset,
    pub disabled: HashSet<String>,
    pub strict_warnings: bool,
    pub min_severity: Severity,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            preset: Preset::Default,
            disabled: HashSet::new(),
            strict_warnings: false,
            min_severity: Severity::Hint,
        }
    }
}

impl Options {
    fn enabled(&self, id: &str) -> bool {
        !self.disabled.contains(id) && rule_for(id).level <= self.preset.level()
    }
}

/// Parse the comma/space/newline separated `disable` field into rule ids,
/// rejecting anything that is not a real rule so a typo is not silently ignored.
pub fn parse_disabled(raw: &str) -> Result<HashSet<String>, String> {
    let mut out = HashSet::new();
    for token in raw.split([',', ' ', '\n', '\r', '\t', ';']) {
        let t = token.trim();
        if t.is_empty() {
            continue;
        }
        if !RULES.iter().any(|r| r.id == t) {
            let mut known: Vec<&str> = RULES.iter().map(|r| r.id).collect();
            known.sort_unstable();
            return Err(format!(
                "unknown rule id '{t}' in disable. Known rule ids: {}",
                known.join(", ")
            ));
        }
        out.insert(t.to_string());
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Findings
// ---------------------------------------------------------------------------

/// One finding, always carrying a 1-based line and column into the input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub line: usize,
    pub column: usize,
    pub severity: Severity,
    pub rule: &'static str,
    pub message: String,
}

/// The full verdict for one document.
#[derive(Debug, Clone)]
pub struct Report {
    pub problems: Vec<Problem>,
    pub services: usize,
    pub networks: usize,
    pub volumes: usize,
    pub parsed: bool,
    pub truncated: bool,
    pub preset: Preset,
}

impl Report {
    pub fn count(&self, sev: Severity) -> usize {
        self.problems.iter().filter(|p| p.severity == sev).count()
    }

    pub fn ok(&self) -> bool {
        self.parsed && self.count(Severity::Error) == 0
    }
}

// ---------------------------------------------------------------------------
// Marked YAML tree
// ---------------------------------------------------------------------------

/// A 1-based source position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pos {
    pub line: usize,
    pub column: usize,
}

impl Pos {
    fn from(mark: &Marker) -> Pos {
        Pos {
            line: mark.line().max(1),
            column: mark.col() + 1,
        }
    }
}

/// A YAML value that remembers where it came from and how it was written.
#[derive(Debug, Clone)]
pub enum Node {
    Scalar {
        value: String,
        plain: bool,
        pos: Pos,
    },
    Seq {
        items: Vec<Node>,
        pos: Pos,
    },
    Map {
        entries: Vec<(String, Pos, Node)>,
        pos: Pos,
    },
}

impl Node {
    pub fn pos(&self) -> Pos {
        match self {
            Node::Scalar { pos, .. } | Node::Seq { pos, .. } | Node::Map { pos, .. } => *pos,
        }
    }

    fn kind(&self) -> &'static str {
        match self {
            Node::Scalar { .. } => "a scalar",
            Node::Seq { .. } => "a list",
            Node::Map { .. } => "a mapping",
        }
    }

    fn as_str(&self) -> Option<&str> {
        match self {
            Node::Scalar { value, .. } => Some(value.as_str()),
            _ => None,
        }
    }

    fn as_map(&self) -> Option<&[(String, Pos, Node)]> {
        match self {
            Node::Map { entries, .. } => Some(entries),
            _ => None,
        }
    }

    fn as_seq(&self) -> Option<&[Node]> {
        match self {
            Node::Seq { items, .. } => Some(items),
            _ => None,
        }
    }

    fn get(&self, key: &str) -> Option<&Node> {
        self.as_map()
            .and_then(|e| e.iter().find(|(k, _, _)| k == key))
            .map(|(_, _, v)| v)
    }

    fn entry(&self, key: &str) -> Option<(Pos, &Node)> {
        self.as_map()
            .and_then(|e| e.iter().find(|(k, _, _)| k == key))
            .map(|(_, p, v)| (*p, v))
    }

    fn is_true(&self) -> bool {
        matches!(self.as_str(), Some(v) if matches!(v.to_ascii_lowercase().as_str(), "true" | "yes" | "on"))
    }

    /// Keys of a mapping, or the string items of a list — the two shapes Compose
    /// accepts interchangeably for `networks`, `depends_on`, `environment`, …
    fn names(&self) -> Vec<(String, Pos)> {
        match self {
            Node::Map { entries, .. } => entries
                .iter()
                .map(|(k, p, _)| (k.clone(), *p))
                .collect(),
            Node::Seq { items, .. } => items
                .iter()
                .filter_map(|i| i.as_str().map(|s| (s.to_string(), i.pos())))
                .collect(),
            _ => Vec::new(),
        }
    }
}

/// Build one marked tree from the first document in `input`.
///
/// Returns `Err` with a positioned message for a parse failure. Aliases are
/// substituted with a clone of the anchored node, and `<<` merge keys are
/// flattened into the surrounding mapping without overriding explicit keys.
fn parse_marked(input: &str) -> Result<Option<Node>, (Pos, String)> {
    enum Frame {
        Seq(Vec<Node>, Pos, usize),
        Map(Vec<(String, Pos, Node)>, Pos, usize, Option<(String, Pos)>),
    }

    let mut parser = Parser::new_from_str(input);
    let mut stack: Vec<Frame> = Vec::new();
    let mut anchors: HashMap<usize, Node> = HashMap::new();
    let mut root: Option<Node> = None;
    let mut depth_guard = 0usize;

    // Push a completed value into whatever container is open (or make it root).
    fn place(
        stack: &mut Vec<Frame>,
        root: &mut Option<Node>,
        key_pos: Option<Pos>,
        node: Node,
    ) -> Result<(), (Pos, String)> {
        match stack.last_mut() {
            None => {
                if root.is_none() {
                    *root = Some(node);
                }
                Ok(())
            }
            Some(Frame::Seq(items, _, _)) => {
                items.push(node);
                Ok(())
            }
            Some(Frame::Map(entries, _, _, pending)) => match pending.take() {
                None => {
                    // This value is a key. Only string-ish keys are addressable.
                    let (key, pos) = match &node {
                        Node::Scalar { value, pos, .. } => (value.clone(), *pos),
                        other => (String::new(), key_pos.unwrap_or(other.pos())),
                    };
                    *pending = Some((key, pos));
                    Ok(())
                }
                Some((key, pos)) => {
                    if key == "<<" {
                        // Merge key: fold the referenced mapping(s) in.
                        let mut merge_sources: Vec<&Node> = Vec::new();
                        if let Node::Seq { items, .. } = &node {
                            merge_sources.extend(items.iter());
                        } else {
                            merge_sources.push(&node);
                        }
                        for src in merge_sources {
                            if let Node::Map { entries: src_e, .. } = src {
                                for (k, p, v) in src_e {
                                    if !entries.iter().any(|(ek, _, _)| ek == k) {
                                        entries.push((k.clone(), *p, v.clone()));
                                    }
                                }
                            }
                        }
                    } else {
                        entries.push((key, pos, node));
                    }
                    Ok(())
                }
            },
        }
    }

    loop {
        let (ev, mark) = parser
            .next_token()
            .map_err(|e| (Pos::from(e.marker()), e.info().to_string()))?;
        let pos = Pos::from(&mark);
        match ev {
            Event::StreamStart | Event::DocumentStart | Event::Nothing => {}
            Event::DocumentEnd => {
                // Only the first document is analysed; stop after it closes.
                if root.is_some() {
                    return Ok(root);
                }
            }
            Event::StreamEnd => return Ok(root),
            Event::Scalar(value, style, aid, _) => {
                let node = Node::Scalar {
                    value,
                    plain: style == TScalarStyle::Plain,
                    pos,
                };
                if aid > 0 {
                    anchors.insert(aid, node.clone());
                }
                place(&mut stack, &mut root, Some(pos), node)?;
            }
            Event::Alias(aid) => {
                let node = anchors.get(&aid).cloned().unwrap_or(Node::Scalar {
                    value: String::new(),
                    plain: true,
                    pos,
                });
                place(&mut stack, &mut root, Some(pos), node)?;
            }
            Event::SequenceStart(aid, _) => {
                depth_guard += 1;
                if depth_guard > 256 {
                    return Err((pos, "document nesting is deeper than 256 levels".into()));
                }
                stack.push(Frame::Seq(Vec::new(), pos, aid));
            }
            Event::MappingStart(aid, _) => {
                depth_guard += 1;
                if depth_guard > 256 {
                    return Err((pos, "document nesting is deeper than 256 levels".into()));
                }
                stack.push(Frame::Map(Vec::new(), pos, aid, None));
            }
            Event::SequenceEnd | Event::MappingEnd => {
                depth_guard = depth_guard.saturating_sub(1);
                let (node, aid) = match stack.pop() {
                    Some(Frame::Seq(items, p, aid)) => (Node::Seq { items, pos: p }, aid),
                    Some(Frame::Map(entries, p, aid, _)) => (Node::Map { entries, pos: p }, aid),
                    None => return Err((pos, "unbalanced collection in document".into())),
                };
                if aid > 0 {
                    anchors.insert(aid, node.clone());
                }
                place(&mut stack, &mut root, None, node)?;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Compose vocabulary
// ---------------------------------------------------------------------------

const TOP_LEVEL_KEYS: &[&str] = &[
    "configs", "include", "name", "networks", "secrets", "services", "version", "volumes",
];

/// Service keys from the Compose specification. `x-` prefixed keys are always
/// accepted and are not listed here.
const SERVICE_KEYS: &[&str] = &[
    "annotations",
    "attach",
    "blkio_config",
    "build",
    "cap_add",
    "cap_drop",
    "cgroup",
    "cgroup_parent",
    "command",
    "configs",
    "container_name",
    "cpu_count",
    "cpu_percent",
    "cpu_period",
    "cpu_quota",
    "cpu_rt_period",
    "cpu_rt_runtime",
    "cpu_shares",
    "cpus",
    "cpuset",
    "credential_spec",
    "depends_on",
    "deploy",
    "develop",
    "device_cgroup_rules",
    "devices",
    "dns",
    "dns_opt",
    "dns_search",
    "domainname",
    "entrypoint",
    "env_file",
    "environment",
    "expose",
    "extends",
    "external_links",
    "extra_hosts",
    "gpus",
    "group_add",
    "healthcheck",
    "hostname",
    "image",
    "init",
    "ipc",
    "isolation",
    "label_file",
    "labels",
    "links",
    "logging",
    "mac_address",
    "mem_limit",
    "mem_reservation",
    "mem_swappiness",
    "memswap_limit",
    "network_mode",
    "networks",
    "oom_kill_disable",
    "oom_score_adj",
    "pid",
    "pids_limit",
    "platform",
    "post_start",
    "pre_stop",
    "privileged",
    "profiles",
    "provider",
    "pull_policy",
    "read_only",
    "restart",
    "runtime",
    "scale",
    "secrets",
    "security_opt",
    "shm_size",
    "stdin_open",
    "stop_grace_period",
    "stop_signal",
    "storage_opt",
    "sysctls",
    "tmpfs",
    "tty",
    "ulimits",
    "user",
    "userns_mode",
    "uts",
    "volumes",
    "volumes_from",
    "working_dir",
];

const DEPENDS_ON_CONDITIONS: &[&str] = &[
    "service_started",
    "service_healthy",
    "service_completed_successfully",
];

/// Tags that pin nothing — the image they resolve to changes under you.
const FLOATING_TAGS: &[&str] = &[
    "latest", "stable", "edge", "main", "master", "dev", "devel", "nightly", "test", "unstable",
];

/// Environment variable names whose literal values should not be committed.
const SECRET_HINTS: &[&str] = &[
    "PASSWORD", "PASSWD", "SECRET", "TOKEN", "API_KEY", "APIKEY", "ACCESS_KEY", "PRIVATE_KEY",
    "CREDENTIAL", "AUTH_KEY",
];

const VOLUME_MODES: &[&str] = &[
    "ro",
    "rw",
    "z",
    "Z",
    "cached",
    "delegated",
    "consistent",
    "nocopy",
    "bind",
    "volume",
    "shared",
    "slave",
    "private",
    "rshared",
    "rslave",
    "rprivate",
];

// ---------------------------------------------------------------------------
// Entry points
// ---------------------------------------------------------------------------

/// Typed entry point (chat/CLI): every argument already has its schema type.
#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &str,
    preset: &str,
    disable: &str,
    strict_warnings: bool,
    min_severity: &str,
    report_format: &str,
) -> Result<String, String> {
    let opts = Options {
        preset: Preset::parse(preset)?,
        disabled: parse_disabled(disable)?,
        strict_warnings,
        min_severity: Severity::parse(min_severity)?,
    };
    let report = validate(input, &opts)?;
    match report_format.trim().to_ascii_lowercase().as_str() {
        "" | "report" | "text" => Ok(format_report(&report)),
        "json" => Ok(format_json(&report)),
        other => Err(format!(
            "report_format must be report or json (got '{other}')"
        )),
    }
}

/// String entry point (browser page): the page hands every field over as text.
pub fn run_str(
    input: &str,
    preset: &str,
    disable: &str,
    strict_warnings: &str,
    min_severity: &str,
    report_format: &str,
) -> Result<String, String> {
    run(
        input,
        preset,
        disable,
        truthy(strict_warnings),
        min_severity,
        report_format,
    )
}

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

// ---------------------------------------------------------------------------
// The checker
// ---------------------------------------------------------------------------

/// Collector that applies preset/disable filtering as findings are added.
struct Findings<'a> {
    opts: &'a Options,
    out: Vec<Problem>,
    truncated: bool,
}

impl<'a> Findings<'a> {
    fn new(opts: &'a Options) -> Self {
        Findings {
            opts,
            out: Vec::new(),
            truncated: false,
        }
    }

    fn on(&self, id: &str) -> bool {
        self.opts.enabled(id)
    }

    fn add(&mut self, id: &'static str, pos: Pos, message: impl Into<String>) {
        if !self.on(id) {
            return;
        }
        if self.out.len() >= MAX_PROBLEMS {
            self.truncated = true;
            return;
        }
        self.out.push(Problem {
            line: pos.line,
            column: pos.column,
            severity: rule_for(id).severity,
            rule: id,
            message: message.into(),
        });
    }
}

/// Validate one Compose document and return the (filtered, sorted) report.
pub fn validate(input: &str, opts: &Options) -> Result<Report, String> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes; the maximum is {} bytes ({} KiB)",
            input.len(),
            MAX_INPUT_BYTES,
            MAX_INPUT_BYTES / 1024
        ));
    }
    if input.trim().is_empty() {
        return Err("input is empty — paste a docker-compose.yml to validate".into());
    }

    let mut f = Findings::new(opts);
    let mut services = 0usize;
    let mut networks = 0usize;
    let mut volumes = 0usize;
    let mut parsed = true;

    match parse_marked(input) {
        Err((pos, msg)) => {
            parsed = false;
            f.add(
                "syntax",
                pos,
                format!("YAML syntax error: {msg}. The file must parse as YAML before Compose can read it."),
            );
        }
        Ok(None) => {
            parsed = false;
            f.add(
                "top-level-type",
                Pos { line: 1, column: 1 },
                "the document is empty — a Compose file needs a top-level 'services:' mapping",
            );
        }
        Ok(Some(root)) => match root.as_map() {
            None => {
                f.add(
                    "top-level-type",
                    root.pos(),
                    format!(
                        "expected the document to be a mapping of top-level keys such as 'services:', got {}",
                        root.kind()
                    ),
                );
            }
            Some(_) => {
                let counts = check_document(&root, &mut f);
                services = counts.0;
                networks = counts.1;
                volumes = counts.2;
            }
        },
    }

    let truncated = f.truncated;
    let mut problems = f.out;

    if opts.strict_warnings {
        for p in problems.iter_mut() {
            if p.severity == Severity::Warning {
                p.severity = Severity::Error;
            }
        }
    }
    problems.retain(|p| p.severity >= opts.min_severity);
    problems.sort_by(|a, b| {
        (a.line, a.column, a.rule).cmp(&(b.line, b.column, b.rule))
    });

    Ok(Report {
        problems,
        services,
        networks,
        volumes,
        parsed,
        truncated,
        preset: opts.preset,
    })
}

/// Walk the whole document. Returns (services, networks, volumes) counts.
fn check_document(root: &Node, f: &mut Findings) -> (usize, usize, usize) {
    let entries = root.as_map().unwrap_or(&[]);

    for (key, pos, _) in entries {
        if key.starts_with("x-") || TOP_LEVEL_KEYS.contains(&key.as_str()) {
            continue;
        }
        f.add(
            "unknown-top-level-key",
            *pos,
            format!(
                "'{key}' is not a Compose top-level key. Expected one of {} (or an 'x-' extension key)",
                TOP_LEVEL_KEYS.join(", ")
            ),
        );
    }

    if let Some((pos, _)) = root.entry("version") {
        f.add(
            "version-field",
            pos,
            "the top-level 'version:' field is obsolete and is ignored by current Compose releases — delete the line",
        );
    }

    if root.get("name").is_none() {
        f.add(
            "project-name",
            root.pos(),
            "no top-level 'name:' — Compose will fall back to the directory name, which changes when the folder is renamed",
        );
    }

    // Declared top-level resources.
    let declared_networks: Vec<(String, Pos)> =
        root.get("networks").map(|n| n.names()).unwrap_or_default();
    let declared_volumes: Vec<(String, Pos)> =
        root.get("volumes").map(|n| n.names()).unwrap_or_default();
    let declared_configs: Vec<(String, Pos)> =
        root.get("configs").map(|n| n.names()).unwrap_or_default();
    let declared_secrets: Vec<(String, Pos)> =
        root.get("secrets").map(|n| n.names()).unwrap_or_default();

    let network_set: HashSet<&str> = declared_networks.iter().map(|(n, _)| n.as_str()).collect();
    let volume_set: HashSet<&str> = declared_volumes.iter().map(|(n, _)| n.as_str()).collect();
    let config_set: HashSet<&str> = declared_configs.iter().map(|(n, _)| n.as_str()).collect();
    let secret_set: HashSet<&str> = declared_secrets.iter().map(|(n, _)| n.as_str()).collect();

    let services_node = match root.entry("services") {
        None => {
            f.add(
                "services-missing",
                root.pos(),
                "no top-level 'services:' key — a Compose file must define at least one service",
            );
            return (0, declared_networks.len(), declared_volumes.len());
        }
        Some((pos, node)) => match node {
            Node::Map { entries, .. } if entries.is_empty() => {
                f.add(
                    "services-missing",
                    pos,
                    "'services:' is empty — define at least one service under it",
                );
                return (0, declared_networks.len(), declared_volumes.len());
            }
            Node::Map { .. } => node,
            other => {
                f.add(
                    "services-missing",
                    pos,
                    format!(
                        "'services:' must be a mapping of service name to definition, got {}",
                        other.kind()
                    ),
                );
                return (0, declared_networks.len(), declared_volumes.len());
            }
        },
    };

    let service_entries = services_node.as_map().unwrap_or(&[]);
    let service_names: HashSet<&str> = service_entries
        .iter()
        .map(|(name, _, _)| name.as_str())
        .collect();

    // Cross-service state.
    let mut container_names: HashMap<String, (String, Pos)> = HashMap::new();
    let mut host_ports: HashMap<String, (String, Pos)> = HashMap::new();
    let mut depends_graph: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for (name, name_pos, svc) in service_entries {
        let svc_map = match svc.as_map() {
            Some(m) => m,
            None => {
                f.add(
                    "service-type",
                    svc.pos(),
                    format!("service '{name}' must be a mapping of Compose keys, got {}", svc.kind()),
                );
                continue;
            }
        };

        check_service_keys(name, svc_map, f);
        check_image_and_build(name, *name_pos, svc, f);
        check_ports(name, svc, &mut host_ports, f);
        check_volumes(name, svc, &volume_set, &declared_volumes, f);
        check_networks(name, svc, &network_set, f);
        check_environment(name, svc, f);
        check_config_secret_refs(name, svc, "configs", &config_set, f);
        check_config_secret_refs(name, svc, "secrets", &secret_set, f);
        check_restart(name, *name_pos, svc, f);
        check_security(name, svc, f);
        check_hints(name, *name_pos, svc, f);

        let deps = check_depends_on(name, svc, &service_names, f);
        depends_graph.insert(name.clone(), deps);

        if let Some((pos, cn)) = svc.entry("container_name") {
            if let Some(value) = cn.as_str() {
                if let Some((other, other_pos)) =
                    container_names.insert(value.to_string(), (name.clone(), pos))
                {
                    f.add(
                        "duplicate-container-name",
                        pos,
                        format!(
                            "container_name '{value}' is already used by service '{other}' on line {} — container names must be unique on a host",
                            other_pos.line
                        ),
                    );
                }
            }
        }
    }

    check_cycles(&depends_graph, services_node, service_entries, f);

    (
        service_entries.len(),
        declared_networks.len(),
        declared_volumes.len(),
    )
}

fn check_service_keys(name: &str, entries: &[(String, Pos, Node)], f: &mut Findings) {
    if !f.on("unknown-service-key") {
        return;
    }
    for (key, pos, _) in entries {
        if key.starts_with("x-") || SERVICE_KEYS.contains(&key.as_str()) {
            continue;
        }
        f.add(
            "unknown-service-key",
            *pos,
            format!("service '{name}' sets '{key}', which is not a key in the Compose specification — check the spelling"),
        );
    }
}

fn check_image_and_build(name: &str, name_pos: Pos, svc: &Node, f: &mut Findings) {
    let image = svc.entry("image");
    let build = svc.get("build");

    match (&image, build) {
        (None, None) => {
            f.add(
                "image-or-build",
                name_pos,
                format!("service '{name}' has neither 'image:' nor 'build:' — Compose has nothing to run"),
            );
        }
        (Some((pos, _)), Some(_)) => {
            f.add(
                "build-and-image",
                *pos,
                format!("service '{name}' sets both 'build:' and 'image:' — Compose will build and then tag the result as that image; drop one unless the tag is intentional"),
            );
        }
        _ => {}
    }

    if let Some((pos, node)) = image {
        if let Some(reference) = node.as_str() {
            check_image_tag(name, pos, reference, f);
        }
    }
}

/// Flag a floating or absent tag. A `@sha256:` digest counts as pinned.
fn check_image_tag(service: &str, pos: Pos, reference: &str, f: &mut Findings) {
    let reference = reference.trim();
    if reference.is_empty() || reference.contains("${") {
        return;
    }
    if reference.contains('@') {
        return;
    }
    // The tag separator is the last ':' that comes after the last '/', so a
    // registry port like `registry:5000/app` is not mistaken for a tag.
    let last_slash = reference.rfind('/').map(|i| i as isize).unwrap_or(-1);
    let tag = reference
        .rfind(':')
        .filter(|i| (*i as isize) > last_slash)
        .map(|i| &reference[i + 1..]);

    match tag {
        None => f.add(
            "image-tag",
            pos,
            format!("service '{service}' uses image '{reference}' with no tag, which resolves to ':latest' — pin an explicit version for reproducible deploys"),
        ),
        Some(t) if FLOATING_TAGS.contains(&t.to_ascii_lowercase().as_str()) => f.add(
            "image-tag",
            pos,
            format!("service '{service}' pins image tag ':{t}', which moves without warning — use a version tag or an image digest"),
        ),
        Some(_) => {}
    }
}

// --- ports -----------------------------------------------------------------

/// One parsed published mapping, used for duplicate detection.
#[derive(Debug)]
struct PublishedPort {
    host_ip: Option<String>,
    host_range: Option<(u32, u32)>,
}

fn check_ports(
    service: &str,
    svc: &Node,
    host_ports: &mut HashMap<String, (String, Pos)>,
    f: &mut Findings,
) {
    let (ports_pos, ports) = match svc.entry("ports") {
        Some(v) => v,
        None => return,
    };
    let items = match ports.as_seq() {
        Some(items) => items,
        None => {
            f.add(
                "port-syntax",
                ports_pos,
                format!("service '{service}': 'ports:' must be a list, got {}", ports.kind()),
            );
            return;
        }
    };

    for item in items {
        let pos = item.pos();
        let published = match item {
            Node::Scalar { value, plain, .. } => {
                if *plain && value.contains(':') {
                    f.add(
                        "quote-ports",
                        pos,
                        format!("service '{service}': quote the port mapping as \"{value}\" — an unquoted host:container pair is read as a sexagesimal number by YAML 1.1 parsers"),
                    );
                }
                match parse_short_port(value) {
                    Ok(p) => Some(p),
                    Err(msg) => {
                        f.add(
                            "port-syntax",
                            pos,
                            format!("service '{service}': invalid port mapping '{value}' — {msg}"),
                        );
                        None
                    }
                }
            }
            Node::Map { .. } => match parse_long_port(item) {
                Ok(p) => Some(p),
                Err(msg) => {
                    f.add(
                        "port-syntax",
                        pos,
                        format!("service '{service}': invalid long-syntax port entry — {msg}"),
                    );
                    None
                }
            },
            other => {
                f.add(
                    "port-syntax",
                    pos,
                    format!(
                        "service '{service}': a port entry must be a string like \"8080:80\" or a mapping with 'target'/'published', got {}",
                        other.kind()
                    ),
                );
                None
            }
        };

        let Some(p) = published else { continue };
        let Some((start, end)) = p.host_range else {
            continue;
        };

        if p.host_ip.is_none() {
            f.add(
                "unbound-port-interface",
                pos,
                format!("service '{service}' publishes host port {start} on every interface — prefix the mapping with 127.0.0.1: to keep it local"),
            );
        }

        if end.saturating_sub(start) > MAX_RANGE_EXPANSION {
            continue;
        }
        let ip = p.host_ip.clone().unwrap_or_else(|| "0.0.0.0".to_string());
        for port in start..=end {
            let key = format!("{ip}:{port}");
            if let Some((other, other_pos)) =
                host_ports.insert(key, (service.to_string(), pos))
            {
                f.add(
                    "duplicate-host-port",
                    pos,
                    format!("host port {ip}:{port} is already published by service '{other}' on line {} — only one service can bind it", other_pos.line),
                );
                break;
            }
        }
    }
}

/// Parse a short-syntax port mapping: `[HOST_IP:][HOST[-RANGE]:]CONTAINER[-RANGE][/PROTO]`.
fn parse_short_port(raw: &str) -> Result<PublishedPort, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("the entry is empty".into());
    }
    if raw.contains("${") {
        // Interpolated at runtime; nothing verifiable here.
        return Ok(PublishedPort {
            host_ip: None,
            host_range: None,
        });
    }

    // Split off an optional /protocol suffix.
    let (body, proto) = match raw.rsplit_once('/') {
        Some((b, p)) => (b, Some(p)),
        None => (raw, None),
    };
    if let Some(p) = proto {
        if !matches!(p.to_ascii_lowercase().as_str(), "tcp" | "udp" | "sctp") {
            return Err(format!(
                "'{p}' is not a known protocol; expected tcp, udp or sctp"
            ));
        }
    }

    // An optional bracketed IPv6 host address comes first.
    let (host_ip, rest) = if let Some(stripped) = body.strip_prefix('[') {
        match stripped.split_once(']') {
            Some((addr, tail)) => {
                let tail = tail.strip_prefix(':').ok_or_else(|| {
                    "a bracketed host address must be followed by ':' and a port".to_string()
                })?;
                (Some(addr.to_string()), tail)
            }
            None => return Err("the bracketed host address is missing its closing ']'".into()),
        }
    } else {
        (None, body)
    };

    let parts: Vec<&str> = rest.split(':').collect();
    let (host_ip, host, container) = match (host_ip, parts.as_slice()) {
        (Some(ip), [host, container]) => (Some(ip), Some(*host), *container),
        (Some(_), _) => {
            return Err("expected HOST_IP:HOST_PORT:CONTAINER_PORT after the bracketed address".into())
        }
        (None, [container]) => (None, None, *container),
        (None, [host, container]) => (None, Some(*host), *container),
        (None, [ip, host, container]) => {
            if !is_ipv4(ip) {
                return Err(format!(
                    "'{ip}' is not an IPv4 host address; use 127.0.0.1:HOST:CONTAINER or bracket an IPv6 address"
                ));
            }
            (Some(ip.to_string()), Some(*host), *container)
        }
        (None, _) => {
            return Err(
                "expected CONTAINER, HOST:CONTAINER or HOST_IP:HOST:CONTAINER".to_string()
            )
        }
    };

    let container_range = parse_port_range(container).map_err(|e| format!("container port {e}"))?;
    let host_range = match host {
        // `:8080` asks Docker for an ephemeral host port; that is legal.
        None | Some("") => None,
        Some(h) => Some(parse_port_range(h).map_err(|e| format!("host port {e}"))?),
    };

    if let (Some((hs, he)), (cs, ce)) = (host_range, container_range) {
        let host_width = he - hs;
        let container_width = ce - cs;
        if host_width != container_width {
            return Err(format!(
                "the host range spans {} port(s) but the container range spans {} — the two must be the same width",
                host_width + 1,
                container_width + 1
            ));
        }
    }

    Ok(PublishedPort {
        host_ip,
        host_range,
    })
}

/// Parse the long-syntax mapping form of a port entry.
fn parse_long_port(item: &Node) -> Result<PublishedPort, String> {
    let entries = item.as_map().unwrap_or(&[]);
    for (key, _, _) in entries {
        if !matches!(
            key.as_str(),
            "target" | "published" | "host_ip" | "protocol" | "mode" | "name" | "app_protocol"
        ) && !key.starts_with("x-")
        {
            return Err(format!(
                "'{key}' is not a long-syntax port key; expected target, published, host_ip, protocol, mode, name or app_protocol"
            ));
        }
    }

    let target = item
        .get("target")
        .and_then(|n| n.as_str())
        .ok_or_else(|| "'target' is required and must be a container port".to_string())?;
    parse_port_range(target).map_err(|e| format!("target {e}"))?;

    if let Some(p) = item.get("protocol").and_then(|n| n.as_str()) {
        if !matches!(p.to_ascii_lowercase().as_str(), "tcp" | "udp" | "sctp") {
            return Err(format!(
                "protocol '{p}' is not known; expected tcp, udp or sctp"
            ));
        }
    }
    if let Some(m) = item.get("mode").and_then(|n| n.as_str()) {
        if !matches!(m, "host" | "ingress") {
            return Err(format!("mode '{m}' is not known; expected host or ingress"));
        }
    }

    let host_ip = item
        .get("host_ip")
        .and_then(|n| n.as_str())
        .map(str::to_string);
    let host_range = match item.get("published").and_then(|n| n.as_str()) {
        None => None,
        Some(p) if p.contains("${") => None,
        Some(p) => Some(parse_port_range(p).map_err(|e| format!("published {e}"))?),
    };

    Ok(PublishedPort {
        host_ip,
        host_range,
    })
}

/// Parse `N` or `N-M` into an inclusive range, rejecting anything out of 1–65535.
fn parse_port_range(raw: &str) -> Result<(u32, u32), String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("is empty".into());
    }
    let (start_s, end_s) = match raw.split_once('-') {
        Some((a, b)) => (a, b),
        None => (raw, raw),
    };
    let start = parse_port_number(start_s)?;
    let end = parse_port_number(end_s)?;
    if end < start {
        return Err(format!("range {start}-{end} runs backwards"));
    }
    Ok((start, end))
}

fn parse_port_number(raw: &str) -> Result<u32, String> {
    let t = raw.trim();
    if t.is_empty() || !t.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("'{t}' is not a number"));
    }
    let n: u32 = t.parse().map_err(|_| format!("'{t}' is out of range"))?;
    if !(1..=65535).contains(&n) {
        return Err(format!("{n} is outside the valid range 1-65535"));
    }
    Ok(n)
}

fn is_ipv4(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() == 4
        && parts.iter().all(|p| {
            !p.is_empty()
                && p.len() <= 3
                && p.bytes().all(|b| b.is_ascii_digit())
                && p.parse::<u32>().map(|n| n <= 255).unwrap_or(false)
        })
}

// --- volumes ---------------------------------------------------------------

fn check_volumes(
    service: &str,
    svc: &Node,
    declared: &HashSet<&str>,
    declared_list: &[(String, Pos)],
    f: &mut Findings,
) {
    let (vols_pos, vols) = match svc.entry("volumes") {
        Some(v) => v,
        None => return,
    };
    let items = match vols.as_seq() {
        Some(items) => items,
        None => {
            f.add(
                "volume-syntax",
                vols_pos,
                format!("service '{service}': 'volumes:' must be a list, got {}", vols.kind()),
            );
            return;
        }
    };

    for item in items {
        let pos = item.pos();
        match item {
            Node::Scalar { value, .. } => match parse_short_volume(value) {
                Err(msg) => f.add(
                    "volume-syntax",
                    pos,
                    format!("service '{service}': invalid volume '{value}' — {msg}"),
                ),
                Ok(Some(named)) => {
                    check_named_volume(service, &named, pos, declared, declared_list, f)
                }
                Ok(None) => {}
            },
            Node::Map { .. } => {
                let ty = item.get("type").and_then(|n| n.as_str()).unwrap_or("volume");
                if item.get("target").is_none() {
                    f.add(
                        "volume-syntax",
                        pos,
                        format!("service '{service}': a long-syntax volume needs a 'target:' mount path inside the container"),
                    );
                }
                if !matches!(ty, "volume" | "bind" | "tmpfs" | "npipe" | "cluster" | "image") {
                    f.add(
                        "volume-syntax",
                        pos,
                        format!("service '{service}': volume type '{ty}' is not known; expected volume, bind, tmpfs, npipe, image or cluster"),
                    );
                    continue;
                }
                if ty == "volume" {
                    if let Some(src) = item.get("source").and_then(|n| n.as_str()) {
                        check_named_volume(service, src, pos, declared, declared_list, f);
                    }
                }
            }
            other => f.add(
                "volume-syntax",
                pos,
                format!(
                    "service '{service}': a volume entry must be a string like \"data:/var/lib/data\" or a long-syntax mapping, got {}",
                    other.kind()
                ),
            ),
        }
    }
}

fn check_named_volume(
    service: &str,
    named: &str,
    pos: Pos,
    declared: &HashSet<&str>,
    declared_list: &[(String, Pos)],
    f: &mut Findings,
) {
    if named.contains("${") || declared.contains(named) {
        return;
    }
    let known = if declared_list.is_empty() {
        "no top-level 'volumes:' key exists yet".to_string()
    } else {
        format!(
            "declared volumes are: {}",
            declared_list
                .iter()
                .map(|(n, _)| n.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    f.add(
        "undefined-volume",
        pos,
        format!("service '{service}' mounts named volume '{named}', which is not declared under the top-level 'volumes:' key — {known}"),
    );
}

/// Validate a short-syntax volume. Returns the named volume it references, if any.
fn parse_short_volume(raw: &str) -> Result<Option<String>, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("the entry is empty".into());
    }
    let parts: Vec<&str> = raw.split(':').collect();
    let (source, target, mode) = match parts.as_slice() {
        [target] => (None, *target, None),
        [source, target] => (Some(*source), *target, None),
        [source, target, mode] => (Some(*source), *target, Some(*mode)),
        _ => {
            return Err(
                "expected CONTAINER_PATH, SOURCE:CONTAINER_PATH or SOURCE:CONTAINER_PATH:MODE"
                    .into(),
            )
        }
    };

    if !target.starts_with('/') && !target.contains("${") {
        return Err(format!(
            "the container mount path '{target}' must be absolute (start with '/')"
        ));
    }
    if let Some(mode) = mode {
        for flag in mode.split(',') {
            if !VOLUME_MODES.contains(&flag) && !flag.is_empty() {
                return Err(format!(
                    "'{flag}' is not a known mount option; expected ro, rw, z, Z, nocopy or a propagation mode"
                ));
            }
        }
    }

    Ok(match source {
        Some(s)
            if !s.starts_with('/')
                && !s.starts_with('.')
                && !s.starts_with('~')
                && !s.starts_with('$')
                && !s.is_empty() =>
        {
            Some(s.to_string())
        }
        _ => None,
    })
}

// --- networks --------------------------------------------------------------

fn check_networks(service: &str, svc: &Node, declared: &HashSet<&str>, f: &mut Findings) {
    if let Some((mode_pos, mode)) = svc.entry("network_mode") {
        if svc.get("networks").is_some() {
            f.add(
                "network-mode-conflict",
                mode_pos,
                format!("service '{service}' sets both 'network_mode:' and 'networks:' — Compose rejects that combination; keep one"),
            );
        }
        if mode.as_str() == Some("host") {
            f.add(
                "host-network",
                mode_pos,
                format!("service '{service}' uses 'network_mode: host', which removes network isolation and ignores published ports"),
            );
        }
    }

    let (nets_pos, nets) = match svc.entry("networks") {
        Some(v) => v,
        None => return,
    };
    if matches!(nets, Node::Scalar { .. }) {
        f.add(
            "undefined-network",
            nets_pos,
            format!("service '{service}': 'networks:' must be a list or mapping of network names, not a single scalar"),
        );
        return;
    }

    for (name, pos) in nets.names() {
        if name == "default" || name.contains("${") || declared.contains(name.as_str()) {
            continue;
        }
        let mut known: Vec<&str> = declared.iter().copied().collect();
        known.sort_unstable();
        let tail = if known.is_empty() {
            "no top-level 'networks:' key exists yet".to_string()
        } else {
            format!("declared networks are: {}", known.join(", "))
        };
        f.add(
            "undefined-network",
            pos,
            format!("service '{service}' joins network '{name}', which is not declared under the top-level 'networks:' key — {tail}"),
        );
    }
}

// --- environment -----------------------------------------------------------

fn check_environment(service: &str, svc: &Node, f: &mut Findings) {
    let (env_pos, env) = match svc.entry("environment") {
        Some(v) => v,
        None => return,
    };

    let pairs: Vec<(String, Option<String>, Pos)> = match env {
        Node::Map { entries, .. } => entries
            .iter()
            .map(|(k, p, v)| (k.clone(), v.as_str().map(str::to_string), *p))
            .collect(),
        Node::Seq { items, .. } => {
            let mut out = Vec::new();
            for item in items {
                match item.as_str() {
                    Some(s) => match s.split_once('=') {
                        Some((k, v)) => {
                            out.push((k.trim().to_string(), Some(v.to_string()), item.pos()))
                        }
                        // A bare NAME passes the host value through; that is legal.
                        None => out.push((s.trim().to_string(), None, item.pos())),
                    },
                    None => f.add(
                        "environment-syntax",
                        item.pos(),
                        format!("service '{service}': list-form 'environment:' entries must be strings like \"KEY=value\", got {}", item.kind()),
                    ),
                }
            }
            out
        }
        other => {
            f.add(
                "environment-syntax",
                env_pos,
                format!("service '{service}': 'environment:' must be a mapping or a list of KEY=value strings, got {}", other.kind()),
            );
            return;
        }
    };

    for (key, value, pos) in pairs {
        let Some(value) = value else { continue };
        let trimmed = value.trim();
        if trimmed.is_empty() || trimmed.contains("${") || trimmed.starts_with('$') {
            continue;
        }
        let upper = key.to_ascii_uppercase();
        if SECRET_HINTS.iter().any(|h| upper.contains(h)) {
            f.add(
                "env-secrets",
                pos,
                format!("service '{service}' hard-codes a literal value for '{key}' — move it to an env file, an interpolated ${{{upper}}} variable or a Compose secret"),
            );
        }
    }
}

// --- configs / secrets -----------------------------------------------------

fn check_config_secret_refs(
    service: &str,
    svc: &Node,
    key: &'static str,
    declared: &HashSet<&str>,
    f: &mut Findings,
) {
    let node = match svc.get(key) {
        Some(n) => n,
        None => return,
    };
    let items = match node.as_seq() {
        Some(items) => items,
        None => return,
    };
    let singular = key.trim_end_matches('s');

    for item in items {
        let (name, pos) = match item {
            Node::Scalar { value, pos, .. } => (value.clone(), *pos),
            Node::Map { .. } => match item.get("source").and_then(|n| n.as_str()) {
                Some(s) => (s.to_string(), item.pos()),
                None => continue,
            },
            _ => continue,
        };
        if name.contains("${") || declared.contains(name.as_str()) {
            continue;
        }
        f.add(
            "undefined-config-secret",
            pos,
            format!("service '{service}' uses {singular} '{name}', which is not declared under the top-level '{key}:' key"),
        );
    }
}

// --- depends_on ------------------------------------------------------------

fn check_depends_on(
    service: &str,
    svc: &Node,
    service_names: &HashSet<&str>,
    f: &mut Findings,
) -> Vec<String> {
    let (dep_pos, deps) = match svc.entry("depends_on") {
        Some(v) => v,
        None => return Vec::new(),
    };

    let mut out = Vec::new();

    match deps {
        Node::Seq { items, .. } => {
            for item in items {
                match item.as_str() {
                    Some(name) => {
                        record_dep(service, name, item.pos(), service_names, &mut out, f);
                    }
                    None => f.add(
                        "depends-on-syntax",
                        item.pos(),
                        format!("service '{service}': list-form 'depends_on:' entries must be service names, got {}", item.kind()),
                    ),
                }
            }
        }
        Node::Map { entries, .. } => {
            for (name, pos, cfg) in entries {
                record_dep(service, name, *pos, service_names, &mut out, f);
                match cfg.get("condition").and_then(|n| n.as_str()) {
                    None => {}
                    Some(c) if DEPENDS_ON_CONDITIONS.contains(&c) => {}
                    Some(c) => f.add(
                        "depends-on-syntax",
                        cfg.entry("condition").map(|(p, _)| p).unwrap_or(*pos),
                        format!(
                            "service '{service}': depends_on condition '{c}' is not valid; expected one of {}",
                            DEPENDS_ON_CONDITIONS.join(", ")
                        ),
                    ),
                }
            }
        }
        other => f.add(
            "depends-on-syntax",
            dep_pos,
            format!("service '{service}': 'depends_on:' must be a list of service names or a mapping with conditions, got {}", other.kind()),
        ),
    }

    out
}

fn record_dep(
    service: &str,
    name: &str,
    pos: Pos,
    service_names: &HashSet<&str>,
    out: &mut Vec<String>,
    f: &mut Findings,
) {
    out.push(name.to_string());
    if service_names.contains(name) || name.contains("${") {
        return;
    }
    let mut known: Vec<&str> = service_names.iter().copied().collect();
    known.sort_unstable();
    f.add(
        "undefined-depends-on",
        pos,
        format!("service '{service}' depends on '{name}', which is not defined under 'services:' — defined services are: {}", known.join(", ")),
    );
}

/// Report every `depends_on` cycle once, naming the services on the path.
fn check_cycles(
    graph: &BTreeMap<String, Vec<String>>,
    services_node: &Node,
    service_entries: &[(String, Pos, Node)],
    f: &mut Findings,
) {
    if !f.on("circular-depends-on") {
        return;
    }
    let pos_of = |name: &str| -> Pos {
        service_entries
            .iter()
            .find(|(n, _, _)| n == name)
            .map(|(_, p, _)| *p)
            .unwrap_or(services_node.pos())
    };

    let mut state: HashMap<&str, u8> = HashMap::new(); // 0 unseen, 1 on stack, 2 done
    let mut path: Vec<&str> = Vec::new();
    let mut reported: HashSet<String> = HashSet::new();

    fn visit<'a>(
        node: &'a str,
        graph: &'a BTreeMap<String, Vec<String>>,
        state: &mut HashMap<&'a str, u8>,
        path: &mut Vec<&'a str>,
        reported: &mut HashSet<String>,
        found: &mut Vec<(String, String)>,
    ) {
        state.insert(node, 1);
        path.push(node);
        for dep in graph.get(node).map(|v| v.as_slice()).unwrap_or(&[]) {
            let Some((dep_key, _)) = graph.get_key_value(dep.as_str()) else {
                continue;
            };
            match state.get(dep_key.as_str()).copied().unwrap_or(0) {
                0 => visit(dep_key, graph, state, path, reported, found),
                1 => {
                    // Cycle: the slice of `path` from dep onwards.
                    let start = path.iter().position(|n| *n == dep_key.as_str()).unwrap_or(0);
                    let mut cycle: Vec<&str> = path[start..].to_vec();
                    cycle.push(dep_key.as_str());
                    // Canonical key so one cycle is reported once.
                    let mut sorted = cycle.clone();
                    sorted.sort_unstable();
                    sorted.dedup();
                    let key = sorted.join(">");
                    if reported.insert(key) {
                        found.push((cycle[0].to_string(), cycle.join(" -> ")));
                    }
                }
                _ => {}
            }
        }
        path.pop();
        state.insert(node, 2);
    }

    let mut found: Vec<(String, String)> = Vec::new();
    for name in graph.keys() {
        if state.get(name.as_str()).copied().unwrap_or(0) == 0 {
            visit(name, graph, &mut state, &mut path, &mut reported, &mut found);
        }
    }

    for (start, chain) in found {
        f.add(
            "circular-depends-on",
            pos_of(&start),
            format!("circular 'depends_on' dependency: {chain} — Compose cannot decide a start order, so break the loop or use a healthcheck condition"),
        );
    }
}

// --- restart, security, hints ----------------------------------------------

fn check_restart(service: &str, name_pos: Pos, svc: &Node, f: &mut Findings) {
    match svc.entry("restart") {
        Some((pos, node)) => {
            if let Some(value) = node.as_str() {
                let v = value.trim();
                let valid = matches!(v, "no" | "always" | "unless-stopped" | "on-failure")
                    || v.strip_prefix("on-failure:")
                        .map(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
                        .unwrap_or(false)
                    || v.contains("${");
                if !valid {
                    f.add(
                        "restart-policy",
                        pos,
                        format!("service '{service}': restart policy '{v}' is not valid; expected no, always, unless-stopped, on-failure or on-failure:N"),
                    );
                }
            }
        }
        None => {
            if svc.get("deploy").and_then(|d| d.get("restart_policy")).is_none() {
                f.add(
                    "missing-restart",
                    name_pos,
                    format!("service '{service}' has no 'restart:' policy — it will stay down after a crash or a host reboot"),
                );
            }
        }
    }
}

fn check_security(service: &str, svc: &Node, f: &mut Findings) {
    if let Some((pos, node)) = svc.entry("privileged") {
        if node.is_true() {
            f.add(
                "privileged",
                pos,
                format!("service '{service}' runs privileged, which gives the container full host device access — grant individual capabilities with 'cap_add:' instead"),
            );
        }
    }
    if let Some((pos, _)) = svc.entry("links") {
        f.add(
            "deprecated-links",
            pos,
            format!("service '{service}' uses the legacy 'links:' key — services on a shared network already reach each other by name"),
        );
    }
}

fn check_hints(service: &str, name_pos: Pos, svc: &Node, f: &mut Findings) {
    if svc.get("healthcheck").is_none() {
        f.add(
            "missing-healthcheck",
            name_pos,
            format!("service '{service}' has no 'healthcheck:' — dependents using 'condition: service_healthy' cannot wait for it"),
        );
    }
    let has_limits = svc
        .get("deploy")
        .and_then(|d| d.get("resources"))
        .and_then(|r| r.get("limits"))
        .is_some()
        || svc.get("mem_limit").is_some()
        || svc.get("cpus").is_some();
    if !has_limits {
        f.add(
            "resource-limits",
            name_pos,
            format!("service '{service}' sets no memory or CPU limit — one runaway container can starve the host"),
        );
    }
    if svc.get("logging").is_none() {
        f.add(
            "logging-options",
            name_pos,
            format!("service '{service}' has no 'logging:' options — the default json-file driver grows without bound unless max-size is set"),
        );
    }
}

// ---------------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------------

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("{n} {one}")
    } else {
        format!("{n} {many}")
    }
}

/// Human-readable report: a summary line, then `line:col severity rule message`.
pub fn format_report(report: &Report) -> String {
    let errors = report.count(Severity::Error);
    let warnings = report.count(Severity::Warning);
    let hints = report.count(Severity::Hint);

    let mut out = String::new();
    out.push_str(&format!(
        "{} — {}, {}, {}\n",
        if report.ok() { "VALID" } else { "INVALID" },
        plural(report.services, "service", "services"),
        plural(report.networks, "network", "networks"),
        plural(report.volumes, "volume", "volumes"),
    ));
    out.push_str(&format!(
        "preset {} — {}, {}, {}\n",
        report.preset.as_str(),
        plural(errors, "error", "errors"),
        plural(warnings, "warning", "warnings"),
        plural(hints, "hint", "hints"),
    ));

    if report.problems.is_empty() {
        out.push('\n');
        out.push_str("No problems found.\n");
        return out;
    }

    let width = report
        .problems
        .iter()
        .map(|p| format!("{}:{}", p.line, p.column).len())
        .max()
        .unwrap_or(4);
    let rule_width = report.problems.iter().map(|p| p.rule.len()).max().unwrap_or(4);

    out.push('\n');
    for p in &report.problems {
        let loc = format!("{}:{}", p.line, p.column);
        out.push_str(&format!(
            "{loc:<width$}  {:<7}  {:<rule_width$}  {}\n",
            p.severity.as_str(),
            p.rule,
            p.message
        ));
    }
    if report.truncated {
        out.push_str(&format!(
            "\n… report truncated at {MAX_PROBLEMS} problems.\n"
        ));
    }
    out
}

/// Machine-readable report for CI.
pub fn format_json(report: &Report) -> String {
    let problems: Vec<_> = report
        .problems
        .iter()
        .map(|p| {
            json!({
                "line": p.line,
                "column": p.column,
                "severity": p.severity.as_str(),
                "rule": p.rule,
                "message": p.message,
            })
        })
        .collect();

    let value = json!({
        "valid": report.ok(),
        "preset": report.preset.as_str(),
        "summary": {
            "services": report.services,
            "networks": report.networks,
            "volumes": report.volumes,
            "errors": report.count(Severity::Error),
            "warnings": report.count(Severity::Warning),
            "hints": report.count(Severity::Hint),
            "truncated": report.truncated,
        },
        "problems": problems,
    });
    serde_json::to_string_pretty(&value).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn lint(input: &str, preset: &str) -> Report {
        let opts = Options {
            preset: Preset::parse(preset).unwrap(),
            ..Options::default()
        };
        validate(input, &opts).unwrap()
    }

    fn rules(report: &Report) -> Vec<&str> {
        report.problems.iter().map(|p| p.rule).collect()
    }

    const CLEAN: &str = r#"name: demo
services:
  web:
    image: nginx:1.27-alpine
    restart: unless-stopped
    ports:
      - "127.0.0.1:8080:80"
    depends_on:
      - api
    networks:
      - front
  api:
    image: ghcr.io/acme/api:2.3.1
    restart: unless-stopped
    volumes:
      - dbdata:/var/lib/data
    networks:
      - front
networks:
  front:
volumes:
  dbdata:
"#;

    #[test]
    fn happy_path_clean_file_has_no_findings() {
        let report = lint(CLEAN, "default");
        assert!(report.ok(), "expected valid, got {:?}", report.problems);
        assert_eq!(report.problems, vec![]);
        assert_eq!(report.services, 2);
        assert_eq!(report.networks, 1);
        assert_eq!(report.volumes, 1);
        let text = format_report(&report);
        assert!(text.starts_with("VALID — 2 services, 1 network, 1 volume\n"));
        assert!(text.contains("No problems found."));
    }

    #[test]
    fn error_syntax_failure_is_positioned() {
        let report = lint("services:\n  web:\n   image: a\n  \tbad: 1\n", "default");
        assert!(!report.parsed);
        assert_eq!(rules(&report), vec!["syntax"]);
        assert!(report.problems[0].line >= 1);
        assert!(!report.ok());
    }

    #[test]
    fn error_empty_input_is_rejected() {
        let err = validate("   \n", &Options::default()).unwrap_err();
        assert!(err.contains("empty"), "{err}");
    }

    #[test]
    fn undefined_volume_network_and_service_references() {
        let src = r#"services:
  web:
    image: nginx:1.27
    volumes:
      - dbdata:/data
    networks:
      - backend
    depends_on:
      - missing
"#;
        let report = lint(src, "essential");
        let found = rules(&report);
        assert!(found.contains(&"undefined-volume"), "{found:?}");
        assert!(found.contains(&"undefined-network"), "{found:?}");
        assert!(found.contains(&"undefined-depends-on"), "{found:?}");
        let msg = &report
            .problems
            .iter()
            .find(|p| p.rule == "undefined-volume")
            .unwrap()
            .message;
        assert!(msg.contains("dbdata"), "{msg}");
    }

    #[test]
    fn relative_and_absolute_bind_mounts_are_not_named_volumes() {
        let src = r#"services:
  web:
    image: nginx:1.27
    volumes:
      - ./site:/usr/share/nginx/html:ro
      - /var/run/docker.sock:/var/run/docker.sock
      - /data
"#;
        let report = lint(src, "essential");
        assert_eq!(rules(&report), Vec::<&str>::new());
    }

    #[test]
    fn volume_errors_report_bad_target_and_mode() {
        let src = r#"services:
  web:
    image: nginx:1.27
    volumes:
      - data:relative/path
      - other:/data:rx
"#;
        let report = lint(src, "essential");
        let msgs: Vec<&str> = report.problems.iter().map(|p| p.message.as_str()).collect();
        assert!(msgs.iter().any(|m| m.contains("must be absolute")), "{msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("not a known mount option")), "{msgs:?}");
    }

    #[test]
    fn port_forms_that_must_be_accepted() {
        for good in [
            "3000",
            "3000-3005",
            "8000:8000",
            "9090-9091:8080-8081",
            "127.0.0.1:8001:8001",
            "127.0.0.1:5000-5010:5000-5010",
            "6060:6060/udp",
            "[::1]:6000:6000",
            ":8080",
        ] {
            assert!(parse_short_port(good).is_ok(), "expected '{good}' to parse");
        }
    }

    #[test]
    fn port_forms_that_must_be_rejected() {
        for (bad, needle) in [
            ("8080:80:90:100", "expected CONTAINER"),
            ("70000:80", "1-65535"),
            ("0:80", "1-65535"),
            ("8080:", "is empty"),
            ("abc:80", "not a number"),
            ("8080:80/icmp", "not a known protocol"),
            ("9000-9001:80", "same width"),
            ("9005-9001:80", "backwards"),
            ("[::18000:80", "closing ']'"),
            ("[::1]8000:80", "must be followed by ':'"),
        ] {
            let err = parse_short_port(bad).unwrap_err();
            assert!(err.contains(needle), "'{bad}' gave '{err}', wanted '{needle}'");
        }
    }

    #[test]
    fn duplicate_host_port_across_services() {
        let src = r#"services:
  a:
    image: nginx:1.27
    ports:
      - "8080:80"
  b:
    image: httpd:2.4
    ports:
      - "8080:8080"
"#;
        let report = lint(src, "essential");
        let dup = report
            .problems
            .iter()
            .find(|p| p.rule == "duplicate-host-port")
            .expect("expected a duplicate-host-port finding");
        assert!(dup.message.contains("8080"), "{}", dup.message);
        assert!(dup.message.contains("'a'"), "{}", dup.message);
    }

    #[test]
    fn duplicate_container_names_are_reported_once() {
        let src = r#"services:
  a:
    image: nginx:1.27
    container_name: web
  b:
    image: httpd:2.4
    container_name: web
"#;
        let report = lint(src, "essential");
        assert_eq!(
            rules(&report)
                .iter()
                .filter(|r| **r == "duplicate-container-name")
                .count(),
            1
        );
    }

    #[test]
    fn dependency_cycle_is_reported_once_with_the_path() {
        let src = r#"services:
  a:
    image: nginx:1.27
    depends_on: [b]
  b:
    image: nginx:1.27
    depends_on: [c]
  c:
    image: nginx:1.27
    depends_on: [a]
"#;
        let report = lint(src, "essential");
        let cycles: Vec<&Problem> = report
            .problems
            .iter()
            .filter(|p| p.rule == "circular-depends-on")
            .collect();
        assert_eq!(cycles.len(), 1, "{:?}", rules(&report));
        assert!(cycles[0].message.contains("->"), "{}", cycles[0].message);
    }

    #[test]
    fn self_dependency_is_a_cycle() {
        let src = "services:\n  a:\n    image: nginx:1.27\n    depends_on: [a]\n";
        let report = lint(src, "essential");
        assert!(rules(&report).contains(&"circular-depends-on"));
    }

    #[test]
    fn depends_on_condition_is_validated() {
        let src = r#"services:
  a:
    image: nginx:1.27
    depends_on:
      b:
        condition: service_ready
  b:
    image: nginx:1.27
"#;
        let report = lint(src, "essential");
        let p = report
            .problems
            .iter()
            .find(|p| p.rule == "depends-on-syntax")
            .unwrap();
        assert!(p.message.contains("service_healthy"), "{}", p.message);
    }

    #[test]
    fn image_or_build_missing_and_both_present() {
        let missing = lint("services:\n  a:\n    restart: always\n", "essential");
        assert!(rules(&missing).contains(&"image-or-build"));

        let both = lint(
            "services:\n  a:\n    build: .\n    image: acme/app:1.0\n",
            "default",
        );
        assert!(rules(&both).contains(&"build-and-image"));
    }

    #[test]
    fn floating_and_missing_image_tags() {
        let src = r#"services:
  a:
    image: nginx
  b:
    image: nginx:latest
  c:
    image: registry.example.com:5000/team/app:1.4.2
  d:
    image: nginx@sha256:0000000000000000000000000000000000000000000000000000000000000000
"#;
        let report = lint(src, "default");
        let tag_hits: Vec<&Problem> = report
            .problems
            .iter()
            .filter(|p| p.rule == "image-tag")
            .collect();
        assert_eq!(tag_hits.len(), 2, "{tag_hits:?}");
        assert!(tag_hits[0].message.contains("no tag"));
        assert!(tag_hits[1].message.contains(":latest"));
    }

    #[test]
    fn obsolete_version_field_and_legacy_links() {
        let src = "version: \"3.8\"\nservices:\n  a:\n    image: nginx:1.27\n    links:\n      - b\n  b:\n    image: nginx:1.27\n";
        let report = lint(src, "default");
        let found = rules(&report);
        assert!(found.contains(&"version-field"), "{found:?}");
        assert!(found.contains(&"deprecated-links"), "{found:?}");
    }

    #[test]
    fn unquoted_port_mapping_is_flagged_but_quoted_is_not() {
        let unquoted = lint(
            "services:\n  a:\n    image: nginx:1.27\n    ports:\n      - 8080:80\n",
            "default",
        );
        assert!(rules(&unquoted).contains(&"quote-ports"));

        let quoted = lint(
            "services:\n  a:\n    image: nginx:1.27\n    ports:\n      - \"8080:80\"\n",
            "default",
        );
        assert!(!rules(&quoted).contains(&"quote-ports"));
    }

    #[test]
    fn hard_coded_secrets_flagged_interpolated_ones_are_not() {
        let src = r#"services:
  db:
    image: postgres:16
    environment:
      POSTGRES_PASSWORD: hunter2
      API_TOKEN: ${API_TOKEN}
      LOG_LEVEL: debug
"#;
        let report = lint(src, "default");
        let hits: Vec<&Problem> = report
            .problems
            .iter()
            .filter(|p| p.rule == "env-secrets")
            .collect();
        assert_eq!(hits.len(), 1, "{hits:?}");
        assert!(hits[0].message.contains("POSTGRES_PASSWORD"));
    }

    #[test]
    fn privileged_and_host_network_are_warnings() {
        let src = "services:\n  a:\n    image: nginx:1.27\n    privileged: true\n    network_mode: host\n";
        let report = lint(src, "default");
        let found = rules(&report);
        assert!(found.contains(&"privileged"), "{found:?}");
        assert!(found.contains(&"host-network"), "{found:?}");
    }

    #[test]
    fn network_mode_and_networks_together_is_an_error() {
        let src = "services:\n  a:\n    image: nginx:1.27\n    network_mode: bridge\n    networks: [default]\n";
        let report = lint(src, "essential");
        assert!(rules(&report).contains(&"network-mode-conflict"));
    }

    #[test]
    fn invalid_restart_policy_value() {
        let bad = lint("services:\n  a:\n    image: nginx:1.27\n    restart: sometimes\n", "essential");
        assert!(rules(&bad).contains(&"restart-policy"));

        let good = lint("services:\n  a:\n    image: nginx:1.27\n    restart: on-failure:5\n", "essential");
        assert!(!rules(&good).contains(&"restart-policy"));
    }

    #[test]
    fn anchors_aliases_and_merge_keys_resolve() {
        let src = r#"x-common: &common
  image: nginx:1.27
  restart: unless-stopped
services:
  a:
    <<: *common
  b:
    <<: *common
    container_name: b
"#;
        let report = lint(src, "default");
        let found = rules(&report);
        assert!(!found.contains(&"image-or-build"), "{found:?}");
        assert!(!found.contains(&"missing-restart"), "{found:?}");
    }

    #[test]
    fn strict_preset_adds_hints_default_does_not() {
        let src = "services:\n  a:\n    image: nginx:1.27\n    ports:\n      - \"8080:80\"\n";
        let default_report = lint(src, "default");
        let default = rules(&default_report);
        assert!(!default.contains(&"missing-healthcheck"), "{default:?}");
        assert!(!default.contains(&"unbound-port-interface"), "{default:?}");

        let strict_report = lint(src, "strict");
        let strict = rules(&strict_report);
        for expected in [
            "missing-healthcheck",
            "missing-restart",
            "resource-limits",
            "logging-options",
            "unbound-port-interface",
            "project-name",
        ] {
            assert!(strict.contains(&expected), "strict missing {expected}: {strict:?}");
        }
    }

    #[test]
    fn unknown_keys_are_flagged_at_the_right_level() {
        let src = "servics:\n  a:\n    image: nginx:1.27\n";
        let report = lint(src, "default");
        let found = rules(&report);
        assert!(found.contains(&"unknown-top-level-key"), "{found:?}");
        assert!(found.contains(&"services-missing"), "{found:?}");

        let svc = lint(
            "services:\n  a:\n    image: nginx:1.27\n    portz:\n      - \"80:80\"\n",
            "strict",
        );
        assert!(rules(&svc).contains(&"unknown-service-key"));
    }

    #[test]
    fn extension_keys_are_always_accepted() {
        let src = "x-shared: &s\n  a: 1\nservices:\n  a:\n    image: nginx:1.27\n    x-note: hello\n";
        let report = lint(src, "strict");
        let found = rules(&report);
        assert!(!found.contains(&"unknown-top-level-key"), "{found:?}");
        assert!(!found.contains(&"unknown-service-key"), "{found:?}");
    }

    #[test]
    fn disable_switches_a_rule_off_and_rejects_typos() {
        let opts = Options {
            preset: Preset::Default,
            disabled: parse_disabled("image-tag, version-field").unwrap(),
            ..Options::default()
        };
        let src = "version: \"3\"\nservices:\n  a:\n    image: nginx\n";
        let report = validate(src, &opts).unwrap();
        let found = rules(&report);
        assert!(!found.contains(&"image-tag"), "{found:?}");
        assert!(!found.contains(&"version-field"), "{found:?}");

        let err = parse_disabled("image-tags").unwrap_err();
        assert!(err.contains("unknown rule id 'image-tags'"), "{err}");
    }

    #[test]
    fn strict_warnings_promotes_and_min_severity_filters() {
        let src = "version: \"3\"\nservices:\n  a:\n    image: nginx:1.27\n";
        let promoted = validate(
            src,
            &Options {
                strict_warnings: true,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(promoted.count(Severity::Error), 1);
        assert!(!promoted.ok());

        let filtered = validate(
            src,
            &Options {
                min_severity: Severity::Error,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(filtered.problems, vec![]);
    }

    #[test]
    fn json_output_shape() {
        let out = run(CLEAN, "default", "", false, "hint", "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["valid"], true);
        assert_eq!(v["preset"], "default");
        assert_eq!(v["summary"]["services"], 2);
        assert_eq!(v["summary"]["errors"], 0);
        assert_eq!(v["problems"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn string_entry_point_matches_typed_one() {
        let a = run(CLEAN, "strict", "project-name", true, "warning", "report").unwrap();
        let b = run_str(CLEAN, "strict", "project-name", "true", "warning", "report").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn bad_option_values_are_rejected_with_guidance() {
        assert!(Preset::parse("loose").unwrap_err().contains("essential"));
        assert!(Severity::parse("fatal").unwrap_err().contains("warning"));
        let err = run(CLEAN, "default", "", false, "hint", "yaml").unwrap_err();
        assert!(err.contains("report or json"), "{err}");
    }

    #[test]
    fn input_larger_than_the_cap_is_rejected() {
        let big = "#".repeat(MAX_INPUT_BYTES + 1);
        let err = validate(&big, &Options::default()).unwrap_err();
        assert!(err.contains("maximum"), "{err}");
    }

    #[test]
    fn long_syntax_ports_and_volumes_are_understood() {
        let src = r#"services:
  a:
    image: nginx:1.27
    ports:
      - target: 80
        published: "8080"
        host_ip: 127.0.0.1
        protocol: tcp
        mode: host
    volumes:
      - type: volume
        source: dbdata
        target: /var/lib/data
"#;
        let report = lint(src, "essential");
        assert!(rules(&report).contains(&"undefined-volume"), "{:?}", rules(&report));
        assert!(!rules(&report).contains(&"port-syntax"), "{:?}", rules(&report));
    }

    #[test]
    fn top_level_must_be_a_mapping() {
        let report = lint("- a\n- b\n", "default");
        assert_eq!(rules(&report), vec!["top-level-type"]);
    }

    #[test]
    fn report_lines_carry_location_severity_and_rule() {
        let src = "services:\n  a:\n    image: nginx:1.27\n    ports:\n      - \"99999:80\"\n";
        let text = format_report(&lint(src, "essential"));
        assert!(text.starts_with("INVALID —"), "{text}");
        assert!(text.contains("5:9"), "{text}");
        assert!(text.contains("error"), "{text}");
        assert!(text.contains("port-syntax"), "{text}");
    }
}

//! Dependency risk auditor — pure, offline analysis of an npm `package.json`
//! or lockfile for risky supply-chain patterns.
//!
//! Everything here is deterministic compute over the pasted file contents: no
//! registry lookups, no filesystem walk, no network. That bounds what can be
//! detected to what the file itself records — version specs, install/lifecycle
//! scripts, resolved URLs, integrity hashes and declared-vs-locked agreement.

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

/// Largest accepted input per field. Real lockfiles for very large monorepos
/// can run into the megabytes; 2 MiB covers them while keeping the browser
/// responsive.
pub const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;

/// Hard cap on emitted findings so a pathological lockfile can't produce an
/// unbounded report. Truncation is reported, never silent.
pub const MAX_FINDINGS: usize = 1000;

/// Node.js built-in module names. A dependency with one of these names either
/// shadows the built-in or is a legitimate browser shim, so this is a low
/// severity hint rather than an error.
const NODE_BUILTINS: &[&str] = &[
    "assert",
    "async_hooks",
    "buffer",
    "child_process",
    "cluster",
    "console",
    "constants",
    "crypto",
    "dgram",
    "diagnostics_channel",
    "dns",
    "domain",
    "events",
    "fs",
    "http",
    "http2",
    "https",
    "inspector",
    "module",
    "net",
    "os",
    "path",
    "perf_hooks",
    "process",
    "punycode",
    "querystring",
    "readline",
    "repl",
    "stream",
    "string_decoder",
    "sys",
    "timers",
    "tls",
    "trace_events",
    "tty",
    "url",
    "util",
    "v8",
    "vm",
    "wasi",
    "worker_threads",
    "zlib",
];

/// npm lifecycle scripts that run automatically on `npm install`.
const INSTALL_SCRIPTS: &[&str] = &["preinstall", "install", "postinstall"];

/// Lifecycle scripts that run on other common workflows (`npm install` in a
/// git checkout, `npm publish`, `npm pack`).
const OTHER_LIFECYCLE_SCRIPTS: &[&str] = &[
    "prepare",
    "prepublish",
    "prepublishOnly",
    "prepack",
    "postpack",
    "postpublish",
    "preuninstall",
    "uninstall",
    "postuninstall",
];

/// Hosts treated as the canonical public npm registry.
const PUBLIC_REGISTRY_HOSTS: &[&str] = &["registry.npmjs.org", "registry.yarnpkg.com"];

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Severity {
    Info,
    Low,
    Medium,
    High,
}

impl Severity {
    fn as_str(self) -> &'static str {
        match self {
            Severity::High => "high",
            Severity::Medium => "medium",
            Severity::Low => "low",
            Severity::Info => "info",
        }
    }

    /// Score penalty applied to the 0-100 project score.
    fn weight(self) -> u32 {
        match self {
            Severity::High => 20,
            Severity::Medium => 8,
            Severity::Low => 3,
            Severity::Info => 1,
        }
    }

    fn parse(s: &str) -> Option<Severity> {
        match s.trim().to_ascii_lowercase().as_str() {
            "high" => Some(Severity::High),
            "medium" => Some(Severity::Medium),
            "low" => Some(Severity::Low),
            "info" => Some(Severity::Info),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Finding {
    pub rule: &'static str,
    pub severity: Severity,
    /// Package or script the finding is about (empty for document-level rules).
    pub subject: String,
    /// Where it was found: a package.json section, "scripts", "lockfile", ...
    pub location: String,
    /// The offending value (version spec, resolved URL, ...).
    pub value: String,
    pub message: String,
}

/// One parsed dependency declaration from a `package.json`.
struct Declared {
    name: String,
    spec: String,
    section: &'static str,
}

/// One parsed entry from a lockfile.
struct Locked {
    name: String,
    version: String,
    resolved: String,
    integrity: String,
    /// npm `hasInstallScript` / pnpm `requiresBuild`.
    install_script: bool,
    /// The entry resolves to something other than a registry tarball (a
    /// workspace, directory or git resolution), so integrity rules don't apply.
    local: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    PackageJson,
    PackageLock,
    YarnLock,
    PnpmLock,
}

impl Format {
    fn as_str(self) -> &'static str {
        match self {
            Format::PackageJson => "package-json",
            Format::PackageLock => "package-lock",
            Format::YarnLock => "yarn-lock",
            Format::PnpmLock => "pnpm-lock",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Format::PackageJson => "package.json",
            Format::PackageLock => "package-lock.json",
            Format::YarnLock => "yarn.lock",
            Format::PnpmLock => "pnpm-lock.yaml",
        }
    }

    fn parse(s: &str) -> Option<Format> {
        match s.trim().to_ascii_lowercase().as_str() {
            "package-json" | "package.json" => Some(Format::PackageJson),
            "package-lock" | "package-lock.json" => Some(Format::PackageLock),
            "yarn-lock" | "yarn.lock" => Some(Format::YarnLock),
            "pnpm-lock" | "pnpm-lock.yaml" => Some(Format::PnpmLock),
            _ => None,
        }
    }

    fn is_lockfile(self) -> bool {
        !matches!(self, Format::PackageJson)
    }
}

/// Detect the input format from its content.
pub fn detect_format(text: &str) -> Result<Format, String> {
    let trimmed = text.trim_start();
    if trimmed.starts_with('{') {
        let v: Value = serde_json::from_str(trimmed)
            .map_err(|e| format!("input looks like JSON but did not parse: {e}"))?;
        if v.get("lockfileVersion").is_some() {
            return Ok(Format::PackageLock);
        }
        return Ok(Format::PackageJson);
    }
    if trimmed.contains("# yarn lockfile v1") || trimmed.contains("__metadata:") {
        return Ok(Format::YarnLock);
    }
    if trimmed.contains("lockfileVersion:") {
        return Ok(Format::PnpmLock);
    }
    // A bare yarn v1 lockfile with its header comment stripped still has
    // `name@range:` keys followed by an indented `version "x"` line.
    if trimmed.lines().any(|l| l.trim_start().starts_with("version \"")) {
        return Ok(Format::YarnLock);
    }
    Err("could not detect the input format — expected a package.json, package-lock.json, \
         yarn.lock or pnpm-lock.yaml; set manifest_format explicitly"
        .to_string())
}

fn check_size(label: &str, text: &str) -> Result<(), String> {
    if text.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "{label} is too large ({} bytes, maximum {MAX_INPUT_BYTES})",
            text.len()
        ));
    }
    Ok(())
}

/// Split a comma- or newline-separated list into trimmed, non-empty entries.
fn split_list(s: &str) -> Vec<String> {
    s.split([',', '\n', ';'])
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

// ---------------------------------------------------------------------------
// package.json parsing
// ---------------------------------------------------------------------------

fn dep_sections(include_dev: bool) -> Vec<&'static str> {
    let mut v = vec!["dependencies"];
    if include_dev {
        v.push("devDependencies");
    }
    v.push("optionalDependencies");
    v.push("peerDependencies");
    v
}

fn parse_package_json(text: &str) -> Result<Value, String> {
    serde_json::from_str::<Value>(text).map_err(|e| format!("package.json did not parse: {e}"))
}

fn declared_deps(pkg: &Value, include_dev: bool) -> Vec<Declared> {
    let mut out = Vec::new();
    for section in dep_sections(include_dev) {
        let Some(map) = pkg.get(section).and_then(|v| v.as_object()) else {
            continue;
        };
        for (name, spec) in map {
            out.push(Declared {
                name: name.clone(),
                spec: spec.as_str().unwrap_or_default().to_string(),
                section,
            });
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Version-spec classification
// ---------------------------------------------------------------------------

#[derive(PartialEq, Eq, Debug)]
enum SpecKind {
    Wildcard,
    DistTag,
    Git,
    Url,
    HttpUrl,
    LocalPath,
    Alias,
    Workspace,
    Prerelease,
    RangePrefix,
    Exact,
    Comparator,
}

fn classify_spec(spec: &str) -> SpecKind {
    let s = spec.trim();
    let lower = s.to_ascii_lowercase();

    if s.is_empty() || s == "*" || s == "x" || s == "X" || lower == ">=0.0.0" || lower == "latest" {
        return SpecKind::Wildcard;
    }
    if lower.starts_with("npm:") {
        return SpecKind::Alias;
    }
    if lower.starts_with("workspace:") {
        return SpecKind::Workspace;
    }
    if lower.starts_with("file:") || lower.starts_with("link:") || lower.starts_with("portal:") {
        return SpecKind::LocalPath;
    }
    if lower.starts_with("git+")
        || lower.starts_with("git:")
        || lower.starts_with("github:")
        || lower.starts_with("gitlab:")
        || lower.starts_with("bitbucket:")
        || lower.starts_with("gist:")
        || lower.starts_with("ssh://")
        || lower.ends_with(".git")
        || lower.contains(".git#")
    {
        return SpecKind::Git;
    }
    if lower.starts_with("http://") {
        return SpecKind::HttpUrl;
    }
    if lower.starts_with("https://") {
        return SpecKind::Url;
    }
    // `user/repo` or `user/repo#branch` GitHub shorthand — a slash with no
    // leading scheme and no version characters.
    if s.contains('/') && !s.starts_with('@') && !s.contains(' ') && !s.starts_with('.') {
        let head = s.split('#').next().unwrap_or(s);
        let parts: Vec<&str> = head.split('/').collect();
        if parts.len() == 2
            && !parts[0].is_empty()
            && !parts[1].is_empty()
            && !head.starts_with('/')
        {
            return SpecKind::Git;
        }
    }
    // Multi-clause ranges before the pre-release check: a hyphen range
    // (`1.0.0 - 2.0.0`) and an `||` union both contain characters the
    // pre-release test would otherwise claim.
    if s.contains("||") || s.contains(" - ") || s.contains(' ') {
        return SpecKind::Comparator;
    }
    // `1.2.3-beta.1`, `^1.2.3-rc.0`: a semver core followed by a pre-release tag.
    let core = s.trim_start_matches(['^', '~', '=', '>', '<']);
    let starts_numeric = core.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false);
    if starts_numeric && core.contains('-') {
        return SpecKind::Prerelease;
    }
    if s.starts_with('^') || s.starts_with('~') {
        return SpecKind::RangePrefix;
    }
    if s.starts_with('>') || s.starts_with('<') {
        return SpecKind::Comparator;
    }
    if starts_numeric && !s.starts_with('=') {
        // `1.2.x` / `1.x` are open ranges, not exact pins.
        if s.contains(".x") || s.contains(".X") || s.contains('*') {
            return SpecKind::Wildcard;
        }
        return SpecKind::Exact;
    }
    if s.starts_with('=') {
        return SpecKind::Comparator;
    }
    // A bare word that is not a semver range is a dist-tag (`next`, `beta`).
    SpecKind::DistTag
}

// ---------------------------------------------------------------------------
// Lockfile parsing
// ---------------------------------------------------------------------------

/// npm `package-lock.json` v1 (`dependencies`) and v2/v3 (`packages`).
fn parse_package_lock(text: &str, include_dev: bool) -> Result<(Vec<Locked>, u64), String> {
    let v: Value =
        serde_json::from_str(text).map_err(|e| format!("package-lock.json did not parse: {e}"))?;
    let lockfile_version = v.get("lockfileVersion").and_then(|x| x.as_u64()).unwrap_or(0);
    let mut out = Vec::new();

    if let Some(packages) = v.get("packages").and_then(|x| x.as_object()) {
        for (path, entry) in packages {
            if path.is_empty() {
                continue; // the root project itself
            }
            let dev = entry.get("dev").and_then(|d| d.as_bool()).unwrap_or(false);
            if dev && !include_dev {
                continue;
            }
            let name = entry
                .get("name")
                .and_then(|n| n.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| {
                    // "node_modules/@scope/pkg" -> "@scope/pkg"
                    match path.rfind("node_modules/") {
                        Some(i) => path[i + "node_modules/".len()..].to_string(),
                        None => path.clone(),
                    }
                });
            let resolved = str_field(entry, "resolved");
            let link = entry.get("link").and_then(|b| b.as_bool()).unwrap_or(false);
            out.push(Locked {
                name,
                version: str_field(entry, "version"),
                local: link || resolved.is_empty() || resolved.starts_with("file:"),
                resolved,
                integrity: str_field(entry, "integrity"),
                install_script: entry
                    .get("hasInstallScript")
                    .and_then(|b| b.as_bool())
                    .unwrap_or(false),
            });
        }
    }

    if out.is_empty() {
        if let Some(deps) = v.get("dependencies").and_then(|x| x.as_object()) {
            collect_v1_deps(deps, include_dev, &mut out);
        }
    }
    Ok((out, lockfile_version))
}

/// lockfileVersion 1 nests transitive entries under each dependency.
fn collect_v1_deps(
    deps: &serde_json::Map<String, Value>,
    include_dev: bool,
    out: &mut Vec<Locked>,
) {
    for (name, entry) in deps {
        let dev = entry.get("dev").and_then(|d| d.as_bool()).unwrap_or(false);
        if !(dev && !include_dev) {
            let resolved = str_field(entry, "resolved");
            out.push(Locked {
                name: name.clone(),
                version: str_field(entry, "version"),
                local: resolved.is_empty() || resolved.starts_with("file:"),
                resolved,
                integrity: str_field(entry, "integrity"),
                install_script: entry
                    .get("hasInstallScript")
                    .and_then(|b| b.as_bool())
                    .unwrap_or(false),
            });
        }
        if let Some(nested) = entry.get("dependencies").and_then(|x| x.as_object()) {
            collect_v1_deps(nested, include_dev, out);
        }
    }
}

fn str_field(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(|x| x.as_str())
        .unwrap_or_default()
        .to_string()
}

/// Strip surrounding quotes and a trailing colon from a lockfile key/value.
fn unquote(s: &str) -> &str {
    let s = s.trim();
    let s = s.strip_suffix(':').unwrap_or(s);
    let s = s.trim();
    s.trim_matches('"').trim_matches('\'')
}

/// `chalk@^4.1.2` / `@scope/pkg@npm:^1.0.0` -> `chalk` / `@scope/pkg`.
fn name_from_descriptor(desc: &str) -> String {
    let d = desc.trim();
    let (prefix, rest) = if let Some(r) = d.strip_prefix('@') {
        ("@", r)
    } else {
        ("", d)
    };
    match rest.find('@') {
        Some(i) => format!("{prefix}{}", &rest[..i]),
        None => format!("{prefix}{rest}"),
    }
}

/// yarn.lock v1 (classic) and Berry (YAML). Both use column-0 descriptor keys
/// with indented `version` / `resolved`|`resolution` / `integrity`|`checksum`.
fn parse_yarn_lock(text: &str) -> Vec<Locked> {
    let mut out: Vec<Locked> = Vec::new();
    let mut current: Option<Locked> = None;
    for raw in text.lines() {
        let line = raw.trim_end();
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let indented = line.starts_with(' ') || line.starts_with('\t');
        if !indented {
            if let Some(entry) = current.take() {
                out.push(entry);
            }
            if !line.ends_with(':') {
                continue;
            }
            let key = line.trim_end_matches(':');
            if key == "__metadata" {
                continue;
            }
            // Multiple descriptors can share one entry: `a@^1, a@^1.2:`
            let first = key.split(',').next().unwrap_or(key);
            let name = name_from_descriptor(unquote(first));
            if name.is_empty() {
                continue;
            }
            current = Some(Locked {
                name,
                version: String::new(),
                resolved: String::new(),
                integrity: String::new(),
                install_script: false,
                local: false,
            });
            continue;
        }
        let Some(entry) = current.as_mut() else {
            continue;
        };
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("version") {
            entry.version = unquote(rest.trim_start_matches(':').trim()).to_string();
        } else if let Some(rest) = t.strip_prefix("resolved") {
            entry.resolved = unquote(rest.trim_start_matches(':').trim()).to_string();
        } else if let Some(rest) = t.strip_prefix("resolution") {
            entry.resolved = unquote(rest.trim_start_matches(':').trim()).to_string();
        } else if let Some(rest) = t.strip_prefix("integrity") {
            entry.integrity = unquote(rest.trim_start_matches(':').trim()).to_string();
        } else if let Some(rest) = t.strip_prefix("checksum") {
            entry.integrity = unquote(rest.trim_start_matches(':').trim()).to_string();
        }
    }
    if let Some(entry) = current.take() {
        out.push(entry);
    }
    // Berry resolutions look like `chalk@npm:4.1.2`; workspace/portal/link
    // resolutions are local and exempt from registry-integrity rules.
    for e in &mut out {
        if !e.resolved.starts_with("http") && !e.resolved.contains("@npm:") {
            e.local = true;
        }
        if e.version.is_empty() {
            if let Some(i) = e.resolved.rfind("@npm:") {
                e.version = e.resolved[i + "@npm:".len()..].to_string();
            }
        }
    }
    out
}

/// pnpm-lock.yaml: entries live under `packages:` as `/name@version:` (v6) or
/// `/name/version:` (v5), each with a `resolution: {integrity: ..., tarball: ...}`.
fn parse_pnpm_lock(text: &str) -> Vec<Locked> {
    let mut out: Vec<Locked> = Vec::new();
    let mut current: Option<Locked> = None;
    let mut in_packages = false;
    let mut key_indent = usize::MAX;

    for raw in text.lines() {
        let line = raw.trim_end();
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        let t = line.trim();

        if indent == 0 {
            if let Some(entry) = current.take() {
                out.push(entry);
            }
            // Only `packages:` carries resolutions; pnpm v9's `snapshots:`
            // block repeats the same keys with dependency edges only.
            in_packages = t == "packages:";
            key_indent = usize::MAX;
            continue;
        }
        if !in_packages {
            continue;
        }
        if key_indent == usize::MAX && t.ends_with(':') {
            key_indent = indent;
        }
        if indent == key_indent && t.ends_with(':') {
            if let Some(entry) = current.take() {
                out.push(entry);
            }
            let key = unquote(t);
            let (name, version) = split_pnpm_key(key);
            if name.is_empty() {
                continue;
            }
            current = Some(Locked {
                name,
                version,
                resolved: String::new(),
                integrity: String::new(),
                install_script: false,
                local: false,
            });
            continue;
        }
        let Some(entry) = current.as_mut() else {
            continue;
        };
        if t.starts_with("resolution:") {
            let inner = t.trim_start_matches("resolution:").trim();
            let inner = inner.trim_start_matches('{').trim_end_matches('}');
            for part in inner.split(',') {
                let part = part.trim();
                if let Some(v) = part.strip_prefix("integrity:") {
                    entry.integrity = unquote(v).to_string();
                } else if let Some(v) = part.strip_prefix("tarball:") {
                    entry.resolved = unquote(v).to_string();
                } else if let Some(v) = part.strip_prefix("type:") {
                    // `type: directory` / `type: git` — not a registry tarball.
                    entry.local = unquote(v) != "tarball";
                } else if part.starts_with("directory:") || part.starts_with("repo:") {
                    entry.local = true;
                }
            }
        } else if let Some(v) = t.strip_prefix("integrity:") {
            entry.integrity = unquote(v).to_string();
        } else if let Some(v) = t.strip_prefix("tarball:") {
            entry.resolved = unquote(v).to_string();
        } else if t.starts_with("requiresBuild:") {
            entry.install_script = t.contains("true");
        }
    }
    if let Some(entry) = current.take() {
        out.push(entry);
    }
    out
}

/// `/chalk@4.1.2` (v6+), `/chalk/4.1.2` (v5), `chalk@4.1.2` (v9 snapshots).
/// Peer suffixes such as `(react@18.0.0)` are dropped.
fn split_pnpm_key(key: &str) -> (String, String) {
    let key = key.trim_start_matches('/');
    let key = match key.find('(') {
        Some(i) => &key[..i],
        None => key,
    };
    let (prefix, rest) = if let Some(r) = key.strip_prefix('@') {
        ("@", r)
    } else {
        ("", key)
    };
    if let Some(i) = rest.rfind('@') {
        return (format!("{prefix}{}", &rest[..i]), rest[i + 1..].to_string());
    }
    // v5 `/name/version` form: the last path segment is the version.
    if let Some(i) = rest.rfind('/') {
        let ver = &rest[i + 1..];
        if ver.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            return (format!("{prefix}{}", &rest[..i]), ver.to_string());
        }
    }
    (format!("{prefix}{rest}"), String::new())
}

// ---------------------------------------------------------------------------
// URL helpers
// ---------------------------------------------------------------------------

fn url_host(url: &str) -> Option<String> {
    let rest = url.split("://").nth(1)?;
    let host = rest.split('/').next()?;
    let host = host.rsplit('@').next()?; // strip userinfo
    let host = host.split(':').next()?; // strip port
    if host.is_empty() {
        None
    } else {
        Some(host.to_ascii_lowercase())
    }
}

/// Pull the version out of an npm tarball URL: `.../chalk/-/chalk-4.1.2.tgz`.
fn version_from_tarball(url: &str, name: &str) -> Option<String> {
    let file = url.split('#').next()?.split('?').next()?.rsplit('/').next()?;
    let stem = file.strip_suffix(".tgz").or_else(|| file.strip_suffix(".tar.gz"))?;
    let bare = name.rsplit('/').next().unwrap_or(name);
    let v = stem.strip_prefix(&format!("{bare}-"))?;
    if v.is_empty() {
        None
    } else {
        Some(v.to_string())
    }
}

// ---------------------------------------------------------------------------
// Rules
// ---------------------------------------------------------------------------

fn audit_package_json(pkg: &Value, include_dev: bool, findings: &mut Vec<Finding>) -> usize {
    let declared = declared_deps(pkg, include_dev);

    for d in &declared {
        let kind = classify_spec(&d.spec);
        let shown = if d.spec.is_empty() { "\"\"" } else { &d.spec };
        match kind {
            SpecKind::Wildcard => findings.push(Finding {
                rule: "wildcard-version",
                severity: Severity::High,
                subject: d.name.clone(),
                location: d.section.to_string(),
                value: shown.to_string(),
                message: format!(
                    "Spec {shown} accepts any published version, so the next install can pull a \
                     brand-new release — including a compromised one. Pin a range such as ^1.2.3, \
                     or an exact version."
                ),
            }),
            SpecKind::DistTag => findings.push(Finding {
                rule: "dist-tag-version",
                severity: Severity::High,
                subject: d.name.clone(),
                location: d.section.to_string(),
                value: shown.to_string(),
                message: format!(
                    "Spec {shown} is a dist-tag, not a version range: whatever the maintainer \
                     tags moves under you on every install. Replace it with a semver range."
                ),
            }),
            SpecKind::Git => findings.push(Finding {
                rule: "git-dependency",
                severity: Severity::High,
                subject: d.name.clone(),
                location: d.section.to_string(),
                value: shown.to_string(),
                message: format!(
                    "Spec {shown} installs straight from a git repository, bypassing registry \
                     immutability and integrity hashes. A moved branch or retagged commit \
                     silently changes the code."
                ),
            }),
            SpecKind::HttpUrl => findings.push(Finding {
                rule: "http-dependency",
                severity: Severity::High,
                subject: d.name.clone(),
                location: d.section.to_string(),
                value: shown.to_string(),
                message: format!(
                    "Spec {shown} downloads a tarball over plain http, which any network hop can \
                     rewrite. Use https and, ideally, a registry package."
                ),
            }),
            SpecKind::Url => findings.push(Finding {
                rule: "url-dependency",
                severity: Severity::High,
                subject: d.name.clone(),
                location: d.section.to_string(),
                value: shown.to_string(),
                message: format!(
                    "Spec {shown} installs a remote tarball by URL. The bytes at that URL can be \
                     replaced at any time and no registry integrity hash covers it."
                ),
            }),
            SpecKind::LocalPath => findings.push(Finding {
                rule: "file-dependency",
                severity: Severity::Medium,
                subject: d.name.clone(),
                location: d.section.to_string(),
                value: shown.to_string(),
                message: format!(
                    "Spec {shown} resolves to a local path. It is fine inside a monorepo but \
                     breaks reproducible installs anywhere the path does not exist."
                ),
            }),
            SpecKind::Alias => findings.push(Finding {
                rule: "alias-dependency",
                severity: Severity::Medium,
                subject: d.name.clone(),
                location: d.section.to_string(),
                value: shown.to_string(),
                message: format!(
                    "Spec {shown} is an npm alias, so the import name does not match the package \
                     actually installed. Confirm the aliased target is the one you intend."
                ),
            }),
            SpecKind::Prerelease => findings.push(Finding {
                rule: "prerelease-version",
                severity: Severity::Medium,
                subject: d.name.clone(),
                location: d.section.to_string(),
                value: shown.to_string(),
                message: format!(
                    "Spec {shown} targets a pre-release build, which gets far less real-world \
                     testing than a stable release."
                ),
            }),
            SpecKind::RangePrefix => findings.push(Finding {
                rule: "range-prefix",
                severity: Severity::Low,
                subject: d.name.clone(),
                location: d.section.to_string(),
                value: shown.to_string(),
                message: format!(
                    "Spec {shown} accepts future releases automatically. That is normal practice, \
                     but it does mean a compromised maintainer account can reach you through a \
                     routine install — rely on a committed lockfile and `npm ci`."
                ),
            }),
            SpecKind::Comparator | SpecKind::Exact | SpecKind::Workspace => {}
        }

        if NODE_BUILTINS.contains(&d.name.as_str()) {
            findings.push(Finding {
                rule: "builtin-shadow",
                severity: Severity::Low,
                subject: d.name.clone(),
                location: d.section.to_string(),
                value: d.spec.clone(),
                message: format!(
                    "Package name \"{}\" is also a Node.js built-in module. Browser shims use \
                     these names legitimately, but an unexpected one can hijack an import.",
                    d.name
                ),
            });
        }
    }

    // Same package declared as both a runtime and a dev dependency.
    let runtime: BTreeSet<&str> = declared
        .iter()
        .filter(|d| d.section == "dependencies")
        .map(|d| d.name.as_str())
        .collect();
    for d in declared.iter().filter(|d| d.section == "devDependencies") {
        if runtime.contains(d.name.as_str()) {
            findings.push(Finding {
                rule: "duplicate-dependency",
                severity: Severity::Medium,
                subject: d.name.clone(),
                location: "devDependencies".to_string(),
                value: d.spec.clone(),
                message: format!(
                    "\"{}\" is declared in both dependencies and devDependencies. Whichever spec \
                     wins is a package-manager detail, so the installed version is not obvious.",
                    d.name
                ),
            });
        }
    }

    // Lifecycle scripts.
    if let Some(scripts) = pkg.get("scripts").and_then(|s| s.as_object()) {
        for name in INSTALL_SCRIPTS {
            if let Some(cmd) = scripts.get(*name).and_then(|c| c.as_str()) {
                findings.push(Finding {
                    rule: "install-script",
                    severity: Severity::High,
                    subject: (*name).to_string(),
                    location: "scripts".to_string(),
                    value: cmd.to_string(),
                    message: format!(
                        "The \"{name}\" script runs automatically on install, executing arbitrary \
                         code on every developer and CI machine. Move it behind an explicit \
                         command, or install with --ignore-scripts."
                    ),
                });
            }
        }
        for name in OTHER_LIFECYCLE_SCRIPTS {
            if let Some(cmd) = scripts.get(*name).and_then(|c| c.as_str()) {
                findings.push(Finding {
                    rule: "lifecycle-script",
                    severity: Severity::Medium,
                    subject: (*name).to_string(),
                    location: "scripts".to_string(),
                    value: cmd.to_string(),
                    message: format!(
                        "The \"{name}\" lifecycle script runs automatically during install, pack \
                         or publish. Review what it executes."
                    ),
                });
            }
        }
    }

    // Forced transitive versions: npm `overrides`, yarn `resolutions`, and
    // pnpm's nested `pnpm.overrides`.
    for key in ["overrides", "resolutions", "pnpm.overrides"] {
        let node = match key {
            "pnpm.overrides" => pkg.get("pnpm").and_then(|p| p.get("overrides")),
            other => pkg.get(other),
        };
        if let Some(v) = node {
            let count = v.as_object().map(|o| o.len()).unwrap_or(0);
            if count > 0 {
                findings.push(Finding {
                    rule: "forced-override",
                    severity: Severity::Low,
                    subject: key.to_string(),
                    location: key.to_string(),
                    value: format!("{count} entr{}", if count == 1 { "y" } else { "ies" }),
                    message: format!(
                        "\"{key}\" rewrites transitive dependency versions. That is a legitimate \
                         patching tool, but it silently overrides what your dependencies asked \
                         for — re-check each entry when you upgrade."
                    ),
                });
            }
        }
    }

    if pkg.get("engines").is_none() {
        findings.push(Finding {
            rule: "missing-engines",
            severity: Severity::Low,
            subject: String::new(),
            location: "package.json".to_string(),
            value: String::new(),
            message: "No \"engines\" field, so nothing records which Node.js versions this \
                      project supports and installs can silently run on an unsupported runtime."
                .to_string(),
        });
    }

    declared.len()
}

fn audit_lockfile(entries: &[Locked], format: Format, findings: &mut Vec<Finding>) {
    for e in entries {
        // Workspace/link/git/directory entries never carry a registry hash, so
        // only registry tarball entries are held to the integrity rules.
        if e.integrity.is_empty() && !e.local {
            findings.push(Finding {
                rule: "missing-integrity",
                severity: Severity::High,
                subject: e.name.clone(),
                location: format.label().to_string(),
                value: e.version.clone(),
                message: format!(
                    "\"{}\" has no integrity hash, so the downloaded bytes are never verified \
                     against what was originally locked.",
                    e.name
                ),
            });
        } else if e.integrity.starts_with("sha1-") {
            findings.push(Finding {
                rule: "weak-integrity",
                severity: Severity::Medium,
                subject: e.name.clone(),
                location: format.label().to_string(),
                value: e.integrity.clone(),
                message: format!(
                    "\"{}\" is protected by a SHA-1 integrity hash. SHA-1 is collision-prone; \
                     re-resolve the entry so it gets a SHA-512 hash.",
                    e.name
                ),
            });
        }

        if e.resolved.is_empty() {
            continue;
        }
        let lower = e.resolved.to_ascii_lowercase();
        if lower.starts_with("http://") {
            findings.push(Finding {
                rule: "insecure-resolved-url",
                severity: Severity::High,
                subject: e.name.clone(),
                location: format.label().to_string(),
                value: e.resolved.clone(),
                message: format!(
                    "\"{}\" is fetched over plain http, so the tarball can be swapped in transit.",
                    e.name
                ),
            });
        }
        if lower.starts_with("git+") || lower.starts_with("git:") || lower.contains(".git#") {
            findings.push(Finding {
                rule: "git-resolved",
                severity: Severity::High,
                subject: e.name.clone(),
                location: format.label().to_string(),
                value: e.resolved.clone(),
                message: format!(
                    "\"{}\" is locked to a git source rather than a registry tarball, so it is \
                     not covered by registry immutability.",
                    e.name
                ),
            });
        } else if let Some(host) = url_host(&e.resolved) {
            if !PUBLIC_REGISTRY_HOSTS.contains(&host.as_str()) {
                findings.push(Finding {
                    rule: "third-party-registry",
                    severity: Severity::Medium,
                    subject: e.name.clone(),
                    location: format.label().to_string(),
                    value: e.resolved.clone(),
                    message: format!(
                        "\"{}\" resolves from {host}, not the public npm registry. That is \
                         expected for a private mirror and a red flag otherwise.",
                        e.name
                    ),
                });
            }
        }

        if !e.version.is_empty() {
            if let Some(url_version) = version_from_tarball(&e.resolved, &e.name) {
                if url_version != e.version {
                    findings.push(Finding {
                        rule: "resolved-version-mismatch",
                        severity: Severity::Medium,
                        subject: e.name.clone(),
                        location: format.label().to_string(),
                        value: format!("{} vs {url_version}", e.version),
                        message: format!(
                            "\"{}\" is locked at {} but its resolved URL points at {url_version}. \
                             A hand-edited lockfile is the usual cause.",
                            e.name, e.version
                        ),
                    });
                }
            }
        }

        if e.install_script {
            findings.push(Finding {
                rule: "has-install-script",
                severity: Severity::High,
                subject: e.name.clone(),
                location: format.label().to_string(),
                value: e.version.clone(),
                message: format!(
                    "\"{}\" runs an install script, executing its own code on every machine that \
                     installs this project.",
                    e.name
                ),
            });
        }
    }
}

fn audit_cross_check(
    pkg: &Value,
    include_dev: bool,
    entries: &[Locked],
    findings: &mut Vec<Finding>,
) {
    let mut locked: BTreeMap<&str, &Locked> = BTreeMap::new();
    for e in entries {
        locked.entry(e.name.as_str()).or_insert(e);
    }
    for d in declared_deps(pkg, include_dev) {
        if d.section == "peerDependencies" || d.section == "optionalDependencies" {
            continue;
        }
        match locked.get(d.name.as_str()) {
            None => findings.push(Finding {
                rule: "unlocked-dependency",
                severity: Severity::Medium,
                subject: d.name.clone(),
                location: d.section.to_string(),
                value: d.spec.clone(),
                message: format!(
                    "\"{}\" is declared in {} but has no lockfile entry, so the lockfile is out \
                     of date and `npm ci` would refuse it.",
                    d.name, d.section
                ),
            }),
            Some(e) => {
                if classify_spec(&d.spec) == SpecKind::Exact
                    && !e.version.is_empty()
                    && e.version != d.spec.trim()
                {
                    findings.push(Finding {
                        rule: "pin-mismatch",
                        severity: Severity::Medium,
                        subject: d.name.clone(),
                        location: d.section.to_string(),
                        value: format!("{} vs {}", d.spec.trim(), e.version),
                        message: format!(
                            "\"{}\" is pinned to {} but the lockfile holds {}. The two files \
                             disagree about what gets installed.",
                            d.name,
                            d.spec.trim(),
                            e.version
                        ),
                    });
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Report assembly
// ---------------------------------------------------------------------------

struct Report {
    verdict: &'static str,
    detected_format: Format,
    lockfile_format: Option<Format>,
    strictness: String,
    scanned: usize,
    score: u32,
    grade: char,
    counts: [usize; 4], // high, medium, low, info
    findings: Vec<Finding>,
    truncated: bool,
}

fn grade_for(score: u32) -> char {
    match score {
        90..=100 => 'A',
        80..=89 => 'B',
        70..=79 => 'C',
        60..=69 => 'D',
        _ => 'F',
    }
}

/// Audit a package.json or lockfile for risky dependency patterns.
///
/// * `manifest` — the file contents (package.json or a lockfile).
/// * `lockfile` — an optional second file; when `manifest` is a package.json
///   and this is a lockfile, declared-vs-locked cross-checks also run.
/// * `manifest_format` — `auto` (default) or an explicit format name.
/// * `strictness` — `lenient` (high only), `standard` (high + medium),
///   `strict` (everything).
/// * `include_dev` — audit devDependencies / dev lockfile entries too.
/// * `ignore` — comma-separated rule IDs to suppress.
/// * `fail_on` — lowest severity that makes the verdict FAIL, or `never`.
/// * `output` — `text`, `markdown`, or `json`.
#[allow(clippy::too_many_arguments)]
pub fn audit(
    manifest: &str,
    lockfile: &str,
    manifest_format: &str,
    strictness: &str,
    include_dev: bool,
    ignore: &str,
    fail_on: &str,
    output: &str,
) -> Result<String, String> {
    if manifest.trim().is_empty() {
        return Err("manifest is empty — paste a package.json or lockfile".to_string());
    }
    check_size("manifest", manifest)?;
    check_size("lockfile", lockfile)?;

    let strictness_l = strictness.trim().to_ascii_lowercase();
    let strictness_l = if strictness_l.is_empty() {
        "standard".to_string()
    } else {
        strictness_l
    };
    let min_severity = match strictness_l.as_str() {
        "lenient" => Severity::High,
        "standard" => Severity::Medium,
        "strict" => Severity::Info,
        other => {
            return Err(format!(
                "unknown strictness \"{other}\" — use lenient, standard or strict"
            ))
        }
    };

    let fail_on_l = fail_on.trim().to_ascii_lowercase();
    let fail_on_l = if fail_on_l.is_empty() {
        "high".to_string()
    } else {
        fail_on_l
    };
    let fail_threshold = if fail_on_l == "never" {
        None
    } else {
        Some(Severity::parse(&fail_on_l).ok_or_else(|| {
            format!("unknown fail_on \"{fail_on_l}\" — use high, medium, low, info or never")
        })?)
    };

    let requested = manifest_format.trim().to_ascii_lowercase();
    let format = if requested.is_empty() || requested == "auto" {
        detect_format(manifest)?
    } else {
        Format::parse(&requested).ok_or_else(|| {
            format!(
                "unknown manifest_format \"{requested}\" — use auto, package-json, package-lock, \
                 yarn-lock or pnpm-lock"
            )
        })?
    };

    let mut findings: Vec<Finding> = Vec::new();
    let mut scanned = 0usize;
    let mut lockfile_format = None;

    let pkg_value = if format == Format::PackageJson {
        let pkg = parse_package_json(manifest)?;
        scanned += audit_package_json(&pkg, include_dev, &mut findings);
        Some(pkg)
    } else {
        let entries = parse_lock(manifest, format, include_dev, &mut findings)?;
        scanned += entries.len();
        audit_lockfile(&entries, format, &mut findings);
        None
    };

    if !lockfile.trim().is_empty() {
        let lf = detect_format(lockfile)?;
        if !lf.is_lockfile() {
            return Err(
                "the lockfile field looks like a package.json — paste it into the manifest field \
                 instead"
                    .to_string(),
            );
        }
        lockfile_format = Some(lf);
        let entries = parse_lock(lockfile, lf, include_dev, &mut findings)?;
        scanned += entries.len();
        audit_lockfile(&entries, lf, &mut findings);
        if let Some(pkg) = &pkg_value {
            audit_cross_check(pkg, include_dev, &entries, &mut findings);
        }
    } else if format == Format::PackageJson {
        findings.push(Finding {
            rule: "no-lockfile-supplied",
            severity: Severity::Info,
            subject: String::new(),
            location: "package.json".to_string(),
            value: String::new(),
            message: "Only a package.json was audited. Paste the lockfile too to check integrity \
                      hashes, resolved registries, install-script flags and declared-vs-locked \
                      agreement."
                .to_string(),
        });
    }

    // Filter: strictness gate, then explicit rule suppression.
    let ignored: BTreeSet<String> = split_list(ignore).into_iter().collect();
    findings.retain(|f| f.severity >= min_severity && !ignored.contains(f.rule));

    // Deterministic ordering: severity desc, then rule, then subject.
    findings.sort_by(|a, b| {
        b.severity
            .cmp(&a.severity)
            .then_with(|| a.rule.cmp(b.rule))
            .then_with(|| a.subject.cmp(&b.subject))
            .then_with(|| a.value.cmp(&b.value))
    });

    let truncated = findings.len() > MAX_FINDINGS;
    if truncated {
        findings.truncate(MAX_FINDINGS);
    }

    let mut counts = [0usize; 4];
    let mut penalty: u32 = 0;
    for f in &findings {
        match f.severity {
            Severity::High => counts[0] += 1,
            Severity::Medium => counts[1] += 1,
            Severity::Low => counts[2] += 1,
            Severity::Info => counts[3] += 1,
        }
        penalty = penalty.saturating_add(f.severity.weight());
    }
    let score = 100u32.saturating_sub(penalty);

    let verdict = match fail_threshold {
        None => "pass",
        Some(t) => {
            if findings.iter().any(|f| f.severity >= t) {
                "fail"
            } else {
                "pass"
            }
        }
    };

    let report = Report {
        verdict,
        detected_format: format,
        lockfile_format,
        strictness: strictness_l,
        scanned,
        score,
        grade: grade_for(score),
        counts,
        findings,
        truncated,
    };

    match output.trim().to_ascii_lowercase().as_str() {
        "" | "text" => Ok(render_text(&report)),
        "markdown" => Ok(render_markdown(&report)),
        "json" => Ok(render_json(&report)),
        other => Err(format!(
            "unknown output \"{other}\" — use text, markdown or json"
        )),
    }
}

fn parse_lock(
    text: &str,
    format: Format,
    include_dev: bool,
    findings: &mut Vec<Finding>,
) -> Result<Vec<Locked>, String> {
    match format {
        Format::PackageLock => {
            let (entries, version) = parse_package_lock(text, include_dev)?;
            if version == 1 {
                findings.push(Finding {
                    rule: "legacy-lockfile-version",
                    severity: Severity::Low,
                    subject: String::new(),
                    location: "package-lock.json".to_string(),
                    value: "lockfileVersion 1".to_string(),
                    message: "lockfileVersion 1 predates npm 7 and omits the hasInstallScript \
                              flag, so install-script packages cannot be detected from it. \
                              Re-run npm install with a current npm."
                        .to_string(),
                });
            }
            Ok(entries)
        }
        Format::YarnLock => Ok(parse_yarn_lock(text)),
        Format::PnpmLock => Ok(parse_pnpm_lock(text)),
        Format::PackageJson => Err("expected a lockfile, got a package.json".to_string()),
    }
}

fn header_lines(r: &Report) -> Vec<String> {
    let mut v = vec![format!(
        "Input: {}{}",
        r.detected_format.label(),
        match r.lockfile_format {
            Some(f) => format!(" + {}", f.label()),
            None => String::new(),
        }
    )];
    v.push(format!(
        "Entries scanned: {} | Strictness: {}",
        r.scanned, r.strictness
    ));
    v.push(format!("Risk score: {}/100 (grade {})", r.score, r.grade));
    v.push(format!(
        "Findings: {} high, {} medium, {} low, {} info",
        r.counts[0], r.counts[1], r.counts[2], r.counts[3]
    ));
    v
}

fn render_text(r: &Report) -> String {
    let mut s = format!("DEPENDENCY RISK AUDIT — {}\n", r.verdict.to_uppercase());
    for line in header_lines(r) {
        let _ = writeln!(s, "{line}");
    }
    if r.findings.is_empty() {
        s.push_str("\nNo findings at this strictness level.\n");
        return s;
    }
    let mut last: Option<Severity> = None;
    for f in &r.findings {
        if last != Some(f.severity) {
            let _ = write!(s, "\n{}\n", f.severity.as_str().to_uppercase());
            last = Some(f.severity);
        }
        let subject = if f.subject.is_empty() {
            f.location.clone()
        } else {
            format!("{} ({})", f.subject, f.location)
        };
        let _ = writeln!(s, "  [{}] {}", f.rule, subject);
        if !f.value.is_empty() {
            let _ = writeln!(s, "      value: {}", f.value);
        }
        let _ = writeln!(s, "      {}", f.message);
    }
    if r.truncated {
        let _ = writeln!(s, "\n(report truncated at {MAX_FINDINGS} findings)");
    }
    s
}

fn render_markdown(r: &Report) -> String {
    let mut s = format!("## Dependency risk audit — {}\n\n", r.verdict.to_uppercase());
    for line in header_lines(r) {
        let _ = writeln!(s, "- {line}");
    }
    s.push('\n');
    if r.findings.is_empty() {
        s.push_str("No findings at this strictness level.\n");
        return s;
    }
    s.push_str("| Severity | Rule | Subject | Value | Detail |\n");
    s.push_str("| --- | --- | --- | --- | --- |\n");
    for f in &r.findings {
        let subject = if f.subject.is_empty() {
            f.location.clone()
        } else {
            format!("{} ({})", f.subject, f.location)
        };
        let _ = writeln!(
            s,
            "| {} | `{}` | {} | {} | {} |",
            f.severity.as_str(),
            f.rule,
            md_cell(&subject),
            if f.value.is_empty() {
                String::new()
            } else {
                format!("`{}`", md_cell(&f.value))
            },
            md_cell(&f.message)
        );
    }
    if r.truncated {
        let _ = writeln!(s, "\n_Report truncated at {MAX_FINDINGS} findings._");
    }
    s
}

fn md_cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

fn render_json(r: &Report) -> String {
    let findings: Vec<Value> = r
        .findings
        .iter()
        .map(|f| {
            serde_json::json!({
                "rule": f.rule,
                "severity": f.severity.as_str(),
                "subject": f.subject,
                "location": f.location,
                "value": f.value,
                "message": f.message,
            })
        })
        .collect();
    let doc = serde_json::json!({
        "verdict": r.verdict,
        "detected_format": r.detected_format.as_str(),
        "lockfile_format": r.lockfile_format.map(|f| f.as_str()),
        "strictness": r.strictness,
        "entries_scanned": r.scanned,
        "score": r.score,
        "grade": r.grade.to_string(),
        "summary": {
            "high": r.counts[0],
            "medium": r.counts[1],
            "low": r.counts[2],
            "info": r.counts[3],
            "total": r.findings.len(),
        },
        "truncated": r.truncated,
        "findings": findings,
    });
    serde_json::to_string_pretty(&doc).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const MESSY: &str = r#"{
        "name": "demo",
        "version": "1.0.0",
        "scripts": { "postinstall": "node scripts/setup.js", "build": "tsc" },
        "dependencies": {
            "axios": "*",
            "left-pad": "^1.3.0",
            "internal-tool": "git+ssh://git@github.com/acme/internal-tool.git#main",
            "chalk": "4.1.2"
        },
        "devDependencies": { "chalk": "^5.0.0" }
    }"#;

    fn rules(out: &str) -> Vec<String> {
        let v: Value = serde_json::from_str(out).unwrap();
        v["findings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["rule"].as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn happy_path_flags_the_classic_package_json_risks() {
        let out = audit(MESSY, "", "auto", "standard", true, "", "high", "json").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["detected_format"], "package-json");
        assert_eq!(v["verdict"], "fail");
        let r = rules(&out);
        assert!(r.contains(&"wildcard-version".to_string()), "{r:?}");
        assert!(r.contains(&"git-dependency".to_string()), "{r:?}");
        assert!(r.contains(&"install-script".to_string()), "{r:?}");
        assert!(r.contains(&"duplicate-dependency".to_string()), "{r:?}");
        // Low-severity rules are gated off at standard strictness.
        assert!(!r.contains(&"range-prefix".to_string()), "{r:?}");
        assert!(v["score"].as_u64().unwrap() < 60);
        assert_eq!(v["grade"], "F");
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = audit("   ", "", "auto", "standard", true, "", "high", "text").unwrap_err();
        assert!(err.contains("empty"), "{err}");
    }

    #[test]
    fn undetectable_input_is_an_error() {
        let err = audit("hello world", "", "auto", "standard", true, "", "high", "text")
            .unwrap_err();
        assert!(err.contains("could not detect"), "{err}");
    }

    #[test]
    fn bad_enum_values_are_errors() {
        assert!(audit(MESSY, "", "auto", "paranoid", true, "", "high", "text")
            .unwrap_err()
            .contains("strictness"));
        assert!(audit(MESSY, "", "auto", "standard", true, "", "sometimes", "text")
            .unwrap_err()
            .contains("fail_on"));
        assert!(audit(MESSY, "", "auto", "standard", true, "", "high", "yaml")
            .unwrap_err()
            .contains("output"));
        assert!(audit(MESSY, "", "toml", "standard", true, "", "high", "text")
            .unwrap_err()
            .contains("manifest_format"));
    }

    #[test]
    fn malformed_json_reports_the_parse_error() {
        let err = audit("{ \"dependencies\": ", "", "auto", "standard", true, "", "high", "text")
            .unwrap_err();
        assert!(err.contains("did not parse"), "{err}");
    }

    #[test]
    fn strict_adds_low_severity_rules_and_lenient_drops_medium() {
        let strict = rules(&audit(MESSY, "", "auto", "strict", true, "", "high", "json").unwrap());
        assert!(strict.contains(&"range-prefix".to_string()), "{strict:?}");
        assert!(strict.contains(&"missing-engines".to_string()), "{strict:?}");
        assert!(strict.contains(&"no-lockfile-supplied".to_string()), "{strict:?}");

        let lenient =
            rules(&audit(MESSY, "", "auto", "lenient", true, "", "high", "json").unwrap());
        assert!(lenient.contains(&"wildcard-version".to_string()), "{lenient:?}");
        assert!(!lenient.contains(&"duplicate-dependency".to_string()), "{lenient:?}");
    }

    #[test]
    fn ignore_suppresses_named_rules_and_can_flip_the_verdict() {
        let out = audit(
            MESSY,
            "",
            "auto",
            "standard",
            true,
            "wildcard-version, git-dependency, install-script",
            "high",
            "json",
        )
        .unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["verdict"], "pass");
        assert_eq!(v["summary"]["high"], 0);
    }

    #[test]
    fn include_dev_false_skips_dev_dependencies() {
        let src = r#"{"dependencies":{"a":"1.0.0"},"devDependencies":{"b":"*"},"engines":{"node":">=18"}}"#;
        let with = rules(&audit(src, "", "auto", "standard", true, "", "high", "json").unwrap());
        assert!(with.contains(&"wildcard-version".to_string()));
        let without =
            rules(&audit(src, "", "auto", "standard", false, "", "high", "json").unwrap());
        assert!(!without.contains(&"wildcard-version".to_string()), "{without:?}");
    }

    #[test]
    fn fail_on_never_always_passes() {
        let out = audit(MESSY, "", "auto", "standard", true, "", "never", "json").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["verdict"], "pass");
        assert!(v["summary"]["high"].as_u64().unwrap() > 0);
    }

    #[test]
    fn clean_manifest_scores_an_a() {
        let src = r#"{
            "name": "clean",
            "engines": { "node": ">=20" },
            "dependencies": { "chalk": "5.3.0" }
        }"#;
        let out = audit(src, "", "auto", "standard", true, "", "high", "json").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["verdict"], "pass");
        assert_eq!(v["grade"], "A");
        assert_eq!(v["score"], 100);
        assert_eq!(v["summary"]["total"], 0);
    }

    const LOCK_V3: &str = r#"{
      "name": "demo",
      "lockfileVersion": 3,
      "packages": {
        "": { "name": "demo", "version": "1.0.0" },
        "node_modules/chalk": {
          "version": "4.1.2",
          "resolved": "https://registry.npmjs.org/chalk/-/chalk-4.1.2.tgz",
          "integrity": "sha512-oKnbhFyRIXpUuez8iBMmyEa4nbj4IOQyuhc/wy9kY7/WVPcwIO9VA668Pu8RkO7+0G76SLROeyw9CpQ061i4mA=="
        },
        "node_modules/sharp": {
          "version": "0.32.6",
          "resolved": "https://registry.npmjs.org/sharp/-/sharp-0.32.6.tgz",
          "integrity": "sha512-KyLTWwgcR9Oe4d9HwCwNM2l7+J0dUQwn/yf7S0EnTtb0eVS4RxO0eUSvxPtzT4F3SY+C4K6fqdv/DO27sJ/v/w==",
          "hasInstallScript": true
        },
        "node_modules/mirror-pkg": {
          "version": "2.0.0",
          "resolved": "http://npm.internal.example.com/mirror-pkg/-/mirror-pkg-2.0.0.tgz"
        }
      }
    }"#;

    #[test]
    fn package_lock_rules_fire() {
        let out = audit(LOCK_V3, "", "auto", "standard", true, "", "high", "json").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["detected_format"], "package-lock");
        assert_eq!(v["entries_scanned"], 3);
        let r = rules(&out);
        assert!(r.contains(&"has-install-script".to_string()), "{r:?}");
        assert!(r.contains(&"missing-integrity".to_string()), "{r:?}");
        assert!(r.contains(&"insecure-resolved-url".to_string()), "{r:?}");
        assert!(r.contains(&"third-party-registry".to_string()), "{r:?}");
    }

    #[test]
    fn cross_check_finds_unlocked_and_mismatched_pins() {
        let pkg = r#"{"engines":{"node":">=20"},"dependencies":{"chalk":"4.1.1","missing-dep":"1.0.0"}}"#;
        let out = audit(pkg, LOCK_V3, "auto", "standard", true, "", "high", "json").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["lockfile_format"], "package-lock");
        let r = rules(&out);
        assert!(r.contains(&"unlocked-dependency".to_string()), "{r:?}");
        assert!(r.contains(&"pin-mismatch".to_string()), "{r:?}");
        // The lockfile was supplied, so the info nudge is gone.
        assert!(!r.contains(&"no-lockfile-supplied".to_string()), "{r:?}");
    }

    #[test]
    fn lockfile_v1_is_parsed_and_flagged() {
        let src = r#"{
          "lockfileVersion": 1,
          "dependencies": {
            "old-pkg": {
              "version": "1.0.0",
              "resolved": "https://registry.npmjs.org/old-pkg/-/old-pkg-1.0.0.tgz",
              "integrity": "sha1-abcdefghijklmnopqrstuvwxyz0="
            }
          }
        }"#;
        let out = audit(src, "", "auto", "strict", true, "", "high", "json").unwrap();
        let r = rules(&out);
        assert!(r.contains(&"legacy-lockfile-version".to_string()), "{r:?}");
        assert!(r.contains(&"weak-integrity".to_string()), "{r:?}");
    }

    #[test]
    fn yarn_v1_lock_is_parsed() {
        let src = "# yarn lockfile v1\n\n\nchalk@^4.1.2:\n  version \"4.1.2\"\n  resolved \"https://registry.yarnpkg.com/chalk/-/chalk-4.1.2.tgz#hash\"\n  integrity sha512-abc==\n\nsketchy@^1.0.0:\n  version \"1.0.0\"\n  resolved \"http://mirror.example.com/sketchy/-/sketchy-1.0.0.tgz\"\n";
        let out = audit(src, "", "auto", "standard", true, "", "high", "json").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["detected_format"], "yarn-lock");
        assert_eq!(v["entries_scanned"], 2);
        let r = rules(&out);
        assert!(r.contains(&"insecure-resolved-url".to_string()), "{r:?}");
        assert!(r.contains(&"missing-integrity".to_string()), "{r:?}");
    }

    #[test]
    fn pnpm_lock_is_parsed() {
        let src = "lockfileVersion: '6.0'\n\npackages:\n\n  /chalk@4.1.2:\n    resolution: {integrity: sha512-abc==}\n    dev: false\n\n  /esbuild@0.19.0:\n    resolution: {integrity: sha512-def==, tarball: http://mirror.example.com/esbuild.tgz}\n    requiresBuild: true\n    dev: true\n";
        let out = audit(src, "", "auto", "standard", true, "", "high", "json").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["detected_format"], "pnpm-lock");
        assert_eq!(v["entries_scanned"], 2);
        let r = rules(&out);
        assert!(r.contains(&"has-install-script".to_string()), "{r:?}");
        assert!(r.contains(&"insecure-resolved-url".to_string()), "{r:?}");
    }

    #[test]
    fn spec_classification_covers_every_advertised_form() {
        assert_eq!(classify_spec("*"), SpecKind::Wildcard);
        assert_eq!(classify_spec(""), SpecKind::Wildcard);
        assert_eq!(classify_spec("1.2.x"), SpecKind::Wildcard);
        assert_eq!(classify_spec("latest"), SpecKind::Wildcard);
        assert_eq!(classify_spec("next"), SpecKind::DistTag);
        assert_eq!(classify_spec("github:acme/thing"), SpecKind::Git);
        assert_eq!(classify_spec("acme/thing#v2"), SpecKind::Git);
        assert_eq!(classify_spec("https://ex.com/a.tgz"), SpecKind::Url);
        assert_eq!(classify_spec("http://ex.com/a.tgz"), SpecKind::HttpUrl);
        assert_eq!(classify_spec("file:../local"), SpecKind::LocalPath);
        assert_eq!(classify_spec("npm:other@^1.0.0"), SpecKind::Alias);
        assert_eq!(classify_spec("workspace:*"), SpecKind::Workspace);
        assert_eq!(classify_spec("1.2.3-rc.1"), SpecKind::Prerelease);
        assert_eq!(classify_spec("^1.2.3"), SpecKind::RangePrefix);
        assert_eq!(classify_spec(">=1.0.0 <2"), SpecKind::Comparator);
        assert_eq!(classify_spec("1.2.3"), SpecKind::Exact);
    }

    #[test]
    fn oversized_input_is_rejected() {
        let big = "x".repeat(MAX_INPUT_BYTES + 1);
        let err = audit(&big, "", "auto", "standard", true, "", "high", "text").unwrap_err();
        assert!(err.contains("too large"), "{err}");
    }

    #[test]
    fn text_and_markdown_render() {
        let text = audit(MESSY, "", "auto", "standard", true, "", "high", "text").unwrap();
        assert!(text.starts_with("DEPENDENCY RISK AUDIT — FAIL\n"), "{text}");
        assert!(text.contains("[wildcard-version] axios (dependencies)"), "{text}");
        let md = audit(MESSY, "", "auto", "standard", true, "", "high", "markdown").unwrap();
        assert!(md.contains("| Severity | Rule | Subject | Value | Detail |"), "{md}");
        assert!(md.contains("`wildcard-version`"), "{md}");
    }

    #[test]
    fn builtin_shadow_and_alias_are_reported() {
        let src = r#"{"engines":{"node":">=20"},"dependencies":{"path":"^0.12.7","chalk":"npm:ansi-colors@^4.1.3"}}"#;
        let r = rules(&audit(src, "", "auto", "strict", true, "", "high", "json").unwrap());
        assert!(r.contains(&"builtin-shadow".to_string()), "{r:?}");
        assert!(r.contains(&"alias-dependency".to_string()), "{r:?}");
    }
}

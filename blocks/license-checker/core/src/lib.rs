//! license-checker core — evaluate the **SPDX license identifiers** carried by an
//! SBOM or a dependency list against user-supplied **allow / deny** rules and
//! render a pass/fail compliance report. Pure Rust: the input already records the
//! resolved dependency set and its license metadata, so no package manager, no
//! filesystem walk and no network are needed — parsing, SPDX-expression
//! evaluation and rendering only, and the output is deterministic.
//!
//! Supported inputs (auto-detected, or forced via `input_format`):
//!   * `cyclonedx-json` — a CycloneDX 1.x JSON SBOM (`components[].licenses`)
//!   * `spdx-json`      — an SPDX 2.x JSON SBOM (`packages[].licenseConcluded` /
//!                        `licenseDeclared`)
//!   * `spdx-tag`       — an SPDX 2.x tag-value SBOM
//!   * `npm-json`       — the JSON inventory emitted by npm license tooling,
//!                        i.e. a flat `{"name@version": {"licenses": …}}` map
//!   * `list`           — a plain dependency list, one package per line
//!                        (`name@version: MIT`, `name,version,MIT`, `name MIT`)
//!
//! Rules are given as comma / newline separated entries in `allow` and `deny`.
//! An entry is either an SPDX identifier (optionally `… WITH <exception>`) or a
//! `category:<name>` token — `category:permissive`, `category:weak-copyleft`,
//! `category:strong-copyleft`, `category:network-copyleft`,
//! `category:public-domain`, `category:proprietary`, `category:unknown`.
//!
//! Evaluation follows the SPDX expression grammar: `OR` is a choice (acceptable
//! when any branch is acceptable), `AND` is a conjunction (every branch must be
//! acceptable), and `WITH` attaches a license exception. `deny` wins over
//! `allow`; a license matching neither falls to the `unlisted` policy when an
//! allow list is configured, and is accepted otherwise.

use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

/// Hard cap on the number of packages evaluated in one run.
pub const MAX_PACKAGES: usize = 100_000;

/// The license categories used by `category:` rules and reported per package.
pub const CATEGORIES: [&str; 7] = [
    "public-domain",
    "permissive",
    "weak-copyleft",
    "strong-copyleft",
    "network-copyleft",
    "proprietary",
    "unknown",
];

// ---------------------------------------------------------------------------
// SPDX identifier knowledge
// ---------------------------------------------------------------------------

/// Deprecated / shorthand SPDX identifiers mapped to their current form. The
/// GNU family is "pedantic": the bare `GPL-3.0` form is deprecated in favour of
/// `GPL-3.0-only`, and a trailing `+` means `-or-later`.
const DEPRECATED: [(&str, &str); 20] = [
    ("gpl-1.0", "GPL-1.0-only"),
    ("gpl-2.0", "GPL-2.0-only"),
    ("gpl-3.0", "GPL-3.0-only"),
    ("lgpl-2.0", "LGPL-2.0-only"),
    ("lgpl-2.1", "LGPL-2.1-only"),
    ("lgpl-3.0", "LGPL-3.0-only"),
    ("agpl-1.0", "AGPL-1.0-only"),
    ("agpl-3.0", "AGPL-3.0-only"),
    ("gfdl-1.1", "GFDL-1.1-only"),
    ("gfdl-1.2", "GFDL-1.2-only"),
    ("gfdl-1.3", "GFDL-1.3-only"),
    ("bsd-2-clause-freebsd", "BSD-2-Clause-Views"),
    ("bsd-2-clause-netbsd", "BSD-2-Clause"),
    ("nunit", "Nunit"),
    ("wxwindows", "wxWindows"),
    ("standardml-nj", "SMLNJ"),
    (
        "gpl-2.0-with-classpath-exception",
        "GPL-2.0-only WITH Classpath-exception-2.0",
    ),
    (
        "gpl-2.0-with-gcc-exception",
        "GPL-2.0-only WITH GCC-exception-2.0",
    ),
    (
        "gpl-3.0-with-gcc-exception",
        "GPL-3.0-only WITH GCC-exception-3.1",
    ),
    (
        "gpl-3.0-with-autoconf-exception",
        "GPL-3.0-only WITH Autoconf-exception-3.0",
    ),
];

/// Recognised SPDX identifiers with the category this tool files them under.
/// The categories are a practical grouping for policy rules, not legal advice.
const KNOWN: &[(&str, &str)] = &[
    // ---- public domain / near-public-domain ----
    ("0BSD", "public-domain"),
    ("CC0-1.0", "public-domain"),
    ("NIST-PD", "public-domain"),
    ("NIST-PD-fallback", "public-domain"),
    ("PDDL-1.0", "public-domain"),
    ("Unlicense", "public-domain"),
    ("WTFPL", "public-domain"),
    ("blessing", "public-domain"),
    // ---- permissive ----
    ("AFL-2.1", "permissive"),
    ("AFL-3.0", "permissive"),
    ("AML", "permissive"),
    ("Apache-1.0", "permissive"),
    ("Apache-1.1", "permissive"),
    ("Apache-2.0", "permissive"),
    ("Artistic-1.0", "permissive"),
    ("Artistic-2.0", "permissive"),
    ("BSD-1-Clause", "permissive"),
    ("BSD-2-Clause", "permissive"),
    ("BSD-2-Clause-Patent", "permissive"),
    ("BSD-2-Clause-Views", "permissive"),
    ("BSD-3-Clause", "permissive"),
    ("BSD-3-Clause-Clear", "permissive"),
    ("BSD-3-Clause-Attribution", "permissive"),
    ("BSD-4-Clause", "permissive"),
    ("BSD-4-Clause-UC", "permissive"),
    ("BSD-Source-Code", "permissive"),
    ("BSL-1.0", "permissive"),
    ("Beerware", "permissive"),
    ("Bitstream-Vera", "permissive"),
    ("CC-BY-3.0", "permissive"),
    ("CC-BY-4.0", "permissive"),
    ("CECILL-B", "permissive"),
    ("CNRI-Python", "permissive"),
    ("ECL-2.0", "permissive"),
    ("EFL-2.0", "permissive"),
    ("Fair", "permissive"),
    ("FTL", "permissive"),
    ("HPND", "permissive"),
    ("HPND-sell-variant", "permissive"),
    ("ICU", "permissive"),
    ("ImageMagick", "permissive"),
    ("Info-ZIP", "permissive"),
    ("Intel", "permissive"),
    ("ISC", "permissive"),
    ("Libpng", "permissive"),
    ("libpng-2.0", "permissive"),
    ("libtiff", "permissive"),
    ("Linux-OpenIB", "permissive"),
    ("MIT", "permissive"),
    ("MIT-0", "permissive"),
    ("MIT-advertising", "permissive"),
    ("MIT-Modern-Variant", "permissive"),
    ("MS-PL", "permissive"),
    ("MirOS", "permissive"),
    ("Motosoto", "permissive"),
    ("Mup", "permissive"),
    ("NCSA", "permissive"),
    ("NTP", "permissive"),
    ("Naumen", "permissive"),
    ("ODC-By-1.0", "permissive"),
    ("OML", "permissive"),
    ("OpenSSL", "permissive"),
    ("PHP-3.0", "permissive"),
    ("PHP-3.01", "permissive"),
    ("PSF-2.0", "permissive"),
    ("PostgreSQL", "permissive"),
    ("Python-2.0", "permissive"),
    ("Python-2.0.1", "permissive"),
    ("Ruby", "permissive"),
    ("SGI-B-2.0", "permissive"),
    ("SMLNJ", "permissive"),
    ("SSH-OpenSSH", "permissive"),
    ("Saxpath", "permissive"),
    ("TCL", "permissive"),
    ("TCP-wrappers", "permissive"),
    ("UPL-1.0", "permissive"),
    ("Unicode-3.0", "permissive"),
    ("Unicode-DFS-2015", "permissive"),
    ("Unicode-DFS-2016", "permissive"),
    ("W3C", "permissive"),
    ("W3C-20150513", "permissive"),
    ("X11", "permissive"),
    ("XFree86-1.1", "permissive"),
    ("Xnet", "permissive"),
    ("ZPL-2.0", "permissive"),
    ("ZPL-2.1", "permissive"),
    ("Zend-2.0", "permissive"),
    ("Zlib", "permissive"),
    ("zlib-acknowledgement", "permissive"),
    ("bzip2-1.0.6", "permissive"),
    ("curl", "permissive"),
    ("wxWindows", "permissive"),
    // ---- weak / file-level copyleft ----
    ("APSL-2.0", "weak-copyleft"),
    ("Artistic-1.0-Perl", "weak-copyleft"),
    ("CDDL-1.0", "weak-copyleft"),
    ("CDDL-1.1", "weak-copyleft"),
    ("CECILL-C", "weak-copyleft"),
    ("CPL-1.0", "weak-copyleft"),
    ("EPL-1.0", "weak-copyleft"),
    ("EPL-2.0", "weak-copyleft"),
    ("IPL-1.0", "weak-copyleft"),
    ("LGPL-2.0-only", "weak-copyleft"),
    ("LGPL-2.0-or-later", "weak-copyleft"),
    ("LGPL-2.1-only", "weak-copyleft"),
    ("LGPL-2.1-or-later", "weak-copyleft"),
    ("LGPL-3.0-only", "weak-copyleft"),
    ("LGPL-3.0-or-later", "weak-copyleft"),
    ("LiLiQ-R-1.1", "weak-copyleft"),
    ("MPL-1.0", "weak-copyleft"),
    ("MPL-1.1", "weak-copyleft"),
    ("MPL-2.0", "weak-copyleft"),
    ("MPL-2.0-no-copyleft-exception", "weak-copyleft"),
    ("MS-RL", "weak-copyleft"),
    ("NPL-1.0", "weak-copyleft"),
    ("NPL-1.1", "weak-copyleft"),
    ("QPL-1.0", "weak-copyleft"),
    ("SPL-1.0", "weak-copyleft"),
    ("Sendmail", "weak-copyleft"),
    ("SugarCRM-1.1.3", "weak-copyleft"),
    // ---- strong copyleft ----
    ("CC-BY-SA-3.0", "strong-copyleft"),
    ("CC-BY-SA-4.0", "strong-copyleft"),
    ("CECILL-2.0", "strong-copyleft"),
    ("CECILL-2.1", "strong-copyleft"),
    ("GFDL-1.1-only", "strong-copyleft"),
    ("GFDL-1.1-or-later", "strong-copyleft"),
    ("GFDL-1.2-only", "strong-copyleft"),
    ("GFDL-1.2-or-later", "strong-copyleft"),
    ("GFDL-1.3-only", "strong-copyleft"),
    ("GFDL-1.3-or-later", "strong-copyleft"),
    ("GPL-1.0-only", "strong-copyleft"),
    ("GPL-1.0-or-later", "strong-copyleft"),
    ("GPL-2.0-only", "strong-copyleft"),
    ("GPL-2.0-or-later", "strong-copyleft"),
    ("GPL-3.0-only", "strong-copyleft"),
    ("GPL-3.0-or-later", "strong-copyleft"),
    ("ODbL-1.0", "strong-copyleft"),
    ("OSL-2.1", "strong-copyleft"),
    ("OSL-3.0", "strong-copyleft"),
    ("Parity-7.0.0", "strong-copyleft"),
    ("Sleepycat", "strong-copyleft"),
    ("copyleft-next-0.3.1", "strong-copyleft"),
    // ---- network / service copyleft ----
    ("AGPL-1.0-only", "network-copyleft"),
    ("AGPL-1.0-or-later", "network-copyleft"),
    ("AGPL-3.0-only", "network-copyleft"),
    ("AGPL-3.0-or-later", "network-copyleft"),
    ("CPAL-1.0", "network-copyleft"),
    ("EUPL-1.1", "network-copyleft"),
    ("EUPL-1.2", "network-copyleft"),
    ("RPL-1.1", "network-copyleft"),
    ("RPL-1.5", "network-copyleft"),
    ("SSPL-1.0", "network-copyleft"),
    // ---- source-available / non-free ----
    ("Aladdin", "proprietary"),
    ("BUSL-1.1", "proprietary"),
    ("CC-BY-NC-4.0", "proprietary"),
    ("CC-BY-NC-ND-4.0", "proprietary"),
    ("CC-BY-NC-SA-4.0", "proprietary"),
    ("CC-BY-ND-4.0", "proprietary"),
    ("Elastic-2.0", "proprietary"),
    ("JSON", "proprietary"),
];

/// Recognised SPDX license-exception identifiers (the right-hand side of `WITH`).
const KNOWN_EXCEPTIONS: [&str; 20] = [
    "389-exception",
    "Autoconf-exception-2.0",
    "Autoconf-exception-3.0",
    "Bison-exception-2.2",
    "Bootloader-exception",
    "Classpath-exception-2.0",
    "CLISP-exception-2.0",
    "DigiRule-FOSS-exception",
    "FLTK-exception",
    "Font-exception-2.0",
    "GCC-exception-2.0",
    "GCC-exception-3.1",
    "LGPL-3.0-linking-exception",
    "LLVM-exception",
    "Linux-syscall-note",
    "OpenJDK-assembly-exception-1.0",
    "Qt-GPL-exception-1.0",
    "Qt-LGPL-exception-1.1",
    "WxWindows-exception-3.1",
    "eCos-exception-2.0",
];

/// Values that mean "no usable license metadata" rather than a real identifier.
fn is_missing(raw: &str) -> bool {
    let t = raw.trim().trim_matches('"').trim();
    if t.is_empty() {
        return true;
    }
    let lower = t.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "noassertion" | "none" | "unknown" | "unlicensed" | "n/a" | "na" | "-" | "null" | "custom"
    ) || lower.starts_with("see license in")
        || lower.starts_with("see-license-in")
}

/// Canonicalise one SPDX identifier: trim, expand a trailing `+` to `-or-later`,
/// map deprecated spellings, and restore the official casing when recognised.
fn normalize_id(raw: &str) -> String {
    let mut id = raw.trim().trim_matches('"').trim().to_string();
    // `MIT*` — some inventories mark a guessed identifier with a trailing star.
    while id.ends_with('*') {
        id.pop();
    }
    let id = id.trim().to_string();
    if id.is_empty() {
        return id;
    }
    let expanded = if let Some(stem) = id.strip_suffix('+') {
        format!("{stem}-or-later")
    } else {
        id.clone()
    };
    let lower = expanded.to_ascii_lowercase();
    if let Some((_, current)) = DEPRECATED.iter().find(|(dep, _)| *dep == lower) {
        return (*current).to_string();
    }
    match KNOWN.iter().find(|(k, _)| k.to_ascii_lowercase() == lower) {
        Some((k, _)) => (*k).to_string(),
        None => expanded,
    }
}

/// The category filed against an identifier; `unknown` when unrecognised.
fn category_of(id: &str) -> &'static str {
    let lower = id.to_ascii_lowercase();
    KNOWN
        .iter()
        .find(|(k, _)| k.to_ascii_lowercase() == lower)
        .map(|(_, c)| *c)
        .unwrap_or("unknown")
}

/// `LicenseRef-…` / `DocumentRef-…:LicenseRef-…` are valid SPDX syntax for a
/// license that is not on the SPDX list.
fn is_license_ref(id: &str) -> bool {
    let tail = match id.split_once(':') {
        Some((head, tail)) if head.to_ascii_lowercase().starts_with("documentref-") => tail,
        _ => id,
    };
    tail.to_ascii_lowercase().starts_with("licenseref-")
}

fn is_known_id(id: &str) -> bool {
    let lower = id.to_ascii_lowercase();
    KNOWN.iter().any(|(k, _)| k.to_ascii_lowercase() == lower) || is_license_ref(id)
}

fn is_known_exception(id: &str) -> bool {
    let lower = id.to_ascii_lowercase();
    KNOWN_EXCEPTIONS
        .iter()
        .any(|k| k.to_ascii_lowercase() == lower)
        || is_license_ref(id)
}

// ---------------------------------------------------------------------------
// Rules
// ---------------------------------------------------------------------------

/// One side of the policy: explicit identifiers plus category tokens.
#[derive(Default)]
struct RuleSet {
    /// Lower-cased canonical identifiers (may include a `… with …` suffix).
    ids: Vec<String>,
    /// Category names, already validated against [`CATEGORIES`].
    categories: Vec<String>,
    /// The rule text as the user wrote it, index-aligned with `ids`.
    id_labels: Vec<String>,
}

impl RuleSet {
    fn is_empty(&self) -> bool {
        self.ids.is_empty() && self.categories.is_empty()
    }

    /// Does this rule set cover `full` (`id` or `id WITH exception`)?
    /// Returns the matching rule's label.
    fn matches(&self, id: &str, full: &str, category: &str) -> Option<String> {
        let id_l = id.to_ascii_lowercase();
        let full_l = full.to_ascii_lowercase();
        for (i, rule) in self.ids.iter().enumerate() {
            if *rule == full_l || *rule == id_l {
                return Some(self.id_labels[i].clone());
            }
        }
        if self.categories.iter().any(|c| c == category) {
            return Some(format!("category:{category}"));
        }
        None
    }
}

/// Split a rule list on commas, semicolons and newlines.
fn split_rules(raw: &str) -> Vec<String> {
    raw.split(['\n', '\r', ',', ';'])
        .map(|s| s.trim().trim_matches('"').trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

fn parse_rules(raw: &str, side: &str) -> Result<RuleSet, String> {
    let mut set = RuleSet::default();
    for entry in split_rules(raw) {
        let lower = entry.to_ascii_lowercase();
        if let Some(cat) = lower.strip_prefix("category:") {
            let cat = cat.trim();
            if !CATEGORIES.contains(&cat) {
                return Err(format!(
                    "{side}: unknown category '{cat}' (expected one of {})",
                    CATEGORIES.join(", ")
                ));
            }
            set.categories.push(cat.to_string());
            continue;
        }
        // `Apache-2.0 WITH LLVM-exception` — normalise both halves.
        let canonical = match split_with(&entry) {
            Some((base, exc)) => format!("{} WITH {}", normalize_id(base), normalize_id(exc)),
            None => normalize_id(&entry),
        };
        if canonical.is_empty() {
            continue;
        }
        set.ids.push(canonical.to_ascii_lowercase());
        set.id_labels.push(canonical);
    }
    Ok(set)
}

/// Split `A WITH B` on the case-insensitive keyword when it stands alone.
fn split_with(s: &str) -> Option<(&str, &str)> {
    let bytes = s.as_bytes();
    let lower = s.to_ascii_lowercase();
    let mut from = 0;
    while let Some(rel) = lower[from..].find(" with ") {
        let at = from + rel;
        let before_ok = at > 0 && !bytes[at - 1].is_ascii_whitespace();
        let after = at + 6;
        if before_ok && after < s.len() {
            return Some((s[..at].trim(), s[after..].trim()));
        }
        from = at + 1;
    }
    None
}

/// A package-level exception: always accept this package's license.
struct Exception {
    name: String,
    version: Option<String>,
    label: String,
}

fn parse_exceptions(raw: &str) -> Vec<Exception> {
    split_rules(raw)
        .into_iter()
        .map(|entry| {
            let (name, version) = split_name_version(&entry);
            Exception {
                name: name.to_ascii_lowercase(),
                version,
                label: entry,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// SPDX expression evaluation
// ---------------------------------------------------------------------------

/// The policy outcome for a license (expression or leaf).
#[derive(Clone, Debug, PartialEq)]
enum Verdict {
    /// Explicitly permitted by the rule carried here.
    Allowed(String),
    /// Explicitly forbidden by the rule carried here.
    Denied(String),
    /// Matched no rule at all.
    Unlisted,
}

/// One license identifier seen inside an expression.
struct Leaf {
    id: String,
    exception: Option<String>,
    category: &'static str,
    /// The identifier (and any exception) is on the recognised SPDX list.
    valid: bool,
}

struct Eval<'a> {
    tokens: Vec<String>,
    pos: usize,
    allow: &'a RuleSet,
    deny: &'a RuleSet,
    leaves: Vec<Leaf>,
}

/// Tokenise an SPDX expression: parentheses are their own tokens, everything
/// else splits on whitespace.
fn tokenize(expr: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in expr.chars() {
        match ch {
            '(' | ')' => {
                if !cur.trim().is_empty() {
                    out.push(cur.trim().to_string());
                }
                cur.clear();
                out.push(ch.to_string());
            }
            c if c.is_whitespace() => {
                if !cur.trim().is_empty() {
                    out.push(cur.trim().to_string());
                }
                cur.clear();
            }
            c => cur.push(c),
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

impl<'a> Eval<'a> {
    fn new(expr: &str, allow: &'a RuleSet, deny: &'a RuleSet) -> Self {
        Eval {
            tokens: tokenize(expr),
            pos: 0,
            allow,
            deny,
            leaves: Vec::new(),
        }
    }

    fn peek(&self) -> Option<&str> {
        self.tokens.get(self.pos).map(|s| s.as_str())
    }

    fn peek_kw(&self, kw: &str) -> bool {
        self.peek()
            .map(|t| t.eq_ignore_ascii_case(kw))
            .unwrap_or(false)
    }

    fn next(&mut self) -> Option<String> {
        let t = self.tokens.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    /// `or_expr := and_expr (OR and_expr)*`
    fn parse_or(&mut self) -> Result<Verdict, String> {
        let mut left = self.parse_and()?;
        while self.peek_kw("OR") {
            self.next();
            let right = self.parse_and()?;
            left = combine_or(left, right);
        }
        Ok(left)
    }

    /// `and_expr := with_expr (AND with_expr)*`
    fn parse_and(&mut self) -> Result<Verdict, String> {
        let mut left = self.parse_with()?;
        while self.peek_kw("AND") {
            self.next();
            let right = self.parse_with()?;
            left = combine_and(left, right);
        }
        Ok(left)
    }

    /// `with_expr := '(' or_expr ')' | id (WITH exception_id)?`
    fn parse_with(&mut self) -> Result<Verdict, String> {
        if self.peek() == Some("(") {
            self.next();
            let inner = self.parse_or()?;
            match self.next().as_deref() {
                Some(")") => {}
                _ => return Err("unbalanced parentheses in license expression".into()),
            }
            if self.peek_kw("WITH") {
                return Err("WITH must follow a single license identifier".into());
            }
            return Ok(inner);
        }
        let raw = match self.next() {
            Some(t) => t,
            None => return Err("license expression ended unexpectedly".into()),
        };
        if raw == ")" || raw.eq_ignore_ascii_case("AND") || raw.eq_ignore_ascii_case("OR") {
            return Err(format!("unexpected '{raw}' in license expression"));
        }
        let id = normalize_id(&raw);
        let mut exception = None;
        if self.peek_kw("WITH") {
            self.next();
            match self.next() {
                Some(exc) => exception = Some(normalize_id(&exc)),
                None => return Err("WITH is missing an exception identifier".into()),
            }
        }
        Ok(self.leaf(id, exception))
    }

    /// Record a leaf and decide it against the deny-then-allow rules.
    fn leaf(&mut self, id: String, exception: Option<String>) -> Verdict {
        let full = match &exception {
            Some(e) => format!("{id} WITH {e}"),
            None => id.clone(),
        };
        let category = category_of(&id);
        let valid =
            is_known_id(&id) && exception.as_deref().map(is_known_exception).unwrap_or(true);
        self.leaves.push(Leaf {
            id: id.clone(),
            exception,
            category,
            valid,
        });
        if let Some(rule) = self.deny.matches(&id, &full, category) {
            return Verdict::Denied(rule);
        }
        if let Some(rule) = self.allow.matches(&id, &full, category) {
            return Verdict::Allowed(rule);
        }
        Verdict::Unlisted
    }
}

/// `OR` is a choice: acceptable when any branch is.
fn combine_or(a: Verdict, b: Verdict) -> Verdict {
    match (&a, &b) {
        (Verdict::Allowed(_), _) => a,
        (_, Verdict::Allowed(_)) => b,
        (Verdict::Unlisted, _) | (_, Verdict::Unlisted) => Verdict::Unlisted,
        _ => a,
    }
}

/// `AND` is a conjunction: every branch must be acceptable.
fn combine_and(a: Verdict, b: Verdict) -> Verdict {
    match (&a, &b) {
        (Verdict::Denied(_), _) => a,
        (_, Verdict::Denied(_)) => b,
        (Verdict::Unlisted, _) | (_, Verdict::Unlisted) => Verdict::Unlisted,
        _ => a,
    }
}

/// Evaluate a whole expression, returning the verdict plus every leaf seen.
fn evaluate(expr: &str, allow: &RuleSet, deny: &RuleSet) -> Result<(Verdict, Vec<Leaf>), String> {
    let mut ev = Eval::new(expr, allow, deny);
    if ev.tokens.is_empty() {
        return Err("empty license expression".into());
    }
    let verdict = ev.parse_or()?;
    if ev.pos != ev.tokens.len() {
        return Err(format!(
            "unexpected '{}' after the license expression",
            ev.tokens[ev.pos]
        ));
    }
    Ok((verdict, ev.leaves))
}

// ---------------------------------------------------------------------------
// Input parsing
// ---------------------------------------------------------------------------

/// One dependency read from the input.
struct Dep {
    name: String,
    version: Option<String>,
    /// The raw license expression, or `None` when the entry carries none.
    license: Option<String>,
}

/// Split `name@version`, `name==version`, `name (version)` or a bare name.
fn split_name_version(raw: &str) -> (String, Option<String>) {
    let s = raw.trim();
    if let Some((name, ver)) = s.split_once("==") {
        return (name.trim().to_string(), non_empty(ver));
    }
    if let Some(open) = s.rfind('(') {
        if s.ends_with(')') && open > 0 {
            return (
                s[..open].trim().to_string(),
                non_empty(&s[open + 1..s.len() - 1]),
            );
        }
    }
    // Scoped npm names start with '@', so only a LATER '@' separates the version.
    if let Some(at) = s.rfind('@') {
        if at > 0 {
            return (s[..at].trim().to_string(), non_empty(&s[at + 1..]));
        }
    }
    (s.to_string(), None)
}

fn non_empty(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

/// Resolve `input_format`, auto-detecting from the content when asked.
fn detect_format(text: &str, format: &str) -> Result<&'static str, String> {
    match format.trim() {
        "cyclonedx-json" => return Ok("cyclonedx-json"),
        "spdx-json" => return Ok("spdx-json"),
        "spdx-tag" => return Ok("spdx-tag"),
        "npm-json" => return Ok("npm-json"),
        "list" => return Ok("list"),
        "" | "auto" => {}
        other => {
            return Err(format!(
                "unknown input_format '{other}' (expected auto, cyclonedx-json, spdx-json, \
                 spdx-tag, npm-json, or list)"
            ))
        }
    }
    let trimmed = text.trim_start();
    if trimmed.starts_with("SPDXVersion:") || text.contains("PackageLicenseConcluded:") {
        return Ok("spdx-tag");
    }
    if trimmed.starts_with('{') {
        if text.contains("\"bomFormat\"") || text.contains("\"CycloneDX\"") {
            return Ok("cyclonedx-json");
        }
        if text.contains("\"spdxVersion\"") || text.contains("\"SPDXID\"") {
            return Ok("spdx-json");
        }
        return Ok("npm-json");
    }
    Ok("list")
}

fn parse_input(text: &str, format: &str) -> Result<Vec<Dep>, String> {
    let fmt = detect_format(text, format)?;
    let deps = match fmt {
        "cyclonedx-json" => parse_cyclonedx(text)?,
        "spdx-json" => parse_spdx_json(text)?,
        "spdx-tag" => parse_spdx_tag(text),
        "npm-json" => parse_npm_json(text)?,
        _ => parse_list(text),
    };
    if deps.len() > MAX_PACKAGES {
        return Err(format!(
            "too many packages: {} (max {MAX_PACKAGES} per run)",
            deps.len()
        ));
    }
    if deps.is_empty() {
        return Err(format!(
            "no packages found in the {fmt} input (expected at least one dependency with a \
             license field)"
        ));
    }
    Ok(deps)
}

fn parse_cyclonedx(text: &str) -> Result<Vec<Dep>, String> {
    let doc: Value =
        serde_json::from_str(text).map_err(|e| format!("invalid CycloneDX JSON: {e}"))?;
    let comps = doc
        .get("components")
        .and_then(|c| c.as_array())
        .ok_or("CycloneDX SBOM has no \"components\" array")?;
    let mut out = Vec::new();
    for c in comps {
        let name = match c.get("name").and_then(|v| v.as_str()) {
            Some(n) if !n.trim().is_empty() => n.trim(),
            _ => continue,
        };
        let name = match c.get("group").and_then(|v| v.as_str()) {
            Some(g) if !g.trim().is_empty() => format!("{}/{}", g.trim(), name),
            _ => name.to_string(),
        };
        let version = c
            .get("version")
            .and_then(|v| v.as_str())
            .and_then(|v| non_empty(v));
        out.push(Dep {
            name,
            version,
            license: cyclonedx_license(c),
        });
    }
    Ok(out)
}

/// Read a CycloneDX `licenses` array. A single `expression` is used verbatim;
/// several `license` objects are joined with `AND` (a *choice* is expressed with
/// an `expression`, per the CycloneDX schema).
fn cyclonedx_license(component: &Value) -> Option<String> {
    let arr = component.get("licenses")?.as_array()?;
    let mut parts: Vec<String> = Vec::new();
    for entry in arr {
        if let Some(expr) = entry.get("expression").and_then(|v| v.as_str()) {
            if !is_missing(expr) {
                parts.push(expr.trim().to_string());
            }
            continue;
        }
        let lic = entry.get("license").unwrap_or(entry);
        let value = lic
            .get("id")
            .and_then(|v| v.as_str())
            .or_else(|| lic.get("name").and_then(|v| v.as_str()));
        if let Some(v) = value {
            if !is_missing(v) {
                parts.push(v.trim().to_string());
            }
        }
    }
    match parts.len() {
        0 => None,
        1 => Some(parts.remove(0)),
        _ => Some(
            parts
                .into_iter()
                .map(|p| if p.contains(' ') { format!("({p})") } else { p })
                .collect::<Vec<_>>()
                .join(" AND "),
        ),
    }
}

fn parse_spdx_json(text: &str) -> Result<Vec<Dep>, String> {
    let doc: Value = serde_json::from_str(text).map_err(|e| format!("invalid SPDX JSON: {e}"))?;
    let pkgs = doc
        .get("packages")
        .and_then(|p| p.as_array())
        .ok_or("SPDX document has no \"packages\" array")?;
    let mut out = Vec::new();
    for p in pkgs {
        let name = match p.get("name").and_then(|v| v.as_str()) {
            Some(n) if !n.trim().is_empty() => n.trim().to_string(),
            _ => continue,
        };
        let version = p
            .get("versionInfo")
            .and_then(|v| v.as_str())
            .and_then(|v| non_empty(v));
        let concluded = p.get("licenseConcluded").and_then(|v| v.as_str());
        let declared = p.get("licenseDeclared").and_then(|v| v.as_str());
        let license = match concluded {
            Some(c) if !is_missing(c) => Some(c.trim().to_string()),
            _ => declared
                .filter(|d| !is_missing(d))
                .map(|d| d.trim().to_string()),
        };
        out.push(Dep {
            name,
            version,
            license,
        });
    }
    Ok(out)
}

fn parse_spdx_tag(text: &str) -> Vec<Dep> {
    let mut out: Vec<Dep> = Vec::new();
    let mut cur: Option<Dep> = None;
    let mut concluded: Option<String> = None;
    let mut declared: Option<String> = None;
    let flush = |cur: &mut Option<Dep>,
                 concluded: &mut Option<String>,
                 declared: &mut Option<String>,
                 out: &mut Vec<Dep>| {
        if let Some(mut dep) = cur.take() {
            dep.license = concluded.take().or_else(|| declared.take());
            out.push(dep);
        }
        *concluded = None;
        *declared = None;
    };
    for line in text.lines() {
        let line = line.trim();
        let Some((tag, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match tag.trim() {
            "PackageName" => {
                flush(&mut cur, &mut concluded, &mut declared, &mut out);
                cur = Some(Dep {
                    name: value.to_string(),
                    version: None,
                    license: None,
                });
            }
            "PackageVersion" => {
                if let Some(d) = cur.as_mut() {
                    d.version = non_empty(value);
                }
            }
            "PackageLicenseConcluded" => {
                if !is_missing(value) {
                    concluded = Some(value.to_string());
                }
            }
            "PackageLicenseDeclared" => {
                if !is_missing(value) {
                    declared = Some(value.to_string());
                }
            }
            _ => {}
        }
    }
    flush(&mut cur, &mut concluded, &mut declared, &mut out);
    out
}

fn parse_npm_json(text: &str) -> Result<Vec<Dep>, String> {
    let doc: Value = serde_json::from_str(text)
        .map_err(|e| format!("invalid dependency-inventory JSON: {e}"))?;
    let map = doc.as_object().ok_or(
        "expected a JSON object mapping \"name@version\" to a record with a licenses field",
    )?;
    let mut out = Vec::new();
    for (key, record) in map {
        let (name, version) = split_name_version(key);
        if name.is_empty() {
            continue;
        }
        let license = record
            .get("licenses")
            .or_else(|| record.get("license"))
            .and_then(json_license);
        out.push(Dep {
            name,
            version,
            license,
        });
    }
    Ok(out)
}

/// A `licenses` field may be a string or an array of strings (dual licensing).
fn json_license(v: &Value) -> Option<String> {
    match v {
        Value::String(s) if !is_missing(s) => Some(s.trim().to_string()),
        Value::Array(items) => {
            let parts: Vec<String> = items
                .iter()
                .filter_map(|i| i.as_str())
                .filter(|s| !is_missing(s))
                .map(|s| s.trim().to_string())
                .collect();
            match parts.len() {
                0 => None,
                1 => Some(parts[0].clone()),
                _ => Some(parts.join(" OR ")),
            }
        }
        _ => None,
    }
}

/// Parse a plain dependency list, one package per line.
fn parse_list(text: &str) -> Vec<Dep> {
    let mut out = Vec::new();
    for raw in text.lines() {
        let line = match raw.find('#') {
            Some(i) => &raw[..i],
            None => raw,
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (left, license) = split_list_line(line);
        if left.is_empty() {
            continue;
        }
        // Skip a spreadsheet header row.
        let l = left.to_ascii_lowercase();
        if matches!(l.as_str(), "package" | "name" | "component" | "module")
            && license
                .as_deref()
                .map(|s| {
                    s.trim().eq_ignore_ascii_case("license")
                        || s.trim().eq_ignore_ascii_case("licenses")
                })
                .unwrap_or(false)
        {
            continue;
        }
        let (name, version) = split_name_version(&left);
        out.push(Dep {
            name,
            version,
            license: license.filter(|s| !is_missing(s)),
        });
    }
    out
}

/// Split one list line into the package side and the license side. Delimiters
/// are tried in order: tab, comma, colon, then the first whitespace run (so an
/// expression like `(MIT OR Apache-2.0)` survives on the license side).
fn split_list_line(line: &str) -> (String, Option<String>) {
    if let Some((a, b)) = line.split_once('\t') {
        return (a.trim().to_string(), non_empty(b));
    }
    if line.contains(',') {
        let fields: Vec<&str> = line.split(',').map(|f| f.trim()).collect();
        return match fields.len() {
            0 | 1 => (line.trim().to_string(), None),
            2 => (fields[0].to_string(), non_empty(fields[1])),
            // name,version,license[,…]
            _ => {
                let name = fields[0].to_string();
                let version = non_empty(fields[1]);
                let license = non_empty(fields[2]);
                let left = match version {
                    Some(v) => format!("{name}@{v}"),
                    None => name,
                };
                (left, license)
            }
        };
    }
    if let Some((a, b)) = line.split_once(':') {
        return (a.trim().to_string(), non_empty(b));
    }
    match line.split_once(char::is_whitespace) {
        Some((a, b)) => (a.trim().to_string(), non_empty(b)),
        None => (line.trim().to_string(), None),
    }
}

// ---------------------------------------------------------------------------
// Report model
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct Finding {
    status: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    /// The license expression as evaluated (canonical identifiers), or `null`.
    #[serde(skip_serializing_if = "Option::is_none")]
    license: Option<String>,
    category: String,
    reason: String,
}

#[derive(Serialize)]
struct Summary {
    packages: usize,
    allowed: usize,
    warnings: usize,
    denied: usize,
    unknown: usize,
    invalid: usize,
}

#[derive(Serialize)]
struct LicenseUse {
    license: String,
    category: String,
    count: usize,
}

#[derive(Serialize)]
struct Report {
    verdict: String,
    summary: Summary,
    denied: Vec<Finding>,
    warnings: Vec<Finding>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    allowed: Vec<Finding>,
    licenses: Vec<LicenseUse>,
}

const NO_LICENSE: &str = "(no license)";

/// Validate a `allow`/`warn`/`deny` policy word.
fn policy(raw: &str, field: &str, fallback: &str) -> Result<String, String> {
    let v = raw.trim();
    let v = if v.is_empty() { fallback } else { v };
    match v {
        "allow" | "warn" | "deny" => Ok(v.to_string()),
        other => Err(format!(
            "unknown {field} '{other}' (expected allow, warn, or deny)"
        )),
    }
}

/// Check the licenses in an SBOM or dependency list against allow/deny rules.
///
/// * `dependencies` — the SBOM or dependency-list text.
/// * `input_format` — `auto` (default), `cyclonedx-json`, `spdx-json`,
///   `spdx-tag`, `npm-json` or `list`.
/// * `allow` / `deny` — comma/newline separated SPDX identifiers and
///   `category:<name>` tokens. `deny` wins over `allow`.
/// * `exceptions` — `name` or `name@version` entries always accepted.
/// * `unlisted` — policy for a license matching no rule while an allow list is
///   configured: `allow`, `warn` or `deny` (default `deny`).
/// * `unknown` — policy for a package with no license metadata: `allow`, `warn`
///   or `deny` (default `warn`).
/// * `validate_ids` — flag identifiers that are not on the recognised SPDX list.
/// * `include_allowed` — list compliant packages too, not just violations.
/// * `output` — `text` (default), `markdown`, `json` or `csv`.
#[allow(clippy::too_many_arguments)]
pub fn check(
    dependencies: &str,
    input_format: &str,
    allow: &str,
    deny: &str,
    exceptions: &str,
    unlisted: &str,
    unknown: &str,
    validate_ids: bool,
    include_allowed: bool,
    output: &str,
) -> Result<String, String> {
    let output = if output.trim().is_empty() {
        "text"
    } else {
        output.trim()
    };
    if !matches!(output, "text" | "markdown" | "json" | "csv") {
        return Err(format!(
            "unknown output '{output}' (expected text, markdown, json, or csv)"
        ));
    }
    if dependencies.trim().is_empty() {
        return Err("no dependencies input: paste an SBOM or a dependency list".into());
    }
    let unlisted = policy(unlisted, "unlisted", "deny")?;
    let unknown = policy(unknown, "unknown", "warn")?;
    let allow_rules = parse_rules(allow, "allow")?;
    let deny_rules = parse_rules(deny, "deny")?;
    let exception_rules = parse_exceptions(exceptions);
    let has_allow_list = !allow_rules.is_empty();

    let deps = parse_input(dependencies, input_format)?;

    let mut denied: Vec<Finding> = Vec::new();
    let mut warnings: Vec<Finding> = Vec::new();
    let mut allowed: Vec<Finding> = Vec::new();
    let mut usage: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut unknown_count = 0usize;
    let mut invalid_count = 0usize;

    for dep in &deps {
        let exception = exception_rules.iter().find(|e| {
            e.name == dep.name.to_ascii_lowercase()
                && match (&e.version, &dep.version) {
                    (None, _) => true,
                    (Some(v), Some(dv)) => v == dv,
                    (Some(_), None) => false,
                }
        });

        let (display, category, status, reason) = match &dep.license {
            None => {
                unknown_count += 1;
                *usage
                    .entry((NO_LICENSE.to_string(), "unknown".to_string()))
                    .or_insert(0) += 1;
                match exception {
                    Some(e) => (
                        NO_LICENSE.to_string(),
                        "unknown".to_string(),
                        "allowed".to_string(),
                        format!("package exception '{}'", e.label),
                    ),
                    None => (
                        NO_LICENSE.to_string(),
                        "unknown".to_string(),
                        match unknown.as_str() {
                            "allow" => "allowed",
                            "deny" => "denied",
                            _ => "warning",
                        }
                        .to_string(),
                        "no license metadata".to_string(),
                    ),
                }
            }
            Some(expr) => {
                let (verdict, leaves) = evaluate(expr, &allow_rules, &deny_rules)
                    .map_err(|e| format!("{}: {e}", dep.name))?;
                let display = render_expression(expr, &leaves);
                let category = merged_category(&leaves);
                *usage
                    .entry((display.clone(), category.clone()))
                    .or_insert(0) += 1;
                let bad: Vec<String> = leaves
                    .iter()
                    .filter(|l| !l.valid)
                    .map(|l| match &l.exception {
                        Some(e) if !is_known_exception(e) => e.clone(),
                        _ => l.id.clone(),
                    })
                    .collect();
                if !bad.is_empty() {
                    invalid_count += 1;
                }
                if let Some(e) = exception {
                    (
                        display,
                        category,
                        "allowed".to_string(),
                        format!("package exception '{}'", e.label),
                    )
                } else {
                    let (mut status, mut reason) = match &verdict {
                        Verdict::Denied(rule) => {
                            ("denied".to_string(), format!("denied by rule '{rule}'"))
                        }
                        Verdict::Allowed(rule) => {
                            ("allowed".to_string(), format!("allowed by rule '{rule}'"))
                        }
                        Verdict::Unlisted if has_allow_list => (
                            match unlisted.as_str() {
                                "allow" => "allowed",
                                "warn" => "warning",
                                _ => "denied",
                            }
                            .to_string(),
                            "not on the allow list".to_string(),
                        ),
                        Verdict::Unlisted => (
                            "allowed".to_string(),
                            "no allow list configured".to_string(),
                        ),
                    };
                    if validate_ids && !bad.is_empty() && status == "allowed" {
                        status = "warning".to_string();
                        reason = format!(
                            "unrecognized SPDX identifier{} {}",
                            if bad.len() > 1 { "s" } else { "" },
                            bad.iter()
                                .map(|b| format!("'{b}'"))
                                .collect::<Vec<_>>()
                                .join(", ")
                        );
                    } else if validate_ids && !bad.is_empty() {
                        reason = format!(
                            "{reason}; unrecognized SPDX identifier{} {}",
                            if bad.len() > 1 { "s" } else { "" },
                            bad.iter()
                                .map(|b| format!("'{b}'"))
                                .collect::<Vec<_>>()
                                .join(", ")
                        );
                    }
                    (display, category, status, reason)
                }
            }
        };

        let finding = Finding {
            status: status.clone(),
            name: dep.name.clone(),
            version: dep.version.clone(),
            license: if display == NO_LICENSE {
                None
            } else {
                Some(display)
            },
            category,
            reason,
        };
        match status.as_str() {
            "denied" => denied.push(finding),
            "warning" => warnings.push(finding),
            _ => allowed.push(finding),
        }
    }

    let mut licenses: Vec<LicenseUse> = usage
        .into_iter()
        .map(|((license, category), count)| LicenseUse {
            license,
            category,
            count,
        })
        .collect();
    licenses.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| a.license.cmp(&b.license))
    });

    let report = Report {
        verdict: if denied.is_empty() { "pass" } else { "fail" }.to_string(),
        summary: Summary {
            packages: deps.len(),
            allowed: allowed.len(),
            warnings: warnings.len(),
            denied: denied.len(),
            unknown: unknown_count,
            invalid: if validate_ids { invalid_count } else { 0 },
        },
        denied,
        warnings,
        allowed: if include_allowed { allowed } else { Vec::new() },
        licenses,
    };

    Ok(match output {
        "json" => serde_json::to_string_pretty(&report).unwrap(),
        "markdown" => render_markdown(&report),
        "csv" => render_csv(&report),
        _ => render_text(&report),
    })
}

/// Re-render an expression with canonical identifiers; a single leaf collapses
/// to just that identifier so `mit` displays as `MIT`.
fn render_expression(original: &str, leaves: &[Leaf]) -> String {
    if leaves.len() == 1 {
        let l = &leaves[0];
        return match &l.exception {
            Some(e) => format!("{} WITH {}", l.id, e),
            None => l.id.clone(),
        };
    }
    original.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// One category for a whole expression: the shared one, or `mixed`.
fn merged_category(leaves: &[Leaf]) -> String {
    let mut it = leaves.iter().map(|l| l.category);
    match it.next() {
        None => "unknown".to_string(),
        Some(first) => {
            if it.all(|c| c == first) {
                first.to_string()
            } else {
                "mixed".to_string()
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn coord(f: &Finding) -> String {
    match &f.version {
        Some(v) => format!("{}@{}", f.name, v),
        None => f.name.clone(),
    }
}

fn license_of(f: &Finding) -> &str {
    f.license.as_deref().unwrap_or(NO_LICENSE)
}

fn render_text(r: &Report) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "License check: {}\n\n",
        r.verdict.to_ascii_uppercase()
    ));
    out.push_str("Summary\n");
    out.push_str(&format!("  packages:  {}\n", r.summary.packages));
    out.push_str(&format!("  allowed:   {}\n", r.summary.allowed));
    out.push_str(&format!("  warnings:  {}\n", r.summary.warnings));
    out.push_str(&format!("  denied:    {}\n", r.summary.denied));
    out.push_str(&format!("  unknown:   {}\n", r.summary.unknown));
    out.push_str(&format!("  invalid:   {}\n", r.summary.invalid));

    if !r.denied.is_empty() {
        out.push_str(&format!("\nDenied ({})\n", r.denied.len()));
        for f in &r.denied {
            out.push_str(&format!(
                "  x {} — {} ({}): {}\n",
                coord(f),
                license_of(f),
                f.category,
                f.reason
            ));
        }
    }
    if !r.warnings.is_empty() {
        out.push_str(&format!("\nWarnings ({})\n", r.warnings.len()));
        for f in &r.warnings {
            out.push_str(&format!(
                "  ! {} — {} ({}): {}\n",
                coord(f),
                license_of(f),
                f.category,
                f.reason
            ));
        }
    }
    if !r.allowed.is_empty() {
        out.push_str(&format!("\nAllowed ({})\n", r.allowed.len()));
        for f in &r.allowed {
            out.push_str(&format!(
                "  + {} — {} ({}): {}\n",
                coord(f),
                license_of(f),
                f.category,
                f.reason
            ));
        }
    }
    out.push_str("\nLicenses in use\n");
    for l in &r.licenses {
        out.push_str(&format!("  {} ({}) — {}\n", l.license, l.category, l.count));
    }
    out
}

fn md_escape(s: &str) -> String {
    s.replace('|', "\\|")
}

fn render_markdown(r: &Report) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "# License check: {}\n\n",
        r.verdict.to_ascii_uppercase()
    ));
    out.push_str(&format!(
        "- packages: {}\n- allowed: {}\n- warnings: {}\n- denied: {}\n- unknown: {}\n- invalid: {}\n\n",
        r.summary.packages,
        r.summary.allowed,
        r.summary.warnings,
        r.summary.denied,
        r.summary.unknown,
        r.summary.invalid
    ));
    out.push_str("| Status | Package | Version | License | Category | Reason |\n");
    out.push_str("| --- | --- | --- | --- | --- | --- |\n");
    for f in r.denied.iter().chain(&r.warnings).chain(&r.allowed) {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            f.status,
            md_escape(&f.name),
            md_escape(f.version.as_deref().unwrap_or("")),
            md_escape(license_of(f)),
            f.category,
            md_escape(&f.reason)
        ));
    }
    out.push_str("\n## Licenses in use\n\n| License | Category | Count |\n| --- | --- | --- |\n");
    for l in &r.licenses {
        out.push_str(&format!(
            "| {} | {} | {} |\n",
            md_escape(&l.license),
            l.category,
            l.count
        ));
    }
    out
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn render_csv(r: &Report) -> String {
    let mut out = String::from("status,package,version,license,category,reason\n");
    for f in r.denied.iter().chain(&r.warnings).chain(&r.allowed) {
        out.push_str(&format!(
            "{},{},{},{},{},{}\n",
            csv_field(&f.status),
            csv_field(&f.name),
            csv_field(f.version.as_deref().unwrap_or("")),
            csv_field(license_of(f)),
            csv_field(&f.category),
            csv_field(&f.reason)
        ));
    }
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const CDX: &str = r#"{
      "bomFormat": "CycloneDX",
      "specVersion": "1.6",
      "components": [
        {"type":"library","name":"chalk","version":"4.1.2","licenses":[{"license":{"id":"MIT"}}]},
        {"type":"library","name":"copyleft-lib","version":"2.0.0","licenses":[{"license":{"id":"GPL-3.0-only"}}]},
        {"type":"library","name":"dual","version":"1.0.0","licenses":[{"expression":"(MIT OR Apache-2.0)"}]},
        {"type":"library","name":"mystery","version":"0.1.0"}
      ]
    }"#;

    fn run(deps: &str, allow: &str, deny: &str, output: &str) -> String {
        check(deps, "auto", allow, deny, "", "", "", true, false, output).unwrap()
    }

    #[test]
    fn happy_path_allow_list_flags_the_copyleft_dependency() {
        let out = run(CDX, "MIT, Apache-2.0", "", "text");
        assert!(out.starts_with("License check: FAIL\n"), "{out}");
        assert!(out.contains("  packages:  4\n"), "{out}");
        assert!(
            out.contains(
                "x copyleft-lib@2.0.0 — GPL-3.0-only (strong-copyleft): not on the allow list"
            ),
            "{out}"
        );
        // An OR expression is satisfied by either acceptable branch.
        assert!(!out.contains("x dual@1.0.0"), "{out}");
        assert!(
            out.contains("! mystery@0.1.0 — (no license) (unknown): no license metadata"),
            "{out}"
        );
    }

    #[test]
    fn no_rules_passes_and_still_reports_the_inventory() {
        let out = run(CDX, "", "", "text");
        assert!(out.starts_with("License check: PASS\n"), "{out}");
        assert!(out.contains("MIT (permissive) — 1"), "{out}");
        assert!(out.contains("GPL-3.0-only (strong-copyleft) — 1"), "{out}");
    }

    #[test]
    fn category_rules_deny_a_whole_family() {
        let out = run(CDX, "", "category:strong-copyleft", "text");
        assert!(
            out.contains("denied by rule 'category:strong-copyleft'"),
            "{out}"
        );
        assert!(out.contains("  denied:    1\n"), "{out}");
    }

    #[test]
    fn deny_beats_allow() {
        let out = run(CDX, "MIT", "MIT", "text");
        assert!(
            out.contains("x chalk@4.1.2 — MIT (permissive): denied by rule 'MIT'"),
            "{out}"
        );
    }

    #[test]
    fn and_expression_requires_every_branch() {
        let deps = "combo@1.0.0: MIT AND GPL-3.0-only";
        let out = run(deps, "MIT", "", "text");
        assert!(out.contains("x combo@1.0.0"), "{out}");
        let out = run(deps, "MIT, GPL-3.0-only", "", "text");
        assert!(out.starts_with("License check: PASS\n"), "{out}");
    }

    #[test]
    fn with_exception_is_matchable_whole_and_by_base_id() {
        let deps = "llvm-thing@1.0.0: Apache-2.0 WITH LLVM-exception";
        let out = run(deps, "Apache-2.0 WITH LLVM-exception", "", "text");
        assert!(out.starts_with("License check: PASS\n"), "{out}");
        let out = run(deps, "Apache-2.0", "", "text");
        assert!(out.starts_with("License check: PASS\n"), "{out}");
        let out = run(deps, "MIT", "", "text");
        assert!(out.contains("x llvm-thing@1.0.0"), "{out}");
    }

    #[test]
    fn deprecated_and_plus_forms_normalize() {
        assert_eq!(normalize_id("GPL-2.0"), "GPL-2.0-only");
        assert_eq!(normalize_id("GPL-2.0+"), "GPL-2.0-or-later");
        assert_eq!(normalize_id("mit"), "MIT");
        assert_eq!(normalize_id("MIT*"), "MIT");
        let out = run("old@1: GPL-2.0", "", "GPL-2.0-only", "text");
        assert!(out.contains("denied by rule 'GPL-2.0-only'"), "{out}");
    }

    #[test]
    fn or_later_is_not_satisfied_by_an_only_rule() {
        let out = run("g@1: GPL-2.0-or-later", "GPL-2.0-only", "", "text");
        assert!(out.contains("x g@1"), "{out}");
    }

    #[test]
    fn package_exceptions_bypass_the_rules() {
        let out = check(
            CDX,
            "auto",
            "MIT",
            "",
            "copyleft-lib@2.0.0",
            "",
            "allow",
            true,
            false,
            "text",
        )
        .unwrap();
        assert!(out.starts_with("License check: PASS\n"), "{out}");
        assert!(!out.contains("Denied"), "{out}");
    }

    #[test]
    fn invalid_identifiers_are_flagged_when_validation_is_on() {
        let out = run("weird@1.0.0: Totally-Made-Up-1.0", "", "", "text");
        assert!(
            out.contains("! weird@1.0.0 — Totally-Made-Up-1.0 (unknown): unrecognized SPDX identifier 'Totally-Made-Up-1.0'"),
            "{out}"
        );
        assert!(out.contains("  invalid:   1\n"), "{out}");
        // LicenseRef- identifiers are valid SPDX syntax, so they are not flagged.
        let out = run("custom@1.0.0: LicenseRef-Acme-Internal", "", "", "text");
        assert!(out.contains("  invalid:   0\n"), "{out}");
    }

    #[test]
    fn validation_can_be_turned_off() {
        let out = check(
            "weird@1.0.0: Totally-Made-Up-1.0",
            "auto",
            "",
            "",
            "",
            "",
            "",
            false,
            false,
            "text",
        )
        .unwrap();
        assert!(out.contains("  invalid:   0\n"), "{out}");
        assert!(!out.contains("Warnings"), "{out}");
    }

    #[test]
    fn unlisted_policy_can_warn_instead_of_deny() {
        let out = check(
            CDX, "auto", "MIT", "", "", "warn", "allow", true, false, "text",
        )
        .unwrap();
        assert!(out.starts_with("License check: PASS\n"), "{out}");
        assert!(out.contains("! copyleft-lib@2.0.0"), "{out}");
    }

    #[test]
    fn spdx_json_and_tag_inputs_parse() {
        let json = r#"{"spdxVersion":"SPDX-2.3","packages":[
          {"name":"alpha","versionInfo":"1.0.0","licenseConcluded":"NOASSERTION","licenseDeclared":"MIT"},
          {"name":"beta","versionInfo":"2.0.0","licenseConcluded":"AGPL-3.0-only"}]}"#;
        let out = run(json, "", "category:network-copyleft", "text");
        assert!(
            out.contains("x beta@2.0.0 — AGPL-3.0-only (network-copyleft)"),
            "{out}"
        );
        assert!(out.contains("MIT (permissive) — 1"), "{out}");

        let tag = "SPDXVersion: SPDX-2.3\n\nPackageName: alpha\nPackageVersion: 1.0.0\nPackageLicenseConcluded: MIT\n\nPackageName: beta\nPackageVersion: 2.0.0\nPackageLicenseDeclared: GPL-3.0-only\n";
        let out = run(tag, "", "category:strong-copyleft", "text");
        assert!(
            out.contains("x beta@2.0.0 — GPL-3.0-only (strong-copyleft)"),
            "{out}"
        );
    }

    #[test]
    fn npm_inventory_json_parses_arrays_and_scoped_names() {
        let npm = r#"{"@scope/pkg@1.2.3":{"licenses":["MIT","Apache-2.0"]},"lodash@4.17.21":{"licenses":"MIT*"}}"#;
        let out = run(npm, "", "", "text");
        assert!(
            out.contains("+ @scope/pkg@1.2.3") || out.contains("MIT OR Apache-2.0"),
            "{out}"
        );
        assert!(out.contains("MIT (permissive) — 1"), "{out}");
    }

    #[test]
    fn list_input_accepts_several_line_shapes() {
        let list = "# policy check\nchalk@4.1.2: MIT\nrequests==2.31.0, Apache-2.0\nleft-pad,1.3.0,WTFPL\ndual 1.0.0\t(MIT OR Apache-2.0)\nlonely\n";
        let out = check(list, "list", "", "", "", "", "warn", true, true, "json").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["summary"]["packages"], 5);
        let names: Vec<&str> = v["allowed"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"requests"), "{out}");
        assert!(names.contains(&"left-pad"), "{out}");
        assert_eq!(v["summary"]["unknown"], 1);
    }

    #[test]
    fn json_markdown_and_csv_outputs_render() {
        let j: Value = serde_json::from_str(&run(CDX, "MIT", "", "json")).unwrap();
        assert_eq!(j["verdict"], "fail");
        assert_eq!(j["summary"]["denied"], 1);
        assert_eq!(j["denied"][0]["name"], "copyleft-lib");

        let md = run(CDX, "MIT", "", "markdown");
        assert!(
            md.contains("| Status | Package | Version | License | Category | Reason |"),
            "{md}"
        );
        assert!(
            md.contains("| denied | copyleft-lib | 2.0.0 | GPL-3.0-only | strong-copyleft |"),
            "{md}"
        );

        let csv = run(CDX, "MIT", "", "csv");
        assert!(
            csv.starts_with("status,package,version,license,category,reason\n"),
            "{csv}"
        );
        assert!(
            csv.contains("denied,copyleft-lib,2.0.0,GPL-3.0-only,strong-copyleft,"),
            "{csv}"
        );
    }

    #[test]
    fn include_allowed_lists_compliant_packages() {
        let out = check(
            CDX, "auto", "MIT", "", "", "warn", "allow", true, true, "text",
        )
        .unwrap();
        assert!(out.contains("Allowed (3)"), "{out}");
        assert!(
            out.contains("+ chalk@4.1.2 — MIT (permissive): allowed by rule 'MIT'"),
            "{out}"
        );
    }

    // ---- error paths ----

    #[test]
    fn empty_input_errors() {
        let err = check("   ", "auto", "", "", "", "", "", true, false, "text").unwrap_err();
        assert!(err.contains("no dependencies input"), "{err}");
    }

    #[test]
    fn unknown_output_errors() {
        let err = check(CDX, "auto", "", "", "", "", "", true, false, "yaml").unwrap_err();
        assert_eq!(
            err,
            "unknown output 'yaml' (expected text, markdown, json, or csv)"
        );
    }

    #[test]
    fn unknown_input_format_errors() {
        let err = check(CDX, "toml", "", "", "", "", "", true, false, "text").unwrap_err();
        assert!(err.starts_with("unknown input_format 'toml'"), "{err}");
    }

    #[test]
    fn unknown_category_token_errors() {
        let err = check(
            CDX,
            "auto",
            "category:copyleft",
            "",
            "",
            "",
            "",
            true,
            false,
            "text",
        )
        .unwrap_err();
        assert!(
            err.starts_with("allow: unknown category 'copyleft'"),
            "{err}"
        );
    }

    #[test]
    fn unknown_policy_word_errors() {
        let err = check(CDX, "auto", "", "", "", "maybe", "", true, false, "text").unwrap_err();
        assert_eq!(
            err,
            "unknown unlisted 'maybe' (expected allow, warn, or deny)"
        );
    }

    #[test]
    fn malformed_expression_errors_with_the_package_name() {
        let err = check(
            "broken@1.0.0: (MIT OR Apache-2.0",
            "list",
            "",
            "",
            "",
            "",
            "",
            true,
            false,
            "text",
        )
        .unwrap_err();
        assert!(err.contains("broken"), "{err}");
        assert!(err.contains("unbalanced parentheses"), "{err}");
    }

    #[test]
    fn malformed_sbom_json_errors() {
        let err = check(
            "{\"bomFormat\":\"CycloneDX\"}",
            "auto",
            "",
            "",
            "",
            "",
            "",
            true,
            false,
            "text",
        )
        .unwrap_err();
        assert!(err.contains("no \"components\" array"), "{err}");
    }
}

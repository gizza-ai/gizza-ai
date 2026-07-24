//! sbom-diff core — compare two resolved dependency **lockfiles or SBOMs** and
//! report which dependencies were **added**, **removed**, or **version-bumped**
//! between the old and new file, with counts. Pure Rust: both inputs are already
//! fully-resolved dependency sets, so no package-manager resolver and no network
//! are needed — parsing + set comparison only, and output is deterministic.
//!
//! Supported inputs (auto-detected per side, or forced via `old_format` /
//! `new_format`):
//!   * `npm`            — `package-lock.json` (v1 tree or v2/v3 packages map)
//!   * `cargo`          — `Cargo.lock`
//!   * `pip`            — `requirements.txt`
//!   * `cyclonedx-json` — a CycloneDX 1.x JSON SBOM
//!   * `spdx-json`      — an SPDX 2.x JSON SBOM
//!   * `spdx-tag`       — an SPDX 2.x tag-value SBOM
//!
//! The three lockfile formats reuse the sibling `sbom-generator` parser (its
//! CycloneDX output is read back), so the component inventory stays identical
//! between the two tools. The three SBOM formats are parsed directly here —
//! `sbom-generator` only ever *emits* them.
//!
//! Outputs (`output`): `text` (grouped human report, default), `markdown`
//! (PR/CI table), or `json` (machine-readable change report).

use serde::Serialize;
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

/// Hard cap on the number of components parsed from one side.
pub const MAX_COMPONENTS: usize = 200_000;

/// A dependency coordinate: purl ecosystem (`npm`, `cargo`, `pypi`, …; empty if
/// unknown) plus the package name. Versions are tracked separately as a set.
type Coord = (String, String);

/// The parsed dependency inventory of one file: coordinate -> set of versions.
/// A present package always has a non-empty set (an unversioned package stores a
/// single empty string), so map-key presence alone tells added/removed apart.
type Inventory = BTreeMap<Coord, BTreeSet<String>>;

/// One entry in the diff report.
#[derive(Serialize)]
struct Entry {
    ecosystem: String,
    name: String,
    /// Versions on the old side (only for `removed` / `changed`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    old: Vec<String>,
    /// Versions on the new side (only for `added` / `changed`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    new: Vec<String>,
    /// For `changed` only: `upgraded`, `downgraded`, or `changed`.
    #[serde(skip_serializing_if = "Option::is_none")]
    direction: Option<String>,
}

#[derive(Serialize)]
struct Summary {
    added: usize,
    removed: usize,
    changed: usize,
    unchanged: usize,
}

#[derive(Serialize)]
struct Report {
    summary: Summary,
    added: Vec<Entry>,
    removed: Vec<Entry>,
    changed: Vec<Entry>,
}

/// Compare two lockfiles/SBOMs and render the diff.
///
/// * `old` / `new` — the two files' contents.
/// * `old_format` / `new_format` — `auto` (default) or one of the six explicit
///   formats.
/// * `include_dev` — include npm dev/optional dependencies (npm lockfile input
///   only). Default true.
/// * `output` — `text` (default), `markdown`, or `json`.
pub fn diff(
    old: &str,
    new: &str,
    old_format: &str,
    new_format: &str,
    include_dev: bool,
    output: &str,
) -> Result<String, String> {
    match output {
        "" | "text" | "markdown" | "json" => {}
        other => {
            return Err(format!(
                "unknown output '{other}' (expected text, markdown, or json)"
            ))
        }
    }
    if old.trim().is_empty() {
        return Err("no old input".into());
    }
    if new.trim().is_empty() {
        return Err("no new input".into());
    }

    let old_inv = parse_any(old, old_format, include_dev).map_err(|e| format!("old input: {e}"))?;
    let new_inv = parse_any(new, new_format, include_dev).map_err(|e| format!("new input: {e}"))?;

    let report = build_report(&old_inv, &new_inv);

    let out = if output == "json" {
        serde_json::to_string_pretty(&report).unwrap()
    } else if output == "markdown" {
        render_markdown(&report)
    } else {
        render_text(&report)
    };
    Ok(out)
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Parse one side into an inventory, resolving `auto` from the content.
fn parse_any(text: &str, format: &str, include_dev: bool) -> Result<Inventory, String> {
    let fmt = detect_format(text, format)?;
    let pkgs = match fmt {
        "npm" | "cargo" | "pip" => parse_lockfile(text, fmt, include_dev)?,
        "cyclonedx-json" => parse_cyclonedx(text)?,
        "spdx-json" => parse_spdx_json(text)?,
        "spdx-tag" => parse_spdx_tag(text)?,
        other => return Err(format!("unknown format '{other}'")),
    };
    if pkgs.len() > MAX_COMPONENTS {
        return Err(format!(
            "too many components: {} (max {MAX_COMPONENTS} per side)",
            pkgs.len()
        ));
    }
    let mut inv: Inventory = BTreeMap::new();
    for p in pkgs {
        inv.entry((p.eco, p.name))
            .or_default()
            .insert(p.version.unwrap_or_default());
    }
    Ok(inv)
}

/// Resolve `format` (or auto-detect from the content). SBOM markers are checked
/// before the lockfile heuristics, since a CycloneDX/SPDX-JSON SBOM and an npm
/// `package-lock.json` all start with `{`.
fn detect_format(text: &str, format: &str) -> Result<&'static str, String> {
    match format.trim() {
        "npm" => return Ok("npm"),
        "cargo" => return Ok("cargo"),
        "pip" => return Ok("pip"),
        "cyclonedx-json" => return Ok("cyclonedx-json"),
        "spdx-json" => return Ok("spdx-json"),
        "spdx-tag" => return Ok("spdx-tag"),
        "" | "auto" => {}
        other => {
            return Err(format!(
                "unknown format '{other}' (expected auto, npm, cargo, pip, \
                 cyclonedx-json, spdx-json, or spdx-tag)"
            ))
        }
    }
    let trimmed = text.trim_start();
    // SPDX tag-value text starts with the SPDXVersion tag.
    if trimmed.starts_with("SPDXVersion:") {
        return Ok("spdx-tag");
    }
    if trimmed.starts_with('{') {
        // Distinguish an SBOM JSON from an npm package-lock.json by marker keys.
        if text.contains("\"bomFormat\"") || text.contains("\"CycloneDX\"") {
            return Ok("cyclonedx-json");
        }
        if text.contains("\"spdxVersion\"") || text.contains("\"SPDXID\"") {
            return Ok("spdx-json");
        }
        return Ok("npm");
    }
    if text.contains("[[package]]") {
        return Ok("cargo");
    }
    Ok("pip")
}

/// One parsed dependency.
struct Pkg {
    eco: String,
    name: String,
    version: Option<String>,
}

/// Parse a lockfile by reusing `sbom-generator`: emit CycloneDX, read it back.
/// Keeps the npm/cargo/pip component inventory byte-identical to that tool.
fn parse_lockfile(text: &str, fmt: &str, include_dev: bool) -> Result<Vec<Pkg>, String> {
    let cdx = gizza_ai_sbom_generator_core::generate(
        text,
        fmt,
        "cyclonedx-json",
        "",
        "",
        include_dev,
        "",
        false,
    )?;
    parse_cyclonedx(&cdx)
}

/// Parse a CycloneDX JSON SBOM's `components` array. The root project lives in
/// `metadata.component`, not `components`, so it is naturally excluded.
fn parse_cyclonedx(text: &str) -> Result<Vec<Pkg>, String> {
    let doc: Value =
        serde_json::from_str(text).map_err(|e| format!("invalid CycloneDX JSON: {e}"))?;
    let comps = doc
        .get("components")
        .and_then(Value::as_array)
        .ok_or_else(|| "not a CycloneDX SBOM: missing \"components\"".to_string())?;
    let mut out = Vec::new();
    for c in comps {
        let name = c.get("name").and_then(Value::as_str);
        let purl = c.get("purl").and_then(Value::as_str);
        let eco = purl.map(eco_from_purl).unwrap_or_default();
        // Prefer the explicit name; fall back to the purl's name segment.
        let name = match name {
            Some(n) if !n.is_empty() => n.to_string(),
            _ => match purl.map(name_from_purl) {
                Some(n) if !n.is_empty() => n,
                _ => continue,
            },
        };
        let version = c
            .get("version")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| purl.and_then(version_from_purl));
        out.push(Pkg { eco, name, version });
    }
    Ok(out)
}

/// Parse an SPDX JSON SBOM's `packages` array. Packages that the document
/// `DESCRIBES` (the root project) are excluded so the diff stays dependency-only.
fn parse_spdx_json(text: &str) -> Result<Vec<Pkg>, String> {
    let doc: Value = serde_json::from_str(text).map_err(|e| format!("invalid SPDX JSON: {e}"))?;
    let packages = doc
        .get("packages")
        .and_then(Value::as_array)
        .ok_or_else(|| "not an SPDX SBOM: missing \"packages\"".to_string())?;

    // SPDXIDs the DOCUMENT describes -> the root project(s), excluded from deps.
    let mut described: BTreeSet<String> = BTreeSet::new();
    if let Some(rels) = doc.get("relationships").and_then(Value::as_array) {
        for r in rels {
            if r.get("relationshipType").and_then(Value::as_str) == Some("DESCRIBES")
                && r.get("spdxElementId").and_then(Value::as_str) == Some("SPDXRef-DOCUMENT")
            {
                if let Some(id) = r.get("relatedSpdxElement").and_then(Value::as_str) {
                    described.insert(id.to_string());
                }
            }
        }
    }

    let mut out = Vec::new();
    for p in packages {
        let id = p.get("SPDXID").and_then(Value::as_str).unwrap_or("");
        if described.contains(id) {
            continue;
        }
        let name = match p.get("name").and_then(Value::as_str) {
            Some(n) if !n.is_empty() => n.to_string(),
            _ => continue,
        };
        let version = p
            .get("versionInfo")
            .and_then(Value::as_str)
            .filter(|v| !v.is_empty())
            .map(str::to_string);
        let purl = spdx_json_purl(p);
        let eco = purl.as_deref().map(eco_from_purl).unwrap_or_default();
        out.push(Pkg { eco, name, version });
    }
    Ok(out)
}

/// Pull the `purl` external reference locator out of an SPDX-JSON package.
fn spdx_json_purl(pkg: &Value) -> Option<String> {
    let refs = pkg.get("externalRefs").and_then(Value::as_array)?;
    for r in refs {
        if r.get("referenceType").and_then(Value::as_str) == Some("purl") {
            if let Some(loc) = r.get("referenceLocator").and_then(Value::as_str) {
                return Some(loc.to_string());
            }
        }
    }
    None
}

/// Parse an SPDX tag-value SBOM. Blocks are separated by `PackageName:` tags;
/// the root project (the `SPDXRef-DOCUMENT DESCRIBES <id>` target) is excluded.
fn parse_spdx_tag(text: &str) -> Result<Vec<Pkg>, String> {
    if !text.contains("PackageName:") {
        return Err("not an SPDX tag-value SBOM: no PackageName entries".into());
    }
    // Root SPDXIDs the document describes.
    let mut described: BTreeSet<String> = BTreeSet::new();
    for line in text.lines() {
        let l = line.trim();
        if let Some(rest) = l.strip_prefix("Relationship:") {
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() == 3 && parts[0] == "SPDXRef-DOCUMENT" && parts[1] == "DESCRIBES" {
                described.insert(parts[2].to_string());
            }
        }
    }

    // Split into package blocks and read each block's fields.
    let mut out = Vec::new();
    let mut cur_name: Option<String> = None;
    let mut cur_id = String::new();
    let mut cur_ver: Option<String> = None;
    let mut cur_purl: Option<String> = None;

    let flush = |out: &mut Vec<Pkg>,
                 name: &Option<String>,
                 id: &str,
                 ver: &Option<String>,
                 purl: &Option<String>,
                 described: &BTreeSet<String>| {
        if let Some(n) = name {
            if !described.contains(id) {
                let eco = purl.as_deref().map(eco_from_purl).unwrap_or_default();
                out.push(Pkg {
                    eco,
                    name: n.clone(),
                    version: ver.clone(),
                });
            }
        }
    };

    for line in text.lines() {
        let l = line.trim();
        if let Some(v) = l.strip_prefix("PackageName:") {
            // Starting a new package block — flush the previous one.
            flush(&mut out, &cur_name, &cur_id, &cur_ver, &cur_purl, &described);
            cur_name = Some(v.trim().to_string());
            cur_id.clear();
            cur_ver = None;
            cur_purl = None;
        } else if let Some(v) = l.strip_prefix("SPDXID:") {
            if cur_name.is_some() {
                cur_id = v.trim().to_string();
            }
        } else if let Some(v) = l.strip_prefix("PackageVersion:") {
            let v = v.trim();
            if !v.is_empty() {
                cur_ver = Some(v.to_string());
            }
        } else if let Some(v) = l.strip_prefix("ExternalRef:") {
            // "PACKAGE-MANAGER purl pkg:npm/lodash@4.17.21"
            let parts: Vec<&str> = v.split_whitespace().collect();
            if parts.len() >= 3 && parts[1] == "purl" {
                cur_purl = Some(parts[2].to_string());
            }
        }
    }
    flush(&mut out, &cur_name, &cur_id, &cur_ver, &cur_purl, &described);
    Ok(out)
}

/// Ecosystem token of a purl: `pkg:npm/…` -> `npm`, `pkg:cargo/…` -> `cargo`.
fn eco_from_purl(purl: &str) -> String {
    let rest = purl.strip_prefix("pkg:").unwrap_or(purl);
    rest.split(['/', '@']).next().unwrap_or("").to_string()
}

/// Name segment of a purl (namespace preserved), stripping the `@version`.
fn name_from_purl(purl: &str) -> String {
    let rest = purl.strip_prefix("pkg:").unwrap_or(purl);
    // Drop the leading ecosystem token.
    let after_eco = match rest.split_once('/') {
        Some((_, r)) => r,
        None => return String::new(),
    };
    // Strip any ?qualifiers or #subpath, then the @version.
    let after_eco = after_eco.split(['?', '#']).next().unwrap_or(after_eco);
    match after_eco.rsplit_once('@') {
        Some((n, _)) => n.to_string(),
        None => after_eco.to_string(),
    }
}

/// Version segment of a purl, if present.
fn version_from_purl(purl: &str) -> Option<String> {
    let rest = purl.split(['?', '#']).next().unwrap_or(purl);
    let (_, after) = rest.split_once('/')?;
    let (_, ver) = after.rsplit_once('@')?;
    if ver.is_empty() {
        None
    } else {
        Some(ver.to_string())
    }
}

// ---------------------------------------------------------------------------
// Diff
// ---------------------------------------------------------------------------

fn build_report(old: &Inventory, new: &Inventory) -> Report {
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut changed = Vec::new();
    let mut unchanged = 0usize;

    let mut keys: BTreeSet<&Coord> = BTreeSet::new();
    keys.extend(old.keys());
    keys.extend(new.keys());

    for key in keys {
        let (eco, name) = key;
        match (old.get(key), new.get(key)) {
            (None, Some(nv)) => added.push(Entry {
                ecosystem: eco.clone(),
                name: name.clone(),
                old: Vec::new(),
                new: versions_vec(nv),
                direction: None,
            }),
            (Some(ov), None) => removed.push(Entry {
                ecosystem: eco.clone(),
                name: name.clone(),
                old: versions_vec(ov),
                new: Vec::new(),
                direction: None,
            }),
            (Some(ov), Some(nv)) => {
                if ov == nv {
                    unchanged += 1;
                } else {
                    changed.push(Entry {
                        ecosystem: eco.clone(),
                        name: name.clone(),
                        old: versions_vec(ov),
                        new: versions_vec(nv),
                        direction: Some(direction(ov, nv)),
                    });
                }
            }
            (None, None) => unreachable!(),
        }
    }

    Report {
        summary: Summary {
            added: added.len(),
            removed: removed.len(),
            changed: changed.len(),
            unchanged,
        },
        added,
        removed,
        changed,
    }
}

/// Version set -> a sorted vec, dropping the empty-string "unversioned" marker.
fn versions_vec(set: &BTreeSet<String>) -> Vec<String> {
    set.iter().filter(|v| !v.is_empty()).cloned().collect()
}

/// Classify a single-version change as `upgraded`/`downgraded`; multi-version or
/// incomparable changes are just `changed`.
fn direction(old: &BTreeSet<String>, new: &BTreeSet<String>) -> String {
    let ov = versions_vec(old);
    let nv = versions_vec(new);
    if ov.len() == 1 && nv.len() == 1 {
        match version_cmp(&ov[0], &nv[0]) {
            Ordering::Less => return "upgraded".into(),
            Ordering::Greater => return "downgraded".into(),
            Ordering::Equal => {}
        }
    }
    "changed".into()
}

/// Dependency-free dotted version compare: split on `. - + ~`, compare segments
/// numerically when both are integers, else lexically. A numeric trailing
/// segment counts as a higher release (`1.2` < `1.2.1`), but a *non-numeric*
/// trailing segment is a prerelease identifier and ranks *below* the shorter
/// release (`1.0.0` > `1.0.0-rc1`). All-zero trailing segments are equal.
fn version_cmp(a: &str, b: &str) -> Ordering {
    let seps = |c: char| matches!(c, '.' | '-' | '+' | '~');
    let mut ai = a.split(seps);
    let mut bi = b.split(seps);
    loop {
        match (ai.next(), bi.next()) {
            (None, None) => return Ordering::Equal,
            // `a` has extra trailing segments -> order of the longer side vs the shorter.
            (Some(x), None) => return trailing_cmp(x, &mut ai),
            // `b` has the extra segments -> reverse (we want `a` relative to `b`).
            (None, Some(y)) => return trailing_cmp(y, &mut bi).reverse(),
            (Some(x), Some(y)) => {
                let ord = match (x.parse::<u64>(), y.parse::<u64>()) {
                    (Ok(nx), Ok(ny)) => nx.cmp(&ny),
                    _ => x.cmp(y),
                };
                if ord != Ordering::Equal {
                    return ord;
                }
            }
        }
    }
}

/// Order of the side that has extra trailing segments (`first` + `rest`) relative
/// to the side that ran out. All-zero padding is equal; a numeric segment ranks
/// the longer side higher (patch release); a non-numeric segment is a prerelease
/// identifier and ranks the longer side lower.
fn trailing_cmp<'a>(first: &str, rest: &mut impl Iterator<Item = &'a str>) -> Ordering {
    if seg_is_zero(first) && rest_all_zero(rest) {
        return Ordering::Equal;
    }
    match first.parse::<u64>() {
        Ok(_) => Ordering::Greater,
        Err(_) => Ordering::Less,
    }
}

fn seg_is_zero(s: &str) -> bool {
    s.parse::<u64>() == Ok(0)
}

fn rest_all_zero<'a>(it: &mut impl Iterator<Item = &'a str>) -> bool {
    it.all(seg_is_zero)
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// `eco name@version` label; the ecosystem prefix is dropped when unknown, and
/// `@version` is dropped when the package is unversioned.
fn coord_label(eco: &str, name: &str, versions: &[String]) -> String {
    let head = if eco.is_empty() {
        name.to_string()
    } else {
        format!("{eco} {name}")
    };
    if versions.is_empty() {
        head
    } else {
        format!("{head}@{}", versions.join(", "))
    }
}

fn render_text(r: &Report) -> String {
    let mut s = String::new();
    s.push_str("Dependency diff\n");
    s.push_str(&format!(
        "  added:     {}\n  removed:   {}\n  changed:   {}\n  unchanged: {}\n",
        r.summary.added, r.summary.removed, r.summary.changed, r.summary.unchanged
    ));
    if r.added.is_empty() && r.removed.is_empty() && r.changed.is_empty() {
        s.push_str("\nNo dependency changes.\n");
        return s;
    }
    if !r.added.is_empty() {
        s.push_str(&format!("\nAdded ({})\n", r.added.len()));
        for e in &r.added {
            s.push_str(&format!("  + {}\n", coord_label(&e.ecosystem, &e.name, &e.new)));
        }
    }
    if !r.removed.is_empty() {
        s.push_str(&format!("\nRemoved ({})\n", r.removed.len()));
        for e in &r.removed {
            s.push_str(&format!("  - {}\n", coord_label(&e.ecosystem, &e.name, &e.old)));
        }
    }
    if !r.changed.is_empty() {
        s.push_str(&format!("\nChanged ({})\n", r.changed.len()));
        for e in &r.changed {
            let head = if e.ecosystem.is_empty() {
                e.name.clone()
            } else {
                format!("{} {}", e.ecosystem, e.name)
            };
            let dir = e.direction.as_deref().unwrap_or("changed");
            s.push_str(&format!(
                "  ~ {} {} -> {}  ({})\n",
                head,
                join_versions(&e.old),
                join_versions(&e.new),
                dir
            ));
        }
    }
    s
}

fn render_markdown(r: &Report) -> String {
    let mut s = String::new();
    s.push_str("### Dependency diff\n\n");
    s.push_str(&format!(
        "**Added:** {} · **Removed:** {} · **Changed:** {} · **Unchanged:** {}\n",
        r.summary.added, r.summary.removed, r.summary.changed, r.summary.unchanged
    ));
    if r.added.is_empty() && r.removed.is_empty() && r.changed.is_empty() {
        s.push_str("\nNo dependency changes.\n");
        return s;
    }
    s.push_str("\n| Change | Ecosystem | Package | Old | New |\n");
    s.push_str("| --- | --- | --- | --- | --- |\n");
    for e in &r.added {
        s.push_str(&format!(
            "| added | {} | {} |  | {} |\n",
            md(&e.ecosystem),
            md(&e.name),
            md(&join_versions(&e.new))
        ));
    }
    for e in &r.removed {
        s.push_str(&format!(
            "| removed | {} | {} | {} |  |\n",
            md(&e.ecosystem),
            md(&e.name),
            md(&join_versions(&e.old))
        ));
    }
    for e in &r.changed {
        s.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            e.direction.as_deref().unwrap_or("changed"),
            md(&e.ecosystem),
            md(&e.name),
            md(&join_versions(&e.old)),
            md(&join_versions(&e.new))
        ));
    }
    s
}

/// Render a version list for a report cell; empty -> the em dash placeholder.
fn join_versions(v: &[String]) -> String {
    if v.is_empty() {
        "—".to_string()
    } else {
        v.join(", ")
    }
}

/// Escape the pipe so a package/version never breaks a markdown table row.
fn md(s: &str) -> String {
    s.replace('|', "\\|")
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD_NPM: &str = r#"{"name":"app","version":"1.0.0","lockfileVersion":3,"packages":{
        "":{"name":"app","version":"1.0.0"},
        "node_modules/chalk":{"version":"4.1.2"},
        "node_modules/left-pad":{"version":"1.3.0"},
        "node_modules/mocha":{"version":"10.0.0","dev":true}
    }}"#;
    const NEW_NPM: &str = r#"{"name":"app","version":"1.1.0","lockfileVersion":3,"packages":{
        "":{"name":"app","version":"1.1.0"},
        "node_modules/chalk":{"version":"5.0.0"},
        "node_modules/lodash":{"version":"4.17.21"},
        "node_modules/mocha":{"version":"10.0.0","dev":true}
    }}"#;

    fn json_report(old: &str, new: &str, of: &str, nf: &str, dev: bool) -> Value {
        let s = diff(old, new, of, nf, dev, "json").unwrap();
        serde_json::from_str(&s).unwrap()
    }

    #[test]
    fn npm_added_removed_changed() {
        let v = json_report(OLD_NPM, NEW_NPM, "auto", "auto", true);
        assert_eq!(v["summary"]["added"], 1);
        assert_eq!(v["summary"]["removed"], 1);
        assert_eq!(v["summary"]["changed"], 1);
        assert_eq!(v["summary"]["unchanged"], 1); // mocha
        assert_eq!(v["added"][0]["name"], "lodash");
        assert_eq!(v["added"][0]["ecosystem"], "npm");
        assert_eq!(v["added"][0]["new"][0], "4.17.21");
        assert_eq!(v["removed"][0]["name"], "left-pad");
        assert_eq!(v["changed"][0]["name"], "chalk");
        assert_eq!(v["changed"][0]["old"][0], "4.1.2");
        assert_eq!(v["changed"][0]["new"][0], "5.0.0");
        assert_eq!(v["changed"][0]["direction"], "upgraded");
    }

    #[test]
    fn include_dev_false_drops_npm_dev_from_both_sides() {
        // A dev-only add must not appear when include_dev is false.
        let new_with_dev_add = r#"{"name":"app","version":"1.1.0","lockfileVersion":3,"packages":{
            "":{"name":"app","version":"1.1.0"},
            "node_modules/chalk":{"version":"5.0.0"},
            "node_modules/lodash":{"version":"4.17.21"},
            "node_modules/mocha":{"version":"10.0.0","dev":true},
            "node_modules/jest":{"version":"29.0.0","dev":true}
        }}"#;
        let with = json_report(OLD_NPM, new_with_dev_add, "npm", "npm", true);
        assert_eq!(with["summary"]["added"], 2); // lodash + jest
        let without = json_report(OLD_NPM, new_with_dev_add, "npm", "npm", false);
        assert_eq!(without["summary"]["added"], 1); // jest (dev) dropped
        assert_eq!(without["added"][0]["name"], "lodash");
    }

    #[test]
    fn downgrade_detected() {
        let old = "tokio==1.20.0\n";
        let new = "tokio==1.19.0\n";
        let v = json_report(old, new, "pip", "pip", true);
        assert_eq!(v["changed"][0]["direction"], "downgraded");
    }

    #[test]
    fn cargo_multi_version_change() {
        let old = r#"
[[package]]
name = "app"
version = "0.1.0"

[[package]]
name = "serde"
version = "1.0.100"
source = "registry+https://github.com/rust-lang/crates.io-index"
"#;
        let new = r#"
[[package]]
name = "app"
version = "0.1.0"

[[package]]
name = "serde"
version = "1.0.100"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "serde"
version = "2.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
"#;
        let v = json_report(old, new, "cargo", "cargo", true);
        assert_eq!(v["summary"]["changed"], 1);
        assert_eq!(v["changed"][0]["name"], "serde");
        // old {1.0.100} -> new {1.0.100, 2.0.0}: multi-version -> "changed"
        assert_eq!(v["changed"][0]["direction"], "changed");
        let newv: Vec<&str> = v["changed"][0]["new"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap())
            .collect();
        assert_eq!(newv, vec!["1.0.100", "2.0.0"]);
    }

    #[test]
    fn cyclonedx_sbom_input() {
        // Generate two CycloneDX SBOMs from the npm locks, then diff those.
        let old_cdx = gizza_ai_sbom_generator_core::generate(
            OLD_NPM, "npm", "cyclonedx-json", "", "", true, "", false,
        )
        .unwrap();
        let new_cdx = gizza_ai_sbom_generator_core::generate(
            NEW_NPM, "npm", "cyclonedx-json", "", "", true, "", false,
        )
        .unwrap();
        let v = json_report(&old_cdx, &new_cdx, "auto", "auto", true);
        assert_eq!(v["summary"]["added"], 1);
        assert_eq!(v["summary"]["removed"], 1);
        assert_eq!(v["summary"]["changed"], 1);
        assert_eq!(v["changed"][0]["name"], "chalk");
    }

    #[test]
    fn spdx_json_sbom_input_excludes_root() {
        let old_spdx = gizza_ai_sbom_generator_core::generate(
            OLD_NPM, "npm", "spdx-json", "", "", true, "", false,
        )
        .unwrap();
        let new_spdx = gizza_ai_sbom_generator_core::generate(
            NEW_NPM, "npm", "spdx-json", "", "", true, "", false,
        )
        .unwrap();
        let v = json_report(&old_spdx, &new_spdx, "spdx-json", "spdx-json", true);
        // Root "app" is DESCRIBES-excluded on both sides, so its 1.0.0->1.1.0
        // bump must NOT show up as a change.
        assert_eq!(v["summary"]["changed"], 1);
        assert_eq!(v["changed"][0]["name"], "chalk");
        assert!(v["changed"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["name"] != "app"));
    }

    #[test]
    fn spdx_tag_sbom_input() {
        let old_tag = gizza_ai_sbom_generator_core::generate(
            OLD_NPM, "npm", "spdx-tag", "", "", true, "", false,
        )
        .unwrap();
        let new_tag = gizza_ai_sbom_generator_core::generate(
            NEW_NPM, "npm", "spdx-tag", "", "", true, "", false,
        )
        .unwrap();
        let v = json_report(&old_tag, &new_tag, "auto", "auto", true);
        assert_eq!(v["summary"]["changed"], 1);
        assert_eq!(v["changed"][0]["name"], "chalk");
        assert_eq!(v["summary"]["added"], 1);
        assert_eq!(v["summary"]["removed"], 1);
    }

    #[test]
    fn cross_format_lockfile_vs_sbom() {
        // Old as a raw npm lockfile, new as a CycloneDX SBOM of a different lock.
        let new_cdx = gizza_ai_sbom_generator_core::generate(
            NEW_NPM, "npm", "cyclonedx-json", "", "", true, "", false,
        )
        .unwrap();
        let v = json_report(OLD_NPM, &new_cdx, "npm", "cyclonedx-json", true);
        assert_eq!(v["summary"]["added"], 1);
        assert_eq!(v["changed"][0]["name"], "chalk");
    }

    #[test]
    fn text_and_markdown_render() {
        let t = diff(OLD_NPM, NEW_NPM, "npm", "npm", true, "text").unwrap();
        assert!(t.contains("Dependency diff"));
        assert!(t.contains("+ npm lodash@4.17.21"));
        assert!(t.contains("- npm left-pad@1.3.0"));
        assert!(t.contains("~ npm chalk 4.1.2 -> 5.0.0  (upgraded)"));

        let m = diff(OLD_NPM, NEW_NPM, "npm", "npm", true, "markdown").unwrap();
        assert!(m.contains("| Change | Ecosystem | Package | Old | New |"));
        assert!(m.contains("| added | npm | lodash |  | 4.17.21 |"));
        assert!(m.contains("| upgraded | npm | chalk | 4.1.2 | 5.0.0 |"));
    }

    #[test]
    fn no_changes_message() {
        let t = diff(OLD_NPM, OLD_NPM, "npm", "npm", true, "text").unwrap();
        assert!(t.contains("No dependency changes."));
        let v = json_report(OLD_NPM, OLD_NPM, "npm", "npm", true);
        assert_eq!(v["summary"]["added"], 0);
        assert_eq!(v["summary"]["changed"], 0);
        assert_eq!(v["summary"]["unchanged"], 3);
    }

    #[test]
    fn unpinned_pip_versionless() {
        let old = "numpy\nrequests==2.28.0\n";
        let new = "numpy\nrequests==2.31.0\n";
        let v = json_report(old, new, "pip", "pip", true);
        // numpy unversioned on both sides -> unchanged; requests bumped.
        assert_eq!(v["summary"]["unchanged"], 1);
        assert_eq!(v["summary"]["changed"], 1);
        assert_eq!(v["changed"][0]["name"], "requests");
        let t = diff(old, new, "pip", "pip", true, "text").unwrap();
        assert!(t.contains("~ pypi requests 2.28.0 -> 2.31.0  (upgraded)"));
    }

    #[test]
    fn rejects_bad_input() {
        assert_eq!(
            diff("", "x", "auto", "auto", true, "text").unwrap_err(),
            "no old input"
        );
        assert_eq!(
            diff("x", "", "auto", "auto", true, "text").unwrap_err(),
            "no new input"
        );
        let e = diff(OLD_NPM, NEW_NPM, "npm", "npm", true, "yaml").unwrap_err();
        assert!(e.contains("unknown output 'yaml'"), "{e}");
        let e = diff(OLD_NPM, NEW_NPM, "ruby", "npm", true, "text").unwrap_err();
        assert!(e.contains("old input:"), "{e}");
        assert!(e.contains("unknown format 'ruby'"), "{e}");
    }

    #[test]
    fn version_cmp_basics() {
        assert_eq!(version_cmp("1.2.0", "1.2.0"), Ordering::Equal);
        assert_eq!(version_cmp("1.2", "1.2.0"), Ordering::Equal);
        assert_eq!(version_cmp("1.2.0", "1.10.0"), Ordering::Less);
        assert_eq!(version_cmp("2.0.0", "1.9.9"), Ordering::Greater);
        assert_eq!(version_cmp("1.0.0", "1.0.0-rc1"), Ordering::Greater);
    }
}

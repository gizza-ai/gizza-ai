//! sbom-generator core — turn a resolved dependency **lockfile** into a
//! Software Bill of Materials (SBOM) in CycloneDX or SPDX format. Pure Rust: a
//! lockfile is already the fully-resolved dependency set, so no package-manager
//! resolver and no network are needed — parsing + serialization only.
//!
//! Supported inputs (auto-detected, or forced via `input_format`):
//!   * npm    — `package-lock.json` (v1 `dependencies` tree AND v2/v3 `packages` map)
//!   * cargo  — `Cargo.lock` (TOML `[[package]]` entries)
//!   * pip    — `requirements.txt` (pins, extras, environment markers, comments)
//!
//! Outputs:
//!   * `cyclonedx-json` — CycloneDX 1.6 JSON (default)
//!   * `spdx-json`      — SPDX 2.3 JSON
//!   * `spdx-tag`       — SPDX 2.3 tag-value text
//!
//! Every component carries a package URL (purl): `pkg:npm/…`, `pkg:cargo/…`,
//! `pkg:pypi/…`. Output is deterministic: components are de-duplicated and
//! sorted, and the SPDX creation time defaults to a fixed placeholder (pass
//! `timestamp` for a real value) so the same lockfile always yields the same SBOM.

use serde_json::{json, Map, Value};

/// CycloneDX spec version emitted.
pub const CYCLONEDX_SPEC: &str = "1.6";
/// SPDX spec version emitted.
pub const SPDX_VERSION: &str = "SPDX-2.3";
/// The tool name recorded in the SBOM metadata.
pub const TOOL_NAME: &str = "gizza-sbom-generator";
/// Fixed SPDX `created` value used when no `timestamp` is supplied — keeps
/// output reproducible.
pub const DEFAULT_TIMESTAMP: &str = "1970-01-01T00:00:00Z";
/// Hard cap on the number of components in one run.
pub const MAX_COMPONENTS: usize = 50_000;

/// One resolved dependency.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Component {
    /// purl ecosystem: `npm`, `cargo`, or `pypi`.
    eco: &'static str,
    name: String,
    version: Option<String>,
}

impl Component {
    /// Package URL (purl) for this component, e.g. `pkg:npm/lodash@4.17.21`.
    fn purl(&self) -> String {
        let ns_name = match self.eco {
            // scoped npm packages: @scope/name -> namespace @scope (with @ encoded)
            "npm" => match self.name.split_once('/') {
                Some((scope, rest)) if scope.starts_with('@') => {
                    format!("{}/{}", scope.replace('@', "%40"), rest)
                }
                _ => self.name.clone(),
            },
            // PEP 503 normalization for pypi purls.
            "pypi" => normalize_pypi(&self.name),
            _ => self.name.clone(),
        };
        match &self.version {
            Some(v) => format!("pkg:{}/{}@{}", self.eco, ns_name, v),
            None => format!("pkg:{}/{}", self.eco, ns_name),
        }
    }
}

/// Root/primary component derived from the lockfile (the project itself).
struct Root {
    eco: &'static str,
    name: String,
    version: Option<String>,
}

/// Parsed lockfile: the root project (if the lockfile names it) + its components.
struct Parsed {
    root: Option<Root>,
    components: Vec<Component>,
}

/// Generate an SBOM from a lockfile.
///
/// * `lockfile` — the lockfile text (package-lock.json / Cargo.lock / requirements.txt).
/// * `input_format` — `auto` (default), `npm`, `cargo`, or `pip`.
/// * `output` — `cyclonedx-json` (default), `spdx-json`, or `spdx-tag`.
/// * `component_name` / `component_version` — override the root component
///   (otherwise auto-derived from the lockfile when it records the project).
/// * `include_dev` — include dev/optional dependencies (npm only). Default true.
/// * `timestamp` — RFC-3339 creation time for the SBOM metadata; empty ->
///   reproducible fixed placeholder.
/// * `pretty` — pretty-print JSON output (ignored for `spdx-tag`).
#[allow(clippy::too_many_arguments)]
pub fn generate(
    lockfile: &str,
    input_format: &str,
    output: &str,
    component_name: &str,
    component_version: &str,
    include_dev: bool,
    timestamp: &str,
    pretty: bool,
) -> Result<String, String> {
    if lockfile.trim().is_empty() {
        return Err("no lockfile input".into());
    }
    match output {
        "cyclonedx-json" | "spdx-json" | "spdx-tag" => {}
        other => {
            return Err(format!(
                "unknown output '{other}' (expected cyclonedx-json, spdx-json, or spdx-tag)"
            ))
        }
    }

    let format = detect_format(lockfile, input_format)?;
    let mut parsed = match format {
        "npm" => parse_npm(lockfile, include_dev)?,
        "cargo" => parse_cargo(lockfile)?,
        "pip" => parse_pip(lockfile)?,
        other => {
            return Err(format!(
                "unknown input_format '{other}' (expected auto, npm, cargo, or pip)"
            ))
        }
    };

    // Deterministic, de-duplicated component inventory.
    parsed.components.sort();
    parsed.components.dedup();
    if parsed.components.len() > MAX_COMPONENTS {
        return Err(format!(
            "too many components: {} (max {MAX_COMPONENTS} per run)",
            parsed.components.len()
        ));
    }
    if parsed.components.is_empty() && parsed.root.is_none() {
        return Err("no dependencies found in the lockfile".into());
    }

    // Root component: explicit params win, else what the lockfile recorded.
    let eco = format_eco(format);
    let root = if !component_name.trim().is_empty() {
        Some(Root {
            eco,
            name: component_name.trim().to_string(),
            version: non_empty(component_version),
        })
    } else {
        parsed.root.map(|mut r| {
            if let Some(v) = non_empty(component_version) {
                r.version = Some(v);
            }
            r
        })
    };

    let ts = if timestamp.trim().is_empty() {
        DEFAULT_TIMESTAMP.to_string()
    } else {
        timestamp.trim().to_string()
    };

    match output {
        "cyclonedx-json" => Ok(cyclonedx_json(&parsed.components, root.as_ref(), timestamp, pretty)),
        "spdx-json" => Ok(spdx_json(&parsed.components, root.as_ref(), &ts, pretty)),
        "spdx-tag" => Ok(spdx_tag(&parsed.components, root.as_ref(), &ts)),
        _ => unreachable!(),
    }
}

/// Map the parser format to the purl ecosystem token.
fn format_eco(format: &str) -> &'static str {
    match format {
        "npm" => "npm",
        "cargo" => "cargo",
        _ => "pypi",
    }
}

fn non_empty(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

/// Resolve `input_format` (or auto-detect from the content).
fn detect_format(lockfile: &str, input_format: &str) -> Result<&'static str, String> {
    match input_format.trim() {
        "npm" => return Ok("npm"),
        "cargo" => return Ok("cargo"),
        "pip" => return Ok("pip"),
        "" | "auto" => {}
        other => {
            return Err(format!(
                "unknown input_format '{other}' (expected auto, npm, cargo, or pip)"
            ))
        }
    }
    let trimmed = lockfile.trim_start();
    if trimmed.starts_with('{') {
        Ok("npm")
    } else if lockfile.contains("[[package]]") {
        Ok("cargo")
    } else {
        Ok("pip")
    }
}

// ---------------------------------------------------------------------------
// Parsers
// ---------------------------------------------------------------------------

/// Parse an npm `package-lock.json` (v1 `dependencies` tree or v2/v3 `packages`).
fn parse_npm(lockfile: &str, include_dev: bool) -> Result<Parsed, String> {
    let root: Value = serde_json::from_str(lockfile).map_err(|e| format!("invalid JSON: {e}"))?;
    let obj = root
        .as_object()
        .ok_or_else(|| "not a package-lock.json: expected a JSON object".to_string())?;

    let mut components: Vec<Component> = Vec::new();

    // Root project name/version (top-level, mirrored in the "" package entry).
    let proj_name = obj.get("name").and_then(Value::as_str);
    let proj_version = obj.get("version").and_then(Value::as_str);

    if let Some(packages) = obj.get("packages").and_then(Value::as_object) {
        // v2/v3: keyed by install path; "" is the root project.
        for (path, entry) in packages {
            if path.is_empty() {
                continue; // the root project, handled below
            }
            let name = match path.rsplit_once("node_modules/") {
                Some((_, n)) => n,
                None => path.as_str(),
            };
            if name.is_empty() {
                continue;
            }
            let Some(e) = entry.as_object() else { continue };
            let dev = e.get("dev").and_then(Value::as_bool).unwrap_or(false)
                || e.get("devOptional").and_then(Value::as_bool).unwrap_or(false);
            if !include_dev && dev {
                continue;
            }
            let version = e.get("version").and_then(Value::as_str).map(str::to_string);
            components.push(Component { eco: "npm", name: name.to_string(), version });
        }
        // Root name/version can live in the "" entry too.
        let root_entry = packages.get("");
        let rname = root_entry
            .and_then(|e| e.get("name"))
            .and_then(Value::as_str)
            .or(proj_name);
        let rver = root_entry
            .and_then(|e| e.get("version"))
            .and_then(Value::as_str)
            .or(proj_version);
        let root = rname.map(|n| Root {
            eco: "npm",
            name: n.to_string(),
            version: rver.map(str::to_string),
        });
        return Ok(Parsed { root, components });
    }

    if let Some(deps) = obj.get("dependencies").and_then(Value::as_object) {
        // v1: nested tree keyed by package name.
        collect_v1(deps, include_dev, &mut components);
        let root = proj_name.map(|n| Root {
            eco: "npm",
            name: n.to_string(),
            version: proj_version.map(str::to_string),
        });
        return Ok(Parsed { root, components });
    }

    Err("not a package-lock.json: missing both \"packages\" and \"dependencies\"".into())
}

/// Recursively flatten a v1 `dependencies` tree.
fn collect_v1(deps: &Map<String, Value>, include_dev: bool, out: &mut Vec<Component>) {
    for (name, entry) in deps {
        let Some(e) = entry.as_object() else { continue };
        let dev = e.get("dev").and_then(Value::as_bool).unwrap_or(false);
        if include_dev || !dev {
            let version = e.get("version").and_then(Value::as_str).map(str::to_string);
            out.push(Component { eco: "npm", name: name.clone(), version });
        }
        if let Some(nested) = e.get("dependencies").and_then(Value::as_object) {
            collect_v1(nested, include_dev, out);
        }
    }
}

/// Parse a Cargo.lock (TOML `[[package]]` array).
fn parse_cargo(lockfile: &str) -> Result<Parsed, String> {
    let doc: toml::Value = toml::from_str(lockfile).map_err(|e| format!("invalid TOML: {e}"))?;
    let packages = doc
        .get("package")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| "not a Cargo.lock: missing [[package]] entries".to_string())?;

    let mut components: Vec<Component> = Vec::new();
    let mut root: Option<Root> = None;
    for p in packages {
        let Some(name) = p.get("name").and_then(toml::Value::as_str) else { continue };
        let version = p.get("version").and_then(toml::Value::as_str).map(str::to_string);
        let has_source = p.get("source").is_some();
        // A package without a `source` is a local/workspace crate — the first
        // such crate is the project root.
        if !has_source && root.is_none() {
            root = Some(Root {
                eco: "cargo",
                name: name.to_string(),
                version: version.clone(),
            });
        }
        components.push(Component { eco: "cargo", name: name.to_string(), version });
    }
    Ok(Parsed { root, components })
}

/// Parse a pip `requirements.txt`.
fn parse_pip(lockfile: &str) -> Result<Parsed, String> {
    let mut components: Vec<Component> = Vec::new();
    for raw in lockfile.lines() {
        if let Some((name, version)) = parse_requirement(raw) {
            components.push(Component { eco: "pypi", name, version });
        }
    }
    if components.is_empty() {
        return Err("no packages found in the requirements.txt".into());
    }
    Ok(Parsed { root: None, components })
}

/// Parse one requirements.txt line into `(name, version)`. Returns `None` for
/// blank lines, comments, and option/directive lines (`-r`, `-e`, `--hash`, …).
fn parse_requirement(raw: &str) -> Option<(String, Option<String>)> {
    // Strip an inline comment ( "pkg==1.0  # note" ), keeping '#' inside URLs.
    let mut line = raw;
    if let Some(pos) = find_comment(line) {
        line = &line[..pos];
    }
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') || line.starts_with('-') {
        return None;
    }
    // Drop an environment marker: "pkg==1 ; python_version < '3.8'".
    let spec = line.split(';').next().unwrap_or("").trim();
    if spec.is_empty() {
        return None;
    }
    // PEP 508 direct URL: "pkg @ https://…".
    let spec = spec.split(" @ ").next().unwrap_or(spec).trim();
    // Name = leading run up to an extras bracket / version operator / space.
    let name_end = spec
        .find(|c: char| matches!(c, '[' | '=' | '<' | '>' | '!' | '~' | '(' | ' ' | '\t'))
        .unwrap_or(spec.len());
    let name = spec[..name_end].trim();
    if name.is_empty() {
        return None;
    }
    // Exact-pinned version via "==" / "===".
    let version = spec.find("==").map(|p| {
        spec[p + 2..]
            .trim_start_matches('=')
            .split(|c: char| c == ',' || c == ';' || c.is_whitespace())
            .next()
            .unwrap_or("")
            .trim()
            .to_string()
    });
    let version = version.filter(|v| !v.is_empty());
    Some((name.to_string(), version))
}

/// Find the byte offset of an inline comment `#` (start-of-line or after
/// whitespace), or `None` if the line has no comment.
fn find_comment(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'#' && (i == 0 || bytes[i - 1] == b' ' || bytes[i - 1] == b'\t') {
            return Some(i);
        }
    }
    None
}

/// PEP 503 name normalization for pypi purls: lowercase, runs of `-_.` -> `-`.
fn normalize_pypi(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut prev_sep = false;
    for c in name.chars() {
        if matches!(c, '-' | '_' | '.') {
            if !prev_sep {
                out.push('-');
                prev_sep = true;
            }
        } else {
            out.push(c.to_ascii_lowercase());
            prev_sep = false;
        }
    }
    out.trim_matches('-').to_string()
}

// ---------------------------------------------------------------------------
// Serializers
// ---------------------------------------------------------------------------

fn cyclonedx_json(components: &[Component], root: Option<&Root>, timestamp: &str, pretty: bool) -> String {
    let mut metadata = Map::new();
    if !timestamp.trim().is_empty() {
        metadata.insert("timestamp".into(), json!(timestamp.trim()));
    }
    metadata.insert(
        "tools".into(),
        json!({ "components": [ { "type": "application", "name": TOOL_NAME } ] }),
    );
    if let Some(r) = root {
        let purl = Component { eco: r.eco, name: r.name.clone(), version: r.version.clone() }.purl();
        let mut c = Map::new();
        c.insert("type".into(), json!("application"));
        c.insert("bom-ref".into(), json!(purl));
        c.insert("name".into(), json!(r.name));
        if let Some(v) = &r.version {
            c.insert("version".into(), json!(v));
        }
        c.insert("purl".into(), json!(purl));
        metadata.insert("component".into(), Value::Object(c));
    }

    let comps: Vec<Value> = components
        .iter()
        .map(|c| {
            let purl = c.purl();
            let mut m = Map::new();
            m.insert("type".into(), json!("library"));
            m.insert("bom-ref".into(), json!(purl));
            m.insert("name".into(), json!(c.name));
            if let Some(v) = &c.version {
                m.insert("version".into(), json!(v));
            }
            m.insert("purl".into(), json!(purl));
            Value::Object(m)
        })
        .collect();

    let mut doc = Map::new();
    doc.insert("bomFormat".into(), json!("CycloneDX"));
    doc.insert("specVersion".into(), json!(CYCLONEDX_SPEC));
    doc.insert("version".into(), json!(1));
    doc.insert("metadata".into(), Value::Object(metadata));
    doc.insert("components".into(), Value::Array(comps));
    let doc = Value::Object(doc);
    if pretty {
        serde_json::to_string_pretty(&doc).unwrap()
    } else {
        serde_json::to_string(&doc).unwrap()
    }
}

/// SPDXID for the package at index `i`.
fn spdx_pkg_id(i: usize) -> String {
    format!("SPDXRef-Package-{i}")
}

const SPDX_ROOT_ID: &str = "SPDXRef-Package-Root";

fn spdx_json(components: &[Component], root: Option<&Root>, timestamp: &str, pretty: bool) -> String {
    let doc_name = root.map(|r| r.name.as_str()).unwrap_or("sbom-document");

    let mut packages: Vec<Value> = Vec::new();
    let mut relationships: Vec<Value> = Vec::new();

    if let Some(r) = root {
        let purl = Component { eco: r.eco, name: r.name.clone(), version: r.version.clone() }.purl();
        packages.push(spdx_pkg_value(SPDX_ROOT_ID, &r.name, r.version.as_deref(), &purl));
        relationships.push(json!({
            "spdxElementId": "SPDXRef-DOCUMENT",
            "relationshipType": "DESCRIBES",
            "relatedSpdxElement": SPDX_ROOT_ID
        }));
    }

    for (i, c) in components.iter().enumerate() {
        let id = spdx_pkg_id(i);
        let purl = c.purl();
        packages.push(spdx_pkg_value(&id, &c.name, c.version.as_deref(), &purl));
        if root.is_some() {
            relationships.push(json!({
                "spdxElementId": SPDX_ROOT_ID,
                "relationshipType": "DEPENDS_ON",
                "relatedSpdxElement": id
            }));
        } else {
            relationships.push(json!({
                "spdxElementId": "SPDXRef-DOCUMENT",
                "relationshipType": "DESCRIBES",
                "relatedSpdxElement": id
            }));
        }
    }

    let doc = json!({
        "spdxVersion": SPDX_VERSION,
        "dataLicense": "CC0-1.0",
        "SPDXID": "SPDXRef-DOCUMENT",
        "name": doc_name,
        "documentNamespace": format!("https://spdx.org/spdxdocs/{}", sanitize_ns(doc_name)),
        "creationInfo": {
            "created": timestamp,
            "creators": [ format!("Tool: {TOOL_NAME}") ]
        },
        "packages": packages,
        "relationships": relationships
    });
    if pretty {
        serde_json::to_string_pretty(&doc).unwrap()
    } else {
        serde_json::to_string(&doc).unwrap()
    }
}

fn spdx_pkg_value(id: &str, name: &str, version: Option<&str>, purl: &str) -> Value {
    let mut m = Map::new();
    m.insert("name".into(), json!(name));
    m.insert("SPDXID".into(), json!(id));
    if let Some(v) = version {
        m.insert("versionInfo".into(), json!(v));
    }
    m.insert("downloadLocation".into(), json!("NOASSERTION"));
    m.insert("filesAnalyzed".into(), json!(false));
    m.insert("licenseConcluded".into(), json!("NOASSERTION"));
    m.insert("licenseDeclared".into(), json!("NOASSERTION"));
    m.insert("copyrightText".into(), json!("NOASSERTION"));
    m.insert(
        "externalRefs".into(),
        json!([{
            "referenceCategory": "PACKAGE-MANAGER",
            "referenceType": "purl",
            "referenceLocator": purl
        }]),
    );
    Value::Object(m)
}

fn spdx_tag(components: &[Component], root: Option<&Root>, timestamp: &str) -> String {
    let doc_name = root.map(|r| r.name.as_str()).unwrap_or("sbom-document");
    let mut s = String::new();
    s.push_str(&format!("SPDXVersion: {SPDX_VERSION}\n"));
    s.push_str("DataLicense: CC0-1.0\n");
    s.push_str("SPDXID: SPDXRef-DOCUMENT\n");
    s.push_str(&format!("DocumentName: {doc_name}\n"));
    s.push_str(&format!(
        "DocumentNamespace: https://spdx.org/spdxdocs/{}\n",
        sanitize_ns(doc_name)
    ));
    s.push_str(&format!("Creator: Tool: {TOOL_NAME}\n"));
    s.push_str(&format!("Created: {timestamp}\n"));

    // DOCUMENT DESCRIBES relationship for the root.
    if root.is_some() {
        s.push_str(&format!("\nRelationship: SPDXRef-DOCUMENT DESCRIBES {SPDX_ROOT_ID}\n"));
    }

    let push_pkg = |s: &mut String, id: &str, name: &str, version: Option<&str>, purl: &str| {
        s.push('\n');
        s.push_str(&format!("PackageName: {name}\n"));
        s.push_str(&format!("SPDXID: {id}\n"));
        if let Some(v) = version {
            s.push_str(&format!("PackageVersion: {v}\n"));
        }
        s.push_str("PackageDownloadLocation: NOASSERTION\n");
        s.push_str("FilesAnalyzed: false\n");
        s.push_str("PackageLicenseConcluded: NOASSERTION\n");
        s.push_str("PackageLicenseDeclared: NOASSERTION\n");
        s.push_str("PackageCopyrightText: NOASSERTION\n");
        s.push_str(&format!("ExternalRef: PACKAGE-MANAGER purl {purl}\n"));
    };

    if let Some(r) = root {
        let purl = Component { eco: r.eco, name: r.name.clone(), version: r.version.clone() }.purl();
        push_pkg(&mut s, SPDX_ROOT_ID, &r.name, r.version.as_deref(), &purl);
    }
    for (i, c) in components.iter().enumerate() {
        let id = spdx_pkg_id(i);
        push_pkg(&mut s, &id, &c.name, c.version.as_deref(), &c.purl());
    }

    // DEPENDS_ON / DESCRIBES edges after the package list.
    s.push('\n');
    for (i, _) in components.iter().enumerate() {
        let id = spdx_pkg_id(i);
        if root.is_some() {
            s.push_str(&format!("Relationship: {SPDX_ROOT_ID} DEPENDS_ON {id}\n"));
        } else {
            s.push_str(&format!("Relationship: SPDXRef-DOCUMENT DESCRIBES {id}\n"));
        }
    }
    s
}

/// Sanitize a name for use in an SPDX document namespace URL.
fn sanitize_ns(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') { c } else { '-' })
        .collect();
    let s = s.trim_matches('-').to_string();
    if s.is_empty() {
        "sbom-document".to_string()
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NPM_V3: &str = r#"{
        "name": "my-app",
        "version": "1.2.3",
        "lockfileVersion": 3,
        "packages": {
            "": { "name": "my-app", "version": "1.2.3", "dependencies": { "lodash": "^4.17.21" } },
            "node_modules/lodash": { "version": "4.17.21" },
            "node_modules/@scope/util": { "version": "2.0.0" },
            "node_modules/mocha": { "version": "10.0.0", "dev": true }
        }
    }"#;

    const NPM_V1: &str = r#"{
        "name": "legacy-app",
        "version": "0.1.0",
        "lockfileVersion": 1,
        "dependencies": {
            "left-pad": { "version": "1.3.0" },
            "chalk": { "version": "5.0.0", "dependencies": {
                "ansi-styles": { "version": "6.0.0" }
            }},
            "jest": { "version": "29.0.0", "dev": true }
        }
    }"#;

    const CARGO: &str = r#"
version = 3

[[package]]
name = "my-crate"
version = "0.1.0"
dependencies = ["serde"]

[[package]]
name = "serde"
version = "1.0.200"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "abc"

[[package]]
name = "serde"
version = "1.0.200"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "abc"
"#;

    const PIP: &str = "# a requirements file\n\
        Flask==2.3.0\n\
        requests==2.31.0  # http client\n\
        Django[argon2]==4.2 ; python_version >= '3.8'\n\
        numpy>=1.24\n\
        -r other.txt\n\
        -e .\n\
        urllib3 @ https://example.com/urllib3.whl\n\
        \n";

    fn cdx(input: &str, fmt: &str, dev: bool) -> Value {
        let s = generate(input, fmt, "cyclonedx-json", "", "", dev, "", true).unwrap();
        serde_json::from_str(&s).unwrap()
    }

    #[test]
    fn npm_v3_components_and_root() {
        let v = cdx(NPM_V3, "auto", true);
        assert_eq!(v["bomFormat"], "CycloneDX");
        assert_eq!(v["metadata"]["component"]["name"], "my-app");
        assert_eq!(v["metadata"]["component"]["purl"], "pkg:npm/my-app@1.2.3");
        let comps = v["components"].as_array().unwrap();
        // lodash, @scope/util, mocha  (root "" excluded)
        assert_eq!(comps.len(), 3);
        let purls: Vec<&str> = comps.iter().map(|c| c["purl"].as_str().unwrap()).collect();
        assert!(purls.contains(&"pkg:npm/lodash@4.17.21"));
        assert!(purls.contains(&"pkg:npm/%40scope/util@2.0.0"), "scoped purl encoded: {purls:?}");
        assert!(purls.contains(&"pkg:npm/mocha@10.0.0"));
    }

    #[test]
    fn npm_include_dev_false_drops_dev_deps() {
        let v = cdx(NPM_V3, "npm", false);
        let comps = v["components"].as_array().unwrap();
        assert_eq!(comps.len(), 2, "mocha (dev) dropped");
        let purls: Vec<&str> = comps.iter().map(|c| c["purl"].as_str().unwrap()).collect();
        assert!(!purls.iter().any(|p| p.contains("mocha")));
    }

    #[test]
    fn npm_v1_nested_tree_flattened() {
        let v = cdx(NPM_V1, "auto", true);
        let comps = v["components"].as_array().unwrap();
        // left-pad, chalk, ansi-styles, jest
        assert_eq!(comps.len(), 4);
        let names: Vec<&str> = comps.iter().map(|c| c["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"ansi-styles"), "nested dep flattened: {names:?}");
        assert_eq!(v["metadata"]["component"]["name"], "legacy-app");
        // dev excluded when include_dev=false
        let v2 = cdx(NPM_V1, "auto", false);
        assert_eq!(v2["components"].as_array().unwrap().len(), 3, "jest dropped");
    }

    #[test]
    fn cargo_packages_deduped_and_root() {
        let v = cdx(CARGO, "auto", true);
        let comps = v["components"].as_array().unwrap();
        // my-crate + serde (duplicate serde entry deduped)
        assert_eq!(comps.len(), 2, "duplicate serde collapsed: {comps:?}");
        let purls: Vec<&str> = comps.iter().map(|c| c["purl"].as_str().unwrap()).collect();
        assert!(purls.contains(&"pkg:cargo/serde@1.0.200"));
        assert_eq!(v["metadata"]["component"]["name"], "my-crate");
        assert_eq!(v["metadata"]["component"]["purl"], "pkg:cargo/my-crate@0.1.0");
    }

    #[test]
    fn pip_parses_pins_extras_markers_skips_directives() {
        let v = cdx(PIP, "auto", true);
        let comps = v["components"].as_array().unwrap();
        let purls: Vec<&str> = comps.iter().map(|c| c["purl"].as_str().unwrap()).collect();
        assert!(purls.contains(&"pkg:pypi/flask@2.3.0"), "PEP503 lowercased: {purls:?}");
        assert!(purls.contains(&"pkg:pypi/requests@2.31.0"), "inline comment stripped");
        assert!(purls.contains(&"pkg:pypi/django@4.2"), "extras stripped, marker dropped");
        // numpy>=1.24 has no exact pin -> no version in the purl
        assert!(purls.contains(&"pkg:pypi/numpy"), "unpinned -> versionless purl: {purls:?}");
        // urllib3 @ URL -> name only
        assert!(purls.contains(&"pkg:pypi/urllib3"), "direct-url requirement: {purls:?}");
        // -r / -e directives skipped
        assert_eq!(comps.len(), 5, "5 real packages: {purls:?}");
        // pip has no root project
        assert!(v["metadata"].get("component").is_none());
    }

    #[test]
    fn spdx_json_shape() {
        let s = generate(NPM_V3, "npm", "spdx-json", "", "", true, "2024-05-01T00:00:00Z", true).unwrap();
        let v: Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["spdxVersion"], "SPDX-2.3");
        assert_eq!(v["dataLicense"], "CC0-1.0");
        assert_eq!(v["SPDXID"], "SPDXRef-DOCUMENT");
        assert_eq!(v["name"], "my-app");
        assert_eq!(v["creationInfo"]["created"], "2024-05-01T00:00:00Z");
        assert_eq!(v["creationInfo"]["creators"][0], "Tool: gizza-sbom-generator");
        // root package + 3 deps
        assert_eq!(v["packages"].as_array().unwrap().len(), 4);
        assert_eq!(v["packages"][0]["SPDXID"], "SPDXRef-Package-Root");
        let ext = &v["packages"][1]["externalRefs"][0];
        assert_eq!(ext["referenceType"], "purl");
        // DESCRIBES root + DEPENDS_ON x3
        let rels = v["relationships"].as_array().unwrap();
        assert_eq!(rels.len(), 4);
        assert_eq!(rels[0]["relationshipType"], "DESCRIBES");
        assert_eq!(rels[0]["relatedSpdxElement"], "SPDXRef-Package-Root");
        assert!(rels.iter().any(|r| r["relationshipType"] == "DEPENDS_ON"));
    }

    #[test]
    fn spdx_tag_shape() {
        let s = generate(CARGO, "cargo", "spdx-tag", "", "", true, "", false).unwrap();
        assert!(s.starts_with("SPDXVersion: SPDX-2.3\n"));
        assert!(s.contains("DataLicense: CC0-1.0\n"));
        assert!(s.contains("DocumentName: my-crate\n"));
        assert!(s.contains(&format!("Created: {DEFAULT_TIMESTAMP}\n")), "default timestamp: {s}");
        assert!(s.contains("SPDXID: SPDXRef-Package-Root\n"));
        assert!(s.contains("ExternalRef: PACKAGE-MANAGER purl pkg:cargo/serde@1.0.200\n"));
        assert!(s.contains("Relationship: SPDXRef-DOCUMENT DESCRIBES SPDXRef-Package-Root\n"));
        assert!(s.contains("Relationship: SPDXRef-Package-Root DEPENDS_ON SPDXRef-Package-0\n"));
    }

    #[test]
    fn explicit_component_name_overrides() {
        let v = cdx(PIP, "pip", true);
        assert!(v["metadata"].get("component").is_none());
        let s = generate(PIP, "pip", "cyclonedx-json", "my-py-service", "9.9", true, "", true).unwrap();
        let v: Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["metadata"]["component"]["name"], "my-py-service");
        assert_eq!(v["metadata"]["component"]["version"], "9.9");
        assert_eq!(v["metadata"]["component"]["purl"], "pkg:pypi/my-py-service@9.9");
    }

    #[test]
    fn timestamp_optional_in_cyclonedx() {
        let v = cdx(NPM_V3, "npm", true);
        assert!(v["metadata"].get("timestamp").is_none(), "no timestamp by default");
        let s = generate(NPM_V3, "npm", "cyclonedx-json", "", "", true, "2024-01-01T12:00:00Z", true).unwrap();
        let v: Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["metadata"]["timestamp"], "2024-01-01T12:00:00Z");
    }

    #[test]
    fn components_are_sorted_deterministically() {
        let s1 = generate(NPM_V3, "npm", "cyclonedx-json", "", "", true, "", true).unwrap();
        let s2 = generate(NPM_V3, "npm", "cyclonedx-json", "", "", true, "", true).unwrap();
        assert_eq!(s1, s2, "identical input -> identical SBOM");
        let v: Value = serde_json::from_str(&s1).unwrap();
        let names: Vec<&str> = v["components"].as_array().unwrap()
            .iter().map(|c| c["name"].as_str().unwrap()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted, "components sorted by name");
    }

    #[test]
    fn pretty_vs_compact() {
        let pretty = generate(NPM_V3, "npm", "cyclonedx-json", "", "", true, "", true).unwrap();
        let compact = generate(NPM_V3, "npm", "cyclonedx-json", "", "", true, "", false).unwrap();
        assert!(pretty.contains('\n'));
        assert!(!compact.contains('\n'));
    }

    #[test]
    fn rejects_bad_input() {
        assert_eq!(generate("   ", "auto", "cyclonedx-json", "", "", true, "", true).unwrap_err(), "no lockfile input");
        let e = generate("{}", "npm", "cyclonedx-json", "", "", true, "", true).unwrap_err();
        assert!(e.contains("missing both"), "{e}");
        let e = generate("not json", "npm", "cyclonedx-json", "", "", true, "", true).unwrap_err();
        assert!(e.starts_with("invalid JSON:"), "{e}");
        let e = generate(CARGO, "cargo", "bogus", "", "", true, "", true).unwrap_err();
        assert!(e.contains("unknown output 'bogus'"), "{e}");
        let e = generate(NPM_V3, "ruby", "cyclonedx-json", "", "", true, "", true).unwrap_err();
        assert!(e.contains("unknown input_format 'ruby'"), "{e}");
    }

    #[test]
    fn empty_requirements_errors() {
        let e = generate("# just a comment\n\n-r other.txt\n", "pip", "cyclonedx-json", "", "", true, "", true).unwrap_err();
        assert!(e.contains("no packages found"), "{e}");
    }
}

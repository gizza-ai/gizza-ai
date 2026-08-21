//! k8s-manifest-splitter core — pure compute, shared by the chat skill block and the web page.
//!
//! Splits a multi-document Kubernetes YAML stream (the output of `helm template`,
//! `kustomize build`, `kubectl get -o yaml`, or a hand-written bundle) into its
//! individual resources, names each one from a filename template, and renders the
//! result as a file bundle, an index table, JSON, a `kustomization.yaml`, or a
//! POSIX script that recreates the files.
//!
//! Document bodies are kept BYTE-VERBATIM (comments, ordering and formatting are
//! preserved); YAML is only parsed to read `apiVersion`, `kind` and `metadata.*`.
//! The one exception is `expand_lists`, where items lifted out of a `*List`
//! wrapper have to be re-serialized.

use std::collections::HashMap;

/// Largest manifest accepted, in bytes.
pub const MAX_INPUT_BYTES: usize = 2_000_000;
/// Largest number of YAML documents accepted in one manifest.
pub const MAX_DOCUMENTS: usize = 1_000;

/// Default filename template — kubectl-style `deployment-web.yaml`.
pub const DEFAULT_TEMPLATE: &str = "{kind}-{name}.yaml";

/// Heredoc delimiter used by the `shell` output.
const HEREDOC: &str = "K8S_SPLIT_EOF";

/// Placeholders accepted in a filename template, in documentation order.
pub const PLACEHOLDERS: &[&str] = &[
    "{kind}",
    "{Kind}",
    "{name}",
    "{namespace}",
    "{apiVersion}",
    "{group}",
    "{version}",
    "{index}",
];

/// Kind ordering used by `sort = "apply"` — the dependency-safe install order
/// (namespaces and CRDs first, workloads and ingress last).
const APPLY_ORDER: &[&str] = &[
    "Namespace",
    "NetworkPolicy",
    "ResourceQuota",
    "LimitRange",
    "PodSecurityPolicy",
    "PodDisruptionBudget",
    "ServiceAccount",
    "Secret",
    "SecretList",
    "ConfigMap",
    "StorageClass",
    "PersistentVolume",
    "PersistentVolumeClaim",
    "CustomResourceDefinition",
    "ClusterRole",
    "ClusterRoleList",
    "ClusterRoleBinding",
    "ClusterRoleBindingList",
    "Role",
    "RoleList",
    "RoleBinding",
    "RoleBindingList",
    "Service",
    "DaemonSet",
    "Pod",
    "ReplicationController",
    "ReplicaSet",
    "Deployment",
    "HorizontalPodAutoscaler",
    "StatefulSet",
    "Job",
    "CronJob",
    "Ingress",
    "APIService",
];

/// What the tool renders.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Output {
    /// Every document, each under a `# ===== <file> =====` header.
    Files,
    /// An aligned table of the resources and the filenames they would get.
    Index,
    /// A JSON array with one object per resource (including its YAML).
    Json,
    /// A `kustomization.yaml` listing the filenames.
    Kustomization,
    /// A POSIX `sh` script that writes every file.
    Shell,
}

impl Output {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "files" => Ok(Output::Files),
            "index" => Ok(Output::Index),
            "json" => Ok(Output::Json),
            "kustomization" => Ok(Output::Kustomization),
            "shell" => Ok(Output::Shell),
            other => Err(format!(
                "unknown output '{other}': expected one of files, index, json, kustomization, shell"
            )),
        }
    }
}

/// Order the resources are emitted in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sort {
    /// Keep the order the documents appear in the input.
    Document,
    /// Alphabetical by kind, then name.
    Kind,
    /// Alphabetical by name, then kind.
    Name,
    /// Dependency-safe apply order (Namespace → CRD → … → Deployment → Ingress).
    Apply,
}

impl Sort {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "document" => Ok(Sort::Document),
            "kind" => Ok(Sort::Kind),
            "name" => Ok(Sort::Name),
            "apply" => Ok(Sort::Apply),
            other => Err(format!(
                "unknown sort '{other}': expected one of document, kind, name, apply"
            )),
        }
    }
}

/// Everything the splitter can be told, beside the manifest itself.
#[derive(Clone, Debug)]
pub struct Options {
    pub output: Output,
    pub filename_template: String,
    pub include: String,
    pub exclude: String,
    pub sort: Sort,
    pub skip_non_k8s: bool,
    pub expand_lists: bool,
    pub include_triple_dash: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            output: Output::Files,
            filename_template: DEFAULT_TEMPLATE.to_string(),
            include: String::new(),
            exclude: String::new(),
            sort: Sort::Document,
            skip_non_k8s: false,
            expand_lists: true,
            include_triple_dash: false,
        }
    }
}

/// One split-out resource.
#[derive(Clone, Debug, PartialEq)]
pub struct Resource {
    /// Verbatim YAML of the document (no leading `---`, no trailing blank lines).
    pub text: String,
    pub api_version: String,
    pub kind: String,
    pub name: String,
    pub namespace: String,
    /// 1-based position of the document in the input.
    pub source_index: usize,
    /// Filename assigned by the template (filled in after sorting).
    pub file: String,
    /// Parsed document, kept so a filename template can read any field by path.
    pub doc: serde_yml::Value,
}

impl Resource {
    /// Does the document carry the three fields every real Kubernetes object has?
    pub fn is_k8s(&self) -> bool {
        !self.api_version.is_empty() && !self.kind.is_empty() && !self.name.is_empty()
    }

    pub fn lines(&self) -> usize {
        self.text.lines().count()
    }
}

// ---------------------------------------------------------------- splitting

/// Split a YAML stream on column-0 `---` / `...` markers.
///
/// A `---` at column 0 always starts a new document in YAML (block-scalar and
/// flow-scalar continuation lines are required to be indented), so this is a
/// faithful split that never has to reformat the body.
fn split_stream(src: &str) -> Vec<String> {
    let mut docs: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut after_end_marker = false;

    for line in src.lines() {
        let trimmed = line.trim_end();
        if trimmed == "---" || trimmed.starts_with("--- ") || trimmed.starts_with("---\t") {
            docs.push(std::mem::take(&mut cur));
            after_end_marker = false;
            let rest = trimmed[3..].trim_start();
            if !rest.is_empty() && !rest.starts_with('#') {
                cur.push_str(rest);
                cur.push('\n');
            }
            continue;
        }
        if trimmed == "..." {
            docs.push(std::mem::take(&mut cur));
            after_end_marker = true;
            continue;
        }
        if after_end_marker {
            // Content resuming after `...` without a `---` opens a new document.
            after_end_marker = false;
        }
        cur.push_str(line);
        cur.push('\n');
    }
    docs.push(cur);
    docs
}

/// A document with nothing but blank lines and comments carries no resource.
fn is_empty_document(text: &str) -> bool {
    text.lines()
        .all(|l| l.trim().is_empty() || l.trim_start().starts_with('#') || l.trim_start().starts_with('%'))
}

fn trim_blank_edges(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.iter().position(|l| !l.trim().is_empty()).unwrap_or(0);
    let end = lines
        .iter()
        .rposition(|l| !l.trim().is_empty())
        .map(|i| i + 1)
        .unwrap_or(0);
    lines[start..end].join("\n")
}

fn field(value: &serde_yml::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn meta_field(value: &serde_yml::Value, key: &str) -> String {
    value
        .get("metadata")
        .map(|m| field(m, key))
        .unwrap_or_default()
}

/// Parse the stream into resources, expanding `*List` wrappers when asked.
pub fn parse_resources(manifest: &str, expand_lists: bool) -> Result<Vec<Resource>, String> {
    if manifest.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "manifest is {} bytes: the limit is {} bytes ({} MB)",
            manifest.len(),
            MAX_INPUT_BYTES,
            MAX_INPUT_BYTES / 1_000_000
        ));
    }
    if manifest.trim().is_empty() {
        return Err("manifest is empty: paste a Kubernetes YAML manifest (documents separated by '---')".into());
    }

    let raw: Vec<String> = split_stream(manifest)
        .into_iter()
        .filter(|d| !is_empty_document(d))
        .collect();

    if raw.is_empty() {
        return Err("manifest contains no YAML documents: only blank lines and comments were found".into());
    }
    if raw.len() > MAX_DOCUMENTS {
        return Err(format!(
            "manifest holds {} documents: the limit is {}",
            raw.len(),
            MAX_DOCUMENTS
        ));
    }

    let mut out: Vec<Resource> = Vec::new();
    for (i, doc) in raw.iter().enumerate() {
        let position = i + 1;
        let text = trim_blank_edges(doc);
        let value: serde_yml::Value = serde_yml::from_str(&text)
            .map_err(|e| format!("YAML parse error in document {position}: {e}"))?;

        let kind = field(&value, "kind");
        let is_list = expand_lists
            && kind.ends_with("List")
            && value.get("items").map(|v| v.is_sequence()).unwrap_or(false);

        if is_list {
            let items = value
                .get("items")
                .and_then(|v| v.as_sequence())
                .cloned()
                .unwrap_or_default();
            for item in items {
                let body = serde_yml::to_string(&item)
                    .map_err(|e| format!("cannot re-serialize an item of document {position}: {e}"))?;
                let body = body.strip_prefix("---\n").unwrap_or(&body).to_string();
                out.push(Resource {
                    api_version: field(&item, "apiVersion"),
                    kind: field(&item, "kind"),
                    name: meta_field(&item, "name"),
                    namespace: meta_field(&item, "namespace"),
                    text: trim_blank_edges(&body),
                    source_index: position,
                    file: String::new(),
                    doc: item,
                });
            }
            continue;
        }

        out.push(Resource {
            api_version: field(&value, "apiVersion"),
            kind,
            name: meta_field(&value, "name"),
            namespace: meta_field(&value, "namespace"),
            text,
            source_index: position,
            file: String::new(),
            doc: value,
        });
    }
    Ok(out)
}

// ---------------------------------------------------------------- filtering

/// Case-insensitive `*` / `?` glob.
fn glob_match(pattern: &str, subject: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let s: Vec<char> = subject.to_lowercase().chars().collect();
    // Iterative backtracking matcher — linear in the common case, no recursion.
    let (mut pi, mut si) = (0usize, 0usize);
    let (mut star, mut mark) = (usize::MAX, 0usize);
    while si < s.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == s[si]) {
            pi += 1;
            si += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = pi;
            mark = si;
            pi += 1;
        } else if star != usize::MAX {
            pi = star + 1;
            mark += 1;
            si = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

/// A selector is `kind/name`; a bare selector matches the kind only.
fn selector_matches(selector: &str, res: &Resource) -> bool {
    let sel = selector.trim();
    if sel.is_empty() {
        return false;
    }
    let (kind_pat, name_pat) = match sel.split_once('/') {
        Some((k, n)) => (k.trim(), n.trim()),
        None => (sel, "*"),
    };
    let kind_pat = if kind_pat.is_empty() { "*" } else { kind_pat };
    let name_pat = if name_pat.is_empty() { "*" } else { name_pat };
    glob_match(kind_pat, &res.kind) && glob_match(name_pat, &res.name)
}

fn selectors(list: &str) -> Vec<&str> {
    list.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect()
}

// ---------------------------------------------------------------- filenames

fn sanitize(value: &str) -> String {
    value
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '-' })
        .collect()
}

/// Read a dotted path (`metadata.labels.app`, `spec.template.spec.containers.0.image`)
/// out of the parsed document. Only scalars can name a file, so a path landing on a
/// mapping or a sequence is an error rather than a stringified blob.
fn path_value(doc: &serde_yml::Value, path: &str) -> Result<String, String> {
    let mut cur = doc;
    for step in path.split('.') {
        let next = match cur {
            serde_yml::Value::Sequence(items) => step
                .parse::<usize>()
                .ok()
                .and_then(|i| items.get(i)),
            _ => cur.get(step),
        };
        cur = next.ok_or_else(|| {
            format!("filename template asks for '{{{path}}}', which this document does not have")
        })?;
    }
    match cur {
        serde_yml::Value::String(s) => Ok(s.trim().to_string()),
        serde_yml::Value::Number(n) => Ok(n.to_string()),
        serde_yml::Value::Bool(b) => Ok(b.to_string()),
        _ => Err(format!(
            "filename template asks for '{{{path}}}', which is not a single value in this document"
        )),
    }
}

fn apply_template(template: &str, res: &Resource, index: usize, width: usize) -> Result<String, String> {
    let kind = if res.kind.is_empty() { "unknown".to_string() } else { res.kind.clone() };
    let name = if res.name.is_empty() { "unnamed".to_string() } else { res.name.clone() };
    let namespace = if res.namespace.is_empty() { "default".to_string() } else { res.namespace.clone() };
    let (group, version) = match res.api_version.split_once('/') {
        Some((g, v)) => (g.to_string(), v.to_string()),
        None => ("core".to_string(), res.api_version.clone()),
    };

    let mut out = String::with_capacity(template.len() + 24);
    let chars: Vec<char> = template.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '{' {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        let close = chars[i..]
            .iter()
            .position(|c| *c == '}')
            .map(|p| i + p)
            .ok_or_else(|| format!("unclosed '{{' in filename template '{template}'"))?;
        let key: String = chars[i + 1..close].iter().collect();
        let value = match key.as_str() {
            "kind" => sanitize(&kind.to_lowercase()),
            "Kind" => sanitize(&kind),
            "name" => sanitize(&name),
            "namespace" => sanitize(&namespace),
            "apiVersion" => sanitize(&res.api_version),
            "group" => sanitize(&group),
            "version" => sanitize(&version),
            "index" => format!("{:0width$}", index, width = width),
            other if other.contains('.') => sanitize(&path_value(&res.doc, other)?),
            other => {
                return Err(format!(
                    "unknown placeholder '{{{other}}}' in filename template: supported placeholders are {}, or any dotted field path such as {{metadata.labels.app}}",
                    PLACEHOLDERS.join(", ")
                ))
            }
        };
        out.push_str(&value);
        i = close + 1;
    }
    if out.trim().is_empty() || out.trim() == "/" {
        return Err(format!("filename template '{template}' produced an empty filename"));
    }
    Ok(out)
}

/// Give the same filename twice? The second one gets `-2`, the third `-3`, …
fn dedupe(name: String, seen: &mut HashMap<String, usize>) -> String {
    let count = seen.entry(name.clone()).or_insert(0);
    *count += 1;
    if *count == 1 {
        return name;
    }
    match name.rfind('.') {
        Some(dot) if dot > 0 => format!("{}-{}{}", &name[..dot], count, &name[dot..]),
        _ => format!("{name}-{count}"),
    }
}

// ---------------------------------------------------------------- rendering

fn pad(value: &str, width: usize) -> String {
    let mut s = value.to_string();
    while s.chars().count() < width {
        s.push(' ');
    }
    s
}

fn render_index(list: &[Resource]) -> String {
    let headers = ["#", "KIND", "NAME", "NAMESPACE", "APIVERSION", "LINES", "FILE"];
    let mut rows: Vec<[String; 7]> = Vec::with_capacity(list.len());
    for (i, r) in list.iter().enumerate() {
        rows.push([
            (i + 1).to_string(),
            if r.kind.is_empty() { "-".into() } else { r.kind.clone() },
            if r.name.is_empty() { "-".into() } else { r.name.clone() },
            if r.namespace.is_empty() { "-".into() } else { r.namespace.clone() },
            if r.api_version.is_empty() { "-".into() } else { r.api_version.clone() },
            r.lines().to_string(),
            r.file.clone(),
        ]);
    }
    let mut widths = [0usize; 7];
    for (c, h) in headers.iter().enumerate() {
        widths[c] = h.chars().count();
    }
    for row in &rows {
        for (c, cell) in row.iter().enumerate() {
            widths[c] = widths[c].max(cell.chars().count());
        }
    }

    let mut out = String::new();
    let render = |cells: &[String], out: &mut String| {
        let line: Vec<String> = cells
            .iter()
            .enumerate()
            .map(|(c, cell)| if c == cells.len() - 1 { cell.clone() } else { pad(cell, widths[c]) })
            .collect();
        out.push_str(line.join("  ").trim_end());
        out.push('\n');
    };
    render(&headers.iter().map(|h| h.to_string()).collect::<Vec<_>>(), &mut out);
    for row in &rows {
        render(row, &mut out);
    }

    let mut kinds: Vec<&str> = list.iter().map(|r| r.kind.as_str()).collect();
    kinds.sort_unstable();
    kinds.dedup();
    out.push('\n');
    out.push_str(&format!(
        "{} document{}, {} kind{}",
        list.len(),
        if list.len() == 1 { "" } else { "s" },
        kinds.len(),
        if kinds.len() == 1 { "" } else { "s" }
    ));
    out.push('\n');
    out
}

fn render_files(list: &[Resource], triple_dash: bool) -> String {
    let mut chunks: Vec<String> = Vec::with_capacity(list.len());
    for r in list {
        let mut chunk = format!("# ===== {} =====\n", r.file);
        if triple_dash {
            chunk.push_str("---\n");
        }
        chunk.push_str(&r.text);
        chunk.push('\n');
        chunks.push(chunk);
    }
    chunks.join("\n")
}

fn render_json(list: &[Resource]) -> String {
    let arr: Vec<serde_json::Value> = list
        .iter()
        .enumerate()
        .map(|(i, r)| {
            serde_json::json!({
                "index": i + 1,
                "file": r.file,
                "apiVersion": r.api_version,
                "kind": r.kind,
                "name": r.name,
                "namespace": r.namespace,
                "lines": r.lines(),
                "content": r.text,
            })
        })
        .collect();
    let mut s = serde_json::to_string_pretty(&arr).unwrap_or_else(|_| "[]".to_string());
    s.push('\n');
    s
}

fn render_kustomization(list: &[Resource]) -> String {
    let mut out = String::from("apiVersion: kustomize.config.k8s.io/v1beta1\nkind: Kustomization\nresources:\n");
    let mut seen: Vec<&str> = Vec::new();
    for r in list {
        if seen.contains(&r.file.as_str()) {
            continue;
        }
        seen.push(&r.file);
        out.push_str(&format!("  - {}\n", r.file));
    }
    out
}

fn render_shell(list: &[Resource]) -> Result<String, String> {
    for r in list {
        if r.text.lines().any(|l| l.trim_end() == HEREDOC) {
            return Err(format!(
                "document '{}' contains a line equal to the heredoc marker {HEREDOC}: use the 'files' output instead",
                r.file
            ));
        }
    }
    let mut out = String::from("#!/bin/sh\n");
    out.push_str(&format!(
        "# Recreates {} file{} split from a multi-document Kubernetes manifest.\n",
        list.len(),
        if list.len() == 1 { "" } else { "s" }
    ));
    out.push_str("set -e\n");
    for r in list {
        out.push('\n');
        if r.file.contains('/') {
            out.push_str(&format!("mkdir -p \"$(dirname '{}')\"\n", r.file));
        }
        out.push_str(&format!("cat > '{}' <<'{HEREDOC}'\n", r.file));
        out.push_str(&r.text);
        out.push('\n');
        out.push_str(HEREDOC);
        out.push('\n');
    }
    Ok(out)
}

// ---------------------------------------------------------------- entry point

fn apply_rank(kind: &str) -> usize {
    APPLY_ORDER
        .iter()
        .position(|k| k.eq_ignore_ascii_case(kind))
        .unwrap_or(APPLY_ORDER.len())
}

/// Split `manifest` and render it according to `opts`.
pub fn split(manifest: &str, opts: &Options) -> Result<String, String> {
    let mut list = parse_resources(manifest, opts.expand_lists)?;

    if opts.skip_non_k8s {
        list.retain(|r| r.is_k8s());
        if list.is_empty() {
            return Err("no Kubernetes resources left: every document was missing apiVersion, kind or metadata.name".into());
        }
    }

    let include = selectors(&opts.include);
    if !include.is_empty() {
        list.retain(|r| include.iter().any(|s| selector_matches(s, r)));
    }
    let exclude = selectors(&opts.exclude);
    if !exclude.is_empty() {
        list.retain(|r| !exclude.iter().any(|s| selector_matches(s, r)));
    }
    if list.is_empty() {
        return Err(format!(
            "no documents matched the filters (include '{}', exclude '{}'): selectors are 'Kind' or 'Kind/name' and accept * wildcards",
            opts.include, opts.exclude
        ));
    }

    match opts.sort {
        Sort::Document => list.sort_by_key(|r| r.source_index),
        Sort::Kind => list.sort_by(|a, b| {
            a.kind
                .to_lowercase()
                .cmp(&b.kind.to_lowercase())
                .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                .then(a.source_index.cmp(&b.source_index))
        }),
        Sort::Name => list.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then(a.kind.to_lowercase().cmp(&b.kind.to_lowercase()))
                .then(a.source_index.cmp(&b.source_index))
        }),
        Sort::Apply => list.sort_by(|a, b| {
            apply_rank(&a.kind)
                .cmp(&apply_rank(&b.kind))
                .then(a.kind.to_lowercase().cmp(&b.kind.to_lowercase()))
                .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                .then(a.source_index.cmp(&b.source_index))
        }),
    }

    let template = if opts.filename_template.trim().is_empty() {
        DEFAULT_TEMPLATE
    } else {
        opts.filename_template.trim()
    };
    let width = list.len().to_string().len().max(2);
    let mut seen: HashMap<String, usize> = HashMap::new();
    for (i, r) in list.iter_mut().enumerate() {
        let file = apply_template(template, r, i + 1, width)?;
        r.file = dedupe(file, &mut seen);
    }

    Ok(match opts.output {
        Output::Files => render_files(&list, opts.include_triple_dash),
        Output::Index => render_index(&list),
        Output::Json => render_json(&list),
        Output::Kustomization => render_kustomization(&list),
        Output::Shell => render_shell(&list)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "apiVersion: v1\nkind: Service\nmetadata:\n  name: web\n  namespace: prod\nspec:\n  ports:\n    - port: 80\n---\napiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: web\nspec:\n  replicas: 2\n";

    fn opts(output: Output) -> Options {
        Options { output, ..Options::default() }
    }

    #[test]
    fn splits_two_documents_into_named_files() {
        let out = split(SAMPLE, &opts(Output::Files)).unwrap();
        assert_eq!(
            out,
            "# ===== service-web.yaml =====\napiVersion: v1\nkind: Service\nmetadata:\n  name: web\n  namespace: prod\nspec:\n  ports:\n    - port: 80\n\n# ===== deployment-web.yaml =====\napiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: web\nspec:\n  replicas: 2\n"
        );
    }

    #[test]
    fn index_table_lists_every_resource() {
        let out = split(SAMPLE, &opts(Output::Index)).unwrap();
        assert_eq!(
            out,
            "#  KIND        NAME  NAMESPACE  APIVERSION  LINES  FILE\n\
             1  Service     web   prod       v1          8      service-web.yaml\n\
             2  Deployment  web   -          apps/v1     6      deployment-web.yaml\n\
             \n2 documents, 2 kinds\n"
        );
    }

    #[test]
    fn leading_separator_and_helm_comments_are_kept_verbatim() {
        let src = "---\n# Source: chart/templates/svc.yaml\napiVersion: v1\nkind: Service\nmetadata:\n  name: api\n";
        let out = split(src, &opts(Output::Files)).unwrap();
        assert!(out.contains("# Source: chart/templates/svc.yaml"));
        assert!(out.starts_with("# ===== service-api.yaml ====="));
    }

    #[test]
    fn document_end_marker_closes_a_document() {
        let src = "kind: ConfigMap\napiVersion: v1\nmetadata:\n  name: a\n...\nkind: ConfigMap\napiVersion: v1\nmetadata:\n  name: b\n";
        let list = parse_resources(src, true).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[1].name, "b");
    }

    #[test]
    fn triple_dash_inside_a_block_scalar_is_not_a_separator() {
        let src = "apiVersion: v1\nkind: ConfigMap\nmetadata:\n  name: notes\ndata:\n  readme: |\n    ---\n    still the same document\n";
        let list = parse_resources(src, true).unwrap();
        assert_eq!(list.len(), 1);
        assert!(list[0].text.contains("still the same document"));
    }

    #[test]
    fn include_and_exclude_selectors_filter_by_kind_and_name() {
        let mut o = opts(Output::Index);
        o.include = "Deployment,Service/web".into();
        o.exclude = "Service/*".into();
        let out = split(SAMPLE, &o).unwrap();
        assert!(out.contains("deployment-web.yaml"));
        assert!(!out.contains("service-web.yaml"));
    }

    #[test]
    fn glob_selectors_are_case_insensitive() {
        let mut o = opts(Output::Index);
        o.include = "deploy*".into();
        let out = split(SAMPLE, &o).unwrap();
        assert!(out.contains("1 document, 1 kind"));
        assert!(out.contains("Deployment"));
    }

    #[test]
    fn apply_sort_puts_dependencies_first() {
        let src = "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: web\n---\napiVersion: v1\nkind: Namespace\nmetadata:\n  name: prod\n---\napiVersion: v1\nkind: ConfigMap\nmetadata:\n  name: cfg\n";
        let mut o = opts(Output::Kustomization);
        o.sort = Sort::Apply;
        let out = split(src, &o).unwrap();
        assert_eq!(
            out,
            "apiVersion: kustomize.config.k8s.io/v1beta1\nkind: Kustomization\nresources:\n  - namespace-prod.yaml\n  - configmap-cfg.yaml\n  - deployment-web.yaml\n"
        );
    }

    #[test]
    fn template_placeholders_render_index_namespace_and_group() {
        let mut o = opts(Output::Kustomization);
        o.filename_template = "{index}-{namespace}/{group}-{version}-{kind}-{name}.yml".into();
        let out = split(SAMPLE, &o).unwrap();
        assert!(out.contains("  - 01-prod/core-v1-service-web.yml"));
        assert!(out.contains("  - 02-default/apps-v1-deployment-web.yml"));
    }

    #[test]
    fn dotted_paths_name_files_from_any_field() {
        let src = "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: web\n  labels:\n    app: storefront\nspec:\n  replicas: 3\n  template:\n    spec:\n      containers:\n        - image: nginx:1.27\n";
        let mut o = opts(Output::Kustomization);
        o.filename_template = "{metadata.labels.app}-{spec.replicas}-{spec.template.spec.containers.0.image}.yaml".into();
        let out = split(src, &o).unwrap();
        assert!(out.contains("  - storefront-3-nginx-1.27.yaml"), "{out}");
    }

    #[test]
    fn a_missing_or_non_scalar_dotted_path_is_rejected() {
        let mut o = opts(Output::Files);
        o.filename_template = "{metadata.labels.app}.yaml".into();
        let err = split(SAMPLE, &o).unwrap_err();
        assert!(err.contains("'{metadata.labels.app}', which this document does not have"), "{err}");

        o.filename_template = "{metadata}.yaml".into();
        let err = split(SAMPLE, &o).unwrap_err();
        assert!(err.contains("unknown placeholder '{metadata}'"), "{err}");

        o.filename_template = "{spec.ports}.yaml".into();
        let err = split(SAMPLE, &o).unwrap_err();
        assert!(err.contains("not a single value"), "{err}");
    }

    #[test]
    fn duplicate_filenames_get_a_numeric_suffix() {
        let src = "apiVersion: v1\nkind: ConfigMap\nmetadata:\n  name: cfg\n  namespace: a\n---\napiVersion: v1\nkind: ConfigMap\nmetadata:\n  name: cfg\n  namespace: b\n";
        let out = split(src, &opts(Output::Kustomization)).unwrap();
        assert!(out.contains("  - configmap-cfg.yaml"));
        assert!(out.contains("  - configmap-cfg-2.yaml"));
    }

    #[test]
    fn list_wrappers_expand_into_their_items() {
        let src = "apiVersion: v1\nkind: List\nitems:\n  - apiVersion: v1\n    kind: Service\n    metadata:\n      name: a\n  - apiVersion: v1\n    kind: Service\n    metadata:\n      name: b\n";
        let list = parse_resources(src, true).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "a");
        assert!(list[0].text.contains("kind: Service"));

        let unexpanded = parse_resources(src, false).unwrap();
        assert_eq!(unexpanded.len(), 1);
        assert_eq!(unexpanded[0].kind, "List");
    }

    #[test]
    fn skip_non_k8s_drops_documents_without_the_identity_fields() {
        let src = "foo: bar\n---\napiVersion: v1\nkind: Service\nmetadata:\n  name: web\n";
        let mut o = opts(Output::Index);
        assert!(split(src, &o).unwrap().contains("unknown-unnamed.yaml"));
        o.skip_non_k8s = true;
        let out = split(src, &o).unwrap();
        assert!(!out.contains("unknown-unnamed.yaml"));
        assert!(out.contains("1 document, 1 kind"));
    }

    #[test]
    fn triple_dash_option_prefixes_every_document() {
        let mut o = opts(Output::Files);
        o.include_triple_dash = true;
        let out = split(SAMPLE, &o).unwrap();
        assert!(out.contains("# ===== service-web.yaml =====\n---\napiVersion: v1"));
    }

    #[test]
    fn json_output_carries_identity_and_content() {
        let out = split(SAMPLE, &opts(Output::Json)).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed[0]["file"], "service-web.yaml");
        assert_eq!(parsed[0]["namespace"], "prod");
        assert_eq!(parsed[1]["kind"], "Deployment");
        assert!(parsed[1]["content"].as_str().unwrap().contains("replicas: 2"));
    }

    #[test]
    fn shell_output_writes_a_heredoc_per_file() {
        let mut o = opts(Output::Shell);
        o.filename_template = "{kind}/{name}.yaml".into();
        let out = split(SAMPLE, &o).unwrap();
        assert!(out.starts_with("#!/bin/sh\n"));
        assert!(out.contains("mkdir -p \"$(dirname 'service/web.yaml')\""));
        assert!(out.contains("cat > 'service/web.yaml' <<'K8S_SPLIT_EOF'"));
        assert!(out.contains("\nK8S_SPLIT_EOF\n"));
    }

    #[test]
    fn empty_manifest_is_rejected() {
        let err = split("   \n", &opts(Output::Files)).unwrap_err();
        assert!(err.contains("manifest is empty"), "{err}");
    }

    #[test]
    fn comment_only_manifest_is_rejected() {
        let err = split("# nothing here\n---\n# still nothing\n", &opts(Output::Files)).unwrap_err();
        assert!(err.contains("no YAML documents"), "{err}");
    }

    #[test]
    fn malformed_yaml_names_the_document() {
        let src = "apiVersion: v1\nkind: Service\n---\nkey: [unclosed\n";
        let err = split(src, &opts(Output::Files)).unwrap_err();
        assert!(err.starts_with("YAML parse error in document 2:"), "{err}");
    }

    #[test]
    fn unknown_placeholder_is_rejected_with_the_supported_list() {
        let mut o = opts(Output::Files);
        o.filename_template = "{kind}-{label}.yaml".into();
        let err = split(SAMPLE, &o).unwrap_err();
        assert!(err.contains("unknown placeholder '{label}'"), "{err}");
        assert!(err.contains("{namespace}"), "{err}");
    }

    #[test]
    fn filters_that_match_nothing_are_an_error() {
        let mut o = opts(Output::Files);
        o.include = "Ingress".into();
        let err = split(SAMPLE, &o).unwrap_err();
        assert!(err.contains("no documents matched the filters"), "{err}");
    }

    #[test]
    fn unknown_output_and_sort_report_the_choices() {
        assert!(Output::parse("yaml").unwrap_err().contains("kustomization"));
        assert!(Sort::parse("size").unwrap_err().contains("apply"));
    }

    #[test]
    fn document_cap_is_enforced_at_the_boundary() {
        let doc = "apiVersion: v1\nkind: ConfigMap\nmetadata:\n  name: c\n";
        let at_cap = vec![doc; MAX_DOCUMENTS].join("---\n");
        assert_eq!(parse_resources(&at_cap, true).unwrap().len(), MAX_DOCUMENTS);
        let over_cap = vec![doc; MAX_DOCUMENTS + 1].join("---\n");
        let err = parse_resources(&over_cap, true).unwrap_err();
        assert!(err.contains("the limit is 1000"), "{err}");
    }

    #[test]
    fn oversized_manifest_is_rejected() {
        let big = "a".repeat(MAX_INPUT_BYTES + 1);
        let err = parse_resources(&big, true).unwrap_err();
        assert!(err.contains("the limit is 2000000 bytes"), "{err}");
    }
}

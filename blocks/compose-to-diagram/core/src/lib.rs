//! compose-to-diagram core — pure compute, shared by the chat skill block and
//! the web page. No wafer/wasm-bindgen deps.
//!
//! Parses a Docker Compose file (`docker-compose.yml`) far enough to recover its
//! architecture — services, `depends_on` edges (list *and* map form, including
//! `condition:`), `links`, `network_mode: service:*`, same-file `extends`,
//! published ports, named volumes and bind mounts, network membership, and the
//! `image:` / `build:` behind each service — and renders it as Mermaid
//! `flowchart` text, a Markdown-fenced diagram, or a plain-text summary report.
//!
//! Only the structure is read: environment values, commands, healthcheck bodies
//! and other runtime detail are deliberately ignored, and nothing is executed or
//! fetched. `${VAR}` interpolation is left verbatim (no `.env` file is read).

use serde::Deserialize;
use serde_yml::{Mapping, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Largest accepted compose document, in bytes.
pub const MAX_INPUT_BYTES: usize = 2_000_000;
/// Largest accepted number of services.
pub const MAX_SERVICES: usize = 500;

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Flowchart orientation.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction {
    Td,
    Lr,
    Bt,
    Rl,
}

impl Direction {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_uppercase().as_str() {
            "" | "TD" | "TB" => Ok(Direction::Td),
            "LR" => Ok(Direction::Lr),
            "BT" => Ok(Direction::Bt),
            "RL" => Ok(Direction::Rl),
            other => Err(format!(
                "unknown direction '{other}' (use 'TD', 'LR', 'BT', or 'RL')"
            )),
        }
    }
    fn keyword(self) -> &'static str {
        match self {
            Direction::Td => "TD",
            Direction::Lr => "LR",
            Direction::Bt => "BT",
            Direction::Rl => "RL",
        }
    }
}

/// How networks are drawn.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NetworkStyle {
    Subgraph,
    Node,
    Off,
}

impl NetworkStyle {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "subgraph" => Ok(NetworkStyle::Subgraph),
            "node" => Ok(NetworkStyle::Node),
            "off" | "none" => Ok(NetworkStyle::Off),
            other => Err(format!(
                "unknown networks '{other}' (use 'subgraph', 'node', or 'off')"
            )),
        }
    }
}

/// How much detail goes inside a service node's label.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LabelDetail {
    Name,
    Image,
    Full,
}

impl LabelDetail {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "name" => Ok(LabelDetail::Name),
            "" | "image" => Ok(LabelDetail::Image),
            "full" => Ok(LabelDetail::Full),
            other => Err(format!(
                "unknown labels '{other}' (use 'name', 'image', or 'full')"
            )),
        }
    }
}

/// What the tool returns.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Output {
    Mermaid,
    Markdown,
    Summary,
}

impl Output {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "mermaid" => Ok(Output::Mermaid),
            "markdown" | "md" => Ok(Output::Markdown),
            "summary" | "report" => Ok(Output::Summary),
            other => Err(format!(
                "unknown output '{other}' (use 'mermaid', 'markdown', or 'summary')"
            )),
        }
    }
}

/// Everything the renderer needs beyond the compose file itself.
#[derive(Clone, Copy, Debug)]
pub struct Options<'a> {
    pub direction: Direction,
    pub networks: NetworkStyle,
    pub ports: bool,
    pub volumes: bool,
    pub labels: LabelDetail,
    pub profile: &'a str,
    pub styled: bool,
    pub title: &'a str,
    pub output: Output,
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Parse `compose` (a docker-compose YAML document) and render the requested
/// diagram. String-typed wrapper so every surface (chat, CLI, page) can call it
/// with raw field values.
#[allow(clippy::too_many_arguments)]
pub fn compose_to_diagram(
    compose: &str,
    direction: &str,
    networks: &str,
    ports: bool,
    volumes: bool,
    labels: &str,
    profile: &str,
    styled: bool,
    title: &str,
    output: &str,
) -> Result<String, String> {
    let opts = Options {
        direction: Direction::parse(direction)?,
        networks: NetworkStyle::parse(networks)?,
        ports,
        volumes,
        labels: LabelDetail::parse(labels)?,
        profile: profile.trim(),
        styled,
        title: title.trim(),
        output: Output::parse(output)?,
    };
    render(compose, opts)
}

/// Typed entry point: parse + render with already-validated options.
pub fn render(compose: &str, opts: Options) -> Result<String, String> {
    let project = Project::parse(compose, opts.profile)?;
    Ok(match opts.output {
        Output::Mermaid => project.to_mermaid(opts),
        Output::Markdown => {
            let mut out = String::new();
            if !opts.title.is_empty() {
                out.push_str(&format!("# {}\n\n", opts.title));
            }
            out.push_str("```mermaid\n");
            out.push_str(&project.to_mermaid(opts));
            out.push_str("\n```\n");
            out
        }
        Output::Summary => project.to_summary(opts),
    })
}

// ---------------------------------------------------------------------------
// Compose model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct PortMap {
    host_ip: Option<String>,
    published: Option<String>,
    target: String,
    protocol: Option<String>,
}

impl PortMap {
    /// Human label, e.g. `8080:80`, `127.0.0.1:8080:80/udp`, `expose 5432`.
    fn label(&self) -> String {
        let proto = match self.protocol.as_deref() {
            Some(p) if !p.eq_ignore_ascii_case("tcp") => format!("/{p}"),
            _ => String::new(),
        };
        match (&self.host_ip, &self.published) {
            (Some(ip), Some(p)) => format!("{ip}:{p}:{}{proto}", self.target),
            (None, Some(p)) => format!("{p}:{}{proto}", self.target),
            _ => format!("expose {}{proto}", self.target),
        }
    }
}

#[derive(Debug, Clone)]
struct VolumeMount {
    /// Named volume name, or host path for a bind mount.
    source: String,
    /// Path inside the container (may be empty for an odd mapping).
    target: String,
    named: bool,
    read_only: bool,
}

#[derive(Debug, Clone)]
struct Service {
    name: String,
    image: Option<String>,
    build: Option<String>,
    restart: Option<String>,
    replicas: Option<String>,
    profiles: Vec<String>,
    networks: Vec<String>,
    network_mode: Option<String>,
    /// (target service, optional condition)
    depends_on: Vec<(String, Option<String>)>,
    /// (target service, optional alias)
    links: Vec<(String, Option<String>)>,
    /// Same-file `extends: {service: base}` only.
    extends: Option<String>,
    /// `extends` that points at another file — recorded for the summary.
    extends_external: Option<String>,
    ports: Vec<PortMap>,
    volumes: Vec<VolumeMount>,
}

#[derive(Debug)]
struct Project {
    services: Vec<Service>,
    /// Declared top-level networks, in file order → external?
    declared_networks: Vec<(String, bool)>,
    /// Declared top-level volumes, in file order → external?
    declared_volumes: Vec<(String, bool)>,
    warnings: Vec<String>,
}

impl Project {
    fn parse(compose: &str, profile: &str) -> Result<Self, String> {
        if compose.trim().is_empty() {
            return Err("no input: paste a docker-compose.yml to diagram".into());
        }
        if compose.len() > MAX_INPUT_BYTES {
            return Err(format!(
                "compose file is {} bytes, over the {MAX_INPUT_BYTES} byte limit",
                compose.len()
            ));
        }

        let root = Value::deserialize(serde_yml::Deserializer::from_str(compose))
            .map_err(|e| format!("invalid YAML: {e}"))?;
        let root = match root {
            Value::Mapping(m) => m,
            Value::Null => return Err("no input: the document is empty".into()),
            _ => return Err(
                "not a compose file: the document root must be a mapping with a 'services:' key"
                    .into(),
            ),
        };

        let services_val = get(&root, "services").ok_or_else(|| {
            "not a compose file: no top-level 'services:' key was found".to_string()
        })?;
        let services_map = match services_val {
            Value::Mapping(m) => m,
            _ => return Err("'services' must be a mapping of service name to definition".into()),
        };
        if services_map.is_empty() {
            return Err("'services' is empty: there is nothing to diagram".into());
        }
        if services_map.len() > MAX_SERVICES {
            return Err(format!(
                "{} services, over the {MAX_SERVICES} service limit",
                services_map.len()
            ));
        }

        let mut warnings = Vec::new();
        let mut all_services = Vec::new();
        for (k, v) in services_map.iter() {
            let name = match scalar_string(k) {
                Some(n) => n,
                None => continue,
            };
            let def = match v {
                Value::Mapping(m) => m.clone(),
                // `svc:` with an empty body is legal YAML but has nothing to show.
                Value::Null => Mapping::new(),
                _ => {
                    warnings.push(format!(
                        "service \"{name}\" is not a mapping and was skipped"
                    ));
                    continue;
                }
            };
            all_services.push(parse_service(&name, &def, &mut warnings));
        }

        // Profile filter: blank keeps everything; otherwise keep services with no
        // profiles (always active) plus those listing this profile.
        let services: Vec<Service> = if profile.is_empty() {
            all_services
        } else {
            let kept: Vec<Service> = all_services
                .iter()
                .filter(|s| s.profiles.is_empty() || s.profiles.iter().any(|p| p == profile))
                .cloned()
                .collect();
            if kept.is_empty() {
                return Err(format!(
                    "no services match profile '{profile}' (and no profile-less services exist)"
                ));
            }
            kept
        };

        let declared_networks = declared_section(&root, "networks");
        let declared_volumes = declared_section(&root, "volumes");

        let mut project = Project {
            services,
            declared_networks,
            declared_volumes,
            warnings,
        };
        project.validate();
        Ok(project)
    }

    fn names(&self) -> BTreeSet<&str> {
        self.services.iter().map(|s| s.name.as_str()).collect()
    }

    /// Collect structural warnings: dangling references, unused declarations,
    /// duplicate host ports and circular `depends_on` chains.
    fn validate(&mut self) {
        let known: BTreeSet<String> = self.services.iter().map(|s| s.name.clone()).collect();
        let mut warnings = Vec::new();

        for s in &self.services {
            for (dep, _) in &s.depends_on {
                if !known.contains(dep) {
                    warnings.push(format!(
                        "service \"{}\" depends_on \"{dep}\", which is not defined here",
                        s.name
                    ));
                }
            }
            for (target, _) in &s.links {
                if !known.contains(target) {
                    warnings.push(format!(
                        "service \"{}\" links to \"{target}\", which is not defined here",
                        s.name
                    ));
                }
            }
            if let Some(m) = &s.network_mode {
                if !known.contains(m) {
                    warnings.push(format!(
                        "service \"{}\" uses network_mode service:{m}, which is not defined here",
                        s.name
                    ));
                }
            }
            if s.image.is_none() && s.build.is_none() {
                warnings.push(format!(
                    "service \"{}\" declares neither image: nor build:",
                    s.name
                ));
            }
            let declared: BTreeSet<&str> = self
                .declared_networks
                .iter()
                .map(|(n, _)| n.as_str())
                .collect();
            for n in &s.networks {
                if !declared.is_empty() && !declared.contains(n.as_str()) {
                    warnings.push(format!(
                        "service \"{}\" joins network \"{n}\", which is not declared at the top level",
                        s.name
                    ));
                }
            }
        }

        // Host ports published more than once collide at runtime.
        let mut host_ports: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for s in &self.services {
            for p in &s.ports {
                if let Some(pub_) = &p.published {
                    let key = match &p.host_ip {
                        Some(ip) => format!("{ip}:{pub_}"),
                        None => pub_.clone(),
                    };
                    host_ports.entry(key).or_default().push(s.name.clone());
                }
            }
        }
        for (port, owners) in host_ports {
            if owners.len() > 1 {
                warnings.push(format!(
                    "host port {port} is published by {} services ({})",
                    owners.len(),
                    owners.join(", ")
                ));
            }
        }

        // Unused top-level declarations.
        let used_networks: BTreeSet<&str> = self
            .services
            .iter()
            .flat_map(|s| s.networks.iter().map(|n| n.as_str()))
            .collect();
        for (n, _) in &self.declared_networks {
            if !used_networks.contains(n.as_str()) {
                warnings.push(format!(
                    "network \"{n}\" is declared but no service joins it"
                ));
            }
        }
        let used_volumes: BTreeSet<&str> = self
            .services
            .iter()
            .flat_map(|s| {
                s.volumes
                    .iter()
                    .filter(|v| v.named)
                    .map(|v| v.source.as_str())
            })
            .collect();
        for (n, _) in &self.declared_volumes {
            if !used_volumes.contains(n.as_str()) {
                warnings.push(format!(
                    "volume \"{n}\" is declared but no service mounts it"
                ));
            }
        }

        for cycle in self.dependency_cycles() {
            warnings.push(format!("circular depends_on: {}", cycle.join(" -> ")));
        }

        self.warnings.extend(warnings);
    }

    /// Every distinct `depends_on` cycle, each rendered as a closed path.
    fn dependency_cycles(&self) -> Vec<Vec<String>> {
        let mut edges: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        let known = self.names();
        for s in &self.services {
            let outs: Vec<&str> = s
                .depends_on
                .iter()
                .map(|(d, _)| d.as_str())
                .filter(|d| known.contains(d))
                .collect();
            edges.insert(s.name.as_str(), outs);
        }

        let mut found: BTreeSet<Vec<String>> = BTreeSet::new();
        let mut stack: Vec<&str> = Vec::new();
        let mut on_stack: BTreeSet<&str> = BTreeSet::new();
        let mut done: BTreeSet<&str> = BTreeSet::new();

        // Iterative DFS keeps deep chains off the call stack.
        for s in &self.services {
            let start = s.name.as_str();
            if done.contains(start) {
                continue;
            }
            let mut work: Vec<(&str, usize)> = vec![(start, 0)];
            stack.push(start);
            on_stack.insert(start);
            while let Some((node, idx)) = work.pop() {
                let outs = edges.get(node).cloned().unwrap_or_default();
                if idx < outs.len() {
                    work.push((node, idx + 1));
                    let next = outs[idx];
                    if on_stack.contains(next) {
                        // Rotate the cycle so it starts at its smallest member,
                        // making the same loop report identically from any entry.
                        if let Some(pos) = stack.iter().position(|n| *n == next) {
                            let loop_nodes: Vec<&str> = stack[pos..].to_vec();
                            let min = loop_nodes
                                .iter()
                                .enumerate()
                                .min_by_key(|(_, n)| **n)
                                .map(|(i, _)| i)
                                .unwrap_or(0);
                            let mut path: Vec<String> = loop_nodes[min..]
                                .iter()
                                .chain(loop_nodes[..min].iter())
                                .map(|n| n.to_string())
                                .collect();
                            path.push(path[0].clone());
                            found.insert(path);
                        }
                    } else if !done.contains(next) {
                        work.push((next, 0));
                        stack.push(next);
                        on_stack.insert(next);
                    }
                } else {
                    on_stack.remove(node);
                    done.insert(node);
                    stack.pop();
                }
            }
        }
        found.into_iter().collect()
    }
}

/// Read one service definition into the model.
fn parse_service(name: &str, def: &Mapping, warnings: &mut Vec<String>) -> Service {
    let image = get(def, "image").and_then(scalar_string);
    let build = get(def, "build").and_then(|v| match v {
        Value::Mapping(m) => {
            let ctx = get(m, "context").and_then(scalar_string);
            let file = get(m, "dockerfile").and_then(scalar_string);
            match (ctx, file) {
                (Some(c), Some(f)) => Some(format!("{c} ({f})")),
                (Some(c), None) => Some(c),
                (None, Some(f)) => Some(f),
                (None, None) => Some(".".to_string()),
            }
        }
        other => scalar_string(other),
    });
    let restart = get(def, "restart").and_then(scalar_string);
    let replicas = get(def, "deploy")
        .and_then(|v| match v {
            Value::Mapping(m) => get(m, "replicas").cloned(),
            _ => None,
        })
        .and_then(|v| scalar_string(&v));

    let profiles = string_list(get(def, "profiles"));
    let networks = match get(def, "networks") {
        // `networks: {backend: {aliases: [...]}}` — only the keys matter here.
        Some(Value::Mapping(m)) => m.keys().filter_map(scalar_string).collect(),
        other => string_list(other),
    };

    let mut network_mode = None;
    if let Some(mode) = get(def, "network_mode").and_then(scalar_string) {
        if let Some(rest) = mode.strip_prefix("service:") {
            network_mode = Some(rest.trim().to_string());
        }
    }

    let depends_on = match get(def, "depends_on") {
        // Long form: `depends_on: {db: {condition: service_healthy}}`.
        Some(Value::Mapping(m)) => m
            .iter()
            .filter_map(|(k, v)| {
                let target = scalar_string(k)?;
                let cond = match v {
                    Value::Mapping(inner) => get(inner, "condition").and_then(scalar_string),
                    _ => None,
                };
                Some((target, cond))
            })
            .collect(),
        // Short form: `depends_on: [db, redis]`.
        other => string_list(other).into_iter().map(|d| (d, None)).collect(),
    };

    let links = string_list(get(def, "links"))
        .into_iter()
        .map(|l| match l.split_once(':') {
            Some((svc, alias)) => (svc.trim().to_string(), Some(alias.trim().to_string())),
            None => (l, None),
        })
        .collect();

    let (extends, extends_external) = match get(def, "extends") {
        Some(Value::Mapping(m)) => {
            let svc = get(m, "service").and_then(scalar_string);
            let file = get(m, "file").and_then(scalar_string);
            match (svc, file) {
                (Some(s), None) => (Some(s), None),
                (Some(s), Some(f)) => (None, Some(format!("{s} (in {f})"))),
                (None, _) => (None, None),
            }
        }
        // Short form `extends: base` refers to a service in the same file.
        Some(other) => (scalar_string(other), None),
        None => (None, None),
    };

    let ports = match get(def, "ports") {
        Some(Value::Sequence(seq)) => seq
            .iter()
            .filter_map(|p| parse_port(p, name, warnings))
            .collect(),
        _ => Vec::new(),
    };
    let volumes = match get(def, "volumes") {
        Some(Value::Sequence(seq)) => seq.iter().filter_map(parse_volume).collect(),
        _ => Vec::new(),
    };

    Service {
        name: name.to_string(),
        image,
        build,
        restart,
        replicas,
        profiles,
        networks,
        network_mode,
        depends_on,
        links,
        extends,
        extends_external,
        ports,
        volumes,
    }
}

/// Short (`"127.0.0.1:8080:80/udp"`, `8080`) and long (`{target, published}`)
/// port syntax.
fn parse_port(v: &Value, service: &str, warnings: &mut Vec<String>) -> Option<PortMap> {
    if let Value::Mapping(m) = v {
        let target = get(m, "target").and_then(scalar_string)?;
        return Some(PortMap {
            host_ip: get(m, "host_ip").and_then(scalar_string),
            published: get(m, "published").and_then(scalar_string),
            target,
            protocol: get(m, "protocol").and_then(scalar_string),
        });
    }
    let raw = scalar_string(v)?;
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let (body, protocol) = match raw.rsplit_once('/') {
        Some((b, p)) if !p.is_empty() && p.chars().all(|c| c.is_ascii_alphabetic()) => {
            (b, Some(p.to_string()))
        }
        _ => (raw, None),
    };
    let parts: Vec<&str> = body.split(':').collect();
    let mapped = match parts.len() {
        1 => PortMap {
            host_ip: None,
            published: None,
            target: parts[0].to_string(),
            protocol,
        },
        2 => PortMap {
            host_ip: None,
            published: Some(parts[0].to_string()),
            target: parts[1].to_string(),
            protocol,
        },
        3 => PortMap {
            host_ip: Some(parts[0].to_string()),
            published: Some(parts[1].to_string()),
            target: parts[2].to_string(),
            protocol,
        },
        _ => {
            warnings.push(format!(
                "service \"{service}\" has an unrecognized port mapping \"{raw}\""
            ));
            return None;
        }
    };
    Some(mapped)
}

/// Short (`"data:/var/lib:ro"`) and long (`{type, source, target}`) volume syntax.
/// Anonymous volumes (a bare container path) carry no structure and are dropped.
fn parse_volume(v: &Value) -> Option<VolumeMount> {
    if let Value::Mapping(m) = v {
        let source = get(m, "source").and_then(scalar_string)?;
        let target = get(m, "target").and_then(scalar_string).unwrap_or_default();
        let kind = get(m, "type").and_then(scalar_string).unwrap_or_default();
        let named = if kind.is_empty() {
            is_named_volume(&source)
        } else {
            kind == "volume"
        };
        let read_only = matches!(get(m, "read_only"), Some(Value::Bool(true)));
        return Some(VolumeMount {
            source,
            target,
            named,
            read_only,
        });
    }
    let raw = scalar_string(v)?;
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let parts: Vec<&str> = raw.split(':').collect();
    match parts.len() {
        // A bare container path is an anonymous volume — nothing to draw.
        1 => None,
        _ => {
            let source = parts[0].to_string();
            let target = parts[1].to_string();
            let read_only = parts.get(2).map(|m| m.contains("ro")).unwrap_or(false);
            let named = is_named_volume(&source);
            Some(VolumeMount {
                source,
                target,
                named,
                read_only,
            })
        }
    }
}

/// A mount source is a *named volume* when it is a plain name — anything with a
/// path separator or a leading `.`/`~`/`$` is a bind mount.
fn is_named_volume(source: &str) -> bool {
    !source.is_empty()
        && !source.contains('/')
        && !source.contains('\\')
        && !source.starts_with('.')
        && !source.starts_with('~')
        && !source.starts_with('$')
}

/// Read a top-level `networks:`/`volumes:` block into (name, external) pairs.
fn declared_section(root: &Mapping, key: &str) -> Vec<(String, bool)> {
    match get(root, key) {
        Some(Value::Mapping(m)) => m
            .iter()
            .filter_map(|(k, v)| {
                let name = scalar_string(k)?;
                let external = match v {
                    Value::Mapping(inner) => match get(inner, "external") {
                        Some(Value::Bool(b)) => *b,
                        Some(Value::Mapping(_)) => true,
                        _ => false,
                    },
                    _ => false,
                };
                Some((name, external))
            })
            .collect(),
        Some(Value::Sequence(seq)) => seq
            .iter()
            .filter_map(|v| scalar_string(v).map(|n| (n, false)))
            .collect(),
        _ => Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// YAML helpers
// ---------------------------------------------------------------------------

fn get<'a>(m: &'a Mapping, key: &str) -> Option<&'a Value> {
    m.get(Value::String(key.to_string()))
}

/// Scalars become strings; anything structural becomes `None`. Keeps `${VAR}`
/// placeholders and numeric ports usable as labels.
fn scalar_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// A scalar or a sequence of scalars, both flattened to a list of strings.
fn string_list(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::Sequence(seq)) => seq.iter().filter_map(scalar_string).collect(),
        Some(other) => scalar_string(other).into_iter().collect(),
        None => Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Mermaid rendering
// ---------------------------------------------------------------------------

/// Mermaid node ids must be identifier-safe; labels must not contain raw quotes.
fn sanitize_id(s: &str) -> String {
    let mut out: String = s
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    if out.is_empty() {
        out.push('_');
    }
    out
}

fn escape_label(s: &str) -> String {
    s.replace('"', "#quot;")
        .replace('<', "#lt;")
        .replace('>', "#gt;")
}

/// Hands out unique, stable mermaid ids per (prefix, key).
#[derive(Default)]
struct Ids {
    map: HashMap<String, String>,
    used: BTreeSet<String>,
}

impl Ids {
    fn id(&mut self, prefix: &str, key: &str) -> String {
        let map_key = format!("{prefix}\u{0}{key}");
        if let Some(existing) = self.map.get(&map_key) {
            return existing.clone();
        }
        let base = format!("{prefix}_{}", sanitize_id(key));
        let mut candidate = base.clone();
        let mut n = 2;
        while self.used.contains(&candidate) {
            candidate = format!("{base}_{n}");
            n += 1;
        }
        self.used.insert(candidate.clone());
        self.map.insert(map_key, candidate.clone());
        candidate
    }
}

impl Project {
    fn service_label(&self, s: &Service, detail: LabelDetail) -> String {
        let mut lines = vec![escape_label(&s.name)];
        if detail == LabelDetail::Name {
            return lines.join("<br/>");
        }
        match (&s.image, &s.build) {
            (Some(img), _) => lines.push(escape_label(img)),
            (None, Some(b)) => lines.push(format!("build: {}", escape_label(b))),
            (None, None) => {}
        }
        if detail == LabelDetail::Full {
            if let Some(r) = &s.replicas {
                lines.push(format!("x{} replicas", escape_label(r)));
            }
            if let Some(r) = &s.restart {
                lines.push(format!("restart: {}", escape_label(r)));
            }
            if !s.profiles.is_empty() {
                lines.push(format!(
                    "profiles: {}",
                    escape_label(&s.profiles.join(", "))
                ));
            }
        }
        lines.join("<br/>")
    }

    fn to_mermaid(&self, opts: Options) -> String {
        let mut ids = Ids::default();
        let mut out = String::new();

        if !opts.title.is_empty() {
            // A YAML front-matter title is rendered by mermaid itself.
            out.push_str(&format!("---\ntitle: {}\n---\n", opts.title));
        }
        out.push_str(&format!("flowchart {}\n", opts.direction.keyword()));

        let known: BTreeSet<String> = self.services.iter().map(|s| s.name.clone()).collect();
        let external_networks: BTreeMap<&str, bool> = self
            .declared_networks
            .iter()
            .map(|(n, e)| (n.as_str(), *e))
            .collect();

        // Reserve service ids up-front so edges never invent a new node id.
        for s in &self.services {
            ids.id("svc", &s.name);
        }

        // --- service nodes, grouped by their first network when asked ---------
        let mut service_lines: BTreeMap<String, Vec<String>> = BTreeMap::new(); // network -> lines
        let mut ungrouped: Vec<String> = Vec::new();
        let group_by_network = opts.networks == NetworkStyle::Subgraph;
        let mut service_ids: Vec<String> = Vec::new();

        for s in &self.services {
            let id = ids.id("svc", &s.name);
            service_ids.push(id.clone());
            let line = format!("    {id}[\"{}\"]", self.service_label(s, opts.labels));
            match (group_by_network, s.networks.first()) {
                (true, Some(net)) => service_lines.entry(net.clone()).or_default().push(line),
                _ => ungrouped.push(line),
            }
        }

        // Network ids are shared between subgraph headers and plain nodes.
        let mut network_order: Vec<String> = Vec::new();
        for (n, _) in &self.declared_networks {
            if !network_order.contains(n) {
                network_order.push(n.clone());
            }
        }
        for s in &self.services {
            for n in &s.networks {
                if !network_order.contains(n) {
                    network_order.push(n.clone());
                }
            }
        }

        let mut network_ids: BTreeMap<String, String> = BTreeMap::new();
        let mut network_node_ids: Vec<String> = Vec::new();
        if opts.networks != NetworkStyle::Off {
            for n in &network_order {
                network_ids.insert(n.clone(), ids.id("net", n));
            }
        }

        let net_label = |n: &str| -> String {
            let mut label = escape_label(n);
            if external_networks.get(n).copied().unwrap_or(false) {
                label.push_str(" (external)");
            }
            label
        };

        if group_by_network {
            for n in &network_order {
                if let Some(lines) = service_lines.get(n) {
                    let nid = &network_ids[n];
                    out.push_str(&format!("  subgraph {nid}[\"net: {}\"]\n", net_label(n)));
                    for l in lines {
                        out.push_str(l);
                        out.push('\n');
                    }
                    out.push_str("  end\n");
                }
            }
        }
        // Services that no subgraph claimed sit at the top level.
        for l in &ungrouped {
            out.push_str(&format!("  {}\n", l.trim_start()));
        }

        // Standalone network nodes: `node` mode draws them all; `subgraph` mode
        // draws only those that never became a subgraph (secondary-only members).
        if opts.networks != NetworkStyle::Off {
            for n in &network_order {
                let has_subgraph = group_by_network && service_lines.contains_key(n);
                let joined_by_any = self.services.iter().any(|s| s.networks.contains(n));
                if has_subgraph || !joined_by_any {
                    continue;
                }
                let nid = &network_ids[n];
                out.push_str(&format!("  {nid}{{{{\"net: {}\"}}}}\n", net_label(n)));
                network_node_ids.push(nid.clone());
            }
        }

        // --- edges ------------------------------------------------------------
        let mut missing_ids: Vec<String> = Vec::new();
        let mut edges = String::new();
        for s in &self.services {
            let from = ids.id("svc", &s.name);
            for (dep, cond) in &s.depends_on {
                let to = if known.contains(dep) {
                    ids.id("svc", dep)
                } else {
                    let mid = ids.id("svc", dep);
                    if !missing_ids.contains(&mid) {
                        edges.push_str(&format!(
                            "  {mid}[\"{}<br/>(not defined)\"]\n",
                            escape_label(dep)
                        ));
                        missing_ids.push(mid.clone());
                    }
                    mid
                };
                match cond.as_deref() {
                    Some(c) if !c.is_empty() && c != "service_started" => {
                        let short = c.strip_prefix("service_").unwrap_or(c);
                        edges
                            .push_str(&format!("  {from} -->|\"{}\"| {to}\n", escape_label(short)));
                    }
                    _ => edges.push_str(&format!("  {from} --> {to}\n")),
                }
            }
            for (target, alias) in &s.links {
                let to = if known.contains(target) {
                    ids.id("svc", target)
                } else {
                    let mid = ids.id("svc", target);
                    if !missing_ids.contains(&mid) {
                        edges.push_str(&format!(
                            "  {mid}[\"{}<br/>(not defined)\"]\n",
                            escape_label(target)
                        ));
                        missing_ids.push(mid.clone());
                    }
                    mid
                };
                let label = match alias {
                    Some(a) => format!("link: {}", escape_label(a)),
                    None => "link".to_string(),
                };
                edges.push_str(&format!("  {from} -->|\"{label}\"| {to}\n"));
            }
            if let Some(m) = &s.network_mode {
                if known.contains(m) {
                    let to = ids.id("svc", m);
                    edges.push_str(&format!("  {from} -->|\"network_mode\"| {to}\n"));
                }
            }
            if let Some(base) = &s.extends {
                if known.contains(base) {
                    let to = ids.id("svc", base);
                    edges.push_str(&format!("  {from} -.->|\"extends\"| {to}\n"));
                }
            }
        }

        // Secondary network membership (the first network already grouped it).
        if opts.networks != NetworkStyle::Off {
            for s in &self.services {
                let from = ids.id("svc", &s.name);
                let skip_first = group_by_network && !s.networks.is_empty();
                for (i, n) in s.networks.iter().enumerate() {
                    if skip_first && i == 0 {
                        continue;
                    }
                    if let Some(nid) = network_ids.get(n) {
                        edges.push_str(&format!("  {from} -.-> {nid}\n"));
                    }
                }
            }
        }

        // --- ports + volumes --------------------------------------------------
        let mut port_ids: Vec<String> = Vec::new();
        if opts.ports {
            for s in &self.services {
                let to = ids.id("svc", &s.name);
                for (i, p) in s.ports.iter().enumerate() {
                    let pid = ids.id("port", &format!("{}_{i}", s.name));
                    edges.push_str(&format!(
                        "  {pid}([\"{}\"]) --> {to}\n",
                        escape_label(&p.label())
                    ));
                    port_ids.push(pid);
                }
            }
        }

        let external_volumes: BTreeMap<&str, bool> = self
            .declared_volumes
            .iter()
            .map(|(n, e)| (n.as_str(), *e))
            .collect();
        let mut volume_ids: Vec<String> = Vec::new();
        let mut bind_ids: Vec<String> = Vec::new();
        if opts.volumes {
            let mut declared_nodes: BTreeSet<String> = BTreeSet::new();
            for s in &self.services {
                let from = ids.id("svc", &s.name);
                for v in &s.volumes {
                    let prefix = if v.named { "vol" } else { "bind" };
                    let vid = ids.id(prefix, &v.source);
                    if declared_nodes.insert(vid.clone()) {
                        let mut label = escape_label(&v.source);
                        if v.named && external_volumes.get(v.source.as_str()).copied() == Some(true)
                        {
                            label.push_str(" (external)");
                        }
                        if v.named {
                            edges.push_str(&format!("  {vid}[(\"{label}\")]\n"));
                            volume_ids.push(vid.clone());
                        } else {
                            edges.push_str(&format!("  {vid}[/\"{label}\"/]\n"));
                            bind_ids.push(vid.clone());
                        }
                    }
                    let mut mount = escape_label(&v.target);
                    if v.read_only {
                        mount.push_str(" (ro)");
                    }
                    if mount.is_empty() {
                        edges.push_str(&format!("  {from} -.-> {vid}\n"));
                    } else {
                        edges.push_str(&format!("  {from} -.->|\"{mount}\"| {vid}\n"));
                    }
                }
            }
        }

        out.push_str(&edges);

        // --- warnings + styling ----------------------------------------------
        for w in &self.warnings {
            out.push_str(&format!("  %% warning: {w}\n"));
        }

        if opts.styled {
            out.push_str("  classDef service fill:#dbeafe,stroke:#2563eb,color:#1e3a5f;\n");
            out.push_str("  classDef port fill:#dcfce7,stroke:#16a34a,color:#14532d;\n");
            out.push_str("  classDef volume fill:#fef3c7,stroke:#d97706,color:#78350f;\n");
            out.push_str("  classDef bind fill:#fae8ff,stroke:#a21caf,color:#4a044e;\n");
            out.push_str("  classDef network fill:#ede9fe,stroke:#7c3aed,color:#3b0764;\n");
            out.push_str("  classDef missing fill:#fee2e2,stroke:#dc2626,color:#7f1d1d,stroke-dasharray: 4 3;\n");
            let class_line = |members: &[String], class: &str, out: &mut String| {
                if !members.is_empty() {
                    out.push_str(&format!("  class {} {class};\n", members.join(","),));
                }
            };
            class_line(&service_ids, "service", &mut out);
            class_line(&port_ids, "port", &mut out);
            class_line(&volume_ids, "volume", &mut out);
            class_line(&bind_ids, "bind", &mut out);
            class_line(&network_node_ids, "network", &mut out);
            class_line(&missing_ids, "missing", &mut out);
        }

        out.trim_end().to_string()
    }

    // -----------------------------------------------------------------------
    // Text summary
    // -----------------------------------------------------------------------

    fn to_summary(&self, opts: Options) -> String {
        let mut out = String::new();
        let heading = if opts.title.is_empty() {
            "Compose summary".to_string()
        } else {
            format!("Compose summary — {}", opts.title)
        };
        out.push_str(&format!(
            "{heading}\n{}\n\n",
            "=".repeat(heading.chars().count())
        ));

        out.push_str(&format!("Services ({})\n", self.services.len()));
        for s in &self.services {
            out.push_str(&format!("  {}\n", s.name));
            if let Some(i) = &s.image {
                out.push_str(&format!("    image:      {i}\n"));
            }
            if let Some(b) = &s.build {
                out.push_str(&format!("    build:      {b}\n"));
            }
            if !s.depends_on.is_empty() {
                let deps: Vec<String> = s
                    .depends_on
                    .iter()
                    .map(|(d, c)| match c {
                        Some(c) if !c.is_empty() => format!("{d} ({c})"),
                        _ => d.clone(),
                    })
                    .collect();
                out.push_str(&format!("    depends_on: {}\n", deps.join(", ")));
            }
            if !s.links.is_empty() {
                let links: Vec<String> = s
                    .links
                    .iter()
                    .map(|(t, a)| match a {
                        Some(a) => format!("{t} as {a}"),
                        None => t.clone(),
                    })
                    .collect();
                out.push_str(&format!("    links:      {}\n", links.join(", ")));
            }
            if let Some(m) = &s.network_mode {
                out.push_str(&format!("    net mode:   service:{m}\n"));
            }
            if let Some(e) = &s.extends {
                out.push_str(&format!("    extends:    {e}\n"));
            }
            if let Some(e) = &s.extends_external {
                out.push_str(&format!("    extends:    {e}\n"));
            }
            if !s.networks.is_empty() {
                out.push_str(&format!("    networks:   {}\n", s.networks.join(", ")));
            }
            if !s.ports.is_empty() {
                let ports: Vec<String> = s.ports.iter().map(|p| p.label()).collect();
                out.push_str(&format!("    ports:      {}\n", ports.join(", ")));
            }
            if !s.volumes.is_empty() {
                let vols: Vec<String> = s
                    .volumes
                    .iter()
                    .map(|v| {
                        let kind = if v.named { "volume" } else { "bind" };
                        let ro = if v.read_only { ", ro" } else { "" };
                        format!("{} -> {} ({kind}{ro})", v.source, v.target)
                    })
                    .collect();
                out.push_str(&format!("    volumes:    {}\n", vols.join(", ")));
            }
            if let Some(r) = &s.replicas {
                out.push_str(&format!("    replicas:   {r}\n"));
            }
            if let Some(r) = &s.restart {
                out.push_str(&format!("    restart:    {r}\n"));
            }
            if !s.profiles.is_empty() {
                out.push_str(&format!("    profiles:   {}\n", s.profiles.join(", ")));
            }
        }

        out.push_str(&format!("\nNetworks ({})\n", self.declared_networks.len()));
        if self.declared_networks.is_empty() {
            out.push_str("  (none declared — services share the implicit default network)\n");
        }
        for (n, external) in &self.declared_networks {
            let members: Vec<&str> = self
                .services
                .iter()
                .filter(|s| s.networks.contains(n))
                .map(|s| s.name.as_str())
                .collect();
            let ext = if *external { " (external)" } else { "" };
            out.push_str(&format!("  {n}{ext}: {}\n", members.join(", ")));
        }

        out.push_str(&format!("\nVolumes ({})\n", self.declared_volumes.len()));
        if self.declared_volumes.is_empty() {
            out.push_str("  (none declared)\n");
        }
        for (v, external) in &self.declared_volumes {
            let users: Vec<&str> = self
                .services
                .iter()
                .filter(|s| s.volumes.iter().any(|m| m.named && &m.source == v))
                .map(|s| s.name.as_str())
                .collect();
            let ext = if *external { " (external)" } else { "" };
            out.push_str(&format!("  {v}{ext}: {}\n", users.join(", ")));
        }

        out.push_str(&format!("\nWarnings ({})\n", self.warnings.len()));
        if self.warnings.is_empty() {
            out.push_str("  (none)\n");
        }
        for w in &self.warnings {
            out.push_str(&format!("  - {w}\n"));
        }

        out.trim_end().to_string()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
services:
  web:
    image: nginx:1.27
    ports:
      - "8080:80"
    depends_on:
      - api
    networks: [frontend]
  api:
    build: ./api
    depends_on:
      db:
        condition: service_healthy
    networks: [frontend, backend]
  db:
    image: postgres:16
    volumes:
      - pgdata:/var/lib/postgresql/data
    networks: [backend]
volumes:
  pgdata:
networks:
  frontend:
  backend:
"#;

    fn defaults<'a>() -> Options<'a> {
        Options {
            direction: Direction::Td,
            networks: NetworkStyle::Subgraph,
            ports: true,
            volumes: true,
            labels: LabelDetail::Image,
            profile: "",
            styled: true,
            title: "",
            output: Output::Mermaid,
        }
    }

    #[test]
    fn happy_path_renders_services_and_dependencies() {
        let out = render(SAMPLE, defaults()).unwrap();
        assert!(out.starts_with("flowchart TD\n"), "{out}");
        assert!(out.contains("svc_web[\"web<br/>nginx:1.27\"]"), "{out}");
        assert!(out.contains("svc_api[\"api<br/>build: ./api\"]"), "{out}");
        assert!(out.contains("svc_web --> svc_api"), "{out}");
        // depends_on condition becomes an edge label.
        assert!(out.contains("svc_api -->|\"healthy\"| svc_db"), "{out}");
        // networks become subgraphs, keyed off each service's first network.
        assert!(
            out.contains("subgraph net_frontend[\"net: frontend\"]"),
            "{out}"
        );
        assert!(
            out.contains("subgraph net_backend[\"net: backend\"]"),
            "{out}"
        );
        // api's second network is a dotted membership edge.
        assert!(out.contains("svc_api -.-> net_backend"), "{out}");
        // ports + named volumes get their own nodes.
        assert!(
            out.contains("port_web_0([\"8080:80\"]) --> svc_web"),
            "{out}"
        );
        assert!(out.contains("vol_pgdata[(\"pgdata\")]"), "{out}");
        assert!(
            out.contains("svc_db -.->|\"/var/lib/postgresql/data\"| vol_pgdata"),
            "{out}"
        );
        assert!(out.contains("classDef service"), "{out}");
    }

    #[test]
    fn depends_on_list_and_map_forms_agree() {
        let list = "services:\n  a:\n    image: x\n    depends_on: [b]\n  b:\n    image: y\n";
        let map = "services:\n  a:\n    image: x\n    depends_on:\n      b: {condition: service_started}\n  b:\n    image: y\n";
        let a = render(list, defaults()).unwrap();
        let b = render(map, defaults()).unwrap();
        assert!(a.contains("svc_a --> svc_b"), "{a}");
        // service_started is the default condition — no edge label for it.
        assert_eq!(a, b);
    }

    #[test]
    fn exact_minimal_output() {
        let mut o = defaults();
        o.styled = false;
        o.networks = NetworkStyle::Off;
        let out = render(
            "services:\n  api:\n    image: node:20\n    depends_on: [db]\n  db:\n    image: postgres:16\n",
            o,
        )
        .unwrap();
        assert_eq!(
            out,
            "flowchart TD\n  svc_api[\"api<br/>node:20\"]\n  svc_db[\"db<br/>postgres:16\"]\n  svc_api --> svc_db"
        );
    }

    #[test]
    fn direction_and_label_detail() {
        let mut o = defaults();
        o.direction = Direction::Lr;
        o.labels = LabelDetail::Name;
        let out = render(SAMPLE, o).unwrap();
        assert!(out.starts_with("flowchart LR\n"), "{out}");
        assert!(out.contains("svc_web[\"web\"]"), "{out}");
        assert!(!out.contains("nginx:1.27"), "{out}");

        let mut o = defaults();
        o.labels = LabelDetail::Full;
        let src = "services:\n  w:\n    image: nginx\n    restart: always\n    deploy:\n      replicas: 3\n";
        let out = render(src, o).unwrap();
        assert!(out.contains("x3 replicas"), "{out}");
        assert!(out.contains("restart: always"), "{out}");
    }

    #[test]
    fn network_node_style_and_off() {
        let mut o = defaults();
        o.networks = NetworkStyle::Node;
        let out = render(SAMPLE, o).unwrap();
        assert!(!out.contains("subgraph"), "{out}");
        assert!(out.contains("net_frontend{{\"net: frontend\"}}"), "{out}");
        assert!(out.contains("svc_web -.-> net_frontend"), "{out}");
        assert!(out.contains("svc_api -.-> net_frontend"), "{out}");
        assert!(out.contains("svc_api -.-> net_backend"), "{out}");

        let mut o = defaults();
        o.networks = NetworkStyle::Off;
        let out = render(SAMPLE, o).unwrap();
        assert!(!out.contains("net_frontend"), "{out}");
    }

    #[test]
    fn ports_and_volumes_can_be_hidden() {
        let mut o = defaults();
        o.ports = false;
        o.volumes = false;
        let out = render(SAMPLE, o).unwrap();
        assert!(!out.contains("8080:80"), "{out}");
        assert!(!out.contains("pgdata"), "{out}");
    }

    #[test]
    fn long_port_and_volume_syntax() {
        let src = r#"
services:
  app:
    image: app
    ports:
      - target: 80
        published: "8080"
        protocol: udp
        host_ip: 127.0.0.1
      - "5432"
    volumes:
      - type: bind
        source: ./src
        target: /app
        read_only: true
"#;
        let out = render(src, defaults()).unwrap();
        assert!(out.contains("127.0.0.1:8080:80/udp"), "{out}");
        assert!(out.contains("expose 5432"), "{out}");
        assert!(out.contains("bind___src[/\"./src\"/]"), "{out}");
        assert!(
            out.contains("svc_app -.->|\"/app (ro)\"| bind___src"),
            "{out}"
        );
    }

    #[test]
    fn links_network_mode_and_extends_edges() {
        let src = r#"
services:
  base:
    image: alpine
  sidecar:
    image: alpine
    network_mode: "service:base"
    extends:
      service: base
  app:
    image: alpine
    links:
      - base:database
"#;
        let out = render(src, defaults()).unwrap();
        assert!(
            out.contains("svc_sidecar -->|\"network_mode\"| svc_base"),
            "{out}"
        );
        assert!(
            out.contains("svc_sidecar -.->|\"extends\"| svc_base"),
            "{out}"
        );
        assert!(
            out.contains("svc_app -->|\"link: database\"| svc_base"),
            "{out}"
        );
    }

    #[test]
    fn missing_dependency_becomes_a_flagged_node() {
        let src = "services:\n  a:\n    image: x\n    depends_on: [ghost]\n";
        let out = render(src, defaults()).unwrap();
        assert!(
            out.contains("svc_ghost[\"ghost<br/>(not defined)\"]"),
            "{out}"
        );
        assert!(out.contains("class svc_ghost missing;"), "{out}");
        assert!(
            out.contains(
                "%% warning: service \"a\" depends_on \"ghost\", which is not defined here"
            ),
            "{out}"
        );
    }

    #[test]
    fn circular_dependencies_are_reported_once() {
        let src = "services:\n  a:\n    image: x\n    depends_on: [b]\n  b:\n    image: y\n    depends_on: [a]\n";
        let out = render(src, defaults()).unwrap();
        assert_eq!(
            out.matches("circular depends_on").count(),
            1,
            "one cycle reported once: {out}"
        );
        assert!(out.contains("circular depends_on: a -> b -> a"), "{out}");
    }

    #[test]
    fn duplicate_host_ports_warn() {
        let src = "services:\n  a:\n    image: x\n    ports: [\"80:80\"]\n  b:\n    image: y\n    ports: [\"80:8080\"]\n";
        let out = render(src, defaults()).unwrap();
        assert!(
            out.contains("host port 80 is published by 2 services (a, b)"),
            "{out}"
        );
    }

    #[test]
    fn external_network_and_volume_labels() {
        let src = r#"
services:
  a:
    image: x
    networks: [shared]
    volumes:
      - assets:/data
networks:
  shared:
    external: true
volumes:
  assets:
    external: true
"#;
        let out = render(src, defaults()).unwrap();
        assert!(out.contains("net: shared (external)"), "{out}");
        assert!(out.contains("vol_assets[(\"assets (external)\")]"), "{out}");
    }

    #[test]
    fn profile_filter_selects_services() {
        let src = r#"
services:
  core:
    image: x
  debugger:
    image: y
    profiles: [debug]
"#;
        let mut o = defaults();
        o.profile = "debug";
        let out = render(src, o).unwrap();
        assert!(out.contains("svc_debugger"), "{out}");
        assert!(
            out.contains("svc_core"),
            "profile-less services stay active: {out}"
        );

        let mut o = defaults();
        o.profile = "";
        let out = render(src, o).unwrap();
        assert!(
            out.contains("svc_debugger"),
            "blank profile shows everything: {out}"
        );
    }

    #[test]
    fn networks_mapping_form_with_aliases() {
        let src =
            "services:\n  a:\n    image: x\n    networks:\n      backend:\n        aliases: [db]\n";
        let out = render(src, defaults()).unwrap();
        assert!(
            out.contains("subgraph net_backend[\"net: backend\"]"),
            "{out}"
        );
    }

    #[test]
    fn markdown_and_summary_outputs() {
        let mut o = defaults();
        o.output = Output::Markdown;
        o.title = "My stack";
        let out = render(SAMPLE, o).unwrap();
        assert!(out.starts_with("# My stack\n\n```mermaid\n"), "{out}");
        assert!(out.trim_end().ends_with("```"), "{out}");

        let mut o = defaults();
        o.output = Output::Summary;
        let out = render(SAMPLE, o).unwrap();
        assert!(out.starts_with("Compose summary\n"), "{out}");
        assert!(out.contains("Services (3)"), "{out}");
        assert!(out.contains("    image:      nginx:1.27"), "{out}");
        assert!(out.contains("depends_on: db (service_healthy)"), "{out}");
        assert!(out.contains("frontend: web, api"), "{out}");
        assert!(out.contains("pgdata: db"), "{out}");
    }

    #[test]
    fn title_becomes_mermaid_front_matter() {
        let mut o = defaults();
        o.title = "Stack";
        let out = render(SAMPLE, o).unwrap();
        assert!(
            out.starts_with("---\ntitle: Stack\n---\nflowchart TD\n"),
            "{out}"
        );
    }

    #[test]
    fn labels_with_quotes_are_escaped() {
        let src = "services:\n  a:\n    image: 'my\"img<x>'\n";
        let out = render(src, defaults()).unwrap();
        assert!(out.contains("my#quot;img#lt;x#gt;"), "{out}");
        assert!(!out.contains("my\"img"), "{out}");
    }

    #[test]
    fn service_names_with_dots_get_distinct_ids() {
        let src = "services:\n  a.b:\n    image: x\n  a-b:\n    image: y\n";
        let out = render(src, defaults()).unwrap();
        assert!(out.contains("svc_a_b[\"a.b"), "{out}");
        assert!(out.contains("svc_a_b_2[\"a-b"), "{out}");
    }

    // --- error paths --------------------------------------------------------

    #[test]
    fn empty_input_errors() {
        let err = render("   \n", defaults()).unwrap_err();
        assert!(err.contains("no input"), "{err}");
    }

    #[test]
    fn invalid_yaml_errors() {
        let err = render("services:\n  a: [unclosed\n", defaults()).unwrap_err();
        assert!(err.starts_with("invalid YAML:"), "{err}");
    }

    #[test]
    fn missing_services_key_errors() {
        let err = render("version: \"3\"\nnetworks:\n  a:\n", defaults()).unwrap_err();
        assert!(err.contains("no top-level 'services:' key"), "{err}");
    }

    #[test]
    fn empty_services_errors() {
        let err = render("services: {}\n", defaults()).unwrap_err();
        assert!(err.contains("'services' is empty"), "{err}");
    }

    #[test]
    fn services_not_a_mapping_errors() {
        let err = render("services:\n  - a\n  - b\n", defaults()).unwrap_err();
        assert!(err.contains("'services' must be a mapping"), "{err}");
    }

    #[test]
    fn scalar_root_errors() {
        let err = render("just a string\n", defaults()).unwrap_err();
        assert!(err.contains("not a compose file"), "{err}");
    }

    #[test]
    fn unknown_profile_errors_when_nothing_matches() {
        let src = "services:\n  a:\n    image: x\n    profiles: [debug]\n";
        let mut o = defaults();
        o.profile = "prod";
        let err = render(src, o).unwrap_err();
        assert!(err.contains("no services match profile 'prod'"), "{err}");
    }

    #[test]
    fn unknown_enum_values_error() {
        let ok = "services:\n  a:\n    image: x\n";
        let e = compose_to_diagram(
            ok, "sideways", "subgraph", true, true, "image", "", true, "", "mermaid",
        )
        .unwrap_err();
        assert!(e.contains("unknown direction"), "{e}");
        let e = compose_to_diagram(
            ok, "TD", "cloud", true, true, "image", "", true, "", "mermaid",
        )
        .unwrap_err();
        assert!(e.contains("unknown networks"), "{e}");
        let e = compose_to_diagram(
            ok, "TD", "subgraph", true, true, "verbose", "", true, "", "mermaid",
        )
        .unwrap_err();
        assert!(e.contains("unknown labels"), "{e}");
        let e = compose_to_diagram(
            ok, "TD", "subgraph", true, true, "image", "", true, "", "svg",
        )
        .unwrap_err();
        assert!(e.contains("unknown output"), "{e}");
    }

    #[test]
    fn oversized_input_errors() {
        let mut src = String::from("services:\n  a:\n    image: x\n");
        src.push_str(&"# pad\n".repeat(MAX_INPUT_BYTES / 6));
        let err = render(&src, defaults()).unwrap_err();
        assert!(err.contains("byte limit"), "{err}");
    }

    #[test]
    fn string_wrapper_matches_typed_render() {
        let a = compose_to_diagram(
            SAMPLE, "TD", "subgraph", true, true, "image", "", true, "", "mermaid",
        )
        .unwrap();
        let b = render(SAMPLE, defaults()).unwrap();
        assert_eq!(a, b);
    }
}

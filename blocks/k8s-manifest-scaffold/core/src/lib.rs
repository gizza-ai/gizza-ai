//! k8s-manifest-scaffold core — build a Kubernetes Deployment + Service YAML
//! manifest from a small set of app settings. Pure compute, no deps: input
//! validation (RFC 1123 names, port ranges, resource quantities, env keys,
//! label syntax, probe path), a YAML string-scalar emitter that quotes anything
//! ambiguous, and a fixed key order so the same inputs always produce the same
//! bytes. Shared by the chat skill block and the web page.

/// How the Service is exposed.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ServiceType {
    ClusterIp,
    NodePort,
    LoadBalancer,
}

impl ServiceType {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "ClusterIP" | "clusterip" | "cluster-ip" => Ok(ServiceType::ClusterIp),
            "NodePort" | "nodeport" | "node-port" => Ok(ServiceType::NodePort),
            "LoadBalancer" | "loadbalancer" | "load-balancer" => Ok(ServiceType::LoadBalancer),
            other => Err(format!(
                "invalid service_type {other:?}: expected \"ClusterIP\", \"NodePort\" or \"LoadBalancer\""
            )),
        }
    }
    fn as_str(self) -> &'static str {
        match self {
            ServiceType::ClusterIp => "ClusterIP",
            ServiceType::NodePort => "NodePort",
            ServiceType::LoadBalancer => "LoadBalancer",
        }
    }
}

/// When the kubelet pulls the image.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PullPolicy {
    IfNotPresent,
    Always,
    Never,
}

impl PullPolicy {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "IfNotPresent" | "ifnotpresent" | "if-not-present" => Ok(PullPolicy::IfNotPresent),
            "Always" | "always" => Ok(PullPolicy::Always),
            "Never" | "never" => Ok(PullPolicy::Never),
            other => Err(format!(
                "invalid image_pull_policy {other:?}: expected \"IfNotPresent\", \"Always\" or \"Never\""
            )),
        }
    }
    fn as_str(self) -> &'static str {
        match self {
            PullPolicy::IfNotPresent => "IfNotPresent",
            PullPolicy::Always => "Always",
            PullPolicy::Never => "Never",
        }
    }
}

/// Which quantity syntax a resource field accepts — CPU allows the `m`
/// (millicore) suffix, memory allows the binary/decimal byte suffixes.
#[derive(Clone, Copy)]
enum Resource {
    Cpu,
    Memory,
}

impl Resource {
    /// Suffixes Kubernetes accepts for this resource, longest-first so `Mi`
    /// wins over `M` when both would match.
    fn suffixes(self) -> &'static [&'static str] {
        match self {
            Resource::Cpu => &["m", ""],
            Resource::Memory => &[
                "Ki", "Mi", "Gi", "Ti", "Pi", "Ei", "k", "M", "G", "T", "P", "E", "",
            ],
        }
    }
    fn example(self) -> &'static str {
        match self {
            Resource::Cpu => "100m, 0.5 or 2",
            Resource::Memory => "128Mi, 512M or 1Gi",
        }
    }
}

/// The largest replica count the tool accepts. Mirrors the schema's `maximum`
/// so the page slider and the chat schema agree with the validator.
const MAX_REPLICAS: i64 = 100;

/// Build a Kubernetes Deployment + Service manifest (two YAML documents
/// separated by `---`).
///
/// - `name`: `metadata.name` for both resources and the value of the `app`
///   selector label (RFC 1123).
/// - `image`: container image reference, e.g. `nginx:1.27`.
/// - `namespace`: `metadata.namespace`; omitted when blank.
/// - `replicas`: Deployment replica count (0–[`MAX_REPLICAS`]).
/// - `container_port` / `service_port`: `containerPort` and the Service `port`
///   (whose `targetPort` is the container port).
/// - `service_type`: `ClusterIP` (default), `NodePort` or `LoadBalancer`.
/// - `node_port`: fixed `nodePort` (30000–32767); NodePort Services only.
/// - `image_pull_policy`: `IfNotPresent` (default), `Always` or `Never`.
/// - `cpu_request` / `cpu_limit` / `memory_request` / `memory_limit`: resource
///   quantities; each omitted when blank, and the whole `resources:` block is
///   omitted when all four are blank.
/// - `env`: container environment as `KEY=value` lines.
/// - `labels`: extra `key=value` labels (newline- or comma-separated) merged
///   after the standard `app` label.
/// - `probe_path`: HTTP path for a liveness *and* readiness probe; blank emits
///   neither.
///
/// Returns `Err` with an actionable message for any invalid input.
#[allow(clippy::too_many_arguments)]
pub fn scaffold(
    name: &str,
    image: &str,
    namespace: &str,
    replicas: i64,
    container_port: i64,
    service_port: i64,
    service_type: &str,
    node_port: Option<i64>,
    image_pull_policy: &str,
    cpu_request: &str,
    cpu_limit: &str,
    memory_request: &str,
    memory_limit: &str,
    env: &str,
    labels: &str,
    probe_path: &str,
) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("name is required: the Deployment/Service name, e.g. \"web\"".to_string());
    }
    if !is_rfc1123_name(name) {
        return Err(format!(
            "invalid name {name:?}: use lowercase letters, digits and '-', starting and ending with a letter or digit, at most 63 characters (RFC 1123)"
        ));
    }

    let image = image.trim();
    if image.is_empty() {
        return Err(
            "image is required: a container image reference, e.g. \"nginx:1.27\"".to_string(),
        );
    }
    if image.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(format!(
            "invalid image {image:?}: an image reference cannot contain spaces or control characters, e.g. \"ghcr.io/acme/api:v1.2.3\""
        ));
    }

    let namespace = namespace.trim();
    if !namespace.is_empty() && !is_rfc1123_name(namespace) {
        return Err(format!(
            "invalid namespace {namespace:?}: use lowercase letters, digits and '-', starting and ending with a letter or digit, at most 63 characters (RFC 1123)"
        ));
    }

    if !(0..=MAX_REPLICAS).contains(&replicas) {
        return Err(format!(
            "invalid replicas {replicas}: use a whole number from 0 to {MAX_REPLICAS} (0 scales the Deployment down to no pods)"
        ));
    }
    let container_port = check_port("container_port", container_port)?;
    let service_port = check_port("service_port", service_port)?;

    let service_type = ServiceType::parse(service_type)?;
    let pull_policy = PullPolicy::parse(image_pull_policy)?;

    let node_port = match node_port {
        None => None,
        Some(np) => {
            if service_type != ServiceType::NodePort {
                return Err(format!(
                    "node_port {np} is only valid when service_type is \"NodePort\" (got {:?}): clear node_port or switch the service type",
                    service_type.as_str()
                ));
            }
            if !(30000..=32767).contains(&np) {
                return Err(format!(
                    "invalid node_port {np}: a nodePort must be 30000-32767 (the cluster's default range); leave it blank to let Kubernetes pick one"
                ));
            }
            Some(np)
        }
    };

    let cpu_request = check_quantity("cpu_request", cpu_request, Resource::Cpu)?;
    let cpu_limit = check_quantity("cpu_limit", cpu_limit, Resource::Cpu)?;
    let memory_request = check_quantity("memory_request", memory_request, Resource::Memory)?;
    let memory_limit = check_quantity("memory_limit", memory_limit, Resource::Memory)?;

    let env = parse_env(env)?;
    let extra_labels = parse_labels(labels)?;

    let probe_path = probe_path.trim();
    if !probe_path.is_empty() {
        if !probe_path.starts_with('/') {
            return Err(format!(
                "invalid probe_path {probe_path:?}: an HTTP probe path must start with '/', e.g. \"/healthz\""
            ));
        }
        if probe_path.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Err(format!(
                "invalid probe_path {probe_path:?}: a probe path cannot contain spaces or control characters"
            ));
        }
    }

    let mut out = String::new();

    // ---- Deployment -------------------------------------------------------
    out.push_str("apiVersion: apps/v1\n");
    out.push_str("kind: Deployment\n");
    out.push_str("metadata:\n");
    out.push_str(&format!("  name: {}\n", yaml_scalar(name)));
    if !namespace.is_empty() {
        out.push_str(&format!("  namespace: {}\n", yaml_scalar(namespace)));
    }
    out.push_str("  labels:\n");
    push_labels(&mut out, "    ", name, &extra_labels);
    out.push_str("spec:\n");
    out.push_str(&format!("  replicas: {replicas}\n"));
    out.push_str("  selector:\n");
    out.push_str("    matchLabels:\n");
    out.push_str(&format!("      app: {}\n", yaml_scalar(name)));
    out.push_str("  template:\n");
    out.push_str("    metadata:\n");
    out.push_str("      labels:\n");
    push_labels(&mut out, "        ", name, &extra_labels);
    out.push_str("    spec:\n");
    out.push_str("      containers:\n");
    out.push_str(&format!("        - name: {}\n", yaml_scalar(name)));
    out.push_str(&format!("          image: {}\n", yaml_scalar(image)));
    out.push_str(&format!(
        "          imagePullPolicy: {}\n",
        pull_policy.as_str()
    ));
    out.push_str("          ports:\n");
    out.push_str(&format!("            - name: http\n              containerPort: {container_port}\n              protocol: TCP\n"));
    if !env.is_empty() {
        out.push_str("          env:\n");
        for (k, v) in &env {
            out.push_str(&format!("            - name: {k}\n"));
            // Env values are always strings in Kubernetes — quote every one so
            // `PORT=8080` can't be read back as a YAML integer.
            out.push_str(&format!("              value: {}\n", yaml_quoted(v)));
        }
    }
    let requests: Vec<(&str, &str)> = [("cpu", cpu_request), ("memory", memory_request)]
        .into_iter()
        .filter(|(_, v)| !v.is_empty())
        .collect();
    let limits: Vec<(&str, &str)> = [("cpu", cpu_limit), ("memory", memory_limit)]
        .into_iter()
        .filter(|(_, v)| !v.is_empty())
        .collect();
    if !requests.is_empty() || !limits.is_empty() {
        out.push_str("          resources:\n");
        for (heading, rows) in [("requests", &requests), ("limits", &limits)] {
            if rows.is_empty() {
                continue;
            }
            out.push_str(&format!("            {heading}:\n"));
            for (k, v) in rows.iter() {
                // Quantities are validated above, so they are safe bare scalars.
                out.push_str(&format!("              {k}: {v}\n"));
            }
        }
    }
    if !probe_path.is_empty() {
        for (probe, initial_delay) in [("livenessProbe", 15), ("readinessProbe", 5)] {
            out.push_str(&format!("          {probe}:\n"));
            out.push_str("            httpGet:\n");
            out.push_str(&format!("              path: {}\n", yaml_scalar(probe_path)));
            out.push_str(&format!("              port: {container_port}\n"));
            out.push_str(&format!(
                "            initialDelaySeconds: {initial_delay}\n"
            ));
            out.push_str("            periodSeconds: 10\n");
        }
    }

    // ---- Service ----------------------------------------------------------
    out.push_str("---\n");
    out.push_str("apiVersion: v1\n");
    out.push_str("kind: Service\n");
    out.push_str("metadata:\n");
    out.push_str(&format!("  name: {}\n", yaml_scalar(name)));
    if !namespace.is_empty() {
        out.push_str(&format!("  namespace: {}\n", yaml_scalar(namespace)));
    }
    out.push_str("  labels:\n");
    push_labels(&mut out, "    ", name, &extra_labels);
    out.push_str("spec:\n");
    out.push_str(&format!("  type: {}\n", service_type.as_str()));
    out.push_str("  selector:\n");
    out.push_str(&format!("    app: {}\n", yaml_scalar(name)));
    out.push_str("  ports:\n");
    out.push_str(&format!("    - name: http\n      port: {service_port}\n      targetPort: {container_port}\n      protocol: TCP\n"));
    if let Some(np) = node_port {
        out.push_str(&format!("      nodePort: {np}\n"));
    }

    Ok(out)
}

/// Emit the `app: <name>` selector label followed by the extra labels, each at
/// `indent`. Used for all three label blocks so they never drift apart.
fn push_labels(out: &mut String, indent: &str, name: &str, extra: &[(String, String)]) {
    out.push_str(&format!("{indent}app: {}\n", yaml_scalar(name)));
    for (k, v) in extra {
        out.push_str(&format!(
            "{indent}{}: {}\n",
            yaml_scalar(k),
            yaml_scalar(v)
        ));
    }
}

/// Validate a TCP port number, returning it unchanged.
fn check_port(field: &str, port: i64) -> Result<i64, String> {
    if (1..=65535).contains(&port) {
        Ok(port)
    } else {
        Err(format!(
            "invalid {field} {port}: a TCP port must be 1-65535"
        ))
    }
}

/// Validate a Kubernetes resource quantity, returning the trimmed value (`""`
/// when the field was left blank).
fn check_quantity<'a>(
    field: &str,
    value: &'a str,
    resource: Resource,
) -> Result<&'a str, String> {
    let v = value.trim();
    if v.is_empty() {
        return Ok("");
    }
    let bad = || {
        format!(
            "invalid {field} {v:?}: use a Kubernetes quantity like {}",
            resource.example()
        )
    };
    let suffix = resource
        .suffixes()
        .iter()
        .find(|s| v.len() > s.len() && v.ends_with(**s))
        .ok_or_else(bad)?;
    let number = &v[..v.len() - suffix.len()];
    if number.is_empty() || !number.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return Err(bad());
    }
    match number.parse::<f64>() {
        Ok(n) if n > 0.0 => Ok(v),
        Ok(_) => Err(format!(
            "invalid {field} {v:?}: a resource quantity must be greater than zero (leave it blank to omit the field)"
        )),
        Err(_) => Err(bad()),
    }
}

/// Parse `KEY=value` lines into ordered pairs. Blank and `#`-comment lines are
/// skipped, a leading `export ` is dropped, and a value wrapped in matching
/// single or double quotes is unquoted. Duplicate keys keep their first-seen
/// position with the last value winning.
fn parse_env(env: &str) -> Result<Vec<(String, String)>, String> {
    let mut out: Vec<(String, String)> = Vec::new();
    for raw in env.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").map(str::trim).unwrap_or(line);
        let eq = line.find('=').ok_or_else(|| {
            format!("invalid env line {line:?}: expected KEY=value, one per line")
        })?;
        let key = line[..eq].trim().to_string();
        if !is_env_key(&key) {
            return Err(format!(
                "invalid env name {key:?}: an environment variable name must start with a letter or '_' and contain only letters, digits and '_'"
            ));
        }
        let value = unquote(line[eq + 1..].trim());
        match out.iter_mut().find(|(k, _)| *k == key) {
            Some(slot) => slot.1 = value,
            None => out.push((key, value)),
        }
    }
    Ok(out)
}

/// Parse extra `key=value` labels, newline- or comma-separated. The `app` key
/// is reserved because it is the selector both resources match on.
fn parse_labels(labels: &str) -> Result<Vec<(String, String)>, String> {
    let mut out: Vec<(String, String)> = Vec::new();
    for tok in labels.split(['\n', ',']) {
        let tok = tok.trim();
        if tok.is_empty() || tok.starts_with('#') {
            continue;
        }
        let eq = tok
            .find('=')
            .ok_or_else(|| format!("invalid label {tok:?}: expected key=value"))?;
        let key = tok[..eq].trim().to_string();
        let value = tok[eq + 1..].trim().to_string();
        if key == "app" {
            return Err(
                "the label key \"app\" is reserved: it is set from the name and used as the Deployment and Service selector"
                    .to_string(),
            );
        }
        if !is_label_key(&key) {
            return Err(format!(
                "invalid label key {key:?}: use letters, digits, '-', '_' and '.' (at most 63 characters), optionally prefixed with a DNS subdomain and '/'"
            ));
        }
        if !is_label_value(&value) {
            return Err(format!(
                "invalid label value {value:?}: use letters, digits, '-', '_' and '.' (at most 63 characters), starting and ending with a letter or digit"
            ));
        }
        match out.iter_mut().find(|(k, _)| *k == key) {
            Some(slot) => slot.1 = value,
            None => out.push((key, value)),
        }
    }
    Ok(out)
}

/// Strip surrounding matching quotes from an env value, interpreting the usual
/// escapes inside double quotes.
fn unquote(raw: &str) -> String {
    let bytes = raw.as_bytes();
    if bytes.len() >= 2 && bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\'' {
        return raw[1..raw.len() - 1].to_string();
    }
    if bytes.len() < 2 || bytes[0] != b'"' || bytes[bytes.len() - 1] != b'"' {
        return raw.to_string();
    }
    let inner = &raw[1..raw.len() - 1];
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            Some('"') => out.push('"'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// A valid RFC 1123 object name: 1–63 lowercase alphanumerics, `-` and `.`,
/// starting and ending with an alphanumeric.
fn is_rfc1123_name(n: &str) -> bool {
    if n.is_empty() || n.len() > 63 {
        return false;
    }
    let alnum = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit();
    alnum(n.chars().next().unwrap())
        && alnum(n.chars().last().unwrap())
        && n.chars().all(|c| alnum(c) || c == '-' || c == '.')
}

/// A C_IDENTIFIER-style environment variable name.
fn is_env_key(k: &str) -> bool {
    let mut chars = k.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// A label key: an optional `dns.subdomain/` prefix plus a 1–63 character name
/// of alphanumerics, `-`, `_` and `.` that starts and ends alphanumeric.
fn is_label_key(k: &str) -> bool {
    let (prefix, name) = match k.split_once('/') {
        Some((p, n)) => (Some(p), n),
        None => (None, k),
    };
    if let Some(p) = prefix {
        if p.is_empty() || p.len() > 253 || !p.chars().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.'
        }) {
            return false;
        }
    }
    is_label_value(name) && !name.is_empty()
}

/// A label value: empty, or 1–63 characters of alphanumerics, `-`, `_` and `.`
/// starting and ending with an alphanumeric.
fn is_label_value(v: &str) -> bool {
    if v.is_empty() {
        return true;
    }
    if v.len() > 63 {
        return false;
    }
    let first = v.chars().next().unwrap();
    let last = v.chars().last().unwrap();
    first.is_ascii_alphanumeric()
        && last.is_ascii_alphanumeric()
        && v.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

/// Render a string as a YAML scalar, double-quoting whenever a plain scalar
/// would be ambiguous. Over-quoting is always safe.
fn yaml_scalar(s: &str) -> String {
    if is_plain_safe(s) {
        s.to_string()
    } else {
        yaml_quoted(s)
    }
}

/// Always-double-quoted YAML scalar, with the special characters escaped.
fn yaml_quoted(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// True when `s` can be written bare with no chance of being reinterpreted as a
/// number, boolean, null or structural token. Image references and probe paths
/// stay bare (`nginx:1.27`, `/healthz`) because they contain no space — a `:`
/// only starts a mapping when a space follows it.
fn is_plain_safe(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let first = s.chars().next().unwrap();
    if !(first.is_ascii_alphabetic() || first == '_' || first == '/') {
        return false;
    }
    if !s
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/' | ':' | '@'))
    {
        return false;
    }
    // YAML 1.1 boolean/null spellings that must be quoted to stay strings.
    !matches!(
        s.to_ascii_lowercase().as_str(),
        "true" | "false" | "yes" | "no" | "on" | "off" | "y" | "n" | "null" | "none" | "nan"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The minimal call: everything optional left blank/defaulted.
    fn minimal() -> String {
        scaffold(
            "web", "nginx:1.27", "", 1, 8080, 80, "ClusterIP", None, "IfNotPresent", "", "", "",
            "", "", "", "",
        )
        .unwrap()
    }

    #[test]
    fn minimal_deployment_and_service() {
        let expected = "\
apiVersion: apps/v1
kind: Deployment
metadata:
  name: web
  labels:
    app: web
spec:
  replicas: 1
  selector:
    matchLabels:
      app: web
  template:
    metadata:
      labels:
        app: web
    spec:
      containers:
        - name: web
          image: nginx:1.27
          imagePullPolicy: IfNotPresent
          ports:
            - name: http
              containerPort: 8080
              protocol: TCP
---
apiVersion: v1
kind: Service
metadata:
  name: web
  labels:
    app: web
spec:
  type: ClusterIP
  selector:
    app: web
  ports:
    - name: http
      port: 80
      targetPort: 8080
      protocol: TCP
";
        assert_eq!(minimal(), expected);
    }

    #[test]
    fn deterministic_for_the_same_inputs() {
        assert_eq!(minimal(), minimal());
    }

    #[test]
    fn full_manifest_has_every_optional_block() {
        let out = scaffold(
            "api",
            "ghcr.io/acme/api:v1.2.3",
            "prod",
            3,
            8000,
            80,
            "NodePort",
            Some(30080),
            "Always",
            "100m",
            "500m",
            "128Mi",
            "256Mi",
            "LOG_LEVEL=info\nPORT=8000\n",
            "tier=backend\nteam=payments",
            "/healthz",
        )
        .unwrap();
        assert!(out.contains("  namespace: prod\n"), "{out}");
        assert!(out.contains("  replicas: 3\n"), "{out}");
        assert!(out.contains("          imagePullPolicy: Always\n"), "{out}");
        assert!(out.contains("          image: ghcr.io/acme/api:v1.2.3\n"), "{out}");
        // Env values are always quoted so numeric-looking ones stay strings.
        assert!(
            out.contains("            - name: PORT\n              value: \"8000\"\n"),
            "{out}"
        );
        assert!(
            out.contains(
                "          resources:\n            requests:\n              cpu: 100m\n              memory: 128Mi\n            limits:\n              cpu: 500m\n              memory: 256Mi\n"
            ),
            "{out}"
        );
        assert!(out.contains("          livenessProbe:\n"), "{out}");
        assert!(out.contains("          readinessProbe:\n"), "{out}");
        assert!(out.contains("              path: /healthz\n"), "{out}");
        assert!(out.contains("  type: NodePort\n"), "{out}");
        assert!(out.contains("      nodePort: 30080\n"), "{out}");
        // Extra labels are merged after `app`, in input order, in all three blocks.
        assert_eq!(out.matches("app: api\n").count(), 5, "{out}");
        assert_eq!(out.matches("tier: backend\n").count(), 3, "{out}");
    }

    #[test]
    fn resources_block_omitted_when_all_blank_and_partial_when_some_set() {
        assert!(!minimal().contains("resources:"));
        let out = scaffold(
            "web", "nginx", "", 1, 8080, 80, "ClusterIP", None, "", "", "", "256Mi", "", "", "",
            "",
        )
        .unwrap();
        assert!(
            out.contains("          resources:\n            requests:\n              memory: 256Mi\n"),
            "{out}"
        );
        assert!(!out.contains("limits:"), "{out}");
    }

    #[test]
    fn env_comments_export_quotes_and_duplicates() {
        let out = scaffold(
            "web",
            "nginx",
            "",
            1,
            8080,
            80,
            "ClusterIP",
            None,
            "",
            "",
            "",
            "",
            "",
            "# config\nexport MODE=\"a,b\"\nMODE=final\nGREETING='hello world'\n",
            "",
            "",
        )
        .unwrap();
        assert!(
            out.contains("            - name: MODE\n              value: \"final\"\n"),
            "{out}"
        );
        assert!(
            out.contains("            - name: GREETING\n              value: \"hello world\"\n"),
            "{out}"
        );
        assert!(!out.contains("config"), "{out}");
    }

    #[test]
    fn enum_spellings_are_accepted_case_insensitively() {
        let out = scaffold(
            "web", "nginx", "", 1, 8080, 80, "loadbalancer", None, "never", "", "", "", "", "",
            "", "",
        )
        .unwrap();
        assert!(out.contains("  type: LoadBalancer\n"), "{out}");
        assert!(out.contains("          imagePullPolicy: Never\n"), "{out}");
    }

    #[test]
    fn ambiguous_scalars_are_quoted() {
        // An all-digit name and a boolean-looking label value must stay strings.
        let out = scaffold(
            "123", "nginx", "", 1, 8080, 80, "ClusterIP", None, "", "", "", "", "", "",
            "enabled=true", "",
        )
        .unwrap();
        assert!(out.contains("  name: \"123\"\n"), "{out}");
        assert!(out.contains("enabled: \"true\"\n"), "{out}");
    }

    #[test]
    fn error_missing_name_and_image() {
        assert!(scaffold("", "nginx", "", 1, 8080, 80, "", None, "", "", "", "", "", "", "", "")
            .unwrap_err()
            .contains("name is required"));
        assert!(scaffold("web", "  ", "", 1, 8080, 80, "", None, "", "", "", "", "", "", "", "")
            .unwrap_err()
            .contains("image is required"));
    }

    #[test]
    fn error_invalid_name_namespace_and_image() {
        let err = scaffold("My_App", "nginx", "", 1, 8080, 80, "", None, "", "", "", "", "", "", "", "")
            .unwrap_err();
        assert!(err.contains("invalid name"), "{err}");
        let err = scaffold("web", "nginx", "Prod", 1, 8080, 80, "", None, "", "", "", "", "", "", "", "")
            .unwrap_err();
        assert!(err.contains("invalid namespace"), "{err}");
        let err = scaffold("web", "my image", "", 1, 8080, 80, "", None, "", "", "", "", "", "", "", "")
            .unwrap_err();
        assert!(err.contains("invalid image"), "{err}");
    }

    #[test]
    fn error_out_of_range_numbers() {
        let err = scaffold("web", "nginx", "", 101, 8080, 80, "", None, "", "", "", "", "", "", "", "")
            .unwrap_err();
        assert!(err.contains("invalid replicas"), "{err}");
        let err = scaffold("web", "nginx", "", 1, 0, 80, "", None, "", "", "", "", "", "", "", "")
            .unwrap_err();
        assert!(err.contains("invalid container_port"), "{err}");
        let err = scaffold("web", "nginx", "", 1, 8080, 70000, "", None, "", "", "", "", "", "", "", "")
            .unwrap_err();
        assert!(err.contains("invalid service_port"), "{err}");
    }

    #[test]
    fn error_node_port_range_and_service_type_mismatch() {
        let err = scaffold(
            "web", "nginx", "", 1, 8080, 80, "NodePort", Some(8080), "", "", "", "", "", "", "",
            "",
        )
        .unwrap_err();
        assert!(err.contains("invalid node_port"), "{err}");
        let err = scaffold(
            "web", "nginx", "", 1, 8080, 80, "ClusterIP", Some(30080), "", "", "", "", "", "", "",
            "",
        )
        .unwrap_err();
        assert!(err.contains("only valid when service_type"), "{err}");
    }

    #[test]
    fn error_bad_enums() {
        let err = scaffold("web", "nginx", "", 1, 8080, 80, "Ingress", None, "", "", "", "", "", "", "", "")
            .unwrap_err();
        assert!(err.contains("invalid service_type"), "{err}");
        let err = scaffold("web", "nginx", "", 1, 8080, 80, "", None, "Sometimes", "", "", "", "", "", "", "")
            .unwrap_err();
        assert!(err.contains("invalid image_pull_policy"), "{err}");
    }

    #[test]
    fn resource_quantities_accepted_and_rejected() {
        for good in ["100m", "0.5", "2"] {
            assert_eq!(check_quantity("cpu_request", good, Resource::Cpu).unwrap(), good);
        }
        for good in ["128Mi", "512M", "1Gi", "134217728"] {
            assert_eq!(check_quantity("memory_limit", good, Resource::Memory).unwrap(), good);
        }
        for bad in ["lots", "100mm", "-1", "0"] {
            assert!(check_quantity("cpu_limit", bad, Resource::Cpu).is_err(), "{bad}");
        }
        // A CPU field must not accept a memory suffix.
        assert!(check_quantity("cpu_limit", "128Mi", Resource::Cpu).is_err());
    }

    #[test]
    fn error_bad_env_label_and_probe() {
        let err = scaffold("web", "nginx", "", 1, 8080, 80, "", None, "", "", "", "", "", "NO_EQUALS", "", "")
            .unwrap_err();
        assert!(err.contains("invalid env line"), "{err}");
        let err = scaffold("web", "nginx", "", 1, 8080, 80, "", None, "", "", "", "", "", "2BAD=x", "", "")
            .unwrap_err();
        assert!(err.contains("invalid env name"), "{err}");
        let err = scaffold("web", "nginx", "", 1, 8080, 80, "", None, "", "", "", "", "", "", "tier", "")
            .unwrap_err();
        assert!(err.contains("invalid label"), "{err}");
        let err = scaffold("web", "nginx", "", 1, 8080, 80, "", None, "", "", "", "", "", "", "app=other", "")
            .unwrap_err();
        assert!(err.contains("reserved"), "{err}");
        let err = scaffold("web", "nginx", "", 1, 8080, 80, "", None, "", "", "", "", "", "", "", "healthz")
            .unwrap_err();
        assert!(err.contains("invalid probe_path"), "{err}");
    }

    #[test]
    fn prefixed_label_keys_are_allowed() {
        let out = scaffold(
            "web", "nginx", "", 1, 8080, 80, "", None, "", "", "", "", "", "",
            "app.kubernetes.io/component=frontend", "",
        )
        .unwrap();
        assert!(out.contains("app.kubernetes.io/component: frontend\n"), "{out}");
    }
}

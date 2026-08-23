//! gizza-ai/docker-compose-generator — turn a short one-line-per-service spec
//! into a complete, validated `docker-compose.yml`.
//!
//! The spec is deliberately terse and diff-friendly:
//!
//! ```text
//! web: nginx:alpine ports=8080:80 volumes=./site:/usr/share/nginx/html:ro depends=api
//! api: ghcr.io/acme/api:v1 ports=3000 env=PORT=3000,LOG_LEVEL=info restart=always
//! db:  postgres:16-alpine volumes=dbdata:/var/lib/postgresql/data env=POSTGRES_PASSWORD=secret
//! ```
//!
//! Everything is validated before a single byte of YAML is written, top-level
//! `volumes:` / `networks:` sections are derived from what the services actually
//! reference, and the same input always produces the same bytes.

/// Hard cap on services in one file — well past any hand-written compose file,
/// and it keeps a pasted 10k-line accident from becoming a 10k-service YAML.
pub const MAX_SERVICES: usize = 25;

const RESTART_POLICIES: [&str; 4] = ["no", "always", "on-failure", "unless-stopped"];
const NETWORK_DRIVERS: [&str; 5] = ["bridge", "host", "overlay", "macvlan", "ipvlan"];
const VOLUME_MODES: [&str; 7] = ["ro", "rw", "z", "Z", "cached", "delegated", "consistent"];
const SERVICE_KEYS: [&str; 18] = [
    "image",
    "build",
    "ports",
    "expose",
    "volumes",
    "env",
    "environment",
    "env_file",
    "depends",
    "depends_on",
    "restart",
    "command",
    "entrypoint",
    "container_name",
    "user",
    "working_dir",
    "networks",
    "labels",
];

/// One parsed service, in spec order.
#[derive(Default)]
struct Service {
    name: String,
    image: String,
    build: String,
    ports: Vec<String>,
    expose: Vec<String>,
    volumes: Vec<String>,
    env: Vec<(String, String)>,
    env_file: Vec<String>,
    depends_on: Vec<String>,
    restart: String,
    command: String,
    entrypoint: String,
    container_name: String,
    user: String,
    working_dir: String,
    networks: Vec<String>,
    labels: Vec<(String, String)>,
    healthcheck: String,
}

// ---------------------------------------------------------------------------
// Tokenizing helpers — double quotes protect spaces and commas inside a value.
// ---------------------------------------------------------------------------

/// Split on whitespace that is not inside double quotes. Quotes are kept so the
/// value parser can strip them per item.
fn split_ws_quoted(s: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut started = false;
    for c in s.chars() {
        match c {
            '"' => {
                in_quotes = !in_quotes;
                cur.push(c);
                started = true;
            }
            c if c.is_whitespace() && !in_quotes => {
                if started {
                    out.push(std::mem::take(&mut cur));
                    started = false;
                }
            }
            c => {
                cur.push(c);
                started = true;
            }
        }
    }
    if in_quotes {
        return Err("unbalanced double quote".to_string());
    }
    if started {
        out.push(cur);
    }
    Ok(out)
}

/// Split a comma-separated option value, respecting double quotes.
fn split_list(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    for c in s.chars() {
        match c {
            '"' => {
                in_quotes = !in_quotes;
                cur.push(c);
            }
            ',' if !in_quotes => out.push(std::mem::take(&mut cur)),
            c => cur.push(c),
        }
    }
    out.push(cur);
    out.into_iter()
        .map(|x| x.trim().to_string())
        .filter(|x| !x.is_empty())
        .collect()
}

/// Strip a surrounding pair of double quotes (and unescape `\"` inside).
fn unquote(s: &str) -> String {
    let t = s.trim();
    if t.len() >= 2 && t.starts_with('"') && t.ends_with('"') {
        t[1..t.len() - 1].replace("\\\"", "\"")
    } else {
        t.to_string()
    }
}

// ---------------------------------------------------------------------------
// YAML scalar emission
// ---------------------------------------------------------------------------

/// True when YAML would read the scalar as something other than a string.
fn yaml_ambiguous(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "true" | "false" | "yes" | "no" | "on" | "off" | "null" | "~" | "y" | "n"
    ) {
        return true;
    }
    if s.parse::<f64>().is_ok() {
        return true;
    }
    // Sexagesimal-looking values (the classic `ports: 22:22` → 1342 trap).
    if s.contains(':') && s.chars().all(|c| c.is_ascii_digit() || c == ':') {
        return true;
    }
    // Leading zero would be read as octal by some parsers.
    s.len() > 1 && s.starts_with('0') && s[1..].chars().all(|c| c.is_ascii_digit())
}

fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Emit a scalar bare when that is unambiguous, double-quoted otherwise.
fn scalar(s: &str) -> String {
    if s.is_empty() {
        return "\"\"".to_string();
    }
    let risky = s.chars().any(|c| {
        matches!(
            c,
            '"' | '\'' | '\\' | '\n' | '\r' | '\t' | '{' | '}' | '[' | ']' | ',' | '&' | '*' | '?'
                | '|' | '>' | '%' | '@' | '`' | '!'
        )
    }) || s.starts_with(['-', ' ', '#', ':'])
        || s.ends_with([' ', ':'])
        || s.contains(": ")
        || s.contains(" #")
        || yaml_ambiguous(s);
    if risky {
        quote(s)
    } else {
        s.to_string()
    }
}

// ---------------------------------------------------------------------------
// Validation helpers
// ---------------------------------------------------------------------------

fn valid_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 63
        && s.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
}

fn valid_env_key(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn valid_port_num(p: &str) -> bool {
    !p.is_empty()
        && p.chars().all(|c| c.is_ascii_digit())
        && p.parse::<u32>().is_ok_and(|n| (1..=65535).contains(&n))
}

/// A port or an inclusive `start-end` range.
fn valid_port_part(p: &str) -> bool {
    match p.split_once('-') {
        Some((a, b)) => valid_port_num(a) && valid_port_num(b),
        None => valid_port_num(p),
    }
}

/// `[ip:][host:]container[/proto]`, with `[::1]`-style bracketed IPv6 handled.
fn check_port(service: &str, raw: &str) -> Result<(), String> {
    let bad = |why: &str| Err(format!("service '{service}': invalid port '{raw}' ({why})"));
    let (body, proto) = match raw.rsplit_once('/') {
        Some((b, p)) => (b, Some(p)),
        None => (raw, None),
    };
    if let Some(p) = proto {
        if !matches!(p, "tcp" | "udp" | "sctp") {
            return bad("protocol must be tcp, udp or sctp");
        }
    }
    // Peel a bracketed IPv6 host address off the front, if present.
    let (ip, rest) = if let Some(stripped) = body.strip_prefix('[') {
        match stripped.split_once(']') {
            Some((addr, tail)) => (Some(addr), tail.strip_prefix(':').unwrap_or(tail)),
            None => return bad("unclosed '[' in bind address"),
        }
    } else {
        (None, body)
    };
    let has_ip = ip.is_some();
    let parts: Vec<&str> = rest.split(':').collect();
    let (host, container) = match (has_ip, parts.len()) {
        (_, 1) => (None, parts[0]),
        (_, 2) => (Some(parts[0]), parts[1]),
        (false, 3) => (Some(parts[1]), parts[2]),
        _ => return bad("expected [ip:][host:]container[/proto]"),
    };
    if !has_ip && parts.len() == 3 && parts[0].is_empty() {
        return bad("bind address is empty");
    }
    if let Some(h) = host {
        // An empty host port means "let Docker pick one" — only valid alongside
        // an explicit bind address.
        let empty_ok = h.is_empty() && (has_ip || parts.len() == 3);
        if !empty_ok && !valid_port_part(h) {
            return bad("host port must be 1-65535 (or a start-end range)");
        }
    }
    if !valid_port_part(container) {
        return bad("container port must be 1-65535 (or a start-end range)");
    }
    Ok(())
}

/// True when a volume source names a top-level volume rather than a host path.
fn is_named_volume(src: &str) -> bool {
    !src.is_empty() && !src.starts_with(['/', '.', '~', '$'])
}

/// `dst` | `src:dst` | `src:dst:mode`. Returns the named volume, if any.
fn check_volume(service: &str, raw: &str) -> Result<Option<String>, String> {
    let bad = |why: &str| Err(format!("service '{service}': invalid volume '{raw}' ({why})"));
    let parts: Vec<&str> = raw.split(':').collect();
    let (src, dst) = match parts.len() {
        1 => (None, parts[0]),
        2 => (Some(parts[0]), parts[1]),
        3 => {
            if !VOLUME_MODES.contains(&parts[2]) {
                return bad("mode must be one of ro, rw, z, Z, cached, delegated, consistent");
            }
            (Some(parts[0]), parts[1])
        }
        _ => return bad("expected [source:]/container/path[:mode]"),
    };
    if !dst.starts_with('/') {
        return bad("the container path must be absolute (start with '/')");
    }
    match src {
        Some(s) if s.is_empty() => bad("the source is empty"),
        Some(s) if is_named_volume(s) => {
            if !valid_name(s) {
                return bad("a named volume must be alphanumerics, '.', '_' or '-'");
            }
            Ok(Some(s.to_string()))
        }
        _ => Ok(None),
    }
}

/// Parse `KEY=value` / bare `KEY` env entries; values may be quoted.
fn parse_env_item(service: &str, raw: &str) -> Result<(String, String), String> {
    let (key, value) = match raw.split_once('=') {
        Some((k, v)) => (k.trim(), unquote(v)),
        None => (raw.trim(), String::new()),
    };
    if !valid_env_key(key) {
        return Err(format!(
            "service '{service}': invalid environment name '{key}' (letters, digits and '_', not starting with a digit)"
        ));
    }
    Ok((key.to_string(), value))
}

/// Insert-or-replace, preserving first-seen order so output stays deterministic.
fn upsert(list: &mut Vec<(String, String)>, key: String, value: String) {
    if let Some(slot) = list.iter_mut().find(|(k, _)| *k == key) {
        slot.1 = value;
    } else {
        list.push((key, value));
    }
}

/// Shared env / env_file text: one entry per line or comma-separated, `#`
/// comments and blank lines skipped, a leading `export ` dropped.
fn parse_shared_lines(text: &str) -> Vec<String> {
    text.lines()
        .flat_map(split_list)
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.strip_prefix("export ").unwrap_or(&l).trim().to_string())
        .collect()
}

// ---------------------------------------------------------------------------
// Spec parsing
// ---------------------------------------------------------------------------

fn parse_service_line(line: &str) -> Result<Service, String> {
    let (name, rest) = line
        .split_once(':')
        .ok_or_else(|| format!("line '{line}': expected \"name: image [key=value ...]\""))?;
    let name = name.trim();
    if !valid_name(name) {
        return Err(format!(
            "invalid service name '{name}': use letters, digits, '.', '_' or '-', starting with a letter or digit, max 63 characters"
        ));
    }
    let mut svc = Service {
        name: name.to_string(),
        ..Default::default()
    };
    let tokens =
        split_ws_quoted(rest).map_err(|e| format!("service '{}': {e}", svc.name))?;
    for (i, token) in tokens.iter().enumerate() {
        let key_value = token
            .split_once('=')
            .filter(|(k, _)| SERVICE_KEYS.contains(k) || *k == "healthcheck");
        let Some((key, raw)) = key_value else {
            // The first bare token is the image; anything later is a typo'd key.
            if i == 0 {
                svc.image = unquote(token);
                continue;
            }
            return Err(format!(
                "service '{}': unknown option '{token}' (valid options: {}, healthcheck)",
                svc.name,
                SERVICE_KEYS.join(", ")
            ));
        };
        let value = unquote(raw);
        match key {
            "image" => svc.image = value,
            "build" => svc.build = value,
            "ports" => svc.ports = split_list(raw).iter().map(|v| unquote(v)).collect(),
            "expose" => svc.expose = split_list(raw).iter().map(|v| unquote(v)).collect(),
            "volumes" => svc.volumes = split_list(raw).iter().map(|v| unquote(v)).collect(),
            "env" | "environment" => {
                for item in split_list(raw) {
                    let (k, v) = parse_env_item(&svc.name, &item)?;
                    upsert(&mut svc.env, k, v);
                }
            }
            "env_file" => svc.env_file = split_list(raw).iter().map(|v| unquote(v)).collect(),
            "depends" | "depends_on" => {
                svc.depends_on = split_list(raw).iter().map(|v| unquote(v)).collect()
            }
            "restart" => svc.restart = value,
            "command" => svc.command = value,
            "entrypoint" => svc.entrypoint = value,
            "container_name" => svc.container_name = value,
            "user" => svc.user = value,
            "working_dir" => svc.working_dir = value,
            "networks" => svc.networks = split_list(raw).iter().map(|v| unquote(v)).collect(),
            "labels" => {
                for item in split_list(raw) {
                    let (k, v) = item
                        .split_once('=')
                        .map(|(k, v)| (k.trim().to_string(), unquote(v)))
                        .ok_or_else(|| {
                            format!("service '{}': label '{item}' must be key=value", svc.name)
                        })?;
                    if k.is_empty() {
                        return Err(format!("service '{}': label key is empty", svc.name));
                    }
                    upsert(&mut svc.labels, k, v);
                }
            }
            "healthcheck" => svc.healthcheck = value,
            _ => unreachable!("SERVICE_KEYS filter already rejected unknown keys"),
        }
    }
    Ok(svc)
}

// ---------------------------------------------------------------------------
// Generation
// ---------------------------------------------------------------------------

/// Build a `docker-compose.yml` from a short spec.
///
/// `services` is the spec, one service per line. The remaining arguments are
/// file-wide settings; each is ignored when blank (or, for `restart` /
/// `compose_version`, when set to `none`).
#[allow(clippy::too_many_arguments)]
pub fn generate(
    services: &str,
    project_name: &str,
    compose_version: &str,
    network: &str,
    network_driver: &str,
    restart: &str,
    env: &str,
    env_file: &str,
) -> Result<String, String> {
    let project_name = project_name.trim();
    let compose_version = compose_version.trim();
    let network = network.trim();
    let network_driver = network_driver.trim();
    let restart = restart.trim();

    if !project_name.is_empty() && !valid_name(project_name) {
        return Err(format!(
            "invalid project_name '{project_name}': use letters, digits, '.', '_' or '-', starting with a letter or digit"
        ));
    }
    if !network.is_empty() && !valid_name(network) {
        return Err(format!(
            "invalid network '{network}': use letters, digits, '.', '_' or '-', starting with a letter or digit"
        ));
    }
    let driver = if network_driver.is_empty() {
        "bridge"
    } else {
        network_driver
    };
    if !NETWORK_DRIVERS.contains(&driver) {
        return Err(format!(
            "unknown network_driver '{driver}' (use one of: {})",
            NETWORK_DRIVERS.join(", ")
        ));
    }
    let default_restart = if restart.is_empty() || restart == "none" {
        ""
    } else {
        if !RESTART_POLICIES.contains(&restart) {
            return Err(format!(
                "unknown restart policy '{restart}' (use one of: none, {})",
                RESTART_POLICIES.join(", ")
            ));
        }
        restart
    };

    // Parse every service line first, so errors mention the right line.
    let lines: Vec<&str> = services
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    if lines.is_empty() {
        return Err(
            "no services: give at least one line like \"web: nginx:alpine ports=8080:80\""
                .to_string(),
        );
    }
    if lines.len() > MAX_SERVICES {
        return Err(format!(
            "too many services: {} (maximum {MAX_SERVICES})",
            lines.len()
        ));
    }

    let mut parsed: Vec<Service> = Vec::with_capacity(lines.len());
    for line in &lines {
        let svc = parse_service_line(line)?;
        if parsed.iter().any(|s| s.name == svc.name) {
            return Err(format!("duplicate service '{}'", svc.name));
        }
        parsed.push(svc);
    }

    // File-wide env / env_file, applied under each service's own entries.
    let shared_env: Vec<(String, String)> = {
        let mut out = Vec::new();
        for item in parse_shared_lines(env) {
            let (k, v) = parse_env_item("(shared env)", &item)?;
            upsert(&mut out, k, v);
        }
        out
    };
    let shared_env_file = parse_shared_lines(env_file);

    let mut named_volumes: Vec<String> = Vec::new();
    let mut networks: Vec<String> = Vec::new();
    if !network.is_empty() {
        networks.push(network.to_string());
    }

    for i in 0..parsed.len() {
        let name = parsed[i].name.clone();
        if parsed[i].image.is_empty() && parsed[i].build.is_empty() {
            return Err(format!(
                "service '{name}': needs an image (\"{name}: nginx:alpine\") or a build context (build=./dir)"
            ));
        }
        if !parsed[i].restart.is_empty() && !RESTART_POLICIES.contains(&parsed[i].restart.as_str())
        {
            return Err(format!(
                "service '{name}': unknown restart policy '{}' (use one of: {})",
                parsed[i].restart,
                RESTART_POLICIES.join(", ")
            ));
        }
        if !parsed[i].container_name.is_empty() && !valid_name(&parsed[i].container_name) {
            return Err(format!(
                "service '{name}': invalid container_name '{}'",
                parsed[i].container_name
            ));
        }
        for port in parsed[i].ports.clone() {
            check_port(&name, &port)?;
        }
        for port in parsed[i].expose.clone() {
            if !valid_port_part(&port) {
                return Err(format!(
                    "service '{name}': invalid expose port '{port}' (1-65535, or a start-end range)"
                ));
            }
        }
        for volume in parsed[i].volumes.clone() {
            if let Some(named) = check_volume(&name, &volume)? {
                if !named_volumes.contains(&named) {
                    named_volumes.push(named);
                }
            }
        }
        for dep in parsed[i].depends_on.clone() {
            if !parsed.iter().any(|s| s.name == dep) {
                return Err(format!(
                    "service '{name}': depends on '{dep}', which is not one of the services in the spec"
                ));
            }
            if dep == name {
                return Err(format!("service '{name}': cannot depend on itself"));
            }
        }
        for net in parsed[i].networks.clone() {
            if !valid_name(&net) {
                return Err(format!("service '{name}': invalid network '{net}'"));
            }
            if !networks.contains(&net) {
                networks.push(net);
            }
        }
        // Merge in the file-wide settings, letting the service's own entries win.
        if parsed[i].networks.is_empty() && !network.is_empty() {
            parsed[i].networks = vec![network.to_string()];
        }
        if parsed[i].restart.is_empty() && !default_restart.is_empty() {
            parsed[i].restart = default_restart.to_string();
        }
        if !shared_env.is_empty() {
            let mut merged = shared_env.clone();
            for (k, v) in parsed[i].env.clone() {
                upsert(&mut merged, k, v);
            }
            parsed[i].env = merged;
        }
        for f in shared_env_file.iter().rev() {
            if !parsed[i].env_file.contains(f) {
                parsed[i].env_file.insert(0, f.clone());
            }
        }
    }

    named_volumes.sort();
    networks.sort();

    // ---- emit -------------------------------------------------------------
    let mut out = String::new();
    if !compose_version.is_empty() && compose_version != "none" {
        out.push_str(&format!("version: {}\n", quote(compose_version)));
    }
    if !project_name.is_empty() {
        out.push_str(&format!("name: {}\n", scalar(project_name)));
    }
    out.push_str("services:\n");
    for svc in &parsed {
        out.push_str(&format!("  {}:\n", svc.name));
        if !svc.image.is_empty() {
            out.push_str(&format!("    image: {}\n", scalar(&svc.image)));
        }
        if !svc.build.is_empty() {
            out.push_str(&format!("    build: {}\n", scalar(&svc.build)));
        }
        if !svc.container_name.is_empty() {
            out.push_str(&format!(
                "    container_name: {}\n",
                scalar(&svc.container_name)
            ));
        }
        if !svc.entrypoint.is_empty() {
            out.push_str(&format!("    entrypoint: {}\n", scalar(&svc.entrypoint)));
        }
        if !svc.command.is_empty() {
            out.push_str(&format!("    command: {}\n", scalar(&svc.command)));
        }
        if !svc.user.is_empty() {
            out.push_str(&format!("    user: {}\n", quote(&svc.user)));
        }
        if !svc.working_dir.is_empty() {
            out.push_str(&format!("    working_dir: {}\n", scalar(&svc.working_dir)));
        }
        if !svc.restart.is_empty() {
            out.push_str(&format!("    restart: {}\n", scalar(&svc.restart)));
        }
        if !svc.ports.is_empty() {
            out.push_str("    ports:\n");
            for p in &svc.ports {
                out.push_str(&format!("      - {}\n", quote(p)));
            }
        }
        if !svc.expose.is_empty() {
            out.push_str("    expose:\n");
            for p in &svc.expose {
                out.push_str(&format!("      - {}\n", quote(p)));
            }
        }
        if !svc.env.is_empty() {
            out.push_str("    environment:\n");
            for (k, v) in &svc.env {
                out.push_str(&format!("      {k}: {}\n", quote(v)));
            }
        }
        if !svc.env_file.is_empty() {
            out.push_str("    env_file:\n");
            for f in &svc.env_file {
                out.push_str(&format!("      - {}\n", scalar(f)));
            }
        }
        if !svc.volumes.is_empty() {
            out.push_str("    volumes:\n");
            for v in &svc.volumes {
                out.push_str(&format!("      - {}\n", scalar(v)));
            }
        }
        if !svc.networks.is_empty() {
            out.push_str("    networks:\n");
            for n in &svc.networks {
                out.push_str(&format!("      - {}\n", scalar(n)));
            }
        }
        if !svc.labels.is_empty() {
            out.push_str("    labels:\n");
            for (k, v) in &svc.labels {
                out.push_str(&format!("      {}: {}\n", scalar(k), quote(v)));
            }
        }
        if !svc.depends_on.is_empty() {
            out.push_str("    depends_on:\n");
            for d in &svc.depends_on {
                out.push_str(&format!("      - {}\n", scalar(d)));
            }
        }
        if !svc.healthcheck.is_empty() {
            out.push_str("    healthcheck:\n");
            out.push_str(&format!(
                "      test: [\"CMD-SHELL\", {}]\n",
                quote(&svc.healthcheck)
            ));
            out.push_str("      interval: 30s\n");
            out.push_str("      timeout: 10s\n");
            out.push_str("      retries: 3\n");
            out.push_str("      start_period: 10s\n");
        }
    }
    if !named_volumes.is_empty() {
        out.push_str("volumes:\n");
        for v in &named_volumes {
            out.push_str(&format!("  {v}:\n"));
        }
    }
    if !networks.is_empty() {
        out.push_str("networks:\n");
        for n in &networks {
            out.push_str(&format!("  {n}:\n"));
            if n == network {
                out.push_str(&format!("    driver: {driver}\n"));
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gen(spec: &str) -> String {
        generate(spec, "", "none", "", "bridge", "none", "", "").unwrap()
    }

    #[test]
    fn minimal_single_service() {
        assert_eq!(
            gen("web: nginx:alpine ports=8080:80"),
            "services:\n  web:\n    image: nginx:alpine\n    ports:\n      - \"8080:80\"\n"
        );
    }

    #[test]
    fn full_stack_wires_volumes_networks_and_depends() {
        let out = generate(
            "web: nginx:alpine ports=8080:80 depends=db\ndb: postgres:16-alpine volumes=dbdata:/var/lib/postgresql/data env=POSTGRES_PASSWORD=secret",
            "shop",
            "none",
            "appnet",
            "bridge",
            "unless-stopped",
            "",
            "",
        )
        .unwrap();
        assert!(out.starts_with("name: shop\nservices:\n"), "{out}");
        assert!(out.contains("    restart: unless-stopped\n"), "{out}");
        assert!(out.contains("      - dbdata:/var/lib/postgresql/data\n"), "{out}");
        assert!(out.contains("volumes:\n  dbdata:\n"), "{out}");
        assert!(out.contains("networks:\n  appnet:\n    driver: bridge\n"), "{out}");
        assert!(out.contains("    depends_on:\n      - db\n"), "{out}");
        // Env values are always quoted so YAML keeps them as strings.
        assert!(out.contains("      POSTGRES_PASSWORD: \"secret\"\n"), "{out}");
    }

    #[test]
    fn version_key_is_quoted_when_requested() {
        let out = generate("web: nginx", "", "3.8", "", "bridge", "none", "", "").unwrap();
        assert!(out.starts_with("version: \"3.8\"\n"), "{out}");
    }

    #[test]
    fn ambiguous_scalars_are_quoted() {
        let out = gen("web: nginx env=DEBUG=true,PORT=8080,GREETING=\"hello, world\"");
        assert!(out.contains("      DEBUG: \"true\"\n"), "{out}");
        assert!(out.contains("      PORT: \"8080\"\n"), "{out}");
        assert!(out.contains("      GREETING: \"hello, world\"\n"), "{out}");
    }

    #[test]
    fn quoted_command_keeps_its_spaces() {
        let out = gen("api: node:22 command=\"npm run start\" working_dir=/app user=1000:1000");
        assert!(out.contains("    command: npm run start\n"), "{out}");
        assert!(out.contains("    working_dir: /app\n"), "{out}");
        assert!(out.contains("    user: \"1000:1000\"\n"), "{out}");
    }

    #[test]
    fn healthcheck_emits_cmd_shell_block() {
        let out = gen("db: postgres:16 healthcheck=\"pg_isready -U postgres\"");
        assert!(
            out.contains("    healthcheck:\n      test: [\"CMD-SHELL\", \"pg_isready -U postgres\"]\n      interval: 30s\n"),
            "{out}"
        );
    }

    #[test]
    fn build_context_replaces_image() {
        let out = gen("app: build=./app ports=3000");
        assert!(out.contains("    build: ./app\n"), "{out}");
        assert!(!out.contains("image:"), "{out}");
    }

    #[test]
    fn shared_env_merges_under_service_env() {
        let out = generate(
            "a: alpine env=TZ=Europe/Berlin\nb: alpine",
            "",
            "none",
            "",
            "bridge",
            "none",
            "TZ=UTC\nLOG_LEVEL=info",
            ".env",
        )
        .unwrap();
        // Service a overrides the shared TZ; service b keeps it.
        assert!(out.contains("  a:\n    image: alpine\n    environment:\n      TZ: \"Europe/Berlin\"\n      LOG_LEVEL: \"info\"\n"), "{out}");
        assert!(out.contains("  b:\n    image: alpine\n    environment:\n      TZ: \"UTC\"\n"), "{out}");
        assert!(out.contains("    env_file:\n      - .env\n"), "{out}");
    }

    #[test]
    fn output_is_deterministic() {
        let spec = "web: nginx ports=80\ndb: redis volumes=cache:/data";
        assert_eq!(gen(spec), gen(spec));
    }

    #[test]
    fn comments_and_blank_lines_are_skipped() {
        let out = gen("# the front end\n\nweb: nginx\n");
        assert_eq!(out, "services:\n  web:\n    image: nginx\n");
    }

    // ---- error paths ------------------------------------------------------

    #[test]
    fn empty_spec_is_an_error() {
        let err = generate("  \n# nothing here\n", "", "none", "", "bridge", "none", "", "")
            .unwrap_err();
        assert!(err.contains("no services"), "{err}");
    }

    #[test]
    fn missing_image_is_an_error() {
        let err = generate("web:", "", "none", "", "bridge", "none", "", "").unwrap_err();
        assert!(err.contains("needs an image"), "{err}");
    }

    #[test]
    fn unknown_option_is_an_error() {
        let err = generate("web: nginx portz=80", "", "none", "", "bridge", "none", "", "")
            .unwrap_err();
        assert!(err.contains("unknown option 'portz=80'"), "{err}");
    }

    #[test]
    fn bad_port_is_an_error() {
        let err = generate("web: nginx ports=99999:80", "", "none", "", "bridge", "none", "", "")
            .unwrap_err();
        assert!(err.contains("host port must be 1-65535"), "{err}");
    }

    #[test]
    fn relative_container_path_is_an_error() {
        let err = generate("web: nginx volumes=./a:b", "", "none", "", "bridge", "none", "", "")
            .unwrap_err();
        assert!(err.contains("must be absolute"), "{err}");
    }

    #[test]
    fn unknown_dependency_is_an_error() {
        let err = generate("web: nginx depends=db", "", "none", "", "bridge", "none", "", "")
            .unwrap_err();
        assert!(err.contains("not one of the services"), "{err}");
    }

    #[test]
    fn duplicate_service_is_an_error() {
        let err = generate("web: nginx\nweb: httpd", "", "none", "", "bridge", "none", "", "")
            .unwrap_err();
        assert!(err.contains("duplicate service 'web'"), "{err}");
    }

    #[test]
    fn missing_colon_is_an_error() {
        let err = generate("web nginx", "", "none", "", "bridge", "none", "", "").unwrap_err();
        assert!(err.contains("expected"), "{err}");
    }

    #[test]
    fn bad_env_name_is_an_error() {
        let err = generate("web: nginx env=1BAD=x", "", "none", "", "bridge", "none", "", "")
            .unwrap_err();
        assert!(err.contains("invalid environment name"), "{err}");
    }

    #[test]
    fn service_cap_boundary() {
        let at_cap: String = (0..MAX_SERVICES)
            .map(|i| format!("s{i}: alpine\n"))
            .collect();
        assert!(generate(&at_cap, "", "none", "", "bridge", "none", "", "").is_ok());
        let over = format!("{at_cap}s25: alpine\n");
        let err = generate(&over, "", "none", "", "bridge", "none", "", "").unwrap_err();
        assert!(err.contains("too many services: 26 (maximum 25)"), "{err}");
    }

    #[test]
    fn bad_network_driver_is_an_error() {
        let err = generate("web: nginx", "", "none", "net", "wat", "none", "", "").unwrap_err();
        assert!(err.contains("unknown network_driver"), "{err}");
    }

    #[test]
    fn bad_restart_policy_is_an_error() {
        let err =
            generate("web: nginx restart=sometimes", "", "none", "", "bridge", "none", "", "")
                .unwrap_err();
        assert!(err.contains("unknown restart policy"), "{err}");
    }

    #[test]
    fn self_dependency_is_an_error() {
        let err = generate("web: nginx depends=web", "", "none", "", "bridge", "none", "", "")
            .unwrap_err();
        assert!(err.contains("cannot depend on itself"), "{err}");
    }

    #[test]
    fn unbalanced_quote_is_an_error() {
        let err = generate("web: nginx command=\"npm run", "", "none", "", "bridge", "none", "", "")
            .unwrap_err();
        assert!(err.contains("unbalanced double quote"), "{err}");
    }
}

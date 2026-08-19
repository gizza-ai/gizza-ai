//! rest-file-to-curl core — expand a `.http` / `.rest` / `.ain` request file
//! (with `{{variables}}` and an environment) into concrete `curl` commands.
//!
//! Pure compute, shared by the chat skill block and the web page. Nothing is
//! ever sent: the file is parsed, variables are substituted, and the result is
//! rendered as shell-quoted curl command text.

use chrono::DateTime;
use std::collections::BTreeMap;

/// Largest request file accepted, in bytes.
pub const MAX_INPUT_BYTES: usize = 1_000_000;
/// Depth limit for variables that reference other variables.
const MAX_EXPAND_DEPTH: usize = 8;

/// One request parsed out of the file, with every `{{variable}}` already expanded.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedRequest {
    pub name: Option<String>,
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
    /// Set when the body was a `< path` / `<@ path` file reference.
    pub body_file: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Unresolved {
    Keep,
    Empty,
    Error,
}

// ---------------------------------------------------------------------------
// variable context
// ---------------------------------------------------------------------------

struct Ctx {
    vars: BTreeMap<String, String>,
    now_ms: u64,
    rng: u64,
    mode: Unresolved,
    unresolved: Vec<String>,
}

impl Ctx {
    fn new(vars: BTreeMap<String, String>, mode: Unresolved, now_ms: u64) -> Self {
        let seed = now_ms
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407)
            | 1;
        Ctx {
            vars,
            now_ms,
            rng: seed,
            mode,
            unresolved: Vec::new(),
        }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng = x;
        x
    }

    fn miss(&mut self, name: &str) -> String {
        if !self.unresolved.iter().any(|n| n == name) {
            self.unresolved.push(name.to_string());
        }
        match self.mode {
            Unresolved::Empty => String::new(),
            // Error is reported by the caller once every request is parsed.
            Unresolved::Keep | Unresolved::Error => format!("{{{{{name}}}}}"),
        }
    }

    /// Expand every `{{ … }}` reference in `text`.
    fn expand(&mut self, text: &str, depth: usize) -> String {
        if depth > MAX_EXPAND_DEPTH || !text.contains("{{") {
            return text.to_string();
        }
        let bytes = text.as_bytes();
        let mut out = String::with_capacity(text.len());
        let mut i = 0usize;
        while i < text.len() {
            if bytes[i] == b'{' && i + 1 < text.len() && bytes[i + 1] == b'{' {
                if let Some(rel) = text[i + 2..].find("}}") {
                    let name = text[i + 2..i + 2 + rel].trim().to_string();
                    let value = self.resolve(&name, depth);
                    out.push_str(&value);
                    i = i + 2 + rel + 2;
                    continue;
                }
            }
            let ch = text[i..].chars().next().unwrap_or('\u{fffd}');
            out.push(ch);
            i += ch.len_utf8();
        }
        out
    }

    /// Expand `$VAR` / `${VAR}` shell-style references (used by `.ain` files).
    fn expand_dollar(&mut self, text: &str) -> String {
        if !text.contains('$') {
            return text.to_string();
        }
        let bytes = text.as_bytes();
        let mut out = String::with_capacity(text.len());
        let mut i = 0usize;
        while i < text.len() {
            if bytes[i] == b'$' && i + 1 < text.len() {
                let rest = &text[i + 1..];
                let (name, consumed) = if let Some(inner) = rest.strip_prefix('{') {
                    match inner.find('}') {
                        Some(end) => (inner[..end].trim().to_string(), end + 3),
                        None => (String::new(), 0),
                    }
                } else {
                    let end = rest
                        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                        .unwrap_or(rest.len());
                    (rest[..end].to_string(), end + 1)
                };
                if !name.is_empty() {
                    let value = match self.vars.get(&name).cloned() {
                        Some(v) => self.expand(&v, 1),
                        None => {
                            if !self.unresolved.iter().any(|n| n == &name) {
                                self.unresolved.push(name.clone());
                            }
                            match self.mode {
                                Unresolved::Empty => String::new(),
                                Unresolved::Keep | Unresolved::Error => format!("${name}"),
                            }
                        }
                    };
                    out.push_str(&value);
                    i += consumed;
                    continue;
                }
            }
            let ch = text[i..].chars().next().unwrap_or('\u{fffd}');
            out.push(ch);
            i += ch.len_utf8();
        }
        out
    }

    fn resolve(&mut self, name: &str, depth: usize) -> String {
        if let Some(spec) = name.strip_prefix('$') {
            return self.dynamic(spec.trim());
        }
        if let Some(v) = self.vars.get(name).cloned() {
            return self.expand(&v, depth + 1);
        }
        self.miss(name)
    }

    /// REST-Client-style system variables: `$guid`, `$timestamp`, `$datetime`,
    /// `$randomInt`, `$processEnv`, `$dotenv`.
    fn dynamic(&mut self, spec: &str) -> String {
        let tokens: Vec<&str> = spec.split_whitespace().collect();
        let kind = tokens.first().copied().unwrap_or("");
        let args = &tokens[tokens.len().min(1)..];
        let now_s = (self.now_ms / 1000) as i64;
        match kind {
            "guid" | "uuid" | "random.uuid" => self.uuid_v4(),
            "timestamp" => (now_s + parse_offset(args).unwrap_or(0)).to_string(),
            "isoTimestamp" => format_datetime(now_s, "iso8601"),
            "datetime" | "localDatetime" => {
                let fmt = args.first().copied().unwrap_or("iso8601");
                let off = parse_offset(&args[args.len().min(1)..]).unwrap_or(0);
                format_datetime(now_s + off, fmt)
            }
            "randomInt" => {
                let min = args.first().and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
                let max = args
                    .get(1)
                    .and_then(|s| s.parse::<i64>().ok())
                    .unwrap_or(1000);
                if max <= min {
                    return min.to_string();
                }
                let span = (max - min + 1) as u64;
                (min + (self.next_u64() % span) as i64).to_string()
            }
            "processEnv" | "dotenv" => {
                let key = args
                    .last()
                    .copied()
                    .unwrap_or("")
                    .trim_start_matches('%')
                    .to_string();
                match self.vars.get(&key).cloned() {
                    Some(v) => self.expand(&v, 1),
                    None => self.miss(&key),
                }
            }
            _ => self.miss(&format!("${kind}")),
        }
    }

    fn uuid_v4(&mut self) -> String {
        let a = self.next_u64();
        let b = self.next_u64();
        let mut bytes = [0u8; 16];
        bytes[..8].copy_from_slice(&a.to_be_bytes());
        bytes[8..].copy_from_slice(&b.to_be_bytes());
        bytes[6] = (bytes[6] & 0x0f) | 0x40;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        format!(
            "{}-{}-{}-{}-{}",
            &hex[0..8],
            &hex[8..12],
            &hex[12..16],
            &hex[16..20],
            &hex[20..32]
        )
    }
}

fn parse_offset(tokens: &[&str]) -> Option<i64> {
    if tokens.is_empty() {
        return None;
    }
    let (num, unit) = if tokens.len() >= 2 {
        (tokens[0].to_string(), tokens[1].to_string())
    } else {
        let t = tokens[0];
        let split = t.find(|c: char| c.is_ascii_alphabetic())?;
        (t[..split].to_string(), t[split..].to_string())
    };
    let n: i64 = num.parse().ok()?;
    let secs = match unit.as_str() {
        "ms" => return Some(n / 1000),
        "s" => 1,
        "m" => 60,
        "h" => 3_600,
        "d" => 86_400,
        "w" => 604_800,
        "M" => 2_592_000,
        "y" => 31_536_000,
        _ => return None,
    };
    Some(n * secs)
}

fn format_datetime(epoch_secs: i64, fmt: &str) -> String {
    let dt = match DateTime::from_timestamp(epoch_secs, 0) {
        Some(d) => d,
        None => return epoch_secs.to_string(),
    };
    match fmt {
        "iso8601" | "ISO8601" => dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        "rfc1123" | "RFC1123" => dt.to_rfc2822().replace("+0000", "GMT"),
        "unix" | "timestamp" => epoch_secs.to_string(),
        other => {
            // A custom strftime pattern. An invalid specifier makes chrono's
            // Display impl fail, which would PANIC inside `to_string()`, so the
            // pattern is validated first and falls back to ISO 8601.
            let items: Vec<_> = chrono::format::StrftimeItems::new(other.trim_matches('"')).collect();
            if items
                .iter()
                .any(|i| matches!(i, chrono::format::Item::Error))
            {
                return dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
            }
            dt.format_with_items(items.iter()).to_string()
        }
    }
}

// ---------------------------------------------------------------------------
// environment parsing
// ---------------------------------------------------------------------------

fn json_scalar(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn parse_env(env: &str, environment: &str) -> Result<BTreeMap<String, String>, String> {
    let mut out = BTreeMap::new();
    let trimmed = env.trim();
    if trimmed.is_empty() {
        return Ok(out);
    }
    if trimmed.starts_with('{') {
        let value: serde_json::Value = serde_json::from_str(trimmed)
            .map_err(|e| format!("env is not valid JSON: {e}. Use a JSON object or KEY=VALUE lines"))?;
        let map = value
            .as_object()
            .ok_or_else(|| "env JSON must be an object of variables".to_string())?;
        let nested: Vec<&String> = map
            .iter()
            .filter(|(_, v)| v.is_object())
            .map(|(k, _)| k)
            .collect();
        let wanted = environment.trim();
        if !wanted.is_empty() {
            let picked = map.get(wanted).ok_or_else(|| {
                let names: Vec<&str> = nested.iter().map(|s| s.as_str()).collect();
                if names.is_empty() {
                    format!("environment '{wanted}' is not defined in env")
                } else {
                    format!(
                        "environment '{wanted}' is not defined in env (available: {})",
                        names.join(", ")
                    )
                }
            })?;
            let obj = picked
                .as_object()
                .ok_or_else(|| format!("environment '{wanted}' must be a JSON object"))?;
            for (k, v) in obj {
                out.insert(k.clone(), json_scalar(v));
            }
            return Ok(out);
        }
        if nested.len() == 1 {
            let only = nested[0].clone();
            if let Some(obj) = map.get(&only).and_then(|v| v.as_object()) {
                for (k, v) in obj {
                    out.insert(k.clone(), json_scalar(v));
                }
            }
            return Ok(out);
        }
        if nested.len() > 1 {
            let names: Vec<&str> = nested.iter().map(|s| s.as_str()).collect();
            return Err(format!(
                "env defines named environments ({}) — set 'environment' to the one you want",
                names.join(", ")
            ));
        }
        for (k, v) in map {
            out.insert(k.clone(), json_scalar(v));
        }
        return Ok(out);
    }
    for line in trimmed.lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') || l.starts_with("//") {
            continue;
        }
        let l = l.strip_prefix("export ").unwrap_or(l);
        let (k, v) = match l.split_once('=') {
            Some(kv) => kv,
            None => continue,
        };
        let mut value = v.trim().to_string();
        if value.len() >= 2
            && ((value.starts_with('"') && value.ends_with('"'))
                || (value.starts_with('\'') && value.ends_with('\'')))
        {
            value = value[1..value.len() - 1].to_string();
        }
        out.insert(k.trim().trim_start_matches('@').to_string(), value);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// .http / .rest parsing
// ---------------------------------------------------------------------------

const METHODS: [&str; 16] = [
    "GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS", "TRACE", "CONNECT", "LOCK",
    "UNLOCK", "PROPFIND", "PROPPATCH", "COPY", "MOVE", "MKCOL",
];

fn is_method(token: &str) -> bool {
    let upper = token.to_ascii_uppercase();
    METHODS.contains(&upper.as_str())
}

/// Split a request line on whitespace, but treat a whole `{{ … }}` reference as
/// one token. System variables carry arguments separated by spaces
/// (`{{$timestamp -1 d}}`, `{{$datetime iso8601}}`), so a plain
/// `split_whitespace` would tear them apart before they can be expanded.
fn split_outside_braces(line: &str) -> Vec<String> {
    let mut tokens: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut depth = 0usize;
    let mut chars = line.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c == '{' && line[i..].starts_with("{{") {
            depth += 1;
            cur.push_str("{{");
            chars.next();
            continue;
        }
        if c == '}' && depth > 0 && line[i..].starts_with("}}") {
            depth -= 1;
            cur.push_str("}}");
            chars.next();
            continue;
        }
        if depth == 0 && c.is_whitespace() {
            if !cur.is_empty() {
                tokens.push(std::mem::take(&mut cur));
            }
            continue;
        }
        cur.push(c);
    }
    if !cur.is_empty() {
        tokens.push(cur);
    }
    tokens
}

fn parse_request_line(line: &str) -> Result<(String, String), String> {
    let mut parts = split_outside_braces(line);
    if parts.is_empty() {
        return Err("request line is empty".into());
    }
    if parts.len() > 1
        && parts
            .last()
            .map(|t| t.to_ascii_uppercase().starts_with("HTTP/"))
            .unwrap_or(false)
    {
        parts.pop();
    }
    let (method, url) = if parts.len() >= 2 && is_method(&parts[0]) {
        (parts[0].to_ascii_uppercase(), parts[1..].join(""))
    } else if parts.len() == 1 && is_method(&parts[0]) {
        return Err(format!("request '{}' is missing a URL", parts[0]));
    } else {
        ("GET".to_string(), parts.join(""))
    };
    if url.is_empty() {
        return Err("request is missing a URL".into());
    }
    Ok((method, url))
}

fn is_form_urlencoded(headers: &[(String, String)]) -> bool {
    headers.iter().any(|(k, v)| {
        k.eq_ignore_ascii_case("content-type")
            && v.to_ascii_lowercase()
                .contains("application/x-www-form-urlencoded")
    })
}

fn join_form_body(body: &str) -> String {
    let mut out = String::new();
    for line in body.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if out.is_empty() {
            out.push_str(t.trim_start_matches('&'));
        } else if let Some(rest) = t.strip_prefix('&') {
            out.push('&');
            out.push_str(rest);
        } else {
            out.push('&');
            out.push_str(t);
        }
    }
    out
}

/// Split the file into `###`-separated blocks; the separator's trailing text is
/// kept as a fallback request name.
fn split_blocks(src: &str) -> Vec<(Option<String>, Vec<&str>)> {
    let mut blocks: Vec<(Option<String>, Vec<&str>)> = vec![(None, Vec::new())];
    for line in src.lines() {
        let t = line.trim_start();
        if t.starts_with("###") {
            let title = t.trim_start_matches('#').trim();
            let title = if title.is_empty() {
                None
            } else {
                Some(title.to_string())
            };
            blocks.push((title, Vec::new()));
        } else {
            blocks.last_mut().unwrap().1.push(line);
        }
    }
    blocks
}

fn parse_http(src: &str, ctx: &mut Ctx) -> Result<Vec<ParsedRequest>, String> {
    let mut requests = Vec::new();
    for (title, lines) in split_blocks(src) {
        let mut name = None;
        let mut method = String::new();
        let mut url = String::new();
        let mut headers: Vec<(String, String)> = Vec::new();
        let mut body_lines: Vec<String> = Vec::new();
        let mut have_request = false;
        let mut in_body = false;

        for raw in lines {
            let line = raw.trim_end_matches('\r');
            let t = line.trim();
            if in_body {
                body_lines.push(line.to_string());
                continue;
            }
            if t.is_empty() {
                if have_request {
                    in_body = true;
                }
                continue;
            }
            if let Some(rest) = t.strip_prefix("//").or_else(|| t.strip_prefix('#')) {
                let directive = rest.trim();
                if let Some(n) = directive.strip_prefix("@name") {
                    let n = n.trim().trim_start_matches('=').trim();
                    if !n.is_empty() {
                        name = Some(n.to_string());
                    }
                }
                continue;
            }
            if !have_request && t.starts_with('@') && t[1..].contains('=') {
                let (k, v) = t[1..].split_once('=').unwrap();
                let value = ctx.expand(v.trim(), 0);
                ctx.vars.insert(k.trim().to_string(), value);
                continue;
            }
            if !have_request {
                let (m, u) = parse_request_line(t)?;
                method = m;
                url = ctx.expand(&u, 0);
                have_request = true;
                continue;
            }
            if headers.is_empty() && (t.starts_with('?') || t.starts_with('&')) {
                url.push_str(&ctx.expand(t, 0));
                continue;
            }
            match t.split_once(':') {
                Some((k, v)) if !k.trim().is_empty() => {
                    headers.push((ctx.expand(k.trim(), 0), ctx.expand(v.trim(), 0)));
                }
                _ => {
                    return Err(format!(
                        "could not parse '{t}' — expected a header line like 'Name: value', or a blank line before the body"
                    ))
                }
            }
        }

        if !have_request {
            continue;
        }

        while body_lines.first().map(|l| l.trim().is_empty()) == Some(true) {
            body_lines.remove(0);
        }
        while body_lines.last().map(|l| l.trim().is_empty()) == Some(true) {
            body_lines.pop();
        }
        let raw_body = body_lines.join("\n");

        let mut body = String::new();
        let mut body_file = None;
        if let Some(rest) = raw_body.trim_start().strip_prefix('<') {
            let path = rest
                .trim_start_matches('@')
                .split_whitespace()
                .last()
                .unwrap_or("")
                .to_string();
            if !path.is_empty() {
                body_file = Some(ctx.expand(&path, 0));
            }
        } else if !raw_body.is_empty() {
            let expanded = ctx.expand(&raw_body, 0);
            body = if is_form_urlencoded(&headers) {
                join_form_body(&expanded)
            } else {
                expanded
            };
        }

        requests.push(ParsedRequest {
            name: name.or(title),
            method,
            url,
            headers,
            body,
            body_file,
        });
    }
    Ok(requests)
}

// ---------------------------------------------------------------------------
// .ain parsing
// ---------------------------------------------------------------------------

const AIN_SECTIONS: [&str; 8] = [
    "host",
    "headers",
    "query",
    "body",
    "method",
    "backend",
    "backendoptions",
    "config",
];

fn looks_like_ain(src: &str) -> bool {
    src.lines().any(|l| {
        let t = l.trim().to_ascii_lowercase();
        t.starts_with('[')
            && t.ends_with(']')
            && AIN_SECTIONS.contains(&t.trim_matches(['[', ']'].as_ref()))
    })
}

fn parse_ain(src: &str, ctx: &mut Ctx) -> Result<Vec<ParsedRequest>, String> {
    let mut section = String::new();
    let mut host = String::new();
    let mut method = String::new();
    let mut query: Vec<String> = Vec::new();
    let mut headers: Vec<(String, String)> = Vec::new();
    let mut body_lines: Vec<String> = Vec::new();

    for raw in src.lines() {
        let t = raw.trim();
        if t.is_empty() && section != "body" {
            continue;
        }
        if t.starts_with('[') && t.ends_with(']') {
            section = t.trim_matches(['[', ']'].as_ref()).to_ascii_lowercase();
            continue;
        }
        if t.starts_with('#') && section != "body" {
            continue;
        }
        match section.as_str() {
            "host" => host.push_str(t),
            "method" => {
                if method.is_empty() {
                    method = t.to_ascii_uppercase();
                }
            }
            "query" => query.push(t.trim_start_matches('&').to_string()),
            "headers" => {
                if let Some((k, v)) = t.split_once(':') {
                    headers.push((k.trim().to_string(), v.trim().to_string()));
                }
            }
            "body" => body_lines.push(raw.trim_end_matches('\r').to_string()),
            _ => {}
        }
    }

    if host.trim().is_empty() {
        return Err("no [Host] section found — an .ain file needs a [Host] URL".into());
    }
    let mut url = expand_both(ctx, host.trim());
    if !query.is_empty() {
        let q: Vec<String> = query.iter().map(|p| expand_both(ctx, p)).collect();
        url.push(if url.contains('?') { '&' } else { '?' });
        url.push_str(&q.join("&"));
    }
    let headers: Vec<(String, String)> = headers
        .into_iter()
        .map(|(k, v)| (expand_both(ctx, &k), expand_both(ctx, &v)))
        .collect();

    while body_lines.first().map(|l| l.trim().is_empty()) == Some(true) {
        body_lines.remove(0);
    }
    while body_lines.last().map(|l| l.trim().is_empty()) == Some(true) {
        body_lines.pop();
    }
    let raw_body = body_lines.join("\n");
    let body = if raw_body.is_empty() {
        String::new()
    } else {
        let expanded = expand_both(ctx, &raw_body);
        if is_form_urlencoded(&headers) {
            join_form_body(&expanded)
        } else {
            expanded
        }
    };

    let method = if method.is_empty() {
        if body.is_empty() {
            "GET".to_string()
        } else {
            "POST".to_string()
        }
    } else {
        method
    };

    Ok(vec![ParsedRequest {
        name: None,
        method,
        url,
        headers,
        body,
        body_file: None,
    }])
}

fn expand_both(ctx: &mut Ctx, text: &str) -> String {
    let braced = ctx.expand(text, 0);
    ctx.expand_dollar(&braced)
}

// ---------------------------------------------------------------------------
// curl rendering
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shell {
    Bash,
    Cmd,
    PowerShell,
}

impl Shell {
    fn parse(s: &str) -> Result<Shell, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "bash" | "sh" | "zsh" => Ok(Shell::Bash),
            "cmd" | "batch" => Ok(Shell::Cmd),
            "powershell" | "pwsh" => Ok(Shell::PowerShell),
            other => Err(format!(
                "unknown shell '{other}' — use bash, cmd, or powershell"
            )),
        }
    }
    fn program(self) -> &'static str {
        match self {
            Shell::PowerShell => "curl.exe",
            _ => "curl",
        }
    }
    fn continuation(self) -> &'static str {
        match self {
            Shell::Bash => " \\\n  ",
            Shell::Cmd => " ^\n  ",
            Shell::PowerShell => " `\n  ",
        }
    }
    fn quote(self, s: &str) -> String {
        match self {
            Shell::Bash => format!("'{}'", s.replace('\'', "'\\''")),
            Shell::PowerShell => format!("'{}'", s.replace('\'', "''")),
            Shell::Cmd => format!("\"{}\"", s.replace('"', "\\\"")),
        }
    }
}

struct Flags {
    request: &'static str,
    header: &'static str,
    data: &'static str,
    location: &'static str,
    insecure: &'static str,
}

fn flags_for(style: &str) -> Result<Flags, String> {
    match style.trim().to_ascii_lowercase().as_str() {
        "" | "short" => Ok(Flags {
            request: "-X",
            header: "-H",
            data: "-d",
            location: "-L",
            insecure: "-k",
        }),
        "long" => Ok(Flags {
            request: "--request",
            header: "--header",
            data: "--data-raw",
            location: "--location",
            insecure: "--insecure",
        }),
        other => Err(format!(
            "unknown flag_style '{other}' — use short or long"
        )),
    }
}

#[allow(clippy::too_many_arguments)]
fn render(
    req: &ParsedRequest,
    shell: Shell,
    flags: &Flags,
    multiline: bool,
    follow_redirects: bool,
    compressed: bool,
    insecure: bool,
) -> String {
    let has_body = !req.body.is_empty() || req.body_file.is_some();
    // The first line always reads `curl [-X METHOD] URL` — that is what makes a
    // wrapped command scannable; only the flags after it get their own lines.
    let mut head = shell.program().to_string();
    if req.method != "GET" || has_body {
        head.push_str(&format!(" {} {}", flags.request, req.method));
    }
    head.push(' ');
    head.push_str(&shell.quote(&req.url));
    let mut parts: Vec<String> = vec![head];
    for (k, v) in &req.headers {
        parts.push(format!("{} {}", flags.header, shell.quote(&format!("{k}: {v}"))));
    }
    if let Some(path) = &req.body_file {
        parts.push(format!(
            "--data-binary {}",
            shell.quote(&format!("@{path}"))
        ));
    } else if !req.body.is_empty() {
        parts.push(format!("{} {}", flags.data, shell.quote(&req.body)));
    }
    if follow_redirects {
        parts.push(flags.location.to_string());
    }
    if compressed {
        parts.push("--compressed".to_string());
    }
    if insecure {
        parts.push(flags.insecure.to_string());
    }
    parts.join(if multiline { shell.continuation() } else { " " })
}

// ---------------------------------------------------------------------------
// entry point
// ---------------------------------------------------------------------------

/// Expand a `.http` / `.rest` / `.ain` request file into curl command text.
#[allow(clippy::too_many_arguments)]
pub fn convert(
    data: &str,
    env: &str,
    environment: &str,
    request: &str,
    format: &str,
    shell: &str,
    flag_style: &str,
    multiline: bool,
    unresolved: &str,
    follow_redirects: bool,
    compressed: bool,
    insecure: bool,
    include_comments: bool,
    now_ms: u64,
) -> Result<String, String> {
    if data.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "request file is {} bytes; the limit is {MAX_INPUT_BYTES} bytes",
            data.len()
        ));
    }
    if data.trim().is_empty() {
        return Err("no request file provided — paste the contents of a .http, .rest, or .ain file"
            .into());
    }
    let mode = match unresolved.trim().to_ascii_lowercase().as_str() {
        "" | "keep" => Unresolved::Keep,
        "empty" => Unresolved::Empty,
        "error" => Unresolved::Error,
        other => {
            return Err(format!(
                "unknown unresolved '{other}' — use keep, empty, or error"
            ))
        }
    };
    let shell = Shell::parse(shell)?;
    let flags = flags_for(flag_style)?;
    let vars = parse_env(env, environment)?;
    let mut ctx = Ctx::new(vars, mode, now_ms);

    let format_norm = format.trim().to_ascii_lowercase();
    let requests = match format_norm.as_str() {
        "" | "auto" => {
            if looks_like_ain(data) {
                parse_ain(data, &mut ctx)?
            } else {
                parse_http(data, &mut ctx)?
            }
        }
        "http" | "rest" => parse_http(data, &mut ctx)?,
        "ain" => parse_ain(data, &mut ctx)?,
        other => {
            return Err(format!(
                "unknown format '{other}' — use auto, http, or ain"
            ))
        }
    };

    if requests.is_empty() {
        return Err(
            "no requests found — a request needs a line like 'GET https://example.com'".into(),
        );
    }

    if mode == Unresolved::Error && !ctx.unresolved.is_empty() {
        return Err(format!(
            "unresolved variable(s): {} — define them in env, or set unresolved to keep or empty",
            ctx.unresolved.join(", ")
        ));
    }

    let selector = request.trim();
    let selected: Vec<(usize, &ParsedRequest)> = if selector.is_empty()
        || selector.eq_ignore_ascii_case("all")
    {
        requests.iter().enumerate().collect()
    } else if let Ok(n) = selector.parse::<usize>() {
        if n == 0 || n > requests.len() {
            return Err(format!(
                "request {n} is out of range — the file has {} request(s)",
                requests.len()
            ));
        }
        vec![(n - 1, &requests[n - 1])]
    } else {
        let hit = requests.iter().enumerate().find(|(_, r)| {
            r.name
                .as_deref()
                .map(|n| n.eq_ignore_ascii_case(selector))
                .unwrap_or(false)
        });
        match hit {
            Some(pair) => vec![pair],
            None => {
                let names: Vec<String> = requests
                    .iter()
                    .enumerate()
                    .map(|(i, r)| match &r.name {
                        Some(n) => n.clone(),
                        None => format!("{}", i + 1),
                    })
                    .collect();
                return Err(format!(
                    "no request named '{selector}' — available: {}",
                    names.join(", ")
                ));
            }
        }
    };

    let label_all = include_comments && (requests.len() > 1 || selected[0].1.name.is_some());
    let mut out = String::new();
    for (idx, req) in selected {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        if label_all {
            match &req.name {
                Some(n) => out.push_str(&format!("# {n}\n")),
                None => out.push_str(&format!("# request {}\n", idx + 1)),
            }
        }
        out.push_str(&render(
            req,
            shell,
            &flags,
            multiline,
            follow_redirects,
            compressed,
            insecure,
        ));
    }
    out.push('\n');
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_760_000_000_000; // 2025-10-09T08:53:20Z

    #[allow(clippy::too_many_arguments)]
    fn conv(data: &str, env: &str, environment: &str, request: &str) -> Result<String, String> {
        convert(
            data,
            env,
            environment,
            request,
            "auto",
            "bash",
            "short",
            true,
            "keep",
            false,
            false,
            false,
            true,
            NOW,
        )
    }

    #[test]
    fn simple_get_with_file_variable() {
        let src = "@host = https://api.example.com\n\nGET {{host}}/users HTTP/1.1\nAccept: application/json\n";
        assert_eq!(
            conv(src, "", "", "all").unwrap(),
            "curl 'https://api.example.com/users' \\\n  -H 'Accept: application/json'\n"
        );
    }

    #[test]
    fn post_json_body_and_env_variables() {
        let src = "POST {{host}}/items\nContent-Type: application/json\nAuthorization: Bearer {{token}}\n\n{\"name\":\"gizza\"}\n";
        let out = conv(src, "{\"host\":\"https://api.example.com\",\"token\":\"t0k\"}", "", "all")
            .unwrap();
        assert_eq!(
            out,
            "curl -X POST 'https://api.example.com/items' \\\n  -H 'Content-Type: application/json' \\\n  -H 'Authorization: Bearer t0k' \\\n  -d '{\"name\":\"gizza\"}'\n"
        );
    }

    #[test]
    fn named_environments_need_a_selection() {
        let env = r#"{"dev":{"host":"http://localhost:3000"},"prod":{"host":"https://api.example.com"}}"#;
        let src = "GET {{host}}/ping\n";
        let err = conv(src, env, "", "all").unwrap_err();
        assert!(err.contains("named environments"), "{err}");
        let out = conv(src, env, "prod", "all").unwrap();
        assert_eq!(out, "curl 'https://api.example.com/ping'\n");
    }

    #[test]
    fn dotenv_style_env_lines() {
        let src = "GET {{HOST}}/health\n";
        let out = conv(src, "# comment\nexport HOST=\"https://example.org\"\n", "", "all").unwrap();
        assert_eq!(out, "curl 'https://example.org/health'\n");
    }

    #[test]
    fn multiple_requests_are_labelled_and_selectable() {
        let src = "### login\n# @name login\nPOST https://api.example.com/login\n\nsecret\n\n### list\nGET https://api.example.com/items\n";
        let all = conv(src, "", "", "all").unwrap();
        assert!(all.starts_with("# login\n"), "{all}");
        assert!(all.contains("# list\n"), "{all}");
        let one = conv(src, "", "", "list").unwrap();
        assert_eq!(one, "# list\ncurl 'https://api.example.com/items'\n");
        let by_index = conv(src, "", "", "2").unwrap();
        assert_eq!(by_index, one);
    }

    #[test]
    fn query_continuation_and_form_body() {
        let src = "POST https://api.example.com/search\n  ?q=rust\n  &page=2\nContent-Type: application/x-www-form-urlencoded\n\nname=foo\n&password=bar\n";
        let out = conv(src, "", "", "all").unwrap();
        assert_eq!(out, "curl -X POST 'https://api.example.com/search?q=rust&page=2' \\\n  -H 'Content-Type: application/x-www-form-urlencoded' \\\n  -d 'name=foo&password=bar'\n");
    }

    #[test]
    fn body_file_reference_becomes_data_binary() {
        let src = "POST https://api.example.com/upload\nContent-Type: application/json\n\n< ./payload.json\n";
        let out = conv(src, "", "", "all").unwrap();
        assert!(out.contains("--data-binary '@./payload.json'"), "{out}");
    }

    #[test]
    fn ain_sections_are_supported() {
        let src = "[Method]\nPOST\n\n[Host]\nhttps://api.example.com/orders\n\n[Query]\nlimit=10\n\n[Headers]\nAuthorization: Bearer $TOKEN\n\n[Body]\n{\"ok\":true}\n";
        let out = conv(src, "TOKEN=abc123", "", "all").unwrap();
        assert_eq!(out, "curl -X POST 'https://api.example.com/orders?limit=10' \\\n  -H 'Authorization: Bearer abc123' \\\n  -d '{\"ok\":true}'\n");
    }

    #[test]
    fn dynamic_variables_use_the_supplied_clock() {
        let src = "GET https://api.example.com/t?ts={{$timestamp}}&iso={{$datetime iso8601}}\n";
        let out = conv(src, "", "", "all").unwrap();
        assert_eq!(
            out,
            "curl 'https://api.example.com/t?ts=1760000000&iso=2025-10-09T08:53:20Z'\n"
        );
    }

    #[test]
    fn dynamic_offsets_and_uuid() {
        let src = "GET https://x.test/?a={{$timestamp -1 d}}&b={{$guid}}\n";
        let out = conv(src, "", "", "all").unwrap();
        assert!(out.contains("a=1759913600"), "{out}");
        let uuid_start = out.find("b=").unwrap() + 2;
        let uuid = &out[uuid_start..uuid_start + 36];
        assert_eq!(uuid.chars().filter(|c| *c == '-').count(), 4, "{uuid}");
        assert_eq!(&uuid[14..15], "4", "{uuid}");
    }

    #[test]
    fn shells_and_flag_styles_change_quoting() {
        let src = "POST https://api.example.com/x\n\nit's json\n";
        let cmd = convert(
            src, "", "", "all", "http", "cmd", "long", false, "keep", true, true, true, false, NOW,
        )
        .unwrap();
        assert_eq!(
            cmd,
            "curl --request POST \"https://api.example.com/x\" --data-raw \"it's json\" --location --compressed --insecure\n"
        );
        let ps = convert(
            src, "", "", "all", "http", "powershell", "short", false, "keep", false, false, false,
            false, NOW,
        )
        .unwrap();
        assert_eq!(
            ps,
            "curl.exe -X POST 'https://api.example.com/x' -d 'it''s json'\n"
        );
    }

    #[test]
    fn unresolved_variables_can_error_or_blank() {
        let src = "GET {{host}}/ping\n";
        let kept = conv(src, "", "", "all").unwrap();
        assert_eq!(kept, "curl '{{host}}/ping'\n");
        let err = convert(
            src, "", "", "all", "auto", "bash", "short", true, "error", false, false, false, true,
            NOW,
        )
        .unwrap_err();
        assert!(err.contains("unresolved variable(s): host"), "{err}");
        let blank = convert(
            src, "", "", "all", "auto", "bash", "short", true, "empty", false, false, false, true,
            NOW,
        )
        .unwrap();
        assert_eq!(blank, "curl '/ping'\n");
    }

    #[test]
    fn errors_on_empty_input_bad_selector_and_bad_header() {
        assert!(conv("   ", "", "", "all")
            .unwrap_err()
            .contains("no request file provided"));
        assert!(conv("GET https://x.test/\n", "", "", "nope")
            .unwrap_err()
            .contains("no request named 'nope'"));
        assert!(conv("GET https://x.test/\n", "", "", "9")
            .unwrap_err()
            .contains("out of range"));
        assert!(conv("GET https://x.test/\nnot a header\n", "", "", "all")
            .unwrap_err()
            .contains("expected a header line"));
        assert!(conv("GET https://x.test/\n", "{bad json", "", "all")
            .unwrap_err()
            .contains("not valid JSON"));
    }

    #[test]
    fn oversized_input_is_rejected() {
        let big = "x".repeat(MAX_INPUT_BYTES + 1);
        assert!(conv(&big, "", "", "all").unwrap_err().contains("limit is"));
    }

    #[test]
    fn nested_variables_expand_recursively() {
        let src = "@base = {{scheme}}://{{domain}}\nGET {{base}}/v1\n";
        let out = conv(src, "scheme=https\ndomain=api.example.com", "", "all").unwrap();
        assert_eq!(out, "curl 'https://api.example.com/v1'\n");
    }

    #[test]
    fn custom_datetime_formats_are_safe() {
        let src = "GET https://x.test/?d={{$datetime \"%Y-%m-%d\"}}&bad={{$datetime %Q}}\n";
        let out = conv(src, "", "", "all").unwrap();
        // A valid strftime pattern is honoured; an invalid one falls back to
        // ISO 8601 instead of panicking inside chrono's Display impl.
        assert!(out.contains("d=2025-10-09"), "{out}");
        assert!(out.contains("bad=2025-10-09T08:53:20Z"), "{out}");
    }

    #[test]
    fn comments_can_be_switched_off() {
        let src = "# @name ping\nGET https://x.test/ping\n";
        let with = conv(src, "", "", "all").unwrap();
        assert_eq!(with, "# ping\ncurl 'https://x.test/ping'\n");
        let without = convert(
            src, "", "", "all", "auto", "bash", "short", true, "keep", false, false, false, false,
            NOW,
        )
        .unwrap();
        assert_eq!(without, "curl 'https://x.test/ping'\n");
    }
}

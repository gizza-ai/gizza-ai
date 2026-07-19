//! env-to-configmap core — turn a `.env` file into a Kubernetes ConfigMap or
//! Secret manifest (YAML). Pure compute, no deps: a small `.env` parser, a
//! YAML string-scalar emitter that quotes anything ambiguous, and a standard
//! base64 encoder for Secret `data`. Shared by the chat skill block and the web
//! page.

/// Which manifest to emit.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    ConfigMap,
    Secret,
}

/// How Secret values are carried: base64 `data` (canonical) or plaintext
/// `stringData` (Kubernetes base64-encodes it on apply).
#[derive(Clone, Copy, PartialEq, Eq)]
enum SecretEncoding {
    Data,
    StringData,
}

/// Convert a `.env` document into a Kubernetes ConfigMap or Secret manifest.
///
/// - `env`: the `.env` file contents (`KEY=value` lines; `#` comments, blank
///   lines and a leading `export ` are ignored; quoted values are unquoted).
/// - `kind`: `"configmap"` (default) or `"secret"`.
/// - `name`: `metadata.name` (default `"app-config"`).
/// - `namespace`: `metadata.namespace`; omitted when blank.
/// - `secret_encoding`: for Secrets, `"data"` (base64, default) or
///   `"stringData"` (plaintext). Ignored for a ConfigMap.
/// - `labels`: optional comma-separated `key=value` pairs for `metadata.labels`.
///
/// Returns `Err` on an unknown `kind`/`secret_encoding`, an invalid data key or
/// label, or an env document with no `KEY=value` entries.
pub fn to_manifest(
    env: &str,
    kind: &str,
    name: &str,
    namespace: &str,
    secret_encoding: &str,
    labels: &str,
) -> Result<String, String> {
    let kind = match kind.trim() {
        "" | "configmap" | "ConfigMap" => Kind::ConfigMap,
        "secret" | "Secret" => Kind::Secret,
        other => return Err(format!("invalid kind {other:?}: expected \"configmap\" or \"secret\"")),
    };
    let encoding = match secret_encoding.trim() {
        "" | "data" => SecretEncoding::Data,
        "stringData" | "stringdata" | "string_data" => SecretEncoding::StringData,
        other => {
            return Err(format!(
                "invalid secret_encoding {other:?}: expected \"data\" or \"stringData\""
            ))
        }
    };

    let name = {
        let n = name.trim();
        if n.is_empty() { "app-config" } else { n }
    };
    if !is_valid_name(name) {
        return Err(format!(
            "invalid name {name:?}: use lowercase letters, digits, '-' and '.', starting and ending with an alphanumeric (RFC 1123)"
        ));
    }
    let namespace = namespace.trim();
    if !namespace.is_empty() && !is_valid_name(namespace) {
        return Err(format!(
            "invalid namespace {namespace:?}: use lowercase letters, digits and '-' (RFC 1123)"
        ));
    }

    let entries = parse_env(env)?;
    if entries.is_empty() {
        return Err("no KEY=value pairs found in the .env input".to_string());
    }
    let labels = parse_labels(labels)?;

    let mut out = String::new();
    out.push_str("apiVersion: v1\n");
    match kind {
        Kind::ConfigMap => out.push_str("kind: ConfigMap\n"),
        Kind::Secret => out.push_str("kind: Secret\n"),
    }
    out.push_str("metadata:\n");
    out.push_str(&format!("  name: {}\n", yaml_scalar(name)));
    if !namespace.is_empty() {
        out.push_str(&format!("  namespace: {}\n", yaml_scalar(namespace)));
    }
    if !labels.is_empty() {
        out.push_str("  labels:\n");
        for (k, v) in &labels {
            out.push_str(&format!("    {}: {}\n", yaml_scalar(k), yaml_scalar(v)));
        }
    }

    match kind {
        Kind::ConfigMap => {
            out.push_str("data:\n");
            for (k, v) in &entries {
                out.push_str(&format!("  {}: {}\n", yaml_scalar(k), yaml_scalar(v)));
            }
        }
        Kind::Secret => {
            out.push_str("type: Opaque\n");
            match encoding {
                SecretEncoding::Data => {
                    out.push_str("data:\n");
                    for (k, v) in &entries {
                        let b64 = base64_encode(v.as_bytes());
                        out.push_str(&format!("  {}: {}\n", yaml_scalar(k), yaml_scalar(&b64)));
                    }
                }
                SecretEncoding::StringData => {
                    out.push_str("stringData:\n");
                    for (k, v) in &entries {
                        out.push_str(&format!("  {}: {}\n", yaml_scalar(k), yaml_scalar(v)));
                    }
                }
            }
        }
    }

    Ok(out)
}

/// Parse a `.env` document into ordered `(key, value)` pairs. Blank and
/// `#`-comment lines are skipped; a leading `export ` is dropped; the value is
/// unquoted (single- or double-quoted, with `\n`/`\t`/`\\`/`\"` unescaped inside
/// double quotes) and an unquoted trailing ` # comment` removed. Duplicate keys
/// keep first-seen position with the last value winning.
fn parse_env(env: &str) -> Result<Vec<(String, String)>, String> {
    let mut entries: Vec<(String, String)> = Vec::new();
    for raw in env.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").map(str::trim).unwrap_or(line);
        let eq = match line.find('=') {
            Some(i) => i,
            None => continue, // not a KEY=value assignment — skip
        };
        let key = line[..eq].trim().to_string();
        if !is_valid_key(&key) {
            return Err(format!(
                "invalid key {key:?}: keys may contain only letters, digits, '-', '_' and '.'"
            ));
        }
        let value = unquote_value(line[eq + 1..].trim());
        if let Some(slot) = entries.iter_mut().find(|(k, _)| *k == key) {
            slot.1 = value;
        } else {
            entries.push((key, value));
        }
    }
    Ok(entries)
}

/// Strip surrounding matching quotes and interpret escapes for a `.env` value.
fn unquote_value(raw: &str) -> String {
    let bytes = raw.as_bytes();
    if bytes.len() >= 2 && bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"' {
        let inner = &raw[1..raw.len() - 1];
        let mut out = String::with_capacity(inner.len());
        let mut chars = inner.chars();
        while let Some(c) = chars.next() {
            if c == '\\' {
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
            } else {
                out.push(c);
            }
        }
        return out;
    }
    if bytes.len() >= 2 && bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\'' {
        return raw[1..raw.len() - 1].to_string();
    }
    // Unquoted: drop a trailing ` # inline comment` (a `#` preceded by whitespace).
    if let Some(idx) = find_inline_comment(raw) {
        return raw[..idx].trim_end().to_string();
    }
    raw.to_string()
}

/// Byte index of an unquoted inline comment marker (` #`), or None.
fn find_inline_comment(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    for i in 1..bytes.len() {
        if bytes[i] == b'#' && (bytes[i - 1] == b' ' || bytes[i - 1] == b'\t') {
            return Some(i);
        }
    }
    None
}

/// Parse `key=value,key2=value2` label pairs (blank entries skipped).
fn parse_labels(labels: &str) -> Result<Vec<(String, String)>, String> {
    let mut out: Vec<(String, String)> = Vec::new();
    for tok in labels.split(',') {
        let tok = tok.trim();
        if tok.is_empty() {
            continue;
        }
        let eq = tok
            .find('=')
            .ok_or_else(|| format!("invalid label {tok:?}: expected key=value"))?;
        let k = tok[..eq].trim().to_string();
        let v = tok[eq + 1..].trim().to_string();
        if !is_valid_key(&k) {
            return Err(format!(
                "invalid label key {k:?}: use letters, digits, '-', '_' and '.'"
            ));
        }
        out.push((k, v));
    }
    Ok(out)
}

/// A valid ConfigMap/Secret data key: non-empty, `[-._a-zA-Z0-9]` only.
fn is_valid_key(k: &str) -> bool {
    !k.is_empty()
        && k.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

/// A valid RFC 1123 object name (lenient): non-empty, lowercase alnum plus
/// `-`/`.`, starting and ending with an alphanumeric.
fn is_valid_name(n: &str) -> bool {
    if n.is_empty() || n.len() > 253 {
        return false;
    }
    let first = n.chars().next().unwrap();
    let last = n.chars().last().unwrap();
    if !first.is_ascii_alphanumeric() || !last.is_ascii_alphanumeric() {
        return false;
    }
    n.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.')
}

/// Render a string as a YAML scalar, double-quoting (with escaping) whenever a
/// plain scalar would be ambiguous — empty, whitespace-edged, YAML
/// bool/null-looking, numeric-looking, or containing an indicator/special char.
/// Over-quoting is always safe; a quoted string is the same string.
fn yaml_scalar(s: &str) -> String {
    if is_plain_safe(s) {
        s.to_string()
    } else {
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
}

/// True when `s` can be written as a bare YAML scalar with no chance of being
/// reinterpreted as a number, boolean, null, or structural token.
fn is_plain_safe(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let first = s.chars().next().unwrap();
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    if !s
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
    {
        return false;
    }
    // YAML 1.1 booleans / null spellings that must be quoted to stay strings.
    !matches!(
        s.to_ascii_lowercase().as_str(),
        "true" | "false" | "yes" | "no" | "on" | "off" | "y" | "n" | "null" | "none" | "nan"
    )
}

/// Standard (RFC 4648) base64 encoding with `=` padding.
fn base64_encode(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[((n >> 18) & 0x3f) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[((n >> 6) & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[(n & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configmap_happy_path() {
        let env = "# db config\nexport DB_HOST=localhost\nDB_PORT=5432\n";
        let out = to_manifest(env, "configmap", "app-config", "", "data", "").unwrap();
        let expected = "apiVersion: v1\nkind: ConfigMap\nmetadata:\n  name: app-config\ndata:\n  DB_HOST: localhost\n  DB_PORT: \"5432\"\n";
        assert_eq!(out, expected);
    }

    #[test]
    fn secret_base64_data() {
        let env = "API_TOKEN=s3cr3t\n";
        let out = to_manifest(env, "secret", "api", "prod", "data", "").unwrap();
        // base64("s3cr3t") = "czNjcjN0"
        let expected = "apiVersion: v1\nkind: Secret\nmetadata:\n  name: api\n  namespace: prod\ntype: Opaque\ndata:\n  API_TOKEN: czNjcjN0\n";
        assert_eq!(out, expected);
    }

    #[test]
    fn secret_string_data_plaintext() {
        let env = "PASSWORD=hunter2\n";
        let out = to_manifest(env, "secret", "db", "", "stringData", "").unwrap();
        assert!(out.contains("stringData:\n  PASSWORD: hunter2\n"), "{out}");
        assert!(out.contains("type: Opaque\n"));
    }

    #[test]
    fn quoting_and_types() {
        // numeric, boolean, empty, spaces, colon and leading '/'.
        let env = "PORT=8080\nDEBUG=true\nEMPTY=\nGREETING=hello world\nURL=https://x/y\nRATIO=1:2\n";
        let out = to_manifest(env, "configmap", "cfg", "", "data", "").unwrap();
        assert!(out.contains("  PORT: \"8080\"\n"), "{out}");
        assert!(out.contains("  DEBUG: \"true\"\n"), "{out}");
        assert!(out.contains("  EMPTY: \"\"\n"), "{out}");
        assert!(out.contains("  GREETING: \"hello world\"\n"), "{out}");
        assert!(out.contains("  URL: \"https://x/y\"\n"), "{out}");
        assert!(out.contains("  RATIO: \"1:2\"\n"), "{out}");
    }

    #[test]
    fn quoted_value_with_escapes() {
        let env = "MSG=\"line1\\nline2\"\n";
        let out = to_manifest(env, "configmap", "cfg", "", "data", "").unwrap();
        assert!(out.contains("  MSG: \"line1\\nline2\"\n"), "{out}");
    }

    #[test]
    fn inline_comment_and_dup_last_wins() {
        let env = "HOST=example.com # primary\nHOST=127.0.0.1\n";
        let out = to_manifest(env, "configmap", "cfg", "", "data", "").unwrap();
        // 127.0.0.1 starts with a digit → quoted to stay a string.
        assert!(out.contains("  HOST: \"127.0.0.1\"\n"), "{out}");
        assert!(!out.contains("primary"), "{out}");
    }

    #[test]
    fn labels_rendered() {
        let env = "K=v\n";
        let out = to_manifest(env, "configmap", "cfg", "", "data", "app=web, tier=backend").unwrap();
        assert!(out.contains("  labels:\n    app: web\n    tier: backend\n"), "{out}");
    }

    #[test]
    fn error_empty_env() {
        let err = to_manifest("# only comments\n\n", "configmap", "cfg", "", "data", "").unwrap_err();
        assert!(err.contains("no KEY=value"), "{err}");
    }

    #[test]
    fn error_bad_kind() {
        let err = to_manifest("A=b\n", "daemonset", "cfg", "", "data", "").unwrap_err();
        assert!(err.contains("invalid kind"), "{err}");
    }

    #[test]
    fn error_bad_key() {
        let err = to_manifest("BAD KEY=1\n", "configmap", "cfg", "", "data", "").unwrap_err();
        assert!(err.contains("invalid key"), "{err}");
    }

    #[test]
    fn error_bad_name() {
        let err = to_manifest("A=b\n", "configmap", "App_Config", "", "data", "").unwrap_err();
        assert!(err.contains("invalid name"), "{err}");
    }

    #[test]
    fn base64_known_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }
}

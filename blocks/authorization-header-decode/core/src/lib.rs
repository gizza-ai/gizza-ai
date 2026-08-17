//! gizza-ai/authorization-header-decode core — pure compute, shared by the chat skill
//! block and the web page. No wafer/wasm-bindgen deps.
//!
//! Takes an HTTP `Authorization` header (with or without the `Authorization:` name) and
//! splits it into its RFC 7235 parts: the auth-scheme and the credentials that follow.
//! `Basic` credentials are base64-decoded into username/password (RFC 7617), `Bearer`
//! tokens are described STRUCTURALLY (JWT vs opaque, segment shape, JOSE header) without
//! decoding payload claims — claim inspection belongs to a dedicated JWT decoder —
//! `Digest`/`AWS4-HMAC-SHA256`/custom schemes have their auth-params parsed into a map,
//! and `Negotiate`/`NTLM` blobs are identified from their binary signature.

use base64::{
    engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD},
    Engine as _,
};
use serde_json::{json, Map, Value};

/// Longest accepted input. Real headers are far shorter; SPNEGO blobs are the biggest
/// realistic case and still land well under this.
pub const MAX_INPUT_CHARS: usize = 8192;

const MASK: &str = "********";

const BASE64_NOT_ENCRYPTION: &str =
    "base64 is an encoding, not encryption: a Basic credential is trivially reversible, so treat the decoded password as if it were sent in cleartext.";

/// (lowercase match, canonical spelling, credential kind, registered?, description)
const SCHEMES: &[(&str, &str, Kind, bool, &str)] = &[
    ("basic", "Basic", Kind::Token68, true, "RFC 7617 Basic authentication: base64 of username:password."),
    ("bearer", "Bearer", Kind::Token68, true, "RFC 6750 Bearer token, as issued by OAuth 2.0 and most modern APIs."),
    ("digest", "Digest", Kind::Params, true, "RFC 7616 Digest access authentication: a challenge-response hash, sent as auth-params."),
    ("negotiate", "Negotiate", Kind::Token68, true, "RFC 4559 SPNEGO: a base64 GSS-API blob wrapping Kerberos or NTLM."),
    ("ntlm", "NTLM", Kind::Token68, false, "Microsoft NTLM: a base64 NTLMSSP message (type 1 negotiate, 2 challenge, 3 authenticate)."),
    ("aws4-hmac-sha256", "AWS4-HMAC-SHA256", Kind::Params, false, "AWS Signature Version 4: a credential scope, the signed header list, and a signature."),
    ("hoba", "HOBA", Kind::Token68, true, "RFC 7486 HTTP Origin-Bound Authentication (digital-signature based)."),
    ("mutual", "Mutual", Kind::Params, true, "RFC 8120 Mutual authentication (password-based, mutually authenticating)."),
    ("scram-sha-1", "SCRAM-SHA-1", Kind::Params, true, "RFC 7804 SCRAM over HTTP using SHA-1."),
    ("scram-sha-256", "SCRAM-SHA-256", Kind::Params, true, "RFC 7804 SCRAM over HTTP using SHA-256."),
    ("vapid", "vapid", Kind::Params, true, "RFC 8292 VAPID: a signed JWT plus the sender's public key, for Web Push."),
    ("dpop", "DPoP", Kind::Token68, true, "RFC 9449 DPoP: a sender-constrained access token bound to a proof JWT."),
    ("oauth", "OAuth", Kind::Params, true, "OAuth 1.0 signed request parameters (largely superseded by Bearer)."),
    ("hawk", "Hawk", Kind::Params, false, "Hawk: a MAC-based scheme sending id, ts, nonce and mac as auth-params."),
    ("signature", "Signature", Kind::Params, false, "HTTP Message Signatures / draft-cavage: keyId, algorithm, headers and signature as auth-params."),
    ("token", "Token", Kind::Token68, false, "Non-standard: an API token, used like Bearer by some services."),
    ("apikey", "ApiKey", Kind::Token68, false, "Non-standard: a raw API key, used like Bearer by some services."),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Token68,
    Params,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Kind::Token68 => "token68",
            Kind::Params => "auth-params",
        }
    }
}

struct Analysis {
    header_name: Option<String>,
    scheme: Option<String>,
    scheme_canonical: Option<String>,
    scheme_known: bool,
    scheme_registered: bool,
    scheme_description: String,
    credentials: String,
    kind: Kind,
    basic: Option<Basic>,
    bearer: Option<Bearer>,
    params: Vec<(String, String)>,
    scope: Vec<(String, String)>,
    blob: Option<Blob>,
    warnings: Vec<String>,
}

struct Basic {
    username: String,
    password: String,
    has_separator: bool,
    valid_utf8: bool,
    decoded_bytes: usize,
}

struct Bearer {
    token: String,
    token_type: &'static str,
    charset: &'static str,
    segments: Vec<usize>,
    jose_header: Option<Value>,
}

struct Blob {
    bytes: usize,
    content: String,
}

/// Decode with defaults (JSON output, nothing masked, warnings stay warnings).
pub fn run(header: &str) -> Result<String, String> {
    run_with_options(header, "json", false, false)
}

/// Decode an Authorization header.
///
/// * `format` — `json` | `text` | `table`
/// * `mask_credentials` — replace the secret parts (password, bearer token, digest
///   response, signature, binary blob) with asterisks while keeping their lengths
/// * `strict` — turn every warning into an error
pub fn run_with_options(
    header: &str,
    format: &str,
    mask_credentials: bool,
    strict: bool,
) -> Result<String, String> {
    let format = format.trim();
    let format = if format.is_empty() { "json" } else { format };
    if !matches!(format, "json" | "text" | "table") {
        return Err(format!(
            "unknown format '{format}': expected one of json, text, table"
        ));
    }

    let analysis = analyze(header)?;
    if strict && !analysis.warnings.is_empty() {
        return Err(format!(
            "strict mode: {} warning(s) — {}",
            analysis.warnings.len(),
            analysis.warnings.join(" | ")
        ));
    }

    Ok(match format {
        "text" => render_text(&analysis, mask_credentials),
        "table" => render_table(&analysis, mask_credentials),
        _ => render_json(&analysis, mask_credentials),
    })
}

// ---------------------------------------------------------------- parsing

fn analyze(input: &str) -> Result<Analysis, String> {
    if input.trim().is_empty() {
        return Err("header is required: paste an Authorization header value such as \"Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ==\", with or without the leading \"Authorization:\" name".into());
    }
    let len = input.chars().count();
    if len > MAX_INPUT_CHARS {
        return Err(format!(
            "header is {len} characters; the limit is {MAX_INPUT_CHARS}"
        ));
    }

    let mut warnings: Vec<String> = Vec::new();
    let raw = input.trim();
    if raw.contains('\n') || raw.contains('\r') || raw.contains('\t') {
        warnings.push(
            "the pasted header contained line breaks or tabs; they were treated as spaces before parsing".into(),
        );
    }
    let flattened: String = raw
        .chars()
        .map(|c| if c == '\n' || c == '\r' || c == '\t' { ' ' } else { c })
        .collect();
    let raw = flattened.trim();

    // "Authorization: <value>" — a header NAME is a token followed by ':' before any space.
    let (header_name, value) = split_header_name(raw);
    if let Some(name) = &header_name {
        let lower = name.to_ascii_lowercase();
        if lower != "authorization" && lower != "proxy-authorization" {
            warnings.push(format!(
                "the header name is \"{name}\", not Authorization or Proxy-Authorization; it was parsed with the same RFC 7235 grammar anyway"
            ));
        }
    }
    let value = value.trim();
    if value.is_empty() {
        return Err(format!(
            "the header name \"{}\" has no value after the colon",
            header_name.unwrap_or_else(|| "Authorization".into())
        ));
    }

    // "<scheme> <credentials>" — or a naked credential with no scheme at all.
    let (scheme_word, rest) = match value.find(' ') {
        Some(i) => (&value[..i], value[i + 1..].trim()),
        None => (value, ""),
    };
    let lookup = lookup_scheme(scheme_word);

    let (scheme, entry, credentials) = if rest.is_empty() {
        match lookup {
            Some(e) => {
                return Err(format!(
                    "the header carries the scheme \"{}\" but no credentials after it",
                    e.1
                ))
            }
            None => {
                // No scheme word — treat the whole value as bare credentials and guess.
                let guess = guess_scheme(value);
                match guess {
                    Some(e) => warnings.push(format!(
                        "no authentication scheme was present; the value looks like {} credentials and was decoded as such — a real header must start with \"{} \"",
                        e.1, e.1
                    )),
                    None => warnings.push(
                        "no authentication scheme was present and the value did not match a known credential shape, so only its raw form is reported".into(),
                    ),
                }
                (None, guess, value)
            }
        }
    } else {
        if lookup.is_none() {
            warnings.push(format!(
                "\"{scheme_word}\" is not a scheme this tool knows; its credentials were parsed with the generic RFC 7235 grammar"
            ));
        }
        (Some(scheme_word.to_string()), lookup, rest)
    };

    let kind = match entry {
        Some(e) => e.2,
        None => {
            if looks_like_params(credentials) {
                Kind::Params
            } else {
                Kind::Token68
            }
        }
    };

    // token68 credentials never contain spaces; params legitimately do.
    let credentials = if kind == Kind::Token68 && credentials.contains(' ') {
        warnings.push(
            "the credentials contained spaces, which a token68 credential may not; they were removed before decoding".into(),
        );
        credentials.chars().filter(|c| *c != ' ').collect::<String>()
    } else {
        credentials.to_string()
    };

    if let (Some(written), Some(e)) = (&scheme, entry) {
        if written != e.1 {
            warnings.push(format!(
                "the scheme is written \"{written}\"; schemes are matched case-insensitively, but the registered spelling is \"{}\"",
                e.1
            ));
        }
    }

    let mut analysis = Analysis {
        header_name,
        scheme,
        scheme_canonical: entry.map(|e| e.1.to_string()),
        scheme_known: entry.is_some(),
        scheme_registered: entry.map(|e| e.3).unwrap_or(false),
        scheme_description: entry
            .map(|e| e.4.to_string())
            .unwrap_or_else(|| "Unrecognized auth-scheme; parsed with the generic RFC 7235 grammar.".into()),
        credentials: credentials.clone(),
        kind,
        basic: None,
        bearer: None,
        params: Vec::new(),
        scope: Vec::new(),
        blob: None,
        warnings,
    };

    match entry.map(|e| e.1) {
        Some("Basic") => decode_basic(&mut analysis, &credentials)?,
        Some("Bearer") | Some("DPoP") | Some("Token") | Some("ApiKey") => {
            decode_bearer(&mut analysis, &credentials)
        }
        Some("Negotiate") | Some("NTLM") => decode_blob(&mut analysis, &credentials),
        Some("AWS4-HMAC-SHA256") => {
            analysis.params = parse_auth_params(&credentials);
            decode_aws_scope(&mut analysis);
        }
        _ => {
            if kind == Kind::Params {
                analysis.params = parse_auth_params(&credentials);
                if analysis.params.is_empty() {
                    analysis
                        .warnings
                        .push("no name=value auth-params could be parsed out of the credentials".into());
                }
            } else {
                // Any other token68 credential (a known scheme like HOBA, or an entirely
                // unknown one): still report its shape rather than nothing.
                decode_bearer(&mut analysis, &credentials);
                if let Some(b) = analysis.bearer.as_mut() {
                    if b.token_type == "opaque" {
                        b.token_type = "token68";
                    }
                }
            }
        }
    }

    Ok(analysis)
}

fn split_header_name(raw: &str) -> (Option<String>, &str) {
    let colon = match raw.find(':') {
        Some(i) => i,
        None => return (None, raw),
    };
    let space = raw.find(' ').unwrap_or(usize::MAX);
    if colon > space {
        return (None, raw);
    }
    let name = &raw[..colon];
    let is_token = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if is_token {
        (Some(name.to_string()), &raw[colon + 1..])
    } else {
        (None, raw)
    }
}

type SchemeEntry = &'static (&'static str, &'static str, Kind, bool, &'static str);

fn lookup_scheme(word: &str) -> Option<SchemeEntry> {
    let lower = word.to_ascii_lowercase();
    SCHEMES.iter().find(|e| e.0 == lower)
}

/// A bare credential with no scheme: JWT-shaped → Bearer, base64 of `user:pass` → Basic.
fn guess_scheme(value: &str) -> Option<SchemeEntry> {
    if value.matches('.').count() == 2 && jose_header_of(value).is_some() {
        return lookup_scheme("bearer");
    }
    if let Ok((bytes, _)) = decode_base64(value) {
        if let Ok(text) = std::str::from_utf8(&bytes) {
            if text.contains(':') && text.chars().all(|c| !c.is_control()) {
                return lookup_scheme("basic");
            }
        }
    }
    None
}

fn looks_like_params(credentials: &str) -> bool {
    // A trailing '=' is base64 padding, not an auth-param assignment — drop it first.
    let trimmed = credentials.trim_end_matches('=');
    match trimmed.find('=') {
        Some(i) => {
            let name = &trimmed[..i];
            !name.is_empty()
                && name.chars().all(|c| {
                    c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '*'
                })
        }
        None => false,
    }
}

fn decode_basic(a: &mut Analysis, credentials: &str) -> Result<(), String> {
    let (bytes, mut warns) = decode_base64(credentials).map_err(|e| {
        format!("the Basic credentials are not valid base64: {e}. Expected \"Basic \" followed by base64 of username:password, e.g. Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ==")
    })?;
    a.warnings.append(&mut warns);

    let (text, valid_utf8) = match String::from_utf8(bytes.clone()) {
        Ok(t) => (t, true),
        Err(_) => {
            a.warnings.push(
                "the decoded credentials are not valid UTF-8; invalid bytes are shown as U+FFFD. RFC 7617 encodes non-ASCII credentials as UTF-8 unless a charset parameter says otherwise".into(),
            );
            (String::from_utf8_lossy(&bytes).into_owned(), false)
        }
    };

    let (username, password, has_separator) = match text.find(':') {
        Some(i) => (text[..i].to_string(), text[i + 1..].to_string(), true),
        None => (text.clone(), String::new(), false),
    };
    if !has_separator {
        a.warnings.push(
            "the decoded credentials contain no ':' separator, so the whole value is reported as the username and the password is empty".into(),
        );
    }
    if has_separator && password.is_empty() {
        a.warnings.push("the password is empty".into());
    }
    if password.contains(':') {
        a.warnings.push(
            "the password contains a ':'; RFC 7617 splits on the FIRST colon only, so every later colon belongs to the password".into(),
        );
    }
    a.warnings.push(BASE64_NOT_ENCRYPTION.into());

    a.basic = Some(Basic {
        username,
        password,
        has_separator,
        valid_utf8,
        decoded_bytes: bytes.len(),
    });
    Ok(())
}

fn decode_bearer(a: &mut Analysis, token: &str) {
    let segments: Vec<usize> = token.split('.').map(|s| s.chars().count()).collect();
    let jose = jose_header_of(token);
    let token_type = match (segments.len(), jose.is_some()) {
        (3, true) => "jwt",
        (5, true) => "jwe",
        _ => "opaque",
    };
    if token_type == "jwt" {
        a.warnings.push(
            "the token is a JWT; only its JOSE header is decoded here — use a JWT decoder to read the payload claims and check expiry".into(),
        );
    }
    if token_type == "opaque" && token.contains('.') && segments.len() == 3 {
        a.warnings.push(
            "the token has three dot-separated segments like a JWT, but the first segment is not a base64url JOSE header — it is being reported as opaque".into(),
        );
    }
    a.bearer = Some(Bearer {
        token: token.to_string(),
        token_type,
        charset: charset_of(token),
        segments,
        jose_header: jose,
    });
}

fn decode_blob(a: &mut Analysis, credentials: &str) {
    match decode_base64(credentials) {
        Ok((bytes, mut warns)) => {
            a.warnings.append(&mut warns);
            let content = if bytes.starts_with(b"NTLMSSP\0") && bytes.len() >= 12 {
                let t = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
                match t {
                    1 => "NTLMSSP type 1 (negotiate) message".to_string(),
                    2 => "NTLMSSP type 2 (challenge) message".to_string(),
                    3 => "NTLMSSP type 3 (authenticate) message — carries the user, domain and response".to_string(),
                    other => format!("NTLMSSP message of unknown type {other}"),
                }
            } else if bytes.first() == Some(&0x60) {
                "SPNEGO/GSS-API token (DER application tag 0), typically wrapping Kerberos or NTLM".to_string()
            } else {
                "binary credential blob with no recognized signature".to_string()
            };
            a.blob = Some(Blob { bytes: bytes.len(), content });
        }
        Err(e) => a
            .warnings
            .push(format!("the credential blob is not valid base64: {e}")),
    }
}

fn decode_aws_scope(a: &mut Analysis) {
    let cred = a
        .params
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("Credential"))
        .map(|(_, v)| v.clone());
    let Some(cred) = cred else {
        a.warnings
            .push("the AWS4-HMAC-SHA256 header has no Credential parameter".into());
        return;
    };
    let parts: Vec<&str> = cred.split('/').collect();
    if parts.len() < 5 {
        a.warnings.push(format!(
            "the Credential scope \"{cred}\" does not have the expected access-key/date/region/service/aws4_request shape"
        ));
        return;
    }
    a.scope = vec![
        ("access_key_id".into(), parts[0].to_string()),
        ("date".into(), parts[1].to_string()),
        ("region".into(), parts[2].to_string()),
        ("service".into(), parts[3].to_string()),
        ("termination".into(), parts[4..].join("/")),
    ];
}

/// RFC 7235 auth-param list: `name=value` or `name="quoted value"`, comma separated.
fn parse_auth_params(input: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        while i < chars.len() && (chars[i].is_whitespace() || chars[i] == ',') {
            i += 1;
        }
        let start = i;
        while i < chars.len() && chars[i] != '=' && chars[i] != ',' {
            i += 1;
        }
        let name: String = chars[start..i].iter().collect::<String>().trim().to_string();
        if i >= chars.len() || chars[i] == ',' {
            if !name.is_empty() {
                out.push((name, String::new()));
            }
            continue;
        }
        i += 1; // skip '='
        while i < chars.len() && chars[i] == ' ' {
            i += 1;
        }
        let value = if i < chars.len() && chars[i] == '"' {
            i += 1;
            let mut v = String::new();
            while i < chars.len() {
                if chars[i] == '\\' && i + 1 < chars.len() {
                    v.push(chars[i + 1]);
                    i += 2;
                } else if chars[i] == '"' {
                    i += 1;
                    break;
                } else {
                    v.push(chars[i]);
                    i += 1;
                }
            }
            v
        } else {
            let s = i;
            while i < chars.len() && chars[i] != ',' {
                i += 1;
            }
            chars[s..i].iter().collect::<String>().trim().to_string()
        };
        if !name.is_empty() {
            out.push((name, value));
        }
    }
    out
}

fn jose_header_of(token: &str) -> Option<Value> {
    let first = token.split('.').next()?;
    if first.is_empty() {
        return None;
    }
    let bytes = decode_base64(first).ok()?.0;
    let value: Value = serde_json::from_slice(&bytes).ok()?;
    match &value {
        Value::Object(m) if m.contains_key("alg") => Some(value),
        _ => None,
    }
}

fn charset_of(s: &str) -> &'static str {
    if s.is_empty() {
        return "empty";
    }
    let body: String = s.chars().filter(|c| *c != '.').collect();
    if body.chars().all(|c| c.is_ascii_hexdigit()) {
        "hex"
    } else if body
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '=')
    {
        "base64url"
    } else if body
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=')
    {
        "base64"
    } else {
        "other"
    }
}

fn decode_base64(s: &str) -> Result<(Vec<u8>, Vec<String>), String> {
    let mut warns = Vec::new();
    let url_safe = s.contains('-') || s.contains('_');
    let padded = s.ends_with('=');
    if url_safe {
        warns.push(
            "the credentials use the URL-safe base64 alphabet (- and _) instead of standard base64 (+ and /)".into(),
        );
    }
    let result = match (url_safe, padded) {
        (true, true) => URL_SAFE.decode(s),
        (true, false) => URL_SAFE_NO_PAD.decode(s),
        (false, true) => STANDARD.decode(s),
        (false, false) => STANDARD_NO_PAD.decode(s),
    };
    match result {
        Ok(bytes) => {
            if !padded && s.len() % 4 != 0 {
                warns.push(
                    "the base64 value is missing its '=' padding; it was decoded anyway".into(),
                );
            }
            Ok((bytes, warns))
        }
        Err(e) => Err(e.to_string()),
    }
}

// ---------------------------------------------------------------- rendering

fn masked(value: &str, mask: bool) -> String {
    if mask && !value.is_empty() {
        MASK.to_string()
    } else {
        value.to_string()
    }
}

fn is_secret_param(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "response" | "signature" | "cnonce" | "mac" | "sig"
    )
}

fn render_json(a: &Analysis, mask: bool) -> String {
    let mut root = Map::new();
    root.insert(
        "header_name".into(),
        a.header_name.clone().map(Value::from).unwrap_or(Value::Null),
    );
    root.insert("scheme".into(), a.scheme.clone().map(Value::from).unwrap_or(Value::Null));
    root.insert(
        "scheme_canonical".into(),
        a.scheme_canonical.clone().map(Value::from).unwrap_or(Value::Null),
    );
    root.insert("scheme_known".into(), Value::from(a.scheme_known));
    root.insert("scheme_registered".into(), Value::from(a.scheme_registered));
    root.insert("scheme_description".into(), Value::from(a.scheme_description.clone()));
    root.insert("credentials_kind".into(), Value::from(a.kind.as_str()));
    root.insert("credentials".into(), Value::from(masked(&a.credentials, mask)));
    root.insert(
        "credentials_length".into(),
        Value::from(a.credentials.chars().count()),
    );

    root.insert(
        "basic".into(),
        match &a.basic {
            None => Value::Null,
            Some(b) => json!({
                "username": b.username,
                "password": masked(&b.password, mask),
                "username_length": b.username.chars().count(),
                "password_length": b.password.chars().count(),
                "has_separator": b.has_separator,
                "valid_utf8": b.valid_utf8,
                "decoded_bytes": b.decoded_bytes,
            }),
        },
    );

    root.insert(
        "bearer".into(),
        match &a.bearer {
            None => Value::Null,
            Some(b) => json!({
                "token": masked(&b.token, mask),
                "token_length": b.token.chars().count(),
                "token_type": b.token_type,
                "charset": b.charset,
                "segments": b.segments.len(),
                "segment_lengths": b.segments,
                "jose_header": b.jose_header.clone().unwrap_or(Value::Null),
                "claims_decoded": false,
            }),
        },
    );

    root.insert(
        "parameters".into(),
        if a.params.is_empty() {
            Value::Null
        } else {
            let mut m = Map::new();
            for (k, v) in &a.params {
                m.insert(k.clone(), Value::from(masked(v, mask && is_secret_param(k))));
            }
            Value::Object(m)
        },
    );

    root.insert(
        "credential_scope".into(),
        if a.scope.is_empty() {
            Value::Null
        } else {
            let mut m = Map::new();
            for (k, v) in &a.scope {
                m.insert(k.clone(), Value::from(v.clone()));
            }
            Value::Object(m)
        },
    );

    root.insert(
        "binary".into(),
        match &a.blob {
            None => Value::Null,
            Some(b) => json!({ "bytes": b.bytes, "content": b.content }),
        },
    );

    root.insert(
        "warnings".into(),
        Value::Array(a.warnings.iter().cloned().map(Value::from).collect()),
    );

    serde_json::to_string_pretty(&Value::Object(root)).unwrap_or_else(|e| e.to_string())
}

fn rows(a: &Analysis, mask: bool) -> Vec<(String, String)> {
    let mut r: Vec<(String, String)> = Vec::new();
    if let Some(n) = &a.header_name {
        r.push(("header name".into(), n.clone()));
    }
    r.push((
        "scheme".into(),
        match (&a.scheme, &a.scheme_canonical) {
            (Some(s), Some(c)) if s != c => format!("{s} (canonical: {c})"),
            (Some(s), _) => s.clone(),
            (None, Some(c)) => format!("(none — assumed {c})"),
            (None, None) => "(none)".into(),
        },
    ));
    r.push(("scheme is".into(), a.scheme_description.clone()));
    r.push(("credential kind".into(), a.kind.as_str().into()));
    r.push((
        "credentials".into(),
        format!(
            "{} ({} chars)",
            masked(&a.credentials, mask),
            a.credentials.chars().count()
        ),
    ));

    if let Some(b) = &a.basic {
        r.push(("username".into(), b.username.clone()));
        r.push((
            "password".into(),
            format!("{} ({} chars)", masked(&b.password, mask), b.password.chars().count()),
        ));
        r.push(("decoded bytes".into(), b.decoded_bytes.to_string()));
        if !b.valid_utf8 {
            r.push(("valid utf-8".into(), "no".into()));
        }
    }
    if let Some(b) = &a.bearer {
        r.push(("token".into(), masked(&b.token, mask)));
        r.push(("token length".into(), format!("{} chars", b.token.chars().count())));
        r.push(("token type".into(), b.token_type.into()));
        r.push(("charset".into(), b.charset.into()));
        r.push((
            "segments".into(),
            format!(
                "{} ({})",
                b.segments.len(),
                b.segments
                    .iter()
                    .map(|n| n.to_string())
                    .collect::<Vec<_>>()
                    .join(" / ")
            ),
        ));
        if let Some(j) = &b.jose_header {
            r.push((
                "jose header".into(),
                serde_json::to_string(j).unwrap_or_default(),
            ));
        }
    }
    for (k, v) in &a.params {
        r.push((k.clone(), masked(v, mask && is_secret_param(k))));
    }
    for (k, v) in &a.scope {
        r.push((k.replace('_', " "), v.clone()));
    }
    if let Some(b) = &a.blob {
        r.push(("blob size".into(), format!("{} bytes", b.bytes)));
        r.push(("blob content".into(), b.content.clone()));
    }
    r
}

fn render_text(a: &Analysis, mask: bool) -> String {
    let rows = rows(a, mask);
    let width = rows.iter().map(|(k, _)| k.chars().count()).max().unwrap_or(0);
    let mut out = String::new();
    for (k, v) in &rows {
        out.push_str(&format!("{k}:{} {v}\n", " ".repeat(width - k.chars().count())));
    }
    if !a.warnings.is_empty() {
        out.push_str(&format!("\nWarnings ({}):\n", a.warnings.len()));
        for w in &a.warnings {
            out.push_str(&format!("  - {w}\n"));
        }
    }
    out.trim_end().to_string()
}

fn render_table(a: &Analysis, mask: bool) -> String {
    let rows = rows(a, mask);
    let kw = rows
        .iter()
        .map(|(k, _)| k.chars().count())
        .chain(std::iter::once(5))
        .max()
        .unwrap_or(5);
    let vw = rows
        .iter()
        .map(|(_, v)| v.chars().count())
        .chain(std::iter::once(5))
        .max()
        .unwrap_or(5);
    let bar = format!("+{}+{}+", "-".repeat(kw + 2), "-".repeat(vw + 2));
    let mut out = String::new();
    out.push_str(&format!("{bar}\n"));
    out.push_str(&format!(
        "| {:<kw$} | {:<vw$} |\n",
        "Field",
        "Value",
        kw = kw,
        vw = vw
    ));
    out.push_str(&format!("{bar}\n"));
    for (k, v) in &rows {
        out.push_str(&format!(
            "| {:<kw$} | {:<vw$} |\n",
            k,
            v,
            kw = kw,
            vw = vw
        ));
    }
    out.push_str(&bar);
    if !a.warnings.is_empty() {
        out.push_str(&format!("\n\nWarnings ({}):\n", a.warnings.len()));
        for w in &a.warnings {
            out.push_str(&format!("  - {w}\n"));
        }
    }
    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn json_of(header: &str) -> Value {
        serde_json::from_str(&run(header).unwrap()).unwrap()
    }

    #[test]
    fn decodes_the_rfc_7617_basic_example() {
        let v = json_of("Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ==");
        assert_eq!(v["scheme"], "Basic");
        assert_eq!(v["scheme_canonical"], "Basic");
        assert_eq!(v["credentials_kind"], "token68");
        assert_eq!(v["basic"]["username"], "Aladdin");
        assert_eq!(v["basic"]["password"], "open sesame");
        assert_eq!(v["basic"]["has_separator"], true);
        assert_eq!(v["basic"]["decoded_bytes"], 19);
        assert_eq!(v["bearer"], Value::Null);
    }

    #[test]
    fn accepts_the_full_header_line_and_odd_scheme_case() {
        let v = json_of("Authorization: bAsIc QWxhZGRpbjpvcGVuIHNlc2FtZQ==");
        assert_eq!(v["header_name"], "Authorization");
        assert_eq!(v["scheme"], "bAsIc");
        assert_eq!(v["scheme_canonical"], "Basic");
        assert_eq!(v["basic"]["username"], "Aladdin");
        let warns = v["warnings"].as_array().unwrap();
        assert!(warns.iter().any(|w| w.as_str().unwrap().contains("registered spelling")));
    }

    #[test]
    fn splits_on_the_first_colon_only() {
        // base64("user:pa:ss") — the password keeps its colon.
        let v = json_of("Basic dXNlcjpwYTpzcw==");
        assert_eq!(v["basic"]["username"], "user");
        assert_eq!(v["basic"]["password"], "pa:ss");
        assert!(v["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains("FIRST colon")));
    }

    #[test]
    fn empty_password_is_flagged_but_decodes() {
        let v = json_of("Basic dXNlcjo=");
        assert_eq!(v["basic"]["username"], "user");
        assert_eq!(v["basic"]["password"], "");
        assert_eq!(v["basic"]["password_length"], 0);
    }

    #[test]
    fn missing_colon_is_a_warning_not_a_crash() {
        // base64("nocolonhere")
        let v = json_of("Basic bm9jb2xvbmhlcmU=");
        assert_eq!(v["basic"]["has_separator"], false);
        assert_eq!(v["basic"]["username"], "nocolonhere");
    }

    #[test]
    fn bearer_jwt_reports_structure_and_jose_header_only() {
        let t = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.c2lnbmF0dXJl";
        let v = json_of(&format!("Bearer {t}"));
        assert_eq!(v["scheme_canonical"], "Bearer");
        assert_eq!(v["bearer"]["token_type"], "jwt");
        assert_eq!(v["bearer"]["segments"], 3);
        assert_eq!(v["bearer"]["jose_header"]["alg"], "HS256");
        assert_eq!(v["bearer"]["jose_header"]["typ"], "JWT");
        assert_eq!(v["bearer"]["claims_decoded"], false);
        assert_eq!(v["bearer"]["charset"], "base64url");
        assert_eq!(v["basic"], Value::Null);
    }

    #[test]
    fn opaque_bearer_token_is_described_not_rejected() {
        let v = json_of("Bearer 5f3a9c1e7b2d4a6f8c0e1d3b5a7f9c2e");
        assert_eq!(v["bearer"]["token_type"], "opaque");
        assert_eq!(v["bearer"]["charset"], "hex");
        assert_eq!(v["bearer"]["token_length"], 32);
        assert_eq!(v["bearer"]["jose_header"], Value::Null);
    }

    #[test]
    fn digest_auth_params_are_parsed() {
        let v = json_of(
            r#"Digest username="alice", realm="example.com", nonce="dcd98b7102dd2f0e", uri="/dir/index.html", qop=auth, nc=00000001, response="6629fae49393a05397450978507c4ef1""#,
        );
        assert_eq!(v["scheme_canonical"], "Digest");
        assert_eq!(v["credentials_kind"], "auth-params");
        assert_eq!(v["parameters"]["username"], "alice");
        assert_eq!(v["parameters"]["realm"], "example.com");
        assert_eq!(v["parameters"]["qop"], "auth");
        assert_eq!(v["parameters"]["nc"], "00000001");
        assert_eq!(v["parameters"]["response"], "6629fae49393a05397450978507c4ef1");
    }

    #[test]
    fn aws_sigv4_credential_scope_is_split() {
        let v = json_of("AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/iam/aws4_request, SignedHeaders=content-type;host;x-amz-date, Signature=5d672d79c15b13162d9279b0855cfba6789a8edb4c82c400e06b5924a6f2b5d7");
        assert_eq!(v["scheme_canonical"], "AWS4-HMAC-SHA256");
        assert_eq!(v["credential_scope"]["access_key_id"], "AKIDEXAMPLE");
        assert_eq!(v["credential_scope"]["date"], "20150830");
        assert_eq!(v["credential_scope"]["region"], "us-east-1");
        assert_eq!(v["credential_scope"]["service"], "iam");
        assert_eq!(v["parameters"]["SignedHeaders"], "content-type;host;x-amz-date");
    }

    #[test]
    fn ntlm_blob_type_is_identified() {
        // "NTLMSSP\0" + little-endian message type 3.
        let mut bytes = b"NTLMSSP\0".to_vec();
        bytes.extend_from_slice(&3u32.to_le_bytes());
        let b64 = STANDARD.encode(&bytes);
        let v = json_of(&format!("NTLM {b64}"));
        assert_eq!(v["scheme_canonical"], "NTLM");
        assert_eq!(v["binary"]["bytes"], 12);
        assert!(v["binary"]["content"].as_str().unwrap().contains("type 3"));
    }

    #[test]
    fn hawk_params_are_parsed() {
        let v = json_of(r#"Hawk id="dh37fgj492je", ts="1353832234", mac="6R4rV5iE+NPoym+WwjeHzjAGXUtLNIxmo1vpMofpLAE=""#);
        assert_eq!(v["scheme"], "Hawk");
        assert_eq!(v["parameters"]["id"], "dh37fgj492je");
        assert_eq!(v["parameters"]["ts"], "1353832234");
    }

    #[test]
    fn a_scheme_we_dont_know_is_still_parsed_generically() {
        let v = json_of(r#"MyScheme keyId="key-1", algorithm="hs2019""#);
        assert_eq!(v["scheme"], "MyScheme");
        assert_eq!(v["scheme_canonical"], Value::Null);
        assert_eq!(v["scheme_known"], false);
        assert_eq!(v["credentials_kind"], "auth-params");
        assert_eq!(v["parameters"]["keyId"], "key-1");
        assert!(v["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains("not a scheme this tool knows")));
    }

    #[test]
    fn an_unknown_scheme_with_an_opaque_token_reports_its_shape() {
        let v = json_of("MyScheme QWxhZGRpbjpvcGVu");
        assert_eq!(v["credentials_kind"], "token68");
        assert_eq!(v["bearer"]["token_type"], "token68");
        assert_eq!(v["bearer"]["token_length"], 16);
    }

    #[test]
    fn bare_base64_with_no_scheme_is_assumed_basic() {
        let v = json_of("QWxhZGRpbjpvcGVuIHNlc2FtZQ==");
        assert_eq!(v["scheme"], Value::Null);
        assert_eq!(v["scheme_canonical"], "Basic");
        assert_eq!(v["basic"]["username"], "Aladdin");
        assert!(v["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains("no authentication scheme")));
    }

    #[test]
    fn masking_hides_secrets_but_keeps_lengths() {
        let out = run_with_options("Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ==", "json", true, false).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["basic"]["username"], "Aladdin");
        assert_eq!(v["basic"]["password"], MASK);
        assert_eq!(v["basic"]["password_length"], 11);
        assert_eq!(v["credentials"], MASK);
        assert!(!out.contains("open sesame"));
    }

    #[test]
    fn masking_hides_the_digest_response_but_not_the_realm() {
        let out = run_with_options(
            r#"Digest username="alice", realm="example.com", response="6629fae49393a05397450978507c4ef1""#,
            "json",
            true,
            false,
        )
        .unwrap();
        assert!(out.contains("example.com"));
        assert!(!out.contains("6629fae49393a05397450978507c4ef1"));
    }

    #[test]
    fn text_format_is_aligned_key_value_lines() {
        let out = run_with_options("Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ==", "text", false, false).unwrap();
        assert!(out.contains("username:"));
        assert!(out.contains("Aladdin"));
        assert!(out.contains("open sesame"));
        assert!(out.contains("Warnings ("));
    }

    #[test]
    fn table_format_has_a_border() {
        let out = run_with_options("Basic dXNlcjpwYXNz", "table", false, false).unwrap();
        assert!(out.starts_with('+'));
        assert!(out.contains("| Field"));
        assert!(out.contains("user"));
    }

    #[test]
    fn strict_mode_turns_warnings_into_errors() {
        let err = run_with_options("Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ==", "json", false, true)
            .unwrap_err();
        assert!(err.starts_with("strict mode:"));
    }

    #[test]
    fn strict_mode_passes_a_clean_digest_header() {
        let ok = run_with_options(
            r#"Digest username="alice", realm="example.com", nonce="dcd98b7102dd2f0e", response="6629fae49393a05397450978507c4ef1""#,
            "json",
            false,
            true,
        );
        assert!(ok.is_ok(), "unexpected strict failure: {ok:?}");
    }

    #[test]
    fn empty_input_errors() {
        let err = run("   ").unwrap_err();
        assert!(err.contains("header is required"));
    }

    #[test]
    fn invalid_base64_basic_errors_with_guidance() {
        let err = run("Basic not!valid!base64").unwrap_err();
        assert!(err.contains("not valid base64"), "got: {err}");
        assert!(err.contains("QWxhZGRpbjpvcGVuIHNlc2FtZQ=="));
    }

    #[test]
    fn scheme_without_credentials_errors() {
        let err = run("Bearer").unwrap_err();
        assert!(err.contains("no credentials"), "got: {err}");
    }

    #[test]
    fn over_long_input_errors() {
        let long = format!("Basic {}", "A".repeat(MAX_INPUT_CHARS));
        let err = run(&long).unwrap_err();
        assert!(err.contains("the limit is"));
    }

    #[test]
    fn unknown_format_errors() {
        let err = run_with_options("Basic dXNlcjpwYXNz", "yaml", false, false).unwrap_err();
        assert!(err.contains("expected one of json, text, table"));
    }

    #[test]
    fn line_wrapped_header_still_decodes() {
        let v = json_of("Authorization: Basic\nQWxhZGRpbjpvcGVuIHNlc2FtZQ==");
        assert_eq!(v["basic"]["username"], "Aladdin");
        assert!(v["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|w| w.as_str().unwrap().contains("line breaks")));
    }

    #[test]
    fn non_utf8_credentials_are_reported_not_fatal() {
        // 0xff is not valid UTF-8; the pair is still split on the colon.
        let bytes = [b'u', b's', b'e', b'r', b':', 0xff, 0xfe];
        let v = json_of(&format!("Basic {}", STANDARD.encode(bytes)));
        assert_eq!(v["basic"]["username"], "user");
        assert_eq!(v["basic"]["valid_utf8"], false);
    }
}

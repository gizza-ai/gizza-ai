//! aws-sigv4-signer core — compute an AWS Signature Version 4 (SigV4) request
//! signature. Pure compute, shared by the chat skill block and the web page. No
//! wafer/wasm-bindgen deps.
//!
//! SigV4 (AWS4-HMAC-SHA256) is the signing protocol every AWS API request uses.
//! Given the request (method, URL, headers, payload), the credentials
//! (access/secret key, optional session token), and the target region + service,
//! this module builds the four canonical artifacts AWS itself reconstructs to
//! verify a request:
//!
//!   1. Canonical Request — method, canonical URI, canonical query string,
//!      canonical headers, signed-header list, and the hex SHA-256 of the payload.
//!   2. String to Sign — `AWS4-HMAC-SHA256`, the request timestamp, the credential
//!      scope (`YYYYMMDD/region/service/aws4_request`), and the hex SHA-256 of the
//!      canonical request.
//!   3. Signing key — HMAC-SHA256 chain over date → region → service →
//!      `aws4_request`, keyed initially by `"AWS4" + secret`.
//!   4. Signature + Authorization header — HMAC-SHA256(signing key, string to
//!      sign) as lowercase hex, assembled into the `Authorization` header value.
//!
//! Everything is pure-Rust HMAC/SHA-256 (RustCrypto) so it runs on every backend,
//! including the chat Service Worker. The timestamp is passed in explicitly
//! (`amz_date`) so signing is deterministic; each surface supplies "now" when the
//! caller leaves it blank.

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

/// Which artifact(s) to return. `All` is a labelled multi-section report; the
/// others return a single piece for copy-paste / scripting.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Output {
    All,
    Authorization,
    Headers,
    CanonicalRequest,
    StringToSign,
    Signature,
    Curl,
}

fn parse_output(s: &str) -> Result<Output, String> {
    let canon: String = s
        .trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|c| *c != '_' && *c != '-')
        .collect();
    match canon.as_str() {
        "" | "all" => Ok(Output::All),
        "authorization" | "auth" => Ok(Output::Authorization),
        "headers" => Ok(Output::Headers),
        "canonicalrequest" | "canonical" => Ok(Output::CanonicalRequest),
        "stringtosign" | "sts" => Ok(Output::StringToSign),
        "signature" | "sig" => Ok(Output::Signature),
        "curl" => Ok(Output::Curl),
        other => Err(format!(
            "invalid output '{other}': expected one of all, authorization, headers, canonical-request, string-to-sign, signature, curl"
        )),
    }
}

/// HTTP methods this tool accepts.
fn normalize_method(s: &str) -> Result<String, String> {
    let m = s.trim().to_ascii_uppercase();
    match m.as_str() {
        "" => Ok("GET".into()),
        "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS" => Ok(m),
        other => Err(format!(
            "invalid method '{other}': expected GET, POST, PUT, PATCH, DELETE, HEAD, or OPTIONS"
        )),
    }
}

/// The parsed pieces of a request URL.
struct ParsedUrl {
    /// Host header value (authority minus userinfo, default ports stripped).
    host: String,
    /// Raw path component (may be empty).
    path: String,
    /// Raw query string (without the leading `?`; empty if none).
    query: String,
}

/// Hand-parse `scheme://host[:port]/path?query` — no `url` crate dependency.
fn parse_url(url: &str) -> Result<ParsedUrl, String> {
    let url = url.trim();
    if url.is_empty() {
        return Err("url is required".into());
    }
    // Strip scheme.
    let after_scheme = match url.find("://") {
        Some(i) => &url[i + 3..],
        None => {
            return Err(format!(
                "invalid url '{url}': expected an absolute URL like https://service.region.amazonaws.com/path"
            ))
        }
    };
    // Authority ends at the first '/', '?', or '#'.
    let auth_end = after_scheme
        .find(['/', '?', '#'])
        .unwrap_or(after_scheme.len());
    let authority = &after_scheme[..auth_end];
    let rest = &after_scheme[auth_end..];
    // Strip userinfo (user:pass@).
    let hostport = match authority.rfind('@') {
        Some(i) => &authority[i + 1..],
        None => authority,
    };
    if hostport.is_empty() {
        return Err(format!("invalid url '{url}': missing host"));
    }
    // Drop default ports so the Host header matches what a client actually sends.
    let host = hostport
        .strip_suffix(":443")
        .or_else(|| hostport.strip_suffix(":80"))
        .unwrap_or(hostport)
        .to_string();
    // Split path / query (ignore any fragment).
    let (path_part, query) = match rest.split_once('?') {
        Some((p, q)) => (p, q.split('#').next().unwrap_or("")),
        None => (rest.split('#').next().unwrap_or(""), ""),
    };
    Ok(ParsedUrl {
        host,
        path: path_part.to_string(),
        query: query.to_string(),
    })
}

/// AWS `UriEncode`: percent-encode every byte except the unreserved set
/// `A-Z a-z 0-9 - . _ ~`. When `encode_slash` is false, `/` is left as-is (used
/// for path segments); when true, `/` is also encoded (used for query keys/values).
fn uri_encode(s: &str, encode_slash: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        let keep = b.is_ascii_alphanumeric()
            || matches!(b, b'-' | b'.' | b'_' | b'~')
            || (b == b'/' && !encode_slash);
        if keep {
            out.push(b as char);
        } else {
            out.push('%');
            out.push_str(&format!("{b:02X}"));
        }
    }
    out
}

/// Remove RFC 3986 dot-segments (`.` / `..`) from a path (used for every service
/// except S3, which signs the raw path).
fn normalize_path(path: &str) -> String {
    let leading_slash = path.starts_with('/');
    let trailing_slash = path.ends_with('/') && path.len() > 1;
    let mut out: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            s => out.push(s),
        }
    }
    let mut joined = out.join("/");
    if leading_slash {
        joined.insert(0, '/');
    }
    if trailing_slash && !joined.ends_with('/') {
        joined.push('/');
    }
    if joined.is_empty() {
        joined.push('/');
    }
    joined
}

/// Build the canonical URI: normalize (unless S3), then URI-encode each segment.
fn canonical_uri(path: &str, is_s3: bool) -> String {
    let path = if path.is_empty() { "/" } else { path };
    let base = if is_s3 {
        path.to_string()
    } else {
        normalize_path(path)
    };
    // Encode segment-by-segment so `/` separators are preserved.
    base.split('/')
        .map(|seg| uri_encode(seg, true))
        .collect::<Vec<_>>()
        .join("/")
}

/// Build the canonical query string: URI-encode each name/value, sort by
/// encoded name then encoded value, join `name=value` pairs with `&`.
fn canonical_query(query: &str) -> String {
    if query.is_empty() {
        return String::new();
    }
    let mut pairs: Vec<(String, String)> = query
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| match p.split_once('=') {
            Some((k, v)) => (uri_encode(k, true), uri_encode(v, true)),
            None => (uri_encode(p, true), String::new()),
        })
        .collect();
    pairs.sort();
    pairs
        .into_iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&")
}

/// Trim leading/trailing whitespace and collapse internal runs of spaces to a
/// single space (AWS header-value `Trim`).
fn trim_header_value(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    let mut prev_space = false;
    for ch in v.trim().chars() {
        if ch == ' ' || ch == '\t' {
            if !prev_space {
                out.push(' ');
            }
            prev_space = true;
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    out
}

/// Parse the user-supplied additional headers (one `Name: Value` per line) into
/// (lowercased-name, trimmed-value) pairs. Duplicate names are comma-joined.
fn parse_headers(headers: &str) -> Result<Vec<(String, String)>, String> {
    let mut acc: Vec<(String, String)> = Vec::new();
    for (i, line) in headers.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (name, value) = line.split_once(':').ok_or_else(|| {
            format!(
                "invalid header on line {}: expected 'Name: Value', got '{}'",
                i + 1,
                line
            )
        })?;
        let name = name.trim().to_ascii_lowercase();
        if name.is_empty() {
            return Err(format!("invalid header on line {}: empty header name", i + 1));
        }
        let value = trim_header_value(value);
        if let Some(existing) = acc.iter_mut().find(|(n, _)| *n == name) {
            existing.1 = format!("{},{}", existing.1, value);
        } else {
            acc.push((name, value));
        }
    }
    Ok(acc)
}

/// Validate + normalize an `amz_date`. Accepts the canonical basic format
/// `YYYYMMDDTHHMMSSZ` and the extended ISO-8601 form `YYYY-MM-DDTHH:MM:SSZ`
/// (separators are stripped). Returns `(amz_date_basic, yyyymmdd)`.
fn parse_amz_date(amz_date: &str) -> Result<(String, String), String> {
    let cleaned: String = amz_date
        .trim()
        .chars()
        .filter(|c| *c != '-' && *c != ':')
        .collect();
    let bytes = cleaned.as_bytes();
    let ok = cleaned.len() == 16
        && bytes[8] == b'T'
        && (bytes[15] == b'Z' || bytes[15] == b'z')
        && bytes[..8].iter().all(|b| b.is_ascii_digit())
        && bytes[9..15].iter().all(|b| b.is_ascii_digit());
    if !ok {
        return Err(format!(
            "invalid amz_date '{amz_date}': expected ISO-8601 basic UTC 'YYYYMMDDTHHMMSSZ' (e.g. 20150830T123600Z)"
        ));
    }
    let normalized = format!("{}Z", &cleaned[..15]); // force uppercase Z
    let date = cleaned[..8].to_string();
    Ok((normalized, date))
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut m = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
    m.update(data);
    m.finalize().into_bytes().to_vec()
}

fn sha256_hex(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    hex::encode(h.finalize())
}

/// Derive the SigV4 signing key: HMAC chain date → region → service →
/// `aws4_request`, keyed initially by `"AWS4" + secret`.
fn signing_key(secret: &str, date: &str, region: &str, service: &str) -> Vec<u8> {
    let k_date = hmac_sha256(format!("AWS4{secret}").as_bytes(), date.as_bytes());
    let k_region = hmac_sha256(&k_date, region.as_bytes());
    let k_service = hmac_sha256(&k_region, service.as_bytes());
    hmac_sha256(&k_service, b"aws4_request")
}

/// Format an epoch-seconds timestamp as `YYYYMMDDTHHMMSSZ` (UTC). Used by each
/// surface to supply "now" when the caller leaves `amz_date` blank. Pure integer
/// civil-from-days math (Howard Hinnant's algorithm) — no chrono dependency.
pub fn format_amz_date(epoch_secs: i64) -> String {
    let days = epoch_secs.div_euclid(86_400);
    let secs = epoch_secs.rem_euclid(86_400);
    let (hh, mm, ss) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    // days since 1970-01-01 → civil date.
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let year = if m <= 2 { y + 1 } else { y };
    format!("{year:04}{m:02}{d:02}T{hh:02}{mm:02}{ss:02}Z")
}

/// The full result of a signing operation. `render` turns it into the requested
/// output form.
pub struct Signed {
    method: String,
    url: String,
    host: String,
    amz_date: String,
    scope: String,
    canonical_request: String,
    string_to_sign: String,
    signature: String,
    authorization: String,
    signed_headers: String,
    /// Ordered (Name, Value) of the headers a client must send, incl. Authorization.
    send_headers: Vec<(String, String)>,
    payload: String,
    has_payload: bool,
}

impl Signed {
    fn render(&self, output: Output) -> String {
        match output {
            Output::Authorization => self.authorization.clone(),
            Output::Signature => self.signature.clone(),
            Output::CanonicalRequest => self.canonical_request.clone(),
            Output::StringToSign => self.string_to_sign.clone(),
            Output::Headers => self.headers_block(),
            Output::Curl => self.curl(),
            Output::All => {
                format!(
                    "=== Authorization header ===\n{}\n\n=== Headers to send ===\n{}\n\n=== Canonical request ===\n{}\n\n=== String to sign ===\n{}\n\n=== Signature ===\n{}\n\n=== Credential scope ===\n{}\n\n=== Signed headers ===\n{}",
                    self.authorization,
                    self.headers_block(),
                    self.canonical_request,
                    self.string_to_sign,
                    self.signature,
                    self.scope,
                    self.signed_headers,
                )
            }
        }
    }

    fn headers_block(&self) -> String {
        self.send_headers
            .iter()
            .map(|(n, v)| format!("{n}: {v}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn curl(&self) -> String {
        let mut parts = vec![format!("curl -X {}", self.method)];
        for (n, v) in &self.send_headers {
            parts.push(format!("  -H '{}: {}'", n, v.replace('\'', "'\\''")));
        }
        if self.has_payload {
            parts.push(format!("  --data-raw '{}'", self.payload.replace('\'', "'\\''")));
        }
        parts.push(format!("  '{}'", self.url));
        // Suppress unused-field warning while keeping host available for callers.
        let _ = &self.host;
        let _ = &self.amz_date;
        parts.join(" \\\n")
    }
}

/// Compute the SigV4 signature for a request.
///
/// - `method`: HTTP method (GET/POST/PUT/PATCH/DELETE/HEAD/OPTIONS; blank ⇒ GET).
/// - `url`: full request URL incl. path + query (`https://host/path?query`).
/// - `region` / `service`: the AWS region (e.g. `us-east-1`) and service code
///   (e.g. `s3`, `iam`, `execute-api`).
/// - `access_key` / `secret_key`: the AWS credentials.
/// - `session_token`: optional STS session token; when non-empty it is added as
///   `x-amz-security-token` and signed.
/// - `payload`: the request body (used for the SHA-256 payload hash).
/// - `extra_headers`: additional headers, one `Name: Value` per line. The `host`
///   and `x-amz-date` headers are added automatically.
/// - `amz_date`: request timestamp `YYYYMMDDTHHMMSSZ` (must be non-empty — the
///   calling surface fills "now" when the user leaves it blank).
/// - `unsigned_payload`: sign with the literal `UNSIGNED-PAYLOAD` instead of the
///   payload hash (common for S3 streaming uploads).
/// - `sign_content_sha256`: add + sign an `x-amz-content-sha256` header (required
///   by Amazon S3).
/// - `output`: which artifact(s) to return.
#[allow(clippy::too_many_arguments)]
pub fn sign(
    method: &str,
    url: &str,
    region: &str,
    service: &str,
    access_key: &str,
    secret_key: &str,
    session_token: &str,
    payload: &str,
    extra_headers: &str,
    amz_date: &str,
    unsigned_payload: bool,
    sign_content_sha256: bool,
    output: &str,
) -> Result<String, String> {
    let method = normalize_method(method)?;
    let out = parse_output(output)?;
    let region = region.trim();
    let service = service.trim();
    let access_key = access_key.trim();
    if region.is_empty() {
        return Err("region is required (e.g. us-east-1)".into());
    }
    if service.is_empty() {
        return Err("service is required (e.g. s3, iam, execute-api)".into());
    }
    if access_key.is_empty() {
        return Err("access_key is required".into());
    }
    if secret_key.is_empty() {
        return Err("secret_key is required".into());
    }
    let (amz_date, date) = parse_amz_date(amz_date)?;
    let parsed = parse_url(url)?;
    let is_s3 = service.eq_ignore_ascii_case("s3");

    // Payload hash.
    let payload_hash = if unsigned_payload {
        "UNSIGNED-PAYLOAD".to_string()
    } else {
        sha256_hex(payload.as_bytes())
    };

    // Assemble the header set to sign: host + x-amz-date + optional
    // security-token / content-sha256 + user headers.
    let mut headers: Vec<(String, String)> = vec![
        ("host".to_string(), parsed.host.clone()),
        ("x-amz-date".to_string(), amz_date.clone()),
    ];
    if !session_token.trim().is_empty() {
        headers.push((
            "x-amz-security-token".to_string(),
            session_token.trim().to_string(),
        ));
    }
    if sign_content_sha256 {
        headers.push(("x-amz-content-sha256".to_string(), payload_hash.clone()));
    }
    for (n, v) in parse_headers(extra_headers)? {
        if let Some(existing) = headers.iter_mut().find(|(en, _)| *en == n) {
            existing.1 = v; // user override wins for auto headers
        } else {
            headers.push((n, v));
        }
    }
    headers.sort_by(|a, b| a.0.cmp(&b.0));

    let canonical_headers: String = headers
        .iter()
        .map(|(n, v)| format!("{n}:{v}\n"))
        .collect();
    let signed_headers = headers
        .iter()
        .map(|(n, _)| n.as_str())
        .collect::<Vec<_>>()
        .join(";");

    let canonical_request = format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        method,
        canonical_uri(&parsed.path, is_s3),
        canonical_query(&parsed.query),
        canonical_headers,
        signed_headers,
        payload_hash,
    );

    let scope = format!("{date}/{region}/{service}/aws4_request");
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{}\n{}\n{}",
        amz_date,
        scope,
        sha256_hex(canonical_request.as_bytes()),
    );

    let key = signing_key(secret_key, &date, region, service);
    let signature = hex::encode(hmac_sha256(&key, string_to_sign.as_bytes()));

    let authorization = format!(
        "AWS4-HMAC-SHA256 Credential={access_key}/{scope}, SignedHeaders={signed_headers}, Signature={signature}"
    );

    // Headers a client must actually send (signed set + Authorization).
    let mut send_headers = headers.clone();
    send_headers.push(("Authorization".to_string(), authorization.clone()));

    let signed = Signed {
        method,
        url: url.trim().to_string(),
        host: parsed.host,
        amz_date,
        scope,
        canonical_request,
        string_to_sign,
        signature,
        authorization,
        signed_headers,
        send_headers,
        payload: payload.to_string(),
        has_payload: !payload.is_empty(),
    };
    Ok(signed.render(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- AWS documented worked examples (external correctness ground truth) ----

    // AWS S3 GET Object doc example. Secret uses a '/' (not '+').
    #[test]
    fn s3_get_object_doc_vector() {
        let sig = sign(
            "GET",
            "https://examplebucket.s3.amazonaws.com/test.txt",
            "us-east-1",
            "s3",
            "AKIAIOSFODNN7EXAMPLE",
            "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
            "",
            "",
            "range:bytes=0-9",
            "20130524T000000Z",
            false,
            true, // sign x-amz-content-sha256 (S3)
            "signature",
        )
        .unwrap();
        assert_eq!(
            sig, "f0e8bdb87c964420e857bd35b5d6ed310bd44f0170aba48dd91039c6036bdb41",
            "S3 GET Object documented signature"
        );
    }

    #[test]
    fn s3_get_object_canonical_request() {
        let cr = sign(
            "GET",
            "https://examplebucket.s3.amazonaws.com/test.txt",
            "us-east-1",
            "s3",
            "AKIAIOSFODNN7EXAMPLE",
            "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
            "",
            "",
            "range:bytes=0-9",
            "20130524T000000Z",
            false,
            true,
            "canonical-request",
        )
        .unwrap();
        assert_eq!(
            cr,
            "GET\n/test.txt\n\nhost:examplebucket.s3.amazonaws.com\nrange:bytes=0-9\nx-amz-content-sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855\nx-amz-date:20130524T000000Z\n\nhost;range;x-amz-content-sha256;x-amz-date\ne3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    // AWS IAM ListUsers doc example. Secret uses a '+'.
    #[test]
    fn iam_list_users_doc_vector() {
        let sig = sign(
            "GET",
            "https://iam.amazonaws.com/?Action=ListUsers&Version=2010-05-08",
            "us-east-1",
            "iam",
            "AKIDEXAMPLE",
            "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY",
            "",
            "",
            "content-type:application/x-www-form-urlencoded; charset=utf-8",
            "20150830T123600Z",
            false,
            false,
            "signature",
        )
        .unwrap();
        assert_eq!(
            sig, "5d672d79c15b13162d9279b0855cfba6789a8edb4c82c400e06b5924a6f2b5d7",
            "IAM ListUsers documented signature"
        );
    }

    #[test]
    fn authorization_header_shape() {
        let auth = sign(
            "GET",
            "https://iam.amazonaws.com/?Action=ListUsers&Version=2010-05-08",
            "us-east-1",
            "iam",
            "AKIDEXAMPLE",
            "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY",
            "",
            "",
            "content-type:application/x-www-form-urlencoded; charset=utf-8",
            "20150830T123600Z",
            false,
            false,
            "authorization",
        )
        .unwrap();
        assert_eq!(
            auth,
            "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/iam/aws4_request, SignedHeaders=content-type;host;x-amz-date, Signature=5d672d79c15b13162d9279b0855cfba6789a8edb4c82c400e06b5924a6f2b5d7"
        );
    }

    // ---- Feature behavior ----

    #[test]
    fn session_token_is_signed() {
        let auth = sign(
            "GET",
            "https://example.execute-api.us-east-1.amazonaws.com/prod",
            "us-east-1",
            "execute-api",
            "AKIDEXAMPLE",
            "secret",
            "FQoGZXIvYXdzEExampleToken",
            "",
            "",
            "20150830T123600Z",
            false,
            false,
            "authorization",
        )
        .unwrap();
        assert!(
            auth.contains("x-amz-security-token"),
            "session token adds x-amz-security-token to SignedHeaders: {auth}"
        );
    }

    #[test]
    fn unsigned_payload_uses_literal() {
        let cr = sign(
            "PUT",
            "https://examplebucket.s3.amazonaws.com/big.bin",
            "us-east-1",
            "s3",
            "AKIDEXAMPLE",
            "secret",
            "",
            "ignored body",
            "",
            "20150830T123600Z",
            true,
            true,
            "canonical-request",
        )
        .unwrap();
        assert!(cr.contains("x-amz-content-sha256:UNSIGNED-PAYLOAD"), "{cr}");
        assert!(cr.ends_with("UNSIGNED-PAYLOAD"), "payload hash line: {cr}");
    }

    #[test]
    fn query_string_is_sorted_and_encoded() {
        let cr = sign(
            "GET",
            "https://svc.us-east-1.amazonaws.com/?b=2&a=1&c=a b",
            "us-east-1",
            "svc",
            "AKIDEXAMPLE",
            "secret",
            "",
            "",
            "",
            "20150830T123600Z",
            false,
            false,
            "canonical-request",
        )
        .unwrap();
        // second line is the canonical query string.
        let qline = cr.lines().nth(2).unwrap();
        assert_eq!(qline, "a=1&b=2&c=a%20b", "{cr}");
    }

    #[test]
    fn curl_includes_authorization_and_body() {
        let curl = sign(
            "POST",
            "https://svc.us-east-1.amazonaws.com/",
            "us-east-1",
            "svc",
            "AKIDEXAMPLE",
            "secret",
            "",
            "{\"k\":1}",
            "content-type:application/json",
            "20150830T123600Z",
            false,
            false,
            "curl",
        )
        .unwrap();
        assert!(curl.starts_with("curl -X POST"), "{curl}");
        assert!(curl.contains("-H 'Authorization: AWS4-HMAC-SHA256"), "{curl}");
        assert!(curl.contains("--data-raw '{\"k\":1}'"), "{curl}");
    }

    #[test]
    fn all_output_has_every_section() {
        let all = sign(
            "GET",
            "https://iam.amazonaws.com/?Action=ListUsers&Version=2010-05-08",
            "us-east-1",
            "iam",
            "AKIDEXAMPLE",
            "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY",
            "",
            "",
            "content-type:application/x-www-form-urlencoded; charset=utf-8",
            "20150830T123600Z",
            false,
            false,
            "all",
        )
        .unwrap();
        for section in [
            "=== Authorization header ===",
            "=== Headers to send ===",
            "=== Canonical request ===",
            "=== String to sign ===",
            "=== Signature ===",
        ] {
            assert!(all.contains(section), "missing {section} in:\n{all}");
        }
    }

    #[test]
    fn extended_iso_date_is_accepted() {
        let a = sign(
            "GET",
            "https://iam.amazonaws.com/?Action=ListUsers&Version=2010-05-08",
            "us-east-1",
            "iam",
            "AKIDEXAMPLE",
            "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY",
            "",
            "",
            "content-type:application/x-www-form-urlencoded; charset=utf-8",
            "2015-08-30T12:36:00Z",
            false,
            false,
            "signature",
        )
        .unwrap();
        assert_eq!(a, "5d672d79c15b13162d9279b0855cfba6789a8edb4c82c400e06b5924a6f2b5d7");
    }

    #[test]
    fn format_amz_date_is_correct() {
        // 20150830T123600Z = 1440938160 epoch seconds.
        assert_eq!(format_amz_date(1_440_938_160), "20150830T123600Z");
        // 1970-01-01T00:00:00Z
        assert_eq!(format_amz_date(0), "19700101T000000Z");
        // 2013-05-24T00:00:00Z = 1369353600
        assert_eq!(format_amz_date(1_369_353_600), "20130524T000000Z");
    }

    // ---- Error paths ----

    #[test]
    fn missing_region_errors() {
        let e = sign(
            "GET", "https://x.amazonaws.com/", "", "s3", "AK", "sk", "", "", "",
            "20150830T123600Z", false, false, "all",
        )
        .unwrap_err();
        assert!(e.contains("region is required"), "{e}");
    }

    #[test]
    fn bad_amz_date_errors() {
        let e = sign(
            "GET", "https://x.amazonaws.com/", "us-east-1", "s3", "AK", "sk", "", "", "",
            "not-a-date", false, false, "all",
        )
        .unwrap_err();
        assert!(e.contains("invalid amz_date"), "{e}");
    }

    #[test]
    fn bad_url_errors() {
        let e = sign(
            "GET", "iam.amazonaws.com/path", "us-east-1", "s3", "AK", "sk", "", "", "",
            "20150830T123600Z", false, false, "all",
        )
        .unwrap_err();
        assert!(e.contains("invalid url"), "{e}");
    }

    #[test]
    fn bad_header_line_errors() {
        let e = sign(
            "GET", "https://x.amazonaws.com/", "us-east-1", "s3", "AK", "sk", "", "",
            "NoColonHeaderLine", "20150830T123600Z", false, false, "all",
        )
        .unwrap_err();
        assert!(e.contains("invalid header"), "{e}");
    }

    #[test]
    fn bad_output_errors() {
        let e = sign(
            "GET", "https://x.amazonaws.com/", "us-east-1", "s3", "AK", "sk", "", "", "",
            "20150830T123600Z", false, false, "nonsense",
        )
        .unwrap_err();
        assert!(e.contains("invalid output"), "{e}");
    }

    #[test]
    fn bad_method_errors() {
        let e = sign(
            "TRACE", "https://x.amazonaws.com/", "us-east-1", "s3", "AK", "sk", "", "", "",
            "20150830T123600Z", false, false, "all",
        )
        .unwrap_err();
        assert!(e.contains("invalid method"), "{e}");
    }
}

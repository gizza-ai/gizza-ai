//! har-redact core — replace the sensitive VALUES in a HAR (HTTP Archive)
//! capture with a placeholder so the capture is safe to attach to a bug
//! report while its full structure stays intact for debugging. Pure Rust
//! (`serde_json` with `preserve_order`, so the redacted capture keeps the
//! original key order and stays diff-able against the source).
//!
//! What is redacted (each VALUE is replaced, the surrounding structure —
//! header/cookie/param NAMES, URLs' paths, methods, status codes, timings,
//! sizes — is kept): cookie values (`request.cookies[].value`,
//! `response.cookies[].value` and the `Cookie`/`Set-Cookie` header values);
//! `Authorization` and common API-key/token header values; the values of
//! sensitive query-string parameters (in `request.queryString[]` and in the
//! `request.url` query string); and, per the `bodies` mode, request
//! `postData` and response `content.text`.
//!
//! Distinct from `har-body-stripper`, which DELETES body fields to shrink a
//! capture (and deliberately leaves cookies/headers alone). This tool
//! SUBSTITUTES values in place — nothing is removed, so any HAR viewer still
//! opens the result and the request waterfall still renders.

use serde_json::Value;

/// Hard cap on `log.entries` per run.
pub const MAX_ENTRIES: usize = 10_000;

/// Header names (case-insensitive) whose values are cookie material.
const COOKIE_HEADERS: &[&str] = &["cookie", "set-cookie", "cookie2", "set-cookie2"];

/// Built-in auth / API-key / token header names (case-insensitive).
const AUTH_HEADERS: &[&str] = &[
    "authorization",
    "proxy-authorization",
    "www-authenticate",
    "proxy-authenticate",
    "authentication",
    "x-api-key",
    "api-key",
    "apikey",
    "x-auth-token",
    "auth-token",
    "x-auth",
    "x-access-token",
    "x-session-token",
    "x-amz-security-token",
    "x-csrf-token",
    "x-xsrf-token",
    "x-client-data",
];

/// Built-in sensitive query/form parameter names (case-insensitive, exact
/// name match). Union of the terms the common HAR sanitizers ship with.
const SENSITIVE_PARAMS: &[&str] = &[
    "password", "passwd", "pwd", "pass",
    "token", "access_token", "refresh_token", "id_token", "auth_token",
    "authenticity_token", "csrf_token", "xsrf_token", "antiforgery",
    "api_key", "apikey", "api-key", "key", "client_secret", "secret", "client_id",
    "sig", "signature", "hmac",
    "code", "code_verifier", "challenge", "assertion",
    "session", "sessionid", "session_id", "sid", "jsessionid", "phpsessid",
    "jwt", "bearer", "auth", "authorization", "otp", "pin",
    "samlrequest", "samlresponse", "state", "email", "usg",
];

#[derive(Default)]
struct Counts {
    cookies: usize,
    headers: usize,
    query: usize,
    bodies: usize,
}

/// Redact a HAR document.
///
/// * `cookies` — redact cookie values (request/response `cookies[].value` and
///   the `Cookie`/`Set-Cookie` header values).
/// * `auth_headers` — redact `Authorization` and built-in API-key/token
///   header values.
/// * `extra_headers` — comma-separated ADDITIONAL header names (case-insensitive)
///   whose values to redact.
/// * `query_params` — redact the values of sensitive query-string parameters,
///   in both `request.queryString[]` and the `request.url` query string.
/// * `sensitive_params` — comma-separated ADDITIONAL parameter names to treat
///   as sensitive (merged with the built-in list).
/// * `bodies` — `none`, `request`, `response`, or `both`: which bodies to
///   replace with the placeholder.
/// * `placeholder` — the text each redacted value is replaced with.
/// * `output` — `har` (the redacted capture as JSON) or `summary` (a dry-run
///   report of per-category counts).
/// * `pretty` — pretty-print the output HAR (2-space indent).
#[allow(clippy::too_many_arguments)]
pub fn redact_har(
    har: &str,
    cookies: bool,
    auth_headers: bool,
    extra_headers: &str,
    query_params: bool,
    sensitive_params: &str,
    bodies: &str,
    placeholder: &str,
    output: &str,
    pretty: bool,
) -> Result<String, String> {
    if har.trim().is_empty() {
        return Err("no HAR input".into());
    }
    if placeholder.is_empty() {
        return Err("placeholder must not be empty".into());
    }
    let (body_req, body_resp) = match bodies {
        "none" => (false, false),
        "request" => (true, false),
        "response" => (false, true),
        "both" => (true, true),
        other => {
            return Err(format!(
                "unknown bodies mode '{other}' (expected none, request, response, or both)"
            ))
        }
    };
    if output != "har" && output != "summary" {
        return Err(format!("unknown output '{output}' (expected har or summary)"));
    }

    // Extra header names + sensitive param names, lowercased.
    let extra_hdrs: Vec<String> = split_names(extra_headers);
    let mut param_names: Vec<String> = SENSITIVE_PARAMS.iter().map(|s| s.to_string()).collect();
    param_names.extend(split_names(sensitive_params));

    let is_auth_header =
        |lname: &str| AUTH_HEADERS.contains(&lname) || extra_hdrs.iter().any(|h| h == lname);
    let is_cookie_header = |lname: &str| COOKIE_HEADERS.contains(&lname);
    let is_sensitive_param = |lname: &str| param_names.iter().any(|p| p == lname);

    let mut root: Value =
        serde_json::from_str(har).map_err(|e| format!("invalid JSON: {e}"))?;

    let mut c = Counts::default();
    let entry_count;

    {
        let entries = root
            .get_mut("log")
            .and_then(|l| l.get_mut("entries"))
            .and_then(|e| e.as_array_mut())
            .ok_or_else(|| {
                "not a HAR capture: expected a top-level { \"log\": { \"entries\": [ … ] } } \
                 object (browser DevTools → Network tab → \"Save all as HAR\")"
                    .to_string()
            })?;
        if entries.len() > MAX_ENTRIES {
            return Err(format!(
                "too many entries: {} (max {MAX_ENTRIES} entries per run)",
                entries.len()
            ));
        }
        entry_count = entries.len();

        for entry in entries.iter_mut() {
            let Some(entry) = entry.as_object_mut() else {
                continue; // forgiving: skip malformed entries, redact the rest
            };

            // ---- request side ----
            if let Some(req) = entry.get_mut("request").and_then(Value::as_object_mut) {
                if let Some(hs) = req.get_mut("headers").and_then(Value::as_array_mut) {
                    redact_headers(
                        hs, placeholder, &mut c, cookies, auth_headers,
                        &is_cookie_header, &is_auth_header,
                    );
                }
                if cookies {
                    if let Some(cs) = req.get_mut("cookies").and_then(Value::as_array_mut) {
                        redact_nv_values(cs, placeholder, &mut c.cookies, |_| true);
                    }
                }
                if query_params {
                    if let Some(qs) = req.get_mut("queryString").and_then(Value::as_array_mut) {
                        redact_nv_values(qs, placeholder, &mut c.query, &is_sensitive_param);
                    }
                    if let Some(url) = req.get("url").and_then(Value::as_str) {
                        let (redacted, n) = redact_url_query(url, placeholder, &is_sensitive_param);
                        if n > 0 {
                            c.query += n;
                            req.insert("url".into(), Value::String(redacted));
                        }
                    }
                }
                if body_req {
                    if let Some(post) = req.get_mut("postData").and_then(Value::as_object_mut) {
                        if redact_string_value(post, "text", placeholder) {
                            c.bodies += 1;
                        }
                        if let Some(params) = post.get_mut("params").and_then(Value::as_array_mut) {
                            redact_nv_values(params, placeholder, &mut c.bodies, |_| true);
                        }
                    }
                }
            }

            // ---- response side ----
            if let Some(resp) = entry.get_mut("response").and_then(Value::as_object_mut) {
                if let Some(hs) = resp.get_mut("headers").and_then(Value::as_array_mut) {
                    redact_headers(
                        hs, placeholder, &mut c, cookies, auth_headers,
                        &is_cookie_header, &is_auth_header,
                    );
                }
                if cookies {
                    if let Some(cs) = resp.get_mut("cookies").and_then(Value::as_array_mut) {
                        redact_nv_values(cs, placeholder, &mut c.cookies, |_| true);
                    }
                }
                if body_resp {
                    if let Some(content) = resp.get_mut("content").and_then(Value::as_object_mut) {
                        if redact_string_value(content, "text", placeholder) {
                            c.bodies += 1;
                        }
                    }
                }
            }
        }
    }

    let out = if pretty {
        serde_json::to_string_pretty(&root)
    } else {
        serde_json::to_string(&root)
    }
    .map_err(|e| format!("serialize failed: {e}"))?;

    if output == "har" {
        return Ok(out);
    }

    Ok(format!(
        "HAR redaction summary\n\
         entries scanned: {entry_count}\n\
         cookies redacted: {}\n\
         auth/api-key headers redacted: {}\n\
         query-string values redacted: {}\n\
         body fields redacted: {}\n\
         placeholder: {placeholder}\n\
         Run with output=har to get the redacted capture.",
        c.cookies, c.headers, c.query, c.bodies,
    ))
}

/// Split a comma-separated list of names into a lowercased, trimmed, non-empty Vec.
fn split_names(s: &str) -> Vec<String> {
    s.split(',')
        .map(|x| x.trim().to_ascii_lowercase())
        .filter(|x| !x.is_empty())
        .collect()
}

/// Replace `obj[key]` with `placeholder` if it holds a non-empty string that
/// isn't already the placeholder. Returns whether a replacement happened.
fn redact_string_value(
    obj: &mut serde_json::Map<String, Value>,
    key: &str,
    placeholder: &str,
) -> bool {
    match obj.get(key).and_then(Value::as_str) {
        Some(v) if !v.is_empty() && v != placeholder => {
            obj.insert(key.into(), Value::String(placeholder.into()));
            true
        }
        _ => false,
    }
}

/// Redact the `value` of each `{name, value}` object in `arr` whose lowercased
/// `name` satisfies `want`. Increments `count` per value actually replaced.
fn redact_nv_values(
    arr: &mut [Value],
    placeholder: &str,
    count: &mut usize,
    want: impl Fn(&str) -> bool,
) {
    for item in arr.iter_mut() {
        let Some(obj) = item.as_object_mut() else { continue };
        let name = obj.get("name").and_then(Value::as_str).unwrap_or("");
        if !want(&name.to_ascii_lowercase()) {
            continue;
        }
        if redact_string_value(obj, "value", placeholder) {
            *count += 1;
        }
    }
}

/// Redact header values, routing each hit to the cookie or the auth counter.
#[allow(clippy::too_many_arguments)]
fn redact_headers(
    headers: &mut [Value],
    placeholder: &str,
    c: &mut Counts,
    cookies: bool,
    auth_headers: bool,
    is_cookie_header: &impl Fn(&str) -> bool,
    is_auth_header: &impl Fn(&str) -> bool,
) {
    for h in headers.iter_mut() {
        let Some(obj) = h.as_object_mut() else { continue };
        let lname = obj
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_ascii_lowercase();
        if cookies && is_cookie_header(&lname) {
            if redact_string_value(obj, "value", placeholder) {
                c.cookies += 1;
            }
        } else if auth_headers && is_auth_header(&lname) {
            if redact_string_value(obj, "value", placeholder) {
                c.headers += 1;
            }
        }
    }
}

/// Redact the sensitive parameter values in a URL's query string, preserving
/// the base, the parameter order/names, and any `#fragment`. Returns the new
/// URL and how many values were replaced.
fn redact_url_query(url: &str, placeholder: &str, want: &impl Fn(&str) -> bool) -> (String, usize) {
    let Some(qpos) = url.find('?') else {
        return (url.to_string(), 0);
    };
    let (base, after) = url.split_at(qpos);
    let after = &after[1..]; // drop '?'
    let (query, fragment) = match after.find('#') {
        Some(fpos) => (&after[..fpos], Some(&after[fpos..])),
        None => (after, None),
    };

    let mut count = 0;
    let parts: Vec<String> = query
        .split('&')
        .map(|pair| {
            if pair.is_empty() {
                return pair.to_string();
            }
            match pair.split_once('=') {
                Some((k, v)) if !v.is_empty() && want(&k.to_ascii_lowercase()) => {
                    count += 1;
                    format!("{k}={placeholder}")
                }
                _ => pair.to_string(),
            }
        })
        .collect();

    if count == 0 {
        return (url.to_string(), 0);
    }
    let mut out = format!("{base}?{}", parts.join("&"));
    if let Some(frag) = fragment {
        out.push_str(frag);
    }
    (out, count)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PH: &str = "[REDACTED]";

    /// A HAR with a login POST (Cookie + Authorization headers, a sessionid
    /// cookie, a ?token= query param, a form postData) and a JSON response
    /// (Set-Cookie header, a response cookie, a JSON body).
    fn sample() -> String {
        r#"{"log":{"version":"1.2","creator":{"name":"t","version":"1"},"entries":[
            {"request":{"method":"POST",
                "url":"https://x.test/login?token=SECRET123&page=2",
                "headers":[
                    {"name":"Cookie","value":"sid=abcdef; theme=dark"},
                    {"name":"Authorization","value":"Bearer eyJhbGc.payload.sig"},
                    {"name":"Accept","value":"application/json"}],
                "cookies":[
                    {"name":"sessionid","value":"topsecretsession"},
                    {"name":"theme","value":"dark"}],
                "queryString":[
                    {"name":"token","value":"SECRET123"},
                    {"name":"page","value":"2"}],
                "postData":{"mimeType":"application/x-www-form-urlencoded",
                    "text":"user=alice&password=hunter2",
                    "params":[{"name":"user","value":"alice"},{"name":"password","value":"hunter2"}]}},
             "response":{"status":200,
                "headers":[
                    {"name":"Set-Cookie","value":"sid=newsecret; HttpOnly"},
                    {"name":"Content-Type","value":"application/json"}],
                "cookies":[{"name":"sid","value":"newsecret"}],
                "content":{"size":25,"mimeType":"application/json","text":"{\"session\":\"privatetok\"}"}}}
        ]}}"#
            .to_string()
    }

    #[test]
    fn defaults_redact_cookies_auth_query_and_response_body() {
        // Defaults: cookies+auth+query on, bodies=response.
        let out = redact_har(&sample(), true, true, "", true, "", "response", PH, "har", false)
            .unwrap();
        // Cookie header + cookie arrays gone.
        assert!(!out.contains("abcdef"), "cookie header value redacted");
        assert!(!out.contains("topsecretsession"), "request cookie redacted");
        assert!(!out.contains("newsecret"), "set-cookie + response cookie redacted");
        // Auth header gone.
        assert!(!out.contains("eyJhbGc"), "authorization header redacted");
        // Query token gone in queryString AND url.
        assert!(!out.contains("SECRET123"), "query token redacted everywhere");
        // Response body gone.
        assert!(!out.contains("privatetok"), "response body redacted");
        // Request body kept (bodies=response only).
        assert!(out.contains("hunter2"), "request postData kept by default");
        // Structure/metadata survives.
        assert!(out.contains("\"name\":\"Authorization\""), "header names kept");
        assert!(out.contains("\"mimeType\":\"application/json\""), "mime kept");
        assert!(out.contains("token=[REDACTED]"), "url token replaced literally, not url-encoded");
        assert!(out.contains("page=2"), "non-sensitive query param kept");
        assert!(out.contains("\"size\":25"), "content size kept");
        assert!(out.matches(PH).count() >= 7, "several values redacted: {out}");
    }

    #[test]
    fn bodies_both_redacts_request_body_too() {
        let out = redact_har(&sample(), false, false, "", false, "", "both", PH, "har", false)
            .unwrap();
        assert!(!out.contains("hunter2"), "postData text + param redacted");
        // postData.text replaced wholesale, so the inline user=alice text is gone too.
        assert!(!out.contains("user=alice"), "postData text replaced wholesale");
        // The params array values are ALL redacted in body mode (predicate is all).
        assert!(!out.contains("\"value\":\"alice\""), "postData param values redacted in body mode");
        assert!(!out.contains("privatetok"), "response body redacted under both");
        // cookies/auth/query off → those survive.
        assert!(out.contains("abcdef"), "cookie kept when cookies=false");
        assert!(out.contains("SECRET123"), "query kept when query_params=false");
    }

    #[test]
    fn nothing_redacted_when_all_off_and_bodies_none() {
        let out = redact_har(&sample(), false, false, "", false, "", "none", PH, "har", false)
            .unwrap();
        assert!(!out.contains(PH), "no redactions: {out}");
        assert!(out.contains("hunter2") && out.contains("topsecretsession"));
    }

    #[test]
    fn extra_headers_and_sensitive_params_extend_the_lists() {
        let har = r#"{"log":{"entries":[
            {"request":{"method":"GET","url":"https://x.test/?tenant=acme&page=1",
                "headers":[{"name":"X-Tenant","value":"acme-corp"}],
                "queryString":[{"name":"tenant","value":"acme"},{"name":"page","value":"1"}]}}
        ]}}"#;
        // Without extras, X-Tenant / tenant are not sensitive.
        let base = redact_har(har, true, true, "", true, "", "none", PH, "har", false).unwrap();
        assert!(base.contains("acme-corp") && base.contains("\"value\":\"acme\""));
        // With extras they are.
        let out = redact_har(har, true, true, "x-tenant", true, "tenant", "none", PH, "har", false)
            .unwrap();
        assert!(!out.contains("acme-corp"), "extra header redacted");
        assert!(!out.contains("\"value\":\"acme\""), "extra param redacted");
        assert!(out.contains("page=1"), "other query param kept");
    }

    #[test]
    fn custom_placeholder_used() {
        let out = redact_har(&sample(), true, true, "", true, "", "response", "***", "har", false)
            .unwrap();
        assert!(out.contains("token=***"), "custom placeholder in url");
        assert!(!out.contains("SECRET123"));
        assert!(!out.contains("[REDACTED]"), "default placeholder not used");
    }

    #[test]
    fn summary_reports_per_category_counts() {
        let out = redact_har(&sample(), true, true, "", true, "", "response", PH, "summary", false)
            .unwrap();
        assert!(out.starts_with("HAR redaction summary\n"));
        assert!(out.contains("entries scanned: 1"), "got:\n{out}");
        // cookies: Cookie header + 2 req cookies + Set-Cookie header + 1 resp cookie = 5
        assert!(out.contains("cookies redacted: 5"), "got:\n{out}");
        // auth headers: Authorization = 1
        assert!(out.contains("auth/api-key headers redacted: 1"), "got:\n{out}");
        // query: queryString token + url token = 2
        assert!(out.contains("query-string values redacted: 2"), "got:\n{out}");
        // body: response content.text = 1
        assert!(out.contains("body fields redacted: 1"), "got:\n{out}");
        assert!(out.contains("placeholder: [REDACTED]"));
        assert!(out.trim_end().ends_with("Run with output=har to get the redacted capture."));
    }

    #[test]
    fn url_without_query_is_unchanged() {
        let (out, n) = redact_url_query("https://x.test/path", PH, &|_| true);
        assert_eq!(n, 0);
        assert_eq!(out, "https://x.test/path");
    }

    #[test]
    fn url_query_preserves_fragment_and_order() {
        let want = |k: &str| k == "token";
        let (out, n) = redact_url_query("https://x.test/p?a=1&token=xyz&b=2#frag", PH, &want);
        assert_eq!(n, 1);
        assert_eq!(out, "https://x.test/p?a=1&token=[REDACTED]&b=2#frag");
    }

    #[test]
    fn url_empty_and_flag_params_skipped() {
        let want = |k: &str| k == "token" || k == "flag";
        let (out, n) = redact_url_query("https://x.test/p?token=&flag&keep=1", PH, &want);
        assert_eq!(n, 0, "empty value and value-less flag are not redacted");
        assert_eq!(out, "https://x.test/p?token=&flag&keep=1");
    }

    #[test]
    fn idempotent_second_run_is_a_no_op() {
        let once = redact_har(&sample(), true, true, "", true, "", "both", PH, "har", false).unwrap();
        let twice = redact_har(&once, true, true, "", true, "", "both", PH, "har", false).unwrap();
        assert_eq!(once, twice, "re-redacting an already-redacted capture changes nothing");
    }

    #[test]
    fn pretty_and_compact_serialization() {
        let compact = redact_har(&sample(), true, true, "", true, "", "response", PH, "har", false)
            .unwrap();
        assert!(!compact.contains('\n'), "compact output is one line");
        let pretty = redact_har(&sample(), true, true, "", true, "", "response", PH, "har", true)
            .unwrap();
        assert!(pretty.contains("\n  \"log\""), "pretty output is 2-space indented");
    }

    #[test]
    fn key_order_is_preserved() {
        let har = r#"{"log":{"version":"1.2","creator":{"name":"t","version":"1"},"pages":[],"entries":[]}}"#;
        let out = redact_har(har, true, true, "", true, "", "both", PH, "har", false).unwrap();
        let v = out.find("\"version\"").unwrap();
        let cr = out.find("\"creator\"").unwrap();
        let p = out.find("\"pages\"").unwrap();
        let e = out.find("\"entries\"").unwrap();
        assert!(v < cr && cr < p && p < e, "original key order must survive: {out}");
    }

    #[test]
    fn rejects_non_json_and_non_har() {
        let err = redact_har("not json", true, true, "", true, "", "response", PH, "har", false)
            .unwrap_err();
        assert!(err.starts_with("invalid JSON:"), "{err}");
        let err = redact_har(r#"{"foo":1}"#, true, true, "", true, "", "response", PH, "har", false)
            .unwrap_err();
        assert!(err.starts_with("not a HAR capture:"), "{err}");
    }

    #[test]
    fn rejects_empty_bad_modes_and_empty_placeholder() {
        assert_eq!(
            redact_har("  ", true, true, "", true, "", "response", PH, "har", false).unwrap_err(),
            "no HAR input"
        );
        let err = redact_har(&sample(), true, true, "", true, "", "everything", PH, "har", false)
            .unwrap_err();
        assert!(err.contains("unknown bodies mode 'everything'"), "{err}");
        let err = redact_har(&sample(), true, true, "", true, "", "response", PH, "csv", false)
            .unwrap_err();
        assert!(err.contains("unknown output 'csv'"), "{err}");
        let err = redact_har(&sample(), true, true, "", true, "", "response", "", "har", false)
            .unwrap_err();
        assert!(err.contains("placeholder must not be empty"), "{err}");
    }

    #[test]
    fn entry_cap_boundary_at_and_over() {
        let entry = r#"{"request":{"method":"GET","url":"https://x.test/?token=x","queryString":[{"name":"token","value":"x"}]}}"#;
        let at = format!(r#"{{"log":{{"entries":[{}]}}}}"#, vec![entry; MAX_ENTRIES].join(","));
        let out = redact_har(&at, true, true, "", true, "", "none", PH, "summary", false).unwrap();
        assert!(out.contains(&format!("entries scanned: {MAX_ENTRIES}")));

        let over = format!(r#"{{"log":{{"entries":[{}]}}}}"#, vec![entry; MAX_ENTRIES + 1].join(","));
        let err = redact_har(&over, true, true, "", true, "", "none", PH, "summary", false)
            .unwrap_err();
        assert_eq!(
            err,
            format!("too many entries: {} (max {MAX_ENTRIES} entries per run)", MAX_ENTRIES + 1)
        );
    }

    #[test]
    fn malformed_entries_are_skipped_not_fatal() {
        let har = r#"{"log":{"entries":[
            "garbage",
            {"request":{"method":"GET","url":"https://x.test/?token=zzz","queryString":[{"name":"token","value":"zzz"}]}}
        ]}}"#;
        let out = redact_har(har, true, true, "", true, "", "none", PH, "har", false).unwrap();
        assert!(!out.contains("zzz"), "the well-formed entry is still redacted");
        assert!(out.contains("garbage"), "the malformed entry is left as-is");
    }
}

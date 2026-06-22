//! gizza-ai/safelink-decoder core — pure compute, shared by the chat skill block
//! and the web page. No wafer/wasm-bindgen deps. Unwraps URL-rewrite wrappers
//! (Outlook SafeLinks, Proofpoint URLDefense v2/v3, Google redirects, and any
//! generic `?url=`/`?q=`/`?u=` redirector) to reveal the original destination.
//! Pure string work — it never fetches anything.

use percent_encoding::percent_decode_str;

fn pct_decode(s: &str) -> String {
    percent_decode_str(s).decode_utf8_lossy().to_string()
}

/// Read a query parameter's raw (still-encoded) value from a URL.
fn query_param<'a>(url: &'a str, name: &str) -> Option<&'a str> {
    let q = url.split_once('?')?.1;
    let q = q.split('#').next().unwrap_or(q);
    for pair in q.split('&') {
        let (k, v) = pair.split_once('=')?;
        if k.eq_ignore_ascii_case(name) {
            return Some(v);
        }
    }
    None
}

fn host_of(url: &str) -> String {
    let after = url.split("://").nth(1).unwrap_or(url);
    after.split(['/', '?', '#']).next().unwrap_or("").to_ascii_lowercase()
}

/// Unwrap one layer. Returns the embedded URL if `url` is a recognized wrapper,
/// else None.
fn unwrap_once(url: &str) -> Option<String> {
    let host = host_of(url);

    // Outlook SafeLinks: *.safelinks.protection.outlook.com/?url=<enc>
    if host.contains("safelinks.protection.outlook") {
        if let Some(v) = query_param(url, "url") {
            return Some(pct_decode(v));
        }
    }
    // Proofpoint URLDefense v2: .../v2/url?u=<enc with - => % and _ => />
    if host.contains("urldefense") && url.contains("/v2/") {
        if let Some(v) = query_param(url, "u") {
            let restored = v.replace('-', "%").replace('_', "/");
            return Some(pct_decode(&restored));
        }
    }
    // Proofpoint URLDefense v3: .../v3/__<URL>__;<base64>!!...
    if host.contains("urldefense") && url.contains("/v3/__") {
        if let Some(rest) = url.split("/v3/__").nth(1) {
            if let Some(inner) = rest.split("__;").next() {
                // v3 replaces some run chars with '*'; strip a trailing '*' marker.
                return Some(pct_decode(inner.trim_end_matches('*')));
            }
        }
    }
    // Google redirect: google.*/url?q=<enc> (or &url=).
    if host.starts_with("www.google.") || host == "google.com" || host.starts_with("google.") {
        if url.contains("/url") {
            for p in ["q", "url"] {
                if let Some(v) = query_param(url, p) {
                    return Some(pct_decode(v));
                }
            }
        }
    }
    // Generic redirector: a query param named url/u/q/target/dest/redirect whose
    // decoded value looks like an http(s) URL.
    for p in ["url", "u", "q", "target", "dest", "destination", "redirect", "r"] {
        if let Some(v) = query_param(url, p) {
            let dec = pct_decode(v);
            if dec.starts_with("http://") || dec.starts_with("https://") {
                return Some(dec);
            }
        }
    }
    None
}

/// Fully unwrap `url`, following nested wrappers (a SafeLink wrapping a Google
/// redirect, …) up to a small depth. Returns the input unchanged if it's not a
/// recognized wrapper. Errors only on empty input.
pub fn decode_one(url: &str) -> String {
    let mut cur = url.trim().to_string();
    for _ in 0..8 {
        match unwrap_once(&cur) {
            Some(next) if next != cur && !next.is_empty() => cur = next,
            _ => break,
        }
    }
    cur
}

/// Decode `input`. With `per_line`, each non-empty line is decoded independently.
pub fn decode(input: &str, per_line: bool) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err("input is empty".into());
    }
    if per_line {
        Ok(input
            .lines()
            .map(|l| if l.trim().is_empty() { String::new() } else { decode_one(l) })
            .collect::<Vec<_>>()
            .join("\n"))
    } else {
        Ok(decode_one(input))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unwraps_outlook_safelink() {
        let sl = "https://nam12.safelinks.protection.outlook.com/?url=https%3A%2F%2Fexample.com%2Fpath%3Fa%3D1&data=05%7C01";
        assert_eq!(decode_one(sl), "https://example.com/path?a=1");
    }

    #[test]
    fn unwraps_google_redirect() {
        let g = "https://www.google.com/url?q=https%3A%2F%2Fexample.org%2Fx&sa=D";
        assert_eq!(decode_one(g), "https://example.org/x");
    }

    #[test]
    fn unwraps_proofpoint_v2() {
        // u with - => % and _ => / : "https://example.com/a"
        let pp = "https://urldefense.proofpoint.com/v2/url?u=https-3A__example.com_a&d=Dw";
        assert_eq!(decode_one(pp), "https://example.com/a");
    }

    #[test]
    fn unwraps_nested_safelink_over_google() {
        let inner = "https%3A%2F%2Fwww.google.com%2Furl%3Fq%3Dhttps%253A%252F%252Ffinal.example%252Fz";
        let sl = format!("https://x.safelinks.protection.outlook.com/?url={inner}");
        assert_eq!(decode_one(&sl), "https://final.example/z");
    }

    #[test]
    fn passes_through_plain_url() {
        assert_eq!(decode_one("https://example.com/clean?x=1"), "https://example.com/clean?x=1");
    }

    #[test]
    fn per_line_batch() {
        let inp = "https://nam.safelinks.protection.outlook.com/?url=https%3A%2F%2Fa.com\nhttps://b.com";
        assert_eq!(decode(inp, true).unwrap(), "https://a.com\nhttps://b.com");
    }

    #[test]
    fn empty_errors() {
        assert!(decode("  ", false).is_err());
    }
}

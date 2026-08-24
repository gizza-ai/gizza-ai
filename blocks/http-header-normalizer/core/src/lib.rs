//! http-header-normalizer core — turn a pasted HTTP header block into ONE
//! canonical header block.
//!
//! Header field names are case-insensitive on the wire but string-compared
//! everywhere else, so the same request arrives spelled `Content-Type`,
//! `content-type` and `CONTENT-TYPE`, with ragged spacing after the colon and
//! in a different order every time. This tool rewrites the names to one casing
//! style (canonical `Title-Case` by default, with the exception table that
//! plain title-casing gets wrong — `ETag`, `WWW-Authenticate`, `DNT`, …),
//! trims every value, joins obsolete line folds, collapses repeated names per
//! a chosen policy (`Set-Cookie` is never comma-joined, RFC 6265), applies an
//! allow/deny list, and sorts the result so two captures of the same request
//! diff cleanly. Text in, text out — a `curl -H` form and a CSV summary of the
//! run are the other two outputs.
//!
//! Structured `name → value` JSON is deliberately NOT an output here; that is
//! `blocks/http-header-parser`. Pure Rust, no dependencies, no I/O.

use std::collections::HashMap;

/// Largest input accepted in one run.
pub const MAX_BYTES: usize = 1_000_000;
/// Largest number of input lines accepted in one run.
pub const MAX_LINES: usize = 20_000;

/// Canonical spellings that hyphen-segment title-casing gets wrong.
/// Matched case-insensitively against the lower-cased field name.
const CANONICAL_EXCEPTIONS: &[&str] = &[
    "A-IM",
    "ALPN",
    "Content-DPR",
    "Content-MD5",
    "DNT",
    "DPR",
    "ETag",
    "Expect-CT",
    "IM",
    "Last-Event-ID",
    "MIME-Version",
    "NEL",
    "P3P",
    "Sec-CH-UA",
    "Sec-CH-UA-Mobile",
    "Sec-CH-UA-Platform",
    "Sec-WebSocket-Accept",
    "Sec-WebSocket-Extensions",
    "Sec-WebSocket-Key",
    "Sec-WebSocket-Protocol",
    "Sec-WebSocket-Version",
    "SourceMap",
    "TE",
    "WWW-Authenticate",
    "X-API-Key",
    "X-ATT-DeviceId",
    "X-Correlation-ID",
    "X-CSRF-Token",
    "X-DNS-Prefetch-Control",
    "X-Real-IP",
    "X-Request-ID",
    "X-SourceMap",
    "X-UA-Compatible",
    "X-WebKit-CSP",
    "X-XSS-Protection",
];

/// A parsed field: the case-folded key, the name as first written, and every
/// value seen for it in input order.
struct Field {
    lower: String,
    first_seen: String,
    values: Vec<String>,
}

struct Opts<'a> {
    case: &'a str,
    sort: &'a str,
    duplicates: &'a str,
    unfold: bool,
    drop_empty: bool,
    drop: Vec<String>,
    keep: Vec<String>,
}

/// Counters reported by `output = "summary"`.
#[derive(Default)]
struct Stats {
    lines_in: usize,
    headers_in: usize,
    names_in: usize,
    headers_out: usize,
    names_recased: usize,
    values_trimmed: usize,
    duplicates_merged: usize,
    names_dropped: usize,
    body_lines_skipped: usize,
}

fn pick<'a>(value: &str, allowed: &[&'a str], param: &str) -> Result<&'a str, String> {
    let v = value.trim();
    if v.is_empty() {
        return Ok(allowed[0]);
    }
    allowed
        .iter()
        .find(|a| a.eq_ignore_ascii_case(v))
        .copied()
        .ok_or_else(|| format!("{param} must be one of {} — got '{v}'", allowed.join(", ")))
}

/// Split a comma-separated rule list into lower-cased names; a trailing `*`
/// is kept and read as a prefix rule by [`matches_rule`].
fn parse_rules(csv: &str) -> Vec<String> {
    csv.split(',')
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

fn matches_rule(lower_name: &str, rules: &[String]) -> bool {
    rules.iter().any(|r| match r.strip_suffix('*') {
        Some(prefix) => lower_name.starts_with(prefix),
        None => lower_name == r,
    })
}

/// `content-type` → `Content-Type`, honoring [`CANONICAL_EXCEPTIONS`].
fn canonical_name(lower: &str) -> String {
    if let Some(exact) = CANONICAL_EXCEPTIONS
        .iter()
        .find(|c| c.eq_ignore_ascii_case(lower))
    {
        return (*exact).to_string();
    }
    lower
        .split('-')
        .map(|seg| {
            let mut chars = seg.chars();
            match chars.next() {
                Some(first) => {
                    let mut out = first.to_ascii_uppercase().to_string();
                    out.push_str(&chars.as_str().to_ascii_lowercase());
                    out
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join("-")
}

/// Render one field name in the requested casing. HTTP/2 pseudo-headers
/// (`:method`, `:authority`, …) are always lower-case — the protocol requires it.
fn display_name(case: &str, lower: &str, first_seen: &str) -> String {
    if lower.starts_with(':') {
        return lower.to_string();
    }
    match case {
        "lower" => lower.to_string(),
        "upper" => lower.to_ascii_uppercase(),
        "preserve" => first_seen.to_string(),
        _ => canonical_name(lower),
    }
}

/// RFC 7230 `token` characters, plus a leading `:` for pseudo-headers.
fn valid_name(name: &str) -> bool {
    let body = name.strip_prefix(':').unwrap_or(name);
    !body.is_empty()
        && body.chars().all(|c| {
            c.is_ascii_alphanumeric() || "!#$%&'*+-.^_`|~".contains(c)
        })
}

/// True when the FIRST line of a block is a request line (`GET /p HTTP/1.1`)
/// or a status line (`HTTP/1.1 200 OK`) rather than a header.
fn is_start_line(line: &str) -> bool {
    let t = line.trim();
    if t.is_empty() {
        return false;
    }
    if t.starts_with("HTTP/") {
        return true;
    }
    let parts: Vec<&str> = t.split_whitespace().collect();
    parts.len() >= 3
        && parts[parts.len() - 1].starts_with("HTTP/")
        && !parts[0].is_empty()
        && parts[0].chars().all(|c| c.is_ascii_uppercase())
}

/// Quote a value for a `curl -H '…'` argument (POSIX single-quote escaping).
fn shell_single_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Normalize a pasted HTTP header block.
///
/// * `case` — `canonical` | `lower` | `upper` | `preserve`
/// * `sort` — `name` | `none`
/// * `duplicates` — `combine` | `list` | `first` | `last`
/// * `unfold` — join obsolete continuation lines onto the header they belong to
/// * `drop_empty` — remove fields whose value is empty after trimming
/// * `drop_headers` / `keep_headers` — comma-separated names, `prefix-*` allowed
/// * `output` — `headers` | `curl` | `summary`
#[allow(clippy::too_many_arguments)]
pub fn normalize(
    input: &str,
    case: &str,
    sort: &str,
    duplicates: &str,
    unfold: bool,
    drop_empty: bool,
    drop_headers: &str,
    keep_headers: &str,
    output: &str,
) -> Result<String, String> {
    if input.len() > MAX_BYTES {
        return Err(format!(
            "input is {} bytes — the limit is {MAX_BYTES} bytes per run",
            input.len()
        ));
    }
    if input.trim().is_empty() {
        return Err(
            "input is empty — paste an HTTP header block, one 'Name: value' per line".into(),
        );
    }

    let opts = Opts {
        case: pick(case, &["canonical", "lower", "upper", "preserve"], "case")?,
        sort: pick(sort, &["name", "none"], "sort")?,
        duplicates: pick(
            duplicates,
            &["combine", "list", "first", "last"],
            "duplicates",
        )?,
        unfold,
        drop_empty,
        drop: parse_rules(drop_headers),
        keep: parse_rules(keep_headers),
    };
    let output = pick(output, &["headers", "curl", "summary"], "output")?;

    let text = input.trim_start_matches('\u{feff}');
    let lines: Vec<&str> = text
        .split('\n')
        .map(|l| l.trim_end_matches('\r'))
        .collect();
    if lines.len() > MAX_LINES {
        return Err(format!(
            "input has {} lines — the limit is {MAX_LINES} lines per run",
            lines.len()
        ));
    }

    let mut stats = Stats::default();
    let mut idx = 0usize;
    // Leading blank lines before the block are ignored.
    while idx < lines.len() && lines[idx].trim().is_empty() {
        idx += 1;
    }
    stats.lines_in = lines.iter().filter(|l| !l.trim().is_empty()).count();

    // Optional request/status line, kept verbatim at the top of the output.
    let mut start_line: Option<String> = None;
    if idx < lines.len() && is_start_line(lines[idx]) {
        start_line = Some(lines[idx].trim().to_string());
        idx += 1;
    }

    let mut fields: Vec<Field> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut last: Option<usize> = None;

    while idx < lines.len() {
        let line = lines[idx];
        idx += 1;
        if line.trim().is_empty() {
            // A blank line ends the header block (the head/body split).
            stats.body_lines_skipped = lines[idx..].iter().filter(|l| !l.trim().is_empty()).count();
            break;
        }
        // Obsolete line folding: a continuation line starts with whitespace.
        if line.starts_with(' ') || line.starts_with('\t') {
            let owner = last.ok_or_else(|| {
                format!("line {} is an indented continuation but no header comes before it — remove the leading space or paste the header it belongs to", idx)
            })?;
            let cont = line.trim();
            let value = fields[owner]
                .values
                .last_mut()
                .expect("a field always has at least one value");
            if opts.unfold {
                if !value.is_empty() {
                    value.push(' ');
                }
                value.push_str(cont);
            } else {
                value.push('\n');
                value.push(' ');
                value.push_str(cont);
            }
            continue;
        }

        let (name, raw_value) = if let Some(rest) = line.strip_prefix(':') {
            let (n, v) = rest.split_once(':').ok_or_else(|| {
                format!("line {idx} ('{line}') is not a 'Name: value' header — the colon after the field name is missing")
            })?;
            (format!(":{}", n.trim()), v)
        } else {
            let (n, v) = line.split_once(':').ok_or_else(|| {
                format!("line {idx} ('{line}') is not a 'Name: value' header — the colon after the field name is missing")
            })?;
            (n.trim().to_string(), v)
        };
        if name.trim().is_empty() || name == ":" {
            return Err(format!("line {idx} ('{line}') has an empty header name"));
        }
        if !valid_name(&name) {
            return Err(format!(
                "line {idx}: '{name}' is not a valid header name — names may only use letters, digits and !#$%&'*+-.^_`|~"
            ));
        }
        let value = raw_value.trim().to_string();
        if value.len() != raw_value.len() {
            stats.values_trimmed += 1;
        }
        stats.headers_in += 1;

        let lower = name.to_ascii_lowercase();
        let pos = match index.get(&lower) {
            Some(p) => *p,
            None => {
                fields.push(Field {
                    lower: lower.clone(),
                    first_seen: name.clone(),
                    values: Vec::new(),
                });
                index.insert(lower, fields.len() - 1);
                fields.len() - 1
            }
        };
        fields[pos].values.push(value);
        last = Some(pos);
    }

    if fields.is_empty() {
        return Err(
            "no headers found — each header must be on its own 'Name: value' line".into(),
        );
    }
    stats.names_in = fields.len();

    // Allowlist first, then the deny list, then empty-value removal.
    fields.retain(|f| {
        if !opts.keep.is_empty() && !matches_rule(&f.lower, &opts.keep) {
            return false;
        }
        !matches_rule(&f.lower, &opts.drop)
    });
    if opts.drop_empty {
        for f in fields.iter_mut() {
            f.values.retain(|v| !v.is_empty());
        }
        fields.retain(|f| !f.values.is_empty());
    }
    stats.names_dropped = stats.names_in - fields.len();

    // Sorting: pseudo-headers must precede regular fields (RFC 9113).
    if opts.sort == "name" {
        fields.sort_by(|a, b| {
            let pa = !a.lower.starts_with(':');
            let pb = !b.lower.starts_with(':');
            pa.cmp(&pb).then_with(|| a.lower.cmp(&b.lower))
        });
    }

    // Render each surviving field to its output line(s).
    let mut rendered: Vec<(String, String, bool)> = Vec::new(); // (name, value, is_pseudo)
    for f in &fields {
        let name = display_name(opts.case, &f.lower, &f.first_seen);
        if name != f.first_seen {
            stats.names_recased += 1;
        }
        let pseudo = f.lower.starts_with(':');
        let values: Vec<String> = match opts.duplicates {
            "first" => vec![f.values[0].clone()],
            "last" => vec![f.values[f.values.len() - 1].clone()],
            "list" => f.values.clone(),
            // Set-Cookie must never be comma-joined (RFC 6265) — one line each.
            _ if f.lower == "set-cookie" => f.values.clone(),
            _ => vec![f.values.join(", ")],
        };
        stats.duplicates_merged += f.values.len() - values.len();
        for v in values {
            rendered.push((name.clone(), v, pseudo));
        }
    }
    stats.headers_out = rendered.len();

    match output {
        "summary" => {
            let mut out = String::from("metric,value\n");
            for (k, v) in [
                ("lines_in", stats.lines_in),
                ("headers_in", stats.headers_in),
                ("names_in", stats.names_in),
                ("headers_out", stats.headers_out),
                ("names_recased", stats.names_recased),
                ("values_trimmed", stats.values_trimmed),
                ("duplicates_merged", stats.duplicates_merged),
                ("names_dropped", stats.names_dropped),
                ("body_lines_skipped", stats.body_lines_skipped),
            ] {
                out.push_str(&format!("{k},{v}\n"));
            }
            Ok(out.trim_end().to_string())
        }
        "curl" => {
            let mut out: Vec<String> = Vec::new();
            for (name, value, pseudo) in &rendered {
                // curl builds the pseudo-headers itself from the URL/method.
                if *pseudo {
                    continue;
                }
                out.push(format!("-H {}", shell_single_quote(&format!("{name}: {value}"))));
            }
            if out.is_empty() {
                return Err("nothing left to output — every header was removed by the keep/drop filters (pseudo-headers are not emitted as curl flags)".into());
            }
            Ok(out.join(" \\\n"))
        }
        _ => {
            if rendered.is_empty() {
                return Err(
                    "nothing left to output — every header was removed by the keep/drop filters"
                        .into(),
                );
            }
            let mut out: Vec<String> = Vec::new();
            if let Some(s) = &start_line {
                out.push(s.clone());
            }
            for (name, value, _) in &rendered {
                out.push(format!("{name}: {value}"));
            }
            Ok(out.join("\n"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(input: &str) -> String {
        normalize(input, "", "", "", true, false, "", "", "").unwrap()
    }

    #[test]
    fn canonicalizes_trims_and_sorts() {
        let out = run("host:   example.com\ncontent-type:application/json   \nACCEPT: */*\n");
        assert_eq!(
            out,
            "Accept: */*\nContent-Type: application/json\nHost: example.com"
        );
    }

    #[test]
    fn keeps_the_start_line_on_top() {
        let out = run("GET /index.html HTTP/1.1\nx-custom: 1\nhost: example.com\n");
        assert_eq!(
            out,
            "GET /index.html HTTP/1.1\nHost: example.com\nX-Custom: 1"
        );
        let res = run("HTTP/1.1 200 OK\nserver: nginx\netag: \"abc\"\n");
        assert_eq!(res, "HTTP/1.1 200 OK\nETag: \"abc\"\nServer: nginx");
    }

    #[test]
    fn applies_the_canonical_exception_table() {
        let out = run("etag: x\nwww-authenticate: Basic\ndnt: 1\nte: trailers\nx-xss-protection: 0\ncontent-md5: q\n");
        assert_eq!(
            out,
            "Content-MD5: q\nDNT: 1\nETag: x\nTE: trailers\nWWW-Authenticate: Basic\nX-XSS-Protection: 0"
        );
    }

    #[test]
    fn honors_every_case_style() {
        let input = "Content-Type: text/html\n";
        assert_eq!(
            normalize(input, "lower", "", "", true, false, "", "", "").unwrap(),
            "content-type: text/html"
        );
        assert_eq!(
            normalize(input, "upper", "", "", true, false, "", "", "").unwrap(),
            "CONTENT-TYPE: text/html"
        );
        assert_eq!(
            normalize("cOnTeNt-TyPe: text/html", "preserve", "", "", true, false, "", "", "")
                .unwrap(),
            "cOnTeNt-TyPe: text/html"
        );
    }

    #[test]
    fn duplicate_policies() {
        let input = "Accept: a\nX-Tag: 1\naccept: b\n";
        assert_eq!(
            normalize(input, "", "", "combine", true, false, "", "", "").unwrap(),
            "Accept: a, b\nX-Tag: 1"
        );
        assert_eq!(
            normalize(input, "", "", "list", true, false, "", "", "").unwrap(),
            "Accept: a\nAccept: b\nX-Tag: 1"
        );
        assert_eq!(
            normalize(input, "", "", "first", true, false, "", "", "").unwrap(),
            "Accept: a\nX-Tag: 1"
        );
        assert_eq!(
            normalize(input, "", "", "last", true, false, "", "", "").unwrap(),
            "Accept: b\nX-Tag: 1"
        );
    }

    #[test]
    fn set_cookie_is_never_comma_joined() {
        let out = run("set-cookie: a=1; Path=/\nset-cookie: b=2; Path=/\n");
        assert_eq!(out, "Set-Cookie: a=1; Path=/\nSet-Cookie: b=2; Path=/");
    }

    #[test]
    fn keeps_the_original_order_when_asked() {
        let out = normalize(
            "zeta: 1\nalpha: 2\n",
            "",
            "none",
            "",
            true,
            false,
            "",
            "",
            "",
        )
        .unwrap();
        assert_eq!(out, "Zeta: 1\nAlpha: 2");
    }

    #[test]
    fn unfolds_continuation_lines() {
        let folded = "X-Long: part one\n   part two\nhost: e.com\n";
        assert_eq!(run(folded), "Host: e.com\nX-Long: part one part two");
        assert_eq!(
            normalize(folded, "", "", "", false, false, "", "", "").unwrap(),
            "Host: e.com\nX-Long: part one\n part two"
        );
    }

    #[test]
    fn filters_and_empty_values() {
        let input = "Host: e.com\nX-A: 1\nX-B: 2\nCookie: s=1\nX-Empty:\n";
        assert_eq!(
            normalize(input, "", "", "", true, false, "cookie,x-*", "", "").unwrap(),
            "Host: e.com"
        );
        assert_eq!(
            normalize(input, "", "", "", true, false, "", "host,x-a", "").unwrap(),
            "Host: e.com\nX-A: 1"
        );
        assert_eq!(
            normalize(input, "", "", "", true, true, "", "x-*", "").unwrap(),
            "X-A: 1\nX-B: 2"
        );
    }

    #[test]
    fn pseudo_headers_sort_first_and_stay_lowercase() {
        let out = run("Host: e.com\n:METHOD: GET\n:path: /a\n");
        assert_eq!(out, ":method: GET\n:path: /a\nHost: e.com");
    }

    #[test]
    fn curl_output_quotes_values() {
        let out = normalize(
            "accept: */*\nx-note: it's fine\n:method: GET\n",
            "",
            "",
            "",
            true,
            false,
            "",
            "",
            "curl",
        )
        .unwrap();
        assert_eq!(out, "-H 'Accept: */*' \\\n-H 'X-Note: it'\\''s fine'");
    }

    #[test]
    fn summary_counts_the_run() {
        let out = normalize(
            "GET / HTTP/1.1\nhost:  e.com \naccept: a\nACCEPT: b\n\nbody line\n",
            "",
            "",
            "",
            true,
            false,
            "",
            "",
            "summary",
        )
        .unwrap();
        assert!(out.starts_with("metric,value\n"), "{out}");
        assert!(out.contains("headers_in,3"), "{out}");
        assert!(out.contains("names_in,2"), "{out}");
        assert!(out.contains("duplicates_merged,1"), "{out}");
        assert!(out.contains("body_lines_skipped,1"), "{out}");
        assert!(out.contains("names_recased,2"), "{out}");
    }

    #[test]
    fn rejects_a_line_without_a_colon() {
        let err = normalize("Host: e.com\nnot a header\n", "", "", "", true, false, "", "", "")
            .unwrap_err();
        assert!(err.contains("not a 'Name: value' header"), "{err}");
    }

    #[test]
    fn rejects_an_invalid_name_and_empty_input() {
        let err = normalize("Bad Name: 1", "", "", "", true, false, "", "", "").unwrap_err();
        assert!(err.contains("not a valid header name"), "{err}");
        let err = normalize("   ", "", "", "", true, false, "", "", "").unwrap_err();
        assert!(err.contains("input is empty"), "{err}");
    }

    #[test]
    fn rejects_a_bad_enum_value_and_an_oversized_input() {
        let err = normalize("Host: e.com", "sideways", "", "", true, false, "", "", "").unwrap_err();
        assert!(err.contains("case must be one of"), "{err}");
        let big = format!("X-A: {}", "a".repeat(MAX_BYTES));
        let err = normalize(&big, "", "", "", true, false, "", "", "").unwrap_err();
        assert!(err.contains("the limit is 1000000 bytes"), "{err}");
    }

    #[test]
    fn errors_when_every_header_is_filtered_out() {
        let err = normalize("Host: e.com", "", "", "", true, false, "host", "", "").unwrap_err();
        assert!(err.contains("nothing left to output"), "{err}");
    }
}

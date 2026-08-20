//! url-query-normalizer core — pure compute, shared by the chat skill block and
//! the web page. No wafer/wasm-bindgen deps. Canonicalizes the QUERY STRING of a
//! URL: sorts parameters, collapses duplicates, and normalizes percent-encoding,
//! with optional tracking-parameter removal and allow/deny lists. Everything
//! outside the query — scheme, host, port, path and fragment — is copied through
//! byte-for-byte; path/host canonicalization is a different tool's job.

/// Hard caps so a pasted dump can't wedge the page.
const MAX_LINES: usize = 20_000;
const MAX_BYTES: usize = 1_000_000;

/// Exact tracking parameter names dropped by `drop_tracking` (matched
/// case-insensitively).
const TRACKING_EXACT: &[&str] = &[
    "fbclid", "gclid", "gclsrc", "dclid", "gbraid", "wbraid", "msclkid", "yclid", "ysclid",
    "twclid", "ttclid", "igshid", "mc_eid", "mc_cid", "vero_id", "vero_conv", "oly_anon_id",
    "oly_enc_id", "wickedid", "_openstat", "mkt_tok", "fb_action_ids", "fb_action_types",
    "fb_source", "action_object_map", "action_type_map", "action_ref_map", "spm", "scm",
    "ref_src", "ref_url", "s_kwcid", "ml_subscriber", "ml_subscriber_hash", "trk", "trkcampaign",
    "__hstc", "__hssc", "__hsfp", "hsctatracking", "guccounter", "_hsenc", "_hsmi",
];

/// Tracking parameter name prefixes dropped by `drop_tracking` (analytics
/// families: Google, Matomo/Piwik, HubSpot, Mailchimp, Adobe, Facebook).
const TRACKING_PREFIX: &[&str] = &[
    "utm_", "ga_", "_ga", "pk_", "mtm_", "matomo_", "hsa_", "_hs", "mc_", "oly_", "vero_",
    "icid", "wt_", "cm_", "ns_", "__cft", "__tn",
];

/// RFC 3986 unreserved set plus the sub-delimiters and generic delimiters that
/// are legal (and conventional) inside a query component. `=` is excluded here
/// because a literal `=` in a KEY would re-split the pair on a second pass.
fn key_safe(b: u8) -> bool {
    b.is_ascii_alphanumeric()
        || matches!(
            b,
            b'-' | b'.' | b'_' | b'~' | b'!' | b'$' | b'\'' | b'(' | b')' | b'*' | b',' | b':'
                | b';' | b'@' | b'/' | b'?'
        )
}

/// Values may additionally hold a literal `=` — splitting a pair only ever
/// consumes the first one, so `a=b=c` round-trips unchanged.
fn value_safe(b: u8) -> bool {
    key_safe(b) || b == b'='
}

/// How a space byte should be spelled on the way out.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Space {
    Percent,
    Plus,
}

/// Percent-decode `s` into bytes. A literal `+` decodes to a space (the
/// form-urlencoded convention every browser and server applies to query
/// strings). A `%` that does not introduce two hex digits is kept as a literal
/// `%` byte, so malformed input survives instead of erroring.
fn percent_decode(s: &str) -> Vec<u8> {
    let raw = s.as_bytes();
    let mut out = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        match raw[i] {
            b'%' if i + 2 < raw.len() => {
                let hi = (raw[i + 1] as char).to_digit(16);
                let lo = (raw[i + 2] as char).to_digit(16);
                match (hi, lo) {
                    (Some(h), Some(l)) => {
                        out.push((h * 16 + l) as u8);
                        i += 3;
                    }
                    _ => {
                        out.push(b'%');
                        i += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

/// Re-encode decoded bytes: anything in the safe set stays literal, everything
/// else becomes `%XX` with UPPERCASE hex (RFC 3986 §6.2.2.1). A space follows
/// the chosen `space` spelling.
fn percent_encode(bytes: &[u8], safe: fn(u8) -> bool, space: Space) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(bytes.len());
    for &b in bytes {
        if b == b' ' {
            match space {
                Space::Percent => out.push_str("%20"),
                Space::Plus => out.push('+'),
            }
        } else if safe(b) {
            out.push(b as char);
        } else {
            out.push('%');
            out.push(HEX[(b >> 4) as usize] as char);
            out.push(HEX[(b & 0x0f) as usize] as char);
        }
    }
    out
}

/// Normalize one key or value token: decode, then re-encode canonically.
fn normalize_token(tok: &str, safe: fn(u8) -> bool, space: Space) -> String {
    percent_encode(&percent_decode(tok), safe, space)
}

/// A single parsed query parameter. `key`/`value` are the emitted (possibly
/// normalized) spellings; `name` is the decoded, lowercased key used for
/// matching against the tracking list and the allow/deny lists.
#[derive(Clone)]
struct Pair {
    key: String,
    value: Option<String>,
    name: String,
}

/// Does `name` match one of `rules`? A rule ending in `*` is a prefix match,
/// everything else is an exact (case-insensitive) match.
fn matches_rule(name: &str, rules: &[String]) -> bool {
    rules.iter().any(|r| match r.strip_suffix('*') {
        Some(prefix) => name.starts_with(prefix),
        None => name == r.as_str(),
    })
}

fn is_tracking(name: &str) -> bool {
    TRACKING_EXACT.contains(&name) || TRACKING_PREFIX.iter().any(|p| name.starts_with(p))
}

fn parse_rules(csv: &str) -> Vec<String> {
    csv.split(',')
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Everything the caller chose, resolved once so the per-line loop stays cheap.
struct Opts {
    sort: &'static str,
    dedupe: &'static str,
    normalize_encoding: bool,
    space: Space,
    drop_tracking: bool,
    drop: Vec<String>,
    keep: Vec<String>,
    drop_empty: bool,
}

/// What happened to one input line, for the report and summary outputs.
struct LineStat {
    original: String,
    normalized: String,
    params_in: usize,
    params_out: usize,
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

/// Split a line into (prefix-before-query, query, suffix-from-fragment).
/// Returns `None` when the line carries no query string at all.
fn split_query(line: &str) -> Option<(&str, &str, &str)> {
    let (before_frag, frag) = match line.find('#') {
        Some(i) => (&line[..i], &line[i..]),
        None => (line, ""),
    };
    match before_frag.find('?') {
        // A normal URL (or path) with a `?`.
        Some(i) => Some((&before_frag[..i + 1], &before_frag[i + 1..], frag)),
        // A bare query string pasted on its own: no `?`, no scheme, but it looks
        // like `k=v` pairs. Returned without a leading `?`.
        None if !before_frag.contains("://")
            && !before_frag.is_empty()
            && (before_frag.contains('=') || before_frag.contains('&')) =>
        {
            Some(("", before_frag, frag))
        }
        None => None,
    }
}

/// Normalize the query string of one line. Returns the rewritten line plus the
/// input/output parameter counts.
fn normalize_line(line: &str, o: &Opts) -> (String, usize, usize) {
    let Some((prefix, query, frag)) = split_query(line) else {
        return (line.to_string(), 0, 0);
    };

    let mut pairs: Vec<Pair> = Vec::new();
    let mut params_in = 0usize;
    for seg in query.split('&') {
        if seg.is_empty() {
            continue;
        }
        params_in += 1;
        let (raw_key, raw_value) = match seg.find('=') {
            Some(i) => (&seg[..i], Some(&seg[i + 1..])),
            None => (seg, None),
        };
        let (key, value) = if o.normalize_encoding {
            (
                normalize_token(raw_key, key_safe, o.space),
                raw_value.map(|v| normalize_token(v, value_safe, o.space)),
            )
        } else {
            (raw_key.to_string(), raw_value.map(|v| v.to_string()))
        };
        let name = String::from_utf8_lossy(&percent_decode(raw_key)).to_ascii_lowercase();
        pairs.push(Pair { key, value, name });
    }

    // Filter: allowlist first (when set it is authoritative), then the deny
    // rules, the tracking families, and finally empty values.
    pairs.retain(|p| {
        if !o.keep.is_empty() && !matches_rule(&p.name, &o.keep) {
            return false;
        }
        if matches_rule(&p.name, &o.drop) {
            return false;
        }
        if o.drop_tracking && is_tracking(&p.name) {
            return false;
        }
        if o.drop_empty && p.value.as_deref().unwrap_or("").is_empty() {
            return false;
        }
        true
    });

    // Deduplicate.
    match o.dedupe {
        "exact" => {
            let mut seen: Vec<(String, String)> = Vec::new();
            pairs.retain(|p| {
                let k = (p.key.clone(), p.value.clone().unwrap_or_default());
                if seen.contains(&k) {
                    false
                } else {
                    seen.push(k);
                    true
                }
            });
        }
        "first" => {
            let mut seen: Vec<String> = Vec::new();
            pairs.retain(|p| {
                if seen.contains(&p.key) {
                    false
                } else {
                    seen.push(p.key.clone());
                    true
                }
            });
        }
        "last" => {
            let mut seen: Vec<String> = Vec::new();
            let mut kept: Vec<Pair> = Vec::new();
            for p in pairs.iter().rev() {
                if !seen.contains(&p.key) {
                    seen.push(p.key.clone());
                    kept.push(p.clone());
                }
            }
            kept.reverse();
            pairs = kept;
        }
        _ => {}
    }

    // Sort (stable, so equal sort keys keep their input order).
    match o.sort {
        "key" => pairs.sort_by(|a, b| a.key.cmp(&b.key)),
        "key-value" => pairs.sort_by(|a, b| {
            a.key
                .cmp(&b.key)
                .then_with(|| a.value.as_deref().unwrap_or("").cmp(b.value.as_deref().unwrap_or("")))
        }),
        _ => {}
    }

    let params_out = pairs.len();
    let rebuilt: Vec<String> = pairs
        .iter()
        .map(|p| match &p.value {
            Some(v) => format!("{}={}", p.key, v),
            None => p.key.clone(),
        })
        .collect();

    let mut out = String::with_capacity(line.len());
    if rebuilt.is_empty() {
        // Every parameter was removed: drop the now-pointless `?` too.
        out.push_str(prefix.strip_suffix('?').unwrap_or(prefix));
    } else {
        out.push_str(prefix);
        out.push_str(&rebuilt.join("&"));
    }
    out.push_str(frag);
    (out, params_in, params_out)
}

fn csv_escape(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Normalize every non-blank line of `input`.
///
/// * `sort` — `none` | `key` | `key-value`
/// * `dedupe` — `none` | `exact` | `first` | `last`
/// * `encoding` — `normalize` | `preserve`
/// * `space` — `percent` | `plus` (only consulted when `encoding` normalizes)
/// * `drop_params` / `keep_params` — comma-separated names, `prefix_*` allowed
/// * `output` — `urls` | `changed` | `report` | `summary`
#[allow(clippy::too_many_arguments)]
pub fn normalize(
    input: &str,
    sort: &str,
    dedupe: &str,
    encoding: &str,
    space: &str,
    drop_tracking: bool,
    drop_params: &str,
    keep_params: &str,
    drop_empty: bool,
    output: &str,
) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err("input is empty — paste at least one URL or query string".into());
    }
    if input.len() > MAX_BYTES {
        return Err(format!(
            "input is {} bytes — the limit is {MAX_BYTES} bytes per run",
            input.len()
        ));
    }

    let opts = Opts {
        sort: pick(sort, &["key", "key-value", "none"], "sort")?,
        dedupe: pick(dedupe, &["exact", "first", "last", "none"], "dedupe")?,
        normalize_encoding: pick(encoding, &["normalize", "preserve"], "encoding")? == "normalize",
        space: match pick(space, &["percent", "plus"], "space")? {
            "plus" => Space::Plus,
            _ => Space::Percent,
        },
        drop_tracking,
        drop: parse_rules(drop_params),
        keep: parse_rules(keep_params),
        drop_empty,
    };
    let output = pick(output, &["urls", "changed", "report", "summary"], "output")?;

    let lines: Vec<&str> = input
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    if lines.len() > MAX_LINES {
        return Err(format!(
            "{} lines — the limit is {MAX_LINES} URLs per run",
            lines.len()
        ));
    }

    let stats: Vec<LineStat> = lines
        .iter()
        .map(|l| {
            let (normalized, params_in, params_out) = normalize_line(l, &opts);
            LineStat {
                original: l.to_string(),
                normalized,
                params_in,
                params_out,
            }
        })
        .collect();

    match output {
        "urls" => Ok(stats
            .iter()
            .map(|s| s.normalized.as_str())
            .collect::<Vec<_>>()
            .join("\n")),
        "changed" => {
            let changed: Vec<&str> = stats
                .iter()
                .filter(|s| s.normalized != s.original)
                .map(|s| s.normalized.as_str())
                .collect();
            if changed.is_empty() {
                Ok("Every line was already normalized — nothing changed.".into())
            } else {
                Ok(changed.join("\n"))
            }
        }
        "report" => {
            let mut out = String::from("line,original,normalized,params_in,params_out,changed\n");
            for (i, s) in stats.iter().enumerate() {
                out.push_str(&format!(
                    "{},{},{},{},{},{}\n",
                    i + 1,
                    csv_escape(&s.original),
                    csv_escape(&s.normalized),
                    s.params_in,
                    s.params_out,
                    if s.normalized == s.original { "no" } else { "yes" },
                ));
            }
            Ok(out.trim_end().to_string())
        }
        _ => {
            let params_in: usize = stats.iter().map(|s| s.params_in).sum();
            let params_out: usize = stats.iter().map(|s| s.params_out).sum();
            let changed = stats.iter().filter(|s| s.normalized != s.original).count();
            let with_query = stats.iter().filter(|s| s.params_in > 0).count();
            Ok(format!(
                "metric,value\nlines,{}\nlines_with_query,{}\nlines_changed,{}\nlines_unchanged,{}\nparams_in,{}\nparams_out,{}\nparams_removed,{}",
                stats.len(),
                with_query,
                changed,
                stats.len() - changed,
                params_in,
                params_out,
                params_in - params_out,
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Defaults: sort by key, drop exact duplicates, normalize encoding, spaces
    /// as %20, keep everything else.
    fn d(input: &str) -> String {
        normalize(input, "key", "exact", "normalize", "percent", false, "", "", false, "urls").unwrap()
    }

    #[test]
    fn sorts_params_alphabetically() {
        assert_eq!(d("https://example.com/p?z=1&a=2&m=3"), "https://example.com/p?a=2&m=3&z=1");
    }

    #[test]
    fn drops_exactly_duplicated_pairs_but_keeps_multi_value_keys() {
        assert_eq!(d("https://e.com?tag=a&tag=b&tag=a"), "https://e.com?tag=a&tag=b");
    }

    #[test]
    fn dedupe_first_and_last_pick_one_value_per_key() {
        let u = "https://e.com?b=2&a=1&b=3";
        let first =
            normalize(u, "key", "first", "normalize", "percent", false, "", "", false, "urls").unwrap();
        assert_eq!(first, "https://e.com?a=1&b=2");
        let last =
            normalize(u, "key", "last", "normalize", "percent", false, "", "", false, "urls").unwrap();
        assert_eq!(last, "https://e.com?a=1&b=3");
    }

    #[test]
    fn dedupe_none_keeps_every_repeat() {
        assert_eq!(
            normalize("https://e.com?a=1&a=1", "none", "none", "normalize", "percent", false, "", "", false, "urls")
                .unwrap(),
            "https://e.com?a=1&a=1"
        );
    }

    #[test]
    fn uppercases_hex_and_decodes_unreserved() {
        // %2d is '-' (unreserved → literal); %7e is '~'; %c3%a9 stays encoded but uppercased.
        assert_eq!(d("https://e.com?q=a%2db%7ec%c3%a9"), "https://e.com?q=a-b~c%C3%A9");
    }

    #[test]
    fn encodes_raw_unsafe_characters() {
        assert_eq!(d("https://e.com?q=a b&r=<x>"), "https://e.com?q=a%20b&r=%3Cx%3E");
    }

    #[test]
    fn plus_is_read_as_space_and_respells_per_space_option() {
        assert_eq!(d("https://e.com?q=hello+world"), "https://e.com?q=hello%20world");
        assert_eq!(
            normalize("https://e.com?q=hello%20world", "key", "exact", "normalize", "plus", false, "", "", false, "urls")
                .unwrap(),
            "https://e.com?q=hello+world"
        );
    }

    #[test]
    fn literal_plus_survives_as_percent_2b() {
        assert_eq!(d("https://e.com?q=1%2B1"), "https://e.com?q=1%2B1");
    }

    #[test]
    fn equals_inside_a_value_is_kept_but_encoded_inside_a_key() {
        assert_eq!(d("https://e.com?a=b=c"), "https://e.com?a=b=c");
        assert_eq!(d("https://e.com?a%3Db=1"), "https://e.com?a%3Db=1");
    }

    #[test]
    fn preserve_encoding_leaves_tokens_untouched() {
        assert_eq!(
            normalize("https://e.com?z=a%2db&a=x+y", "key", "exact", "preserve", "percent", false, "", "", false, "urls")
                .unwrap(),
            "https://e.com?a=x+y&z=a%2db"
        );
    }

    #[test]
    fn preserves_path_fragment_and_lines_without_a_query() {
        assert_eq!(d("https://e.com/a%20b/?b=2&a=1#sec%20tion"), "https://e.com/a%20b/?a=1&b=2#sec%20tion");
        assert_eq!(d("https://e.com/path"), "https://e.com/path");
        assert_eq!(d("https://e.com/path#frag"), "https://e.com/path#frag");
    }

    #[test]
    fn bare_query_string_round_trips_without_a_question_mark() {
        assert_eq!(d("z=1&a=2"), "a=2&z=1");
    }

    #[test]
    fn drops_tracking_params_on_request() {
        assert_eq!(
            normalize(
                "https://e.com/p?utm_source=x&id=42&fbclid=abc&pk_campaign=q",
                "key", "exact", "normalize", "percent", true, "", "", false, "urls"
            )
            .unwrap(),
            "https://e.com/p?id=42"
        );
    }

    #[test]
    fn drops_the_question_mark_when_nothing_survives() {
        assert_eq!(
            normalize("https://e.com/p?utm_source=x#top", "key", "exact", "normalize", "percent", true, "", "", false, "urls")
                .unwrap(),
            "https://e.com/p#top"
        );
    }

    #[test]
    fn drop_and_keep_rules_support_a_wildcard() {
        assert_eq!(
            normalize("https://e.com?sid=1&x_a=2&x_b=3&keep=4", "key", "exact", "normalize", "percent", false, "sid,x_*", "", false, "urls")
                .unwrap(),
            "https://e.com?keep=4"
        );
        assert_eq!(
            normalize("https://e.com?page=2&sort=asc&junk=1&session=abc", "key", "exact", "normalize", "percent", false, "", "page,sort", false, "urls")
                .unwrap(),
            "https://e.com?page=2&sort=asc"
        );
    }

    #[test]
    fn drop_empty_removes_valueless_and_blank_params() {
        assert_eq!(
            normalize("https://e.com?a=1&b=&flag", "key", "exact", "normalize", "percent", false, "", "", true, "urls")
                .unwrap(),
            "https://e.com?a=1"
        );
        // Off by default, and a bare flag keeps its bare form.
        assert_eq!(d("https://e.com?b=&flag&a=1"), "https://e.com?a=1&b=&flag");
    }

    #[test]
    fn two_spellings_of_the_same_url_converge() {
        let a = d("https://e.com/p?b=hello+world&a=1&b=hello%20world");
        let b = d("https://e.com/p?a=1&b=hello%20world");
        assert_eq!(a, b);
        assert_eq!(a, "https://e.com/p?a=1&b=hello%20world");
    }

    #[test]
    fn key_value_sort_orders_repeats_by_value() {
        assert_eq!(
            normalize("https://e.com?t=z&t=a&s=1", "key-value", "exact", "normalize", "percent", false, "", "", false, "urls")
                .unwrap(),
            "https://e.com?s=1&t=a&t=z"
        );
    }

    #[test]
    fn batch_normalizes_each_line_independently() {
        let got = d("https://a.com?b=1&a=2\n\nhttps://b.com/no-query\nz=9&y=8");
        assert_eq!(got, "https://a.com?a=2&b=1\nhttps://b.com/no-query\ny=8&z=9");
    }

    #[test]
    fn changed_output_lists_only_rewritten_lines() {
        let got = normalize(
            "https://a.com?a=1&b=2\nhttps://b.com?b=2&a=1",
            "key", "exact", "normalize", "percent", false, "", "", false, "changed",
        )
        .unwrap();
        assert_eq!(got, "https://b.com?a=1&b=2");
    }

    #[test]
    fn changed_output_says_so_when_nothing_moved() {
        let got = normalize("https://a.com?a=1", "key", "exact", "normalize", "percent", false, "", "", false, "changed")
            .unwrap();
        assert!(got.contains("nothing changed"), "{got}");
    }

    #[test]
    fn report_output_is_csv_with_counts() {
        let got = normalize("https://a.com?b=1&a=2&a=2", "key", "exact", "normalize", "percent", false, "", "", false, "report")
            .unwrap();
        let mut lines = got.lines();
        assert_eq!(lines.next().unwrap(), "line,original,normalized,params_in,params_out,changed");
        assert_eq!(
            lines.next().unwrap(),
            "1,https://a.com?b=1&a=2&a=2,https://a.com?a=2&b=1,3,2,yes"
        );
    }

    #[test]
    fn summary_output_counts_removed_params() {
        let got = normalize(
            "https://a.com?utm_source=x&a=1\nhttps://b.com/plain",
            "key", "exact", "normalize", "percent", true, "", "", false, "summary",
        )
        .unwrap();
        assert!(got.contains("lines,2"), "{got}");
        assert!(got.contains("lines_with_query,1"), "{got}");
        assert!(got.contains("params_in,2"), "{got}");
        assert!(got.contains("params_out,1"), "{got}");
        assert!(got.contains("params_removed,1"), "{got}");
    }

    #[test]
    fn malformed_percent_escapes_survive() {
        assert_eq!(d("https://e.com?a=100%&b=%zz"), "https://e.com?a=100%25&b=%25zz");
    }

    #[test]
    fn empty_input_errors() {
        assert!(normalize("   ", "key", "exact", "normalize", "percent", false, "", "", false, "urls").is_err());
    }

    #[test]
    fn bad_enum_values_error_with_the_valid_choices() {
        let err = normalize("a=1", "sideways", "exact", "normalize", "percent", false, "", "", false, "urls")
            .unwrap_err();
        assert!(err.contains("sort must be one of key, key-value, none"), "{err}");
        let err = normalize("a=1", "key", "exact", "normalize", "percent", false, "", "", false, "csv").unwrap_err();
        assert!(err.contains("output must be one of"), "{err}");
    }

    #[test]
    fn blank_enum_values_fall_back_to_the_default() {
        assert_eq!(
            normalize("https://e.com?b=1&a=2", "", "", "", "", false, "", "", false, "").unwrap(),
            "https://e.com?a=2&b=1"
        );
    }

    #[test]
    fn too_many_lines_errors() {
        let big = "a=1\n".repeat(MAX_LINES + 1);
        let err = normalize(&big, "key", "exact", "normalize", "percent", false, "", "", false, "urls").unwrap_err();
        assert!(err.contains("limit is 20000 URLs"), "{err}");
    }
}

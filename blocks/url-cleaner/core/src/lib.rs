//! url-cleaner core — pure compute, shared by the chat skill block and the web
//! page. No wafer/wasm-bindgen deps. Strips tracking/analytics query parameters
//! (utm_*, fbclid, gclid, …) from a URL, preserving everything else (scheme,
//! host, path, fragment, and the remaining query params in their original order
//! and encoding — values are never re-encoded).

/// Exact tracking param names to drop (lowercased match).
const TRACKING_EXACT: &[&str] = &[
    "fbclid", "gclid", "gclsrc", "dclid", "gbraid", "wbraid", "msclkid", "yclid",
    "twclid", "ttclid", "igshid", "mc_eid", "mc_cid", "_hsenc", "_hsmi", "vero_id",
    "vero_conv", "oly_anon_id", "oly_enc_id", "wickedid", "_openstat", "mkt_tok",
    "fb_action_ids", "fb_action_types", "fb_source", "action_object_map",
    "action_type_map", "action_ref_map", "spm", "scm", "ref_src", "ref_url",
    "s_kwcid", "ml_subscriber", "ml_subscriber_hash", "trk", "trkcampaign",
    "ysclid", "__hstc", "__hssc", "__hsfp", "hsctatracking", "guccounter",
];

/// Param-name prefixes to drop (lowercased). Covers analytics families.
const TRACKING_PREFIX: &[&str] = &[
    "utm_", "ga_", "_ga", "pk_", "mtm_", "matomo_", "hsa_", "_hs", "mc_", "oly_",
    "vero_", "icid", "wt_", "cm_", "ns_", "__cft", "__tn",
];

fn is_tracking(name: &str, extra: &[String]) -> bool {
    let lname = name.to_ascii_lowercase();
    if extra.iter().any(|e| e.eq_ignore_ascii_case(name)) {
        return true;
    }
    if TRACKING_EXACT.contains(&lname.as_str()) {
        return true;
    }
    TRACKING_PREFIX.iter().any(|p| lname.starts_with(p))
}

/// Clean one URL: drop tracking query params, keep the rest verbatim. `extra` is
/// a list of additional param names to drop. If the input has no query string it
/// is returned unchanged. A trailing `?` left after removing every param is
/// dropped.
pub fn clean_one(url: &str, extra: &[String]) -> String {
    // Split off the fragment first (everything from the first '#').
    let (before_frag, frag) = match url.find('#') {
        Some(i) => (&url[..i], &url[i..]),
        None => (url, ""),
    };
    // Split base from query at the first '?'.
    let (base, query) = match before_frag.find('?') {
        Some(i) => (&before_frag[..i], &before_frag[i + 1..]),
        None => return url.to_string(),
    };
    let kept: Vec<&str> = query
        .split('&')
        .filter(|seg| {
            if seg.is_empty() {
                return false;
            }
            let name = seg.split('=').next().unwrap_or(seg);
            !is_tracking(name, extra)
        })
        .collect();
    let mut out = String::with_capacity(url.len());
    out.push_str(base);
    if !kept.is_empty() {
        out.push('?');
        out.push_str(&kept.join("&"));
    }
    out.push_str(frag);
    out
}

/// Clean `input`. With `per_line`, each non-empty line is cleaned independently
/// and rejoined with newlines; otherwise the whole input is treated as one URL.
/// `extra_csv` is a comma-separated list of extra param names to drop.
pub fn clean(input: &str, per_line: bool, extra_csv: &str) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err("input is empty".into());
    }
    let extra: Vec<String> = extra_csv
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if per_line {
        Ok(input
            .lines()
            .map(|l| {
                let t = l.trim();
                if t.is_empty() {
                    String::new()
                } else {
                    clean_one(t, &extra)
                }
            })
            .collect::<Vec<_>>()
            .join("\n"))
    } else {
        Ok(clean_one(input.trim(), &extra))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clean1(u: &str) -> String {
        clean(u, false, "").unwrap()
    }

    #[test]
    fn strips_utm_and_known_click_ids() {
        assert_eq!(
            clean1("https://ex.com/p?utm_source=x&utm_medium=y&id=42&fbclid=abc"),
            "https://ex.com/p?id=42"
        );
    }

    #[test]
    fn drops_query_marker_when_all_params_removed() {
        assert_eq!(clean1("https://ex.com/p?utm_source=x&gclid=z"), "https://ex.com/p");
    }

    #[test]
    fn preserves_fragment_and_order_and_encoding() {
        assert_eq!(
            clean1("https://ex.com/a%20b?b=2&utm_term=t&a=1#sec%20tion"),
            "https://ex.com/a%20b?b=2&a=1#sec%20tion"
        );
    }

    #[test]
    fn no_query_returned_unchanged() {
        assert_eq!(clean1("https://ex.com/path#frag"), "https://ex.com/path#frag");
    }

    #[test]
    fn extra_param_names_are_dropped() {
        assert_eq!(
            clean("https://ex.com?keepme=1&sid=9", false, "sid").unwrap(),
            "https://ex.com?keepme=1"
        );
    }

    #[test]
    fn per_line_cleans_each_line() {
        let got = clean("https://a.com?utm_source=x&p=1\nhttps://b.com?gclid=z", true, "").unwrap();
        assert_eq!(got, "https://a.com?p=1\nhttps://b.com");
    }

    #[test]
    fn prefix_families_stripped() {
        assert_eq!(clean1("https://e.com?pk_campaign=a&mtm_source=b&x=1"), "https://e.com?x=1");
    }

    #[test]
    fn strips_yandex_and_facebook_extras() {
        assert_eq!(
            clean1("https://e.com/p?ysclid=9&__cft__[0]=a&__tn__=b&keep=1"),
            "https://e.com/p?keep=1"
        );
    }

    #[test]
    fn empty_input_errors() {
        assert!(clean("   ", false, "").is_err());
    }
}

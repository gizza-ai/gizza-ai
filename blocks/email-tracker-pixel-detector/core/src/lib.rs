//! gizza-ai/email-tracker-pixel-detector core — detect remote email images,
//! tiny/hidden open pixels, known tracking domains, and optional click trackers.
//! Pure Rust, no network I/O.

use std::collections::{BTreeMap, BTreeSet};

use regex::Regex;
use serde::Serialize;

const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputFormat {
    Auto,
    Html,
    Raw,
}

impl InputFormat {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(Self::Auto),
            "html" => Ok(Self::Html),
            "raw" | "eml" | "email" => Ok(Self::Raw),
            other => Err(format!("unknown format '{other}' — use auto, html, or raw")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Report {
    Summary,
    Json,
    Hosts,
}

impl Report {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "summary" | "text" => Ok(Self::Summary),
            "json" => Ok(Self::Json),
            "hosts" | "host" => Ok(Self::Hosts),
            other => Err(format!(
                "unknown report '{other}' — use summary, json, or hosts"
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Asset {
    pub kind: String,
    pub url: String,
    pub host: String,
    pub vendor: Option<String>,
    pub embedded: bool,
    pub signals: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Analysis {
    pub verdict: String,
    pub remote_assets: usize,
    pub trackers: usize,
    pub suspected: usize,
    pub embedded: usize,
    pub hosts: Vec<String>,
    pub assets: Vec<Asset>,
}

fn known_vendors() -> &'static [(&'static str, &'static str)] {
    &[
        ("hubspot.com", "HubSpot"),
        ("hs-scripts.com", "HubSpot"),
        ("getsidekick.com", "HubSpot Sidekick"),
        ("mailchimp.com", "Mailchimp"),
        ("list-manage.com", "Mailchimp"),
        ("sendgrid.net", "SendGrid"),
        ("sendgrid.com", "SendGrid"),
        ("mandrillapp.com", "Mandrill"),
        ("mailgun.org", "Mailgun"),
        ("postmarkapp.com", "Postmark"),
        ("sparkpostmail.com", "SparkPost"),
        ("amazonses.com", "Amazon SES"),
        ("salesforce.com", "Salesforce"),
        ("exacttarget.com", "Salesforce Marketing Cloud"),
        ("pardot.com", "Pardot"),
        ("marketo.com", "Marketo"),
        ("mktoresp.com", "Marketo"),
        ("eloqua.com", "Eloqua"),
        ("constantcontact.com", "Constant Contact"),
        ("activecampaign.com", "ActiveCampaign"),
        ("convertkit-mail.com", "ConvertKit"),
        ("mailerlite.com", "MailerLite"),
        ("klaviyo.com", "Klaviyo"),
        ("intercom.io", "Intercom"),
        ("drip.com", "Drip"),
        ("customer.io", "Customer.io"),
        ("braze.com", "Braze"),
        ("iterable.com", "Iterable"),
        ("sendinblue.com", "Brevo"),
        ("mailjet.com", "Mailjet"),
        ("outreach.io", "Outreach"),
        ("salesloft.com", "Salesloft"),
        ("yesware.com", "Yesware"),
        ("mixmax.com", "Mixmax"),
        ("mailtrack.io", "Mailtrack"),
        ("streak.com", "Streak"),
        ("tinyletter.com", "TinyLetter"),
        ("substack.com", "Substack"),
        ("campaignmonitor.com", "Campaign Monitor"),
        ("createsend.com", "Campaign Monitor"),
    ]
}

fn split_custom_vendors(s: &str) -> Vec<String> {
    s.split([',', ';', '\n', '\t', ' '])
        .map(|p| p.trim().trim_start_matches('.').to_ascii_lowercase())
        .filter(|p| !p.is_empty())
        .collect()
}

fn attr_map(tag_attrs: &str) -> BTreeMap<String, String> {
    let re = Regex::new(
        r#"(?is)([a-zA-Z_:][-a-zA-Z0-9_:.]*)\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s"'=<>`]+))"#,
    )
    .unwrap();
    let mut out = BTreeMap::new();
    for cap in re.captures_iter(tag_attrs) {
        let key = cap[1].to_ascii_lowercase();
        let val = cap
            .get(2)
            .or_else(|| cap.get(3))
            .or_else(|| cap.get(4))
            .map(|m| m.as_str())
            .unwrap_or("");
        out.insert(key, html_unescape(val));
    }
    out
}

fn html_unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#34;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

fn is_remote(url: &str) -> bool {
    let u = url.trim().to_ascii_lowercase();
    u.starts_with("http://") || u.starts_with("https://") || u.starts_with("//")
}

fn is_embedded(url: &str) -> bool {
    let u = url.trim().to_ascii_lowercase();
    u.starts_with("cid:") || u.starts_with("data:")
}

fn host_of(url: &str) -> String {
    let mut u = url.trim();
    if let Some(rest) = u.strip_prefix("https://") {
        u = rest;
    } else if let Some(rest) = u.strip_prefix("http://") {
        u = rest;
    } else if let Some(rest) = u.strip_prefix("//") {
        u = rest;
    }
    let end = u.find(['/', '?', '#']).unwrap_or(u.len());
    u[..end]
        .split('@')
        .last()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
}

fn path_query(url: &str) -> String {
    let u = url.to_ascii_lowercase();
    let start = if let Some(p) = u.find("://") {
        p + 3
    } else if u.starts_with("//") {
        2
    } else {
        0
    };
    let rest = &u[start..];
    rest.find(['/', '?', '#'])
        .map(|p| rest[p..].to_string())
        .unwrap_or_default()
}

fn vendor_for(host: &str, custom: &[String]) -> Option<String> {
    let h = host.trim_start_matches("www.");
    for d in custom {
        if h == d || h.ends_with(&format!(".{d}")) {
            return Some(format!("custom:{d}"));
        }
    }
    for (domain, vendor) in known_vendors() {
        if h == *domain || h.ends_with(&format!(".{domain}")) {
            return Some((*vendor).to_string());
        }
    }
    None
}

fn number_attr(attrs: &BTreeMap<String, String>, key: &str) -> Option<u32> {
    attrs.get(key)?.trim().trim_end_matches("px").parse().ok()
}

fn style_dimension(style: &str, key: &str) -> Option<u32> {
    let re = Regex::new(&format!(r"(?i){}\s*:\s*(\d+)\s*px?", regex::escape(key))).unwrap();
    re.captures(style)?.get(1)?.as_str().parse().ok()
}

fn tiny_or_hidden(attrs: &BTreeMap<String, String>) -> Vec<String> {
    let mut out = Vec::new();
    let style = attrs
        .get("style")
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    let w = number_attr(attrs, "width").or_else(|| style_dimension(&style, "width"));
    let h = number_attr(attrs, "height").or_else(|| style_dimension(&style, "height"));
    if matches!((w, h), (Some(a), Some(b)) if a <= 2 && b <= 2)
        || matches!(w, Some(0..=2))
        || matches!(h, Some(0..=2))
    {
        out.push("tiny-pixel".to_string());
    }
    if style.contains("display:none")
        || style.contains("display: none")
        || style.contains("visibility:hidden")
        || style.contains("visibility: hidden")
        || style.contains("opacity:0")
        || style.contains("opacity: 0")
        || style.contains("left:-")
        || style.contains("top:-")
        || style.contains("position:absolute")
            && (style.contains("-999") || style.contains("clip:"))
    {
        out.push("hidden-by-css".to_string());
    }
    out
}

fn url_signals(url: &str) -> Vec<String> {
    let pq = path_query(url);
    let mut out = Vec::new();
    if [
        "/track",
        "track/",
        "open",
        "pixel",
        "beacon",
        "collect",
        "analytics",
        "click",
        "redirect",
        "utm_",
        "__",
    ]
    .iter()
    .any(|p| pq.contains(p))
    {
        out.push("tracking-path".to_string());
    }
    if pq.contains('?') {
        let suspicious_key = [
            "e=",
            "email=",
            "recipient=",
            "rid=",
            "uid=",
            "user_id=",
            "contact_id=",
            "subscriber=",
            "mid=",
            "message_id=",
            "token=",
            "uuid=",
            "guid=",
            "hash=",
        ];
        if suspicious_key.iter().any(|k| pq.contains(k)) || has_long_query_value(&pq) {
            out.push("unique-id-query".to_string());
        }
    }
    out
}

fn has_long_query_value(pq: &str) -> bool {
    let Some(q) = pq.split_once('?').map(|(_, q)| q) else {
        return false;
    };
    q.split('&')
        .filter_map(|kv| kv.split_once('=').map(|(_, v)| v))
        .any(|v| {
            let n = v.chars().filter(|c| c.is_ascii_alphanumeric()).count();
            n >= 18
        })
}

fn first_srcset_url(srcset: &str) -> String {
    srcset
        .split(',')
        .next()
        .unwrap_or("")
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_string()
}

fn add_asset(
    assets: &mut Vec<Asset>,
    kind: &str,
    url: &str,
    attrs: &BTreeMap<String, String>,
    custom: &[String],
    extra: &[&str],
) {
    let url = url.trim();
    if url.is_empty() {
        return;
    }
    let embedded = is_embedded(url);
    let remote = is_remote(url);
    let host = if remote { host_of(url) } else { String::new() };
    let vendor = if remote {
        vendor_for(&host, custom)
    } else {
        None
    };
    let mut signals = Vec::new();
    if vendor.is_some() {
        signals.push("known-tracker-domain".to_string());
    }
    if remote {
        signals.extend(url_signals(url));
    }
    signals.extend(tiny_or_hidden(attrs));
    signals.extend(extra.iter().map(|s| (*s).to_string()));
    signals.sort();
    signals.dedup();
    assets.push(Asset {
        kind: kind.to_string(),
        url: url.to_string(),
        host,
        vendor,
        embedded,
        signals,
    });
}

fn css_urls(s: &str) -> Vec<String> {
    let re = Regex::new(r#"(?is)url\(\s*['\"]?([^'\")]+)['\"]?\s*\)"#).unwrap();
    re.captures_iter(s)
        .filter_map(|c| c.get(1).map(|m| html_unescape(m.as_str().trim())))
        .collect()
}

fn scan_assets(text: &str, include_links: bool, custom: &[String]) -> Vec<Asset> {
    let mut assets = Vec::new();
    let tag_re = Regex::new(r"(?is)<\s*(img|link|a)\b([^>]*)>").unwrap();
    for cap in tag_re.captures_iter(text) {
        let tag = cap[1].to_ascii_lowercase();
        let attrs = attr_map(&cap[2]);
        match tag.as_str() {
            "img" => {
                if let Some(src) = attrs.get("src") {
                    add_asset(&mut assets, "image", src, &attrs, custom, &[]);
                }
                if let Some(srcset) = attrs.get("srcset") {
                    let u = first_srcset_url(srcset);
                    add_asset(&mut assets, "image-srcset", &u, &attrs, custom, &[]);
                }
            }
            "link" => {
                let rel = attrs
                    .get("rel")
                    .map(|s| s.to_ascii_lowercase())
                    .unwrap_or_default();
                if (rel.contains("prefetch")
                    || rel.contains("preload")
                    || rel.contains("prerender"))
                    && attrs.get("href").is_some()
                {
                    let extra = if rel.contains("prefetch") {
                        "prefetch-beacon"
                    } else {
                        "preload-beacon"
                    };
                    add_asset(
                        &mut assets,
                        "prefetch",
                        attrs.get("href").unwrap(),
                        &attrs,
                        custom,
                        &[extra],
                    );
                }
            }
            "a" if include_links => {
                if let Some(href) = attrs.get("href") {
                    let lower = href.to_ascii_lowercase();
                    if is_remote(href)
                        && (vendor_for(&host_of(href), custom).is_some()
                            || lower.contains("track")
                            || lower.contains("click")
                            || lower.contains("redirect"))
                    {
                        add_asset(
                            &mut assets,
                            "link",
                            href,
                            &attrs,
                            custom,
                            &["click-tracker"],
                        );
                    }
                }
            }
            _ => {}
        }
        if let Some(style) = attrs.get("style") {
            for u in css_urls(style) {
                add_asset(
                    &mut assets,
                    "css-background",
                    &u,
                    &attrs,
                    custom,
                    &["css-background-image"],
                );
            }
        }
    }
    let style_re = Regex::new(r"(?is)<style\b[^>]*>(.*?)</style>").unwrap();
    let empty = BTreeMap::new();
    for cap in style_re.captures_iter(text) {
        for u in css_urls(&cap[1]) {
            add_asset(
                &mut assets,
                "css-background",
                &u,
                &empty,
                custom,
                &["css-background-image"],
            );
        }
    }
    assets
}

pub fn analyze(
    text: &str,
    input_format: InputFormat,
    include_links: bool,
    custom_vendors: &str,
) -> Result<Analysis, String> {
    if text.trim().is_empty() {
        return Err("text is empty — paste raw email source or HTML".into());
    }
    if text.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is too large ({} bytes max)",
            MAX_INPUT_BYTES
        ));
    }
    let custom = split_custom_vendors(custom_vendors);
    let body = match input_format {
        InputFormat::Auto | InputFormat::Html | InputFormat::Raw => text,
    };
    let assets = scan_assets(body, include_links, &custom);
    let remote_assets = assets.iter().filter(|a| !a.host.is_empty()).count();
    let embedded = assets.iter().filter(|a| a.embedded).count();
    let trackers = assets
        .iter()
        .filter(|a| {
            a.vendor.is_some()
                || a.signals
                    .iter()
                    .any(|s| s == "tiny-pixel" || s == "known-tracker-domain")
        })
        .count();
    let suspected = assets
        .iter()
        .filter(|a| a.vendor.is_none() && !a.signals.is_empty() && !a.host.is_empty())
        .count();
    let hosts_set: BTreeSet<String> = assets
        .iter()
        .filter(|a| !a.host.is_empty())
        .map(|a| a.host.clone())
        .collect();
    let verdict = if trackers > 0 {
        "TRACKED"
    } else if suspected > 0 {
        "LIKELY_TRACKED"
    } else if remote_assets > 0 {
        "REMOTE_CONTENT"
    } else {
        "CLEAN"
    };
    Ok(Analysis {
        verdict: verdict.to_string(),
        remote_assets,
        trackers,
        suspected,
        embedded,
        hosts: hosts_set.into_iter().collect(),
        assets,
    })
}

fn render_summary(a: &Analysis) -> String {
    let mut out = String::new();
    out.push_str(&format!("Verdict: {}\n", a.verdict));
    out.push_str(&format!(
        "Remote assets: {} (trackers: {}, suspected: {}, embedded: {})\n",
        a.remote_assets, a.trackers, a.suspected, a.embedded
    ));
    if !a.hosts.is_empty() {
        out.push_str(&format!(
            "Hosts contacted on open: {}\n",
            a.hosts.join(", ")
        ));
    }
    if a.assets.is_empty() {
        out.push_str("No images, prefetch assets, or tracked links found.\n");
        return out;
    }
    out.push_str("\nFindings:\n");
    for (i, asset) in a.assets.iter().enumerate() {
        let label = if asset.embedded {
            "embedded"
        } else if asset.host.is_empty() {
            "local"
        } else {
            asset.host.as_str()
        };
        let mut bits = asset.signals.clone();
        if let Some(v) = &asset.vendor {
            bits.push(format!("vendor={v}"));
        }
        let sig = if bits.is_empty() {
            "no tracking signal".to_string()
        } else {
            bits.join(", ")
        };
        out.push_str(&format!(
            "{}. {} {} — {}\n   {}\n",
            i + 1,
            asset.kind,
            label,
            sig,
            asset.url
        ));
    }
    out
}

pub fn run(
    text: &str,
    format: &str,
    report: &str,
    include_links: bool,
    vendors: &str,
) -> Result<String, String> {
    let input_format = InputFormat::parse(format)?;
    let report = Report::parse(report)?;
    let analysis = analyze(text, input_format, include_links, vendors)?;
    match report {
        Report::Summary => Ok(render_summary(&analysis)),
        Report::Hosts => Ok(analysis.hosts.join("\n")),
        Report::Json => serde_json::to_string_pretty(&analysis).map_err(|e| e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_vendor_tiny_pixel() {
        let html = r#"<img src="https://track.hubspot.com/open.gif?email=a@example.com&id=abc123456789abcdef" width="1" height="1" style="display:none">"#;
        let out = run(html, "html", "summary", false, "").unwrap();
        assert!(out.contains("Verdict: TRACKED"));
        assert!(out.contains("known-tracker-domain"));
        assert!(out.contains("tiny-pixel"));
    }

    #[test]
    fn clean_embedded_image_is_not_remote() {
        let out = analyze(
            r#"<img src="cid:logo@example" width="600" height="80">"#,
            InputFormat::Html,
            false,
            "",
        )
        .unwrap();
        assert_eq!(out.verdict, "CLEAN");
        assert_eq!(out.embedded, 1);
        assert_eq!(out.remote_assets, 0);
    }

    #[test]
    fn custom_vendor_flags_host() {
        let out = run(
            r#"<img src="https://img.example.test/pixel.gif" width="10" height="10">"#,
            "html",
            "hosts",
            false,
            "example.test",
        )
        .unwrap();
        assert_eq!(out.trim(), "img.example.test");
        let json = run(
            r#"<img src="https://img.example.test/pixel.gif">"#,
            "html",
            "json",
            false,
            "example.test",
        )
        .unwrap();
        assert!(json.contains("custom:example.test"));
    }

    #[test]
    fn rejects_bad_enum() {
        let err = run("<img src=x>", "xml", "summary", false, "").unwrap_err();
        assert!(err.contains("unknown format"));
    }

    #[test]
    fn optional_click_trackers() {
        let html =
            r#"<a href="https://mailchimp.com/track/click?u=abcdef0123456789abcdef">read</a>"#;
        let off = analyze(html, InputFormat::Html, false, "").unwrap();
        assert_eq!(off.assets.len(), 0);
        let on = analyze(html, InputFormat::Html, true, "").unwrap();
        assert_eq!(on.verdict, "TRACKED");
        assert_eq!(on.assets[0].kind, "link");
    }
}

//! sitemap-url-extractor core — parse an XML sitemap (<urlset>) or a sitemap
//! index (<sitemapindex>) and extract every URL from its <loc> elements, plus
//! the optional <lastmod> date. Pure-Rust (`quick-xml`); no I/O.
//!
//! Handles both document kinds transparently: a `<urlset>` lists page URLs
//! (each `<url><loc>…</loc><lastmod>…</lastmod></url>`), a `<sitemapindex>`
//! lists child sitemap URLs (each `<sitemap><loc>…</loc><lastmod>…</lastmod>`).
//! In both, the URLs live in `<loc>` and the date in the sibling `<lastmod>`,
//! so a single `<loc>`/`<lastmod>` scan covers both — we just record which kind
//! the root element declared. Namespace prefixes (e.g. `ns:loc`) are stripped.

use quick_xml::events::Event;
use quick_xml::Reader;
use serde::Serialize;

/// One extracted entry: its URL and (optional) last-modified date.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Entry {
    pub loc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lastmod: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Extracted {
    /// "urlset" (page sitemap), "sitemapindex" (index of sitemaps), or
    /// "unknown" when the root element is neither (URLs still extracted).
    pub kind: String,
    pub count: usize,
    pub entries: Vec<Entry>,
}

fn local_name(name: &[u8]) -> &[u8] {
    match name.iter().rposition(|&b| b == b':') {
        Some(i) => &name[i + 1..],
        None => name,
    }
}

/// Parse a sitemap / sitemap-index XML document and extract its `<loc>` URLs
/// (each paired with the `<lastmod>` from the same `<url>`/`<sitemap>` group).
pub fn extract(xml: &str) -> Result<Extracted, String> {
    if xml.trim().is_empty() {
        return Err("input is empty — paste sitemap XML".into());
    }
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut kind = "unknown".to_string();
    let mut root_seen = false;
    let mut entries: Vec<Entry> = Vec::new();

    // The current entry being assembled. `<loc>` opens nothing on its own; we
    // collect text of whichever <loc>/<lastmod> is currently open, then flush a
    // pending entry when a new <loc> appears or the enclosing group closes.
    let mut cur_loc: Option<String> = None;
    let mut cur_lastmod: Option<String> = None;
    // Which leaf element's text we are currently inside.
    let mut in_loc = false;
    let mut in_lastmod = false;

    let flush = |entries: &mut Vec<Entry>, loc: &mut Option<String>, lastmod: &mut Option<String>| {
        if let Some(l) = loc.take() {
            let l = l.trim().to_string();
            if !l.is_empty() {
                entries.push(Entry { loc: l, lastmod: lastmod.take() });
            }
        }
        *lastmod = None;
    };

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Err(e) => return Err(format!("invalid XML: {e}")),
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                let ln = local_name(e.name().as_ref()).to_vec();
                if !root_seen {
                    root_seen = true;
                    match ln.as_slice() {
                        b"urlset" => kind = "urlset".into(),
                        b"sitemapindex" => kind = "sitemapindex".into(),
                        _ => {}
                    }
                }
                match ln.as_slice() {
                    b"url" | b"sitemap" => {
                        // New group: flush any dangling entry first.
                        flush(&mut entries, &mut cur_loc, &mut cur_lastmod);
                    }
                    b"loc" => {
                        // A second <loc> without an intervening group close: flush.
                        if cur_loc.is_some() {
                            flush(&mut entries, &mut cur_loc, &mut cur_lastmod);
                        }
                        cur_loc = Some(String::new());
                        in_loc = true;
                    }
                    b"lastmod" => {
                        cur_lastmod = Some(String::new());
                        in_lastmod = true;
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(t)) => {
                let txt = t
                    .unescape()
                    .map_err(|e| format!("invalid XML text: {e}"))?
                    .into_owned();
                if in_loc {
                    if let Some(s) = cur_loc.as_mut() {
                        s.push_str(&txt);
                    }
                } else if in_lastmod {
                    if let Some(s) = cur_lastmod.as_mut() {
                        s.push_str(&txt);
                    }
                }
            }
            Ok(Event::End(e)) => {
                let ln = local_name(e.name().as_ref()).to_vec();
                match ln.as_slice() {
                    b"loc" => in_loc = false,
                    b"lastmod" => {
                        in_lastmod = false;
                        if let Some(s) = cur_lastmod.as_mut() {
                            let trimmed = s.trim().to_string();
                            *s = trimmed;
                        }
                    }
                    b"url" | b"sitemap" => {
                        flush(&mut entries, &mut cur_loc, &mut cur_lastmod);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        buf.clear();
    }
    // Flush a trailing entry (malformed/unclosed group).
    flush(&mut entries, &mut cur_loc, &mut cur_lastmod);

    Ok(Extracted { kind, count: entries.len(), entries })
}

/// Plain-text render for the page + CLI: one URL per line, with the lastmod date
/// appended in a tab-separated column when present.
pub fn render(xml: &str) -> Result<String, String> {
    let r = extract(xml)?;
    if r.count == 0 {
        return Ok("No URLs found in the sitemap.".to_string());
    }
    let mut out = format!("{} URL(s) [{}]:\n", r.count, r.kind);
    for e in &r.entries {
        match &e.lastmod {
            Some(d) if !d.is_empty() => out.push_str(&format!("{}\t{}\n", e.loc, d)),
            _ => out.push_str(&format!("{}\n", e.loc)),
        }
    }
    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const URLSET: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url>
    <loc>https://example.com/</loc>
    <lastmod>2026-01-01</lastmod>
    <changefreq>daily</changefreq>
  </url>
  <url>
    <loc>https://example.com/about</loc>
  </url>
</urlset>"#;

    const INDEX: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <sitemap>
    <loc>https://example.com/sitemap1.xml</loc>
    <lastmod>2026-02-02T10:00:00+00:00</lastmod>
  </sitemap>
  <sitemap>
    <loc>https://example.com/sitemap2.xml</loc>
  </sitemap>
</sitemapindex>"#;

    #[test]
    fn parses_urlset_with_and_without_lastmod() {
        let r = extract(URLSET).unwrap();
        assert_eq!(r.kind, "urlset");
        assert_eq!(r.count, 2);
        assert_eq!(r.entries[0].loc, "https://example.com/");
        assert_eq!(r.entries[0].lastmod.as_deref(), Some("2026-01-01"));
        assert_eq!(r.entries[1].loc, "https://example.com/about");
        assert_eq!(r.entries[1].lastmod, None);
    }

    #[test]
    fn parses_sitemap_index() {
        let r = extract(INDEX).unwrap();
        assert_eq!(r.kind, "sitemapindex");
        assert_eq!(r.count, 2);
        assert_eq!(r.entries[0].loc, "https://example.com/sitemap1.xml");
        assert_eq!(r.entries[0].lastmod.as_deref(), Some("2026-02-02T10:00:00+00:00"));
        assert_eq!(r.entries[1].lastmod, None);
    }

    #[test]
    fn handles_namespace_prefixes() {
        let xml = r#"<ns:urlset xmlns:ns="x"><ns:url><ns:loc>https://a.test/p</ns:loc></ns:url></ns:urlset>"#;
        let r = extract(xml).unwrap();
        assert_eq!(r.kind, "urlset");
        assert_eq!(r.count, 1);
        assert_eq!(r.entries[0].loc, "https://a.test/p");
    }

    #[test]
    fn empty_input_errors() {
        assert!(extract("   ").is_err());
    }

    #[test]
    fn invalid_xml_errors() {
        assert!(extract("<urlset><url><loc>x</loc>").is_ok()); // unclosed flushes leniently
        assert!(extract("<<<not xml").is_err());
    }

    #[test]
    fn render_no_urls() {
        let r = render("<urlset></urlset>").unwrap();
        assert_eq!(r, "No URLs found in the sitemap.");
    }

    #[test]
    fn render_includes_lastmod_column() {
        let r = render(URLSET).unwrap();
        assert!(r.contains("2 URL(s) [urlset]:"));
        assert!(r.contains("https://example.com/\t2026-01-01"));
        assert!(r.contains("https://example.com/about"));
    }
}

//! Sitemap / robots / JSON-LD helpers.

/// Build a sitemap listing the apex site and every tool subdomain.
pub fn sitemap(subdomains: &[String]) -> String {
    let mut urls = String::from("  <url><loc>https://gizza.ai/</loc></url>\n");
    for s in subdomains {
        urls.push_str(&format!("  <url><loc>https://{s}.gizza.ai/</loc></url>\n"));
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n{urls}</urlset>\n"
    )
}

/// robots.txt allowing all and pointing at the sitemap.
pub fn robots() -> String {
    "User-agent: *\nAllow: /\nSitemap: https://gizza.ai/sitemap.xml\n".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sitemap_lists_apex_and_subdomains() {
        let xml = sitemap(&["calculator".into(), "clock".into()]);
        assert!(xml.contains("<loc>https://gizza.ai/</loc>"));
        assert!(xml.contains("<loc>https://calculator.gizza.ai/</loc>"));
        assert!(xml.contains("<loc>https://clock.gizza.ai/</loc>"));
        assert!(xml.starts_with("<?xml"));
    }

    #[test]
    fn robots_points_at_sitemap() {
        let txt = robots();
        assert!(txt.contains("Sitemap: https://gizza.ai/sitemap.xml"));
        assert!(txt.contains("User-agent: *"));
    }
}

//! Site-level theming for rendered pages. Loaded from `--site-config` TOML;
//! the default renders fully generic, unbranded pages. All branding
//! (canonical origin, title suffix, header/footer chrome, favicons) is data.

use serde::Deserialize;
use std::path::Path;

#[derive(Clone, Debug, Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
pub struct SiteConfig {
    /// Absolute origin (no trailing slash), e.g. "https://gizza.ai".
    /// Empty ⇒ no canonical/og:url/JSON-LD publisher/feed link.
    pub base_url: String,
    /// Brand display name for JSON-LD publisher + OG card corner text.
    pub brand_name: String,
    /// Appended verbatim to every <title>, e.g. " — gizza.ai".
    pub title_suffix: String,
    /// HTML fragment files, relative to the config file's directory.
    pub header_html: String,
    pub footer_html: String,
    pub head_extras_html: String,
    #[serde(skip)]
    pub header: String,
    #[serde(skip)]
    pub footer: String,
    #[serde(skip)]
    pub head_extras: String,
}

pub const GENERIC_HEADER: &str = r#"<header class="tool-nav"><a class="tool-brand" href="/tools/">Tools</a></header>"#;
pub const GENERIC_FOOTER: &str = r#"<footer class="tool-footer"><p>Free, private, in-browser tools — everything runs locally, nothing is uploaded.</p></footer>"#;

impl SiteConfig {
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("read {}: {e}", path.display()))?;
        let mut cfg: SiteConfig =
            toml::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))?;
        let dir = path.parent().unwrap_or(Path::new("."));
        let frag = |rel: &str| -> Result<String, String> {
            if rel.is_empty() {
                return Ok(String::new());
            }
            std::fs::read_to_string(dir.join(rel))
                .map_err(|e| format!("read fragment {rel}: {e}"))
        };
        cfg.header = frag(&cfg.header_html)?;
        cfg.footer = frag(&cfg.footer_html)?;
        cfg.head_extras = frag(&cfg.head_extras_html)?;
        Ok(cfg)
    }

    /// Absolute URL for `path` (must start with '/'), or None when unbranded.
    pub fn abs(&self, path: &str) -> Option<String> {
        (!self.base_url.is_empty()).then(|| format!("{}{}", self.base_url, path))
    }

    /// href for links that are absolute on the branded site, relative otherwise.
    pub fn url_or_rel(&self, path: &str) -> String {
        self.abs(path).unwrap_or_else(|| path.to_string())
    }

    pub fn title(&self, base: &str) -> String {
        format!("{base}{}", self.title_suffix)
    }

    /// OG-card corner label, e.g. "gizza.ai/tools/x" or "tools/x".
    pub fn og_label(&self, path_no_slash: &str) -> String {
        match self.base_url.split("://").nth(1) {
            Some(host) if !host.is_empty() => format!("{host}/{path_no_slash}"),
            _ => path_no_slash.to_string(),
        }
    }

    pub fn header_html_fragment(&self) -> &str {
        if self.header.is_empty() { GENERIC_HEADER } else { &self.header }
    }

    pub fn footer_html_fragment(&self) -> &str {
        if self.footer.is_empty() { GENERIC_FOOTER } else { &self.footer }
    }

    /// Suffix a generator-owned title (landing/hub/feed/pair pages and
    /// similar synthesized strings, as opposed to per-tool `meta.toml`
    /// titles) with `brand_name` when set, unsuffixed otherwise.
    ///
    /// Block `page/meta.toml` titles are brand-free by construction — hygiene
    /// check 8 rejects any domain string in `page/`, and `title()` above
    /// appends `title_suffix` to them at render time. `brand_title` is a
    /// separate mechanism for the generator's own synthesized titles, which
    /// have no `meta.toml` to hold a suffix and so derive their suffix
    /// straight from `brand_name` instead of `title_suffix`. Keeping the two
    /// independent means a site config can set `title_suffix` for per-tool
    /// titles without also affecting (or being required for) these
    /// generator-owned titles, and vice versa.
    pub fn brand_title(&self, base: &str) -> String {
        if self.brand_name.is_empty() {
            base.to_string()
        } else {
            format!("{base} — {}", self.brand_name)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_generic() {
        let cfg = SiteConfig::default();
        assert_eq!(cfg.abs("/tools/x/"), None);
        assert_eq!(cfg.url_or_rel("/tools/x/"), "/tools/x/");
        assert_eq!(cfg.title("Age Calculator"), "Age Calculator");
        assert_eq!(cfg.og_label("tools/x"), "tools/x");
        assert!(cfg.header_html_fragment().contains("href=\"/tools/\""));
        assert!(!cfg.header_html_fragment().contains("gizza"));
    }

    #[test]
    fn branded_config() {
        let cfg = SiteConfig {
            base_url: "https://gizza.ai".into(),
            title_suffix: " — gizza.ai".into(),
            ..Default::default()
        };
        assert_eq!(cfg.abs("/tools/x/").as_deref(), Some("https://gizza.ai/tools/x/"));
        assert_eq!(cfg.title("Age Calculator"), "Age Calculator — gizza.ai");
        assert_eq!(cfg.og_label("tools/x"), "gizza.ai/tools/x");
    }

    #[test]
    fn brand_title_uses_brand_name_independent_of_title_suffix() {
        let generic = SiteConfig::default();
        assert_eq!(generic.brand_title("All Tools"), "All Tools");

        // title_suffix stays "" during the rollout (see doc comment), but
        // brand_title must still reproduce the historical suffix from
        // brand_name alone.
        let branded = SiteConfig {
            base_url: "https://gizza.ai".into(),
            brand_name: "gizza.ai".into(),
            title_suffix: "".into(),
            ..Default::default()
        };
        assert_eq!(branded.brand_title("All Tools"), "All Tools — gizza.ai");
    }
}

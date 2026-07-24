//! gizza-ai/html-extract core — run a CSS selector over pasted HTML and pull out
//! the text / inner HTML / outer HTML / a named attribute from every match.
//! "jq for markup". No wafer/wasm-bindgen deps. Uses `scraper` (html5ever-based,
//! wasm32-safe) to parse + select; `serde_json` to emit the count + matches.

use scraper::{Html, Selector};

/// What to pull out of each element that matches the selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Extract {
    /// Visible text content, descendants included (`.text()`).
    Text,
    /// The element's children serialized as HTML (`.inner_html()`).
    InnerHtml,
    /// The element itself serialized as HTML, own tags included (`.html()`).
    OuterHtml,
    /// The value of a named attribute (`.attr(name)`).
    Attr,
}

pub fn parse_extract(s: &str) -> Result<Extract, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "text" => Ok(Extract::Text),
        "inner-html" | "inner_html" | "innerhtml" => Ok(Extract::InnerHtml),
        "outer-html" | "outer_html" | "outerhtml" => Ok(Extract::OuterHtml),
        "attr" | "attribute" => Ok(Extract::Attr),
        other => Err(format!(
            "extract {other:?} not supported (text|inner-html|outer-html|attr)"
        )),
    }
}

/// Collapse runs of whitespace (incl. newlines) to single spaces and trim ends.
fn normalize_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Run `selector` over `html` and extract `mode` from each match, capped at
/// `limit`. For `Extract::Attr`, `attr` names the attribute (required) and
/// elements lacking it are skipped. When `trim` is on, text/attr whitespace is
/// normalized and html ends are trimmed. Returns pretty JSON `{count, matches}`.
pub fn extract(
    html: &str,
    selector: &str,
    mode: Extract,
    attr: &str,
    limit: usize,
    trim: bool,
) -> Result<String, String> {
    if html.trim().is_empty() {
        return Err("input HTML is empty".into());
    }
    if selector.trim().is_empty() {
        return Err("a CSS selector is required".into());
    }
    if limit == 0 {
        return Err("limit must be at least 1".into());
    }
    let attr = attr.trim();
    if mode == Extract::Attr && attr.is_empty() {
        return Err(
            "an attribute name is required when extract=attr (e.g. href, src, class)".into(),
        );
    }

    let sel = Selector::parse(selector.trim())
        .map_err(|e| format!("invalid CSS selector {:?}: {e}", selector.trim()))?;
    let doc = Html::parse_fragment(html);

    let mut matches: Vec<String> = Vec::new();
    for el in doc.select(&sel) {
        if matches.len() >= limit {
            break;
        }
        let value = match mode {
            Extract::Text => {
                let t = el.text().collect::<String>();
                if trim {
                    normalize_ws(&t)
                } else {
                    t
                }
            }
            Extract::InnerHtml => {
                let h = el.inner_html();
                if trim {
                    h.trim().to_string()
                } else {
                    h
                }
            }
            Extract::OuterHtml => {
                let h = el.html();
                if trim {
                    h.trim().to_string()
                } else {
                    h
                }
            }
            Extract::Attr => match el.value().attr(attr) {
                // Elements without the attribute are not matches — skip them.
                None => continue,
                Some(v) => {
                    if trim {
                        normalize_ws(v)
                    } else {
                        v.to_string()
                    }
                }
            },
        };
        matches.push(value);
    }

    let out = serde_json::json!({
        "count": matches.len(),
        "matches": matches,
    });
    serde_json::to_string_pretty(&out).map_err(|e| format!("failed to serialize output: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = r#"<ul>
        <li><a href="/one" class="link">First</a></li>
        <li><a href="/two" class="link">Second</a></li>
        <li><a>No href</a></li>
    </ul>"#;

    #[test]
    fn text_extract_default() {
        let out = extract(DOC, "a.link", Extract::Text, "", 100, true).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["count"], 2);
        assert_eq!(v["matches"][0], "First");
        assert_eq!(v["matches"][1], "Second");
    }

    #[test]
    fn attr_extract() {
        let out = extract(DOC, "a", Extract::Attr, "href", 100, true).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        // Only the two links with an href; the third <a> is skipped.
        assert_eq!(v["count"], 2);
        assert_eq!(v["matches"][0], "/one");
        assert_eq!(v["matches"][1], "/two");
    }

    #[test]
    fn inner_and_outer_html() {
        let inner = extract(
            "<div><b>hi</b></div>",
            "div",
            Extract::InnerHtml,
            "",
            100,
            true,
        )
        .unwrap();
        assert!(inner.contains("<b>hi</b>"), "{inner}");
        let outer = extract(
            "<div><b>hi</b></div>",
            "div",
            Extract::OuterHtml,
            "",
            100,
            true,
        )
        .unwrap();
        assert!(outer.contains("<div><b>hi</b></div>"), "{outer}");
    }

    #[test]
    fn limit_caps_matches() {
        let out = extract(DOC, "a.link", Extract::Text, "", 1, true).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["count"], 1);
        assert_eq!(v["matches"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn trim_off_preserves_whitespace() {
        let html = "<p>  a   b  </p>";
        let trimmed = extract(html, "p", Extract::Text, "", 100, true).unwrap();
        assert!(trimmed.contains("\"a b\""), "{trimmed}");
        let raw = extract(html, "p", Extract::Text, "", 100, false).unwrap();
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v["matches"][0], "  a   b  ");
    }

    #[test]
    fn trim_on_normalizes_attribute_whitespace() {
        let out = extract(
            r#"<a title="  one   two  ">x</a>"#,
            "a",
            Extract::Attr,
            "title",
            100,
            true,
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["matches"][0], "one two");
    }

    #[test]
    fn err_empty_html() {
        assert!(extract("   ", "a", Extract::Text, "", 100, true).is_err());
    }

    #[test]
    fn err_empty_selector() {
        assert!(extract("<a>x</a>", "  ", Extract::Text, "", 100, true).is_err());
    }

    #[test]
    fn err_attr_without_name() {
        assert!(extract("<a href=x>y</a>", "a", Extract::Attr, "", 100, true).is_err());
    }

    #[test]
    fn err_invalid_selector() {
        assert!(extract("<a>x</a>", "a[[[", Extract::Text, "", 100, true).is_err());
    }

    #[test]
    fn err_zero_limit() {
        assert!(extract("<a>x</a>", "a", Extract::Text, "", 0, true).is_err());
    }

    #[test]
    fn parse_extract_forms() {
        assert_eq!(parse_extract("").unwrap(), Extract::Text);
        assert_eq!(parse_extract("TEXT").unwrap(), Extract::Text);
        assert_eq!(parse_extract("inner-html").unwrap(), Extract::InnerHtml);
        assert_eq!(parse_extract("outer-html").unwrap(), Extract::OuterHtml);
        assert_eq!(parse_extract("attr").unwrap(), Extract::Attr);
        assert!(parse_extract("bogus").is_err());
    }

    #[test]
    fn no_matches_is_empty_not_error() {
        let out = extract("<a>x</a>", "table", Extract::Text, "", 100, true).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["count"], 0);
        assert_eq!(v["matches"].as_array().unwrap().len(), 0);
    }
}

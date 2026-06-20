//! gizza-ai/readability-extractor core — pure compute, shared by the chat skill
//! block and the web page. No wafer/wasm-bindgen deps. Extracts the main article
//! (title + body), stripping nav/ads/boilerplate, via `dom_smoothie` (a Rust
//! port of Mozilla's Readability).

use dom_smoothie::{Config, Readability};

/// Extract the main article from `html`. `as_html=true` returns the cleaned
/// article HTML; otherwise readable plain text. The title (when found) is
/// prepended. Errors on empty input or if no article can be extracted.
pub fn extract(html: &str, as_html: bool) -> Result<String, String> {
    if html.trim().is_empty() {
        return Err("input is empty".into());
    }
    // A base URL lets Readability resolve relative links; pasted HTML has none,
    // so a placeholder is fine (we don't fetch anything).
    let mut readability = Readability::new(html, Some("https://example.com/"), Some(Config::default()))
        .map_err(|e| format!("failed to parse HTML: {e}"))?;
    let article = readability
        .parse()
        .map_err(|e| format!("could not extract an article: {e}"))?;

    let title = article.title.trim().to_string();
    // For text mode, render the cleaned article HTML through nanohtml2text so
    // adjacent blocks (heading/paragraphs) are properly separated — dom_smoothie's
    // own text_content concatenates them. HTML mode returns the cleaned markup.
    let body_owned = if as_html {
        article.content.to_string()
    } else {
        nanohtml2text::html2text(&article.content)
            .replace("\r\n", "\n")
            .replace('\r', "\n")
    };
    let body = body_owned.trim();
    if body.is_empty() {
        return Err("no article content found in the HTML".into());
    }
    if title.is_empty() {
        Ok(body.to_string())
    } else if as_html {
        Ok(format!("<h1>{title}</h1>\n{body}"))
    } else {
        Ok(format!("{title}\n\n{body}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = r#"<html><head><title>News</title></head><body>
        <nav><a href="/">Home</a><a href="/about">About</a></nav>
        <div class="ad">BUY NOW!!!</div>
        <article><h1>The Real Headline</h1>
        <p>This is the first substantial paragraph of the article body, with enough
        text that the readability algorithm treats it as the main content rather
        than the surrounding navigation chrome and advertisement boilerplate.</p>
        <p>A second meaningful paragraph continues the article so the extractor has
        a clear, dense block of prose to lock onto as the primary content.</p>
        </article>
        <footer>Copyright 2026</footer></body></html>"#;

    #[test]
    fn extracts_article_text_drops_chrome() {
        let out = extract(PAGE, false).unwrap();
        assert!(out.contains("first substantial paragraph"), "got: {out}");
        assert!(!out.contains("BUY NOW"), "ad should be stripped: {out}");
        assert!(!out.contains("About"), "nav should be stripped: {out}");
    }

    #[test]
    fn html_mode_returns_markup() {
        let out = extract(PAGE, true).unwrap();
        assert!(out.contains('<') && out.contains("substantial paragraph"), "got: {out}");
    }

    #[test]
    fn empty_input_errors() {
        assert!(extract("   ", false).is_err());
    }
}

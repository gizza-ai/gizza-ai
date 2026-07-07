//! Extract FAQ question/answer pairs from a tool page's rendered content HTML.
//!
//! Tool pages author FAQs in `content.md` as a `## FAQ` section containing
//! `<details><summary>Question</summary>Answer…</details>` blocks (raw HTML
//! passes through the markdown renderer). This module pulls those pairs back
//! out of the rendered HTML so the page can emit matching FAQPage JSON-LD —
//! one source of truth, no drift between the visible FAQ and the schema.

/// One extracted FAQ entry.
#[derive(Debug, PartialEq)]
pub struct Faq {
    /// Plain-text question (tags stripped, whitespace collapsed).
    pub question: String,
    /// Answer as an HTML fragment (schema.org allows limited HTML in
    /// `acceptedAnswer.text`).
    pub answer_html: String,
}

/// Extract FAQs from rendered content HTML. Only `<details>` blocks inside the
/// FAQ section (from an `<h2>FAQ…</h2>` heading up to the next `<h2` or end of
/// document) are considered, so collapsible blocks used elsewhere on a page
/// don't leak into the schema. Returns an empty vec when there is no FAQ
/// section — callers skip the JSON-LD entirely in that case.
pub fn extract_faqs(content_html: &str) -> Vec<Faq> {
    let Some(section) = faq_section(content_html) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    let mut rest = section;
    while let Some(start) = find_ci(rest, "<details") {
        let block = &rest[start..];
        let Some(end) = find_ci(block, "</details>") else {
            break;
        };
        if let Some(faq) = parse_details(&block[..end]) {
            out.push(faq);
        }
        rest = &block[end + "</details>".len()..];
    }
    out
}

/// Slice of `html` from just after the FAQ `<h2>` heading to the next `<h2` (or
/// the end). The heading text must START with "FAQ" ("FAQ", "FAQs",
/// "FAQ — …"), case-insensitive.
fn faq_section(html: &str) -> Option<&str> {
    let mut search_from = 0;
    while let Some(rel) = find_ci(&html[search_from..], "<h2") {
        let h2_start = search_from + rel;
        let after_tag = html[h2_start..].find('>').map(|i| h2_start + i + 1)?;
        let text = &html[after_tag..];
        if text.trim_start().len() >= 3
            && text.trim_start()[..3].eq_ignore_ascii_case("faq")
        {
            let body_start = after_tag + text.find("</h2>").map(|i| i + "</h2>".len())?;
            let body = &html[body_start..];
            let body_end = find_ci(body, "<h2").unwrap_or(body.len());
            return Some(&body[..body_end]);
        }
        search_from = after_tag;
    }
    None
}

/// Parse one `<details …>…` block (without the closing tag) into a Faq.
fn parse_details(block: &str) -> Option<Faq> {
    let summary_open = find_ci(block, "<summary")?;
    let q_start = block[summary_open..].find('>')? + summary_open + 1;
    let q_end = find_ci(&block[q_start..], "</summary>")? + q_start;
    let question = strip_tags(&block[q_start..q_end]);
    let answer_html = block[q_end + "</summary>".len()..].trim().to_string();
    if question.is_empty() || answer_html.is_empty() {
        return None;
    }
    Some(Faq { question, answer_html })
}

/// Case-insensitive substring search (the HTML here is generator-authored
/// lowercase, but content.md is hand-written).
fn find_ci(haystack: &str, needle: &str) -> Option<usize> {
    let n = needle.to_ascii_lowercase();
    haystack
        .to_ascii_lowercase()
        .find(&n)
        // byte offsets match because to_ascii_lowercase preserves lengths
}

/// Drop tags and collapse whitespace: `<code>x</code>  y` → `x y`.
fn strip_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = r#"
<h2>Formats</h2>
<details><summary>Not a FAQ</summary><p>Outside the FAQ section.</p></details>
<h2>FAQ</h2>
<details>
<summary>Which format should I pick?</summary>
<p><code>mp3</code> for compatibility.</p>
</details>
<details><summary>Does it upload?</summary><p>No — everything runs locally.</p></details>
<h2>After</h2>
<details><summary>Also not a FAQ</summary><p>x</p></details>
"#;

    #[test]
    fn extracts_only_faq_section_details() {
        let faqs = extract_faqs(PAGE);
        assert_eq!(faqs.len(), 2);
        assert_eq!(faqs[0].question, "Which format should I pick?");
        assert_eq!(faqs[0].answer_html, "<p><code>mp3</code> for compatibility.</p>");
        assert_eq!(faqs[1].question, "Does it upload?");
        assert_eq!(faqs[1].answer_html, "<p>No — everything runs locally.</p>");
    }

    #[test]
    fn no_faq_section_means_no_entries() {
        assert!(extract_faqs("<h2>About</h2><p>x</p>").is_empty());
        assert!(extract_faqs("").is_empty());
    }

    #[test]
    fn question_tags_are_stripped_and_whitespace_collapsed() {
        let html = "<h2>FAQs</h2><details><summary> Convert <em>mp3</em>\n to wav? </summary><p>a</p></details>";
        let faqs = extract_faqs(html);
        assert_eq!(faqs[0].question, "Convert mp3 to wav?");
    }

    #[test]
    fn empty_or_partial_details_are_skipped() {
        let html = "<h2>FAQ</h2><details><summary></summary><p>a</p></details>\
                    <details><summary>q</summary></details>\
                    <details><summary>ok?</summary><p>yes</p></details>";
        let faqs = extract_faqs(html);
        assert_eq!(faqs.len(), 1);
        assert_eq!(faqs[0].question, "ok?");
    }

    #[test]
    fn heading_with_suffix_and_attributes_matches() {
        let html = r#"<h2 id="faq">FAQ — audio</h2><details><summary>q?</summary><p>a</p></details>"#;
        assert_eq!(extract_faqs(html).len(), 1);
    }
}

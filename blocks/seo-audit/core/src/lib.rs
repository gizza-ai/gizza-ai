//! gizza-ai/seo-audit core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps. Audits pasted HTML for on-page SEO issues
//! (title, meta description, heading hierarchy, image alt text, canonical link, and
//! Open Graph tags) and produces a scored, human-readable report.

use regex::Regex;

/// Recommended length bounds (characters) for the `<title>` and meta description.
const TITLE_MIN: usize = 10;
const TITLE_MAX: usize = 60;
const DESC_MIN: usize = 50;
const DESC_MAX: usize = 160;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Level {
    Pass,
    Warn,
    Fail,
}

impl Level {
    fn mark(self) -> char {
        match self {
            Level::Pass => '✓',
            Level::Warn => '⚠',
            Level::Fail => '✗',
        }
    }
    /// Score weight contribution: a pass earns full credit, a warn half, a fail none.
    fn points(self) -> f64 {
        match self {
            Level::Pass => 1.0,
            Level::Warn => 0.5,
            Level::Fail => 0.0,
        }
    }
}

struct Check {
    level: Level,
    title: String,
    detail: String,
}

/// Audit `html` for common on-page SEO issues. Errors on empty input.
pub fn audit(html: &str) -> Result<String, String> {
    if html.trim().is_empty() {
        return Err("input is empty".into());
    }

    let mut checks: Vec<Check> = Vec::new();
    checks.push(check_title(html));
    checks.push(check_description(html));
    checks.push(check_h1(html));
    checks.push(check_heading_hierarchy(html));
    checks.push(check_images(html));
    checks.push(check_canonical(html));
    checks.push(check_lang(html));
    checks.push(check_charset(html));
    checks.push(check_viewport(html));
    checks.push(check_robots(html));
    checks.push(check_favicon(html));
    checks.push(check_hreflang(html));
    checks.push(check_open_graph(html));
    checks.push(check_twitter_card(html));
    checks.push(check_structured_data(html));
    checks.push(check_link_text(html));
    checks.push(check_meta_refresh(html));
    checks.push(check_deprecated(html));

    let earned: f64 = checks.iter().map(|c| c.level.points()).sum();
    let total = checks.len() as f64;
    let score = (earned / total * 100.0).round() as i64;
    let grade = grade_for(score);

    let passed = checks.iter().filter(|c| c.level == Level::Pass).count();
    let warned = checks.iter().filter(|c| c.level == Level::Warn).count();
    let failed = checks.iter().filter(|c| c.level == Level::Fail).count();

    let mut out = String::new();
    out.push_str(&format!("SEO score: {score}/100 (grade {grade})\n"));
    out.push_str(&format!(
        "{passed} passed, {warned} warnings, {failed} failed\n\n"
    ));
    for c in &checks {
        out.push_str(&format!("{} {}\n", c.level.mark(), c.title));
        out.push_str(&format!("   {}\n", c.detail));
    }
    Ok(out.trim_end().to_string())
}

fn grade_for(score: i64) -> char {
    match score {
        90..=100 => 'A',
        75..=89 => 'B',
        60..=74 => 'C',
        40..=59 => 'D',
        _ => 'F',
    }
}

fn check_title(html: &str) -> Check {
    let re = Regex::new(r"(?is)<title\b[^>]*>(.*?)</title>").unwrap();
    match re.captures(html) {
        None => Check {
            level: Level::Fail,
            title: "Title tag".into(),
            detail: "No <title> tag found. Add a unique, descriptive page title.".into(),
        },
        Some(c) => {
            let text = decode_entities(&strip_inner(&c[1])).trim().to_string();
            let len = text.chars().count();
            if text.is_empty() {
                Check {
                    level: Level::Fail,
                    title: "Title tag".into(),
                    detail: "The <title> tag is empty.".into(),
                }
            } else if len < TITLE_MIN {
                Check {
                    level: Level::Warn,
                    title: "Title tag".into(),
                    detail: format!(
                        "Title is short ({len} chars). Aim for {TITLE_MIN}–{TITLE_MAX} characters: \"{text}\""
                    ),
                }
            } else if len > TITLE_MAX {
                Check {
                    level: Level::Warn,
                    title: "Title tag".into(),
                    detail: format!(
                        "Title is long ({len} chars) and may be truncated in search results. Aim for {TITLE_MIN}–{TITLE_MAX} characters."
                    ),
                }
            } else {
                Check {
                    level: Level::Pass,
                    title: "Title tag".into(),
                    detail: format!("Present, {len} chars: \"{text}\""),
                }
            }
        }
    }
}

fn check_description(html: &str) -> Check {
    match meta_named_content(html, "description") {
        None => Check {
            level: Level::Fail,
            title: "Meta description".into(),
            detail: "No <meta name=\"description\"> found. Add a 50–160 character summary.".into(),
        },
        Some(content) => {
            let text = decode_entities(&content).trim().to_string();
            let len = text.chars().count();
            if text.is_empty() {
                Check {
                    level: Level::Fail,
                    title: "Meta description".into(),
                    detail: "The meta description is empty.".into(),
                }
            } else if len < DESC_MIN {
                Check {
                    level: Level::Warn,
                    title: "Meta description".into(),
                    detail: format!(
                        "Description is short ({len} chars). Aim for {DESC_MIN}–{DESC_MAX} characters."
                    ),
                }
            } else if len > DESC_MAX {
                Check {
                    level: Level::Warn,
                    title: "Meta description".into(),
                    detail: format!(
                        "Description is long ({len} chars) and may be truncated. Aim for {DESC_MIN}–{DESC_MAX} characters."
                    ),
                }
            } else {
                Check {
                    level: Level::Pass,
                    title: "Meta description".into(),
                    detail: format!("Present, {len} chars."),
                }
            }
        }
    }
}

fn check_h1(html: &str) -> Check {
    let n = count_headings(html, 1);
    match n {
        0 => Check {
            level: Level::Fail,
            title: "H1 heading".into(),
            detail: "No <h1> found. Every page should have exactly one <h1>.".into(),
        },
        1 => Check {
            level: Level::Pass,
            title: "H1 heading".into(),
            detail: "Exactly one <h1>, as recommended.".into(),
        },
        _ => Check {
            level: Level::Warn,
            title: "H1 heading".into(),
            detail: format!("{n} <h1> tags found. Use a single <h1> per page."),
        },
    }
}

fn check_heading_hierarchy(html: &str) -> Check {
    let levels = heading_sequence(html);
    if levels.is_empty() {
        return Check {
            level: Level::Warn,
            title: "Heading hierarchy".into(),
            detail: "No headings (<h1>–<h6>) found. Use headings to structure content.".into(),
        };
    }
    let mut prev = 0u8;
    let mut skips: Vec<String> = Vec::new();
    for &lvl in &levels {
        if prev != 0 && lvl > prev + 1 {
            skips.push(format!("h{prev} → h{lvl}"));
        }
        prev = lvl;
    }
    if skips.is_empty() {
        Check {
            level: Level::Pass,
            title: "Heading hierarchy".into(),
            detail: format!("{} headings, no skipped levels.", levels.len()),
        }
    } else {
        Check {
            level: Level::Warn,
            title: "Heading hierarchy".into(),
            detail: format!(
                "Skipped heading level(s): {}. Don't jump levels (e.g. h2 → h4).",
                skips.join(", ")
            ),
        }
    }
}

fn check_images(html: &str) -> Check {
    let re = Regex::new(r"(?is)<img\b[^>]*>").unwrap();
    let imgs: Vec<&str> = re.find_iter(html).map(|m| m.as_str()).collect();
    if imgs.is_empty() {
        return Check {
            level: Level::Pass,
            title: "Image alt text".into(),
            detail: "No <img> tags found.".into(),
        };
    }
    let alt_re = Regex::new(r#"(?is)\balt\s*=\s*("([^"]*)"|'([^']*)'|[^\s>]+)"#).unwrap();
    let mut missing = 0usize;
    for tag in &imgs {
        let has_nonempty_alt = alt_re.captures(tag).map(|c| {
            let v = c
                .get(2)
                .or_else(|| c.get(3))
                .map(|m| m.as_str().trim().to_string())
                .unwrap_or_else(|| c[1].trim().to_string());
            !v.is_empty()
        });
        if has_nonempty_alt != Some(true) {
            missing += 1;
        }
    }
    let total = imgs.len();
    if missing == 0 {
        Check {
            level: Level::Pass,
            title: "Image alt text".into(),
            detail: format!("All {total} image(s) have alt text."),
        }
    } else {
        Check {
            level: Level::Fail,
            title: "Image alt text".into(),
            detail: format!("{missing} of {total} image(s) are missing alt text."),
        }
    }
}

fn check_canonical(html: &str) -> Check {
    let re = Regex::new(r"(?is)<link\b[^>]*>").unwrap();
    for tag in re.find_iter(html).map(|m| m.as_str()) {
        if attr(tag, "rel").as_deref() == Some("canonical") {
            let href = attr(tag, "href").unwrap_or_default();
            if href.trim().is_empty() {
                return Check {
                    level: Level::Warn,
                    title: "Canonical link".into(),
                    detail: "<link rel=\"canonical\"> has no href.".into(),
                };
            }
            return Check {
                level: Level::Pass,
                title: "Canonical link".into(),
                detail: format!("Canonical URL set: {}", href.trim()),
            };
        }
    }
    Check {
        level: Level::Warn,
        title: "Canonical link".into(),
        detail: "No <link rel=\"canonical\"> found. Add one to avoid duplicate-content issues."
            .into(),
    }
}

fn check_open_graph(html: &str) -> Check {
    let wanted = ["og:title", "og:description", "og:image"];
    let present: Vec<&str> = wanted
        .iter()
        .copied()
        .filter(|p| meta_property_content(html, p).is_some())
        .collect();
    let missing: Vec<&str> = wanted
        .iter()
        .copied()
        .filter(|p| !present.contains(p))
        .collect();
    if missing.is_empty() {
        Check {
            level: Level::Pass,
            title: "Open Graph tags".into(),
            detail: "og:title, og:description and og:image are all present.".into(),
        }
    } else if present.is_empty() {
        Check {
            level: Level::Fail,
            title: "Open Graph tags".into(),
            detail: "No Open Graph tags found. Add og:title, og:description and og:image for rich social previews.".into(),
        }
    } else {
        Check {
            level: Level::Warn,
            title: "Open Graph tags".into(),
            detail: format!("Missing Open Graph tag(s): {}.", missing.join(", ")),
        }
    }
}

fn check_lang(html: &str) -> Check {
    let re = Regex::new(r"(?is)<html\b[^>]*>").unwrap();
    match re.find(html).map(|m| m.as_str()) {
        Some(tag) => match attr(tag, "lang") {
            Some(lang) if !lang.trim().is_empty() => Check {
                level: Level::Pass,
                title: "Language attribute".into(),
                detail: format!("<html lang=\"{}\"> is set.", lang.trim()),
            },
            _ => Check {
                level: Level::Warn,
                title: "Language attribute".into(),
                detail: "The <html> tag has no lang attribute. Add e.g. <html lang=\"en\"> for accessibility and search.".into(),
            },
        },
        None => Check {
            level: Level::Warn,
            title: "Language attribute".into(),
            detail: "No <html> tag found, so no lang attribute could be checked.".into(),
        },
    }
}

fn check_charset(html: &str) -> Check {
    // <meta charset="utf-8"> or <meta http-equiv="Content-Type" content="...; charset=...">
    let charset_re = Regex::new(r#"(?is)<meta\b[^>]*\bcharset\s*=\s*("([^"]*)"|'([^']*)'|[^\s>]+)[^>]*>"#).unwrap();
    let has_charset = charset_re.is_match(html)
        || meta_content_where(html, "http-equiv", "content-type")
            .map(|c| c.to_lowercase().contains("charset"))
            .unwrap_or(false);
    if has_charset {
        Check {
            level: Level::Pass,
            title: "Character encoding".into(),
            detail: "A character encoding is declared (e.g. <meta charset=\"utf-8\">).".into(),
        }
    } else {
        Check {
            level: Level::Warn,
            title: "Character encoding".into(),
            detail: "No <meta charset> declared. Add <meta charset=\"utf-8\"> early in <head>.".into(),
        }
    }
}

fn check_viewport(html: &str) -> Check {
    match meta_named_content(html, "viewport") {
        None => Check {
            level: Level::Fail,
            title: "Mobile viewport".into(),
            detail: "No <meta name=\"viewport\"> found. Add <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"> for mobile-friendliness.".into(),
        },
        Some(content) => {
            let c = content.to_lowercase();
            if c.contains("width=device-width") {
                Check {
                    level: Level::Pass,
                    title: "Mobile viewport".into(),
                    detail: "Responsive viewport set (width=device-width).".into(),
                }
            } else {
                Check {
                    level: Level::Warn,
                    title: "Mobile viewport".into(),
                    detail: "A viewport tag is present but doesn't set width=device-width. Mobile rendering may be off.".into(),
                }
            }
        }
    }
}

fn check_robots(html: &str) -> Check {
    match meta_named_content(html, "robots") {
        None => Check {
            level: Level::Pass,
            title: "Robots meta".into(),
            detail: "No robots restrictions; the page is indexable by default.".into(),
        },
        Some(content) => {
            let c = content.to_lowercase();
            if c.contains("noindex") {
                Check {
                    level: Level::Fail,
                    title: "Robots meta".into(),
                    detail: format!("<meta name=\"robots\" content=\"{}\"> contains noindex — this page will be kept out of search results.", content.trim()),
                }
            } else if c.contains("nofollow") {
                Check {
                    level: Level::Warn,
                    title: "Robots meta".into(),
                    detail: format!("<meta name=\"robots\"> sets nofollow ({}). Links on this page won't pass authority.", content.trim()),
                }
            } else {
                Check {
                    level: Level::Pass,
                    title: "Robots meta".into(),
                    detail: format!("Robots directive present and allows indexing: {}", content.trim()),
                }
            }
        }
    }
}

fn check_twitter_card(html: &str) -> Check {
    let card = meta_named_content(html, "twitter:card");
    let wanted = ["twitter:title", "twitter:description", "twitter:image"];
    let present_extras: Vec<&str> = wanted
        .iter()
        .copied()
        .filter(|p| meta_named_content(html, p).is_some())
        .collect();
    match card {
        Some(c) if !c.trim().is_empty() => {
            if present_extras.len() == wanted.len() {
                Check {
                    level: Level::Pass,
                    title: "Twitter Card tags".into(),
                    detail: format!("twitter:card ({}) plus title, description and image are present.", c.trim()),
                }
            } else {
                let missing: Vec<&str> = wanted
                    .iter()
                    .copied()
                    .filter(|p| !present_extras.contains(p))
                    .collect();
                Check {
                    level: Level::Warn,
                    title: "Twitter Card tags".into(),
                    detail: format!("twitter:card is set but missing: {}.", missing.join(", ")),
                }
            }
        }
        _ => {
            if present_extras.is_empty() {
                Check {
                    level: Level::Warn,
                    title: "Twitter Card tags".into(),
                    detail: "No Twitter Card tags found. Add twitter:card (and twitter:title/description/image), or rely on Open Graph fallbacks.".into(),
                }
            } else {
                Check {
                    level: Level::Warn,
                    title: "Twitter Card tags".into(),
                    detail: "Twitter tags found but no twitter:card type. Add <meta name=\"twitter:card\" content=\"summary_large_image\">.".into(),
                }
            }
        }
    }
}

fn check_structured_data(html: &str) -> Check {
    let ld = Regex::new(r#"(?is)<script\b[^>]*type\s*=\s*['"]?application/ld\+json['"]?[^>]*>"#).unwrap();
    let ld_count = ld.find_iter(html).count();
    if ld_count > 0 {
        return Check {
            level: Level::Pass,
            title: "Structured data".into(),
            detail: format!("{ld_count} JSON-LD structured-data block(s) found."),
        };
    }
    // Fall back to detecting microdata (itemscope/itemtype).
    let micro = Regex::new(r"(?is)\bitemscope\b").unwrap();
    if micro.is_match(html) {
        return Check {
            level: Level::Pass,
            title: "Structured data".into(),
            detail: "Microdata (itemscope) found.".into(),
        };
    }
    Check {
        level: Level::Warn,
        title: "Structured data".into(),
        detail: "No structured data found. Add JSON-LD (schema.org) for rich results.".into(),
    }
}

fn check_favicon(html: &str) -> Check {
    let re = Regex::new(r"(?is)<link\b[^>]*>").unwrap();
    for tag in re.find_iter(html).map(|m| m.as_str()) {
        if let Some(rel) = attr(tag, "rel") {
            // rel may be "icon", "shortcut icon", "apple-touch-icon", "mask-icon"…
            if rel.split_whitespace().any(|w| w == "icon")
                || rel.contains("apple-touch-icon")
                || rel.contains("mask-icon")
            {
                return Check {
                    level: Level::Pass,
                    title: "Favicon".into(),
                    detail: "A favicon link is declared.".into(),
                };
            }
        }
    }
    Check {
        level: Level::Warn,
        title: "Favicon".into(),
        detail: "No favicon <link rel=\"icon\"> found. Add one for branding in tabs and search results.".into(),
    }
}

fn check_hreflang(html: &str) -> Check {
    let re = Regex::new(r"(?is)<link\b[^>]*>").unwrap();
    let mut langs: Vec<String> = Vec::new();
    for tag in re.find_iter(html).map(|m| m.as_str()) {
        if attr(tag, "rel").as_deref() == Some("alternate") {
            if let Some(h) = attr(tag, "hreflang") {
                if !h.trim().is_empty() {
                    langs.push(h.trim().to_string());
                }
            }
        }
    }
    if langs.is_empty() {
        return Check {
            level: Level::Pass,
            title: "Hreflang".into(),
            detail: "No hreflang annotations (fine for a single-language page).".into(),
        };
    }
    if !langs.iter().any(|l| l == "x-default") {
        return Check {
            level: Level::Warn,
            title: "Hreflang".into(),
            detail: format!(
                "{} hreflang annotation(s) found but no x-default. Add an x-default for unmatched locales.",
                langs.len()
            ),
        };
    }
    Check {
        level: Level::Pass,
        title: "Hreflang".into(),
        detail: format!("{} hreflang annotation(s) incl. x-default.", langs.len()),
    }
}

fn check_link_text(html: &str) -> Check {
    let re = Regex::new(r"(?is)<a\b[^>]*>(.*?)</a>").unwrap();
    let generic = [
        "click here",
        "here",
        "read more",
        "more",
        "learn more",
        "this",
        "this page",
        "link",
        "go",
        "details",
    ];
    let mut total = 0usize;
    let mut vague = 0usize;
    for c in re.captures_iter(html) {
        let text = decode_entities(&strip_inner(&c[1]))
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();
        if text.is_empty() {
            continue; // image/icon-only links handled by alt-text check
        }
        total += 1;
        if generic.contains(&text.as_str()) {
            vague += 1;
        }
    }
    if total == 0 {
        return Check {
            level: Level::Pass,
            title: "Descriptive link text".into(),
            detail: "No text links to evaluate.".into(),
        };
    }
    if vague == 0 {
        Check {
            level: Level::Pass,
            title: "Descriptive link text".into(),
            detail: format!("All {total} text link(s) use descriptive anchor text."),
        }
    } else {
        Check {
            level: Level::Warn,
            title: "Descriptive link text".into(),
            detail: format!(
                "{vague} of {total} link(s) use vague anchor text (e.g. \"click here\"). Use descriptive text."
            ),
        }
    }
}

fn check_meta_refresh(html: &str) -> Check {
    match meta_content_where(html, "http-equiv", "refresh") {
        Some(_) => Check {
            level: Level::Warn,
            title: "Meta refresh redirect".into(),
            detail: "A <meta http-equiv=\"refresh\"> redirect was found. Use a server 301 redirect instead.".into(),
        },
        None => Check {
            level: Level::Pass,
            title: "Meta refresh redirect".into(),
            detail: "No meta-refresh redirect (good).".into(),
        },
    }
}

fn check_deprecated(html: &str) -> Check {
    let tags = [
        "frameset", "frame", "applet", "marquee", "blink", "center", "font", "strike", "big",
    ];
    let mut found: Vec<&str> = Vec::new();
    for t in tags {
        let re = Regex::new(&format!(r"(?is)<{t}\b")).unwrap();
        if re.is_match(html) {
            found.push(t);
        }
    }
    if found.is_empty() {
        Check {
            level: Level::Pass,
            title: "Deprecated HTML".into(),
            detail: "No deprecated or crawler-unfriendly elements found.".into(),
        }
    } else {
        Check {
            level: Level::Warn,
            title: "Deprecated HTML".into(),
            detail: format!(
                "Deprecated element(s) found: {}. Replace with modern, semantic HTML/CSS.",
                found.join(", ")
            ),
        }
    }
}

// ---- helpers -------------------------------------------------------------

/// Count `<hN>` opening tags.
fn count_headings(html: &str, n: u8) -> usize {
    let re = Regex::new(&format!(r"(?is)<h{n}\b[^>]*>")).unwrap();
    re.find_iter(html).count()
}

/// Heading levels in document order, e.g. [1, 2, 2, 3].
fn heading_sequence(html: &str) -> Vec<u8> {
    let re = Regex::new(r"(?is)<h([1-6])\b[^>]*>").unwrap();
    re.captures_iter(html)
        .map(|c| c[1].parse::<u8>().unwrap())
        .collect()
}

/// Value of attribute `name` on a single tag string (case-insensitive, quoted or bare).
fn attr(tag: &str, name: &str) -> Option<String> {
    let re = Regex::new(&format!(
        r#"(?is)\b{}\s*=\s*("([^"]*)"|'([^']*)'|[^\s>]+)"#,
        regex::escape(name)
    ))
    .unwrap();
    let c = re.captures(tag)?;
    let v = c
        .get(2)
        .or_else(|| c.get(3))
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| c[1].to_string());
    Some(decode_entities(&v).trim().to_lowercase())
}

/// `content` of `<meta name="NAME" content="...">` (either attribute order).
fn meta_named_content(html: &str, name: &str) -> Option<String> {
    meta_content_where(html, "name", name)
}

/// `content` of `<meta property="PROP" content="...">` (Open Graph).
fn meta_property_content(html: &str, prop: &str) -> Option<String> {
    meta_content_where(html, "property", prop)
}

fn meta_content_where(html: &str, key: &str, value: &str) -> Option<String> {
    let re = Regex::new(r"(?is)<meta\b[^>]*>").unwrap();
    for tag in re.find_iter(html).map(|m| m.as_str()) {
        if attr(tag, key).as_deref() == Some(&value.to_lowercase()) {
            return Some(raw_attr(tag, "content").unwrap_or_default());
        }
    }
    None
}

/// Like `attr` but preserves case/whitespace (used for content values).
fn raw_attr(tag: &str, name: &str) -> Option<String> {
    let re = Regex::new(&format!(
        r#"(?is)\b{}\s*=\s*("([^"]*)"|'([^']*)'|[^\s>]+)"#,
        regex::escape(name)
    ))
    .unwrap();
    let c = re.captures(tag)?;
    let v = c
        .get(2)
        .or_else(|| c.get(3))
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| c[1].to_string());
    Some(decode_entities(&v))
}

/// Strip nested tags from inner HTML (e.g. `<title>` may wrap a `<span>`).
fn strip_inner(s: &str) -> String {
    let re = Regex::new(r"(?is)<[^>]*>").unwrap();
    re.replace_all(s, "").to_string()
}

/// Decode the handful of HTML entities that show up in titles/descriptions.
fn decode_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perfect_page_scores_high() {
        let html = r#"
            <html lang="en"><head>
            <meta charset="utf-8">
            <meta name="viewport" content="width=device-width, initial-scale=1">
            <title>A Great Page About Widgets and Gadgets</title>
            <meta name="description" content="This is a thoroughly written meta description that sits comfortably within the recommended fifty to one hundred sixty character range.">
            <link rel="canonical" href="https://example.com/widgets">
            <link rel="icon" href="/favicon.ico">
            <meta property="og:title" content="Widgets">
            <meta property="og:description" content="All about widgets">
            <meta property="og:image" content="https://example.com/w.png">
            <meta name="twitter:card" content="summary_large_image">
            <meta name="twitter:title" content="Widgets">
            <meta name="twitter:description" content="All about widgets">
            <meta name="twitter:image" content="https://example.com/w.png">
            <script type="application/ld+json">{"@context":"https://schema.org","@type":"Article"}</script>
            </head><body>
            <h1>Widgets</h1><h2>Types</h2><h3>Round</h3>
            <img src="a.png" alt="a round widget">
            </body></html>"#;
        let report = audit(html).unwrap();
        assert!(report.contains("grade A"), "got: {report}");
        // A genuinely perfect page should clear every check: no warnings, no failures.
        assert!(report.contains("0 warnings, 0 failed"), "got: {report}");
    }

    #[test]
    fn detects_noindex() {
        let html = r#"<h1>x</h1><meta name="robots" content="noindex, nofollow">"#;
        let report = audit(html).unwrap();
        assert!(report.contains("✗ Robots meta"), "got: {report}");
        assert!(report.contains("noindex"), "got: {report}");
    }

    #[test]
    fn missing_viewport_fails() {
        let html = "<h1>x</h1>";
        let report = audit(html).unwrap();
        assert!(report.contains("✗ Mobile viewport"), "got: {report}");
    }

    #[test]
    fn lang_attribute_detected() {
        let html = r#"<html lang="fr"><body><h1>x</h1></body></html>"#;
        let report = audit(html).unwrap();
        assert!(report.contains("✓ Language attribute"), "got: {report}");
    }

    #[test]
    fn charset_detected() {
        let html = r#"<head><meta charset="utf-8"></head><body><h1>x</h1></body>"#;
        let report = audit(html).unwrap();
        assert!(report.contains("✓ Character encoding"), "got: {report}");
    }

    #[test]
    fn structured_data_detected() {
        let html = r#"<h1>x</h1><script type="application/ld+json">{"@type":"WebPage"}</script>"#;
        let report = audit(html).unwrap();
        assert!(report.contains("✓ Structured data"), "got: {report}");
    }

    #[test]
    fn twitter_card_missing_warns() {
        let html = "<h1>x</h1>";
        let report = audit(html).unwrap();
        assert!(report.contains("⚠ Twitter Card tags"), "got: {report}");
    }

    #[test]
    fn missing_title_fails() {
        let html = "<html><body><p>no head here</p></body></html>";
        let report = audit(html).unwrap();
        assert!(report.contains("✗ Title tag"), "got: {report}");
        assert!(report.contains("✗ Meta description"), "got: {report}");
    }

    #[test]
    fn detects_multiple_h1() {
        let html = "<h1>One</h1><h1>Two</h1>";
        let report = audit(html).unwrap();
        assert!(report.contains("2 <h1>"), "got: {report}");
    }

    #[test]
    fn detects_skipped_heading_level() {
        let html = "<h1>A</h1><h3>C</h3>";
        let report = audit(html).unwrap();
        assert!(report.contains("h1 → h3"), "got: {report}");
    }

    #[test]
    fn detects_missing_alt() {
        let html = r#"<h1>x</h1><img src="a.png"><img src="b.png" alt="">"#;
        let report = audit(html).unwrap();
        assert!(report.contains("2 of 2 image(s) are missing alt"), "got: {report}");
    }

    #[test]
    fn alt_present_passes() {
        let html = r#"<h1>x</h1><img src="a.png" alt="hi">"#;
        let report = audit(html).unwrap();
        assert!(report.contains("✓ Image alt text"), "got: {report}");
    }

    #[test]
    fn canonical_detected() {
        let html = r#"<h1>x</h1><link rel="canonical" href="https://e.com/">"#;
        let report = audit(html).unwrap();
        assert!(report.contains("Canonical URL set"), "got: {report}");
    }

    #[test]
    fn open_graph_partial_warns() {
        let html = r#"<h1>x</h1><meta property="og:title" content="t">"#;
        let report = audit(html).unwrap();
        assert!(report.contains("Missing Open Graph tag(s)"), "got: {report}");
        assert!(report.contains("og:description"), "got: {report}");
    }

    #[test]
    fn single_quoted_attrs_parse() {
        let html = "<h1>x</h1><meta name='description' content='short'>";
        let report = audit(html).unwrap();
        // 'short' is < 50 chars → warning, not a fail.
        assert!(report.contains("⚠ Meta description"), "got: {report}");
    }

    #[test]
    fn empty_input_errors() {
        assert!(audit("   ").is_err());
    }
}

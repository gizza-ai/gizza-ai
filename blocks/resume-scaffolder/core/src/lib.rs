//! gizza-ai/resume-scaffolder core — pure compute, shared by the chat skill block
//! and the web page. No wafer/wasm-bindgen deps. Renders structured résumé inputs
//! (contact, summary, experience, education, skills, extra sections) into a clean,
//! self-contained, **print-ready HTML document** — a styled page with an embedded
//! print stylesheet, ready to open and Print → Save-as-PDF from the browser.
//!
//! This differs from the ATS-Markdown `resume-builder`: the deliverable here is a
//! formatted, visually-designed document, not plain text. All résumé text is
//! HTML-escaped so free-form input can never inject markup, and the four style
//! knobs (theme / accent / font / page size) are validated so they can never
//! break out of the embedded CSS.

use serde_json::Value;

/// A validated set of presentation options for the rendered document.
pub struct Options {
    pub theme: Theme,
    /// A safe CSS color (a `#hex` value or a plain CSS color name).
    pub accent: String,
    pub font: Font,
    pub page_size: PageSize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    /// Serif, centered header, uppercase ruled section titles.
    Classic,
    /// Sans, left-aligned header, accent bar beside each section title.
    Modern,
    /// Tighter spacing and smaller type to fit more on one page.
    Compact,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Font {
    Sans,
    Serif,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PageSize {
    Letter,
    A4,
}

impl Theme {
    /// Parse the fixed-choice `theme` value (case-insensitive).
    pub fn parse(s: &str) -> Result<Theme, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "classic" => Ok(Theme::Classic),
            "modern" => Ok(Theme::Modern),
            "compact" => Ok(Theme::Compact),
            other => Err(format!(
                "invalid theme {other:?}: expected one of classic, modern, compact"
            )),
        }
    }
    fn class(self) -> &'static str {
        match self {
            Theme::Classic => "classic",
            Theme::Modern => "modern",
            Theme::Compact => "compact",
        }
    }
}

impl Font {
    pub fn parse(s: &str) -> Result<Font, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "sans" => Ok(Font::Sans),
            "serif" => Ok(Font::Serif),
            other => Err(format!("invalid font {other:?}: expected one of sans, serif")),
        }
    }
    fn family(self) -> &'static str {
        match self {
            Font::Sans => "-apple-system, BlinkMacSystemFont, \"Segoe UI\", Roboto, Helvetica, Arial, sans-serif",
            Font::Serif => "Georgia, Cambria, \"Times New Roman\", Times, serif",
        }
    }
}

impl PageSize {
    pub fn parse(s: &str) -> Result<PageSize, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "letter" | "us-letter" | "us_letter" => Ok(PageSize::Letter),
            "a4" => Ok(PageSize::A4),
            other => Err(format!(
                "invalid page_size {other:?}: expected one of letter, a4"
            )),
        }
    }
    /// (`@page size`, printable content max-width) for this page format.
    fn dims(self) -> (&'static str, &'static str) {
        match self {
            // ~0.75in margins on each side leave a comfortable text column.
            PageSize::Letter => ("letter", "7.0in"),
            PageSize::A4 => ("A4", "17.4cm"),
        }
    }
}

/// Validate a caller-supplied accent color so it can only ever be a `#hex` value
/// (3/4/6/8 hex digits) or a plain CSS color name — never CSS that could escape
/// the `--accent` custom property. Blank falls back to the default blue.
pub fn sanitize_accent(c: &str) -> Result<String, String> {
    let t = c.trim();
    if t.is_empty() {
        return Ok("#2563eb".to_string());
    }
    let ok = if let Some(hex) = t.strip_prefix('#') {
        matches!(hex.len(), 3 | 4 | 6 | 8) && hex.chars().all(|ch| ch.is_ascii_hexdigit())
    } else {
        t.len() <= 32 && t.chars().all(|ch| ch.is_ascii_alphabetic())
    };
    if ok {
        Ok(t.to_string())
    } else {
        Err(format!(
            "invalid accent color {t:?}: use a hex value like #2563eb or a CSS color name like navy"
        ))
    }
}

fn s(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string()
}

fn str_list(v: &Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str())
                .map(|x| x.trim().to_string())
                .filter(|x| !x.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// HTML-escape text so free-form résumé content can never inject markup.
fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#39;"),
            _ => o.push(c),
        }
    }
    o
}

/// Build a print-ready HTML résumé document from a JSON object of résumé fields.
///
/// Recognized keys (all optional except `name`):
/// name, title, email, phone, location, links[], summary,
/// experience[{role,company,location,dates,bullets[]}],
/// education[{degree,school,location,dates,details}], skills[],
/// sections[{heading, items[]}] for extras (Projects, Certifications, …).
pub fn build(json: &str, opts: &Options) -> Result<String, String> {
    if json.trim().is_empty() {
        return Err("input is empty — provide a JSON object of résumé fields".into());
    }
    let v: Value = serde_json::from_str(json).map_err(|e| format!("invalid JSON: {e}"))?;
    if !v.is_object() {
        return Err("expected a JSON object of résumé fields".into());
    }

    let name = s(&v, "name");
    if name.is_empty() {
        return Err("a 'name' field is required".into());
    }

    // ---- Header ----
    let mut body = String::new();
    body.push_str("<header class=\"r-head\">\n");
    body.push_str(&format!("<h1>{}</h1>\n", esc(&name)));
    let title = s(&v, "title");
    if !title.is_empty() {
        body.push_str(&format!("<p class=\"r-title\">{}</p>\n", esc(&title)));
    }
    let mut contact: Vec<String> = Vec::new();
    for k in ["email", "phone", "location"] {
        let val = s(&v, k);
        if !val.is_empty() {
            contact.push(val);
        }
    }
    contact.extend(str_list(&v, "links"));
    if !contact.is_empty() {
        let items = contact
            .iter()
            .map(|c| format!("<span>{}</span>", esc(c)))
            .collect::<Vec<_>>()
            .join("<span class=\"sep\">·</span>");
        body.push_str(&format!("<p class=\"r-contact\">{items}</p>\n"));
    }
    body.push_str("</header>\n");

    // ---- Summary ----
    let summary = s(&v, "summary");
    if !summary.is_empty() {
        body.push_str(&section_open("Summary"));
        body.push_str(&format!("<p class=\"r-summary\">{}</p>\n", esc(&summary)));
        body.push_str("</section>\n");
    }

    // ---- Experience ----
    if let Some(exp) = v.get("experience").and_then(Value::as_array) {
        if !exp.is_empty() {
            body.push_str(&section_open("Experience"));
            for e in exp {
                body.push_str(&render_entry(
                    e,
                    &["role", "company"],
                    Some("bullets"),
                    None,
                ));
            }
            body.push_str("</section>\n");
        }
    }

    // ---- Education ----
    if let Some(ed) = v.get("education").and_then(Value::as_array) {
        if !ed.is_empty() {
            body.push_str(&section_open("Education"));
            for e in ed {
                body.push_str(&render_entry(e, &["degree", "school"], None, Some("details")));
            }
            body.push_str("</section>\n");
        }
    }

    // ---- Skills ----
    let skills = str_list(&v, "skills");
    if !skills.is_empty() {
        body.push_str(&section_open("Skills"));
        let items = skills
            .iter()
            .map(|s| format!("<li>{}</li>", esc(s)))
            .collect::<Vec<_>>()
            .join("");
        body.push_str(&format!("<ul class=\"r-skills\">{items}</ul>\n"));
        body.push_str("</section>\n");
    }

    // ---- Custom sections ----
    if let Some(sections) = v.get("sections").and_then(Value::as_array) {
        for sec in sections {
            let heading = s(sec, "heading");
            let items = str_list(sec, "items");
            if heading.is_empty() || items.is_empty() {
                continue;
            }
            body.push_str(&section_open(&heading));
            body.push_str("<ul>\n");
            for it in items {
                body.push_str(&format!("<li>{}</li>\n", esc(&it)));
            }
            body.push_str("</ul>\n");
            body.push_str("</section>\n");
        }
    }

    Ok(document(&name, &body, opts))
}

/// Render one experience/education entry: a two-column head (title — dates·loc),
/// an optional bullet list (`bullets_key`), and an optional details paragraph
/// (`details_key`).
fn render_entry(
    e: &Value,
    head_keys: &[&str; 2],
    bullets_key: Option<&str>,
    details_key: Option<&str>,
) -> String {
    let a = s(e, head_keys[0]);
    let b = s(e, head_keys[1]);
    let head = [a.as_str(), b.as_str()]
        .iter()
        .filter(|x| !x.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(" — ");
    let meta = [s(e, "dates"), s(e, "location")]
        .iter()
        .filter(|x| !x.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(" · ");

    let mut out = String::from("<div class=\"r-entry\">\n");
    if !head.is_empty() || !meta.is_empty() {
        out.push_str("<div class=\"r-entry-head\">");
        if !head.is_empty() {
            out.push_str(&format!("<span class=\"r-role\">{}</span>", esc(&head)));
        }
        if !meta.is_empty() {
            out.push_str(&format!("<span class=\"r-meta\">{}</span>", esc(&meta)));
        }
        out.push_str("</div>\n");
    }
    if let Some(k) = bullets_key {
        let bullets = str_list(e, k);
        if !bullets.is_empty() {
            out.push_str("<ul>\n");
            for bl in bullets {
                out.push_str(&format!("<li>{}</li>\n", esc(&bl)));
            }
            out.push_str("</ul>\n");
        }
    }
    if let Some(k) = details_key {
        let details = s(e, k);
        if !details.is_empty() {
            out.push_str(&format!("<p class=\"r-details\">{}</p>\n", esc(&details)));
        }
    }
    out.push_str("</div>\n");
    out
}

fn section_open(heading: &str) -> String {
    format!("<section class=\"r-sec\">\n<h2>{}</h2>\n", esc(heading))
}

/// Wrap the rendered body in a complete, self-contained HTML document with the
/// embedded theme + print stylesheet.
fn document(name: &str, body: &str, opts: &Options) -> String {
    let (page, max_w) = opts.page_size.dims();
    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title} — Résumé</title>
<style>
:root {{ --accent: {accent}; --maxw: {max_w}; }}
* {{ box-sizing: border-box; }}
html, body {{ margin: 0; padding: 0; background: #f3f4f6; color: #1a1a1a; }}
body {{ font-family: {family}; line-height: 1.45; }}
.resume {{
  max-width: var(--maxw);
  margin: 24px auto;
  padding: 40px 44px;
  background: #fff;
  box-shadow: 0 1px 4px rgba(0,0,0,.12);
}}
.r-head {{ margin-bottom: 18px; }}
.r-head h1 {{ margin: 0 0 2px; font-size: 26px; letter-spacing: .3px; }}
.r-title {{ margin: 0 0 6px; font-size: 14px; color: #444; }}
.r-contact {{ margin: 0; font-size: 12px; color: #555; }}
.r-contact .sep {{ margin: 0 6px; color: #aaa; }}
.r-sec {{ margin-top: 18px; }}
.r-sec h2 {{
  font-size: 13px; text-transform: uppercase; letter-spacing: 1px;
  color: var(--accent); margin: 0 0 8px; padding-bottom: 3px;
  border-bottom: 2px solid var(--accent);
}}
.r-entry {{ margin-bottom: 12px; }}
.r-entry-head {{ display: flex; justify-content: space-between; gap: 12px; flex-wrap: wrap; }}
.r-role {{ font-weight: 600; font-size: 14px; }}
.r-meta {{ font-size: 12px; color: #666; white-space: nowrap; }}
.r-summary, .r-details {{ margin: 4px 0 0; font-size: 13px; }}
.r-sec ul {{ margin: 6px 0 0; padding-left: 18px; }}
.r-sec li {{ font-size: 13px; margin: 2px 0; }}
.r-skills {{ list-style: none; padding: 0; display: flex; flex-wrap: wrap; gap: 6px; }}
.r-skills li {{
  background: #eef2ff; color: #1a1a1a; border: 1px solid #e0e4f5;
  border-radius: 4px; padding: 2px 8px; font-size: 12px;
}}
/* Theme: classic — serif, centered header, ink-black ruled titles */
.theme-classic .r-head {{ text-align: center; }}
.theme-classic h1, .theme-classic h2 {{ font-family: Georgia, "Times New Roman", serif; }}
.theme-classic .r-sec h2 {{ color: #1a1a1a; border-bottom-color: #1a1a1a; }}
.theme-classic .r-skills li {{ background: #f4f4f4; border-color: #e2e2e2; }}
/* Theme: modern — accent bar beside each section title */
.theme-modern .r-sec h2 {{
  border-bottom: none; padding-left: 10px; border-left: 4px solid var(--accent);
}}
/* Theme: compact — tighter for one-page fit */
.theme-compact {{ line-height: 1.32; }}
.theme-compact .resume {{ padding: 28px 34px; }}
.theme-compact .r-sec {{ margin-top: 12px; }}
.theme-compact .r-entry {{ margin-bottom: 8px; }}
.theme-compact .r-head h1 {{ font-size: 22px; }}
@page {{ size: {page}; margin: 0.6in; }}
@media print {{
  html, body {{ background: #fff; }}
  .resume {{ margin: 0; max-width: none; box-shadow: none; padding: 0; }}
  .r-entry, .r-sec {{ page-break-inside: avoid; }}
}}
</style>
</head>
<body>
<main class="resume theme-{theme}">
{body}</main>
</body>
</html>
"##,
        title = esc(name),
        accent = opts.accent,
        max_w = max_w,
        family = opts.font.family(),
        page = page,
        theme = opts.theme.class(),
        body = body,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options {
            theme: Theme::Modern,
            accent: "#2563eb".to_string(),
            font: Font::Sans,
            page_size: PageSize::Letter,
        }
    }

    const RESUME: &str = r#"{
        "name": "Ada Lovelace", "title": "Software Engineer",
        "email": "ada@example.com", "location": "London",
        "links": ["github.com/ada"],
        "summary": "Pioneering engineer.",
        "experience": [{"role":"Engineer","company":"Analytical Co","dates":"1843–1852","location":"London","bullets":["Wrote the first algorithm","Designed loops"]}],
        "education": [{"degree":"Mathematics","school":"Home tutoring","dates":"1830s","details":"Studied with De Morgan."}],
        "skills": ["Algorithms","Mathematics","Writing"],
        "sections": [{"heading":"Projects","items":["Analytical Engine notes"]}]
    }"#;

    #[test]
    fn renders_full_resume_html() {
        let html = build(RESUME, &opts()).unwrap();
        assert!(html.starts_with("<!DOCTYPE html>"));
        assert!(html.contains("<title>Ada Lovelace — Résumé</title>"));
        assert!(html.contains("<h1>Ada Lovelace</h1>"));
        assert!(html.contains("class=\"resume theme-modern\""));
        assert!(html.contains("--accent: #2563eb;"));
        assert!(html.contains("<h2>Experience</h2>"));
        assert!(html.contains("Engineer — Analytical Co"));
        assert!(html.contains("<li>Wrote the first algorithm</li>"));
        assert!(html.contains("<h2>Education</h2>"));
        assert!(html.contains("Studied with De Morgan."));
        assert!(html.contains("<h2>Skills</h2>"));
        assert!(html.contains("<h2>Projects</h2>"));
        assert!(html.contains("@page { size: letter;"));
        assert!(html.trim_end().ends_with("</html>"));
    }

    #[test]
    fn escapes_html_in_content() {
        let html = build(
            r#"{"name":"A <script>alert(1)</script> B","skills":["c++ & rust"]}"#,
            &opts(),
        )
        .unwrap();
        assert!(!html.contains("<script>alert(1)"));
        assert!(html.contains("&lt;script&gt;"));
        assert!(html.contains("c++ &amp; rust"));
    }

    #[test]
    fn theme_and_page_size_apply() {
        let o = Options {
            theme: Theme::Classic,
            accent: "navy".to_string(),
            font: Font::Serif,
            page_size: PageSize::A4,
        };
        let html = build(r#"{"name":"Bob"}"#, &o).unwrap();
        assert!(html.contains("theme-classic"));
        assert!(html.contains("--accent: navy;"));
        assert!(html.contains("@page { size: A4;"));
        assert!(html.contains("Georgia, Cambria"));
    }

    #[test]
    fn minimal_resume_just_name() {
        let html = build(r#"{"name":"Bob"}"#, &opts()).unwrap();
        assert!(html.contains("<h1>Bob</h1>"));
        assert!(!html.contains("<h2>Experience</h2>"));
        assert!(!html.contains("<h2>Skills</h2>"));
    }

    #[test]
    fn missing_name_errors() {
        assert!(build(r#"{"email":"x@y.com"}"#, &opts()).is_err());
    }

    #[test]
    fn bad_json_and_shape_error() {
        assert!(build("{not json", &opts()).is_err());
        assert!(build("   ", &opts()).is_err());
        assert!(build("[1,2]", &opts()).is_err());
    }

    #[test]
    fn accent_validation() {
        assert_eq!(sanitize_accent("").unwrap(), "#2563eb");
        assert_eq!(sanitize_accent("#0af").unwrap(), "#0af");
        assert_eq!(sanitize_accent("#22C55E").unwrap(), "#22C55E");
        assert_eq!(sanitize_accent("navy").unwrap(), "navy");
        assert!(sanitize_accent("red; } body{display:none").is_err());
        assert!(sanitize_accent("#zzz").is_err());
        assert!(sanitize_accent("#12345").is_err());
    }

    #[test]
    fn enum_parsers() {
        assert!(Theme::parse("MODERN").is_ok());
        assert!(Theme::parse("compact").is_ok());
        assert!(Theme::parse("wild").is_err());
        assert!(Font::parse("Serif").is_ok());
        assert!(Font::parse("comic").is_err());
        assert!(PageSize::parse("A4").is_ok());
        assert!(PageSize::parse("letter").is_ok());
        assert!(PageSize::parse("legal").is_err());
    }
}

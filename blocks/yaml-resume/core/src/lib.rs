//! gizza-ai/yaml-resume core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps.
//!
//! Renders a **JSON Resume v1.0.0** document — written as either YAML or JSON —
//! into a self-contained, print-ready HTML résumé: one file, styles embedded, with
//! an `@page` print stylesheet so the browser's Print → Save-as-PDF produces a
//! clean document.
//!
//! Scope note: this block is the renderer for the *standard* schema
//! (`basics`/`work`/`education`/…), which is what `resume-to-json` emits. The
//! sibling `resume-scaffolder` renders a different, bespoke flat shape and does
//! not read YAML — the two are deliberately not interchangeable.
//!
//! Every string that reaches the document is HTML-escaped and every URL is
//! scheme-checked, so free-form résumé content can never inject markup; the style
//! knobs are validated so they can never break out of the embedded CSS.

use serde_json::Value;

/// Which syntax the pasted document is written in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InputFormat {
    /// Sniff the document: a leading `{` is tried as JSON first, anything else as YAML.
    Auto,
    Yaml,
    Json,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Theme {
    /// Serif, centered header, ruled section titles.
    Classic,
    /// Sans, left header, an accent rule beside each section title.
    Modern,
    /// Tighter spacing to fit more on one page.
    Compact,
    /// Single column, no color and no rules — plain headings for résumé parsers.
    Ats,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Font {
    Sans,
    Serif,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PageSize {
    Letter,
    A4,
}

/// How ISO-8601 dates from the document are rendered.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DateFormat {
    /// `2020-01-15` → `Jan 2020`.
    MonthYear,
    /// `2020-01-15` → `2020`.
    Year,
    /// Printed exactly as written.
    Iso,
}

/// A validated set of presentation options for the rendered document.
pub struct Options {
    pub format: InputFormat,
    pub theme: Theme,
    /// How `startDate`/`endDate`/`date`/`releaseDate` values are printed.
    pub date_format: DateFormat,
    /// A safe CSS color (a `#hex` value or a plain CSS color name).
    pub accent: String,
    pub font: Font,
    /// Body type size in points.
    pub font_size: f64,
    pub page_size: PageSize,
    /// Print margin in inches, applied on all four sides.
    pub margin: f64,
    /// Canonical section keys, in render order. Empty = every section, canonical order.
    pub sections: Vec<String>,
}

/// The section keys this renderer knows, in the order they appear by default.
pub const SECTION_KEYS: [&str; 12] = [
    "summary",
    "work",
    "volunteer",
    "education",
    "projects",
    "skills",
    "awards",
    "certificates",
    "publications",
    "languages",
    "interests",
    "references",
];

impl InputFormat {
    pub fn parse(s: &str) -> Result<InputFormat, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" => Ok(InputFormat::Auto),
            "yaml" | "yml" => Ok(InputFormat::Yaml),
            "json" => Ok(InputFormat::Json),
            other => Err(format!(
                "invalid format {other:?}: expected one of auto, yaml, json"
            )),
        }
    }
}

impl Theme {
    pub fn parse(s: &str) -> Result<Theme, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "classic" => Ok(Theme::Classic),
            "modern" => Ok(Theme::Modern),
            "compact" => Ok(Theme::Compact),
            "ats" => Ok(Theme::Ats),
            other => Err(format!(
                "invalid theme {other:?}: expected one of classic, modern, compact, ats"
            )),
        }
    }
    fn class(self) -> &'static str {
        match self {
            Theme::Classic => "classic",
            Theme::Modern => "modern",
            Theme::Compact => "compact",
            Theme::Ats => "ats",
        }
    }
}

impl Font {
    pub fn parse(s: &str) -> Result<Font, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "sans" => Ok(Font::Sans),
            "serif" => Ok(Font::Serif),
            other => Err(format!(
                "invalid font {other:?}: expected one of sans, serif"
            )),
        }
    }
    fn family(self) -> &'static str {
        match self {
            Font::Sans => {
                "-apple-system, BlinkMacSystemFont, \"Segoe UI\", Roboto, Helvetica, Arial, sans-serif"
            }
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
    /// (`@page size` keyword, full page width in inches).
    fn dims(self) -> (&'static str, f64) {
        match self {
            PageSize::Letter => ("letter", 8.5),
            PageSize::A4 => ("A4", 8.27),
        }
    }
}

impl DateFormat {
    pub fn parse(s: &str) -> Result<DateFormat, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "month-year" | "month_year" => Ok(DateFormat::MonthYear),
            "year" => Ok(DateFormat::Year),
            "iso" => Ok(DateFormat::Iso),
            other => Err(format!(
                "invalid date_format {other:?}: expected one of month-year, year, iso"
            )),
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

/// Validate the body type size (points).
pub fn sanitize_font_size(pt: f64) -> Result<f64, String> {
    if !pt.is_finite() || !(8.0..=14.0).contains(&pt) {
        return Err(format!(
            "invalid font_size {pt}: expected a value between 8 and 14 points"
        ));
    }
    Ok(pt)
}

/// Validate the print margin (inches).
pub fn sanitize_margin(inches: f64) -> Result<f64, String> {
    if !inches.is_finite() || !(0.25..=1.5).contains(&inches) {
        return Err(format!(
            "invalid margin {inches}: expected a value between 0.25 and 1.5 inches"
        ));
    }
    Ok(inches)
}

/// Parse and validate the comma-separated `sections` list into canonical keys.
/// Blank means "every section, canonical order".
pub fn sanitize_sections(list: &str) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for raw in list.split(',') {
        let k = raw.trim().to_ascii_lowercase();
        if k.is_empty() {
            continue;
        }
        if !SECTION_KEYS.contains(&k.as_str()) {
            return Err(format!(
                "unknown section {k:?}: expected any of {}",
                SECTION_KEYS.join(", ")
            ));
        }
        if !out.contains(&k) {
            out.push(k);
        }
    }
    Ok(out)
}

fn s(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string()
}

/// A string list that tolerates a single bare string where the schema says array —
/// hand-written YAML does this constantly (`keywords: rust`).
fn str_list(v: &Value, key: &str) -> Vec<String> {
    match v.get(key) {
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(scalar_text)
            .filter(|x| !x.is_empty())
            .collect(),
        Some(other) => scalar_text(other)
            .filter(|x| !x.is_empty())
            .into_iter()
            .collect(),
        None => Vec::new(),
    }
}

/// Render a scalar YAML/JSON node as text (numbers and bools included — YAML
/// happily turns an unquoted `2020` or `score: 3.9` into a non-string node).
fn scalar_text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.trim().to_string()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// Like [`s`], but also accepts a scalar number/bool node.
fn sv(v: &Value, key: &str) -> String {
    v.get(key).and_then(scalar_text).unwrap_or_default()
}

fn arr<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    match v.get(key) {
        Some(Value::Array(a)) => a.as_slice(),
        _ => &[],
    }
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

/// Only http(s)/mailto links become anchors; anything else (notably `javascript:`)
/// is rendered as plain text. A bare `example.com/x` is promoted to https.
fn safe_href(url: &str) -> Option<String> {
    let t = url.trim();
    if t.is_empty() {
        return None;
    }
    let lower = t.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("mailto:")
    {
        return Some(t.to_string());
    }
    if !t.contains(':') && t.contains('.') && !t.contains(char::is_whitespace) {
        return Some(format!("https://{t}"));
    }
    None
}

/// A link showing `text`, or plain escaped text when the URL isn't linkable.
fn link(url: &str, text: &str) -> String {
    match safe_href(url) {
        Some(href) => format!(
            "<a href=\"{}\">{}</a>",
            esc(&href),
            esc(if text.trim().is_empty() { url } else { text })
        ),
        None => esc(if text.trim().is_empty() { url } else { text }),
    }
}

fn month_name(m: &str) -> Option<&'static str> {
    const NAMES: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    m.trim_start_matches('0')
        .parse::<usize>()
        .ok()
        .filter(|n| (1..=12).contains(n))
        .map(|n| NAMES[n - 1])
}

/// Format one ISO-8601-ish date. Anything that isn't `YYYY[-MM[-DD]]` is passed
/// through verbatim — plenty of real résumés carry `Summer 2019`.
fn fmt_date(raw: &str, f: DateFormat) -> String {
    let t = raw.trim();
    if t.is_empty() || f == DateFormat::Iso {
        return t.to_string();
    }
    let parts: Vec<&str> = t.split('-').collect();
    let year = parts[0];
    if year.len() != 4 || !year.chars().all(|c| c.is_ascii_digit()) {
        return t.to_string();
    }
    match f {
        DateFormat::Year => year.to_string(),
        DateFormat::MonthYear => match parts.get(1).and_then(|m| month_name(m)) {
            Some(m) => format!("{m} {year}"),
            None => year.to_string(),
        },
        DateFormat::Iso => t.to_string(),
    }
}

/// `startDate`–`endDate` as one label; an entry with a start but no end reads as
/// still current, which is what the schema means by an absent `endDate`.
fn date_range(v: &Value, f: DateFormat) -> String {
    let a = fmt_date(&sv(v, "startDate"), f);
    let b = fmt_date(&sv(v, "endDate"), f);
    match (a.is_empty(), b.is_empty()) {
        (true, true) => String::new(),
        (true, false) => b,
        (false, true) => format!("{a} – Present"),
        (false, false) => format!("{a} – {b}"),
    }
}

fn join_nonempty(parts: &[String], sep: &str) -> String {
    parts
        .iter()
        .filter(|p| !p.trim().is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(sep)
}

// ---------------------------------------------------------------- document

/// Parse a résumé document (YAML or JSON) into a `Value`, honouring `format`.
fn parse_doc(data: &str, format: InputFormat) -> Result<Value, String> {
    let t = data.trim();
    if t.is_empty() {
        return Err(
            "input is empty — paste a JSON Resume document (YAML or JSON) with a basics.name field"
                .into(),
        );
    }
    match format {
        InputFormat::Json => serde_json::from_str(t).map_err(|e| format!("invalid JSON: {e}")),
        InputFormat::Yaml => serde_yml::from_str(t).map_err(|e| format!("invalid YAML: {e}")),
        InputFormat::Auto => {
            if t.starts_with('{') {
                serde_json::from_str(t)
                    .or_else(|_| serde_yml::from_str(t))
                    .map_err(|e| format!("invalid JSON/YAML: {e}"))
            } else {
                serde_yml::from_str(t).map_err(|e| {
                    format!("invalid YAML: {e} (if this document is JSON, set format=json)")
                })
            }
        }
    }
}

/// Build a print-ready HTML résumé from a JSON Resume v1.0.0 document.
///
/// `data` is the document as YAML or JSON. Recognized top-level keys:
/// `basics` (`name` required; `label`, `email`, `phone`, `url`, `summary`,
/// `location{address,postalCode,city,region,countryCode}`,
/// `profiles[{network,username,url}]`), `work`, `volunteer`, `education`,
/// `projects`, `skills`, `awards`, `certificates`, `publications`, `languages`,
/// `interests`, `references`. Unknown keys are ignored.
pub fn build(data: &str, opts: &Options) -> Result<String, String> {
    let v = parse_doc(data, opts.format)?;
    if !v.is_object() {
        return Err(
            "expected a JSON Resume document (a mapping with a basics section), not a list or scalar"
                .into(),
        );
    }
    let basics = v.get("basics").cloned().unwrap_or(Value::Null);
    let name = s(&basics, "name");
    if name.is_empty() {
        let hint = if v.get("basics").is_some() {
            "the basics section has no name"
        } else if v.get("name").is_some() {
            "found a top-level name — JSON Resume nests it under basics: { name: ... }"
        } else {
            "no basics section found"
        };
        return Err(format!("resume is missing basics.name ({hint})"));
    }

    let mut body = String::new();
    body.push_str(&render_header(&basics, &name));

    let order: Vec<String> = if opts.sections.is_empty() {
        SECTION_KEYS.iter().map(|k| k.to_string()).collect()
    } else {
        opts.sections.clone()
    };
    for key in &order {
        body.push_str(&render_section(key, &v, &basics, opts));
    }

    Ok(document(&name, &body, opts))
}

fn render_header(basics: &Value, name: &str) -> String {
    let mut h = String::from("<header class=\"r-head\">\n");
    h.push_str(&format!("<h1>{}</h1>\n", esc(name)));
    let label = s(basics, "label");
    if !label.is_empty() {
        h.push_str(&format!("<p class=\"r-label\">{}</p>\n", esc(&label)));
    }

    // Contact line: email · phone · location · site.
    let mut bits: Vec<String> = Vec::new();
    let email = s(basics, "email");
    if !email.is_empty() {
        bits.push(link(&format!("mailto:{email}"), &email));
    }
    let phone = sv(basics, "phone");
    if !phone.is_empty() {
        bits.push(esc(&phone));
    }
    let loc = location_line(basics);
    if !loc.is_empty() {
        bits.push(esc(&loc));
    }
    let url = s(basics, "url");
    if !url.is_empty() {
        bits.push(link(&url, url.trim_start_matches("https://")));
    }
    if !bits.is_empty() {
        h.push_str(&format!(
            "<p class=\"r-contact\">{}</p>\n",
            bits.join("<span class=\"sep\">·</span>")
        ));
    }

    // Profiles line.
    let profiles: Vec<String> = arr(basics, "profiles")
        .iter()
        .filter_map(|p| {
            let network = s(p, "network");
            let username = sv(p, "username");
            let purl = s(p, "url");
            let text = match (network.is_empty(), username.is_empty()) {
                (false, false) => format!("{network}: {username}"),
                (false, true) => network.clone(),
                (true, false) => username.clone(),
                (true, true) => purl.clone(),
            };
            if text.trim().is_empty() {
                return None;
            }
            Some(link(&purl, &text))
        })
        .collect();
    if !profiles.is_empty() {
        h.push_str(&format!(
            "<p class=\"r-profiles\">{}</p>\n",
            profiles.join("<span class=\"sep\">·</span>")
        ));
    }
    h.push_str("</header>\n");
    h
}

fn location_line(basics: &Value) -> String {
    let loc = match basics.get("location") {
        Some(l) if l.is_object() => l.clone(),
        // Some hand-written documents flatten it to a plain string.
        Some(Value::String(s)) => return s.trim().to_string(),
        _ => return String::new(),
    };
    let city_region = join_nonempty(&[sv(&loc, "city"), sv(&loc, "region")], ", ");
    join_nonempty(
        &[
            sv(&loc, "address"),
            city_region,
            sv(&loc, "postalCode"),
            sv(&loc, "countryCode"),
        ],
        ", ",
    )
}

fn section_open(title: &str) -> String {
    format!("<section class=\"r-sec\">\n<h2>{}</h2>\n", esc(title))
}

/// One entry: a two-line head (title — org, dates / sub-line), an optional
/// summary paragraph, optional bullets, and an optional keyword line.
struct Entry {
    title: String,
    org: String,
    meta: String,
    sub: Vec<String>,
    url: String,
    summary: String,
    bullets: Vec<String>,
    keywords: Vec<String>,
}

impl Entry {
    fn render(&self) -> String {
        let mut o = String::from("<article class=\"r-item\">\n<div class=\"r-item-head\">\n");
        let mut main = String::new();
        if !self.title.is_empty() {
            main.push_str(&format!(
                "<span class=\"r-item-title\">{}</span>",
                esc(&self.title)
            ));
        }
        if !self.org.is_empty() {
            let org = if self.url.is_empty() {
                esc(&self.org)
            } else {
                link(&self.url, &self.org)
            };
            if main.is_empty() {
                main.push_str(&format!("<span class=\"r-item-title\">{org}</span>"));
            } else {
                main.push_str(&format!("<span class=\"r-item-org\"> — {org}</span>"));
            }
        }
        o.push_str(&format!("<div class=\"r-item-main\">{main}</div>\n"));
        if !self.meta.is_empty() {
            o.push_str(&format!(
                "<div class=\"r-item-meta\">{}</div>\n",
                esc(&self.meta)
            ));
        }
        o.push_str("</div>\n");

        let sub = join_nonempty(&self.sub, " · ");
        if !sub.is_empty() {
            o.push_str(&format!("<p class=\"r-item-sub\">{}</p>\n", esc(&sub)));
        }
        if !self.summary.is_empty() {
            o.push_str(&format!(
                "<p class=\"r-item-summary\">{}</p>\n",
                esc(&self.summary)
            ));
        }
        if !self.bullets.is_empty() {
            o.push_str("<ul class=\"r-bullets\">\n");
            for b in &self.bullets {
                o.push_str(&format!("<li>{}</li>\n", esc(b)));
            }
            o.push_str("</ul>\n");
        }
        if !self.keywords.is_empty() {
            o.push_str(&format!(
                "<p class=\"r-keywords\">{}</p>\n",
                esc(&self.keywords.join(" · "))
            ));
        }
        o.push_str("</article>\n");
        o
    }
}

fn render_section(key: &str, v: &Value, basics: &Value, opts: &Options) -> String {
    let df = opts.date_format;
    match key {
        "summary" => {
            let summary = s(basics, "summary");
            if summary.is_empty() {
                return String::new();
            }
            format!(
                "{}<p class=\"r-summary\">{}</p>\n</section>\n",
                section_open("Summary"),
                esc(&summary)
            )
        }
        "work" => entries_section(v, "work", "Experience", df, |e, df| Entry {
            title: s(e, "position"),
            org: sv(e, "name"),
            meta: date_range(e, df),
            sub: vec![sv(e, "location"), sv(e, "description")],
            url: s(e, "url"),
            summary: s(e, "summary"),
            bullets: str_list(e, "highlights"),
            keywords: Vec::new(),
        }),
        "volunteer" => entries_section(v, "volunteer", "Volunteering", df, |e, df| Entry {
            title: s(e, "position"),
            org: sv(e, "organization"),
            meta: date_range(e, df),
            sub: vec![sv(e, "location")],
            url: s(e, "url"),
            summary: s(e, "summary"),
            bullets: str_list(e, "highlights"),
            keywords: Vec::new(),
        }),
        "education" => entries_section(v, "education", "Education", df, |e, df| {
            let study = sv(e, "studyType");
            let area = sv(e, "area");
            let score = sv(e, "score");
            Entry {
                title: join_nonempty(&[study, area], ", "),
                org: sv(e, "institution"),
                meta: date_range(e, df),
                sub: vec![
                    sv(e, "location"),
                    if score.is_empty() {
                        String::new()
                    } else {
                        format!("Score: {score}")
                    },
                ],
                url: s(e, "url"),
                summary: s(e, "summary"),
                bullets: str_list(e, "courses"),
                keywords: Vec::new(),
            }
        }),
        "projects" => entries_section(v, "projects", "Projects", df, |e, df| {
            let roles = str_list(e, "roles").join(", ");
            Entry {
                title: sv(e, "name"),
                org: sv(e, "entity"),
                meta: date_range(e, df),
                sub: vec![roles, sv(e, "type")],
                url: s(e, "url"),
                summary: s(e, "description"),
                bullets: str_list(e, "highlights"),
                keywords: str_list(e, "keywords"),
            }
        }),
        "skills" => grouped_section(v, "skills", "Skills", "name", "level"),
        "interests" => grouped_section(v, "interests", "Interests", "name", ""),
        "awards" => entries_section(v, "awards", "Awards", df, |e, df| Entry {
            title: sv(e, "title"),
            org: sv(e, "awarder"),
            meta: fmt_date(&sv(e, "date"), df),
            sub: Vec::new(),
            url: s(e, "url"),
            summary: s(e, "summary"),
            bullets: Vec::new(),
            keywords: Vec::new(),
        }),
        "certificates" => entries_section(v, "certificates", "Certificates", df, |e, df| Entry {
            title: sv(e, "name"),
            org: sv(e, "issuer"),
            meta: fmt_date(&sv(e, "date"), df),
            sub: Vec::new(),
            url: s(e, "url"),
            summary: s(e, "summary"),
            bullets: Vec::new(),
            keywords: Vec::new(),
        }),
        "publications" => entries_section(v, "publications", "Publications", df, |e, df| Entry {
            title: sv(e, "name"),
            org: sv(e, "publisher"),
            meta: fmt_date(&sv(e, "releaseDate"), df),
            sub: Vec::new(),
            url: s(e, "url"),
            summary: s(e, "summary"),
            bullets: Vec::new(),
            keywords: Vec::new(),
        }),
        "languages" => {
            let items: Vec<String> = arr(v, "languages")
                .iter()
                .filter_map(|l| {
                    let lang = sv(l, "language");
                    if lang.is_empty() {
                        return None;
                    }
                    let fluency = sv(l, "fluency");
                    Some(if fluency.is_empty() {
                        format!("<li><b>{}</b></li>", esc(&lang))
                    } else {
                        format!("<li><b>{}</b> — {}</li>", esc(&lang), esc(&fluency))
                    })
                })
                .collect();
            if items.is_empty() {
                return String::new();
            }
            format!(
                "{}<ul class=\"r-inline\">{}</ul>\n</section>\n",
                section_open("Languages"),
                items.join("")
            )
        }
        "references" => {
            let items: Vec<String> = arr(v, "references")
                .iter()
                .filter_map(|r| {
                    let rname = sv(r, "name");
                    let text = s(r, "reference");
                    if rname.is_empty() && text.is_empty() {
                        return None;
                    }
                    let mut o = String::from("<article class=\"r-item\">\n");
                    if !text.is_empty() {
                        o.push_str(&format!("<p class=\"r-quote\">{}</p>\n", esc(&text)));
                    }
                    if !rname.is_empty() {
                        o.push_str(&format!("<p class=\"r-cite\">— {}</p>\n", esc(&rname)));
                    }
                    o.push_str("</article>\n");
                    Some(o)
                })
                .collect();
            if items.is_empty() {
                return String::new();
            }
            format!(
                "{}{}</section>\n",
                section_open("References"),
                items.join("")
            )
        }
        _ => String::new(),
    }
}

fn entries_section(
    v: &Value,
    key: &str,
    title: &str,
    df: DateFormat,
    make: impl Fn(&Value, DateFormat) -> Entry,
) -> String {
    let rows = arr(v, key);
    let rendered: Vec<String> = rows
        .iter()
        .map(|e| make(e, df))
        .filter(|e| {
            !(e.title.is_empty()
                && e.org.is_empty()
                && e.summary.is_empty()
                && e.bullets.is_empty())
        })
        .map(|e| e.render())
        .collect();
    if rendered.is_empty() {
        return String::new();
    }
    format!("{}{}</section>\n", section_open(title), rendered.join(""))
}

/// `skills` / `interests`: a bolded group name with its keywords beneath.
fn grouped_section(v: &Value, key: &str, title: &str, name_key: &str, level_key: &str) -> String {
    let rows: Vec<String> = arr(v, key)
        .iter()
        .filter_map(|g| {
            let gname = sv(g, name_key);
            let keywords = str_list(g, "keywords");
            if gname.is_empty() && keywords.is_empty() {
                return None;
            }
            let level = if level_key.is_empty() {
                String::new()
            } else {
                sv(g, level_key)
            };
            let head = if level.is_empty() {
                esc(&gname)
            } else {
                format!(
                    "{} <span class=\"r-level\">({})</span>",
                    esc(&gname),
                    esc(&level)
                )
            };
            Some(if keywords.is_empty() {
                format!("<li><b>{head}</b></li>")
            } else if gname.is_empty() {
                format!("<li>{}</li>", esc(&keywords.join(", ")))
            } else {
                format!("<li><b>{head}</b> — {}</li>", esc(&keywords.join(", ")))
            })
        })
        .collect();
    if rows.is_empty() {
        return String::new();
    }
    format!(
        "{}<ul class=\"r-groups\">{}</ul>\n</section>\n",
        section_open(title),
        rows.join("")
    )
}

/// Wrap the rendered body in a self-contained HTML document with embedded
/// screen + print styles.
fn document(name: &str, body: &str, opts: &Options) -> String {
    let (page_kw, page_w) = opts.page_size.dims();
    let content_w = page_w - 2.0 * opts.margin;
    let theme = opts.theme.class();
    let family = opts.font.family();
    let accent = &opts.accent;
    let fs = opts.font_size;
    let margin = opts.margin;
    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title} — Résumé</title>
<style>
:root {{
  --accent: {accent};
  --ink: #14171f;
  --muted: #5b6472;
  --rule: #d8dde5;
  --fs: {fs}pt;
  --content: {content_w:.2}in;
}}
* {{ box-sizing: border-box; }}
html {{ background: #f2f4f7; }}
body {{
  margin: 0;
  padding: 24px 16px;
  font-family: {family};
  font-size: var(--fs);
  line-height: 1.42;
  color: var(--ink);
}}
.resume {{
  max-width: var(--content);
  margin: 0 auto;
  background: #fff;
  padding: 0.45in 0.5in;
  box-shadow: 0 1px 3px rgba(16, 24, 40, .12), 0 8px 24px rgba(16, 24, 40, .08);
  border-radius: 4px;
}}
a {{ color: inherit; text-decoration: none; border-bottom: 1px solid var(--rule); }}
h1 {{ font-size: 1.85em; margin: 0 0 .12em; letter-spacing: -.01em; }}
h2 {{
  font-size: .92em; text-transform: uppercase; letter-spacing: .09em;
  margin: 0 0 .5em; color: var(--accent);
}}
.r-head {{ margin-bottom: 1.1em; }}
.r-label {{ margin: 0 0 .45em; font-size: 1.06em; color: var(--muted); }}
.r-contact, .r-profiles {{ margin: .2em 0; color: var(--muted); font-size: .92em; }}
.sep {{ margin: 0 .45em; opacity: .55; }}
.r-sec {{ margin: 0 0 1.05em; }}
.r-sec:last-child {{ margin-bottom: 0; }}
.r-summary {{ margin: 0; }}
.r-item {{ margin: 0 0 .72em; page-break-inside: avoid; break-inside: avoid; }}
.r-item:last-child {{ margin-bottom: 0; }}
.r-item-head {{ display: flex; justify-content: space-between; align-items: baseline; gap: 1em; }}
.r-item-title {{ font-weight: 700; }}
.r-item-org {{ color: var(--muted); }}
.r-item-meta {{ color: var(--muted); font-size: .88em; white-space: nowrap; }}
.r-item-sub {{ margin: .06em 0 0; color: var(--muted); font-size: .88em; }}
.r-item-summary {{ margin: .3em 0 0; }}
.r-bullets {{ margin: .3em 0 0; padding-left: 1.15em; }}
.r-bullets li {{ margin: .12em 0; }}
.r-keywords {{ margin: .28em 0 0; color: var(--muted); font-size: .86em; }}
.r-groups, .r-inline {{ list-style: none; margin: 0; padding: 0; }}
.r-groups li, .r-inline li {{ margin: .16em 0; }}
.r-level {{ color: var(--muted); font-weight: 400; }}
.r-quote {{ margin: 0; font-style: italic; }}
.r-cite {{ margin: .16em 0 0; color: var(--muted); font-size: .9em; }}

/* Theme: classic — centered header, ruled section titles, ink not accent. */
.theme-classic .r-head {{ text-align: center; }}
.theme-classic h2 {{
  color: var(--ink); border-bottom: 1px solid var(--ink);
  padding-bottom: .2em; margin-bottom: .55em;
}}
/* Theme: modern — an accent rule to the left of every section title. */
.theme-modern h2 {{ border-left: 3px solid var(--accent); padding-left: .5em; }}
/* Theme: compact — tighter everything, one more entry per page. */
.theme-compact {{ line-height: 1.3; }}
.theme-compact .resume {{ padding: .35in .4in; }}
.theme-compact .r-sec {{ margin-bottom: .7em; }}
.theme-compact .r-item {{ margin-bottom: .45em; }}
.theme-compact h1 {{ font-size: 1.6em; }}
/* Theme: ats — no color, no rules, plain headings for résumé parsers. */
.theme-ats h2 {{ color: var(--ink); font-weight: 700; letter-spacing: .04em; }}
.theme-ats a {{ border-bottom: 0; }}
.theme-ats .r-item-head {{ display: block; }}
.theme-ats .r-item-meta {{ white-space: normal; }}
.theme-ats .resume {{ box-shadow: none; border-radius: 0; }}

@media print {{
  @page {{ size: {page_kw}; margin: {margin}in; }}
  html, body {{ background: #fff; }}
  body {{ padding: 0; }}
  .resume {{
    max-width: none; margin: 0; padding: 0;
    box-shadow: none; border-radius: 0;
  }}
  a {{ border-bottom: 0; }}
}}
</style>
</head>
<body class="theme-{theme}">
<main class="resume">
{body}</main>
</body>
</html>
"##,
        title = esc(name),
    )
}

// ---------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options {
            format: InputFormat::Auto,
            theme: Theme::Modern,
            date_format: DateFormat::MonthYear,
            accent: "#2563eb".into(),
            font: Font::Sans,
            font_size: 10.5,
            page_size: PageSize::Letter,
            margin: 0.5,
            sections: Vec::new(),
        }
    }

    const YAML_DOC: &str = r#"
basics:
  name: Ada Lovelace
  label: Analytical Engineer
  email: ada@example.com
  phone: "+44 20 7123 4567"
  url: https://example.com/ada
  summary: Wrote the first published algorithm.
  location:
    city: London
    countryCode: UK
  profiles:
    - network: GitHub
      username: ada
      url: https://github.com/ada
work:
  - name: Analytical Engine Co
    position: Lead Engineer
    startDate: "1843-01"
    endDate: "1852-11"
    summary: Designed programs for a mechanical computer.
    highlights:
      - Published the first algorithm intended for a machine
education:
  - institution: Home tutoring
    studyType: Study
    area: Mathematics
    startDate: "1830"
skills:
  - name: Mathematics
    level: Expert
    keywords: [Algorithms, Analysis]
languages:
  - language: English
    fluency: Native speaker
"#;

    #[test]
    fn renders_a_yaml_json_resume_document() {
        let html = build(YAML_DOC, &opts()).unwrap();
        assert!(html.contains("<h1>Ada Lovelace</h1>"));
        assert!(html.contains("Analytical Engineer"));
        // ISO dates become month-year, an open range reads as Present.
        assert!(html.contains("Jan 1843 – Nov 1852"), "{html}");
        assert!(html.contains("1830 – Present"), "{html}");
        // Canonical sections render with friendly headings.
        for h in ["Summary", "Experience", "Education", "Skills", "Languages"] {
            assert!(html.contains(&format!("<h2>{h}</h2>")), "missing {h}");
        }
        assert!(html.contains("Published the first algorithm intended for a machine"));
        assert!(html.contains("Algorithms, Analysis"));
        assert!(html.contains("theme-modern"));
        assert!(
            html.contains("@page { size: letter; margin: 0.5in; }"),
            "{html}"
        );
    }

    #[test]
    fn accepts_the_same_document_as_json() {
        let json = r#"{"basics":{"name":"Ada Lovelace"},"work":[{"name":"Analytical Engine Co","position":"Lead Engineer"}]}"#;
        let html = build(json, &opts()).unwrap();
        assert!(html.contains("<h1>Ada Lovelace</h1>"));
        assert!(html.contains("Lead Engineer"));
        // …and explicitly, with format=json.
        let mut o = opts();
        o.format = InputFormat::Json;
        assert!(build(json, &o).unwrap().contains("Analytical Engine Co"));
    }

    #[test]
    fn errors_without_basics_name() {
        let err = build("basics:\n  label: Engineer\n", &opts()).unwrap_err();
        assert!(err.contains("basics.name"), "{err}");
        assert!(err.contains("has no name"), "{err}");
    }

    #[test]
    fn errors_helpfully_on_a_flat_top_level_name() {
        let err = build(r#"{"name":"Ada"}"#, &opts()).unwrap_err();
        assert!(err.contains("nests it under basics"), "{err}");
    }

    #[test]
    fn errors_on_empty_and_malformed_input() {
        assert!(build("   ", &opts())
            .unwrap_err()
            .contains("input is empty"));
        let mut o = opts();
        o.format = InputFormat::Json;
        assert!(build("{oops", &o).unwrap_err().contains("invalid JSON"));
        o.format = InputFormat::Yaml;
        assert!(build("basics: [unterminated", &o)
            .unwrap_err()
            .contains("invalid YAML"));
    }

    #[test]
    fn errors_on_a_non_mapping_document() {
        let err = build("- one\n- two\n", &opts()).unwrap_err();
        assert!(err.contains("not a list or scalar"), "{err}");
    }

    #[test]
    fn date_formats_cover_every_choice() {
        assert_eq!(fmt_date("2020-01-15", DateFormat::MonthYear), "Jan 2020");
        assert_eq!(fmt_date("2020-01", DateFormat::MonthYear), "Jan 2020");
        assert_eq!(fmt_date("2020", DateFormat::MonthYear), "2020");
        assert_eq!(fmt_date("2020-01-15", DateFormat::Year), "2020");
        assert_eq!(fmt_date("2020-01-15", DateFormat::Iso), "2020-01-15");
        // Non-ISO text passes through untouched.
        assert_eq!(
            fmt_date("Summer 2019", DateFormat::MonthYear),
            "Summer 2019"
        );
        assert_eq!(fmt_date("2020-13", DateFormat::MonthYear), "2020");
    }

    #[test]
    fn section_list_filters_and_reorders() {
        let mut o = opts();
        o.sections = sanitize_sections("skills, work").unwrap();
        o.date_format = DateFormat::MonthYear;
        let html = build(YAML_DOC, &o).unwrap();
        let skills = html.find("<h2>Skills</h2>").expect("skills rendered");
        let work = html.find("<h2>Experience</h2>").expect("work rendered");
        assert!(skills < work, "requested order not honoured");
        assert!(!html.contains("<h2>Education</h2>"));
        assert!(!html.contains("<h2>Summary</h2>"));
    }

    #[test]
    fn section_list_rejects_an_unknown_name() {
        let err = sanitize_sections("work, hobbies").unwrap_err();
        assert!(err.contains("unknown section \"hobbies\""), "{err}");
        assert!(err.contains("publications"), "{err}");
    }

    #[test]
    fn style_options_reach_the_document() {
        let mut o = opts();
        o.theme = Theme::Ats;
        o.accent = "navy".into();
        o.font = Font::Serif;
        o.font_size = 12.0;
        o.page_size = PageSize::A4;
        o.margin = 0.75;
        let html = build(YAML_DOC, &o).unwrap();
        assert!(html.contains("theme-ats"));
        assert!(html.contains("--accent: navy;"));
        assert!(html.contains("Georgia"));
        assert!(html.contains("--fs: 12pt;"));
        assert!(
            html.contains("@page { size: A4; margin: 0.75in; }"),
            "{html}"
        );
        // A4 (8.27in) minus two 0.75in margins.
        assert!(html.contains("--content: 6.77in;"), "{html}");
    }

    #[test]
    fn rejects_out_of_range_style_values() {
        assert!(sanitize_font_size(20.0).unwrap_err().contains("font_size"));
        assert!(sanitize_margin(3.0).unwrap_err().contains("margin"));
        assert!(sanitize_accent("red; }").unwrap_err().contains("accent"));
        assert!(Theme::parse("sparkly")
            .unwrap_err()
            .contains("invalid theme"));
        assert!(InputFormat::parse("xml")
            .unwrap_err()
            .contains("expected one of auto, yaml, json"));
        assert!(DateFormat::parse("euro")
            .unwrap_err()
            .contains("date_format"));
    }

    #[test]
    fn escapes_content_and_refuses_unsafe_links() {
        let doc = r#"
basics:
  name: "<script>alert(1)</script>"
  url: "javascript:alert(1)"
"#;
        let html = build(doc, &opts()).unwrap();
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(!html.contains("<script>alert"));
        // A javascript: URL is printed as text, never as an href.
        assert!(!html.contains("href=\"javascript:"), "{html}");
        assert!(html.contains("javascript:alert(1)"));
    }

    #[test]
    fn tolerates_scalar_and_loose_yaml_shapes() {
        let doc = r#"
basics:
  name: Ada
  phone: 5551234
  location: London, UK
skills:
  - name: Rust
    keywords: systems
education:
  - institution: MIT
    score: 3.9
"#;
        let html = build(doc, &opts()).unwrap();
        assert!(html.contains("5551234"));
        assert!(html.contains("London, UK"));
        assert!(html.contains("systems"));
        assert!(html.contains("Score: 3.9"), "{html}");
    }

    #[test]
    fn renders_every_remaining_section() {
        let doc = r#"
basics: { name: Ada }
volunteer: [{ organization: Red Cross, position: Helper }]
projects: [{ name: Engine, description: A machine, keywords: [gears] }]
awards: [{ title: Medal, awarder: Society, date: "1844-05" }]
certificates: [{ name: Cert, issuer: Board, date: "1845" }]
publications: [{ name: Notes, publisher: Taylor, releaseDate: "1843-09" }]
interests: [{ name: Music, keywords: [piano] }]
references: [{ name: Charles Babbage, reference: An excellent collaborator. }]
"#;
        let html = build(doc, &opts()).unwrap();
        for h in [
            "Volunteering",
            "Projects",
            "Awards",
            "Certificates",
            "Publications",
            "Interests",
            "References",
        ] {
            assert!(
                html.contains(&format!("<h2>{h}</h2>")),
                "missing {h}\n{html}"
            );
        }
        assert!(html.contains("May 1844"));
        assert!(html.contains("Sep 1843"));
        assert!(html.contains("An excellent collaborator."));
        assert!(html.contains("— Charles Babbage"));
    }

    #[test]
    fn date_format_choice_reaches_rendered_entries() {
        let mut o = opts();
        o.date_format = DateFormat::Year;
        let html = build(YAML_DOC, &o).unwrap();
        assert!(html.contains("1843 – 1852"), "{html}");
        o.date_format = DateFormat::Iso;
        let html = build(YAML_DOC, &o).unwrap();
        assert!(html.contains("1843-01 – 1852-11"), "{html}");
    }

    #[test]
    fn output_is_one_self_contained_printable_document() {
        let html = build(YAML_DOC, &opts()).unwrap();
        assert!(html.starts_with("<!DOCTYPE html>"));
        assert!(html.contains("<title>Ada Lovelace — Résumé</title>"));
        // Styles are embedded, so the file works with no network at all.
        assert!(html.contains("<style>"));
        assert!(!html.contains("<link"));
        assert!(!html.contains("<script"));
        assert!(html.contains("@media print"));
        assert!(html.trim_end().ends_with("</html>"));
    }

    #[test]
    fn explicit_yaml_format_and_auto_sniffing_agree() {
        let mut o = opts();
        o.format = InputFormat::Yaml;
        let a = build(YAML_DOC, &o).unwrap();
        o.format = InputFormat::Auto;
        let b = build(YAML_DOC, &o).unwrap();
        assert_eq!(a, b);
        // A JSON document parsed as YAML still works — YAML is a JSON superset.
        assert!(build(r#"{"basics":{"name":"Ada"}}"#, &o)
            .unwrap()
            .contains("<h1>Ada</h1>"));
    }

    #[test]
    fn omits_sections_that_have_no_usable_rows() {
        let html = build("basics: { name: Ada }\nwork: []\nskills: []\n", &opts()).unwrap();
        assert!(!html.contains("<h2>Experience</h2>"));
        assert!(!html.contains("<h2>Skills</h2>"));
        assert!(html.contains("<h1>Ada</h1>"));
    }
}

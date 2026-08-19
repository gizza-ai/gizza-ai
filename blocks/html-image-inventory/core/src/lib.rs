//! gizza-ai/html-image-inventory core — walk a chunk of HTML and build a table
//! of every image source it declares: each `<img>`, each `<picture><source>`
//! candidate, and the attributes that decide how the image renders — `alt`,
//! `width`/`height`, `loading`, `decoding`, `fetchpriority`, `srcset`, `sizes`,
//! `media`, `type`, plus `class`/`id`/`title` for locating the element again.
//!
//! Two defects are flagged because both are named audit rules rather than
//! opinions: an `<img>` with **no `alt` attribute at all** (an accessibility
//! failure — an explicit `alt=""` is the spec's decorative marker and is NOT a
//! defect), and an `<img>` with **no usable `width`/`height` content
//! attribute** (the browser reserves no space, so the page shifts as the image
//! loads — a Cumulative Layout Shift contributor). The content attributes must
//! be valid non-negative integers, so `width="50%"` is reported verbatim and
//! still flagged.
//!
//! Parsing uses `scraper` (html5ever, wasm32-safe) rather than a regex: the
//! `<picture>` → `<source>` → `<img>` relationship this tool reports is
//! structural, and real markup is full of unquoted attributes and implied
//! closing tags.
//!
//! No wafer / wasm-bindgen deps here — this crate is pure logic so both the
//! block and the browser wrapper can share it.

use scraper::{ElementRef, Html, Selector};
use serde_json::{json, Map, Value};

/// Hard cap on how many rows a single run will report. Keeps the 64 MiB wasm
/// sandbox comfortable and keeps a whole-site paste from producing a megabyte
/// of table. Exceeding it is a loud error, never a silent truncation.
pub const MAX_IMAGES: usize = 2000;

/// Output shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Markdown,
    Csv,
    Json,
}

pub fn parse_format(s: &str) -> Result<Format, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "markdown" | "md" => Ok(Format::Markdown),
        "csv" => Ok(Format::Csv),
        "json" => Ok(Format::Json),
        other => Err(format!(
            "format {other:?} not supported (expected markdown, csv, or json)"
        )),
    }
}

/// Run-time switches, mirrored 1:1 by the block descriptor and the page fields.
#[derive(Debug, Clone)]
pub struct Options {
    /// Include `<picture><source>` candidate rows (default: on).
    pub include_sources: bool,
    /// List only rows that carry at least one issue (default: off).
    pub only_issues: bool,
    /// Treat a decorative `alt=""` as an issue too (default: off — an explicit
    /// empty alt is the correct markup for a decorative image).
    pub flag_empty_alt: bool,
    /// Prepend the counts summary (default: on).
    pub include_summary: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            include_sources: true,
            only_issues: false,
            flag_empty_alt: false,
            include_summary: true,
        }
    }
}

/// Three-state `alt`, because "no alt attribute" and `alt=""` mean opposite
/// things and collapsing them to a boolean loses the only distinction that
/// matters to a screen reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AltState {
    /// `alt` is present and non-empty.
    Present,
    /// `alt=""` (or whitespace only) — an explicitly decorative image.
    Empty,
    /// No `alt` attribute at all.
    Missing,
}

impl AltState {
    pub fn as_str(self) -> &'static str {
        match self {
            AltState::Present => "present",
            AltState::Empty => "empty",
            AltState::Missing => "missing",
        }
    }
}

/// One row of the inventory: either an `<img>` or one `<picture><source>`
/// candidate.
#[derive(Debug, Clone)]
pub struct ImageRow {
    /// 1-based position in the reported table.
    pub index: usize,
    /// `img` or `source`.
    pub element: String,
    /// 1-based index of the enclosing `<picture>`, when there is one.
    pub picture: Option<usize>,
    pub src: String,
    pub srcset: String,
    pub sizes: String,
    /// `<source media="…">` — the art-direction breakpoint.
    pub media: String,
    /// `<source type="image/webp">` — the candidate's MIME type.
    pub mime_type: String,
    /// Alt text as written (empty for `alt=""`, for a missing `alt`, and for a
    /// `<source>`, which has no alt of its own).
    pub alt: String,
    pub alt_state: AltState,
    /// `width` / `height` content attributes, verbatim (may be non-numeric).
    pub width: String,
    pub height: String,
    pub loading: String,
    pub decoding: String,
    pub fetchpriority: String,
    pub class: String,
    pub id: String,
    pub title: String,
    /// Issue tokens, e.g. `missing-alt`, `missing-width`.
    pub issues: Vec<String>,
}

impl ImageRow {
    /// `<source>` rows carry no `alt` of their own — the `<img>` inside the
    /// same `<picture>` supplies it for every candidate.
    pub fn is_source(&self) -> bool {
        self.element == "source"
    }
}

/// Whole-run counts, computed over every image found (before `only_issues`
/// filtering) so the totals never lie about what was on the page.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Summary {
    /// `<img>` elements found.
    pub images: usize,
    /// `<picture><source>` candidates found.
    pub sources: usize,
    /// `<picture>` elements found.
    pub pictures: usize,
    /// Images with no `alt` attribute.
    pub missing_alt: usize,
    /// Images with an explicit `alt=""`.
    pub empty_alt: usize,
    /// Images missing a usable `width` and/or `height`.
    pub missing_dimensions: usize,
    /// Images carrying `loading="lazy"`.
    pub lazy: usize,
    /// Rows carrying at least one issue.
    pub flagged: usize,
}

/// The full parse result: the counts plus the rows that survived filtering.
#[derive(Debug, Clone)]
pub struct Inventory {
    pub summary: Summary,
    pub rows: Vec<ImageRow>,
    /// Rows dropped by `only_issues`.
    pub hidden: usize,
}

// ---------------------------------------------------------------- helpers

/// Collapse runs of whitespace (incl. newlines) to single spaces and trim.
fn collapse(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Trimmed attribute value, empty when absent.
fn attr(el: &ElementRef, name: &str) -> String {
    el.value().attr(name).unwrap_or("").trim().to_string()
}

/// Attribute value with internal whitespace collapsed — right for `srcset`,
/// `sizes` and `class`, which are whitespace-separated lists that authors wrap
/// across lines.
fn attr_collapsed(el: &ElementRef, name: &str) -> String {
    collapse(el.value().attr(name).unwrap_or(""))
}

fn sel(s: &str) -> Result<Selector, String> {
    Selector::parse(s).map_err(|e| format!("internal selector error for {s:?}: {e}"))
}

/// Is this a usable `width`/`height` **content attribute**? The HTML spec wants
/// a valid non-negative integer; `50%`, `auto` and `10.5` reserve no layout
/// space, so they do not count even though a browser will not complain.
pub fn is_valid_dimension(v: &str) -> bool {
    let v = v.trim();
    !v.is_empty() && v.chars().all(|c| c.is_ascii_digit())
}

/// 1-based index of the enclosing `<picture>`, if any.
fn owning_picture(el: &ElementRef, pictures: &[ElementRef]) -> Option<usize> {
    for anc in el.ancestors() {
        if let Some(i) = pictures.iter().position(|p| p.id() == anc.id()) {
            return Some(i + 1);
        }
    }
    None
}

/// Shorten a long URL / srcset for the Markdown table, keeping both ends so the
/// filename stays visible.
fn shorten(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let head: String = s.chars().take(max.saturating_sub(12)).collect();
    let tail: String = {
        let all: Vec<char> = s.chars().collect();
        all[all.len().saturating_sub(9)..].iter().collect()
    };
    format!("{head}…{tail}")
}

/// Escape the pipe and backtick characters that would break a Markdown table
/// cell, then wrap in code ticks. Empty renders as an em dash.
fn cell_code(s: &str) -> String {
    if s.is_empty() {
        return "—".to_string();
    }
    format!("`{}`", s.replace('|', "\\|").replace('`', "'"))
}

/// Plain (non-code) Markdown table cell.
fn cell_text(s: &str) -> String {
    if s.is_empty() {
        return "—".to_string();
    }
    s.replace('|', "\\|")
}

// ---------------------------------------------------------------- analysis

/// Parse `html` and return the image inventory, honoring `opts`.
pub fn analyze(html: &str, opts: &Options) -> Result<Inventory, String> {
    if html.trim().is_empty() {
        return Err("input HTML is empty — paste the markup containing the images".into());
    }
    let doc = Html::parse_fragment(html);

    let picture_sel = sel("picture")?;
    // Document order across both element types, so a <picture>'s <source>
    // candidates appear immediately before the <img> they fall back to.
    let node_sel = sel("img, source")?;

    let pictures: Vec<ElementRef> = doc.select(&picture_sel).collect();

    let mut rows: Vec<ImageRow> = Vec::new();
    let mut summary = Summary {
        pictures: pictures.len(),
        ..Summary::default()
    };
    let mut found = 0usize;

    for el in doc.select(&node_sel) {
        let tag = el.value().name().to_ascii_lowercase();
        let picture = owning_picture(&el, &pictures);

        // <source> is also used by <video>/<audio>; only the ones inside a
        // <picture> describe an image. Those are skipped silently — a media
        // <source> is not an image and does not belong in an image inventory.
        if tag == "source" && picture.is_none() {
            continue;
        }
        if tag == "source" && !opts.include_sources {
            summary.sources += 1;
            continue;
        }

        found += 1;
        if found > MAX_IMAGES {
            return Err(format!(
                "too many images: more than {MAX_IMAGES} found — paste one page or section at a time, or turn off \"Include <picture> sources\" to halve the row count"
            ));
        }

        let is_img = tag == "img";
        let src = attr(&el, "src");
        let srcset = attr_collapsed(&el, "srcset");
        let width = attr(&el, "width");
        let height = attr(&el, "height");

        let (alt, alt_state) = if is_img {
            match el.value().attr("alt") {
                None => (String::new(), AltState::Missing),
                Some(a) if a.trim().is_empty() => (String::new(), AltState::Empty),
                Some(a) => (collapse(a), AltState::Present),
            }
        } else {
            (String::new(), AltState::Present)
        };

        let mut issues: Vec<String> = Vec::new();
        if is_img {
            summary.images += 1;
            match alt_state {
                AltState::Missing => {
                    summary.missing_alt += 1;
                    issues.push("missing-alt".into());
                }
                AltState::Empty => {
                    summary.empty_alt += 1;
                    if opts.flag_empty_alt {
                        issues.push("empty-alt".into());
                    }
                }
                AltState::Present => {}
            }
            let bad_w = !is_valid_dimension(&width);
            let bad_h = !is_valid_dimension(&height);
            if bad_w || bad_h {
                summary.missing_dimensions += 1;
            }
            if bad_w {
                issues.push("missing-width".into());
            }
            if bad_h {
                issues.push("missing-height".into());
            }
            if src.is_empty() && srcset.is_empty() {
                issues.push("no-source".into());
            }
        } else {
            summary.sources += 1;
            if srcset.is_empty() && src.is_empty() {
                issues.push("no-source".into());
            }
        }

        let loading = attr(&el, "loading").to_ascii_lowercase();
        if is_img && loading == "lazy" {
            summary.lazy += 1;
        }
        if !issues.is_empty() {
            summary.flagged += 1;
        }

        rows.push(ImageRow {
            index: 0, // assigned after filtering so the table numbers 1..n
            element: tag,
            picture,
            src,
            srcset,
            sizes: attr_collapsed(&el, "sizes"),
            media: attr(&el, "media"),
            mime_type: attr(&el, "type"),
            alt,
            alt_state,
            width,
            height,
            loading,
            decoding: attr(&el, "decoding").to_ascii_lowercase(),
            fetchpriority: attr(&el, "fetchpriority").to_ascii_lowercase(),
            class: attr_collapsed(&el, "class"),
            id: attr(&el, "id"),
            title: collapse(&attr(&el, "title")),
            issues,
        });
    }

    if rows.is_empty() && summary.sources == 0 {
        return Err(
            "no images found — looked for <img> elements and <picture><source> candidates. CSS background images and images added by JavaScript are not in the markup; copy the rendered DOM from DevTools instead of View Source to capture those."
                .into(),
        );
    }

    let total = rows.len();
    if opts.only_issues {
        rows.retain(|r| !r.issues.is_empty());
    }
    let hidden = total - rows.len();
    for (i, r) in rows.iter_mut().enumerate() {
        r.index = i + 1;
    }

    Ok(Inventory {
        summary,
        rows,
        hidden,
    })
}

/// Parse `html` and render the inventory in `fmt`.
pub fn inventory(html: &str, fmt: Format, opts: &Options) -> Result<String, String> {
    let inv = analyze(html, opts)?;
    Ok(match fmt {
        Format::Markdown => render_markdown(&inv, opts),
        Format::Csv => render_csv(&inv)?,
        Format::Json => render_json(&inv, opts),
    })
}

// ---------------------------------------------------------------- render

/// One-line human summary, shared by the Markdown and JSON renderers.
fn summary_line(s: &Summary) -> String {
    let mut parts = vec![format!(
        "{} image{}",
        s.images,
        if s.images == 1 { "" } else { "s" }
    )];
    if s.sources > 0 {
        parts.push(format!(
            "{} <picture> source{}",
            s.sources,
            if s.sources == 1 { "" } else { "s" }
        ));
    }
    parts.push(format!("{} missing alt", s.missing_alt));
    parts.push(format!("{} missing dimensions", s.missing_dimensions));
    if s.empty_alt > 0 {
        parts.push(format!("{} decorative (alt=\"\")", s.empty_alt));
    }
    if s.lazy > 0 {
        parts.push(format!("{} lazy-loaded", s.lazy));
    }
    parts.join(", ")
}

fn render_markdown(inv: &Inventory, opts: &Options) -> String {
    let mut out = String::new();
    if opts.include_summary {
        out.push_str(&format!(
            "## Image inventory\n\n{}\n\n",
            summary_line(&inv.summary)
        ));
    }

    out.push_str("| # | Element | Source | Alt | Size | Loading | Decoding | Issues |\n");
    out.push_str("|---|---------|--------|-----|------|---------|----------|--------|\n");

    for r in &inv.rows {
        let element = match (r.is_source(), r.picture) {
            (true, Some(p)) => format!("source (picture {p})"),
            (false, Some(p)) => format!("img (picture {p})"),
            (false, None) => "img".to_string(),
            (true, None) => "source".to_string(),
        };
        // A <source> declares its candidates in srcset; an <img> leads with src.
        let source = if r.src.is_empty() { &r.srcset } else { &r.src };
        // A <source> has no alt of its own — the <img> in the same <picture>
        // supplies it for every candidate, so "n/a" is the honest cell.
        let alt = if r.is_source() {
            "n/a".to_string()
        } else {
            match r.alt_state {
                AltState::Present => cell_text(&r.alt),
                AltState::Empty => "*(decorative)*".to_string(),
                AltState::Missing => "—".to_string(),
            }
        };
        let size = match (r.width.is_empty(), r.height.is_empty()) {
            (true, true) => "—".to_string(),
            _ => format!(
                "{}×{}",
                if r.width.is_empty() { "?" } else { &r.width },
                if r.height.is_empty() { "?" } else { &r.height }
            ),
        };
        let issues = if r.issues.is_empty() {
            "—".to_string()
        } else {
            r.issues.join(", ")
        };
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} |\n",
            r.index,
            element,
            cell_code(&shorten(source, 60)),
            alt,
            size,
            cell_text(&r.loading),
            cell_text(&r.decoding),
            issues
        ));
    }

    // srcset / sizes / media / type are too wide for the table but are the
    // whole point of a responsive-image audit, so they get their own section.
    let responsive: Vec<&ImageRow> = inv
        .rows
        .iter()
        .filter(|r| !r.srcset.is_empty() || !r.sizes.is_empty() || !r.media.is_empty())
        .collect();
    if !responsive.is_empty() {
        out.push_str("\n### Responsive sources\n\n");
        for r in responsive {
            out.push_str(&format!("- **#{}**", r.index));
            if !r.mime_type.is_empty() {
                out.push_str(&format!(" type=`{}`", r.mime_type));
            }
            if !r.media.is_empty() {
                out.push_str(&format!(" media=`{}`", r.media));
            }
            if !r.srcset.is_empty() {
                out.push_str(&format!(" srcset=`{}`", r.srcset.replace('|', "\\|")));
            }
            if !r.sizes.is_empty() {
                out.push_str(&format!(" sizes=`{}`", r.sizes.replace('|', "\\|")));
            }
            out.push('\n');
        }
    }

    if inv.rows.is_empty() && inv.hidden > 0 {
        out.push_str("\nNo issues found — every image has alt text and explicit dimensions.\n");
        out.push_str(&format!(
            "{} clean row(s) hidden by \"Only flagged images\".\n",
            inv.hidden
        ));
    } else if inv.hidden > 0 {
        out.push_str(&format!(
            "\n{} clean row(s) hidden by \"Only flagged images\".\n",
            inv.hidden
        ));
    } else if inv.rows.is_empty() {
        out.push_str("\nNo issues found — every image has alt text and explicit dimensions.\n");
    }
    out
}

fn render_csv(inv: &Inventory) -> Result<String, String> {
    let mut w = csv::WriterBuilder::new().from_writer(Vec::new());
    w.write_record([
        "index",
        "element",
        "picture",
        "src",
        "srcset",
        "sizes",
        "media",
        "type",
        "alt",
        "alt_state",
        "width",
        "height",
        "loading",
        "decoding",
        "fetchpriority",
        "class",
        "id",
        "title",
        "issues",
    ])
    .map_err(|e| format!("csv error: {e}"))?;

    for r in &inv.rows {
        w.write_record([
            r.index.to_string().as_str(),
            r.element.as_str(),
            &r.picture.map(|p| p.to_string()).unwrap_or_default(),
            r.src.as_str(),
            r.srcset.as_str(),
            r.sizes.as_str(),
            r.media.as_str(),
            r.mime_type.as_str(),
            r.alt.as_str(),
            if r.is_source() {
                "n/a"
            } else {
                r.alt_state.as_str()
            },
            r.width.as_str(),
            r.height.as_str(),
            r.loading.as_str(),
            r.decoding.as_str(),
            r.fetchpriority.as_str(),
            r.class.as_str(),
            r.id.as_str(),
            r.title.as_str(),
            r.issues.join("; ").as_str(),
        ])
        .map_err(|e| format!("csv error: {e}"))?;
    }

    let bytes = w.into_inner().map_err(|e| format!("csv error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("csv utf-8 error: {e}"))
}

fn row_json(r: &ImageRow) -> Value {
    let mut o = Map::new();
    o.insert("index".into(), json!(r.index));
    o.insert("element".into(), json!(r.element));
    if let Some(p) = r.picture {
        o.insert("picture".into(), json!(p));
    }
    o.insert("src".into(), json!(r.src));
    o.insert("srcset".into(), json!(r.srcset));
    o.insert("sizes".into(), json!(r.sizes));
    if r.is_source() {
        o.insert("media".into(), json!(r.media));
        o.insert("type".into(), json!(r.mime_type));
    } else {
        o.insert("alt".into(), json!(r.alt));
        o.insert("alt_state".into(), json!(r.alt_state.as_str()));
    }
    o.insert("width".into(), json!(r.width));
    o.insert("height".into(), json!(r.height));
    o.insert("loading".into(), json!(r.loading));
    o.insert("decoding".into(), json!(r.decoding));
    o.insert("fetchpriority".into(), json!(r.fetchpriority));
    o.insert("class".into(), json!(r.class));
    o.insert("id".into(), json!(r.id));
    o.insert("title".into(), json!(r.title));
    o.insert("issues".into(), json!(r.issues));
    Value::Object(o)
}

fn render_json(inv: &Inventory, opts: &Options) -> String {
    let rows: Vec<Value> = inv.rows.iter().map(row_json).collect();
    let mut top = Map::new();
    if opts.include_summary {
        let s = &inv.summary;
        top.insert(
            "summary".into(),
            json!({
                "images": s.images,
                "picture_sources": s.sources,
                "pictures": s.pictures,
                "missing_alt": s.missing_alt,
                "empty_alt": s.empty_alt,
                "missing_dimensions": s.missing_dimensions,
                "lazy": s.lazy,
                "flagged": s.flagged,
                "text": summary_line(s),
            }),
        );
    }
    top.insert("row_count".into(), json!(inv.rows.len()));
    if inv.hidden > 0 {
        top.insert("hidden_clean_rows".into(), json!(inv.hidden));
    }
    top.insert("images".into(), Value::Array(rows));
    serde_json::to_string_pretty(&Value::Object(top)).unwrap_or_else(|e| format!("json error: {e}"))
}

// ---------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = r#"
<article>
  <img src="/hero.jpg" alt="Team on a rooftop" width="1200" height="800" decoding="async" fetchpriority="high" class="hero">
  <img src="/promo.png" width="600" height="400" loading="lazy">
  <img src="/divider.svg" alt="" width="8" height="8">
  <img src="/chart.png" alt="Revenue by quarter" loading="lazy" decoding="async" id="chart">
</article>"#;

    fn md(html: &str) -> String {
        inventory(html, Format::Markdown, &Options::default()).unwrap()
    }

    #[test]
    fn happy_path_markdown_table() {
        let out = md(PAGE);
        assert!(
            out.contains("4 images, 1 missing alt, 1 missing dimensions, 1 decorative (alt=\"\"), 2 lazy-loaded"),
            "got: {out}"
        );
        assert!(
            out.contains(
                "| 1 | img | `/hero.jpg` | Team on a rooftop | 1200×800 | — | async | — |"
            ),
            "got: {out}"
        );
        assert!(
            out.contains("| 2 | img | `/promo.png` | — | 600×400 | lazy | — | missing-alt |"),
            "got: {out}"
        );
        assert!(
            out.contains("| 3 | img | `/divider.svg` | *(decorative)* | 8×8 | — | — | — |"),
            "got: {out}"
        );
        assert!(
            out.contains(
                "| 4 | img | `/chart.png` | Revenue by quarter | — | lazy | async | missing-width, missing-height |"
            ),
            "got: {out}"
        );
    }

    #[test]
    fn empty_alt_is_not_a_defect_by_default() {
        let inv = analyze(
            r#"<img src="a.png" alt="" width="1" height="1">"#,
            &Options::default(),
        )
        .unwrap();
        assert_eq!(inv.summary.missing_alt, 0);
        assert_eq!(inv.summary.empty_alt, 1);
        assert!(inv.rows[0].issues.is_empty());
        assert_eq!(inv.rows[0].alt_state, AltState::Empty);
    }

    #[test]
    fn flag_empty_alt_opts_into_auditing_decorative_images() {
        let opts = Options {
            flag_empty_alt: true,
            ..Options::default()
        };
        let inv = analyze(r#"<img src="a.png" alt="" width="1" height="1">"#, &opts).unwrap();
        assert_eq!(inv.rows[0].issues, vec!["empty-alt"]);
    }

    #[test]
    fn picture_sources_are_inventoried_and_tied_to_their_picture() {
        let html = r#"
<picture>
  <source srcset="/hero.avif 1x, /hero@2x.avif 2x" type="image/avif" media="(min-width: 800px)">
  <source srcset="/hero.webp" type="image/webp">
  <img src="/hero.jpg" alt="A hero" width="800" height="600" sizes="100vw">
</picture>"#;
        let inv = analyze(html, &Options::default()).unwrap();
        assert_eq!(inv.summary.pictures, 1);
        assert_eq!(inv.summary.sources, 2);
        assert_eq!(inv.summary.images, 1);
        // Document order: both <source> candidates precede the <img> fallback.
        assert_eq!(inv.rows[0].element, "source");
        assert_eq!(inv.rows[0].picture, Some(1));
        assert_eq!(inv.rows[0].mime_type, "image/avif");
        assert_eq!(inv.rows[0].media, "(min-width: 800px)");
        assert_eq!(inv.rows[0].srcset, "/hero.avif 1x, /hero@2x.avif 2x");
        assert_eq!(inv.rows[2].element, "img");
        assert_eq!(inv.rows[2].picture, Some(1));
        assert_eq!(inv.rows[2].sizes, "100vw");

        let out = inventory(html, Format::Markdown, &Options::default()).unwrap();
        assert!(out.contains("| 1 | source (picture 1) |"), "got: {out}");
        assert!(out.contains("| 3 | img (picture 1) |"), "got: {out}");
        assert!(
            out.contains("- **#1** type=`image/avif` media=`(min-width: 800px)` srcset=`/hero.avif 1x, /hero@2x.avif 2x`"),
            "got: {out}"
        );
    }

    #[test]
    fn include_sources_off_drops_source_rows_but_keeps_the_count() {
        let html = r#"<picture><source srcset="/a.webp"><img src="/a.jpg" alt="A" width="2" height="2"></picture>"#;
        let opts = Options {
            include_sources: false,
            ..Options::default()
        };
        let inv = analyze(html, &opts).unwrap();
        assert_eq!(inv.rows.len(), 1);
        assert_eq!(inv.rows[0].element, "img");
        assert_eq!(inv.summary.sources, 1, "the source is still counted");
    }

    #[test]
    fn media_sources_outside_a_picture_are_ignored() {
        let html = r#"<video><source src="/clip.mp4" type="video/mp4"></video><img src="/a.jpg" alt="A" width="2" height="2">"#;
        let inv = analyze(html, &Options::default()).unwrap();
        assert_eq!(inv.rows.len(), 1);
        assert_eq!(inv.summary.sources, 0);
    }

    #[test]
    fn percentage_width_is_reported_verbatim_and_still_flagged() {
        let inv = analyze(
            r#"<img src="a.png" alt="A" width="50%" height="auto">"#,
            &Options::default(),
        )
        .unwrap();
        assert_eq!(inv.rows[0].width, "50%");
        assert_eq!(inv.rows[0].height, "auto");
        assert_eq!(inv.rows[0].issues, vec!["missing-width", "missing-height"]);
        assert_eq!(inv.summary.missing_dimensions, 1);
    }

    #[test]
    fn only_issues_filters_and_renumbers() {
        let opts = Options {
            only_issues: true,
            ..Options::default()
        };
        let out = inventory(PAGE, Format::Markdown, &opts).unwrap();
        assert!(
            out.contains("| 1 | img | `/promo.png` | — | 600×400 | lazy | — | missing-alt |"),
            "got: {out}"
        );
        assert!(out.contains("| 2 | img | `/chart.png` |"), "got: {out}");
        assert!(!out.contains("/hero.jpg"), "clean rows dropped: {out}");
        assert!(out.contains("2 clean row(s) hidden"), "got: {out}");
    }

    #[test]
    fn only_issues_on_a_clean_page_reports_no_issues_rather_than_erroring() {
        let opts = Options {
            only_issues: true,
            ..Options::default()
        };
        let out = inventory(
            r#"<img src="a.png" alt="A" width="1" height="1">"#,
            Format::Markdown,
            &opts,
        )
        .unwrap();
        assert!(out.contains("No issues found"), "got: {out}");
    }

    #[test]
    fn csv_is_flat_and_quoted() {
        let out = inventory(
            r#"<img src="/a.png" alt="Cats, dogs" width="10" height="20" loading="lazy" title="Pets">"#,
            Format::Csv,
            &Options::default(),
        )
        .unwrap();
        assert!(
            out.starts_with("index,element,picture,src,srcset,sizes,media,type,alt,alt_state,width,height,loading,decoding,fetchpriority,class,id,title,issues\n"),
            "got: {out}"
        );
        assert!(
            out.contains("1,img,,/a.png,,,,,\"Cats, dogs\",present,10,20,lazy,,,,,Pets,\n"),
            "got: {out}"
        );
    }

    #[test]
    fn json_carries_the_summary_and_every_attribute() {
        let out = inventory(
            r#"<img src="/a.png" width="10" class="thumb rounded">"#,
            Format::Json,
            &Options::default(),
        )
        .unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["summary"]["images"], json!(1));
        assert_eq!(v["summary"]["missing_alt"], json!(1));
        assert_eq!(v["summary"]["missing_dimensions"], json!(1));
        assert_eq!(v["summary"]["flagged"], json!(1));
        assert_eq!(v["row_count"], json!(1));
        let row = &v["images"][0];
        assert_eq!(row["alt_state"], json!("missing"));
        assert_eq!(row["class"], json!("thumb rounded"));
        assert_eq!(row["width"], json!("10"));
        assert_eq!(row["height"], json!(""));
        assert_eq!(row["issues"], json!(["missing-alt", "missing-height"]));
    }

    #[test]
    fn include_summary_off_drops_the_header() {
        let opts = Options {
            include_summary: false,
            ..Options::default()
        };
        let out = inventory(PAGE, Format::Markdown, &opts).unwrap();
        assert!(!out.contains("## Image inventory"), "got: {out}");
        assert!(out.starts_with("| # | Element |"), "got: {out}");

        let j: Value =
            serde_json::from_str(&inventory(PAGE, Format::Json, &opts).unwrap()).unwrap();
        assert!(j.get("summary").is_none());
    }

    #[test]
    fn messy_markup_still_parses() {
        // Unquoted attributes, unclosed tags, mixed case — the reason a real
        // HTML parser is used instead of a regex.
        let html = "<DIV><IMG SRC=/a.png ALT=Hello WIDTH=10 HEIGHT=20><p>text<img src='/b.png'>";
        let inv = analyze(html, &Options::default()).unwrap();
        assert_eq!(inv.rows.len(), 2);
        assert_eq!(inv.rows[0].src, "/a.png");
        assert_eq!(inv.rows[0].alt, "Hello");
        assert_eq!(inv.rows[1].src, "/b.png");
        assert_eq!(inv.rows[1].alt_state, AltState::Missing);
    }

    #[test]
    fn alt_whitespace_is_collapsed_and_blank_alt_counts_as_decorative() {
        let inv = analyze(
            "<img src=a.png alt=\"  A  long\n  caption \" width=1 height=1><img src=b.png alt=\"   \" width=1 height=1>",
            &Options::default(),
        )
        .unwrap();
        assert_eq!(inv.rows[0].alt, "A long caption");
        assert_eq!(inv.rows[1].alt_state, AltState::Empty);
    }

    #[test]
    fn img_with_only_srcset_is_not_flagged_sourceless() {
        let inv = analyze(
            r#"<img srcset="/a-480.png 480w, /a-960.png 960w" sizes="50vw" alt="A" width="1" height="1">"#,
            &Options::default(),
        )
        .unwrap();
        assert!(
            inv.rows[0].issues.is_empty(),
            "got: {:?}",
            inv.rows[0].issues
        );
        assert_eq!(inv.rows[0].sizes, "50vw");
    }

    #[test]
    fn img_with_no_src_and_no_srcset_is_flagged() {
        let inv = analyze(r#"<img alt="A" width="1" height="1">"#, &Options::default()).unwrap();
        assert_eq!(inv.rows[0].issues, vec!["no-source"]);
    }

    #[test]
    fn long_sources_are_shortened_in_markdown_only() {
        let long = format!("/assets/{}/photo-final.jpg", "x".repeat(90));
        let html = format!(r#"<img src="{long}" alt="A" width="1" height="1">"#);
        let out = inventory(&html, Format::Markdown, &Options::default()).unwrap();
        assert!(out.contains('…'), "got: {out}");
        assert!(out.contains("final.jpg"), "tail kept: {out}");
        let csv = inventory(&html, Format::Csv, &Options::default()).unwrap();
        assert!(csv.contains(&long), "csv keeps the full URL: {csv}");
    }

    #[test]
    fn pipes_in_alt_text_do_not_break_the_markdown_table() {
        let out = inventory(
            r#"<img src="/a.png" alt="Before | After" width="1" height="1">"#,
            Format::Markdown,
            &Options::default(),
        )
        .unwrap();
        assert!(out.contains("Before \\| After"), "got: {out}");
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = analyze("   \n ", &Options::default()).unwrap_err();
        assert!(err.contains("input HTML is empty"), "got: {err}");
    }

    #[test]
    fn html_with_no_images_is_an_error() {
        let err = analyze("<p>Just some text</p>", &Options::default()).unwrap_err();
        assert!(err.contains("no images found"), "got: {err}");
        assert!(err.contains("DevTools"), "points at the workaround: {err}");
    }

    #[test]
    fn bad_format_is_rejected_with_the_valid_set() {
        let err = parse_format("yaml").unwrap_err();
        assert!(err.contains("markdown, csv, or json"), "got: {err}");
    }

    #[test]
    fn format_aliases_and_default() {
        assert_eq!(parse_format("").unwrap(), Format::Markdown);
        assert_eq!(parse_format("MD").unwrap(), Format::Markdown);
        assert_eq!(parse_format(" CSV ").unwrap(), Format::Csv);
        assert_eq!(parse_format("Json").unwrap(), Format::Json);
    }

    #[test]
    fn too_many_images_is_a_loud_error_not_a_truncation() {
        let html = r#"<img src="/a.png" alt="A" width="1" height="1">"#.repeat(MAX_IMAGES + 1);
        let err = analyze(&html, &Options::default()).unwrap_err();
        assert!(err.contains("too many images"), "got: {err}");
        assert!(
            err.contains(&MAX_IMAGES.to_string()),
            "names the cap: {err}"
        );
    }

    #[test]
    fn dimension_validation_matches_the_html_content_attribute_rule() {
        assert!(is_valid_dimension("0"));
        assert!(is_valid_dimension("1200"));
        assert!(!is_valid_dimension(""));
        assert!(!is_valid_dimension("50%"));
        assert!(!is_valid_dimension("auto"));
        assert!(!is_valid_dimension("10.5"));
        assert!(!is_valid_dimension("-4"));
    }
}

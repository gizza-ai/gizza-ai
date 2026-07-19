//! markdown-to-pptx core — turn a Markdown outline into a real, binary
//! PowerPoint `.pptx` presentation (an Office Open XML ZIP of XML parts). Pure
//! logic shared by the chat skill block, the CLI and the web page — no wafer /
//! wasm-bindgen deps.
//!
//! Headings and thematic breaks (`---`) split the outline into slides; list
//! items and paragraphs become bullet lines. The output is a genuine `.pptx`
//! package — a `[Content_Types].xml`, package + presentation relationships, a
//! slide master / layout / theme, and one `ppt/slides/slideN.xml` per slide —
//! not a renamed text file, so PowerPoint, Keynote, Google Slides and
//! LibreOffice Impress all open it natively.

use std::io::{Cursor, Write};
use zip::write::{SimpleFileOptions, ZipWriter};
use zip::CompressionMethod;

/// Where the outline is cut into separate slides.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SplitLevel {
    /// Start a new slide at each top-level `#` heading. `##`/deeper headings
    /// become bullet lines on the current slide.
    H1,
    /// Start a new slide at each `##` heading. `#` headings become bullets.
    H2,
    /// Start a new slide at every `#` and `##` heading.
    Both,
}

impl SplitLevel {
    /// Parse a split-level name (canonical + common aliases).
    pub fn parse(s: &str) -> Result<SplitLevel, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "h1" | "1" => Ok(SplitLevel::H1),
            "h2" | "2" => Ok(SplitLevel::H2),
            "both" | "h1h2" | "h1-h2" => Ok(SplitLevel::Both),
            other => Err(format!("unknown split_level '{other}' (use h1, h2, or both)")),
        }
    }

    /// Does a heading of `depth` (1 = `#`, 2 = `##`, …) start a new slide?
    fn splits_at(self, depth: u8) -> bool {
        match self {
            SplitLevel::H1 => depth == 1,
            SplitLevel::H2 => depth == 2,
            SplitLevel::Both => depth == 1 || depth == 2,
        }
    }
}

/// Colour theme for slide backgrounds and text.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Theme {
    /// Dark text on a white background.
    Light,
    /// Light text on a near-black background.
    Dark,
}

impl Theme {
    /// Parse a theme name (canonical + common aliases).
    pub fn parse(s: &str) -> Result<Theme, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "light" | "white" => Ok(Theme::Light),
            "dark" | "black" | "night" => Ok(Theme::Dark),
            other => Err(format!("unknown theme '{other}' (use light or dark)")),
        }
    }

    /// `(background, title text, body text)` as 6-hex RGB strings.
    fn colors(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Theme::Light => ("FFFFFF", "1A1A1A", "333333"),
            Theme::Dark => ("1A1A1A", "FFFFFF", "E6E6E6"),
        }
    }
}

/// Slide aspect ratio (drives the presentation's `<p:sldSz>` in EMUs).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AspectRatio {
    /// 16:9 widescreen (12192000 × 6858000 EMU).
    Widescreen,
    /// 4:3 standard (9144000 × 6858000 EMU).
    Standard,
}

impl AspectRatio {
    /// Parse an aspect-ratio name (canonical + common aliases).
    pub fn parse(s: &str) -> Result<AspectRatio, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "16:9" | "169" | "16-9" | "widescreen" | "wide" => Ok(AspectRatio::Widescreen),
            "4:3" | "43" | "4-3" | "standard" | "std" => Ok(AspectRatio::Standard),
            other => Err(format!("unknown aspect_ratio '{other}' (use 16:9 or 4:3)")),
        }
    }

    /// `(width, height, sldSz type)` in EMUs.
    fn dims(self) -> (i64, i64, &'static str) {
        match self {
            AspectRatio::Widescreen => (12_192_000, 6_858_000, "screen16x9"),
            AspectRatio::Standard => (9_144_000, 6_858_000, "screen4x3"),
        }
    }
}

/// One bullet on a slide, with an indentation level (0 = top level, capped at 4).
struct Bullet {
    text: String,
    level: u8,
}

/// One slide: a title plus its bullets. `is_title` marks the optional leading
/// deck-title slide (rendered as one big centred line, no body).
struct Slide {
    title: String,
    bullets: Vec<Bullet>,
    is_title: bool,
}

impl Slide {
    fn is_empty(&self) -> bool {
        self.title.trim().is_empty() && self.bullets.is_empty()
    }
}

/// A thematic break: a line of 3+ of `-`, `*`, or `_` (optionally spaced).
fn is_thematic_break(line: &str) -> bool {
    let t = line.trim();
    for m in ['-', '*', '_'] {
        let stripped: String = t.chars().filter(|c| !c.is_whitespace()).collect();
        if stripped.len() >= 3 && stripped.chars().all(|c| c == m) {
            return true;
        }
    }
    false
}

/// If `line` is an ATX heading (`#`…`######`), return `(depth, text)`.
fn heading(line: &str) -> Option<(u8, String)> {
    let t = line.trim_start();
    if !t.starts_with('#') {
        return None;
    }
    let hashes = t.chars().take_while(|&c| c == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &t[hashes..];
    // A real heading needs a space (or nothing) after the run of `#`.
    if !rest.is_empty() && !rest.starts_with(' ') && !rest.starts_with('\t') {
        return None;
    }
    Some((hashes as u8, clean_inline(rest.trim())))
}

/// If `line` is a list item (`-`, `*`, `+`, or `1.`), return `(level, text)`;
/// the level comes from the leading indentation (every 2 spaces / tab = a level).
fn list_item(line: &str) -> Option<(u8, String)> {
    let indent = line.chars().take_while(|c| *c == ' ' || *c == '\t').count();
    let t = &line[indent..];
    let body = if let Some(rest) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")).or_else(|| t.strip_prefix("+ ")) {
        rest
    } else if let Some(rest) = ordered_marker(t) {
        rest
    } else {
        return None;
    };
    let level = (indent / 2).min(4) as u8;
    Some((level, clean_inline(body.trim())))
}

/// Strip a leading ordered-list marker like `1.` / `12)` and return the rest.
fn ordered_marker(t: &str) -> Option<&str> {
    let digits = t.chars().take_while(char::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    let after = &t[digits..];
    let rest = after.strip_prefix(". ").or_else(|| after.strip_prefix(") "))?;
    Some(rest)
}

/// Lightly de-markdown inline text for a slide run: drop emphasis / code marks
/// and unwrap `[label](url)` links to their label. Not a full parser — just
/// enough that bullets read as plain prose, not raw Markdown.
fn clean_inline(s: &str) -> String {
    let no_links = unwrap_links(s);
    no_links
        .replace("**", "")
        .replace("__", "")
        .replace('`', "")
        .trim()
        .to_string()
}

/// Replace every `[label](target)` with just `label`.
fn unwrap_links(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < s.len() {
        if bytes[i] == b'[' {
            if let Some(close) = s[i + 1..].find(']') {
                let label_end = i + 1 + close;
                if s.as_bytes().get(label_end + 1) == Some(&b'(') {
                    if let Some(paren) = s[label_end + 2..].find(')') {
                        out.push_str(&s[i + 1..label_end]);
                        i = label_end + 2 + paren + 1;
                        continue;
                    }
                }
            }
        }
        // Push one full char (handle multi-byte UTF-8).
        let ch_len = s[i..].chars().next().map(char::len_utf8).unwrap_or(1);
        out.push_str(&s[i..i + ch_len]);
        i += ch_len;
    }
    out
}

/// Parse the Markdown outline into slides per `split`. Thematic breaks always
/// start a fresh slide; headings split per [`SplitLevel`]; list items and
/// paragraphs become bullets on the current slide.
fn parse_slides(markdown: &str, split: SplitLevel) -> Vec<Slide> {
    let mut slides: Vec<Slide> = Vec::new();
    let mut cur = Slide { title: String::new(), bullets: Vec::new(), is_title: false };

    let flush = |cur: &mut Slide, slides: &mut Vec<Slide>| {
        if !cur.is_empty() {
            slides.push(std::mem::replace(cur, Slide { title: String::new(), bullets: Vec::new(), is_title: false }));
        }
    };

    let mut in_code = false;
    for raw in markdown.lines() {
        // Fenced code blocks: keep their content verbatim as bullets, don't
        // treat `#`/`---` inside them as structure.
        if raw.trim_start().starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if in_code {
            if !raw.trim().is_empty() {
                cur.bullets.push(Bullet { text: raw.trim_end().to_string(), level: 1 });
            }
            continue;
        }

        if is_thematic_break(raw) {
            flush(&mut cur, &mut slides);
            continue;
        }
        if let Some((depth, text)) = heading(raw) {
            if split.splits_at(depth) {
                flush(&mut cur, &mut slides);
                cur.title = text;
            } else if split == SplitLevel::H2 && depth == 1 && cur.title.is_empty() && cur.bullets.is_empty() {
                // In H2-split mode an opening H1 is the deck/section heading,
                // not a standalone slide. Keep the first actual `##` as slide 1.
                continue;
            } else if cur.title.is_empty() && cur.bullets.is_empty() {
                // A non-splitting heading with nothing before it titles the slide.
                cur.title = text;
            } else {
                cur.bullets.push(Bullet { text, level: 0 });
            }
            continue;
        }
        if let Some((level, text)) = list_item(raw) {
            if !text.is_empty() {
                cur.bullets.push(Bullet { text, level });
            }
            continue;
        }
        let text = clean_inline(raw.trim());
        if !text.is_empty() {
            cur.bullets.push(Bullet { text, level: 0 });
        }
    }
    flush(&mut cur, &mut slides);
    slides
}

/// Escape a string for XML text / attribute content.
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            // Drop control chars XML 1.0 forbids (keep tab/newline/CR).
            c if (c as u32) < 0x20 && c != '\t' && c != '\n' && c != '\r' => {}
            c => out.push(c),
        }
    }
    out
}

/// Convert a Markdown outline into a real binary `.pptx` package.
///
/// - `title` — optional deck title. When non-empty it becomes a leading title
///   slide and the document's `dc:title`.
/// - `split` — where headings cut the outline into slides.
/// - `theme` / `aspect` — colours and slide dimensions.
///
/// Returns an error when the outline (and title) are empty, i.e. there is
/// nothing to put on a slide.
pub fn to_pptx(
    markdown: &str,
    title: &str,
    split: SplitLevel,
    theme: Theme,
    aspect: AspectRatio,
) -> Result<Vec<u8>, String> {
    to_pptx_with_count(markdown, title, split, theme, aspect).map(|(b, _)| b)
}

/// Like [`to_pptx`] but also returns the slide count so callers can summarize
/// the deck without unzipping it.
pub fn to_pptx_with_count(
    markdown: &str,
    title: &str,
    split: SplitLevel,
    theme: Theme,
    aspect: AspectRatio,
) -> Result<(Vec<u8>, usize), String> {
    let mut slides = parse_slides(markdown, split);
    let title = title.trim();
    if !title.is_empty() {
        slides.insert(0, Slide { title: clean_inline(title), bullets: Vec::new(), is_title: true });
    }
    if slides.is_empty() {
        return Err("input is empty — add a heading, a list, or a title to make at least one slide".to_string());
    }

    let (w, h, sz_type) = aspect.dims();
    let colors = theme.colors();

    let mut parts: Vec<(String, String)> = Vec::new();
    parts.push(("[Content_Types].xml".to_string(), content_types(slides.len())));
    parts.push(("_rels/.rels".to_string(), root_rels()));
    parts.push(("docProps/core.xml".to_string(), core_props(title)));
    parts.push(("docProps/app.xml".to_string(), app_props(slides.len())));
    parts.push(("ppt/presentation.xml".to_string(), presentation_xml(slides.len(), w, h, sz_type)));
    parts.push(("ppt/_rels/presentation.xml.rels".to_string(), presentation_rels(slides.len())));
    parts.push(("ppt/theme/theme1.xml".to_string(), theme_xml()));
    parts.push(("ppt/slideMasters/slideMaster1.xml".to_string(), slide_master_xml()));
    parts.push(("ppt/slideMasters/_rels/slideMaster1.xml.rels".to_string(), slide_master_rels()));
    parts.push(("ppt/slideLayouts/slideLayout1.xml".to_string(), slide_layout_xml()));
    parts.push(("ppt/slideLayouts/_rels/slideLayout1.xml.rels".to_string(), slide_layout_rels()));
    for (i, slide) in slides.iter().enumerate() {
        let n = i + 1;
        parts.push((format!("ppt/slides/slide{n}.xml"), slide_xml(slide, w, h, colors)));
        parts.push((format!("ppt/slides/_rels/slide{n}.xml.rels"), slide_rels()));
    }

    let bytes = build_zip(&parts)?;
    Ok((bytes, slides.len()))
}

/// Assemble the parts into a single ZIP (OOXML package). Deflate-compressed with
/// the crate default 1980 DOS timestamp, so it is deterministic and clock-free.
fn build_zip(parts: &[(String, String)]) -> Result<Vec<u8>, String> {
    let mut zw = ZipWriter::new(Cursor::new(Vec::new()));
    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (name, content) in parts {
        zw.start_file(name.as_str(), opts).map_err(|e| format!("zip error on {name}: {e}"))?;
        zw.write_all(content.as_bytes()).map_err(|e| format!("zip write error on {name}: {e}"))?;
    }
    let cursor = zw.finish().map_err(|e| format!("zip finalize error: {e}"))?;
    Ok(cursor.into_inner())
}

// ---------------------------------------------------------------------------
// OOXML part builders. Each returns a complete, standalone XML document.
// ---------------------------------------------------------------------------

const DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n";
const NS_A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const NS_P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";

fn content_types(slide_count: usize) -> String {
    let mut overrides = String::new();
    for n in 1..=slide_count {
        overrides.push_str(&format!(
            "<Override PartName=\"/ppt/slides/slide{n}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slide+xml\"/>"
        ));
    }
    format!(
        "{DECL}<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
<Default Extension=\"xml\" ContentType=\"application/xml\"/>\
<Override PartName=\"/ppt/presentation.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml\"/>\
<Override PartName=\"/ppt/slideMasters/slideMaster1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml\"/>\
<Override PartName=\"/ppt/slideLayouts/slideLayout1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml\"/>\
<Override PartName=\"/ppt/theme/theme1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.theme+xml\"/>\
{overrides}\
<Override PartName=\"/docProps/core.xml\" ContentType=\"application/vnd.openxmlformats-package.core-properties+xml\"/>\
<Override PartName=\"/docProps/app.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.extended-properties+xml\"/>\
</Types>"
    )
}

fn root_rels() -> String {
    format!(
        "{DECL}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"ppt/presentation.xml\"/>\
<Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties\" Target=\"docProps/core.xml\"/>\
<Relationship Id=\"rId3\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties\" Target=\"docProps/app.xml\"/>\
</Relationships>"
    )
}

fn core_props(title: &str) -> String {
    format!(
        "{DECL}<cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:dcterms=\"http://purl.org/dc/terms/\" xmlns:dcmitype=\"http://purl.org/dc/dcmitype/\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\
<dc:title>{}</dc:title>\
<dc:creator>gizza-ai/markdown-to-pptx</dc:creator>\
</cp:coreProperties>",
        esc(title)
    )
}

fn app_props(slide_count: usize) -> String {
    format!(
        "{DECL}<Properties xmlns=\"http://schemas.openxmlformats.org/officeDocument/2006/extended-properties\">\
<Application>gizza-ai/markdown-to-pptx</Application>\
<Slides>{slide_count}</Slides>\
</Properties>"
    )
}

fn presentation_xml(slide_count: usize, w: i64, h: i64, sz_type: &str) -> String {
    let mut ids = String::new();
    for i in 0..slide_count {
        // Slide relationship ids start at rId2 (rId1 = the slide master).
        let rid = i + 2;
        let sld_id = 256 + i;
        ids.push_str(&format!("<p:sldId id=\"{sld_id}\" r:id=\"rId{rid}\"/>"));
    }
    format!(
        "{DECL}<p:presentation xmlns:a=\"{NS_A}\" xmlns:r=\"{NS_R}\" xmlns:p=\"{NS_P}\" saveSubsetFonts=\"1\">\
<p:sldMasterIdLst><p:sldMasterId id=\"2147483648\" r:id=\"rId1\"/></p:sldMasterIdLst>\
<p:sldIdLst>{ids}</p:sldIdLst>\
<p:sldSz cx=\"{w}\" cy=\"{h}\" type=\"{sz_type}\"/>\
<p:notesSz cx=\"6858000\" cy=\"9144000\"/>\
</p:presentation>"
    )
}

fn presentation_rels(slide_count: usize) -> String {
    let mut rels = String::from(
        "<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster\" Target=\"slideMasters/slideMaster1.xml\"/>",
    );
    for i in 0..slide_count {
        let rid = i + 2;
        let n = i + 1;
        rels.push_str(&format!(
            "<Relationship Id=\"rId{rid}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide\" Target=\"slides/slide{n}.xml\"/>"
        ));
    }
    // Theme relationship follows the slides.
    let theme_rid = slide_count + 2;
    rels.push_str(&format!(
        "<Relationship Id=\"rId{theme_rid}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme\" Target=\"theme/theme1.xml\"/>"
    ));
    format!(
        "{DECL}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">{rels}</Relationships>"
    )
}

fn slide_rels() -> String {
    format!(
        "{DECL}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout\" Target=\"../slideLayouts/slideLayout1.xml\"/>\
</Relationships>"
    )
}

fn slide_layout_rels() -> String {
    format!(
        "{DECL}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster\" Target=\"../slideMasters/slideMaster1.xml\"/>\
</Relationships>"
    )
}

fn slide_master_rels() -> String {
    format!(
        "{DECL}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout\" Target=\"../slideLayouts/slideLayout1.xml\"/>\
<Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme\" Target=\"../theme/theme1.xml\"/>\
</Relationships>"
    )
}

/// An empty group-shape tree — the minimum valid `<p:spTree>` body used by the
/// master and layout (slides add their own shapes).
fn empty_sp_tree() -> String {
    "<p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>\
<p:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/><a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"0\" cy=\"0\"/></a:xfrm></p:grpSpPr>"
        .to_string()
}

fn slide_master_xml() -> String {
    format!(
        "{DECL}<p:sldMaster xmlns:a=\"{NS_A}\" xmlns:r=\"{NS_R}\" xmlns:p=\"{NS_P}\">\
<p:cSld><p:spTree>{tree}</p:spTree></p:cSld>\
<p:clrMap bg1=\"lt1\" tx1=\"dk1\" bg2=\"lt2\" tx2=\"dk2\" accent1=\"accent1\" accent2=\"accent2\" accent3=\"accent3\" accent4=\"accent4\" accent5=\"accent5\" accent6=\"accent6\" hlink=\"hlink\" folHlink=\"folHlink\"/>\
<p:sldLayoutIdLst><p:sldLayoutId id=\"2147483649\" r:id=\"rId1\"/></p:sldLayoutIdLst>\
<p:txStyles>\
<p:titleStyle><a:lvl1pPr><a:defRPr sz=\"4000\"/></a:lvl1pPr></p:titleStyle>\
<p:bodyStyle><a:lvl1pPr><a:defRPr sz=\"1800\"/></a:lvl1pPr></p:bodyStyle>\
<p:otherStyle><a:lvl1pPr><a:defRPr sz=\"1800\"/></a:lvl1pPr></p:otherStyle>\
</p:txStyles>\
</p:sldMaster>",
        tree = empty_sp_tree()
    )
}

fn slide_layout_xml() -> String {
    format!(
        "{DECL}<p:sldLayout xmlns:a=\"{NS_A}\" xmlns:r=\"{NS_R}\" xmlns:p=\"{NS_P}\" type=\"blank\" preserve=\"1\">\
<p:cSld name=\"Blank\"><p:spTree>{tree}</p:spTree></p:cSld>\
<p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>\
</p:sldLayout>",
        tree = empty_sp_tree()
    )
}

/// A complete DrawingML theme — colour, font and format schemes. Required for a
/// valid package and referenced by the slide master.
fn theme_xml() -> String {
    format!(
        "{DECL}<a:theme xmlns:a=\"{NS_A}\" name=\"gizza\">\
<a:themeElements>\
<a:clrScheme name=\"gizza\">\
<a:dk1><a:sysClr val=\"windowText\" lastClr=\"000000\"/></a:dk1>\
<a:lt1><a:sysClr val=\"window\" lastClr=\"FFFFFF\"/></a:lt1>\
<a:dk2><a:srgbClr val=\"44546A\"/></a:dk2>\
<a:lt2><a:srgbClr val=\"E7E6E6\"/></a:lt2>\
<a:accent1><a:srgbClr val=\"4472C4\"/></a:accent1>\
<a:accent2><a:srgbClr val=\"ED7D31\"/></a:accent2>\
<a:accent3><a:srgbClr val=\"A5A5A5\"/></a:accent3>\
<a:accent4><a:srgbClr val=\"FFC000\"/></a:accent4>\
<a:accent5><a:srgbClr val=\"5B9BD5\"/></a:accent5>\
<a:accent6><a:srgbClr val=\"70AD47\"/></a:accent6>\
<a:hlink><a:srgbClr val=\"0563C1\"/></a:hlink>\
<a:folHlink><a:srgbClr val=\"954F72\"/></a:folHlink>\
</a:clrScheme>\
<a:fontScheme name=\"gizza\">\
<a:majorFont><a:latin typeface=\"Calibri Light\"/><a:ea typeface=\"\"/><a:cs typeface=\"\"/></a:majorFont>\
<a:minorFont><a:latin typeface=\"Calibri\"/><a:ea typeface=\"\"/><a:cs typeface=\"\"/></a:minorFont>\
</a:fontScheme>\
<a:fmtScheme name=\"gizza\">\
<a:fillStyleLst>\
<a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill>\
<a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill>\
<a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill>\
</a:fillStyleLst>\
<a:lnStyleLst>\
<a:ln w=\"6350\" cap=\"flat\" cmpd=\"sng\" algn=\"ctr\"><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill><a:prstDash val=\"solid\"/></a:ln>\
<a:ln w=\"12700\" cap=\"flat\" cmpd=\"sng\" algn=\"ctr\"><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill><a:prstDash val=\"solid\"/></a:ln>\
<a:ln w=\"19050\" cap=\"flat\" cmpd=\"sng\" algn=\"ctr\"><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill><a:prstDash val=\"solid\"/></a:ln>\
</a:lnStyleLst>\
<a:effectStyleLst>\
<a:effectStyle><a:effectLst/></a:effectStyle>\
<a:effectStyle><a:effectLst/></a:effectStyle>\
<a:effectStyle><a:effectLst/></a:effectStyle>\
</a:effectStyleLst>\
<a:bgFillStyleLst>\
<a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill>\
<a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill>\
<a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill>\
</a:bgFillStyleLst>\
</a:fmtScheme>\
</a:themeElements>\
</a:theme>"
    )
}

/// Build one slide's XML. `colors` is `(bg, title, body)` as 6-hex RGB.
fn slide_xml(slide: &Slide, w: i64, h: i64, colors: (&str, &str, &str)) -> String {
    let (bg, title_clr, body_clr) = colors;
    let margin = 685_800i64; // ~0.75"

    let shapes = if slide.is_title {
        // One big, vertically-centred title covering most of the slide.
        let cx = w - 2 * margin;
        let cy = 1_600_000i64;
        let y = (h - cy) / 2;
        let para = format!(
            "<a:p><a:pPr algn=\"ctr\"/><a:r><a:rPr lang=\"en-US\" sz=\"5400\" b=\"1\" dirty=\"0\"><a:solidFill><a:srgbClr val=\"{title_clr}\"/></a:solidFill></a:rPr><a:t>{}</a:t></a:r></a:p>",
            esc(&slide.title)
        );
        format!(
            "<p:sp><p:nvSpPr><p:cNvPr id=\"2\" name=\"Title\"/><p:cNvSpPr><a:spLocks noGrp=\"1\"/></p:cNvSpPr><p:nvPr><p:ph type=\"ctrTitle\"/></p:nvPr></p:nvSpPr>\
<p:spPr><a:xfrm><a:off x=\"{margin}\" y=\"{y}\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr>\
<p:txBody><a:bodyPr anchor=\"ctr\"/><a:lstStyle/>{para}</p:txBody></p:sp>"
        )
    } else {
        // Title shape at the top.
        let title_cx = w - 2 * margin;
        let title_cy = 1_143_000i64;
        let title_para = if slide.title.trim().is_empty() {
            "<a:p><a:endParaRPr lang=\"en-US\"/></a:p>".to_string()
        } else {
            format!(
                "<a:p><a:r><a:rPr lang=\"en-US\" sz=\"4000\" b=\"1\" dirty=\"0\"><a:solidFill><a:srgbClr val=\"{title_clr}\"/></a:solidFill></a:rPr><a:t>{}</a:t></a:r></a:p>",
                esc(&slide.title)
            )
        };
        let title_sp = format!(
            "<p:sp><p:nvSpPr><p:cNvPr id=\"2\" name=\"Title\"/><p:cNvSpPr><a:spLocks noGrp=\"1\"/></p:cNvSpPr><p:nvPr><p:ph type=\"title\"/></p:nvPr></p:nvSpPr>\
<p:spPr><a:xfrm><a:off x=\"{margin}\" y=\"457200\"/><a:ext cx=\"{title_cx}\" cy=\"{title_cy}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr>\
<p:txBody><a:bodyPr/><a:lstStyle/>{title_para}</p:txBody></p:sp>"
        );

        // Body shape below the title with one paragraph per bullet.
        let body_y = 1_700_000i64;
        let body_cx = w - 2 * margin;
        let body_cy = h - body_y - 457_200;
        let mut body_paras = String::new();
        if slide.bullets.is_empty() {
            body_paras.push_str("<a:p><a:endParaRPr lang=\"en-US\"/></a:p>");
        } else {
            for b in &slide.bullets {
                let lvl = if b.level == 0 { String::new() } else { format!(" lvl=\"{}\"", b.level) };
                body_paras.push_str(&format!(
                    "<a:p><a:pPr{lvl}><a:buChar char=\"\u{2022}\"/></a:pPr><a:r><a:rPr lang=\"en-US\" sz=\"1800\" dirty=\"0\"><a:solidFill><a:srgbClr val=\"{body_clr}\"/></a:solidFill></a:rPr><a:t>{}</a:t></a:r></a:p>",
                    esc(&b.text)
                ));
            }
        }
        let body_sp = format!(
            "<p:sp><p:nvSpPr><p:cNvPr id=\"3\" name=\"Content\"/><p:cNvSpPr><a:spLocks noGrp=\"1\"/></p:cNvSpPr><p:nvPr><p:ph type=\"body\" idx=\"1\"/></p:nvPr></p:nvSpPr>\
<p:spPr><a:xfrm><a:off x=\"{margin}\" y=\"{body_y}\"/><a:ext cx=\"{body_cx}\" cy=\"{body_cy}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr>\
<p:txBody><a:bodyPr/><a:lstStyle/>{body_paras}</p:txBody></p:sp>"
        );
        format!("{title_sp}{body_sp}")
    };

    format!(
        "{DECL}<p:sld xmlns:a=\"{NS_A}\" xmlns:r=\"{NS_R}\" xmlns:p=\"{NS_P}\">\
<p:cSld>\
<p:bg><p:bgPr><a:solidFill><a:srgbClr val=\"{bg}\"/></a:solidFill><a:effectLst/></p:bgPr></p:bg>\
<p:spTree>{tree}{shapes}</p:spTree>\
</p:cSld>\
<p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>\
</p:sld>",
        tree = empty_sp_tree()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use zip::ZipArchive;

    /// Unzip `bytes` and return `(sorted part names, name -> content)`.
    fn unzip(bytes: &[u8]) -> (Vec<String>, std::collections::HashMap<String, String>) {
        let mut archive = ZipArchive::new(Cursor::new(bytes.to_vec())).expect("valid zip");
        let mut names = Vec::new();
        let mut map = std::collections::HashMap::new();
        for i in 0..archive.len() {
            let mut f = archive.by_index(i).unwrap();
            let name = f.name().to_string();
            let mut s = String::new();
            f.read_to_string(&mut s).unwrap();
            names.push(name.clone());
            map.insert(name, s);
        }
        names.sort();
        (names, map)
    }

    fn build(md: &str) -> Vec<u8> {
        to_pptx(md, "", SplitLevel::H1, Theme::Light, AspectRatio::Widescreen).unwrap()
    }

    #[test]
    fn produces_a_real_zip_with_required_pptx_parts() {
        let bytes = build("# Slide One\n\n- point A\n- point B\n\n# Slide Two\n\n- another");
        // ZIP local-file magic.
        assert_eq!(&bytes[0..4], &[0x50, 0x4b, 0x03, 0x04]);
        let (names, _) = unzip(&bytes);
        for required in [
            "[Content_Types].xml",
            "_rels/.rels",
            "ppt/presentation.xml",
            "ppt/_rels/presentation.xml.rels",
            "ppt/slideMasters/slideMaster1.xml",
            "ppt/slideLayouts/slideLayout1.xml",
            "ppt/theme/theme1.xml",
            "ppt/slides/slide1.xml",
            "ppt/slides/slide2.xml",
            "ppt/slides/_rels/slide1.xml.rels",
        ] {
            assert!(names.contains(&required.to_string()), "missing part {required}; got {names:?}");
        }
    }

    #[test]
    fn h1_split_makes_one_slide_per_heading() {
        let bytes = build("# One\n\ntext\n\n# Two\n\n# Three");
        let (names, _) = unzip(&bytes);
        let slides = names.iter().filter(|n| n.starts_with("ppt/slides/slide") && n.ends_with(".xml") && !n.contains("_rels")).count();
        assert_eq!(slides, 3, "expected 3 slides, got {slides}");
    }

    #[test]
    fn slide_xml_carries_title_and_bullet_text() {
        let bytes = build("# My Title\n\n- first bullet\n- second bullet");
        let (_, map) = unzip(&bytes);
        let s1 = &map["ppt/slides/slide1.xml"];
        assert!(s1.contains("<a:t>My Title</a:t>"), "title missing: {s1}");
        assert!(s1.contains("<a:t>first bullet</a:t>"));
        assert!(s1.contains("<a:t>second bullet</a:t>"));
        assert!(s1.contains("type=\"title\""));
        assert!(s1.contains("type=\"body\""));
    }

    #[test]
    fn special_characters_are_xml_escaped() {
        let bytes = build("# A & B <tag>\n\n- x > y \"quoted\"");
        let (_, map) = unzip(&bytes);
        let s1 = &map["ppt/slides/slide1.xml"];
        assert!(s1.contains("A &amp; B &lt;tag&gt;"), "title not escaped: {s1}");
        assert!(s1.contains("x &gt; y &quot;quoted&quot;"));
        assert!(!s1.contains("<tag>"));
    }

    #[test]
    fn thematic_break_starts_a_new_slide() {
        let bytes = build("- alpha\n\n---\n\n- beta");
        let (names, _) = unzip(&bytes);
        let slides = names.iter().filter(|n| n.starts_with("ppt/slides/slide") && n.ends_with(".xml") && !n.contains("_rels")).count();
        assert_eq!(slides, 2);
    }

    #[test]
    fn h2_split_level_cuts_on_double_hash_only() {
        let md = "# Section\n\n## Sub A\n\ntext\n\n## Sub B";
        let bytes = to_pptx(md, "", SplitLevel::H2, Theme::Light, AspectRatio::Widescreen).unwrap();
        let (names, map) = unzip(&bytes);
        let slides = names.iter().filter(|n| n.starts_with("ppt/slides/slide") && n.ends_with(".xml") && !n.contains("_rels")).count();
        assert_eq!(slides, 2, "expected 2 slides from ## split");
        assert!(map["ppt/slides/slide1.xml"].contains("<a:t>Sub A</a:t>"));
    }

    #[test]
    fn title_param_prepends_a_title_slide() {
        let bytes = to_pptx("# Content", "Deck Title", SplitLevel::H1, Theme::Light, AspectRatio::Widescreen).unwrap();
        let (_, map) = unzip(&bytes);
        assert!(map["ppt/slides/slide1.xml"].contains("<a:t>Deck Title</a:t>"));
        assert!(map["ppt/slides/slide1.xml"].contains("type=\"ctrTitle\""));
        assert!(map["ppt/slides/slide2.xml"].contains("<a:t>Content</a:t>"));
        // Deck title is recorded in the document properties.
        assert!(map["docProps/core.xml"].contains("<dc:title>Deck Title</dc:title>"));
    }

    #[test]
    fn dark_theme_and_aspect_ratio_are_applied() {
        let bytes = to_pptx("# Hi", "", SplitLevel::H1, Theme::Dark, AspectRatio::Standard).unwrap();
        let (_, map) = unzip(&bytes);
        assert!(map["ppt/slides/slide1.xml"].contains("val=\"1A1A1A\""), "dark bg missing");
        assert!(map["ppt/presentation.xml"].contains("cx=\"9144000\""), "4:3 width missing");
        assert!(map["ppt/presentation.xml"].contains("screen4x3"));
    }

    #[test]
    fn widescreen_default_dimensions() {
        let bytes = build("# Hi");
        let (_, map) = unzip(&bytes);
        assert!(map["ppt/presentation.xml"].contains("cx=\"12192000\""));
        assert!(map["ppt/presentation.xml"].contains("screen16x9"));
    }

    #[test]
    fn content_types_lists_every_slide() {
        let bytes = build("# A\n\n# B\n\n# C");
        let (_, map) = unzip(&bytes);
        let ct = &map["[Content_Types].xml"];
        for n in 1..=3 {
            assert!(ct.contains(&format!("/ppt/slides/slide{n}.xml")), "content-types missing slide{n}");
        }
    }

    #[test]
    fn empty_input_errors() {
        let err = to_pptx("   \n\n", "", SplitLevel::H1, Theme::Light, AspectRatio::Widescreen).unwrap_err();
        assert!(err.contains("empty"), "got: {err}");
    }

    #[test]
    fn markdown_marks_are_stripped_from_runs() {
        let bytes = build("# Title\n\n- **bold** and `code` and [link](http://x)");
        let (_, map) = unzip(&bytes);
        let s1 = &map["ppt/slides/slide1.xml"];
        assert!(s1.contains("<a:t>bold and code and link</a:t>"), "inline marks not stripped: {s1}");
    }

    #[test]
    fn parse_helpers() {
        assert_eq!(SplitLevel::parse("both").unwrap(), SplitLevel::Both);
        assert!(SplitLevel::parse("h9").is_err());
        assert_eq!(Theme::parse("DARK").unwrap(), Theme::Dark);
        assert!(Theme::parse("blue").is_err());
        assert_eq!(AspectRatio::parse("4:3").unwrap(), AspectRatio::Standard);
        assert!(AspectRatio::parse("21:9").is_err());
    }
}

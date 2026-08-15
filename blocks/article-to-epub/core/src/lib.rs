//! gizza-ai/article-to-epub core — package article text or cleaned HTML into a
//! valid EPUB 3 ebook.
//!
//! No wafer/wasm-bindgen deps, so it compiles natively for unit tests, to
//! `wasm32-wasip1` for the chat/CLI block, and to `wasm32-unknown-unknown` for
//! the page.
//!
//! ## Pipeline
//!
//! 1. **Parse** the article into a flat list of chapters:
//!    - *HTML* — a forgiving, quote-aware tag scanner re-emits an allowlisted
//!      XHTML subset with a balanced tag stack, so tag soup (unclosed `<p>`,
//!      stray `</div>`, `<script>` leftovers) still yields well-formed XHTML.
//!      Named/numeric entities are decoded to real characters, because XHTML
//!      only predefines five of them and a raw `&nbsp;` makes a book unopenable
//!      in strict readers.
//!    - *Text* — blank lines split paragraphs; Markdown-style ATX headings
//!      (`## Title`), `-`/`*`/`•` bullets, `1.` numbered items and `---` rules
//!      are recognised.
//! 2. **Split** into chapters at `<h1>`/`<h2>` boundaries (configurable) and
//!    take each chapter's title from its leading heading.
//! 3. **Assemble** an EPUB 3 OCF ZIP: `mimetype` first and stored uncompressed
//!    (per the OCF spec), `META-INF/container.xml`, a `content.opf` package
//!    document, an EPUB 3 `nav.xhtml`, an EPUB 2 `toc.ncx` for back-compat, a
//!    reading stylesheet and one XHTML file per chapter.
//!
//! Output is deterministic: fixed ZIP timestamps and a content-derived
//! `dc:identifier` mean the same input always produces byte-identical output.
//!
//! Images are not packaged (a pasted article's `<img>` points at a remote URL
//! the browser-local converter can't embed); their `alt` text is kept inline and
//! the dropped count is reported.

use std::io::Write;

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

/// Input cap — a 2 M-character article is already a ~350k-word book.
pub const MAX_INPUT_CHARS: usize = 2_000_000;
/// Chapter cap, so a heading-per-line document can't explode the ZIP entry count.
pub const MAX_CHAPTERS: usize = 2_000;

/// How the `content` argument should be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputFormat {
    /// Sniff HTML vs plain text from the content.
    Auto,
    /// Parse as (possibly messy) HTML.
    Html,
    /// Parse as plain text with Markdown-style headings and lists.
    Text,
}

impl InputFormat {
    /// Parse the `input_format` argument.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(InputFormat::Auto),
            "html" => Ok(InputFormat::Html),
            "text" => Ok(InputFormat::Text),
            other => Err(format!(
                "unknown input_format {other:?}: expected one of auto, html, text"
            )),
        }
    }

    /// Canonical lowercase name (`Auto` reports as `auto`).
    pub fn name(self) -> &'static str {
        match self {
            InputFormat::Auto => "auto",
            InputFormat::Html => "html",
            InputFormat::Text => "text",
        }
    }
}

/// Which heading levels start a new chapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitLevel {
    /// One single chapter, whatever the headings.
    None,
    /// New chapter at every `<h1>` / `# `.
    H1,
    /// New chapter at every `<h2>` / `## `.
    H2,
    /// New chapter at every `<h1>` and `<h2>`.
    H1H2,
}

impl SplitLevel {
    /// Parse the `split_level` argument.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "none" => Ok(SplitLevel::None),
            "" | "h1" => Ok(SplitLevel::H1),
            "h2" => Ok(SplitLevel::H2),
            "h1-h2" => Ok(SplitLevel::H1H2),
            other => Err(format!(
                "unknown split_level {other:?}: expected one of none, h1, h2, h1-h2"
            )),
        }
    }

    fn splits(self, level: u8) -> bool {
        match self {
            SplitLevel::None => false,
            SplitLevel::H1 => level == 1,
            SplitLevel::H2 => level == 2,
            SplitLevel::H1H2 => level <= 2,
        }
    }
}

/// Book metadata + conversion settings.
#[derive(Debug, Clone)]
pub struct Options {
    /// How to read `content`.
    pub input_format: InputFormat,
    /// Book title. Empty → derived from `<title>`, the first heading, or the
    /// first line of text.
    pub title: String,
    /// `dc:creator`. Empty → omitted.
    pub author: String,
    /// `dc:language` (BCP-47). Empty → `en`.
    pub language: String,
    /// `dc:publisher`. Empty → omitted.
    pub publisher: String,
    /// Where chapters begin.
    pub split_level: SplitLevel,
    /// Write a title page as the first spine item.
    pub include_title_page: bool,
    /// Base font size in points for the embedded stylesheet; 0 leaves the size
    /// to the reading app.
    pub base_font_size: f64,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            input_format: InputFormat::Auto,
            title: String::new(),
            author: String::new(),
            language: "en".to_string(),
            publisher: String::new(),
            split_level: SplitLevel::H1,
            include_title_page: true,
            base_font_size: 0.0,
        }
    }
}

/// A converted book.
#[derive(Debug, Clone)]
pub struct Conversion {
    /// The EPUB file bytes.
    pub epub: Vec<u8>,
    /// The resolved book title (derived when the caller left it blank).
    pub title: String,
    /// Chapter count (excluding the optional title page).
    pub chapters: usize,
    /// Word count of the article body.
    pub words: usize,
    /// How many `<img>` elements were dropped (their `alt` text is kept).
    pub images_dropped: usize,
    /// The format actually used (`Auto` resolved to `Html` or `Text`).
    pub format: InputFormat,
    /// Suggested download filename, e.g. `my-article.epub`.
    pub filename: String,
}

/// One chapter: a nav/TOC title plus its XHTML body markup.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Chapter {
    title: String,
    body: String,
}

impl Chapter {
    fn is_empty(&self) -> bool {
        self.title.trim().is_empty() && self.body.trim().is_empty()
    }
}

/// Parser output before metadata resolution.
#[derive(Debug, Default)]
struct Parsed {
    chapters: Vec<Chapter>,
    /// Text of the source `<title>` element, when present.
    doc_title: Option<String>,
    /// Text of the first heading seen, whatever its level.
    first_heading: Option<String>,
    images_dropped: usize,
    words: usize,
}

/// Convert an article into an EPUB.
///
/// Errors when the content is empty, over the size cap, or when `language` is
/// not a plausible BCP-47 tag.
pub fn convert(content: &str, opts: &Options) -> Result<Conversion, String> {
    if content.trim().is_empty() {
        return Err("article content is empty: paste the article text or HTML to package".into());
    }
    let n_chars = content.chars().count();
    if n_chars > MAX_INPUT_CHARS {
        return Err(format!(
            "article is {n_chars} characters, over the {MAX_INPUT_CHARS}-character cap: split it into parts and convert each one"
        ));
    }
    if !(0.0..=72.0).contains(&opts.base_font_size) || opts.base_font_size.is_nan() {
        return Err(format!(
            "base_font_size must be between 0 and 72 points (0 = the reading app's default), got {}",
            opts.base_font_size
        ));
    }
    let language = normalize_language(&opts.language)?;

    let format = match opts.input_format {
        InputFormat::Auto => {
            if looks_like_html(content) {
                InputFormat::Html
            } else {
                InputFormat::Text
            }
        }
        explicit => explicit,
    };

    let parsed = match format {
        InputFormat::Html => parse_html(content, opts.split_level),
        _ => parse_text(content, opts.split_level),
    };

    if parsed.chapters.len() > MAX_CHAPTERS {
        return Err(format!(
            "article split into {} chapters, over the {MAX_CHAPTERS}-chapter cap: use a deeper split level (or 'none')",
            parsed.chapters.len()
        ));
    }

    let title = resolve_title(opts, &parsed, content);
    let author = opts.author.trim();
    let publisher = opts.publisher.trim();

    // A content-derived identifier keeps the output deterministic (no clock, no
    // RNG) while still differing between books.
    let book_id = format!(
        "urn:gizza-ai:article-to-epub:{:016x}",
        fnv1a64(&[title.as_str(), author, publisher, content])
    );

    // Give every chapter a title for the TOC.
    let mut chapters = parsed.chapters;
    for (i, ch) in chapters.iter_mut().enumerate() {
        if ch.title.trim().is_empty() {
            ch.title = if i == 0 {
                title.clone()
            } else {
                format!("Section {}", i + 1)
            };
        }
    }
    if chapters.is_empty() {
        chapters.push(Chapter {
            title: title.clone(),
            body: String::new(),
        });
    }

    let epub = build_epub(
        &Book {
            title: &title,
            author,
            publisher,
            language: &language,
            book_id: &book_id,
            base_font_size: opts.base_font_size,
            title_page: opts.include_title_page,
        },
        &chapters,
    )?;

    Ok(Conversion {
        epub,
        filename: format!("{}.epub", filename_slug(&title)),
        title,
        chapters: chapters.len(),
        words: parsed.words,
        images_dropped: parsed.images_dropped,
        format,
    })
}

/// Resolve the book title: the explicit one, else the source `<title>`, else the
/// first heading, else the first non-empty line, else a fallback.
fn resolve_title(opts: &Options, parsed: &Parsed, content: &str) -> String {
    let explicit = opts.title.trim();
    if !explicit.is_empty() {
        return collapse_ws(explicit).trim().to_string();
    }
    for candidate in [parsed.doc_title.as_deref(), parsed.first_heading.as_deref()] {
        if let Some(t) = candidate {
            let t = collapse_ws(t);
            let t = t.trim();
            if !t.is_empty() {
                return truncate_chars(t, 120);
            }
        }
    }
    // First non-empty line of the raw content, tags stripped.
    for line in content.lines() {
        let stripped = collapse_ws(&strip_tags(line));
        let stripped = stripped.trim();
        if !stripped.is_empty() {
            return truncate_chars(stripped, 120);
        }
    }
    "Untitled Article".to_string()
}

/// Accept a plausible BCP-47 language tag (`en`, `pt-BR`, `zh-Hant-TW`).
fn normalize_language(raw: &str) -> Result<String, String> {
    let lang = raw.trim();
    if lang.is_empty() {
        return Ok("en".to_string());
    }
    let ok = lang.len() <= 35
        && lang
            .split('-')
            .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_alphanumeric()));
    if !ok {
        return Err(format!(
            "language {lang:?} is not a language tag: expected something like 'en', 'de' or 'pt-BR'"
        ));
    }
    Ok(lang.to_string())
}

/// True when the content carries enough markup to be treated as HTML.
fn looks_like_html(content: &str) -> bool {
    let lower = content.to_ascii_lowercase();
    const MARKERS: [&str; 20] = [
        "<html", "<body", "<article", "<section", "<div", "<p>", "<p ", "<h1", "<h2", "<h3", "<br",
        "<ul", "<ol", "<li", "<blockquote", "<span", "<table", "<img", "<a href", "<pre",
    ];
    MARKERS.iter().any(|m| lower.contains(m))
}

// ---------------------------------------------------------------------------
// Shared emitter: builds chapters of well-formed XHTML.
// ---------------------------------------------------------------------------

/// Tags emitted verbatim (open + close) when the source uses them.
const BLOCK_TAGS: [&str; 17] = [
    "p",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "ul",
    "ol",
    "li",
    "blockquote",
    "pre",
    "table",
    "tr",
    "td",
    "th",
    "dl",
];
/// Inline tags kept in the output.
const INLINE_TAGS: [&str; 12] = [
    "em", "strong", "i", "b", "u", "s", "code", "sub", "sup", "del", "ins", "abbr",
];
/// Tags dropped along with everything inside them.
const DROP_TAGS: [&str; 19] = [
    "script", "style", "head", "noscript", "iframe", "object", "embed", "svg", "math", "form",
    "select", "textarea", "button", "canvas", "audio", "video", "template", "map", "nav",
];
/// Elements that can directly contain text; anything else gets an implicit `<p>`.
const TEXT_CONTAINERS: [&str; 12] = [
    "p", "h1", "h2", "h3", "h4", "h5", "h6", "li", "td", "th", "dt", "dd",
];

fn is_block(tag: &str) -> bool {
    BLOCK_TAGS.contains(&tag) || matches!(tag, "dt" | "dd" | "thead" | "tbody" | "tfoot")
}
fn is_inline(tag: &str) -> bool {
    INLINE_TAGS.contains(&tag) || tag == "a"
}
fn is_text_container(tag: &str) -> bool {
    TEXT_CONTAINERS.contains(&tag)
}

/// Map a source tag name to the tag we emit, or `None` to unwrap it (drop the
/// tag, keep its contents).
fn map_tag(name: &str) -> Option<&'static str> {
    Some(match name {
        "p" => "p",
        "h1" => "h1",
        "h2" => "h2",
        "h3" => "h3",
        "h4" => "h4",
        "h5" => "h5",
        "h6" => "h6",
        "ul" | "menu" => "ul",
        "ol" => "ol",
        "li" => "li",
        "blockquote" | "q" => "blockquote",
        "pre" => "pre",
        "table" => "table",
        "thead" => "thead",
        "tbody" => "tbody",
        "tfoot" => "tfoot",
        "tr" => "tr",
        "td" => "td",
        "th" => "th",
        "dl" => "dl",
        "dt" => "dt",
        "dd" => "dd",
        "em" => "em",
        "strong" => "strong",
        "i" => "i",
        "b" => "b",
        "u" => "u",
        "s" | "strike" => "s",
        "code" | "kbd" | "samp" | "var" => "code",
        "sub" => "sub",
        "sup" => "sup",
        "del" => "del",
        "ins" => "ins",
        "abbr" => "abbr",
        "a" => "a",
        // figcaption reads as a caption paragraph.
        "figcaption" | "caption" => "p",
        _ => return None,
    })
}

/// Accumulates XHTML into chapters with a balanced tag stack.
struct Doc {
    chapters: Vec<Chapter>,
    cur: Chapter,
    stack: Vec<&'static str>,
    split: SplitLevel,
    capture_heading: bool,
    heading_buf: String,
    first_heading: Option<String>,
    doc_title: Option<String>,
    images_dropped: usize,
    words: usize,
    /// Whether the current text container already has text (so a stray leading
    /// space isn't emitted right after an opening tag).
    text_since_block: bool,
    pending_space: bool,
}

impl Doc {
    fn new(split: SplitLevel) -> Self {
        Doc {
            chapters: Vec::new(),
            cur: Chapter::default(),
            stack: Vec::new(),
            split,
            capture_heading: false,
            heading_buf: String::new(),
            first_heading: None,
            doc_title: None,
            images_dropped: 0,
            words: 0,
            text_since_block: false,
            pending_space: false,
        }
    }

    fn in_pre(&self) -> bool {
        self.stack.iter().any(|t| *t == "pre")
    }

    fn open_tag(&mut self, tag: &'static str, attrs: &str) {
        // A space between two inline runs belongs BEFORE the opening tag:
        // `Hello <em>world</em>`, never `Hello<em> world</em>`.
        if is_inline(tag) {
            self.flush_pending_space();
        }
        self.cur.body.push('<');
        self.cur.body.push_str(tag);
        self.cur.body.push_str(attrs);
        self.cur.body.push('>');
        self.stack.push(tag);
        if is_text_container(tag) {
            self.text_since_block = false;
            self.pending_space = false;
        }
    }

    /// Emit a deferred inter-word space, if one is owed.
    fn flush_pending_space(&mut self) {
        if self.pending_space && self.text_since_block {
            self.cur.body.push(' ');
        }
        self.pending_space = false;
    }

    fn pop_one(&mut self) {
        if let Some(tag) = self.stack.pop() {
            self.cur.body.push_str("</");
            self.cur.body.push_str(tag);
            self.cur.body.push('>');
            if tag.starts_with('h') && tag.len() == 2 {
                self.end_heading();
            }
        }
    }

    /// Close every open element (used before a heading and at the end).
    fn close_all(&mut self) {
        while !self.stack.is_empty() {
            self.pop_one();
        }
        self.pending_space = false;
    }

    /// Close inline elements plus an open `<p>` — what any new block needs.
    fn close_inline_and_p(&mut self) {
        while let Some(top) = self.stack.last() {
            if is_inline(top) || *top == "p" {
                self.pop_one();
            } else {
                break;
            }
        }
        self.pending_space = false;
    }

    /// Close up to and including the innermost `tag`, if it is open.
    fn close_through(&mut self, tag: &str) {
        if !self.stack.iter().any(|t| *t == tag) {
            return;
        }
        loop {
            let top = match self.stack.last() {
                Some(t) => *t,
                None => break,
            };
            self.pop_one();
            if top == tag {
                break;
            }
        }
        self.pending_space = false;
    }

    fn open_in_stack(&self, tag: &str) -> bool {
        self.stack.iter().any(|t| *t == tag)
    }

    /// Make sure text can be emitted: open an implicit `<p>` when nothing that
    /// can hold text is open.
    fn ensure_text_container(&mut self) {
        if !self.stack.iter().any(|t| is_text_container(t)) {
            self.open_tag("p", "");
        }
    }

    /// Start a heading: headings are top level, so everything else closes first.
    fn start_heading(&mut self, level: u8) {
        self.close_all();
        if self.split.splits(level) && !self.cur.is_empty() {
            self.flush_chapter();
        }
        let tag = match level {
            1 => "h1",
            2 => "h2",
            3 => "h3",
            4 => "h4",
            5 => "h5",
            _ => "h6",
        };
        self.capture_heading = true;
        self.heading_buf.clear();
        self.open_tag(tag, "");
    }

    fn end_heading(&mut self) {
        if !self.capture_heading {
            return;
        }
        self.capture_heading = false;
        let text = collapse_ws(&self.heading_buf).trim().to_string();
        if !text.is_empty() {
            if self.cur.title.trim().is_empty() {
                self.cur.title = truncate_chars(&text, 200);
            }
            if self.first_heading.is_none() {
                self.first_heading = Some(text);
            }
        }
    }

    fn flush_chapter(&mut self) {
        let done = std::mem::take(&mut self.cur);
        if !done.is_empty() {
            self.chapters.push(done);
        }
    }

    /// Emit decoded text.
    fn push_text(&mut self, decoded: &str) {
        if decoded.is_empty() {
            return;
        }
        self.words += decoded.split_whitespace().count();
        if self.capture_heading {
            self.heading_buf.push_str(decoded);
        }
        if self.in_pre() {
            self.cur.body.push_str(&xml_escape(decoded));
            self.text_since_block = true;
            return;
        }
        let leading_ws = decoded.starts_with(is_collapsible_ws);
        let trailing_ws = decoded.ends_with(is_collapsible_ws);
        let collapsed = collapse_ws(decoded);
        let trimmed = collapsed.trim_matches(is_collapsible_ws);
        if trimmed.is_empty() {
            // Whitespace-only run: remember it, so `<em>a</em> <em>b</em>` keeps
            // its space without a stray space after an opening tag.
            if self.text_since_block {
                self.pending_space = true;
            }
            return;
        }
        self.ensure_text_container();
        if leading_ws {
            self.pending_space = true;
        }
        self.flush_pending_space();
        self.cur.body.push_str(&xml_escape(trimmed));
        self.text_since_block = true;
        self.pending_space = trailing_ws;
    }

    /// Emit a self-closing element (`<br/>`, `<hr/>`).
    fn push_void(&mut self, tag: &str) {
        if tag == "hr" {
            self.close_inline_and_p();
            self.cur.body.push_str("<hr/>");
        } else {
            self.ensure_text_container();
            self.cur.body.push_str("<br/>");
            self.pending_space = false;
        }
    }

    fn finish(mut self) -> Parsed {
        self.close_all();
        self.flush_chapter();
        Parsed {
            chapters: self.chapters,
            doc_title: self.doc_title,
            first_heading: self.first_heading,
            images_dropped: self.images_dropped,
            words: self.words,
        }
    }
}

// ---------------------------------------------------------------------------
// HTML parsing
// ---------------------------------------------------------------------------

/// Index just past the tag's closing `>`, respecting quoted attribute values.
fn scan_tag_end(bytes: &[u8], start: usize) -> usize {
    let mut i = start + 1;
    let mut quote: Option<u8> = None;
    while i < bytes.len() {
        let c = bytes[i];
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None if c == b'"' || c == b'\'' => quote = Some(c),
            None if c == b'>' => return i + 1,
            None => {}
        }
        i += 1;
    }
    bytes.len()
}

/// Lowercased tag name out of a raw tag string (`</Div ...` → `div`).
fn tag_name(raw: &str) -> String {
    raw.trim_start_matches('<')
        .trim_start_matches('/')
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase()
}

/// Read an attribute value out of a raw open tag, entity-decoded.
fn attr(raw: &str, name: &str) -> Option<String> {
    let lower = raw.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut from = 0usize;
    while let Some(rel) = lower[from..].find(name) {
        let at = from + rel;
        from = at + name.len();
        // Must be preceded by whitespace and followed by optional ws + '='.
        if at == 0 || !bytes[at - 1].is_ascii_whitespace() {
            continue;
        }
        let mut j = at + name.len();
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        if j >= bytes.len() || bytes[j] != b'=' {
            continue;
        }
        j += 1;
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        if j >= bytes.len() {
            return None;
        }
        let raw_bytes = raw.as_bytes();
        let (start, end) = match raw_bytes[j] {
            q @ (b'"' | b'\'') => {
                let s = j + 1;
                let mut e = s;
                while e < raw_bytes.len() && raw_bytes[e] != q {
                    e += 1;
                }
                (s, e)
            }
            _ => {
                let s = j;
                let mut e = s;
                while e < raw_bytes.len()
                    && !raw_bytes[e].is_ascii_whitespace()
                    && raw_bytes[e] != b'>'
                    && raw_bytes[e] != b'/'
                {
                    e += 1;
                }
                (s, e)
            }
        };
        if start <= end && end <= raw.len() && raw.is_char_boundary(start) && raw.is_char_boundary(end)
        {
            return Some(decode_entities(&raw[start..end]));
        }
        return None;
    }
    None
}

/// Keep only links a reader can actually follow from inside a book.
fn safe_href(href: &str) -> Option<String> {
    let h = href.trim();
    let lower = h.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("mailto:")
    {
        Some(h.to_string())
    } else {
        None
    }
}

fn parse_html(input: &str, split: SplitLevel) -> Parsed {
    let bytes = input.as_bytes();
    let mut doc = Doc::new(split);
    // While inside a dropped element: (tag, nesting depth, capture buffer).
    let mut skip: Option<(String, usize, Option<String>)> = None;
    let mut i = 0usize;

    while i < bytes.len() {
        if bytes[i] == b'<' {
            if input[i..].starts_with("<!--") {
                i = match input[i + 4..].find("-->") {
                    Some(rel) => i + 4 + rel + 3,
                    None => bytes.len(),
                };
                continue;
            }
            let next = bytes.get(i + 1).copied().unwrap_or(0);
            let is_tag = next.is_ascii_alphabetic()
                || (next == b'/' && bytes.get(i + 2).is_some_and(|c| c.is_ascii_alphabetic()));
            if next == b'!' || next == b'?' {
                i = scan_tag_end(bytes, i);
                continue;
            }
            if !is_tag {
                // A stray `<` is literal text.
                if skip.is_none() {
                    doc.push_text("<");
                }
                i += 1;
                continue;
            }
            let end = scan_tag_end(bytes, i);
            let raw = &input[i..end];
            let closing = raw.starts_with("</");
            let name = tag_name(raw);
            i = end;

            if let Some((skip_tag, depth, capture)) = skip.as_mut() {
                if !closing && &name == skip_tag {
                    *depth += 1;
                } else if closing && &name == skip_tag {
                    if *depth == 0 {
                        if let Some(buf) = capture.take() {
                            let t = collapse_ws(&buf).trim().to_string();
                            if !t.is_empty() && doc.doc_title.is_none() {
                                doc.doc_title = Some(t);
                            }
                        }
                        skip = None;
                    } else {
                        *depth -= 1;
                    }
                }
                continue;
            }

            if !closing && (DROP_TAGS.contains(&name.as_str()) || name == "title") {
                let capture = if name == "title" {
                    Some(String::new())
                } else {
                    None
                };
                // Self-closing drop tags (`<embed/>`) have nothing to skip.
                if !raw.trim_end().trim_end_matches('>').trim_end().ends_with('/') {
                    skip = Some((name, 0, capture));
                }
                continue;
            }

            handle_tag(&mut doc, &name, raw, closing);
            continue;
        }

        // Text run up to the next '<'.
        let start = i;
        while i < bytes.len() && bytes[i] != b'<' {
            i += 1;
        }
        let text = &input[start..i];
        if let Some((_, _, capture)) = skip.as_mut() {
            if let Some(buf) = capture.as_mut() {
                buf.push_str(&decode_entities(text));
            }
            continue;
        }
        doc.push_text(&decode_entities(text));
    }

    doc.finish()
}

/// Apply one open/close tag to the document.
fn handle_tag(doc: &mut Doc, name: &str, raw: &str, closing: bool) {
    // Void elements first — they never nest.
    if !closing {
        match name {
            "br" => {
                doc.push_void("br");
                return;
            }
            "hr" => {
                doc.push_void("hr");
                return;
            }
            "img" => {
                doc.images_dropped += 1;
                if let Some(alt) = attr(raw, "alt") {
                    let alt = collapse_ws(&alt).trim().to_string();
                    if !alt.is_empty() {
                        doc.ensure_text_container();
                        doc.flush_pending_space();
                        doc.cur.body.push_str("<em>[Image: ");
                        doc.cur.body.push_str(&xml_escape(&alt));
                        doc.cur.body.push_str("]</em>");
                        doc.text_since_block = true;
                        doc.pending_space = false;
                    }
                }
                return;
            }
            _ => {}
        }
    }
    if matches!(name, "br" | "hr" | "img" | "wbr" | "source" | "col") {
        return; // stray close tags for void elements
    }

    let mapped = match map_tag(name) {
        Some(t) => t,
        // Unknown/structural element (div, span, section, figure…): unwrap it.
        // A block-level one still terminates an open paragraph.
        None => {
            if matches!(
                name,
                "div" | "section" | "article" | "main" | "header" | "footer" | "aside" | "figure"
            ) {
                doc.close_inline_and_p();
            }
            return;
        }
    };

    if closing {
        if mapped.len() == 2 && mapped.starts_with('h') {
            doc.close_through(mapped);
        } else {
            doc.close_through(mapped);
        }
        return;
    }

    match mapped {
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            let level = mapped.as_bytes()[1] - b'0';
            doc.start_heading(level);
        }
        "p" => {
            doc.close_inline_and_p();
            doc.open_tag("p", "");
        }
        "li" => {
            if !(doc.open_in_stack("ul") || doc.open_in_stack("ol")) {
                doc.close_inline_and_p();
                doc.open_tag("p", "");
            } else {
                doc.close_through_if_open("li");
                doc.open_tag("li", "");
            }
        }
        "dt" | "dd" => {
            if !doc.open_in_stack("dl") {
                doc.close_inline_and_p();
                doc.open_tag("p", "");
            } else {
                doc.close_through_if_open("dt");
                doc.close_through_if_open("dd");
                doc.open_tag(if mapped == "dt" { "dt" } else { "dd" }, "");
            }
        }
        "tr" => {
            if !doc.open_in_stack("table") {
                return;
            }
            doc.close_through_if_open("tr");
            doc.open_tag("tr", "");
        }
        "td" | "th" => {
            if !doc.open_in_stack("tr") {
                if !doc.open_in_stack("table") {
                    return;
                }
                doc.open_tag("tr", "");
            }
            doc.close_through_if_open("td");
            doc.close_through_if_open("th");
            doc.open_tag(if mapped == "td" { "td" } else { "th" }, "");
        }
        "thead" | "tbody" | "tfoot" => {
            if !doc.open_in_stack("table") {
                return;
            }
            doc.open_tag(mapped, "");
        }
        "a" => {
            doc.ensure_text_container();
            match attr(raw, "href").as_deref().and_then(safe_href) {
                Some(href) => {
                    let attrs = format!(" href=\"{}\"", xml_escape(&href));
                    doc.open_tag("a", &attrs);
                }
                // No usable target: unwrap, keeping the link text.
                None => {}
            }
        }
        t if is_inline(t) => {
            doc.ensure_text_container();
            doc.open_tag(t, "");
        }
        t if is_block(t) => {
            doc.close_inline_and_p();
            doc.open_tag(t, "");
        }
        _ => {}
    }
}

impl Doc {
    /// `close_through`, but only when that tag is currently open.
    fn close_through_if_open(&mut self, tag: &str) {
        if self.open_in_stack(tag) {
            self.close_through(tag);
        }
    }
}

// ---------------------------------------------------------------------------
// Plain-text parsing
// ---------------------------------------------------------------------------

/// Parse plain text: blank lines separate paragraphs, `#` headings, `-`/`*`/`•`
/// bullets, `1.` numbered items, `---` horizontal rules.
fn parse_text(input: &str, split: SplitLevel) -> Parsed {
    let mut doc = Doc::new(split);
    let mut para: Vec<String> = Vec::new();
    // (list tag, items)
    let mut list: Option<(&'static str, Vec<String>)> = None;

    for raw_line in input.lines() {
        let line = raw_line.trim_end_matches('\r');
        let trimmed = line.trim();

        if trimmed.is_empty() {
            flush_paragraph(&mut doc, &mut para);
            flush_list(&mut doc, &mut list);
            continue;
        }

        if let Some((level, text)) = atx_heading(trimmed) {
            flush_paragraph(&mut doc, &mut para);
            flush_list(&mut doc, &mut list);
            doc.start_heading(level);
            doc.push_text(&text);
            doc.close_all();
            continue;
        }

        if is_rule(trimmed) {
            flush_paragraph(&mut doc, &mut para);
            flush_list(&mut doc, &mut list);
            doc.push_void("hr");
            continue;
        }

        if let Some((kind, item)) = list_item(trimmed) {
            flush_paragraph(&mut doc, &mut para);
            match list.as_mut() {
                Some((k, items)) if *k == kind => items.push(item),
                Some(_) => {
                    flush_list(&mut doc, &mut list);
                    list = Some((kind, vec![item]));
                }
                None => list = Some((kind, vec![item])),
            }
            continue;
        }

        if let Some((_, items)) = list.as_mut() {
            // A plain line right after a bullet continues that item.
            if let Some(last) = items.last_mut() {
                last.push(' ');
                last.push_str(trimmed);
                continue;
            }
        }

        para.push(trimmed.to_string());
    }

    flush_paragraph(&mut doc, &mut para);
    flush_list(&mut doc, &mut list);
    doc.finish()
}

fn flush_paragraph(doc: &mut Doc, para: &mut Vec<String>) {
    if para.is_empty() {
        return;
    }
    let text = para.join(" ");
    para.clear();
    doc.close_inline_and_p();
    doc.open_tag("p", "");
    doc.push_text(&text);
    doc.close_through("p");
}

fn flush_list(doc: &mut Doc, list: &mut Option<(&'static str, Vec<String>)>) {
    let (kind, items) = match list.take() {
        Some(v) => v,
        None => return,
    };
    if items.is_empty() {
        return;
    }
    doc.close_inline_and_p();
    doc.open_tag(kind, "");
    for item in items {
        doc.open_tag("li", "");
        doc.push_text(&item);
        doc.close_through("li");
    }
    doc.close_through(kind);
}

/// `## Heading` → `(2, "Heading")`.
fn atx_heading(line: &str) -> Option<(u8, String)> {
    let hashes = line.chars().take_while(|c| *c == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &line[hashes..];
    if !rest.starts_with(' ') && !rest.starts_with('\t') {
        return None;
    }
    let text = rest.trim().trim_end_matches('#').trim();
    if text.is_empty() {
        return None;
    }
    Some((hashes as u8, text.to_string()))
}

/// `---`, `***` or `___` on their own line.
fn is_rule(line: &str) -> bool {
    let c = match line.chars().next() {
        Some(c @ ('-' | '*' | '_')) => c,
        _ => return false,
    };
    line.len() >= 3 && line.chars().all(|x| x == c)
}

/// `- item` / `* item` / `• item` → `("ul", item)`; `1. item` → `("ol", item)`.
fn list_item(line: &str) -> Option<(&'static str, String)> {
    for marker in ["- ", "* ", "• ", "+ "] {
        if let Some(rest) = line.strip_prefix(marker) {
            let rest = rest.trim();
            if !rest.is_empty() {
                return Some(("ul", rest.to_string()));
            }
        }
    }
    let digits = line.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && digits <= 3 {
        let rest = &line[digits..];
        for sep in [". ", ") "] {
            if let Some(item) = rest.strip_prefix(sep) {
                let item = item.trim();
                if !item.is_empty() {
                    return Some(("ol", item.to_string()));
                }
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Text helpers
// ---------------------------------------------------------------------------

/// XML-escape text for element content / attribute values.
fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            // Control characters are illegal in XML 1.0.
            c if (c as u32) < 0x20 && c != '\n' && c != '\t' && c != '\r' => out.push(' '),
            _ => out.push(c),
        }
    }
    out
}

/// Whitespace that may be folded away. No-break spaces are deliberately
/// excluded: `&nbsp;` decodes to U+00A0 and an author wrote it precisely so it
/// would NOT collapse into an ordinary space.
fn is_collapsible_ws(c: char) -> bool {
    c.is_whitespace() && !matches!(c, '\u{a0}' | '\u{202f}' | '\u{2007}')
}

/// Collapse every run of collapsible whitespace to a single space.
fn collapse_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut ws = false;
    for c in s.chars() {
        if is_collapsible_ws(c) {
            ws = true;
        } else {
            if ws && !out.is_empty() {
                out.push(' ');
            }
            ws = false;
            out.push(c);
        }
    }
    if ws && !out.is_empty() {
        out.push(' ');
    }
    out
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    s.chars().take(max).collect::<String>().trim_end().to_string()
}

/// Decode the HTML entities that matter for producing valid XHTML. XHTML
/// predefines only `&amp; &lt; &gt; &quot; &apos;`, so anything else — very much
/// including `&nbsp;` — must become a real character or the book won't parse.
fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] != b'&' {
            let start = i;
            while i < bytes.len() && bytes[i] != b'&' {
                i += 1;
            }
            out.push_str(&s[start..i]);
            continue;
        }
        // `&name;` or `&#123;` / `&#x7b;` — bounded so stray `&` stays literal.
        let end = s[i..]
            .char_indices()
            .take(12)
            .find(|(_, c)| *c == ';')
            .map(|(rel, _)| i + rel);
        match end {
            Some(e) => {
                let body = &s[i + 1..e];
                match entity_char(body) {
                    Some(c) => {
                        out.push(c);
                        i = e + 1;
                    }
                    None => {
                        out.push('&');
                        i += 1;
                    }
                }
            }
            None => {
                out.push('&');
                i += 1;
            }
        }
    }
    out
}

/// Resolve one entity body (without `&` / `;`) to a character.
fn entity_char(body: &str) -> Option<char> {
    if let Some(num) = body.strip_prefix('#') {
        let code = if let Some(hex) = num.strip_prefix(['x', 'X']) {
            u32::from_str_radix(hex, 16).ok()?
        } else {
            num.parse::<u32>().ok()?
        };
        return char::from_u32(code);
    }
    Some(match body {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => '\u{a0}',
        "mdash" => '—',
        "ndash" => '–',
        "hellip" => '…',
        "ldquo" => '\u{201c}',
        "rdquo" => '\u{201d}',
        "lsquo" => '\u{2018}',
        "rsquo" => '\u{2019}',
        "copy" => '©',
        "reg" => '®',
        "trade" => '™',
        "deg" => '°',
        "eacute" => 'é',
        "egrave" => 'è',
        "uuml" => 'ü',
        "ouml" => 'ö',
        "auml" => 'ä',
        "szlig" => 'ß',
        "middot" => '·',
        "bull" => '•',
        "times" => '×',
        "euro" => '€',
        "pound" => '£',
        "laquo" => '«',
        "raquo" => '»',
        _ => return None,
    })
}

/// Strip every tag from a line (used only for title fallback).
fn strip_tags(line: &str) -> String {
    let mut out = String::new();
    let mut depth = 0usize;
    for c in line.chars() {
        match c {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    decode_entities(&out)
}

/// A filesystem-friendly slug for the download filename.
fn filename_slug(title: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in title.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            dash = false;
        } else if !out.is_empty() && !dash {
            out.push('-');
            dash = true;
        }
    }
    let slug = out.trim_matches('-').to_string();
    let slug = truncate_chars(&slug, 60);
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "article".to_string()
    } else {
        slug
    }
}

/// FNV-1a 64 over the concatenated parts — a stable, clock-free book id.
fn fnv1a64(parts: &[&str]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for part in parts {
        for b in part.as_bytes() {
            hash ^= *b as u64;
            hash = hash.wrapping_mul(0x1000_0000_01b3);
        }
        hash ^= 0xff;
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    hash
}

// ---------------------------------------------------------------------------
// EPUB assembly
// ---------------------------------------------------------------------------

struct Book<'a> {
    title: &'a str,
    author: &'a str,
    publisher: &'a str,
    language: &'a str,
    book_id: &'a str,
    base_font_size: f64,
    title_page: bool,
}

/// The OCF container descriptor pointing at the package document.
const CONTAINER_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>
"#;

/// The embedded reading stylesheet.
fn build_css(base_font_size: f64) -> String {
    let size = if base_font_size > 0.0 {
        format!("  font-size: {}pt;\n", trim_float(base_font_size))
    } else {
        String::new()
    };
    format!(
        "body {{\n  margin: 1em;\n  line-height: 1.5;\n{size}}}\nh1, h2, h3, h4, h5, h6 {{\n  line-height: 1.25;\n  margin: 1.2em 0 0.5em;\n}}\np {{\n  margin: 0 0 1em;\n}}\nblockquote {{\n  margin: 1em 1.5em;\n  font-style: italic;\n}}\npre {{\n  white-space: pre-wrap;\n  word-wrap: break-word;\n}}\ncode {{\n  font-family: monospace;\n}}\ntable {{\n  border-collapse: collapse;\n}}\ntd, th {{\n  border: 1px solid #999;\n  padding: 0.25em 0.5em;\n}}\n.byline {{\n  font-style: italic;\n}}\n"
    )
}

/// Render a float without a trailing `.0` (`12.0` → `12`, `12.5` → `12.5`).
fn trim_float(v: f64) -> String {
    if (v - v.round()).abs() < f64::EPSILON {
        format!("{}", v.round() as i64)
    } else {
        format!("{v}")
    }
}

/// Wrap chapter markup in a complete XHTML document.
fn chapter_xhtml(language: &str, title: &str, body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" xml:lang="{lang}" lang="{lang}">
  <head>
    <meta charset="UTF-8"/>
    <title>{title}</title>
    <link rel="stylesheet" type="text/css" href="style.css"/>
  </head>
  <body>
{body}
  </body>
</html>
"#,
        lang = xml_escape(language),
        title = xml_escape(title),
        body = body,
    )
}

fn title_page_xhtml(book: &Book) -> String {
    let mut body = format!("    <h1>{}</h1>\n", xml_escape(book.title));
    if !book.author.is_empty() {
        body.push_str(&format!(
            "    <p class=\"byline\">{}</p>\n",
            xml_escape(book.author)
        ));
    }
    if !book.publisher.is_empty() {
        body.push_str(&format!(
            "    <p class=\"byline\">{}</p>\n",
            xml_escape(book.publisher)
        ));
    }
    chapter_xhtml(book.language, book.title, body.trim_end())
}

/// The EPUB 3 package document.
fn build_opf(book: &Book, chapters: &[Chapter]) -> String {
    let mut manifest = String::from(
        "    <item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\"/>\n    <item id=\"ncx\" href=\"toc.ncx\" media-type=\"application/x-dtbncx+xml\"/>\n    <item id=\"css\" href=\"style.css\" media-type=\"text/css\"/>\n",
    );
    let mut spine = String::new();
    if book.title_page {
        manifest.push_str("    <item id=\"titlepage\" href=\"title.xhtml\" media-type=\"application/xhtml+xml\"/>\n");
        spine.push_str("    <itemref idref=\"titlepage\"/>\n");
    }
    for i in 0..chapters.len() {
        manifest.push_str(&format!(
            "    <item id=\"ch{i}\" href=\"ch{i}.xhtml\" media-type=\"application/xhtml+xml\"/>\n"
        ));
        spine.push_str(&format!("    <itemref idref=\"ch{i}\"/>\n"));
    }
    let creator = if book.author.is_empty() {
        String::new()
    } else {
        format!(
            "    <dc:creator id=\"creator\">{}</dc:creator>\n",
            xml_escape(book.author)
        )
    };
    let publisher = if book.publisher.is_empty() {
        String::new()
    } else {
        format!(
            "    <dc:publisher>{}</dc:publisher>\n",
            xml_escape(book.publisher)
        )
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="book-id" xml:lang="{lang}">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:identifier id="book-id">{id}</dc:identifier>
    <dc:title>{title}</dc:title>
    <dc:language>{lang}</dc:language>
{creator}{publisher}    <meta property="dcterms:modified">2020-01-01T00:00:00Z</meta>
  </metadata>
  <manifest>
{manifest}  </manifest>
  <spine toc="ncx">
{spine}  </spine>
</package>
"#,
        lang = xml_escape(book.language),
        id = xml_escape(book.book_id),
        title = xml_escape(book.title),
        creator = creator,
        publisher = publisher,
        manifest = manifest,
        spine = spine,
    )
}

/// The EPUB 3 navigation document.
fn build_nav(book: &Book, chapters: &[Chapter]) -> String {
    let mut items = String::new();
    if book.title_page {
        items.push_str("        <li><a href=\"title.xhtml\">Title page</a></li>\n");
    }
    for (i, ch) in chapters.iter().enumerate() {
        items.push_str(&format!(
            "        <li><a href=\"ch{i}.xhtml\">{}</a></li>\n",
            xml_escape(&ch.title)
        ));
    }
    let body = format!(
        "    <nav epub:type=\"toc\" id=\"toc\">\n      <h1>Contents</h1>\n      <ol>\n{items}      </ol>\n    </nav>",
    );
    chapter_xhtml(book.language, book.title, &body)
}

/// The EPUB 2 NCX table of contents, kept for older readers.
fn build_ncx(book: &Book, chapters: &[Chapter]) -> String {
    let mut nav_points = String::new();
    let mut order = 1usize;
    if book.title_page {
        nav_points.push_str(&format!(
            "    <navPoint id=\"navPoint-{order}\" playOrder=\"{order}\">\n      <navLabel><text>Title page</text></navLabel>\n      <content src=\"title.xhtml\"/>\n    </navPoint>\n"
        ));
        order += 1;
    }
    for (i, ch) in chapters.iter().enumerate() {
        nav_points.push_str(&format!(
            "    <navPoint id=\"navPoint-{order}\" playOrder=\"{order}\">\n      <navLabel><text>{title}</text></navLabel>\n      <content src=\"ch{i}.xhtml\"/>\n    </navPoint>\n",
            title = xml_escape(&ch.title),
        ));
        order += 1;
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE ncx PUBLIC "-//NISO//DTD ncx 2005-1//EN" "http://www.daisy.org/z3986/2005/ncx-2005-1.dtd">
<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1">
  <head>
    <meta name="dtb:uid" content="{id}"/>
    <meta name="dtb:depth" content="1"/>
    <meta name="dtb:totalPageCount" content="0"/>
    <meta name="dtb:maxPageNumber" content="0"/>
  </head>
  <docTitle><text>{title}</text></docTitle>
  <navMap>
{nav_points}  </navMap>
</ncx>
"#,
        id = xml_escape(book.book_id),
        title = xml_escape(book.title),
        nav_points = nav_points,
    )
}

/// Assemble the OCF ZIP. `mimetype` MUST be the first entry and stored
/// uncompressed so readers can sniff the container type.
fn build_epub(book: &Book, chapters: &[Chapter]) -> Result<Vec<u8>, String> {
    let mut buf: Vec<u8> = Vec::new();
    {
        let cursor = std::io::Cursor::new(&mut buf);
        let mut zip = ZipWriter::new(cursor);

        // A fixed timestamp → deterministic output.
        let ts = zip::DateTime::from_date_and_time(2020, 1, 1, 0, 0, 0)
            .map_err(|e| format!("epub timestamp: {e:?}"))?;
        let stored = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Stored)
            .last_modified_time(ts);
        let deflated = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .last_modified_time(ts);

        let write = |zip: &mut ZipWriter<std::io::Cursor<&mut Vec<u8>>>,
                         name: &str,
                         opts: SimpleFileOptions,
                         data: &str|
         -> Result<(), String> {
            zip.start_file(name, opts)
                .map_err(|e| format!("zip {name}: {e}"))?;
            zip.write_all(data.as_bytes())
                .map_err(|e| format!("write {name}: {e}"))?;
            Ok(())
        };

        write(&mut zip, "mimetype", stored, "application/epub+zip")?;
        write(&mut zip, "META-INF/container.xml", deflated, CONTAINER_XML)?;
        write(
            &mut zip,
            "OEBPS/content.opf",
            deflated,
            &build_opf(book, chapters),
        )?;
        write(
            &mut zip,
            "OEBPS/nav.xhtml",
            deflated,
            &build_nav(book, chapters),
        )?;
        write(
            &mut zip,
            "OEBPS/toc.ncx",
            deflated,
            &build_ncx(book, chapters),
        )?;
        write(
            &mut zip,
            "OEBPS/style.css",
            deflated,
            &build_css(book.base_font_size),
        )?;
        if book.title_page {
            write(
                &mut zip,
                "OEBPS/title.xhtml",
                deflated,
                &title_page_xhtml(book),
            )?;
        }
        for (i, ch) in chapters.iter().enumerate() {
            let body = if ch.body.trim().is_empty() {
                format!("    <h1>{}</h1>", xml_escape(&ch.title))
            } else {
                format!("    {}", ch.body)
            };
            write(
                &mut zip,
                &format!("OEBPS/ch{i}.xhtml"),
                deflated,
                &chapter_xhtml(book.language, &ch.title, &body),
            )?;
        }

        zip.finish().map_err(|e| format!("finalize epub: {e}"))?;
    }
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use zip::ZipArchive;

    fn entry(epub: &[u8], name: &str) -> String {
        let mut ar = ZipArchive::new(std::io::Cursor::new(epub)).unwrap();
        let mut f = ar
            .by_name(name)
            .unwrap_or_else(|_| panic!("missing entry {name}"));
        let mut s = String::new();
        f.read_to_string(&mut s).unwrap();
        s
    }

    fn names(epub: &[u8]) -> Vec<String> {
        let mut ar = ZipArchive::new(std::io::Cursor::new(epub)).unwrap();
        (0..ar.len())
            .map(|i| ar.by_index(i).unwrap().name().to_string())
            .collect()
    }

    /// Every emitted chapter body must be parseable as XML.
    fn assert_well_formed(xhtml: &str) {
        let mut reader = quick_xml::Reader::from_str(xhtml);
        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(quick_xml::events::Event::Eof) => break,
                Ok(_) => {}
                Err(e) => panic!("not well-formed XHTML: {e}\n---\n{xhtml}"),
            }
            buf.clear();
        }
    }

    fn html_opts() -> Options {
        Options {
            input_format: InputFormat::Html,
            ..Options::default()
        }
    }

    #[test]
    fn packages_html_into_a_valid_epub() {
        let html = "<h1>Chapter One</h1><p>Hello <em>world</em>.</p><h1>Chapter Two</h1><p>More.</p>";
        let c = convert(
            html,
            &Options {
                title: "My Article".into(),
                author: "Jane Doe".into(),
                publisher: "Gizza Press".into(),
                ..html_opts()
            },
        )
        .unwrap();

        assert_eq!(c.chapters, 2);
        assert_eq!(c.title, "My Article");
        assert_eq!(c.filename, "my-article.epub");

        // mimetype first, stored, exact bytes (OCF requirement).
        let mut ar = ZipArchive::new(std::io::Cursor::new(&c.epub)).unwrap();
        {
            let first = ar.by_index(0).unwrap();
            assert_eq!(first.name(), "mimetype");
            assert_eq!(first.compression(), CompressionMethod::Stored);
        }
        assert_eq!(entry(&c.epub, "mimetype"), "application/epub+zip");

        let opf = entry(&c.epub, "OEBPS/content.opf");
        assert!(opf.contains("<dc:title>My Article</dc:title>"), "{opf}");
        assert!(opf.contains("<dc:creator id=\"creator\">Jane Doe</dc:creator>"), "{opf}");
        assert!(opf.contains("<dc:publisher>Gizza Press</dc:publisher>"), "{opf}");
        assert!(opf.contains("<dc:language>en</dc:language>"), "{opf}");
        assert!(opf.contains("idref=\"ch0\""), "{opf}");
        assert!(opf.contains("idref=\"ch1\""), "{opf}");
        assert!(opf.contains("idref=\"titlepage\""), "{opf}");

        // Chapters carry the right content + titles.
        let ch0 = entry(&c.epub, "OEBPS/ch0.xhtml");
        assert!(ch0.contains("<h1>Chapter One</h1>"), "{ch0}");
        assert!(ch0.contains("<p>Hello <em>world</em>.</p>"), "{ch0}");
        assert_well_formed(&ch0);
        let ch1 = entry(&c.epub, "OEBPS/ch1.xhtml");
        assert!(ch1.contains("More."), "{ch1}");
        assert_well_formed(&ch1);

        // Both TOC formats list both chapters.
        let nav = entry(&c.epub, "OEBPS/nav.xhtml");
        assert!(nav.contains(">Chapter One</a>"), "{nav}");
        assert!(nav.contains(">Chapter Two</a>"), "{nav}");
        assert_well_formed(&nav);
        let ncx = entry(&c.epub, "OEBPS/toc.ncx");
        assert!(ncx.contains("<text>Chapter Two</text>"), "{ncx}");

        let all = names(&c.epub);
        for want in [
            "META-INF/container.xml",
            "OEBPS/content.opf",
            "OEBPS/nav.xhtml",
            "OEBPS/toc.ncx",
            "OEBPS/style.css",
            "OEBPS/title.xhtml",
        ] {
            assert!(all.iter().any(|n| n == want), "missing {want} in {all:?}");
        }
    }

    #[test]
    fn plain_text_paragraphs_headings_and_lists() {
        let text = "# Intro\n\nFirst paragraph\nwrapped over lines.\n\n- one\n- two\n\n1. step one\n2. step two\n\n---\n\n## Later\n\nEnd.";
        let c = convert(
            text,
            &Options {
                input_format: InputFormat::Text,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(c.format, InputFormat::Text);
        assert_eq!(c.chapters, 1, "only the h1 splits at the default level");
        assert_eq!(c.title, "Intro", "title derived from the first heading");
        let ch0 = entry(&c.epub, "OEBPS/ch0.xhtml");
        assert!(ch0.contains("<h1>Intro</h1>"), "{ch0}");
        assert!(
            ch0.contains("<p>First paragraph wrapped over lines.</p>"),
            "{ch0}"
        );
        assert!(ch0.contains("<ul><li>one</li><li>two</li></ul>"), "{ch0}");
        assert!(
            ch0.contains("<ol><li>step one</li><li>step two</li></ol>"),
            "{ch0}"
        );
        assert!(ch0.contains("<hr/>"), "{ch0}");
        assert!(ch0.contains("<h2>Later</h2>"), "{ch0}");
        assert_well_formed(&ch0);
    }

    #[test]
    fn tag_soup_becomes_well_formed_xhtml() {
        // Unclosed <p>, stray </div>, an unknown wrapper, and a bare text run.
        let html = "<div><p>one<p>two</div></span>loose text<ul><li>a<li>b</ul>";
        let c = convert(html, &html_opts()).unwrap();
        let ch0 = entry(&c.epub, "OEBPS/ch0.xhtml");
        assert_well_formed(&ch0);
        assert!(ch0.contains("<p>one</p>"), "{ch0}");
        assert!(ch0.contains("<p>two</p>"), "{ch0}");
        assert!(ch0.contains("<p>loose text</p>"), "{ch0}");
        assert!(ch0.contains("<ul><li>a</li><li>b</li></ul>"), "{ch0}");
    }

    #[test]
    fn drops_scripts_styles_and_unsafe_links() {
        let html = "<title>Doc Title</title><script>alert('x')</script><style>p{color:red}</style><p>Text <a href=\"javascript:alert(1)\">bad</a> and <a href=\"https://example.com/a\">good</a>.</p>";
        let c = convert(html, &html_opts()).unwrap();
        let ch0 = entry(&c.epub, "OEBPS/ch0.xhtml");
        assert!(!ch0.contains("alert"), "script leaked: {ch0}");
        assert!(!ch0.contains("color:red"), "style leaked: {ch0}");
        assert!(!ch0.contains("javascript:"), "unsafe href kept: {ch0}");
        assert!(ch0.contains("bad"), "link text dropped: {ch0}");
        assert!(
            ch0.contains("<a href=\"https://example.com/a\">good</a>"),
            "{ch0}"
        );
        assert_eq!(c.title, "Doc Title", "title element wins over the first line");
        assert_well_formed(&ch0);
    }

    #[test]
    fn decodes_entities_so_xhtml_parses() {
        let html = "<p>Caf&eacute;&nbsp;&amp; co &#8212; &#x2019;tis fine &bogus;</p>";
        let c = convert(html, &html_opts()).unwrap();
        let ch0 = entry(&c.epub, "OEBPS/ch0.xhtml");
        assert_well_formed(&ch0);
        assert!(ch0.contains("Café"), "{ch0}");
        assert!(ch0.contains('\u{a0}'), "nbsp not decoded: {ch0}");
        assert!(ch0.contains("&amp; co"), "{ch0}");
        assert!(ch0.contains('—') && ch0.contains('\u{2019}'), "{ch0}");
        assert!(ch0.contains("&amp;bogus;"), "unknown entity not escaped: {ch0}");
    }

    #[test]
    fn images_are_dropped_but_alt_text_survives() {
        let html = "<p>See <img src=\"https://example.com/x.png\" alt=\"a red square\"/> here.</p>";
        let c = convert(html, &html_opts()).unwrap();
        assert_eq!(c.images_dropped, 1);
        let ch0 = entry(&c.epub, "OEBPS/ch0.xhtml");
        assert!(!ch0.contains("<img"), "{ch0}");
        assert!(ch0.contains("<em>[Image: a red square]</em>"), "{ch0}");
        assert_well_formed(&ch0);
    }

    #[test]
    fn split_levels_change_the_chapter_count() {
        let html = "<h1>A</h1><p>1</p><h2>B</h2><p>2</p><h1>C</h1><p>3</p>";
        let count = |split| {
            convert(
                html,
                &Options {
                    split_level: split,
                    ..html_opts()
                },
            )
            .unwrap()
            .chapters
        };
        assert_eq!(count(SplitLevel::None), 1);
        assert_eq!(count(SplitLevel::H1), 2);
        assert_eq!(count(SplitLevel::H2), 2, "content before the h2, then the h2");
        assert_eq!(count(SplitLevel::H1H2), 3);
    }

    #[test]
    fn title_page_and_font_size_are_optional() {
        let with = convert(
            "<p>x</p>",
            &Options {
                title: "T".into(),
                base_font_size: 12.5,
                ..html_opts()
            },
        )
        .unwrap();
        assert!(names(&with.epub).iter().any(|n| n == "OEBPS/title.xhtml"));
        assert!(entry(&with.epub, "OEBPS/style.css").contains("font-size: 12.5pt;"));

        let without = convert(
            "<p>x</p>",
            &Options {
                title: "T".into(),
                include_title_page: false,
                ..html_opts()
            },
        )
        .unwrap();
        assert!(!names(&without.epub).iter().any(|n| n == "OEBPS/title.xhtml"));
        assert!(!entry(&without.epub, "OEBPS/style.css").contains("font-size"));
        assert!(!entry(&without.epub, "OEBPS/content.opf").contains("titlepage"));
    }

    #[test]
    fn auto_detects_html_versus_text() {
        let html = convert("<p>hi there</p>", &Options::default()).unwrap();
        assert_eq!(html.format, InputFormat::Html);
        let text = convert("hi there\n\nsecond para", &Options::default()).unwrap();
        assert_eq!(text.format, InputFormat::Text);
        // The `<` in prose must not be swallowed.
        let prose = convert("a < b and b > c", &Options::default()).unwrap();
        assert_eq!(prose.format, InputFormat::Text);
        let ch0 = entry(&prose.epub, "OEBPS/ch0.xhtml");
        assert!(ch0.contains("a &lt; b and b &gt; c"), "{ch0}");
        assert_well_formed(&ch0);
    }

    #[test]
    fn title_falls_back_to_the_first_line_then_a_placeholder() {
        let c = convert("Just some prose with no heading.", &Options::default()).unwrap();
        assert_eq!(c.title, "Just some prose with no heading.");
        // Nothing usable as a title (punctuation only) still yields a filename.
        let c2 = convert("...", &Options::default()).unwrap();
        assert_eq!(c2.filename, "article.epub");
    }

    #[test]
    fn word_count_and_metadata_reported() {
        let c = convert(
            "<h1>Title</h1><p>one two three</p>",
            &Options {
                author: "A".into(),
                ..html_opts()
            },
        )
        .unwrap();
        assert_eq!(c.words, 4, "heading words count too");
        assert_eq!(c.images_dropped, 0);
    }

    #[test]
    fn deterministic_output() {
        let opts = Options {
            title: "Same".into(),
            ..html_opts()
        };
        let a = convert("<p>same input</p>", &opts).unwrap();
        let b = convert("<p>same input</p>", &opts).unwrap();
        assert_eq!(a.epub, b.epub, "same inputs must give byte-identical EPUBs");
    }

    #[test]
    fn rejects_empty_oversized_and_bad_metadata() {
        let err = convert("   ", &Options::default()).unwrap_err();
        assert!(err.contains("empty"), "{err}");

        let big = "x ".repeat(MAX_INPUT_CHARS);
        let err = convert(&big, &Options::default()).unwrap_err();
        assert!(err.contains("over the"), "{err}");

        let err = convert(
            "hi",
            &Options {
                language: "not a tag!".into(),
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("language tag"), "{err}");

        let err = convert(
            "hi",
            &Options {
                base_font_size: 500.0,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("base_font_size"), "{err}");
    }

    #[test]
    fn language_flows_into_the_package_and_chapters() {
        let c = convert(
            "<p>hola</p>",
            &Options {
                language: "es-ES".into(),
                ..html_opts()
            },
        )
        .unwrap();
        assert!(entry(&c.epub, "OEBPS/content.opf").contains("<dc:language>es-ES</dc:language>"));
        assert!(entry(&c.epub, "OEBPS/ch0.xhtml").contains("xml:lang=\"es-ES\""));
    }

    #[test]
    fn tables_and_blockquotes_survive() {
        let html = "<blockquote>quoted</blockquote><table><tr><th>h</th><td>d</td></tr></table>";
        let c = convert(html, &html_opts()).unwrap();
        let ch0 = entry(&c.epub, "OEBPS/ch0.xhtml");
        assert!(ch0.contains("<blockquote><p>quoted</p></blockquote>"), "{ch0}");
        assert!(
            ch0.contains("<table><tr><th>h</th><td>d</td></tr></table>"),
            "{ch0}"
        );
        assert_well_formed(&ch0);
    }

    #[test]
    fn pre_blocks_keep_their_whitespace() {
        let html = "<pre>line one\n  indented</pre>";
        let c = convert(html, &html_opts()).unwrap();
        let ch0 = entry(&c.epub, "OEBPS/ch0.xhtml");
        assert!(ch0.contains("<pre>line one\n  indented</pre>"), "{ch0}");
        assert_well_formed(&ch0);
    }

    #[test]
    fn input_format_and_split_level_parse_errors() {
        assert!(InputFormat::parse("pdf").unwrap_err().contains("auto, html, text"));
        assert_eq!(InputFormat::parse(" HTML ").unwrap(), InputFormat::Html);
        assert!(SplitLevel::parse("h7").unwrap_err().contains("none, h1, h2, h1-h2"));
        assert_eq!(SplitLevel::parse("H1-H2").unwrap(), SplitLevel::H1H2);
    }

    #[test]
    fn filename_slug_is_safe() {
        assert_eq!(filename_slug("Hello, World! 2026"), "hello-world-2026");
        assert_eq!(filename_slug("   "), "article");
        assert!(filename_slug(&"a".repeat(200)).len() <= 60);
    }

    #[test]
    fn chapter_cap_is_enforced() {
        let many = "# h\n\n".repeat(MAX_CHAPTERS + 5);
        let err = convert(
            &many,
            &Options {
                input_format: InputFormat::Text,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("chapter cap"), "{err}");
    }
}

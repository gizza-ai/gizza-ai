//! markdown-strip core — remove Markdown formatting to produce clean plain text.
//! Pure compute, shared by the chat skill block and the web page. No
//! wafer/wasm-bindgen deps.
//!
//! We parse the Markdown with `pulldown-cmark` (CommonMark + GitHub-flavored
//! tables/strikethrough/task-lists/footnotes) and walk the event stream, keeping
//! the textual content while dropping the syntax: headings lose their `#`,
//! emphasis/strong/strikethrough markers are removed, blockquote `>` and
//! horizontal rules are dropped, fenced/inline code content is kept verbatim
//! (only the fences/backticks go), tables become bare cell text (cells joined by
//! spaces, one row per line), and links/images are handled per the `links` /
//! `images` options.

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

/// Accepted values for the `links` option (how a `[text](url)` link renders).
pub const LINK_MODES: [&str; 3] = ["text", "url", "both"];
/// Accepted values for the `images` option (how a `![alt](url)` image renders).
pub const IMAGE_MODES: [&str; 2] = ["alt", "drop"];

#[derive(Clone, Copy, PartialEq)]
enum Links {
    /// Keep the visible link text, drop the URL (default).
    Text,
    /// Keep the URL, drop the visible text.
    Url,
    /// Keep both: `text (url)`.
    Both,
}

#[derive(Clone, Copy, PartialEq)]
enum Images {
    /// Keep the image's alt text (default).
    Alt,
    /// Drop images entirely.
    Drop,
}

fn parse_links(s: &str) -> Result<Links, String> {
    match s {
        "" | "text" => Ok(Links::Text),
        "url" => Ok(Links::Url),
        "both" => Ok(Links::Both),
        other => Err(format!(
            "invalid links {other:?}: expected \"text\", \"url\", or \"both\""
        )),
    }
}

fn parse_images(s: &str) -> Result<Images, String> {
    match s {
        "" | "alt" => Ok(Images::Alt),
        "drop" => Ok(Images::Drop),
        other => Err(format!(
            "invalid images {other:?}: expected \"alt\" or \"drop\""
        )),
    }
}

/// Strip all Markdown formatting from `text`, returning clean plain text.
///
/// - `links` (`"text"` | `"url"` | `"both"`, blank → `"text"`): how to render a
///   `[label](url)` link — keep the visible label (default), the URL, or
///   `label (url)`.
/// - `images` (`"alt"` | `"drop"`, blank → `"alt"`): keep an image's alt text
///   (default) or remove images entirely.
/// - `keep_list_markers`: when `true`, preserve list bullets (`- `) and ordered
///   numbering (`1. `); when `false` (default) the markers are removed, leaving
///   one item per line.
/// - `collapse_blank_lines`: when `true` (default) blocks are separated by a
///   single newline (compact); when `false` a blank line is kept between blocks.
///
/// Returns `Err` on an unknown `links`/`images` value, or when `text` is empty
/// or whitespace-only.
pub fn strip(
    text: &str,
    links: &str,
    images: &str,
    keep_list_markers: bool,
    collapse_blank_lines: bool,
) -> Result<String, String> {
    let links = parse_links(links)?;
    let images = parse_images(images)?;
    if text.trim().is_empty() {
        return Err("input is empty".into());
    }

    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_FOOTNOTES);

    let mut s = Stripper {
        out: String::new(),
        sep: if collapse_blank_lines { 1 } else { 2 },
        keep_markers: keep_list_markers,
        links,
        images,
        suppress: 0,
        list_stack: Vec::new(),
        link_urls: Vec::new(),
        in_table: false,
        row_first_cell: true,
    };

    for ev in Parser::new_ext(text, options) {
        match ev {
            Event::Start(tag) => s.start(tag),
            Event::End(tag) => s.end(tag),
            Event::Text(t) | Event::Code(t) => s.push_text(&t),
            Event::SoftBreak | Event::HardBreak => s.push_break(),
            // Raw HTML tags, horizontal rules, task-list checkboxes and footnote
            // references carry no plain-text content — drop them (any human text
            // between HTML tags still arrives as separate Text events).
            _ => {}
        }
    }

    let result = s.out.trim().to_string();
    if result.is_empty() {
        return Err("nothing left after stripping Markdown".into());
    }
    Ok(result)
}

struct Stripper {
    out: String,
    /// Newlines inserted between top-level blocks (1 = compact, 2 = blank line).
    sep: usize,
    keep_markers: bool,
    links: Links,
    images: Images,
    /// When > 0, `push_text`/`push_break` emit nothing (inside a dropped image or
    /// a URL-only link where the visible text is discarded).
    suppress: u32,
    /// One entry per open list: `Some(n)` = ordered list, next number `n`;
    /// `None` = bullet list.
    list_stack: Vec<Option<u64>>,
    /// Destination URLs of the currently-open links (for `url`/`both` modes).
    link_urls: Vec<String>,
    in_table: bool,
    row_first_cell: bool,
}

impl Stripper {
    fn push_text(&mut self, s: &str) {
        if self.suppress == 0 {
            self.out.push_str(s);
        }
    }

    /// A soft/hard line break inside inline content: a newline normally, but a
    /// space inside a table cell so the row stays on one line.
    fn push_break(&mut self) {
        if self.suppress > 0 || self.out.is_empty() {
            return;
        }
        self.out.push(if self.in_table { ' ' } else { '\n' });
    }

    /// Ensure the output ends with exactly `sep` newlines before a new top-level
    /// block (no-op at the very start of the document).
    fn start_block(&mut self) {
        self.trim_trailing_newlines();
        if self.out.is_empty() {
            return;
        }
        for _ in 0..self.sep {
            self.out.push('\n');
        }
    }

    /// Ensure the output ends with exactly one newline (between list items and
    /// table rows).
    fn new_line(&mut self) {
        self.trim_trailing_newlines();
        if self.out.is_empty() {
            return;
        }
        self.out.push('\n');
    }

    fn trim_trailing_newlines(&mut self) {
        while self.out.ends_with('\n') {
            self.out.pop();
        }
    }

    fn start(&mut self, tag: Tag) {
        match tag {
            // A paragraph inside a list item continues on the item's own line;
            // at the top level it starts a fresh block.
            Tag::Paragraph => {
                if self.list_stack.is_empty() && !self.in_table {
                    self.start_block();
                }
            }
            Tag::Heading { .. } | Tag::BlockQuote(..) | Tag::CodeBlock(..) => self.start_block(),
            Tag::FootnoteDefinition(..) => self.start_block(),
            Tag::List(start) => {
                // Only a top-level list needs a block separator; a nested list's
                // first item supplies its own newline.
                if self.list_stack.is_empty() {
                    self.start_block();
                }
                self.list_stack.push(start);
            }
            Tag::Item => {
                self.new_line();
                if self.keep_markers {
                    let indent = "  ".repeat(self.list_stack.len().saturating_sub(1));
                    self.out.push_str(&indent);
                    match self.list_stack.last_mut() {
                        Some(Some(n)) => {
                            self.out.push_str(&format!("{n}. "));
                            *n += 1;
                        }
                        Some(None) => self.out.push_str("- "),
                        None => {}
                    }
                }
            }
            Tag::Table(..) => {
                self.start_block();
                self.in_table = true;
            }
            Tag::TableHead | Tag::TableRow => {
                self.new_line();
                self.row_first_cell = true;
            }
            Tag::TableCell => {
                if !self.row_first_cell {
                    self.out.push(' ');
                }
                self.row_first_cell = false;
            }
            Tag::Link { dest_url, .. } => {
                self.link_urls.push(dest_url.to_string());
                if self.links == Links::Url {
                    self.suppress += 1;
                }
            }
            Tag::Image { .. } => {
                if self.images == Images::Drop {
                    self.suppress += 1;
                }
            }
            // Emphasis/Strong/Strikethrough and everything else: keep the inner
            // text, drop the wrapper.
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::List(_) => {
                self.list_stack.pop();
            }
            TagEnd::Table => self.in_table = false,
            TagEnd::Link => {
                let url = self.link_urls.pop().unwrap_or_default();
                match self.links {
                    Links::Text => {}
                    Links::Url => {
                        self.suppress -= 1;
                        self.push_text(&url);
                    }
                    Links::Both => self.push_text(&format!(" ({url})")),
                }
            }
            TagEnd::Image => {
                if self.images == Images::Drop {
                    self.suppress -= 1;
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strip_default(text: &str) -> String {
        strip(text, "", "", false, true).unwrap()
    }

    #[test]
    fn strips_headings_emphasis_and_strikethrough() {
        let md = "# Title\n\nSome **bold**, *italic* and ~~struck~~ text.";
        assert_eq!(strip_default(md), "Title\nSome bold, italic and struck text.");
    }

    #[test]
    fn links_default_keeps_text_drops_url() {
        assert_eq!(
            strip_default("See [the docs](https://example.com/docs) here."),
            "See the docs here."
        );
    }

    #[test]
    fn links_url_and_both_modes() {
        let md = "Read [the docs](https://example.com).";
        assert_eq!(
            strip(md, "url", "", false, true).unwrap(),
            "Read https://example.com."
        );
        assert_eq!(
            strip(md, "both", "", false, true).unwrap(),
            "Read the docs (https://example.com)."
        );
    }

    #[test]
    fn images_alt_default_and_drop() {
        let md = "![a red square](img.png) after";
        assert_eq!(strip(md, "", "alt", false, true).unwrap(), "a red square after");
        assert_eq!(strip(md, "", "drop", false, true).unwrap(), "after");
    }

    #[test]
    fn keeps_fenced_and_inline_code_content_without_fences() {
        let md = "Run `cargo test` then:\n\n```sh\nls -la\n```";
        assert_eq!(strip_default(md), "Run cargo test then:\nls -la");
    }

    #[test]
    fn blockquotes_and_rules_are_stripped() {
        let md = "> quoted line\n\n---\n\nplain";
        assert_eq!(strip_default(md), "quoted line\nplain");
    }

    #[test]
    fn lists_drop_markers_by_default_and_keep_them_when_asked() {
        let md = "- apple\n- banana\n- cherry";
        assert_eq!(strip_default(md), "apple\nbanana\ncherry");
        assert_eq!(
            strip(md, "", "", true, true).unwrap(),
            "- apple\n- banana\n- cherry"
        );
    }

    #[test]
    fn ordered_list_markers_are_numbered_when_kept() {
        let md = "1. first\n2. second\n3. third";
        assert_eq!(
            strip(md, "", "", true, true).unwrap(),
            "1. first\n2. second\n3. third"
        );
        // Numbering follows the source start value.
        let md2 = "5. five\n6. six";
        assert_eq!(strip(md2, "", "", true, true).unwrap(), "5. five\n6. six");
    }

    #[test]
    fn tables_become_bare_cell_text_one_row_per_line() {
        let md = "| Name | Qty |\n| --- | --- |\n| Apple | 3 |\n| Pear | 7 |";
        assert_eq!(
            strip_default(md),
            "Name Qty\nApple 3\nPear 7"
        );
    }

    #[test]
    fn collapse_blank_lines_controls_block_spacing() {
        let md = "First paragraph.\n\nSecond paragraph.";
        // collapse (default): single newline between blocks.
        assert_eq!(
            strip(md, "", "", false, true).unwrap(),
            "First paragraph.\nSecond paragraph."
        );
        // no collapse: a blank line between blocks.
        assert_eq!(
            strip(md, "", "", false, false).unwrap(),
            "First paragraph.\n\nSecond paragraph."
        );
    }

    #[test]
    fn strips_inline_html_tags_keeping_text() {
        assert_eq!(strip_default("a <b>bold</b> word"), "a bold word");
    }

    #[test]
    fn rejects_empty_input() {
        let err = strip("   \n  ", "", "", false, true).unwrap_err();
        assert!(err.contains("empty"), "got: {err}");
    }

    #[test]
    fn rejects_unknown_link_and_image_modes() {
        let err = strip("x", "hyperlink", "", false, true).unwrap_err();
        assert!(err.contains("invalid links"), "got: {err}");
        let err = strip("x", "", "keep", false, true).unwrap_err();
        assert!(err.contains("invalid images"), "got: {err}");
    }

    #[test]
    fn nested_lists_indent_when_markers_kept() {
        let md = "- top\n  - nested\n- back";
        assert_eq!(
            strip(md, "", "", true, true).unwrap(),
            "- top\n  - nested\n- back"
        );
    }
}

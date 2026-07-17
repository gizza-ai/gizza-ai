//! gizza-ai/epub-extract core — parse an EPUB into structured chapters +
//! metadata. An EPUB is a ZIP of XHTML; we read `META-INF/container.xml` → the
//! OPF package (metadata + manifest + spine) for reading order, resolve chapter
//! titles from the real table of contents (NCX navMap and/or the EPUB3 nav
//! document) with a heading-detection fallback, and strip each spine document to
//! plain readable text. Pure-Rust (`zip`, `quick-xml`, `nanohtml2text`) so it
//! runs on every backend including the chat Service Worker.
//!
//! Distinct from `epub-to-markdown` (which concatenates the whole book into ONE
//! Markdown/text blob + a chapter count): this returns a navigable per-chapter
//! structure — each chapter's title, word count, and text — plus book metadata,
//! so callers can search, summarize, or quote an individual chapter.

use std::io::{Cursor, Read};

use quick_xml::events::Event;
use quick_xml::Reader;
use serde::Serialize;
use zip::ZipArchive;

/// Book-level metadata pulled from the OPF `<metadata>` (Dublin Core).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Metadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub language: Option<String>,
    pub publisher: Option<String>,
    pub date: Option<String>,
}

/// One spine document, in reading order, with its resolved title + plain text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Chapter {
    /// 1-based position in reading (spine) order.
    pub index: usize,
    pub title: String,
    pub words: usize,
    pub text: String,
}

/// Full structured extraction result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ebook {
    pub metadata: Metadata,
    pub chapters: Vec<Chapter>,
}

/// Local name of an element, dropping any namespace prefix.
fn local_name(raw: &[u8]) -> &[u8] {
    match raw.iter().position(|&b| b == b':') {
        Some(i) => &raw[i + 1..],
        None => raw,
    }
}

fn attr(e: &quick_xml::events::BytesStart, key: &str) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        if local_name(a.key.as_ref()) == key.as_bytes() {
            Some(String::from_utf8_lossy(&a.value).into_owned())
        } else {
            None
        }
    })
}

/// Collapse all runs of whitespace to single spaces and trim.
fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Drop a URL fragment (`#id`) from a TOC href.
fn strip_fragment(s: &str) -> &str {
    s.split('#').next().unwrap_or(s)
}

fn is_heading(ln: &[u8]) -> bool {
    matches!(ln, b"h1" | b"h2" | b"h3" | b"h4" | b"h5" | b"h6")
}

/// Normalize a path that may contain `..`/`.` segments.
fn normalize(path: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            s => out.push(s),
        }
    }
    out.join("/")
}

/// Join `href` (relative to `base_file`'s directory) into a normalized path.
fn join_href(base_file: &str, href: &str) -> String {
    let dir = match base_file.rfind('/') {
        Some(i) => &base_file[..i],
        None => "",
    };
    let combined = if dir.is_empty() {
        href.to_string()
    } else {
        format!("{dir}/{href}")
    };
    normalize(&combined)
}

fn read_zip_entry(zip: &mut ZipArchive<Cursor<&[u8]>>, name: &str) -> Option<String> {
    let mut f = zip.by_name(name).ok()?;
    let mut s = String::new();
    f.read_to_string(&mut s).ok()?;
    Some(s)
}

/// Find the OPF path from META-INF/container.xml.
fn find_opf_path(container_xml: &str) -> Option<String> {
    let mut reader = Reader::from_str(container_xml);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Empty(e)) | Ok(Event::Start(e)) => {
                if local_name(e.name().as_ref()) == b"rootfile" {
                    if let Some(p) = attr(&e, "full-path") {
                        return Some(p);
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => return None,
            _ => {}
        }
    }
}

struct Opf {
    metadata: Metadata,
    /// spine order of html hrefs (joined to the OPF dir).
    spine_hrefs: Vec<String>,
    /// NCX toc path (joined to OPF dir), if present.
    ncx_href: Option<String>,
    /// EPUB3 nav document path (joined to OPF dir), if present.
    nav_href: Option<String>,
}

/// Parse the OPF: Dublin Core metadata + manifest (id→href/media-type/properties)
/// + spine (idref order + toc id).
fn parse_opf(opf_xml: &str, opf_path: &str) -> Opf {
    let mut reader = Reader::from_str(opf_xml);
    reader.config_mut().trim_text(true);

    // manifest: (id, href, media_type, properties)
    let mut manifest: Vec<(String, String, String, String)> = Vec::new();
    let mut spine: Vec<String> = Vec::new();
    let mut spine_toc: Option<String> = None;
    let mut meta = Metadata::default();
    // Which Dublin Core element's text we're currently collecting.
    let mut field: Option<&'static str> = None;

    let push_item = |manifest: &mut Vec<(String, String, String, String)>,
                     e: &quick_xml::events::BytesStart| {
        if let (Some(id), Some(href)) = (attr(e, "id"), attr(e, "href")) {
            let mt = attr(e, "media-type").unwrap_or_default();
            let props = attr(e, "properties").unwrap_or_default();
            manifest.push((id, href, mt, props));
        }
    };

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let ln = local_name(e.name().as_ref()).to_vec();
                match ln.as_slice() {
                    b"title" => field = Some("title"),
                    b"creator" => field = Some("creator"),
                    b"language" => field = Some("language"),
                    b"publisher" => field = Some("publisher"),
                    b"date" => field = Some("date"),
                    b"item" => push_item(&mut manifest, &e),
                    b"itemref" => {
                        if let Some(idref) = attr(&e, "idref") {
                            spine.push(idref);
                        }
                    }
                    b"spine" => spine_toc = attr(&e, "toc"),
                    _ => {}
                }
            }
            Ok(Event::Empty(e)) => {
                let ln = local_name(e.name().as_ref()).to_vec();
                match ln.as_slice() {
                    b"item" => push_item(&mut manifest, &e),
                    b"itemref" => {
                        if let Some(idref) = attr(&e, "idref") {
                            spine.push(idref);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(t)) => {
                if let Some(f) = field {
                    let s = t.decode().map(|c| c.into_owned()).unwrap_or_default();
                    let s = s.trim();
                    if !s.is_empty() {
                        let slot = match f {
                            "title" => &mut meta.title,
                            "creator" => &mut meta.author,
                            "language" => &mut meta.language,
                            "publisher" => &mut meta.publisher,
                            "date" => &mut meta.date,
                            _ => unreachable!(),
                        };
                        // Keep the FIRST value for each field.
                        if slot.is_none() {
                            *slot = Some(collapse_ws(s));
                        }
                    }
                }
            }
            Ok(Event::End(e)) => {
                let name = e.name();
                let ln = local_name(name.as_ref());
                if matches!(
                    ln,
                    b"title" | b"creator" | b"language" | b"publisher" | b"date"
                ) {
                    field = None;
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }

    // Reading order: spine idrefs → manifest hrefs, keeping only (x)html docs.
    let spine_hrefs = spine
        .iter()
        .filter_map(|idref| {
            manifest
                .iter()
                .find(|(id, _, _, _)| id == idref)
                .filter(|(_, _, mt, _)| mt.contains("html"))
                .map(|(_, href, _, _)| join_href(opf_path, href))
        })
        .collect();

    // NCX: spine toc="<id>" → manifest href, else the dtbncx media-type item.
    let ncx_href = spine_toc
        .as_ref()
        .and_then(|toc_id| manifest.iter().find(|(id, _, _, _)| id == toc_id))
        .or_else(|| {
            manifest
                .iter()
                .find(|(_, _, mt, _)| mt.contains("dtbncx"))
        })
        .map(|(_, href, _, _)| join_href(opf_path, href));

    // EPUB3 nav: manifest item with properties containing "nav".
    let nav_href = manifest
        .iter()
        .find(|(_, _, _, props)| props.split_whitespace().any(|p| p == "nav"))
        .map(|(_, href, _, _)| join_href(opf_path, href));

    Opf {
        metadata: meta,
        spine_hrefs,
        ncx_href,
        nav_href,
    }
}

/// Parse an NCX table of contents → (href-without-fragment, title) pairs, in
/// document order. Titles come from each `navPoint`'s `navLabel > text`.
fn parse_ncx(xml: &str, ncx_path: &str) -> Vec<(String, String)> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut out = Vec::new();
    let mut in_navlabel = false;
    let mut in_text = false;
    let mut cur_label = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let ln = local_name(e.name().as_ref()).to_vec();
                match ln.as_slice() {
                    b"navLabel" => {
                        in_navlabel = true;
                        cur_label.clear();
                    }
                    b"text" if in_navlabel => in_text = true,
                    b"content" => {
                        if let Some(src) = attr(&e, "src") {
                            let t = collapse_ws(&cur_label);
                            if !t.is_empty() {
                                out.push((join_href(ncx_path, strip_fragment(&src)), t));
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(e)) => {
                if local_name(e.name().as_ref()) == b"content" {
                    if let Some(src) = attr(&e, "src") {
                        let t = collapse_ws(&cur_label);
                        if !t.is_empty() {
                            out.push((join_href(ncx_path, strip_fragment(&src)), t));
                        }
                    }
                }
            }
            Ok(Event::Text(t)) => {
                if in_text {
                    if let Ok(s) = t.decode() {
                        cur_label.push_str(&s);
                    }
                }
            }
            Ok(Event::End(e)) => match local_name(e.name().as_ref()) {
                b"text" => in_text = false,
                b"navLabel" => in_navlabel = false,
                _ => {}
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    out
}

/// Parse an EPUB3 nav document → (href-without-fragment, anchor-text) pairs from
/// its `<a href>` links, in document order.
fn parse_nav(xml: &str, nav_path: &str) -> Vec<(String, String)> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut out = Vec::new();
    let mut cur_href: Option<String> = None;
    let mut cur_text = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                if local_name(e.name().as_ref()) == b"a" {
                    cur_href = attr(&e, "href");
                    cur_text.clear();
                }
            }
            Ok(Event::Text(t)) => {
                if cur_href.is_some() {
                    if let Ok(s) = t.decode() {
                        cur_text.push_str(&s);
                    }
                }
            }
            Ok(Event::End(e)) => {
                if local_name(e.name().as_ref()) == b"a" {
                    if let Some(href) = cur_href.take() {
                        let t = collapse_ws(&cur_text);
                        if !t.is_empty() {
                            out.push((join_href(nav_path, strip_fragment(&href)), t));
                        }
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    out
}

/// First heading (`<h1>`..`<h6>`) text in an XHTML document, if any.
fn first_heading(html: &str) -> Option<String> {
    let mut reader = Reader::from_str(html);
    reader.config_mut().trim_text(true);
    let mut tag: Option<Vec<u8>> = None;
    let mut buf = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = e.name();
                let ln = local_name(name.as_ref());
                if tag.is_none() && is_heading(ln) {
                    tag = Some(ln.to_vec());
                }
            }
            Ok(Event::Text(t)) => {
                if tag.is_some() {
                    if let Ok(s) = t.decode() {
                        buf.push_str(&s);
                    }
                }
            }
            Ok(Event::End(e)) => {
                if let Some(ref want) = tag {
                    if local_name(e.name().as_ref()) == want.as_slice() {
                        let s = collapse_ws(&buf);
                        return if s.is_empty() { None } else { Some(s) };
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => return None,
            _ => {}
        }
    }
}

/// `<title>` element text in an XHTML document's head, if any.
fn title_element(html: &str) -> Option<String> {
    let mut reader = Reader::from_str(html);
    reader.config_mut().trim_text(true);
    let mut in_title = false;
    let mut buf = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                if local_name(e.name().as_ref()) == b"title" {
                    in_title = true;
                }
            }
            Ok(Event::Text(t)) => {
                if in_title {
                    if let Ok(s) = t.decode() {
                        buf.push_str(&s);
                    }
                }
            }
            Ok(Event::End(e)) => {
                if local_name(e.name().as_ref()) == b"title" {
                    let s = collapse_ws(&buf);
                    return if s.is_empty() { None } else { Some(s) };
                }
            }
            Ok(Event::Eof) | Err(_) => return None,
            _ => {}
        }
    }
}

/// Extract an EPUB into structured chapters + metadata.
pub fn extract(bytes: &[u8]) -> Result<Ebook, String> {
    if bytes.is_empty() {
        return Err("input is empty".into());
    }
    let mut zip = ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| format!("not a valid EPUB/ZIP container: {e}"))?;

    let container = read_zip_entry(&mut zip, "META-INF/container.xml");
    let opf_path = container
        .as_deref()
        .and_then(find_opf_path)
        // Fallback: first .opf file in the archive.
        .or_else(|| {
            (0..zip.len()).find_map(|i| {
                let name = zip.by_index(i).ok()?.name().to_string();
                name.to_ascii_lowercase().ends_with(".opf").then_some(name)
            })
        })
        .ok_or("could not locate the EPUB OPF package file")?;

    let opf_xml = read_zip_entry(&mut zip, &opf_path)
        .ok_or_else(|| format!("could not read OPF at '{opf_path}'"))?;
    let opf = parse_opf(&opf_xml, &opf_path);

    if opf.spine_hrefs.is_empty() {
        return Err("EPUB spine contained no readable chapters".into());
    }

    // Build the TOC title map (NCX first, then nav; first title per href wins).
    let mut toc: Vec<(String, String)> = Vec::new();
    if let Some(ncx) = &opf.ncx_href {
        if let Some(xml) = read_zip_entry(&mut zip, ncx) {
            toc.extend(parse_ncx(&xml, ncx));
        }
    }
    if let Some(nav) = &opf.nav_href {
        if let Some(xml) = read_zip_entry(&mut zip, nav) {
            toc.extend(parse_nav(&xml, nav));
        }
    }
    let toc_title = |href: &str| -> Option<String> {
        toc.iter().find(|(h, _)| h == href).map(|(_, t)| t.clone())
    };

    let mut chapters: Vec<Chapter> = Vec::new();
    for href in &opf.spine_hrefs {
        let Some(html) = read_zip_entry(&mut zip, href) else {
            continue;
        };
        let text = nanohtml2text::html2text(&html);
        let text = text.trim().to_string();
        if text.is_empty() {
            continue;
        }
        let idx = chapters.len() + 1;
        let title = toc_title(href)
            .or_else(|| first_heading(&html))
            .or_else(|| title_element(&html))
            .unwrap_or_else(|| format!("Chapter {idx}"));
        let words = text.split_whitespace().count();
        chapters.push(Chapter {
            index: idx,
            title,
            words,
            text,
        });
    }

    if chapters.is_empty() {
        return Err("no chapter text could be extracted from the EPUB".into());
    }

    Ok(Ebook {
        metadata: opf.metadata,
        chapters,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_and_helpers() {
        assert_eq!(join_href("OEBPS/toc.ncx", "ch1.xhtml"), "OEBPS/ch1.xhtml");
        assert_eq!(join_href("content.opf", "ch1.xhtml"), "ch1.xhtml");
        assert_eq!(join_href("OEBPS/toc.ncx", "../images/x"), "images/x");
        assert_eq!(normalize("a/./b/../c"), "a/c");
        assert_eq!(strip_fragment("ch1.xhtml#part2"), "ch1.xhtml");
        assert_eq!(collapse_ws("  a\n  b\t c "), "a b c");
    }

    #[test]
    fn first_heading_and_title() {
        assert_eq!(
            first_heading("<html><body><h2>The  Start</h2><p>x</p></body></html>").as_deref(),
            Some("The Start")
        );
        assert_eq!(first_heading("<html><body><p>no heading</p></body></html>"), None);
        assert_eq!(
            title_element("<html><head><title>Doc Title</title></head><body/></html>").as_deref(),
            Some("Doc Title")
        );
    }

    #[test]
    fn parses_opf_metadata_and_spine() {
        let opf = r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf">
          <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
            <dc:title>My Book</dc:title>
            <dc:creator>Ada Lovelace</dc:creator>
            <dc:language>en</dc:language>
            <dc:publisher>Acme Press</dc:publisher>
            <dc:date>1843</dc:date>
          </metadata>
          <manifest>
            <item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/>
            <item id="c1" href="ch1.xhtml" media-type="application/xhtml+xml"/>
            <item id="css" href="s.css" media-type="text/css"/>
            <item id="c2" href="ch2.xhtml" media-type="application/xhtml+xml"/>
          </manifest>
          <spine toc="ncx"><itemref idref="c1"/><itemref idref="c2"/></spine>
        </package>"#;
        let parsed = parse_opf(opf, "OEBPS/content.opf");
        assert_eq!(parsed.metadata.title.as_deref(), Some("My Book"));
        assert_eq!(parsed.metadata.author.as_deref(), Some("Ada Lovelace"));
        assert_eq!(parsed.metadata.language.as_deref(), Some("en"));
        assert_eq!(parsed.metadata.publisher.as_deref(), Some("Acme Press"));
        assert_eq!(parsed.metadata.date.as_deref(), Some("1843"));
        assert_eq!(parsed.spine_hrefs, vec!["OEBPS/ch1.xhtml", "OEBPS/ch2.xhtml"]);
        assert_eq!(parsed.ncx_href.as_deref(), Some("OEBPS/toc.ncx"));
    }

    #[test]
    fn parses_ncx_titles() {
        let ncx = r#"<?xml version="1.0"?><ncx><navMap>
            <navPoint><navLabel><text>Introduction</text></navLabel><content src="ch1.xhtml"/></navPoint>
            <navPoint><navLabel><text>Chapter One</text></navLabel><content src="ch2.xhtml#top"/></navPoint>
          </navMap></ncx>"#;
        let toc = parse_ncx(ncx, "OEBPS/toc.ncx");
        assert_eq!(
            toc,
            vec![
                ("OEBPS/ch1.xhtml".to_string(), "Introduction".to_string()),
                ("OEBPS/ch2.xhtml".to_string(), "Chapter One".to_string()),
            ]
        );
    }

    #[test]
    fn extracts_structured_chapters() {
        let bytes = build_epub();
        let book = extract(&bytes).unwrap();
        assert_eq!(book.metadata.title.as_deref(), Some("Test Book"));
        assert_eq!(book.metadata.author.as_deref(), Some("Jane Roe"));
        assert_eq!(book.chapters.len(), 2);

        // Titles come from the NCX, not the headings.
        assert_eq!(book.chapters[0].index, 1);
        assert_eq!(book.chapters[0].title, "Introduction");
        assert!(book.chapters[0].text.contains("The first chapter"));
        assert!(!book.chapters[0].text.contains("<h1>"));
        assert!(book.chapters[0].words > 0);

        assert_eq!(book.chapters[1].index, 2);
        assert_eq!(book.chapters[1].title, "The Second Bit");
    }

    #[test]
    fn heading_fallback_when_no_toc() {
        // No NCX / nav → titles fall back to the first heading.
        let bytes = build_epub_no_toc();
        let book = extract(&bytes).unwrap();
        assert_eq!(book.chapters.len(), 1);
        assert_eq!(book.chapters[0].title, "Only Heading");
    }

    #[test]
    fn errors() {
        assert!(extract(b"").is_err());
        assert!(extract(b"not a zip").is_err());
    }

    fn zip_bytes(files: &[(&str, &str)]) -> Vec<u8> {
        use std::io::Write;
        use zip::write::SimpleFileOptions;
        let mut buf = Vec::new();
        {
            let mut w = zip::ZipWriter::new(Cursor::new(&mut buf));
            let opts = SimpleFileOptions::default();
            for (name, data) in files {
                w.start_file(*name, opts).unwrap();
                w.write_all(data.as_bytes()).unwrap();
            }
            w.finish().unwrap();
        }
        buf
    }

    fn build_epub() -> Vec<u8> {
        zip_bytes(&[
            ("META-INF/container.xml", r#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#),
            ("OEBPS/content.opf", r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Test Book</dc:title><dc:creator>Jane Roe</dc:creator></metadata><manifest><item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/><item id="c1" href="ch1.xhtml" media-type="application/xhtml+xml"/><item id="c2" href="ch2.xhtml" media-type="application/xhtml+xml"/></manifest><spine toc="ncx"><itemref idref="c1"/><itemref idref="c2"/></spine></package>"#),
            ("OEBPS/toc.ncx", r#"<?xml version="1.0"?><ncx><navMap><navPoint><navLabel><text>Introduction</text></navLabel><content src="ch1.xhtml"/></navPoint><navPoint><navLabel><text>The Second Bit</text></navLabel><content src="ch2.xhtml"/></navPoint></navMap></ncx>"#),
            ("OEBPS/ch1.xhtml", r#"<html><body><h1>Chapter One</h1><p>The first chapter has some words.</p></body></html>"#),
            ("OEBPS/ch2.xhtml", r#"<html><body><h1>Chapter Two</h1><p>The second chapter is here.</p></body></html>"#),
        ])
    }

    fn build_epub_no_toc() -> Vec<u8> {
        zip_bytes(&[
            ("META-INF/container.xml", r#"<?xml version="1.0"?><container><rootfiles><rootfile full-path="content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#),
            ("content.opf", r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>No TOC</dc:title></metadata><manifest><item id="c1" href="only.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="c1"/></spine></package>"#),
            ("only.xhtml", r#"<html><head><title>ignored head title</title></head><body><h3>Only Heading</h3><p>Body text here.</p></body></html>"#),
        ])
    }
}

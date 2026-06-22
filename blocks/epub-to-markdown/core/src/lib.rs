//! gizza-ai/epub-to-markdown core — extract an EPUB's chapters into clean
//! Markdown (or plain text). An EPUB is a ZIP of XHTML; we read the OPF manifest
//! + spine for reading order and convert each chapter. Pure-Rust (`zip`,
//! `quick-xml`, `htmd`, `nanohtml2text`).

use std::io::{Cursor, Read};

use quick_xml::events::Event;
use quick_xml::Reader;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Markdown,
    Text,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Mode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "markdown" | "md" | "" => Ok(Mode::Markdown),
            "text" | "txt" | "plain" => Ok(Mode::Text),
            other => Err(format!("unknown format '{other}' (use markdown or text)")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Book {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub chapters: usize,
    pub content: String,
}

/// Local name of an element, dropping any namespace prefix.
fn local_name<'a>(raw: &'a [u8]) -> &'a [u8] {
    match raw.iter().position(|&b| b == b':') {
        Some(i) => &raw[i + 1..],
        None => raw,
    }
}

fn attr<'a>(e: &'a quick_xml::events::BytesStart, key: &str) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        if local_name(a.key.as_ref()) == key.as_bytes() {
            Some(String::from_utf8_lossy(&a.value).into_owned())
        } else {
            None
        }
    })
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

/// Join an OPF-relative href to the OPF's directory.
fn join_href(opf_path: &str, href: &str) -> String {
    let dir = match opf_path.rfind('/') {
        Some(i) => &opf_path[..i],
        None => "",
    };
    let combined = if dir.is_empty() {
        href.to_string()
    } else {
        format!("{dir}/{href}")
    };
    normalize(&combined)
}

fn read_zip_entry(zip: &mut zip::ZipArchive<Cursor<&[u8]>>, name: &str) -> Option<String> {
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
            Ok(Event::Eof) => return None,
            Err(_) => return None,
            _ => {}
        }
    }
}

struct Opf {
    title: Option<String>,
    /// spine order of hrefs (already joined to the OPF dir).
    hrefs: Vec<String>,
}

/// Parse the OPF: manifest (id→href, media-type) + spine (idref order) + title.
fn parse_opf(opf_xml: &str, opf_path: &str) -> Opf {
    let mut reader = Reader::from_str(opf_xml);
    reader.config_mut().trim_text(true);

    // id -> (href, media_type)
    let mut manifest: Vec<(String, String, String)> = Vec::new();
    let mut spine: Vec<String> = Vec::new();
    let mut title: Option<String> = None;
    let mut in_title = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let ln = local_name(e.name().as_ref()).to_vec();
                if ln == b"title" {
                    in_title = true;
                } else if ln == b"item" {
                    if let (Some(id), Some(href)) = (attr(&e, "id"), attr(&e, "href")) {
                        let mt = attr(&e, "media-type").unwrap_or_default();
                        manifest.push((id, href, mt));
                    }
                } else if ln == b"itemref" {
                    if let Some(idref) = attr(&e, "idref") {
                        spine.push(idref);
                    }
                }
            }
            Ok(Event::Empty(e)) => {
                let ln = local_name(e.name().as_ref()).to_vec();
                if ln == b"item" {
                    if let (Some(id), Some(href)) = (attr(&e, "id"), attr(&e, "href")) {
                        let mt = attr(&e, "media-type").unwrap_or_default();
                        manifest.push((id, href, mt));
                    }
                } else if ln == b"itemref" {
                    if let Some(idref) = attr(&e, "idref") {
                        spine.push(idref);
                    }
                }
            }
            Ok(Event::Text(t)) => {
                if in_title && title.is_none() {
                    let s = t.decode().map(|c| c.into_owned()).unwrap_or_default();
                    if !s.trim().is_empty() {
                        title = Some(s.trim().to_string());
                    }
                }
            }
            Ok(Event::End(e)) => {
                if local_name(e.name().as_ref()) == b"title" {
                    in_title = false;
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }

    // Map spine idrefs → hrefs in order; only keep (x)html documents.
    let hrefs = spine
        .iter()
        .filter_map(|idref| {
            manifest
                .iter()
                .find(|(id, _, _)| id == idref)
                .filter(|(_, _, mt)| mt.contains("html"))
                .map(|(_, href, _)| join_href(opf_path, href))
        })
        .collect();

    Opf { title, hrefs }
}

/// Extract an EPUB into Markdown or plain text.
pub fn convert(bytes: &[u8], mode: Mode) -> Result<Book, String> {
    if bytes.is_empty() {
        return Err("input is empty".into());
    }
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes))
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

    if opf.hrefs.is_empty() {
        return Err("EPUB spine contained no readable chapters".into());
    }

    let sep = match mode {
        Mode::Markdown => "\n\n---\n\n",
        Mode::Text => "\n\n",
    };
    let mut parts: Vec<String> = Vec::new();
    let mut chapters = 0;
    for href in &opf.hrefs {
        let Some(html) = read_zip_entry(&mut zip, href) else {
            continue;
        };
        let converted = match mode {
            Mode::Markdown => htmd::convert(&html).unwrap_or_default(),
            Mode::Text => nanohtml2text::html2text(&html),
        };
        let trimmed = converted.trim();
        if !trimmed.is_empty() {
            parts.push(trimmed.to_string());
            chapters += 1;
        }
    }

    if chapters == 0 {
        return Err("no chapter text could be extracted from the EPUB".into());
    }

    Ok(Book {
        title: opf.title,
        chapters,
        content: parts.join(sep),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_helpers() {
        assert_eq!(join_href("OEBPS/content.opf", "ch1.xhtml"), "OEBPS/ch1.xhtml");
        assert_eq!(join_href("content.opf", "ch1.xhtml"), "ch1.xhtml");
        assert_eq!(join_href("OEBPS/content.opf", "../images/x"), "images/x");
        assert_eq!(normalize("a/./b/../c"), "a/c");
    }

    #[test]
    fn finds_opf_and_parses_spine() {
        let container = r#"<?xml version="1.0"?><container><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#;
        assert_eq!(find_opf_path(container).as_deref(), Some("OEBPS/content.opf"));

        let opf = r#"<?xml version="1.0"?><package><metadata><dc:title>My Book</dc:title></metadata><manifest><item id="c1" href="ch1.xhtml" media-type="application/xhtml+xml"/><item id="css" href="s.css" media-type="text/css"/><item id="c2" href="ch2.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="c1"/><itemref idref="c2"/></spine></package>"#;
        let parsed = parse_opf(opf, "OEBPS/content.opf");
        assert_eq!(parsed.title.as_deref(), Some("My Book"));
        assert_eq!(parsed.hrefs, vec!["OEBPS/ch1.xhtml", "OEBPS/ch2.xhtml"]);
    }

    #[test]
    fn converts_a_minimal_epub() {
        let bytes = build_epub();
        let book = convert(&bytes, Mode::Markdown).unwrap();
        assert_eq!(book.title.as_deref(), Some("Test Book"));
        assert_eq!(book.chapters, 2);
        assert!(book.content.contains("Chapter One"));
        assert!(book.content.contains("Chapter Two"));
        assert!(book.content.contains("---")); // chapter separator

        let txt = convert(&bytes, Mode::Text).unwrap();
        assert!(txt.content.contains("Chapter One"));
        assert!(!txt.content.contains("<h1>"));
    }

    #[test]
    fn errors() {
        assert!(convert(b"", Mode::Markdown).is_err());
        assert!(convert(b"not a zip", Mode::Markdown).is_err());
        assert!(Mode::parse("pdf").is_err());
    }

    /// Build a minimal valid EPUB in-memory for the round-trip test.
    fn build_epub() -> Vec<u8> {
        use std::io::Write;
        use zip::write::SimpleFileOptions;
        let mut buf = Vec::new();
        {
            let mut w = zip::ZipWriter::new(Cursor::new(&mut buf));
            let opts = SimpleFileOptions::default();
            let add = |w: &mut zip::ZipWriter<Cursor<&mut Vec<u8>>>, name: &str, data: &str| {
                w.start_file(name, opts).unwrap();
                w.write_all(data.as_bytes()).unwrap();
            };
            add(&mut w, "META-INF/container.xml", r#"<?xml version="1.0"?><container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>"#);
            add(&mut w, "OEBPS/content.opf", r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Test Book</dc:title></metadata><manifest><item id="c1" href="ch1.xhtml" media-type="application/xhtml+xml"/><item id="c2" href="ch2.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="c1"/><itemref idref="c2"/></spine></package>"#);
            add(&mut w, "OEBPS/ch1.xhtml", r#"<html><body><h1>Chapter One</h1><p>The first chapter.</p></body></html>"#);
            add(&mut w, "OEBPS/ch2.xhtml", r#"<html><body><h1>Chapter Two</h1><p>The second chapter.</p></body></html>"#);
            w.finish().unwrap();
        }
        buf
    }
}

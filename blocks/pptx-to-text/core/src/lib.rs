//! pptx-to-text core — extract text and a per-slide outline from a modern
//! PowerPoint `.pptx` file (Office Open XML PresentationML).

use quick_xml::events::Event;
use quick_xml::Reader;
use serde::Serialize;
use std::collections::HashMap;
use std::io::{Cursor, Read};
use zip::ZipArchive;

pub const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_TEXT_CHARS: usize = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotesMode {
    Include,
    Exclude,
    Only,
}

impl NotesMode {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "include" => Ok(Self::Include),
            "exclude" => Ok(Self::Exclude),
            "only" => Ok(Self::Only),
            other => Err(format!(
                "invalid notes mode {other:?}: use include, exclude, or only"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhitespaceMode {
    Clean,
    Raw,
}

impl WhitespaceMode {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "clean" => Ok(Self::Clean),
            "raw" => Ok(Self::Raw),
            other => Err(format!(
                "invalid whitespace mode {other:?}: use clean or raw"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Options {
    pub notes: NotesMode,
    pub whitespace: WhitespaceMode,
    pub include_hidden: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SlideOutline {
    pub number: usize,
    pub title: String,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    pub hidden: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExtractedPptx {
    pub text: String,
    pub slides: Vec<SlideOutline>,
    pub slide_count: usize,
    pub words: usize,
    pub paragraphs: usize,
    pub truncated: bool,
}

/// Extract `.pptx` text and return a JSON string for CLI snapshots.
pub fn run_json(bytes: &[u8], options: Options) -> Result<String, String> {
    let out = extract(bytes, options)?;
    serde_json::to_string_pretty(&out).map_err(|e| format!("serialize output: {e}"))
}

pub fn extract(bytes: &[u8], options: Options) -> Result<ExtractedPptx, String> {
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input document too large: {} bytes (cap {} bytes)",
            bytes.len(),
            MAX_INPUT_BYTES
        ));
    }

    let mut zip = ZipArchive::new(Cursor::new(bytes)).map_err(|e| {
        format!("not a readable .pptx file: expected a ZIP-based PowerPoint document ({e})")
    })?;

    let order = slide_paths(&mut zip)?;
    if order.is_empty() {
        return Err("no slides found in pptx (missing ppt/slides/slideN.xml)".into());
    }

    let mut slides = Vec::new();
    let mut combined_parts = Vec::new();
    let mut truncated = false;

    for (idx, path) in order.iter().enumerate() {
        let xml = read_zip_string(&mut zip, path)?;
        let parsed = parse_slide_xml(&xml, options.whitespace);
        if parsed.hidden && !options.include_hidden {
            continue;
        }
        let notes = match options.notes {
            NotesMode::Exclude => None,
            NotesMode::Include | NotesMode::Only => notes_path(&mut zip, path)
                .and_then(|p| read_zip_string(&mut zip, &p).ok())
                .map(|notes_xml| extract_text(&notes_xml, options.whitespace).join("\n"))
                .map(|s| clean_final(&s, options.whitespace))
                .filter(|s| !s.is_empty()),
        };
        let slide_text = match options.notes {
            NotesMode::Only => String::new(),
            NotesMode::Include | NotesMode::Exclude => parsed.text.clone(),
        };

        let mut full_piece = String::new();
        match options.notes {
            NotesMode::Only => {
                if let Some(n) = &notes {
                    full_piece.push_str(n);
                }
            }
            NotesMode::Exclude => full_piece.push_str(&slide_text),
            NotesMode::Include => {
                full_piece.push_str(&slide_text);
                if let Some(n) = &notes {
                    if !full_piece.is_empty() {
                        full_piece.push('\n');
                    }
                    full_piece.push_str(n);
                }
            }
        }
        if !full_piece.trim().is_empty() {
            combined_parts.push(format!("Slide {}\n{}", idx + 1, full_piece));
        }
        let mut title = parsed.title;
        if title.is_empty() {
            title = first_non_empty_line(&parsed.text).unwrap_or_default();
        }
        slides.push(SlideOutline {
            number: idx + 1,
            title,
            text: slide_text,
            notes: if options.notes == NotesMode::Exclude {
                None
            } else {
                notes
            },
            hidden: parsed.hidden,
        });
    }

    let mut text = combined_parts.join("\n\n");
    if text.chars().count() > MAX_TEXT_CHARS {
        text = text.chars().take(MAX_TEXT_CHARS).collect();
        truncated = true;
    }
    let words = text.split_whitespace().count();
    let paragraphs = text.split('\n').filter(|p| !p.trim().is_empty()).count();
    let slide_count = slides.len();
    Ok(ExtractedPptx {
        text,
        slides,
        slide_count,
        words,
        paragraphs,
        truncated,
    })
}

#[derive(Debug)]
struct ParsedSlide {
    title: String,
    text: String,
    hidden: bool,
}

fn slide_paths<R: Read + std::io::Seek>(zip: &mut ZipArchive<R>) -> Result<Vec<String>, String> {
    let rels = read_zip_string(zip, "ppt/_rels/presentation.xml.rels").unwrap_or_default();
    let rel_map = parse_relationships(&rels);
    let presentation = read_zip_string(zip, "ppt/presentation.xml").unwrap_or_default();
    let mut ordered = Vec::new();
    let mut reader = Reader::from_str(&presentation);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if local(e.name().as_ref()) == b"sldId" => {
                for a in e.attributes().flatten() {
                    if local(a.key.as_ref()) == b"id" {
                        let v = String::from_utf8_lossy(a.value.as_ref()).to_string();
                        if let Some(target) = rel_map.get(&v) {
                            ordered.push(resolve_part("ppt/presentation.xml", target));
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    if !ordered.is_empty() {
        return Ok(ordered);
    }

    let mut fallback = Vec::new();
    for i in 0..zip.len() {
        let name = zip
            .by_index(i)
            .map_err(|e| format!("read pptx zip entry: {e}"))?
            .name()
            .to_string();
        if name.starts_with("ppt/slides/slide") && name.ends_with(".xml") {
            fallback.push(name);
        }
    }
    fallback.sort_by_key(|p| numeric_tail(p).unwrap_or(usize::MAX));
    Ok(fallback)
}

fn notes_path<R: Read + std::io::Seek>(
    zip: &mut ZipArchive<R>,
    slide_path: &str,
) -> Option<String> {
    let file = slide_path.rsplit('/').next()?;
    let rels_path = format!("ppt/slides/_rels/{file}.rels");
    let rels = read_zip_string(zip, &rels_path).ok()?;
    for (_id, typ, target) in parse_relationships_full(&rels) {
        if typ.contains("/notesSlide") || target.contains("notesSlides/") {
            return Some(resolve_part(slide_path, &target));
        }
    }
    None
}

fn read_zip_string<R: Read + std::io::Seek>(
    zip: &mut ZipArchive<R>,
    path: &str,
) -> Result<String, String> {
    let mut f = zip
        .by_name(path)
        .map_err(|e| format!("missing pptx part {path}: {e}"))?;
    let mut s = String::new();
    f.read_to_string(&mut s)
        .map_err(|e| format!("read pptx part {path}: {e}"))?;
    Ok(s)
}

fn parse_slide_xml(xml: &str, ws: WhitespaceMode) -> ParsedSlide {
    let hidden = xml.contains("show=\"0\"") || xml.contains("show='0'");
    let paragraphs = extract_text(xml, ws);
    let text = clean_final(&paragraphs.join("\n"), ws);
    let title = extract_title(xml, ws)
        .or_else(|| first_non_empty_line(&text))
        .unwrap_or_default();
    ParsedSlide {
        title,
        text,
        hidden,
    }
}

fn extract_text(xml: &str, ws: WhitespaceMode) -> Vec<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut paras = Vec::<String>::new();
    let mut cur = String::new();
    let mut in_para = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"p" => {
                if in_para && !cur.trim().is_empty() {
                    paras.push(clean_final(&cur, ws));
                    cur.clear();
                }
                in_para = true;
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"p" => {
                if !cur.trim().is_empty() || ws == WhitespaceMode::Raw {
                    let c = clean_final(&cur, ws);
                    if !c.is_empty() || ws == WhitespaceMode::Raw {
                        paras.push(c);
                    }
                }
                cur.clear();
                in_para = false;
            }
            Ok(Event::Empty(e)) if local(e.name().as_ref()) == b"br" => cur.push('\n'),
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"t" => {
                if let Ok(t) = reader.read_text(e.name()) {
                    if let Ok(decoded) = t.decode() {
                        cur.push_str(&decoded);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    if !cur.trim().is_empty() {
        paras.push(clean_final(&cur, ws));
    }
    paras
        .into_iter()
        .filter(|p| ws == WhitespaceMode::Raw || !p.is_empty())
        .collect()
}

fn extract_title(xml: &str, ws: WhitespaceMode) -> Option<String> {
    // Pragmatic shape-level scan: split on shape starts, keep text from shapes
    // containing a title/center-title placeholder.
    for chunk in xml.split("<p:sp").skip(1) {
        let head = chunk.split("</p:sp>").next().unwrap_or(chunk);
        if head.contains("type=\"title\"") || head.contains("type=\"ctrTitle\"") {
            let txt = extract_text(head, ws).join(" ");
            let txt = clean_final(&txt, ws);
            if !txt.is_empty() {
                return Some(txt);
            }
        }
    }
    None
}

fn parse_relationships(xml: &str) -> HashMap<String, String> {
    parse_relationships_full(xml)
        .into_iter()
        .map(|(id, _typ, target)| (id, target))
        .collect()
}

fn parse_relationships_full(xml: &str) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local(e.name().as_ref()) == b"Relationship" =>
            {
                let mut id = String::new();
                let mut typ = String::new();
                let mut target = String::new();
                for a in e.attributes().flatten() {
                    match local(a.key.as_ref()) {
                        b"Id" => id = String::from_utf8_lossy(a.value.as_ref()).to_string(),
                        b"Type" => typ = String::from_utf8_lossy(a.value.as_ref()).to_string(),
                        b"Target" => target = String::from_utf8_lossy(a.value.as_ref()).to_string(),
                        _ => {}
                    }
                }
                if !id.is_empty() && !target.is_empty() {
                    out.push((id, typ, target));
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    out
}

fn resolve_part(base: &str, target: &str) -> String {
    if target.starts_with('/') {
        return target.trim_start_matches('/').to_string();
    }
    let mut parts: Vec<&str> = base.split('/').collect();
    parts.pop();
    for seg in target.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

fn local(name: &[u8]) -> &[u8] {
    name.rsplit(|b| *b == b':').next().unwrap_or(name)
}

fn numeric_tail(path: &str) -> Option<usize> {
    let file = path.rsplit('/').next()?;
    let n = file.strip_prefix("slide")?.strip_suffix(".xml")?;
    n.parse().ok()
}

fn first_non_empty_line(s: &str) -> Option<String> {
    s.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(|s| s.to_string())
}

fn clean_final(s: &str, ws: WhitespaceMode) -> String {
    match ws {
        WhitespaceMode::Raw => s.trim_matches('\n').to_string(),
        WhitespaceMode::Clean => s.split_whitespace().collect::<Vec<_>>().join(" "),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    fn add(w: &mut zip::ZipWriter<Cursor<Vec<u8>>>, path: &str, body: &str) {
        w.start_file(path, SimpleFileOptions::default()).unwrap();
        w.write_all(body.as_bytes()).unwrap();
    }

    fn fixture() -> Vec<u8> {
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        add(
            &mut w,
            "ppt/presentation.xml",
            r#"<p:presentation xmlns:p="p" xmlns:r="r"><p:sldIdLst><p:sldId r:id="rId2"/><p:sldId r:id="rId1"/></p:sldIdLst></p:presentation>"#,
        );
        add(
            &mut w,
            "ppt/_rels/presentation.xml.rels",
            r#"<Relationships><Relationship Id="rId1" Type="slide" Target="slides/slide1.xml"/><Relationship Id="rId2" Type="slide" Target="slides/slide2.xml"/></Relationships>"#,
        );
        add(
            &mut w,
            "ppt/slides/slide1.xml",
            r#"<p:sld xmlns:p="p" xmlns:a="a"><p:cSld><p:spTree><p:sp><p:nvSpPr><p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr><p:txBody><a:p><a:r><a:t>First Title</a:t></a:r></a:p></p:txBody></p:sp><p:sp><p:txBody><a:p><a:r><a:t>First body</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:sld>"#,
        );
        add(
            &mut w,
            "ppt/slides/slide2.xml",
            r#"<p:sld xmlns:p="p" xmlns:a="a" show="0"><p:cSld><p:spTree><p:sp><p:nvSpPr><p:nvPr><p:ph type="ctrTitle"/></p:nvPr></p:nvSpPr><p:txBody><a:p><a:r><a:t>Hidden Title</a:t></a:r></a:p></p:txBody></p:sp><p:sp><p:txBody><a:p><a:r><a:t>Hidden body</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:sld>"#,
        );
        add(
            &mut w,
            "ppt/slides/_rels/slide2.xml.rels",
            r#"<Relationships><Relationship Id="rIdNotes" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesSlide" Target="../notesSlides/notesSlide2.xml"/></Relationships>"#,
        );
        add(
            &mut w,
            "ppt/notesSlides/notesSlide2.xml",
            r#"<p:notes xmlns:p="p" xmlns:a="a"><p:cSld><p:spTree><p:sp><p:txBody><a:p><a:r><a:t>Speaker note</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:notes>"#,
        );
        w.finish().unwrap().into_inner()
    }

    #[test]
    fn extracts_in_presentation_order_with_notes_and_hidden() {
        let out = extract(
            &fixture(),
            Options {
                notes: NotesMode::Include,
                whitespace: WhitespaceMode::Clean,
                include_hidden: true,
            },
        )
        .unwrap();
        assert_eq!(out.slide_count, 2);
        assert_eq!(out.slides[0].number, 1);
        assert_eq!(out.slides[0].title, "Hidden Title");
        assert!(out.slides[0].hidden);
        assert_eq!(out.slides[0].notes.as_deref(), Some("Speaker note"));
        assert!(out.text.contains("Slide 1"), "{}", out.text);
        assert!(out.text.contains("Hidden Title"), "{}", out.text);
        assert!(out.text.contains("Hidden body"), "{}", out.text);
        assert!(out.text.contains("Speaker note"), "{}", out.text);
        assert_eq!(out.slides[1].title, "First Title");
    }

    #[test]
    fn can_exclude_hidden_and_notes() {
        let out = extract(
            &fixture(),
            Options {
                notes: NotesMode::Exclude,
                whitespace: WhitespaceMode::Clean,
                include_hidden: false,
            },
        )
        .unwrap();
        assert_eq!(out.slide_count, 1);
        assert_eq!(out.slides[0].title, "First Title");
        assert!(out.slides[0].notes.is_none());
        assert!(!out.text.contains("Speaker note"));
    }

    #[test]
    fn notes_only_uses_notes_text() {
        let out = extract(
            &fixture(),
            Options {
                notes: NotesMode::Only,
                whitespace: WhitespaceMode::Clean,
                include_hidden: true,
            },
        )
        .unwrap();
        assert!(out.text.contains("Speaker note"));
        assert!(!out.text.contains("First body"));
    }

    #[test]
    fn rejects_non_zip() {
        let err = extract(
            b"not zip",
            Options {
                notes: NotesMode::Include,
                whitespace: WhitespaceMode::Clean,
                include_hidden: true,
            },
        )
        .unwrap_err();
        assert!(err.contains("not a readable .pptx"), "{err}");
    }

    #[test]
    fn emits_json() {
        let json = run_json(
            &fixture(),
            Options {
                notes: NotesMode::Include,
                whitespace: WhitespaceMode::Clean,
                include_hidden: true,
            },
        )
        .unwrap();
        assert!(json.contains("\"slide_count\": 2"));
    }
}

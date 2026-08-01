//! video-chapter-export core — parse embedded chapter markers out of a video
//! container and render them to CSV / JSON / CUE / plain-text.
//!
//! Pure Rust, no I/O — the caller (chat skill block or CLI) hands over the raw
//! container bytes and gets a text export back. Two container families are
//! understood, both parsed by hand (no ffmpeg, no heavy demux crate):
//!
//!   * **MP4 / M4V / M4A / MOV** (ISO-BMFF): the Nero-style `chpl` chapter-list
//!     atom in `moov/udta`, plus the movie `mvhd` duration to close the final
//!     chapter. `chpl` start times are in 100-nanosecond units.
//!   * **Matroska / WebM** (EBML): `Segment → Chapters → EditionEntry →
//!     ChapterAtom`, reading `ChapterTimeStart`/`ChapterTimeEnd` (nanoseconds)
//!     and the `ChapString` title inside `ChapterDisplay`.
//!
//! All timestamps are normalized to whole milliseconds internally so every
//! output format renders from one source of truth.

use serde::Serialize;

/// One parsed chapter marker. `end_ms` is `None` only when the container gives
/// no end and there is no following chapter / movie duration to derive one.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Chapter {
    /// 1-based position in the file.
    pub index: usize,
    pub title: String,
    pub start_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_ms: Option<u64>,
}

/// The full result of an export call.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChapterExport {
    /// `"mp4"` or `"matroska"`.
    pub container: String,
    /// The requested output format (`csv`|`json`|`cue`|`text`).
    pub format: String,
    pub chapter_count: usize,
    /// The rendered file body in the requested format.
    pub content: String,
}

/// The output formats this tool can render.
pub const FORMATS: [&str; 4] = ["csv", "json", "cue", "text"];

/// Parse the container bytes and render the chapters in `format`.
///
/// `filename` is a display hint used in the CUE `FILE`/`TITLE` lines (pass the
/// source filename, or `"video"` if unknown).
pub fn export_chapters(bytes: &[u8], format: &str, filename: &str) -> Result<ChapterExport, String> {
    let format = format.trim().to_ascii_lowercase();
    if !FORMATS.contains(&format.as_str()) {
        return Err(format!(
            "unknown format {format:?} — use one of csv, json, cue, text"
        ));
    }
    let (container, chapters) = parse(bytes)?;
    let name = clean_filename(filename);
    let content = match format.as_str() {
        "csv" => render_csv(&chapters),
        "json" => render_json(&chapters),
        "cue" => render_cue(&chapters, &name),
        "text" => render_text(&chapters),
        _ => unreachable!(),
    };
    Ok(ChapterExport {
        container: container.to_string(),
        format,
        chapter_count: chapters.len(),
        content,
    })
}

/// Detect the container and return `(container_label, chapters)`.
pub fn parse(bytes: &[u8]) -> Result<(&'static str, Vec<Chapter>), String> {
    if bytes.len() >= 8 && &bytes[4..8] == b"ftyp" {
        Ok(("mp4", parse_mp4(bytes)?))
    } else if bytes.len() >= 4 && bytes[0..4] == [0x1A, 0x45, 0xDF, 0xA3] {
        Ok(("matroska", parse_matroska(bytes)?))
    } else {
        Err("unsupported container — provide an MP4/M4V/M4A/MOV or Matroska/WebM (.mkv/.webm) file".into())
    }
}

fn clean_filename(name: &str) -> String {
    let n = name.trim();
    // Strip any directory component and query/fragment noise.
    let base = n.rsplit(['/', '\\']).next().unwrap_or(n);
    let base = base.split(['?', '#']).next().unwrap_or(base);
    if base.is_empty() {
        "video".to_string()
    } else {
        base.to_string()
    }
}

// ---------------------------------------------------------------------------
// MP4 / ISO-BMFF (Nero `chpl` atom + `mvhd` duration)
// ---------------------------------------------------------------------------

/// Iterate the direct child boxes within `data[start..end]`, yielding
/// `(box_type, body_start, body_end)`. Malformed/short boxes stop iteration.
fn iter_boxes(data: &[u8], start: usize, end: usize) -> Vec<([u8; 4], usize, usize)> {
    let mut out = Vec::new();
    let mut off = start;
    while off + 8 <= end {
        let size32 = u32::from_be_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
        let mut btype = [0u8; 4];
        btype.copy_from_slice(&data[off + 4..off + 8]);
        let (box_size, header) = if size32 == 1 {
            // 64-bit largesize follows the type.
            if off + 16 > end {
                break;
            }
            let large = u64::from_be_bytes([
                data[off + 8],
                data[off + 9],
                data[off + 10],
                data[off + 11],
                data[off + 12],
                data[off + 13],
                data[off + 14],
                data[off + 15],
            ]);
            (large as usize, 16usize)
        } else if size32 == 0 {
            // Extends to the end of the parent.
            (end - off, 8usize)
        } else {
            (size32 as usize, 8usize)
        };
        if box_size < header || off + box_size > end {
            break;
        }
        out.push((btype, off + header, off + box_size));
        off += box_size;
    }
    out
}

fn parse_mp4(data: &[u8]) -> Result<Vec<Chapter>, String> {
    let top = iter_boxes(data, 0, data.len());
    let moov = top
        .iter()
        .find(|(t, _, _)| t == b"moov")
        .ok_or("no moov box found — file is not a readable MP4")?;
    let moov_children = iter_boxes(data, moov.1, moov.2);

    // Movie duration (ms) from mvhd, used to close the final chapter.
    let duration_ms = moov_children
        .iter()
        .find(|(t, _, _)| t == b"mvhd")
        .and_then(|(_, s, e)| mvhd_duration_ms(data, *s, *e));

    // chpl lives in moov/udta.
    let mut chapters = Vec::new();
    if let Some((_, us, ue)) = moov_children.iter().find(|(t, _, _)| t == b"udta") {
        let udta_children = iter_boxes(data, *us, *ue);
        if let Some((_, cs, ce)) = udta_children.iter().find(|(t, _, _)| t == b"chpl") {
            chapters = parse_chpl(data, *cs, *ce);
        }
    }

    close_ends(&mut chapters, duration_ms);
    Ok(chapters)
}

fn mvhd_duration_ms(data: &[u8], start: usize, end: usize) -> Option<u64> {
    if start + 4 > end {
        return None;
    }
    let version = data[start];
    if version == 1 {
        // version(1)+flags(3) + creation(8)+mod(8) + timescale(4) + duration(8)
        let ts_off = start + 4 + 8 + 8;
        let dur_off = ts_off + 4;
        if dur_off + 8 > end {
            return None;
        }
        let timescale = be_u32(data, ts_off)? as u64;
        let duration = be_u64(data, dur_off)?;
        scale_to_ms(duration, timescale)
    } else {
        // version(1)+flags(3) + creation(4)+mod(4) + timescale(4) + duration(4)
        let ts_off = start + 4 + 4 + 4;
        let dur_off = ts_off + 4;
        if dur_off + 4 > end {
            return None;
        }
        let timescale = be_u32(data, ts_off)? as u64;
        let duration = be_u32(data, dur_off)? as u64;
        scale_to_ms(duration, timescale)
    }
}

fn scale_to_ms(units: u64, timescale: u64) -> Option<u64> {
    if timescale == 0 {
        return None;
    }
    Some((units as u128 * 1000 / timescale as u128) as u64)
}

/// Nero `chpl`: version(1) flags(3) [reserved(4) if version!=0] count(1),
/// then per chapter: start(u64, 100ns units) len(1) title(len bytes, UTF-8).
fn parse_chpl(data: &[u8], start: usize, end: usize) -> Vec<Chapter> {
    let mut p = start;
    if p >= end {
        return Vec::new();
    }
    let version = data[p];
    p += 1; // version
    p += 3; // flags
    if version != 0 {
        p += 4; // reserved
    }
    if p >= end {
        return Vec::new();
    }
    let count = data[p] as usize;
    p += 1;
    let mut chapters = Vec::new();
    for _ in 0..count {
        if p + 8 > end {
            break;
        }
        let start_units = match be_u64(data, p) {
            Some(v) => v,
            None => break,
        };
        p += 8;
        if p >= end {
            break;
        }
        let len = data[p] as usize;
        p += 1;
        if p + len > end {
            break;
        }
        let title = decode_title(&data[p..p + len]);
        p += len;
        chapters.push(Chapter {
            index: chapters.len() + 1,
            title,
            start_ms: start_units / 10_000, // 100ns units -> ms
            end_ms: None,
        });
    }
    chapters
}

/// Derive missing chapter ends: each chapter ends where the next begins; the
/// last chapter ends at `duration_ms` when known and after its start.
fn close_ends(chapters: &mut [Chapter], duration_ms: Option<u64>) {
    let n = chapters.len();
    for i in 0..n {
        if chapters[i].end_ms.is_some() {
            continue;
        }
        if i + 1 < n {
            chapters[i].end_ms = Some(chapters[i + 1].start_ms);
        } else if let Some(d) = duration_ms {
            if d >= chapters[i].start_ms {
                chapters[i].end_ms = Some(d);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Matroska / WebM (EBML)
// ---------------------------------------------------------------------------

const ID_SEGMENT: u64 = 0x1853_8067;
const ID_CHAPTERS: u64 = 0x1043_A770;
const ID_EDITION_ENTRY: u64 = 0x45B9;
const ID_CHAPTER_ATOM: u64 = 0xB6;
const ID_CHAPTER_TIME_START: u64 = 0x91;
const ID_CHAPTER_TIME_END: u64 = 0x92;
const ID_CHAPTER_DISPLAY: u64 = 0x80;
const ID_CHAP_STRING: u64 = 0x85;

/// Read an EBML variable-length integer at `off`. Returns `(value, byte_len)`.
/// When `keep_marker` is false the length-descriptor bits are stripped (used
/// for element sizes); when true they are kept (used for element IDs).
fn read_vint(data: &[u8], off: usize, keep_marker: bool) -> Option<(u64, usize)> {
    if off >= data.len() {
        return None;
    }
    let first = data[off];
    if first == 0 {
        return None;
    }
    let mut mask = 0x80u8;
    let mut length = 1usize;
    while mask != 0 && (first & mask) == 0 {
        mask >>= 1;
        length += 1;
    }
    if length > 8 || off + length > data.len() {
        return None;
    }
    let mut value: u64 = if keep_marker {
        first as u64
    } else {
        (first & (mask.wrapping_sub(1))) as u64
    };
    for i in 1..length {
        value = (value << 8) | data[off + i] as u64;
    }
    Some((value, length))
}

/// Iterate direct EBML children within `data[start..end]`, yielding
/// `(id, body_start, body_end)`.
fn iter_ebml(data: &[u8], start: usize, end: usize) -> Vec<(u64, usize, usize)> {
    let mut out = Vec::new();
    let mut off = start;
    while off < end {
        let (id, id_len) = match read_vint(data, off, true) {
            Some(v) => v,
            None => break,
        };
        let size_off = off + id_len;
        let (size, size_len) = match read_vint(data, size_off, false) {
            Some(v) => v,
            None => break,
        };
        let body = size_off + size_len;
        // "Unknown size" (all data bits set) → run to the parent end.
        let all_ones = (1u64 << (7 * size_len)) - 1;
        let body_end = if size == all_ones {
            end
        } else {
            body.saturating_add(size as usize)
        };
        if body > end || body_end > end || body_end < body {
            break;
        }
        out.push((id, body, body_end));
        off = body_end;
    }
    out
}

fn parse_matroska(data: &[u8]) -> Result<Vec<Chapter>, String> {
    let top = iter_ebml(data, 0, data.len());
    let segment = top
        .iter()
        .find(|(id, _, _)| *id == ID_SEGMENT)
        .ok_or("no EBML Segment found — file is not a readable Matroska/WebM")?;
    let seg_children = iter_ebml(data, segment.1, segment.2);
    let chapters_el = match seg_children.iter().find(|(id, _, _)| *id == ID_CHAPTERS) {
        Some(c) => c,
        None => return Ok(Vec::new()), // no chapters embedded
    };
    let mut chapters = Vec::new();
    for (id, s, e) in iter_ebml(data, chapters_el.1, chapters_el.2) {
        if id != ID_EDITION_ENTRY {
            continue;
        }
        for (aid, as_, ae) in iter_ebml(data, s, e) {
            if aid == ID_CHAPTER_ATOM {
                if let Some(ch) = parse_chapter_atom(data, as_, ae, chapters.len() + 1) {
                    chapters.push(ch);
                }
            }
        }
        // Only the first edition that yields chapters (the default edition).
        if !chapters.is_empty() {
            break;
        }
    }
    Ok(chapters)
}

fn parse_chapter_atom(data: &[u8], start: usize, end: usize, index: usize) -> Option<Chapter> {
    let mut start_ns: Option<u64> = None;
    let mut end_ns: Option<u64> = None;
    let mut title = String::new();
    for (id, s, e) in iter_ebml(data, start, end) {
        match id {
            ID_CHAPTER_TIME_START => start_ns = ebml_uint(data, s, e),
            ID_CHAPTER_TIME_END => end_ns = ebml_uint(data, s, e),
            ID_CHAPTER_DISPLAY => {
                for (did, ds, de) in iter_ebml(data, s, e) {
                    if did == ID_CHAP_STRING && title.is_empty() {
                        title = decode_title(&data[ds..de.min(data.len())]);
                    }
                }
            }
            _ => {}
        }
    }
    let start_ns = start_ns?;
    Some(Chapter {
        index,
        title,
        start_ms: start_ns / 1_000_000, // ns -> ms
        end_ms: end_ns.map(|v| v / 1_000_000),
    })
}

fn ebml_uint(data: &[u8], start: usize, end: usize) -> Option<u64> {
    if start >= end || end > data.len() || end - start > 8 {
        return None;
    }
    let mut v = 0u64;
    for b in &data[start..end] {
        v = (v << 8) | *b as u64;
    }
    Some(v)
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn be_u32(data: &[u8], off: usize) -> Option<u32> {
    data.get(off..off + 4)
        .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

fn be_u64(data: &[u8], off: usize) -> Option<u64> {
    data.get(off..off + 8)
        .map(|b| u64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]))
}

/// Decode a title, stripping a leading UTF-8 BOM and trimming surrounding
/// whitespace / trailing NULs; invalid UTF-8 is replaced, not rejected.
fn decode_title(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    String::from_utf8_lossy(bytes)
        .trim_end_matches('\0')
        .trim()
        .to_string()
}

// ---------------------------------------------------------------------------
// Time formatting
// ---------------------------------------------------------------------------

/// `HH:MM:SS.mmm`
fn fmt_hms_ms(ms: u64) -> String {
    let (h, m, s, milli) = split_ms(ms);
    format!("{h:02}:{m:02}:{s:02}.{milli:03}")
}

/// Decimal seconds with 3 fractional digits (`10.000`).
fn fmt_seconds(ms: u64) -> String {
    format!("{}.{:03}", ms / 1000, ms % 1000)
}

/// YouTube-style timestamp: `H:MM:SS` when there are hours, else `M:SS`.
fn fmt_youtube(ms: u64) -> String {
    let (h, m, s, _) = split_ms(ms);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// CUE-sheet timestamp `MM:SS:FF` (FF = frames, 75 per second).
fn fmt_cue(ms: u64) -> String {
    let total_secs = ms / 1000;
    let mm = total_secs / 60;
    let ss = total_secs % 60;
    // Round the sub-second remainder to the nearest of 75 frames.
    let ff = (((ms % 1000) * 75) + 500) / 1000;
    let ff = if ff >= 75 { 74 } else { ff };
    format!("{mm:02}:{ss:02}:{ff:02}")
}

fn split_ms(ms: u64) -> (u64, u64, u64, u64) {
    let milli = ms % 1000;
    let total_secs = ms / 1000;
    let s = total_secs % 60;
    let m = (total_secs / 60) % 60;
    let h = total_secs / 3600;
    (h, m, s, milli)
}

// ---------------------------------------------------------------------------
// Renderers
// ---------------------------------------------------------------------------

fn csv_escape(field: &str) -> String {
    if field.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

fn render_csv(chapters: &[Chapter]) -> String {
    let mut out = String::from("index,start,end,start_seconds,end_seconds,title");
    for c in chapters {
        let (end_hms, end_sec) = match c.end_ms {
            Some(e) => (fmt_hms_ms(e), fmt_seconds(e)),
            None => (String::new(), String::new()),
        };
        out.push('\n');
        out.push_str(&format!(
            "{},{},{},{},{},{}",
            c.index,
            fmt_hms_ms(c.start_ms),
            end_hms,
            fmt_seconds(c.start_ms),
            end_sec,
            csv_escape(&c.title),
        ));
    }
    out
}

fn render_json(chapters: &[Chapter]) -> String {
    let arr: Vec<serde_json::Value> = chapters
        .iter()
        .map(|c| {
            serde_json::json!({
                "index": c.index,
                "title": c.title,
                "start": fmt_hms_ms(c.start_ms),
                "end": c.end_ms.map(fmt_hms_ms),
                "start_ms": c.start_ms,
                "end_ms": c.end_ms,
            })
        })
        .collect();
    serde_json::to_string_pretty(&arr).unwrap_or_else(|_| "[]".to_string())
}

fn render_text(chapters: &[Chapter]) -> String {
    chapters
        .iter()
        .map(|c| format!("{} {}", fmt_youtube(c.start_ms), c.title))
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_cue(chapters: &[Chapter], filename: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("TITLE \"{}\"\n", cue_escape(filename)));
    out.push_str(&format!("FILE \"{}\" WAVE\n", cue_escape(filename)));
    for c in chapters {
        out.push_str(&format!("  TRACK {:02} AUDIO\n", c.index));
        out.push_str(&format!("    TITLE \"{}\"\n", cue_escape(&c.title)));
        out.push_str(&format!("    INDEX 01 {}\n", fmt_cue(c.start_ms)));
    }
    // Trim the trailing newline so the body has no blank final line.
    out.trim_end().to_string()
}

fn cue_escape(s: &str) -> String {
    s.replace('"', "'")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_helpers() {
        assert_eq!(fmt_hms_ms(0), "00:00:00.000");
        assert_eq!(fmt_hms_ms(10_000), "00:00:10.000");
        assert_eq!(fmt_hms_ms(3_661_500), "01:01:01.500");
        assert_eq!(fmt_youtube(0), "0:00");
        assert_eq!(fmt_youtube(10_000), "0:10");
        assert_eq!(fmt_youtube(3_661_000), "1:01:01");
        assert_eq!(fmt_cue(0), "00:00:00");
        assert_eq!(fmt_cue(10_000), "00:10:00");
        assert_eq!(fmt_seconds(10_500), "10.500");
    }

    #[test]
    fn csv_quotes_titles_with_commas() {
        assert_eq!(csv_escape("Intro"), "Intro");
        assert_eq!(csv_escape("Hello, World"), "\"Hello, World\"");
        assert_eq!(csv_escape("She said \"hi\""), "\"She said \"\"hi\"\"\"");
    }

    #[test]
    fn unknown_format_errors() {
        let err = export_chapters(b"\x1A\x45\xDF\xA3", "yaml", "x.mkv").unwrap_err();
        assert!(err.contains("unknown format"), "got: {err}");
    }

    #[test]
    fn unsupported_container_errors() {
        let err = export_chapters(b"not a video file at all", "json", "x").unwrap_err();
        assert!(err.contains("unsupported container"), "got: {err}");
    }

    #[test]
    fn render_text_yields_youtube_lines() {
        let chapters = vec![
            Chapter { index: 1, title: "Intro".into(), start_ms: 0, end_ms: Some(10_000) },
            Chapter { index: 2, title: "Body".into(), start_ms: 65_000, end_ms: None },
        ];
        assert_eq!(render_text(&chapters), "0:00 Intro\n1:05 Body");
    }

    #[test]
    fn close_ends_fills_from_next_and_duration() {
        let mut chapters = vec![
            Chapter { index: 1, title: "a".into(), start_ms: 0, end_ms: None },
            Chapter { index: 2, title: "b".into(), start_ms: 10_000, end_ms: None },
        ];
        close_ends(&mut chapters, Some(25_000));
        assert_eq!(chapters[0].end_ms, Some(10_000));
        assert_eq!(chapters[1].end_ms, Some(25_000));
    }
}

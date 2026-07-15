//! gizza-ai/detect-file-type core — pure, dependency-free file-type sniffing.
//! Identifies a file's true format from its leading bytes (magic numbers) and a
//! few structural offsets, independent of any filename or extension. No I/O, no
//! deps — runs on every backend including the chat Service Worker.

/// What a successful sniff reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detection {
    /// Canonical IANA-ish media type (e.g. `image/png`, `application/pdf`).
    pub mime: &'static str,
    /// Usual lowercase file extension WITHOUT the dot (e.g. `png`).
    pub ext: &'static str,
    /// Human-readable format name (e.g. `PNG image`).
    pub kind: &'static str,
    /// Coarse bucket: image / video / audio / document / archive / font /
    /// executable / database / text / data / unknown.
    pub category: &'static str,
}

const fn d(mime: &'static str, ext: &'static str, kind: &'static str, category: &'static str) -> Detection {
    Detection { mime, ext, kind, category }
}

fn starts(b: &[u8], sig: &[u8]) -> bool {
    b.len() >= sig.len() && &b[..sig.len()] == sig
}

/// Bytes at `off..off+sig.len()` equal `sig`.
fn at(b: &[u8], off: usize, sig: &[u8]) -> bool {
    b.len() >= off + sig.len() && &b[off..off + sig.len()] == sig
}

/// Map an ISO-BMFF `ftyp` major brand to a concrete type. `brand` is the 4 bytes
/// at offset 8.
fn ftyp_brand(brand: &[u8]) -> Detection {
    match brand {
        b"avif" | b"avis" => d("image/avif", "avif", "AVIF image", "image"),
        b"heic" | b"heix" | b"heim" | b"heis" | b"hevc" | b"hevx" => d("image/heic", "heic", "HEIC image", "image"),
        b"mif1" | b"msf1" => d("image/heif", "heif", "HEIF image", "image"),
        b"qt  " => d("video/quicktime", "mov", "QuickTime video", "video"),
        b"M4A " => d("audio/mp4", "m4a", "M4A audio", "audio"),
        b"M4B " => d("audio/mp4", "m4b", "M4B audiobook", "audio"),
        b"M4V " => d("video/x-m4v", "m4v", "M4V video", "video"),
        b"3gp4" | b"3gp5" | b"3g2a" => d("video/3gpp", "3gp", "3GP video", "video"),
        b"crx " => d("image/x-canon-cr3", "cr3", "Canon CR3 raw image", "image"),
        // isom / mp41 / mp42 / dash / iso2 / ... → generic MP4 video container.
        _ => d("video/mp4", "mp4", "MP4 video", "video"),
    }
}

/// Identify the file behind `bytes`. Returns `None` only for empty input; an
/// unrecognised non-empty blob resolves to a text/binary fallback.
pub fn detect(bytes: &[u8]) -> Option<Detection> {
    if bytes.is_empty() {
        return None;
    }
    let b = bytes;

    // ---- Images ---------------------------------------------------------
    if starts(b, &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Some(d("image/png", "png", "PNG image", "image"));
    }
    if starts(b, &[0xFF, 0xD8, 0xFF]) {
        return Some(d("image/jpeg", "jpg", "JPEG image", "image"));
    }
    if starts(b, b"GIF87a") || starts(b, b"GIF89a") {
        return Some(d("image/gif", "gif", "GIF image", "image"));
    }
    if starts(b, b"BM") {
        return Some(d("image/bmp", "bmp", "BMP image", "image"));
    }
    if starts(b, &[0x49, 0x49, 0x2A, 0x00]) || starts(b, &[0x4D, 0x4D, 0x00, 0x2A]) {
        return Some(d("image/tiff", "tiff", "TIFF image", "image"));
    }
    if starts(b, &[0x00, 0x00, 0x01, 0x00]) {
        return Some(d("image/x-icon", "ico", "ICO icon", "image"));
    }
    if starts(b, b"8BPS") {
        return Some(d("image/vnd.adobe.photoshop", "psd", "Photoshop document", "image"));
    }
    // RIFF container: discriminate by the form type at offset 8.
    if starts(b, b"RIFF") && b.len() >= 12 {
        return Some(match &b[8..12] {
            b"WEBP" => d("image/webp", "webp", "WebP image", "image"),
            b"WAVE" => d("audio/wav", "wav", "WAV audio", "audio"),
            b"AVI " => d("video/x-msvideo", "avi", "AVI video", "video"),
            _ => d("application/octet-stream", "riff", "RIFF container", "data"),
        });
    }

    // ---- ISO-BMFF (ftyp): mp4 / mov / m4a / heic / avif / 3gp ----------
    if at(b, 4, b"ftyp") && b.len() >= 12 {
        return Some(ftyp_brand(&b[8..12]));
    }

    // ---- Documents ------------------------------------------------------
    if starts(b, b"%PDF-") {
        return Some(d("application/pdf", "pdf", "PDF document", "document"));
    }
    if starts(b, b"{\\rtf") {
        return Some(d("application/rtf", "rtf", "RTF document", "document"));
    }
    // Legacy MS Office / OLE2 compound file (doc, xls, ppt, msi).
    if starts(b, &[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]) {
        return Some(d("application/x-ole-storage", "doc", "Microsoft Office (legacy OLE2) document", "document"));
    }

    // ---- Audio (header-based) ------------------------------------------
    if starts(b, b"ID3") {
        return Some(d("audio/mpeg", "mp3", "MP3 audio (ID3)", "audio"));
    }
    if starts(b, &[0xFF, 0xFB]) || starts(b, &[0xFF, 0xF3]) || starts(b, &[0xFF, 0xF2]) {
        return Some(d("audio/mpeg", "mp3", "MP3 audio", "audio"));
    }
    if starts(b, &[0xFF, 0xF1]) || starts(b, &[0xFF, 0xF9]) {
        return Some(d("audio/aac", "aac", "AAC audio (ADTS)", "audio"));
    }
    if starts(b, b"fLaC") {
        return Some(d("audio/flac", "flac", "FLAC audio", "audio"));
    }
    if starts(b, b"OggS") {
        return Some(d("audio/ogg", "ogg", "Ogg media", "audio"));
    }
    if starts(b, b"MThd") {
        return Some(d("audio/midi", "mid", "MIDI audio", "audio"));
    }
    if starts(b, b"FORM") && at(b, 8, b"AIFF") {
        return Some(d("audio/aiff", "aiff", "AIFF audio", "audio"));
    }

    // ---- Video (header-based) ------------------------------------------
    if starts(b, &[0x1A, 0x45, 0xDF, 0xA3]) {
        // EBML — WebM or Matroska. Peek for the "webm" doctype in the header.
        let scan = &b[..b.len().min(64)];
        let webm = scan.windows(4).any(|w| w == b"webm");
        return Some(if webm {
            d("video/webm", "webm", "WebM video", "video")
        } else {
            d("video/x-matroska", "mkv", "Matroska video", "video")
        });
    }
    if starts(b, b"FLV") {
        return Some(d("video/x-flv", "flv", "FLV video", "video"));
    }
    if starts(b, &[0x30, 0x26, 0xB2, 0x75]) {
        return Some(d("video/x-ms-asf", "wmv", "ASF/WMV media", "video"));
    }
    if starts(b, &[0x00, 0x00, 0x01, 0xBA]) || starts(b, &[0x00, 0x00, 0x01, 0xB3]) {
        return Some(d("video/mpeg", "mpg", "MPEG program stream", "video"));
    }

    // ---- Archives / compression ----------------------------------------
    if starts(b, &[0x1F, 0x8B]) {
        return Some(d("application/gzip", "gz", "Gzip archive", "archive"));
    }
    if starts(b, b"BZh") {
        return Some(d("application/x-bzip2", "bz2", "Bzip2 archive", "archive"));
    }
    if starts(b, &[0xFD, b'7', b'z', b'X', b'Z', 0x00]) {
        return Some(d("application/x-xz", "xz", "XZ archive", "archive"));
    }
    if starts(b, &[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C]) {
        return Some(d("application/x-7z-compressed", "7z", "7-Zip archive", "archive"));
    }
    if starts(b, b"Rar!\x1A\x07") {
        return Some(d("application/x-rar-compressed", "rar", "RAR archive", "archive"));
    }
    if starts(b, &[0x28, 0xB5, 0x2F, 0xFD]) {
        return Some(d("application/zstd", "zst", "Zstandard archive", "archive"));
    }
    if starts(b, &[0x04, 0x22, 0x4D, 0x18]) {
        return Some(d("application/x-lz4", "lz4", "LZ4 archive", "archive"));
    }
    if starts(b, b"!<arch>") {
        return Some(d("application/x-archive", "ar", "Unix ar archive", "archive"));
    }
    // TAR: "ustar" magic lives at offset 257.
    if at(b, 257, b"ustar") {
        return Some(d("application/x-tar", "tar", "TAR archive", "archive"));
    }
    // ZIP (and the many ZIP-based formats: docx/xlsx/pptx/epub/jar/apk/odf).
    if starts(b, &[0x50, 0x4B, 0x03, 0x04]) || starts(b, &[0x50, 0x4B, 0x05, 0x06]) || starts(b, &[0x50, 0x4B, 0x07, 0x08]) {
        return Some(detect_zip_payload(b));
    }

    // ---- Databases ------------------------------------------------------
    if starts(b, b"SQLite format 3\x00") {
        return Some(d("application/vnd.sqlite3", "sqlite", "SQLite 3 database", "database"));
    }

    // ---- Fonts ----------------------------------------------------------
    if starts(b, b"wOFF") {
        return Some(d("font/woff", "woff", "WOFF font", "font"));
    }
    if starts(b, b"wOF2") {
        return Some(d("font/woff2", "woff2", "WOFF2 font", "font"));
    }
    if starts(b, b"OTTO") {
        return Some(d("font/otf", "otf", "OpenType font", "font"));
    }
    if starts(b, &[0x00, 0x01, 0x00, 0x00, 0x00]) || starts(b, b"true") || starts(b, b"ttcf") {
        return Some(d("font/ttf", "ttf", "TrueType font", "font"));
    }

    // ---- Executables / bytecode ----------------------------------------
    if starts(b, &[0x7F, b'E', b'L', b'F']) {
        return Some(d("application/x-elf", "elf", "ELF executable", "executable"));
    }
    if starts(b, b"MZ") {
        return Some(d("application/vnd.microsoft.portable-executable", "exe", "Windows PE executable", "executable"));
    }
    if starts(b, &[0x00, 0x61, 0x73, 0x6D]) {
        return Some(d("application/wasm", "wasm", "WebAssembly module", "executable"));
    }
    if starts(b, &[0xCA, 0xFE, 0xBA, 0xBE]) {
        return Some(d("application/java-vm", "class", "Java class / Mach-O fat binary", "executable"));
    }
    if starts(b, &[0xFE, 0xED, 0xFA, 0xCE]) || starts(b, &[0xFE, 0xED, 0xFA, 0xCF])
        || starts(b, &[0xCE, 0xFA, 0xED, 0xFE]) || starts(b, &[0xCF, 0xFA, 0xED, 0xFE]) {
        return Some(d("application/x-mach-binary", "macho", "Mach-O executable", "executable"));
    }

    // ---- Text-ish structured data --------------------------------------
    // BOM-prefixed UTF-8 text.
    let body = if starts(b, &[0xEF, 0xBB, 0xBF]) { &b[3..] } else { b };
    if let Ok(text) = core::str::from_utf8(body) {
        let trimmed = text.trim_start();
        let lower_head: String = trimmed.chars().take(64).collect::<String>().to_ascii_lowercase();
        if lower_head.starts_with("<?xml") {
            if lower_head.contains("<svg") || trimmed.contains("<svg") {
                return Some(d("image/svg+xml", "svg", "SVG image", "image"));
            }
            return Some(d("application/xml", "xml", "XML document", "data"));
        }
        if lower_head.starts_with("<svg") {
            return Some(d("image/svg+xml", "svg", "SVG image", "image"));
        }
        if lower_head.starts_with("<!doctype html") || lower_head.starts_with("<html") {
            return Some(d("text/html", "html", "HTML document", "document"));
        }
        if lower_head.starts_with("%!ps") {
            return Some(d("application/postscript", "ps", "PostScript document", "document"));
        }
        // JSON: a leading { or [ that parses balanced enough to be plausible.
        let first = trimmed.as_bytes().first().copied();
        if (first == Some(b'{') || first == Some(b'[')) && looks_like_json(trimmed) {
            return Some(d("application/json", "json", "JSON data", "data"));
        }
        return Some(d("text/plain", "txt", "Plain text (UTF-8)", "text"));
    }

    Some(d("application/octet-stream", "bin", "Unknown binary data", "unknown"))
}

/// A leading ZIP local-file header may be an OOXML / OpenDocument / EPUB / JAR.
/// Peek the first stored entry name (right after the 30-byte local header) to
/// disambiguate the common cases; otherwise report a plain ZIP.
fn detect_zip_payload(b: &[u8]) -> Detection {
    // Local file header: name length at 26..28 (LE), name starts at offset 30.
    if b.len() >= 30 {
        let name_len = u16::from_le_bytes([b[26], b[27]]) as usize;
        let start = 30;
        let end = (start + name_len).min(b.len());
        let name = &b[start..end];
        if name == b"mimetype" {
            // OpenDocument / EPUB store an uncompressed `mimetype` entry first;
            // its value follows the (stored) header.
            let data_start = end;
            let scan = &b[data_start..b.len().min(data_start + 80)];
            if scan.windows(20).any(|w| w == b"application/epub+zip") {
                return d("application/epub+zip", "epub", "EPUB e-book", "document");
            }
            if scan.windows(33).any(|w| w == b"application/vnd.oasis.opendocument") {
                if scan.windows(4).any(|w| w == b"text") {
                    return d("application/vnd.oasis.opendocument.text", "odt", "OpenDocument text", "document");
                }
                if scan.windows(11).any(|w| w == b"spreadsheet") {
                    return d("application/vnd.oasis.opendocument.spreadsheet", "ods", "OpenDocument spreadsheet", "document");
                }
                return d("application/vnd.oasis.opendocument.text", "odf", "OpenDocument file", "document");
            }
        }
        // OOXML packages name their first part under [Content_Types].xml, but the
        // first entry is commonly `[Content_Types].xml` or `word/`/`xl/`/`ppt/`.
        if name.starts_with(b"word/") {
            return d("application/vnd.openxmlformats-officedocument.wordprocessingml.document", "docx", "Word (DOCX) document", "document");
        }
        if name.starts_with(b"xl/") {
            return d("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet", "xlsx", "Excel (XLSX) spreadsheet", "document");
        }
        if name.starts_with(b"ppt/") {
            return d("application/vnd.openxmlformats-officedocument.presentationml.presentation", "pptx", "PowerPoint (PPTX) presentation", "document");
        }
    }
    d("application/zip", "zip", "ZIP archive (may be docx/xlsx/pptx/epub/jar)", "archive")
}

/// Cheap structural plausibility check for JSON: brackets/braces balance when
/// ignoring string contents, and the first non-space char is `{` or `[`.
fn looks_like_json(s: &str) -> bool {
    let mut depth: i64 = 0;
    let mut in_str = false;
    let mut esc = false;
    let mut saw_open = false;
    for c in s.chars() {
        if in_str {
            if esc {
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        match c {
            '"' => in_str = true,
            '{' | '[' => {
                depth += 1;
                saw_open = true;
            }
            '}' | ']' => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            _ => {}
        }
    }
    saw_open && depth == 0 && !in_str
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ext_of(bytes: &[u8]) -> &'static str {
        detect(bytes).unwrap().ext
    }

    #[test]
    fn empty_is_none() {
        assert!(detect(&[]).is_none());
    }

    #[test]
    fn images() {
        assert_eq!(ext_of(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]), "png");
        assert_eq!(ext_of(&[0xFF, 0xD8, 0xFF, 0xE0]), "jpg");
        assert_eq!(ext_of(b"GIF89a..."), "gif");
        assert_eq!(ext_of(b"BM......"), "bmp");
        assert_eq!(ext_of(&[0x49, 0x49, 0x2A, 0x00]), "tiff");
        // RIFF/WEBP
        let mut webp = b"RIFF".to_vec();
        webp.extend_from_slice(&[0, 0, 0, 0]);
        webp.extend_from_slice(b"WEBP");
        assert_eq!(ext_of(&webp), "webp");
    }

    #[test]
    fn riff_family() {
        let mk = |form: &[u8]| {
            let mut v = b"RIFF".to_vec();
            v.extend_from_slice(&[0, 0, 0, 0]);
            v.extend_from_slice(form);
            v
        };
        assert_eq!(ext_of(&mk(b"WAVE")), "wav");
        assert_eq!(ext_of(&mk(b"AVI ")), "avi");
    }

    #[test]
    fn isobmff_brands() {
        let mk = |brand: &[u8]| {
            let mut v = vec![0, 0, 0, 0x20];
            v.extend_from_slice(b"ftyp");
            v.extend_from_slice(brand);
            v
        };
        assert_eq!(ext_of(&mk(b"isom")), "mp4");
        assert_eq!(ext_of(&mk(b"mp42")), "mp4");
        assert_eq!(ext_of(&mk(b"qt  ")), "mov");
        assert_eq!(ext_of(&mk(b"M4A ")), "m4a");
        assert_eq!(ext_of(&mk(b"avif")), "avif");
        assert_eq!(ext_of(&mk(b"heic")), "heic");
        assert_eq!(ext_of(&mk(b"3gp4")), "3gp");
    }

    #[test]
    fn audio_video_headers() {
        assert_eq!(ext_of(b"ID3\x04\x00"), "mp3");
        assert_eq!(ext_of(&[0xFF, 0xFB, 0x90, 0x00]), "mp3");
        assert_eq!(ext_of(b"fLaC\x00"), "flac");
        assert_eq!(ext_of(b"OggS\x00"), "ogg");
        assert_eq!(ext_of(&[0x1A, 0x45, 0xDF, 0xA3]), "mkv");
        // EBML with a webm doctype nearby
        let mut webm = vec![0x1A, 0x45, 0xDF, 0xA3];
        webm.extend_from_slice(b"............webm....");
        assert_eq!(ext_of(&webm), "webm");
    }

    #[test]
    fn documents_and_office() {
        assert_eq!(ext_of(b"%PDF-1.7"), "pdf");
        assert_eq!(ext_of(b"{\\rtf1"), "rtf");
        assert_eq!(ext_of(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]), "doc");
    }

    #[test]
    fn zip_and_ooxml() {
        // Minimal ZIP local file header: 30 bytes, name_len at 26..28 (LE),
        // extra_len at 28..30, entry name from offset 30.
        let mk_zip = |first: &[u8]| {
            let mut v = vec![0x50, 0x4B, 0x03, 0x04];
            v.extend_from_slice(&[0u8; 26]); // rest of the 30-byte header
            v[26] = first.len() as u8;
            v[27] = 0;
            // 28..30 (extra_len) stay 0
            v.extend_from_slice(first);
            v
        };

        // Bare zip with a generic first entry name.
        assert_eq!(ext_of(&mk_zip(b"hello.txt")), "zip");

        // OOXML: first entry under word/ (resp. xl/, ppt/).
        let mk_ooxml = mk_zip;
        assert_eq!(ext_of(&mk_ooxml(b"word/document.xml")), "docx");
        assert_eq!(ext_of(&mk_ooxml(b"xl/workbook.xml")), "xlsx");
        assert_eq!(ext_of(&mk_ooxml(b"ppt/presentation.xml")), "pptx");
    }

    #[test]
    fn archives() {
        assert_eq!(ext_of(&[0x1F, 0x8B, 0x08]), "gz");
        assert_eq!(ext_of(b"BZh91"), "bz2");
        assert_eq!(ext_of(&[0xFD, b'7', b'z', b'X', b'Z', 0x00]), "xz");
        assert_eq!(ext_of(&[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C]), "7z");
        assert_eq!(ext_of(b"Rar!\x1A\x07\x00"), "rar");
        assert_eq!(ext_of(&[0x28, 0xB5, 0x2F, 0xFD]), "zst");
        // TAR ustar at offset 257
        let mut tar = vec![0u8; 265];
        tar[257..262].copy_from_slice(b"ustar");
        assert_eq!(ext_of(&tar), "tar");
    }

    #[test]
    fn executables_fonts_db() {
        assert_eq!(ext_of(&[0x7F, b'E', b'L', b'F']), "elf");
        assert_eq!(ext_of(b"MZ\x90\x00"), "exe");
        assert_eq!(ext_of(&[0x00, 0x61, 0x73, 0x6D, 0x01]), "wasm");
        assert_eq!(ext_of(b"wOFF"), "woff");
        assert_eq!(ext_of(b"wOF2"), "woff2");
        assert_eq!(ext_of(b"OTTO"), "otf");
        assert_eq!(ext_of(b"SQLite format 3\x00"), "sqlite");
    }

    #[test]
    fn text_xml_svg_json_html() {
        assert_eq!(ext_of(b"<?xml version=\"1.0\"?><note>hi</note>"), "xml");
        assert_eq!(ext_of(b"<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>"), "svg");
        assert_eq!(ext_of(b"<?xml version=\"1.0\"?><svg></svg>"), "svg");
        assert_eq!(ext_of(b"<!DOCTYPE html><html></html>"), "html");
        assert_eq!(ext_of(b"{\"a\": 1, \"b\": [2, 3]}"), "json");
        assert_eq!(ext_of(b"[1, 2, 3]"), "json");
        assert_eq!(ext_of("hello world\n".as_bytes()), "txt");
        // BOM-prefixed text
        let mut bom = vec![0xEF, 0xBB, 0xBF];
        bom.extend_from_slice(b"plain");
        assert_eq!(ext_of(&bom), "txt");
        // not-json text starting with { but unbalanced stays text
        assert_eq!(ext_of(b"{ this is not json"), "txt");
    }

    #[test]
    fn unknown_binary() {
        assert_eq!(ext_of(&[0x12, 0x34, 0xAB, 0xCD, 0xFF]), "bin");
        assert_eq!(detect(&[0x12, 0x34, 0xAB, 0xCD, 0xFF]).unwrap().category, "unknown");
    }

    #[test]
    fn category_assignments() {
        assert_eq!(detect(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]).unwrap().category, "image");
        assert_eq!(detect(b"%PDF-1.4").unwrap().category, "document");
        assert_eq!(detect(&[0x1F, 0x8B]).unwrap().category, "archive");
    }
}

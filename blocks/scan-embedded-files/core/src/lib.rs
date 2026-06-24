//! gizza-ai/scan-embedded-files core — pure, dependency-free byte scanner that
//! finds file signatures (magic numbers) appearing ANYWHERE in a file, not just
//! at offset 0. Useful for spotting hidden / appended / concatenated payloads:
//! a ZIP appended to a PNG (a "polyglot"), an EXE carved into a JPEG, a PDF
//! with an embedded image, files smuggled past an extension, etc.
//!
//! No I/O, no deps — runs on every backend including the chat Service Worker.

/// A known file format we can recognise by a fixed byte signature.
#[derive(Debug, Clone, Copy)]
struct Sig {
    /// The magic bytes that mark the start of this format.
    magic: &'static [u8],
    /// Usual lowercase extension WITHOUT the dot (e.g. `png`).
    ext: &'static str,
    /// Human-readable format name (e.g. `PNG image`).
    kind: &'static str,
    /// Canonical media type (e.g. `image/png`).
    mime: &'static str,
}

const fn s(magic: &'static [u8], ext: &'static str, kind: &'static str, mime: &'static str) -> Sig {
    Sig { magic, ext, kind, mime }
}

/// One signature found at a byte offset inside the scanned file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    /// Byte offset where the signature begins.
    pub offset: usize,
    /// Usual extension without the dot, e.g. `zip`.
    pub ext: String,
    /// Human-readable format name, e.g. `ZIP archive`.
    pub kind: String,
    /// Canonical media type, e.g. `application/zip`.
    pub mime: String,
    /// True for the signature at offset 0 (the file's own / declared type).
    pub is_leading: bool,
    /// True when this hit begins at/after the leading file's logical end, i.e.
    /// it is data APPENDED past the host format's structure (a strong polyglot /
    /// hidden-payload signal). Only set when a cheap logical end is known for the
    /// leading format and the offset lands at/after it.
    pub appended: bool,
}

/// The full scan result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scan {
    /// Total bytes scanned.
    pub bytes: usize,
    /// Every signature occurrence found, in ascending offset order.
    pub hits: Vec<Hit>,
}

/// Signatures we hunt for throughout the buffer. Kept to distinctive,
/// low-false-positive magics (3+ bytes, or structurally anchored) so a scan of
/// arbitrary binary data does not light up with noise.
const SIGS: &[Sig] = &[
    // Images
    s(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A], "png", "PNG image", "image/png"),
    s(b"GIF89a", "gif", "GIF image", "image/gif"),
    s(b"GIF87a", "gif", "GIF image", "image/gif"),
    s(&[0xFF, 0xD8, 0xFF], "jpg", "JPEG image", "image/jpeg"),
    s(b"8BPS", "psd", "Photoshop document", "image/vnd.adobe.photoshop"),
    // Documents
    s(b"%PDF-", "pdf", "PDF document", "application/pdf"),
    s(b"{\\rtf", "rtf", "RTF document", "application/rtf"),
    s(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1], "ole", "Microsoft Office (legacy OLE2)", "application/x-ole-storage"),
    // Audio / video containers
    s(b"ID3", "mp3", "MP3 audio (ID3)", "audio/mpeg"),
    s(b"fLaC", "flac", "FLAC audio", "audio/flac"),
    s(b"OggS", "ogg", "Ogg media", "audio/ogg"),
    s(b"FLV\x01", "flv", "FLV video", "video/x-flv"),
    s(&[0x1A, 0x45, 0xDF, 0xA3], "mkv", "Matroska/WebM (EBML)", "video/x-matroska"),
    s(&[0x30, 0x26, 0xB2, 0x75, 0x8E, 0x66, 0xCF, 0x11], "asf", "ASF/WMV media", "video/x-ms-asf"),
    // Archives / compression
    s(&[0x50, 0x4B, 0x03, 0x04], "zip", "ZIP local-file entry", "application/zip"),
    s(&[0x50, 0x4B, 0x05, 0x06], "zip", "ZIP end-of-central-directory", "application/zip"),
    s(&[0x1F, 0x8B, 0x08], "gz", "Gzip archive", "application/gzip"),
    s(b"BZh", "bz2", "Bzip2 archive", "application/x-bzip2"),
    s(&[0xFD, b'7', b'z', b'X', b'Z', 0x00], "xz", "XZ archive", "application/x-xz"),
    s(&[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C], "7z", "7-Zip archive", "application/x-7z-compressed"),
    s(b"Rar!\x1A\x07", "rar", "RAR archive", "application/x-rar-compressed"),
    s(&[0x28, 0xB5, 0x2F, 0xFD], "zst", "Zstandard archive", "application/zstd"),
    s(b"!<arch>", "ar", "Unix ar archive", "application/x-archive"),
    // Databases
    s(b"SQLite format 3\x00", "sqlite", "SQLite 3 database", "application/vnd.sqlite3"),
    // Fonts
    s(b"wOFF", "woff", "WOFF font", "font/woff"),
    s(b"wOF2", "woff2", "WOFF2 font", "font/woff2"),
    s(b"OTTO", "otf", "OpenType font", "font/otf"),
    // Executables / bytecode
    s(&[0x7F, b'E', b'L', b'F'], "elf", "ELF executable", "application/x-elf"),
    s(&[0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00], "wasm", "WebAssembly module", "application/wasm"),
    s(&[0xCA, 0xFE, 0xBA, 0xBE], "class", "Java class / Mach-O fat binary", "application/java-vm"),
    s(&[0xFE, 0xED, 0xFA, 0xCF], "macho", "Mach-O 64-bit executable", "application/x-mach-binary"),
    s(&[0xFE, 0xED, 0xFA, 0xCE], "macho", "Mach-O 32-bit executable", "application/x-mach-binary"),
];

/// Whether `b[off..]` begins with `sig`.
fn matches_at(b: &[u8], off: usize, sig: &[u8]) -> bool {
    b.len() >= off + sig.len() && &b[off..off + sig.len()] == sig
}

/// First offset `>= from` where `b` contains `needle`.
fn find_from(b: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || from >= b.len() {
        return None;
    }
    b[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

/// Compute the logical end of the LEADING file (the host format starting at
/// offset 0) when it can be determined cheaply from structure. Anything found at
/// or after this offset is genuinely appended data. Returns `None` when the host
/// format has no cheap length (then we don't flag appension by length).
fn leading_logical_end(b: &[u8]) -> Option<usize> {
    // PNG: ends right after the IEND chunk type (4) + its CRC (4).
    if matches_at(b, 0, &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        if let Some(p) = find_from(b, b"IEND", 8) {
            return Some((p + 4 + 4).min(b.len())); // "IEND" + 4-byte CRC
        }
    }
    // GIF: ends at the trailer byte 0x3B (scan from the end).
    if matches_at(b, 0, b"GIF87a") || matches_at(b, 0, b"GIF89a") {
        if let Some(p) = b.iter().rposition(|&c| c == 0x3B) {
            return Some(p + 1);
        }
    }
    // JPEG: ends at the EOI marker FF D9 (scan from the end).
    if matches_at(b, 0, &[0xFF, 0xD8, 0xFF]) {
        if let Some(p) = b.windows(2).rposition(|w| w == [0xFF, 0xD9]) {
            return Some(p + 2);
        }
    }
    None
}

/// Scan `bytes` for embedded/appended file signatures. The leading hit (offset
/// 0), if recognised, is always included and marked `is_leading`. Returns the
/// full ordered list of hits.
pub fn scan(bytes: &[u8]) -> Scan {
    let mut hits: Vec<Hit> = Vec::new();
    let logical_end = leading_logical_end(bytes);

    let mut off = 0usize;
    while off < bytes.len() {
        // Find the longest matching signature at this offset, if any.
        let mut best: Option<&Sig> = None;
        for sig in SIGS {
            if matches_at(bytes, off, sig.magic)
                && best.map(|x| sig.magic.len() > x.magic.len()).unwrap_or(true)
            {
                best = Some(sig);
            }
        }
        if let Some(sig) = best {
            let is_leading = off == 0;
            let appended = !is_leading && logical_end.map(|end| off >= end).unwrap_or(false);
            hits.push(Hit {
                offset: off,
                ext: sig.ext.to_string(),
                kind: sig.kind.to_string(),
                mime: sig.mime.to_string(),
                is_leading,
                appended,
            });
            // Advance past this magic so we don't re-match a prefix of it.
            off += sig.magic.len();
        } else {
            off += 1;
        }
    }

    Scan { bytes: bytes.len(), hits }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

    fn minimal_png() -> Vec<u8> {
        let mut v = PNG.to_vec();
        v.extend_from_slice(&[0, 0, 0, 0]); // IEND length = 0
        v.extend_from_slice(b"IEND");
        v.extend_from_slice(&[0xAE, 0x42, 0x60, 0x82]); // IEND CRC
        v
    }

    #[test]
    fn empty_scans_clean() {
        let r = scan(&[]);
        assert_eq!(r.bytes, 0);
        assert!(r.hits.is_empty());
    }

    #[test]
    fn leading_png_is_detected_and_marked() {
        let png = minimal_png();
        let r = scan(&png);
        assert_eq!(r.hits.len(), 1);
        let h = &r.hits[0];
        assert_eq!(h.offset, 0);
        assert_eq!(h.ext, "png");
        assert!(h.is_leading);
        assert!(!h.appended);
    }

    #[test]
    fn zip_appended_to_png_is_flagged_appended() {
        let mut v = minimal_png();
        let png_len = v.len();
        v.extend_from_slice(b"junkjunk");
        let zip_off = v.len();
        v.extend_from_slice(&[0x50, 0x4B, 0x03, 0x04]);
        v.extend_from_slice(b"....payload....");

        let r = scan(&v);
        assert_eq!(r.hits.len(), 2, "hits: {:?}", r.hits);
        assert!(r.hits[0].is_leading && r.hits[0].ext == "png");
        let zip = &r.hits[1];
        assert_eq!(zip.offset, zip_off);
        assert_eq!(zip.ext, "zip");
        assert!(!zip.is_leading);
        assert!(zip.appended, "zip past PNG IEND should be flagged appended");
        assert!(zip_off > png_len);
    }

    #[test]
    fn embedded_pdf_inside_jpeg_not_flagged_appended_when_before_eoi() {
        let mut v = vec![0xFF, 0xD8, 0xFF, 0xE0];
        v.extend_from_slice(b"some jpeg data ");
        let pdf_off = v.len();
        v.extend_from_slice(b"%PDF-1.4 hidden ");
        v.extend_from_slice(&[0xFF, 0xD9]); // EOI

        let r = scan(&v);
        let jpg = r.hits.iter().find(|h| h.is_leading).unwrap();
        assert_eq!(jpg.ext, "jpg");
        let pdf = r.hits.iter().find(|h| h.ext == "pdf").unwrap();
        assert_eq!(pdf.offset, pdf_off);
        assert!(!pdf.appended, "pdf before EOI is embedded, not appended");
    }

    #[test]
    fn multiple_signatures_all_reported_in_order() {
        let mut v: Vec<u8> = Vec::new();
        v.extend_from_slice(b"GIF89a");
        v.extend_from_slice(&[0; 4]);
        v.push(0x3B); // GIF trailer → logical end
        let after = v.len();
        v.extend_from_slice(b"Rar!\x1A\x07");
        v.extend_from_slice(&[0; 4]);
        v.extend_from_slice(&[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C]); // 7z

        let r = scan(&v);
        assert!(r.hits[0].ext == "gif" && r.hits[0].is_leading);
        let exts: Vec<&str> = r.hits.iter().map(|h| h.ext.as_str()).collect();
        assert!(exts.contains(&"rar"));
        assert!(exts.contains(&"7z"));
        for w in r.hits.windows(2) {
            assert!(w[0].offset < w[1].offset);
        }
        assert!(r.hits.iter().find(|h| h.ext == "rar").unwrap().appended);
        assert!(r.hits.iter().find(|h| h.ext == "rar").unwrap().offset >= after);
    }

    #[test]
    fn random_binary_has_few_false_hits() {
        let v: Vec<u8> = (0u8..=255).cycle().take(4096).collect();
        let r = scan(&v);
        assert!(r.hits.len() < 5, "too many false positives: {}", r.hits.len());
    }

    #[test]
    fn longest_signature_wins_at_offset() {
        let v = vec![0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00, 0xFF];
        let r = scan(&v);
        assert_eq!(r.hits[0].ext, "wasm");
        assert!(r.hits[0].is_leading);
    }
}

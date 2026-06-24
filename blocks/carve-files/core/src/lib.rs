//! gizza-ai/carve-files core — recover embedded files from a binary blob by
//! scanning for known magic-byte signatures ("file carving").
//!
//! Given an arbitrary blob (a disk image, a memory dump, a corrupted container,
//! a concatenation of files, …), this scans for the leading signatures of common
//! formats and, where the format is self-describing, reads its declared length to
//! cut out the embedded file. Formats with no in-band length (e.g. PNG/GIF/PDF)
//! are carved up to their trailer/end-of-file marker; others fall back to a
//! best-effort run up to the next signature or the blob end.
//!
//! Pure Rust, only `base64` for the recovered bytes — no I/O, no wasm-unsafe
//! deps, runs on every backend including the chat Service Worker.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde::Serialize;

/// Default cap on total recovered (inlined) content bytes, so the JSON stays
/// bounded. Files past the budget are still listed (offset/size) without data.
pub const DEFAULT_CONTENT_BUDGET: usize = 8 * 1024 * 1024;

/// A single recovered file.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Carved {
    /// Byte offset of the file's first byte within the blob.
    pub offset: usize,
    /// Length of the carved file in bytes.
    pub size: usize,
    /// Usual extension without the dot, e.g. `png`.
    pub extension: String,
    /// Human-readable format name, e.g. `PNG image`.
    pub kind: String,
    /// Canonical media type, e.g. `image/png`.
    pub mime: String,
    /// True when the file's end was found exactly (a trailer/declared length);
    /// false when the size is a best-effort run to the next signature/blob end.
    pub exact_end: bool,
    /// Base64 of the carved bytes, when within the content budget.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    /// Set when the data was omitted because the budget was exhausted.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub content_omitted: bool,
}

/// Result of a carve pass.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CarveResult {
    pub files: Vec<Carved>,
    /// Number of recovered files.
    pub count: usize,
    /// Size of the scanned blob in bytes.
    pub scanned_bytes: usize,
}

/// A recognised format and how to bound a carved instance of it.
struct Sig {
    /// Leading magic bytes that mark the start of a file.
    magic: &'static [u8],
    ext: &'static str,
    kind: &'static str,
    mime: &'static str,
    /// Strategy for finding the file's end, given its start offset.
    end: End,
}

enum End {
    /// Trailer marker that terminates the file; carve up to and including it.
    Trailer(&'static [u8]),
    /// Size is a 32-bit little-endian field at `at` bytes from the start, and the
    /// total file size is `header + that value`.
    LeU32 { at: usize, header: usize },
    /// Format with no usable in-band end here — carve up to the next signature
    /// or the blob end (best effort).
    Heuristic,
}

const fn s(magic: &'static [u8], ext: &'static str, kind: &'static str, mime: &'static str, end: End) -> Sig {
    Sig { magic, ext, kind, mime, end }
}

/// The signature table. Longer / more specific magics that share a prefix with a
/// shorter one are placed first so the scanner prefers them.
fn signatures() -> &'static [Sig] {
    SIGNATURES
}

static SIGNATURES: &[Sig] = {
    &[
        // ---- Images ----
        s(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A], "png", "PNG image", "image/png",
          End::Trailer(&[0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82])), // IEND + CRC
        s(&[0xFF, 0xD8, 0xFF], "jpg", "JPEG image", "image/jpeg",
          End::Trailer(&[0xFF, 0xD9])), // EOI
        s(b"GIF89a", "gif", "GIF image", "image/gif", End::Trailer(&[0x00, 0x3B])),
        s(b"GIF87a", "gif", "GIF image", "image/gif", End::Trailer(&[0x00, 0x3B])),
        s(b"BM", "bmp", "BMP image", "image/bmp", End::LeU32 { at: 2, header: 0 }),
        // ---- Documents ----
        s(b"%PDF-", "pdf", "PDF document", "application/pdf", End::Trailer(b"%%EOF")),
        // ---- Archives / compressed ----
        s(&[0x50, 0x4B, 0x03, 0x04], "zip", "ZIP archive", "application/zip",
          End::Trailer(&[0x50, 0x4B, 0x05, 0x06])), // EOCD record start (approximate)
        s(&[0x1F, 0x8B], "gz", "gzip data", "application/gzip", End::Heuristic),
        s(&[0x42, 0x5A, 0x68], "bz2", "bzip2 data", "application/x-bzip2", End::Heuristic),
        s(&[0xFD, b'7', b'z', b'X', b'Z', 0x00], "xz", "XZ data", "application/x-xz", End::Heuristic),
        s(&[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C], "7z", "7-Zip archive", "application/x-7z-compressed", End::Heuristic),
        s(b"Rar!\x1A\x07", "rar", "RAR archive", "application/vnd.rar", End::Heuristic),
        // ---- Audio / video ----
        s(b"OggS", "ogg", "Ogg media", "application/ogg", End::Heuristic),
        s(b"fLaC", "flac", "FLAC audio", "audio/flac", End::Heuristic),
        s(b"ID3", "mp3", "MP3 audio (ID3)", "audio/mpeg", End::Heuristic),
        s(b"RIFF", "riff", "RIFF (WAV/AVI/WEBP) container", "application/octet-stream",
          End::LeU32 { at: 4, header: 8 }),
        // ---- Other ----
        s(b"SQLite format 3\x00", "sqlite", "SQLite database", "application/vnd.sqlite3", End::Heuristic),
    ]
};

fn starts_at(b: &[u8], off: usize, sig: &[u8]) -> bool {
    off + sig.len() <= b.len() && &b[off..off + sig.len()] == sig
}

/// Find the first match in the signature table at offset `off`, if any.
fn match_at(b: &[u8], off: usize) -> Option<&'static Sig> {
    signatures().iter().find(|sig| starts_at(b, off, sig.magic))
}

/// Find the next offset `>= from` where ANY signature matches.
fn next_signature(b: &[u8], from: usize) -> Option<usize> {
    (from..b.len()).find(|&i| match_at(b, i).is_some())
}

/// Read a little-endian u32 at `b[off..off+4]`.
fn le_u32(b: &[u8], off: usize) -> Option<u32> {
    let s = b.get(off..off + 4)?;
    Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// Compute the end offset (exclusive) of the file that starts at `start` using
/// the format's `End` strategy. Returns `(end_exclusive, exact)`.
fn carve_end(b: &[u8], start: usize, sig: &Sig) -> (usize, bool) {
    match &sig.end {
        End::Trailer(t) => {
            // Search for the trailer after the magic.
            let search_from = start + sig.magic.len();
            if let Some(rel) = find_subslice(&b[search_from..], t) {
                (search_from + rel + t.len(), true)
            } else {
                // No trailer found — best effort to next signature / blob end.
                (next_boundary(b, start), false)
            }
        }
        End::LeU32 { at, header } => {
            if let Some(v) = le_u32(b, start + at) {
                let total = (v as usize).saturating_add(*header);
                let end = start.saturating_add(total);
                if total > 0 && end <= b.len() {
                    return (end, true);
                }
            }
            (next_boundary(b, start), false)
        }
        End::Heuristic => (next_boundary(b, start), false),
    }
}

/// Best-effort end: the next signature after `start`, or the blob end.
fn next_boundary(b: &[u8], start: usize) -> usize {
    next_signature(b, start + 1).unwrap_or(b.len())
}

/// Index of the first occurrence of `needle` in `hay`, if any.
fn find_subslice(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || needle.len() > hay.len() {
        return None;
    }
    (0..=hay.len() - needle.len()).find(|&i| &hay[i..i + needle.len()] == needle)
}

/// Scan `blob` for embedded files and carve them out. `content_budget` caps the
/// total recovered bytes inlined as base64; files past the budget are listed
/// (offset/size/type) with `content_omitted = true`.
pub fn carve(blob: &[u8], content_budget: usize) -> CarveResult {
    let mut files = Vec::new();
    let mut used = 0usize;
    let mut i = 0usize;
    while i < blob.len() {
        let Some(sig) = match_at(blob, i) else {
            i += 1;
            continue;
        };
        let (end, exact_end) = carve_end(blob, i, sig);
        let size = end.saturating_sub(i);
        if size == 0 {
            i += 1;
            continue;
        }

        let mut data = None;
        let mut content_omitted = false;
        if used.saturating_add(size) <= content_budget {
            data = Some(B64.encode(&blob[i..end]));
            used += size;
        } else {
            content_omitted = true;
        }

        files.push(Carved {
            offset: i,
            size,
            extension: sig.ext.to_string(),
            kind: sig.kind.to_string(),
            mime: sig.mime.to_string(),
            exact_end,
            data,
            content_omitted,
        });

        // Resume scanning after this file. (Embedded files can nest, e.g. a
        // thumbnail inside a JPEG, but resuming past the carved region is the
        // standard carving behaviour and avoids duplicate/overlapping hits.)
        i = end.max(i + 1);
    }

    CarveResult {
        count: files.len(),
        scanned_bytes: blob.len(),
        files,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png() -> Vec<u8> {
        let mut v = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        v.extend_from_slice(b"FAKEDATA");
        v.extend_from_slice(&[0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82]); // IEND
        v
    }

    fn jpeg() -> Vec<u8> {
        let mut v = vec![0xFF, 0xD8, 0xFF, 0xE0];
        v.extend_from_slice(b"jpegbody");
        v.extend_from_slice(&[0xFF, 0xD9]); // EOI
        v
    }

    #[test]
    fn carves_two_concatenated_images() {
        let p = png();
        let j = jpeg();
        let mut blob = Vec::new();
        blob.extend_from_slice(&[0u8; 4]); // junk prefix
        blob.extend_from_slice(&p);
        blob.extend_from_slice(b"some junk between");
        blob.extend_from_slice(&j);
        let r = carve(&blob, DEFAULT_CONTENT_BUDGET);
        assert_eq!(r.count, 2);
        assert_eq!(r.scanned_bytes, blob.len());

        let png = &r.files[0];
        assert_eq!(png.extension, "png");
        assert_eq!(png.offset, 4);
        assert_eq!(png.size, p.len());
        assert!(png.exact_end);
        assert_eq!(B64.decode(png.data.as_ref().unwrap()).unwrap(), p);

        let jpg = &r.files[1];
        assert_eq!(jpg.extension, "jpg");
        assert_eq!(jpg.size, j.len());
        assert!(jpg.exact_end);
        assert_eq!(B64.decode(jpg.data.as_ref().unwrap()).unwrap(), j);
    }

    #[test]
    fn pdf_carved_to_eof_marker() {
        let mut blob = b"junk".to_vec();
        let start = blob.len();
        blob.extend_from_slice(b"%PDF-1.4\nbody bytes\n%%EOF");
        blob.extend_from_slice(b"trailing garbage");
        let r = carve(&blob, DEFAULT_CONTENT_BUDGET);
        assert_eq!(r.count, 1);
        let f = &r.files[0];
        assert_eq!(f.extension, "pdf");
        assert_eq!(f.offset, start);
        assert!(f.exact_end);
        assert_eq!(B64.decode(f.data.as_ref().unwrap()).unwrap(), b"%PDF-1.4\nbody bytes\n%%EOF");
    }

    #[test]
    fn bmp_uses_declared_size() {
        // BM + 4-byte LE total size (header=0 -> size IS the file size)
        let mut file = b"BM".to_vec();
        let body = vec![0xABu8; 20];
        let total = (2 + 4 + body.len()) as u32; // "BM" + size field + body
        file.extend_from_slice(&total.to_le_bytes());
        file.extend_from_slice(&body);
        assert_eq!(file.len(), total as usize);

        let mut blob = file.clone();
        blob.extend_from_slice(b"after");
        let r = carve(&blob, DEFAULT_CONTENT_BUDGET);
        assert_eq!(r.count, 1);
        let f = &r.files[0];
        assert_eq!(f.extension, "bmp");
        assert!(f.exact_end);
        assert_eq!(f.size, total as usize);
    }

    #[test]
    fn budget_omits_data_but_lists_file() {
        let blob = png();
        let r = carve(&blob, 4); // smaller than the PNG
        assert_eq!(r.count, 1);
        let f = &r.files[0];
        assert!(f.content_omitted);
        assert!(f.data.is_none());
        assert_eq!(f.extension, "png");
        assert!(f.size > 0);
    }

    #[test]
    fn empty_and_no_signatures() {
        let r = carve(b"", DEFAULT_CONTENT_BUDGET);
        assert_eq!(r.count, 0);
        assert_eq!(r.scanned_bytes, 0);

        let r = carve(b"just some plain text with no magic bytes at all", DEFAULT_CONTENT_BUDGET);
        assert_eq!(r.count, 0);
    }

    #[test]
    fn heuristic_runs_to_next_signature() {
        // gzip (heuristic end) followed by a PNG -> gzip carved up to the PNG start.
        let mut blob = vec![0x1F, 0x8B, 0x08, 0x00]; // gzip magic + a bit
        blob.extend_from_slice(b"gzbody");
        let png_start = blob.len();
        blob.extend_from_slice(&png());
        let r = carve(&blob, DEFAULT_CONTENT_BUDGET);
        assert_eq!(r.count, 2);
        assert_eq!(r.files[0].extension, "gz");
        assert!(!r.files[0].exact_end);
        assert_eq!(r.files[0].size, png_start); // ran exactly to the PNG
        assert_eq!(r.files[1].extension, "png");
    }
}

//! gizza-ai/identify-archive-format core — pure, dependency-free detection of
//! the COMPRESSION or ARCHIVE format behind a blob, from its leading bytes
//! (magic numbers) and a few structural offsets. Independent of any filename or
//! extension. No I/O, no deps — runs on every backend including the chat
//! Service Worker.
//!
//! Unlike a general file-type sniffer, this is scoped to the
//! "what do I decompress/unpack this with?" question: it covers the common
//! compressors (gzip, zlib, bzip2, xz, LZMA, zstd, lz4, compress/.Z, lzip,
//! lzop, snappy framing) and container archives (zip, tar, 7z, rar, ar, cpio,
//! cab), and reports a concrete decompression hint plus whether the format is a
//! single-stream COMPRESSOR or a multi-file ARCHIVE.

/// What a successful sniff reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detection {
    /// Short lowercase format id, e.g. `gzip`, `zip`, `xz`.
    pub format: &'static str,
    /// Human-readable name, e.g. `Gzip compressed stream`.
    pub name: &'static str,
    /// Canonical media type (e.g. `application/gzip`).
    pub mime: &'static str,
    /// Usual lowercase file extension WITHOUT the dot (e.g. `gz`).
    pub ext: &'static str,
    /// `archive` (holds multiple files) or `compressor` (single byte stream).
    pub kind: &'static str,
    /// A concrete tool / command to decompress or unpack it.
    pub decompress_with: &'static str,
}

const fn det(
    format: &'static str,
    name: &'static str,
    mime: &'static str,
    ext: &'static str,
    kind: &'static str,
    decompress_with: &'static str,
) -> Detection {
    Detection { format, name, mime, ext, kind, decompress_with }
}

fn starts(b: &[u8], sig: &[u8]) -> bool {
    b.len() >= sig.len() && &b[..sig.len()] == sig
}

/// Bytes at `off..off+sig.len()` equal `sig`.
fn at(b: &[u8], off: usize, sig: &[u8]) -> bool {
    b.len() >= off + sig.len() && &b[off..off + sig.len()] == sig
}

/// Identify the archive/compression format behind `bytes`. Returns `None` for
/// empty input OR for a non-empty blob that is not a recognised archive /
/// compression format (the caller reports "not a recognised archive").
pub fn detect(bytes: &[u8]) -> Option<Detection> {
    if bytes.is_empty() {
        return None;
    }
    let b = bytes;

    // ---- Compressors (single-stream) -----------------------------------
    if starts(b, &[0x1F, 0x8B]) {
        return Some(det(
            "gzip", "Gzip compressed stream", "application/gzip", "gz",
            "compressor", "gunzip / gzip -d / zcat",
        ));
    }
    if starts(b, b"BZh") {
        return Some(det(
            "bzip2", "Bzip2 compressed stream", "application/x-bzip2", "bz2",
            "compressor", "bunzip2 / bzip2 -d",
        ));
    }
    if starts(b, &[0xFD, b'7', b'z', b'X', b'Z', 0x00]) {
        return Some(det(
            "xz", "XZ compressed stream", "application/x-xz", "xz",
            "compressor", "unxz / xz -d",
        ));
    }
    // Zstandard. Magic is unambiguous; check before the generic LZMA heuristic.
    if starts(b, &[0x28, 0xB5, 0x2F, 0xFD]) {
        return Some(det(
            "zstd", "Zstandard compressed stream", "application/zstd", "zst",
            "compressor", "unzstd / zstd -d",
        ));
    }
    // LZ4 frame format.
    if starts(b, &[0x04, 0x22, 0x4D, 0x18]) {
        return Some(det(
            "lz4", "LZ4 compressed frame", "application/x-lz4", "lz4",
            "compressor", "lz4 -d",
        ));
    }
    // lzip.
    if starts(b, b"LZIP") {
        return Some(det(
            "lzip", "lzip compressed stream", "application/x-lzip", "lz",
            "compressor", "lzip -d / plzip -d",
        ));
    }
    // lzop.
    if starts(b, &[0x89, b'L', b'Z', b'O', 0x00, 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Some(det(
            "lzop", "lzop compressed stream", "application/x-lzop", "lzo",
            "compressor", "lzop -d",
        ));
    }
    // Unix compress (.Z) — LZW.
    if starts(b, &[0x1F, 0x9D]) {
        return Some(det(
            "compress", "Unix compress (LZW) stream", "application/x-compress", "Z",
            "compressor", "uncompress / gunzip",
        ));
    }
    // Old pack/old-compress (.z) — Huffman, rare but historical.
    if starts(b, &[0x1F, 0x1E]) || starts(b, &[0x1F, 0xA0]) {
        return Some(det(
            "pack", "Unix pack/old-compress stream", "application/x-compress", "z",
            "compressor", "unpack / gunzip",
        ));
    }
    // Snappy framing format ("sNaPpY" stream identifier chunk).
    if starts(b, &[0xFF, 0x06, 0x00, 0x00]) && at(b, 4, b"sNaPpY") {
        return Some(det(
            "snappy", "Snappy framed stream", "application/x-snappy-framed", "sz",
            "compressor", "snzip -d / python-snappy",
        ));
    }
    // Raw LZMA (.lzma / lzma_alone): properties byte (commonly 0x5D) then a
    // 4-byte little-endian dictionary size, then an 8-byte uncompressed size
    // (often 0xFF..FF when unknown). Heuristic: props <= 0xE0 (valid lc/lp/pb)
    // and a plausible power-of-two dictionary size, to avoid false positives.
    if b.len() >= 13 && b[0] <= 0xE0 {
        let dict = u32::from_le_bytes([b[1], b[2], b[3], b[4]]);
        if (0x1000..=0x4000_0000).contains(&dict) && (dict & dict.wrapping_sub(1)) == 0 {
            return Some(det(
                "lzma", "LZMA (alone) compressed stream", "application/x-lzma", "lzma",
                "compressor", "unlzma / xz --format=lzma -d",
            ));
        }
    }
    // zlib stream: low nibble of CMF is 8 (deflate), CINFO <= 7, and the
    // 16-bit (CMF<<8 | FLG) is a multiple of 31. Common headers: 78 01/9C/DA.
    if b.len() >= 2 {
        let cmf = b[0];
        let flg = b[1];
        let cm = cmf & 0x0F;
        let check_ok = ((cmf as u16) << 8 | flg as u16) % 31 == 0;
        if cm == 8 && (cmf >> 4) <= 7 && check_ok {
            return Some(det(
                "zlib", "zlib (deflate) compressed stream", "application/zlib", "zz",
                "compressor", "openssl zlib / python zlib.decompress / pigz -dz",
            ));
        }
    }

    // ---- Archives (multi-file containers) ------------------------------
    if starts(b, &[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C]) {
        return Some(det(
            "7z", "7-Zip archive", "application/x-7z-compressed", "7z",
            "archive", "7z x / 7za x",
        ));
    }
    if starts(b, b"Rar!\x1A\x07") {
        return Some(det(
            "rar", "RAR archive", "application/x-rar-compressed", "rar",
            "archive", "unrar x / 7z x",
        ));
    }
    if starts(b, b"MSCF") {
        return Some(det(
            "cab", "Microsoft Cabinet archive", "application/vnd.ms-cab-compressed", "cab",
            "archive", "cabextract / 7z x",
        ));
    }
    if starts(b, b"!<arch>") {
        return Some(det(
            "ar", "Unix ar / Debian package archive", "application/x-archive", "ar",
            "archive", "ar x / 7z x",
        ));
    }
    // cpio: ASCII "070707"/"070701"/"070702" or the two binary magics.
    if starts(b, b"070707") || starts(b, b"070701") || starts(b, b"070702")
        || starts(b, &[0xC7, 0x71]) || starts(b, &[0x71, 0xC7])
    {
        return Some(det(
            "cpio", "cpio archive", "application/x-cpio", "cpio",
            "archive", "cpio -idv / 7z x",
        ));
    }
    // TAR: "ustar" magic lives at offset 257 (POSIX/GNU).
    if at(b, 257, b"ustar") {
        return Some(det(
            "tar", "TAR archive", "application/x-tar", "tar",
            "archive", "tar -xf",
        ));
    }
    // ZIP local-file / central-dir / spanning header.
    if starts(b, &[0x50, 0x4B, 0x03, 0x04])
        || starts(b, &[0x50, 0x4B, 0x05, 0x06])
        || starts(b, &[0x50, 0x4B, 0x07, 0x08])
    {
        return Some(det(
            "zip", "ZIP archive", "application/zip", "zip",
            "archive", "unzip / 7z x",
        ));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt_of(bytes: &[u8]) -> &'static str {
        detect(bytes).expect("expected a detection").format
    }

    #[test]
    fn empty_is_none() {
        assert!(detect(&[]).is_none());
    }

    #[test]
    fn compressors() {
        assert_eq!(fmt_of(&[0x1F, 0x8B, 0x08]), "gzip");
        assert_eq!(fmt_of(b"BZh91AY"), "bzip2");
        assert_eq!(fmt_of(&[0xFD, b'7', b'z', b'X', b'Z', 0x00]), "xz");
        assert_eq!(fmt_of(&[0x28, 0xB5, 0x2F, 0xFD, 0x00]), "zstd");
        assert_eq!(fmt_of(&[0x04, 0x22, 0x4D, 0x18, 0x60]), "lz4");
        assert_eq!(fmt_of(b"LZIP\x01"), "lzip");
        assert_eq!(fmt_of(&[0x89, b'L', b'Z', b'O', 0x00, 0x0D, 0x0A, 0x1A, 0x0A]), "lzop");
        assert_eq!(fmt_of(&[0x1F, 0x9D, 0x90]), "compress");
    }

    #[test]
    fn snappy_frame() {
        let mut v = vec![0xFF, 0x06, 0x00, 0x00];
        v.extend_from_slice(b"sNaPpY");
        assert_eq!(fmt_of(&v), "snappy");
    }

    #[test]
    fn raw_lzma() {
        // props=0x5D, dict=0x00800000 (8 MiB, power of two), size=unknown.
        let mut v = vec![0x5D, 0x00, 0x00, 0x80, 0x00];
        v.extend_from_slice(&[0xFF; 8]); // uncompressed size (unknown)
        assert_eq!(fmt_of(&v), "lzma");
    }

    #[test]
    fn zlib_stream() {
        // 0x78 0x9C is the canonical default-compression zlib header (31-mult).
        assert_eq!(fmt_of(&[0x78, 0x9C, 0x01, 0x02, 0x03]), "zlib");
        // 0x78 0x01 (no/low compression) and 0x78 0xDA (best) also valid.
        assert_eq!(fmt_of(&[0x78, 0x01, 0x00]), "zlib");
        assert_eq!(fmt_of(&[0x78, 0xDA, 0x00]), "zlib");
    }

    #[test]
    fn archives() {
        assert_eq!(fmt_of(&[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C]), "7z");
        assert_eq!(fmt_of(b"Rar!\x1A\x07\x00"), "rar");
        assert_eq!(fmt_of(b"MSCF\x00\x00"), "cab");
        assert_eq!(fmt_of(b"!<arch>\n"), "ar");
        assert_eq!(fmt_of(b"070701000000"), "cpio");
        // ZIP
        let mut z = vec![0x50, 0x4B, 0x03, 0x04];
        z.extend_from_slice(&[0u8; 26]);
        assert_eq!(fmt_of(&z), "zip");
        // TAR (ustar at 257)
        let mut tar = vec![0u8; 265];
        tar[0] = b'f'; // a filename byte so it doesn't read as zlib
        tar[257..262].copy_from_slice(b"ustar");
        assert_eq!(fmt_of(&tar), "tar");
    }

    #[test]
    fn kind_and_hint() {
        let g = detect(&[0x1F, 0x8B, 0x08]).unwrap();
        assert_eq!(g.kind, "compressor");
        assert_eq!(g.ext, "gz");
        assert!(g.decompress_with.contains("gunzip"));
        let z = detect(&{
            let mut z = vec![0x50, 0x4B, 0x03, 0x04];
            z.extend_from_slice(&[0u8; 26]);
            z
        })
        .unwrap();
        assert_eq!(z.kind, "archive");
        assert!(z.decompress_with.contains("unzip"));
    }

    #[test]
    fn non_archive_is_none() {
        // PNG header — a real file, but NOT an archive/compression format.
        assert!(detect(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]).is_none());
        // Plain ASCII text.
        assert!(detect(b"hello world, this is plain text\n").is_none());
        // Arbitrary short binary that is not a zlib/lzma header.
        assert!(detect(&[0x00, 0x01, 0x02, 0x03]).is_none());
    }
}

//! png-chunk-stripper core — losslessly remove ancillary PNG chunks by walking
//! the chunk stream at the BYTE level. The pixel chunks (IHDR/PLTE/IDAT/IEND)
//! and the transparency chunk (tRNS) are copied VERBATIM — pixels are never
//! decoded or re-encoded, so the displayed image is bit-for-bit identical and
//! every kept chunk keeps its original CRC. Pure Rust, no image decode, no
//! ffmpeg → runs on every backend including the chat Service Worker.
//!
//! A PNG is an 8-byte signature followed by a sequence of chunks, each
//! `[4-byte big-endian length][4-byte type][length bytes of data][4-byte CRC]`.
//! A chunk is *ancillary* (safe to drop) when bit 5 of its first type byte is
//! set — i.e. the first letter is lowercase. Everything else is *critical*.

/// The 8-byte PNG file signature.
const PNG_MAGIC: [u8; 8] = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];

/// Which ancillary chunks to strip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Every ancillary chunk except `tRNS` (smallest file; also drops the color
    /// hints gAMA/cHRM/sRGB/iCCP, which can shift on-screen colors).
    All,
    /// Non-visual metadata only: text (tEXt/zTXt/iTXt), EXIF/XMP (eXIf) and the
    /// modification timestamp (tIME). Keeps color-management and physical-size
    /// chunks so appearance and DPI are preserved.
    Metadata,
    /// Only the privacy carriers: text chunks (tEXt/zTXt/iTXt) and EXIF (eXIf).
    Text,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Mode, String> {
        match s {
            "all" => Ok(Mode::All),
            "metadata" => Ok(Mode::Metadata),
            "text" => Ok(Mode::Text),
            other => Err(format!("unknown mode {other:?}; use all, metadata, or text")),
        }
    }
}

/// One removed chunk type, aggregated across the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Removed {
    /// The 4-character chunk type (e.g. "tEXt").
    pub chunk_type: String,
    /// How many chunks of this type were removed.
    pub count: usize,
    /// Total bytes removed for this type (including the 12-byte
    /// length+type+CRC framing per chunk).
    pub bytes: usize,
}

/// Result of a strip pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stripped {
    /// The output PNG bytes (identical to the input when nothing was removed).
    pub bytes: Vec<u8>,
    pub input_len: usize,
    pub output_len: usize,
    pub width: u32,
    pub height: u32,
    /// Removed chunk types, sorted by descending bytes then type name.
    pub removed: Vec<Removed>,
}

impl Stripped {
    /// Total bytes removed (input_len − output_len).
    pub fn bytes_saved(&self) -> usize {
        self.input_len.saturating_sub(self.output_len)
    }

    /// Total number of chunks removed across all types.
    pub fn chunks_removed(&self) -> usize {
        self.removed.iter().map(|r| r.count).sum()
    }

    /// One-line, LLM-facing summary of what was removed and the byte delta.
    pub fn summary(&self) -> String {
        if self.removed.is_empty() {
            return format!(
                "no removable ancillary chunks: {}×{} PNG, {} bytes unchanged",
                self.width, self.height, self.input_len
            );
        }
        let list = self
            .removed
            .iter()
            .map(|r| format!("{} ({}×, {} B)", r.chunk_type, r.count, r.bytes))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "stripped {} chunk(s) [{}] from {}×{} PNG ({} → {} bytes, {} saved); pixels unchanged",
            self.chunks_removed(),
            list,
            self.width,
            self.height,
            self.input_len,
            self.output_len,
            self.bytes_saved()
        )
    }
}

/// True when the chunk type is ancillary (safe to drop): bit 5 of the first
/// byte is set, i.e. the first letter is lowercase.
fn is_ancillary(t: &[u8; 4]) -> bool {
    t[0] & 0x20 != 0
}

/// Decide whether a chunk should be removed, given the mode + force-keep list.
fn should_remove(t: &[u8; 4], mode: Mode, keep: &[String]) -> bool {
    // Force-keep list wins over everything.
    if keep.iter().any(|k| k.as_bytes() == t) {
        return false;
    }
    // Critical chunks (IHDR/PLTE/IDAT/IEND, …) are never removed.
    if !is_ancillary(t) {
        return false;
    }
    // Transparency is visible-pixel data — never dropped, in any mode.
    if t == b"tRNS" {
        return false;
    }
    match mode {
        Mode::All => true,
        Mode::Metadata => matches!(t, b"tEXt" | b"zTXt" | b"iTXt" | b"eXIf" | b"tIME"),
        Mode::Text => matches!(t, b"tEXt" | b"zTXt" | b"iTXt" | b"eXIf"),
    }
}

/// Losslessly strip ancillary chunks from a PNG. `keep` is a list of 4-character
/// chunk types to always preserve, overriding `mode`. Returns an error if the
/// input is not a structurally valid PNG.
pub fn strip(bytes: &[u8], mode: Mode, keep: &[String]) -> Result<Stripped, String> {
    if bytes.len() < 8 || bytes[..8] != PNG_MAGIC {
        return Err("input is not a PNG file (missing the PNG signature)".into());
    }

    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    out.extend_from_slice(&PNG_MAGIC);

    // Aggregate removals in first-seen order, then sort at the end.
    let mut removed: Vec<Removed> = Vec::new();
    let mut width = 0u32;
    let mut height = 0u32;
    let mut saw_ihdr = false;
    let mut saw_iend = false;

    let mut pos = 8usize;
    while pos < bytes.len() {
        // Each chunk needs at least 8 bytes of length+type before its data.
        if pos + 8 > bytes.len() {
            return Err("truncated PNG: incomplete chunk header".into());
        }
        let len = u32::from_be_bytes([
            bytes[pos],
            bytes[pos + 1],
            bytes[pos + 2],
            bytes[pos + 3],
        ]) as usize;
        let type_start = pos + 4;
        let mut t = [0u8; 4];
        t.copy_from_slice(&bytes[type_start..type_start + 4]);
        let data_start = type_start + 4;
        let crc_end = data_start + len + 4;
        if crc_end > bytes.len() {
            return Err(format!(
                "truncated PNG: chunk {} claims {} data bytes past end of file",
                type_str(&t),
                len
            ));
        }

        // The first chunk must be IHDR (13 data bytes: width, height, …).
        if !saw_ihdr {
            if &t != b"IHDR" || len < 8 {
                return Err("not a valid PNG (first chunk is not a well-formed IHDR)".into());
            }
            width = u32::from_be_bytes([
                bytes[data_start],
                bytes[data_start + 1],
                bytes[data_start + 2],
                bytes[data_start + 3],
            ]);
            height = u32::from_be_bytes([
                bytes[data_start + 4],
                bytes[data_start + 5],
                bytes[data_start + 6],
                bytes[data_start + 7],
            ]);
            saw_ihdr = true;
        }

        let whole = &bytes[pos..crc_end];
        if should_remove(&t, mode, keep) {
            record(&mut removed, &t, whole.len());
        } else {
            out.extend_from_slice(whole);
        }

        pos = crc_end;
        if &t == b"IEND" {
            saw_iend = true;
            break; // IEND is the last chunk; ignore any trailing bytes.
        }
    }

    if !saw_iend {
        return Err("truncated PNG: no IEND chunk found".into());
    }

    // Sort: biggest savings first, ties broken by chunk type for stable output.
    removed.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.chunk_type.cmp(&b.chunk_type)));

    let output_len = out.len();
    Ok(Stripped {
        bytes: out,
        input_len: bytes.len(),
        output_len,
        width,
        height,
        removed,
    })
}

/// Render a chunk type as a string (chunk types are printable ASCII by spec;
/// fall back to an escaped form for anything unexpected).
fn type_str(t: &[u8; 4]) -> String {
    if t.iter().all(|&b| b.is_ascii_graphic()) {
        String::from_utf8_lossy(t).into_owned()
    } else {
        t.iter().map(|b| format!("\\x{b:02x}")).collect()
    }
}

/// Add a removed chunk to the aggregate (bumping count/bytes if already seen).
fn record(removed: &mut Vec<Removed>, t: &[u8; 4], whole_len: usize) {
    let ts = type_str(t);
    if let Some(e) = removed.iter_mut().find(|e| e.chunk_type == ts) {
        e.count += 1;
        e.bytes += whole_len;
    } else {
        removed.push(Removed {
            chunk_type: ts,
            count: 1,
            bytes: whole_len,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal but structurally valid PNG from a list of
    /// `(type, data)` chunks. CRC is computed so kept chunks round-trip.
    fn build_png(chunks: &[(&[u8; 4], Vec<u8>)]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&PNG_MAGIC);
        for (t, data) in chunks {
            v.extend_from_slice(&(data.len() as u32).to_be_bytes());
            v.extend_from_slice(*t);
            v.extend_from_slice(data);
            let mut crc_input = t.to_vec();
            crc_input.extend_from_slice(data);
            v.extend_from_slice(&crc32(&crc_input).to_be_bytes());
        }
        v
    }

    fn ihdr_data(w: u32, h: u32) -> Vec<u8> {
        let mut d = Vec::new();
        d.extend_from_slice(&w.to_be_bytes());
        d.extend_from_slice(&h.to_be_bytes());
        d.extend_from_slice(&[8, 6, 0, 0, 0]); // bit depth, color type, …
        d
    }

    // Minimal CRC-32 (IEEE) so the test fixtures are real PNG bytes.
    fn crc32(data: &[u8]) -> u32 {
        let mut crc: u32 = 0xffff_ffff;
        for &b in data {
            crc ^= b as u32;
            for _ in 0..8 {
                let mask = (crc & 1).wrapping_neg();
                crc = (crc >> 1) ^ (0xedb8_8320 & mask);
            }
        }
        !crc
    }

    fn sample() -> Vec<u8> {
        build_png(&[
            (b"IHDR", ihdr_data(4, 2)),
            (b"gAMA", vec![0, 0, 0, 1]),
            (b"tEXt", b"Comment\0secret note".to_vec()),
            (b"tIME", vec![7, 0xe4, 7, 22, 1, 2, 3]),
            (b"iCCP", vec![b'p', 0, 0, 1, 2, 3, 4]),
            (b"tRNS", vec![0, 0, 0]),
            (b"IDAT", vec![1, 2, 3, 4, 5, 6, 7, 8]),
            (b"IEND", vec![]),
        ])
    }

    #[test]
    fn strips_all_ancillary_but_keeps_critical_and_trns() {
        let png = sample();
        let res = strip(&png, Mode::All, &[]).unwrap();
        assert_eq!((res.width, res.height), (4, 2));
        // gAMA, tEXt, tIME, iCCP removed; IHDR/tRNS/IDAT/IEND kept.
        let types: Vec<&str> = res.removed.iter().map(|r| r.chunk_type.as_str()).collect();
        assert!(types.contains(&"tEXt") && types.contains(&"gAMA"));
        assert!(types.contains(&"tIME") && types.contains(&"iCCP"));
        assert_eq!(res.chunks_removed(), 4);
        assert!(res.output_len < res.input_len);
        // Kept chunks survive verbatim: tRNS + IDAT still present.
        assert!(find_chunk(&res.bytes, b"tRNS").is_some());
        assert!(find_chunk(&res.bytes, b"IDAT").is_some());
        assert!(find_chunk(&res.bytes, b"tEXt").is_none());
        // Pixel data byte-identical to the source IDAT.
        assert_eq!(find_chunk(&png, b"IDAT"), find_chunk(&res.bytes, b"IDAT"));
    }

    #[test]
    fn metadata_mode_keeps_color_and_physical() {
        let res = strip(&sample(), Mode::Metadata, &[]).unwrap();
        let types: Vec<&str> = res.removed.iter().map(|r| r.chunk_type.as_str()).collect();
        // Text + timestamp removed; color-management (gAMA/iCCP) kept.
        assert!(types.contains(&"tEXt") && types.contains(&"tIME"));
        assert!(!types.contains(&"gAMA") && !types.contains(&"iCCP"));
        assert!(find_chunk(&res.bytes, b"gAMA").is_some());
    }

    #[test]
    fn text_mode_keeps_time_and_color() {
        let res = strip(&sample(), Mode::Text, &[]).unwrap();
        let types: Vec<&str> = res.removed.iter().map(|r| r.chunk_type.as_str()).collect();
        assert_eq!(types, vec!["tEXt"]); // only the text chunk goes
    }

    #[test]
    fn keep_list_overrides_mode() {
        let keep = vec!["tEXt".to_string(), "gAMA".to_string()];
        let res = strip(&sample(), Mode::All, &keep).unwrap();
        assert!(find_chunk(&res.bytes, b"tEXt").is_some());
        assert!(find_chunk(&res.bytes, b"gAMA").is_some());
        let types: Vec<&str> = res.removed.iter().map(|r| r.chunk_type.as_str()).collect();
        assert!(!types.contains(&"tEXt") && !types.contains(&"gAMA"));
    }

    #[test]
    fn nothing_to_remove_returns_identical_bytes() {
        let png = build_png(&[
            (b"IHDR", ihdr_data(1, 1)),
            (b"IDAT", vec![9, 9, 9]),
            (b"IEND", vec![]),
        ]);
        let res = strip(&png, Mode::All, &[]).unwrap();
        assert_eq!(res.bytes, png);
        assert!(res.removed.is_empty());
        assert_eq!(res.bytes_saved(), 0);
    }

    #[test]
    fn rejects_non_png() {
        let err = strip(b"not a png at all", Mode::All, &[]).unwrap_err();
        assert!(err.contains("not a PNG"));
    }

    #[test]
    fn rejects_truncated_chunk() {
        // IHDR claims 13 bytes but the file ends early.
        let mut png = PNG_MAGIC.to_vec();
        png.extend_from_slice(&13u32.to_be_bytes());
        png.extend_from_slice(b"IHDR");
        png.extend_from_slice(&[0, 0, 0, 1]); // only 4 of 13 data bytes
        let err = strip(&png, Mode::All, &[]).unwrap_err();
        assert!(err.contains("truncated"));
    }

    #[test]
    fn summary_lists_removed_and_delta() {
        let res = strip(&sample(), Mode::All, &[]).unwrap();
        let s = res.summary();
        assert!(s.contains("tEXt") && s.contains("gAMA"));
        assert!(s.contains("pixels unchanged"));
        assert!(s.contains(&res.bytes_saved().to_string()));
    }

    #[test]
    fn summary_when_nothing_removed() {
        let png = build_png(&[
            (b"IHDR", ihdr_data(1, 1)),
            (b"IDAT", vec![9, 9, 9]),
            (b"IEND", vec![]),
        ]);
        let res = strip(&png, Mode::All, &[]).unwrap();
        assert!(res.summary().contains("unchanged"));
    }

    #[test]
    fn parse_mode() {
        assert_eq!(Mode::parse("metadata").unwrap(), Mode::Metadata);
        assert!(Mode::parse("bogus").is_err());
    }

    /// Locate a chunk's full framing (length+type+data+crc) by type, for asserts.
    fn find_chunk<'a>(png: &'a [u8], want: &[u8; 4]) -> Option<&'a [u8]> {
        let mut pos = 8;
        while pos + 8 <= png.len() {
            let len =
                u32::from_be_bytes([png[pos], png[pos + 1], png[pos + 2], png[pos + 3]]) as usize;
            let t = &png[pos + 4..pos + 8];
            let end = pos + 12 + len;
            if end > png.len() {
                return None;
            }
            if t == want {
                return Some(&png[pos..end]);
            }
            pos = end;
        }
        None
    }
}

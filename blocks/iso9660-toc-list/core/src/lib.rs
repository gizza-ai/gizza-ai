//! iso9660-toc-list core — parse an ISO 9660 (CD/DVD) disc image supplied as
//! base64 and list its directory tree, per-file sizes and volume label WITHOUT
//! extracting any file contents.
//!
//! Pure compute: only the volume descriptors and directory extents are read
//! (a few kilobytes), never the file data. A hand-rolled base64 decoder keeps
//! this dependency-free so the exact same logic runs on chat / CLI / page.
//!
//! Handles the Primary Volume Descriptor (ISO 9660 names) and, when present,
//! prefers the Joliet Supplementary Volume Descriptor (long UCS-2 names). The
//! new-tool skill single-sources the chat/CLI schema from `src/lib.rs`.

/// Volume descriptors and the reserved system area always use 2048-byte logical
/// sectors, regardless of the logical *block* size declared for file data.
const SECTOR: usize = 2048;
/// The first 16 sectors (0x8000) are the reserved "system area"; the volume
/// descriptor set begins at sector 16.
const SYSTEM_AREA: usize = 16 * SECTOR;

// Bounds so a malformed/adversarial image can never blow up time or memory.
const MAX_ENTRIES: usize = 200_000;
const MAX_DEPTH: usize = 64;
const MAX_DESCRIPTORS: usize = 64;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Tree,
    List,
    Summary,
}

impl Format {
    fn parse(s: &str) -> Result<Format, String> {
        match s.trim() {
            "tree" | "" => Ok(Format::Tree),
            "list" => Ok(Format::List),
            "summary" => Ok(Format::Summary),
            other => Err(format!(
                "unknown format '{other}' — use tree, list, or summary"
            )),
        }
    }
}

struct Node {
    name: String,
    is_dir: bool,
    size: u64,
    children: Vec<Node>,
}

struct Iso {
    label: String,
    joliet: bool,
    block_size: u64,
    volume_blocks: u64,
    root: Vec<Node>,
}

/// Public entry point used by the block, the CLI and the web wrapper.
/// Decodes the base64 image and renders it in the requested format.
pub fn run(data_b64: &str, format: &str) -> Result<String, String> {
    let fmt = Format::parse(format)?;
    if data_b64.trim().is_empty() {
        return Err("no input — paste the base64-encoded contents of an .iso image".into());
    }
    let bytes = b64_decode(data_b64)?;
    let iso = parse_iso(&bytes)?;
    Ok(render(&iso, fmt))
}

// ---------------------------------------------------------------------------
// ISO 9660 parsing
// ---------------------------------------------------------------------------

fn parse_iso(bytes: &[u8]) -> Result<Iso, String> {
    if bytes.len() < SYSTEM_AREA + SECTOR {
        return Err(format!(
            "not an ISO 9660 image — file is only {} bytes, smaller than the 34 KiB minimum (system area + one volume descriptor)",
            bytes.len()
        ));
    }
    // Sector 16 must carry the "CD001" standard identifier at offset 1.
    if &bytes[SYSTEM_AREA + 1..SYSTEM_AREA + 6] != b"CD001" {
        return Err(
            "not an ISO 9660 image — sector 16 does not carry the 'CD001' standard identifier"
                .into(),
        );
    }

    // Walk the volume descriptor set, keeping the Primary (type 1) and, if any,
    // a Joliet Supplementary descriptor (type 2 with a UCS-2 escape sequence).
    let mut pvd: Option<usize> = None;
    let mut joliet: Option<usize> = None;
    for i in 16..(16 + MAX_DESCRIPTORS) {
        let off = i * SECTOR;
        if off + SECTOR > bytes.len() {
            break;
        }
        if &bytes[off + 1..off + 6] != b"CD001" {
            break;
        }
        match bytes[off] {
            1 => {
                if pvd.is_none() {
                    pvd = Some(off);
                }
            }
            2 if is_joliet(&bytes[off..off + SECTOR]) => {
                if joliet.is_none() {
                    joliet = Some(off);
                }
            }
            255 => break, // volume descriptor set terminator
            _ => {}
        }
    }

    // Prefer Joliet (long, correctly-cased names) when present, else the PVD.
    let use_joliet = joliet.is_some();
    let vd = joliet
        .or(pvd)
        .ok_or("no Primary Volume Descriptor found in the image")?;

    let label = read_label(&bytes[vd..vd + SECTOR], use_joliet);
    let block_size = {
        let bs = read_u16_le(&bytes[vd..], 128) as u64;
        if bs == 0 {
            SECTOR as u64
        } else {
            bs
        }
    };
    let volume_blocks = read_u32_le(&bytes[vd..], 80) as u64;

    // The root directory record lives at offset 156 of the volume descriptor.
    let root_rec = vd + 156;
    let root_lba = read_u32_le(&bytes[root_rec..], 2);
    let root_len = read_u32_le(&bytes[root_rec..], 10);

    let mut visited = Vec::new();
    let mut count = 0usize;
    let root = walk_dir(
        bytes,
        root_lba,
        root_len,
        block_size,
        use_joliet,
        0,
        &mut visited,
        &mut count,
    );

    Ok(Iso {
        label,
        joliet: use_joliet,
        block_size,
        volume_blocks,
        root,
    })
}

/// Type-2 descriptor is Joliet if its escape-sequence field (offset 88) begins
/// with `%/@`, `%/C` or `%/E` (UCS-2 levels 1/2/3).
fn is_joliet(vd: &[u8]) -> bool {
    if vd.len() < 91 {
        return false;
    }
    vd[88] == 0x25 && vd[89] == 0x2F && matches!(vd[90], 0x40 | 0x43 | 0x45)
}

fn read_label(vd: &[u8], joliet: bool) -> String {
    // Volume identifier: 32 bytes at offset 40 (d-characters, or UCS-2 for Joliet).
    let raw = &vd[40..72];
    let label = if joliet {
        decode_ucs2(raw)
    } else {
        String::from_utf8_lossy(raw).into_owned()
    };
    let trimmed = label.trim_matches(|c: char| c == ' ' || c == '\0');
    if trimmed.is_empty() {
        "(unnamed)".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Read a directory extent and return its child nodes (recursively), sorted
/// directories-first then case-insensitively by name. Bounds- and cycle-guarded.
#[allow(clippy::too_many_arguments)]
fn walk_dir(
    bytes: &[u8],
    lba: u32,
    len: u32,
    block_size: u64,
    joliet: bool,
    depth: usize,
    visited: &mut Vec<u32>,
    count: &mut usize,
) -> Vec<Node> {
    let mut out = Vec::new();
    if depth >= MAX_DEPTH || *count >= MAX_ENTRIES {
        return out;
    }
    // Cycle guard: never re-enter a directory extent we already walked.
    if visited.contains(&lba) {
        return out;
    }
    visited.push(lba);

    let base = lba as usize * block_size as usize;
    let extent_len = len as usize;
    let end = match base.checked_add(extent_len) {
        Some(e) if e <= bytes.len() => e,
        _ => return out, // extent points outside the image — skip gracefully
    };
    let region = &bytes[base..end];

    let bs = block_size as usize;
    let mut pos = 0usize;
    while pos + 33 <= region.len() {
        let rec_len = region[pos] as usize;
        if rec_len == 0 {
            // No more records in this logical block — jump to the next one.
            let next = (pos / bs + 1) * bs;
            if next <= pos {
                break;
            }
            pos = next;
            continue;
        }
        if pos + rec_len > region.len() || rec_len < 33 {
            break;
        }
        let rec = &region[pos..pos + rec_len];

        let ext_lba = read_u32_le(rec, 2);
        let data_len = read_u32_le(rec, 10);
        let flags = rec[25];
        let is_dir = flags & 0x02 != 0;
        let fi_len = rec[32] as usize;

        // Skip hidden-flag entries? No — list everything, but always skip the
        // "." (0x00) and ".." (0x01) self/parent pseudo-entries.
        if 33 + fi_len <= rec.len() {
            let fi = &rec[33..33 + fi_len];
            let is_dot = fi_len == 1 && (fi[0] == 0 || fi[0] == 1);
            if !is_dot {
                let name = decode_name(fi, joliet, is_dir);
                if !name.is_empty() {
                    *count += 1;
                    let children = if is_dir {
                        walk_dir(
                            bytes,
                            ext_lba,
                            data_len,
                            block_size,
                            joliet,
                            depth + 1,
                            visited,
                            count,
                        )
                    } else {
                        Vec::new()
                    };
                    out.push(Node {
                        name,
                        is_dir,
                        size: data_len as u64,
                        children,
                    });
                }
            }
        }
        pos += rec_len;
    }

    out.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    out
}

/// Decode a file identifier to a display name, stripping the ISO 9660 `;version`
/// suffix and a trailing dot on extensionless names.
fn decode_name(fi: &[u8], joliet: bool, is_dir: bool) -> String {
    let mut name = if joliet {
        decode_ucs2(fi)
    } else {
        String::from_utf8_lossy(fi).into_owned()
    };
    if !is_dir {
        // "FILE.TXT;1" -> "FILE.TXT"
        if let Some(idx) = name.rfind(';') {
            name.truncate(idx);
        }
        // "FILE." -> "FILE" (extensionless ISO 9660 padding)
        if name.ends_with('.') {
            name.pop();
        }
    }
    name.trim_matches('\0').to_string()
}

/// Decode big-endian UCS-2 (Joliet) into a String, replacing invalid units.
fn decode_ucs2(b: &[u8]) -> String {
    let units: Vec<u16> = b
        .chunks_exact(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16_lossy(&units)
}

fn read_u16_le(b: &[u8], off: usize) -> u16 {
    if off + 2 > b.len() {
        return 0;
    }
    u16::from_le_bytes([b[off], b[off + 1]])
}

fn read_u32_le(b: &[u8], off: usize) -> u32 {
    if off + 4 > b.len() {
        return 0;
    }
    u32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

struct Counts {
    files: u64,
    dirs: u64,
    total_bytes: u64,
}

fn tally(nodes: &[Node], c: &mut Counts) {
    for n in nodes {
        if n.is_dir {
            c.dirs += 1;
            tally(&n.children, c);
        } else {
            c.files += 1;
            c.total_bytes += n.size;
        }
    }
}

fn render(iso: &Iso, fmt: Format) -> String {
    match fmt {
        Format::Summary => render_summary(iso),
        Format::List => render_list(iso),
        Format::Tree => render_tree(iso),
    }
}

fn render_summary(iso: &Iso) -> String {
    let mut c = Counts {
        files: 0,
        dirs: 0,
        total_bytes: 0,
    };
    tally(&iso.root, &mut c);
    let vol_bytes = iso.volume_blocks * iso.block_size;
    let standard = if iso.joliet {
        "ISO 9660 + Joliet"
    } else {
        "ISO 9660"
    };
    format!(
        "ISO 9660 image\n\
         ==============\n\
         Volume label:    {label}\n\
         Standard:        {standard}\n\
         Block size:      {bs} bytes\n\
         Volume size:     {blocks} blocks ({volh})\n\
         Directories:     {dirs}\n\
         Files:           {files}\n\
         Total file size: {totalh} ({total} bytes)",
        label = iso.label,
        standard = standard,
        bs = iso.block_size,
        blocks = iso.volume_blocks,
        volh = human(vol_bytes),
        dirs = c.dirs,
        files = c.files,
        totalh = human(c.total_bytes),
        total = c.total_bytes,
    )
}

fn render_list(iso: &Iso) -> String {
    let mut lines = Vec::new();
    list_into(&iso.root, "/", &mut lines);
    if lines.is_empty() {
        return format!("/  (volume: {}) — empty image", iso.label);
    }
    lines.join("\n")
}

fn list_into(nodes: &[Node], prefix: &str, out: &mut Vec<String>) {
    for n in nodes {
        if n.is_dir {
            let path = format!("{prefix}{}/", n.name);
            out.push(path.clone());
            list_into(&n.children, &path, out);
        } else {
            out.push(format!("{prefix}{}  ({})", n.name, human(n.size)));
        }
    }
}

fn render_tree(iso: &Iso) -> String {
    let mut out = String::new();
    out.push_str(&format!("/  (volume: {})\n", iso.label));
    tree_into(&iso.root, "", &mut out);
    // Trim the trailing newline for clean single-block output.
    if out.ends_with('\n') {
        out.pop();
    }
    out
}

fn tree_into(nodes: &[Node], prefix: &str, out: &mut String) {
    let last = nodes.len().saturating_sub(1);
    for (i, n) in nodes.iter().enumerate() {
        let is_last = i == last;
        let branch = if is_last { "└── " } else { "├── " };
        if n.is_dir {
            out.push_str(&format!("{prefix}{branch}{}/\n", n.name));
            let child_prefix = format!("{prefix}{}", if is_last { "    " } else { "│   " });
            tree_into(&n.children, &child_prefix, out);
        } else {
            out.push_str(&format!(
                "{prefix}{branch}{}  ({})\n",
                n.name,
                human(n.size)
            ));
        }
    }
}

/// Human-readable byte size (binary units, 1 decimal for KiB and up).
fn human(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut v = bytes as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    format!("{v:.1} {}", UNITS[u])
}

// ---------------------------------------------------------------------------
// base64 decode (standard + URL-safe alphabet, whitespace tolerant)
// ---------------------------------------------------------------------------

fn b64_decode(s: &str) -> Result<Vec<u8>, String> {
    fn val(c: u8) -> i16 {
        match c {
            b'A'..=b'Z' => (c - b'A') as i16,
            b'a'..=b'z' => (c - b'a' + 26) as i16,
            b'0'..=b'9' => (c - b'0' + 52) as i16,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => -1,
        }
    }
    let mut out = Vec::with_capacity(s.len() / 4 * 3 + 3);
    let mut buf: u32 = 0;
    let mut bits: u32 = 0;
    for &c in s.as_bytes() {
        if c == b'=' {
            break;
        }
        if c.is_ascii_whitespace() {
            continue;
        }
        let v = val(c);
        if v < 0 {
            return Err(format!(
                "invalid base64 input (unexpected character '{}') — paste the base64 encoding of an .iso image",
                c as char
            ));
        }
        buf = (buf << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal but valid ISO 9660 image in memory:
    /// - volume label "TESTDISC"
    /// - root directory with one file README.TXT (11 bytes) and one dir DOCS
    /// - DOCS contains one file A.TXT (4 bytes)
    fn build_iso() -> Vec<u8> {
        const BS: usize = 2048;
        // Layout (logical sectors): 16=PVD, 17=terminator, 18=root dir,
        // 19=DOCS dir, 20=README.TXT data, 21=A.TXT data.
        let root_lba = 18u32;
        let docs_lba = 19u32;
        let mut img = vec![0u8; 22 * BS];

        // ---- Primary Volume Descriptor at sector 16 ----
        let pvd = 16 * BS;
        img[pvd] = 1; // type = primary
        img[pvd + 1..pvd + 6].copy_from_slice(b"CD001");
        img[pvd + 6] = 1; // version
                          // volume identifier (offset 40, 32 bytes, space-padded)
        let label = b"TESTDISC";
        img[pvd + 40..pvd + 40 + label.len()].copy_from_slice(label);
        for b in &mut img[pvd + 40 + label.len()..pvd + 72] {
            *b = b' ';
        }
        // volume space size (offset 80, both-endian u32) = 22 blocks
        write_u32_both(&mut img, pvd + 80, 22);
        // logical block size (offset 128, both-endian u16) = 2048
        write_u16_both(&mut img, pvd + 128, 2048);
        // root directory record at offset 156
        write_dir_record(&mut img, pvd + 156, root_lba, BS as u32, true, &[0]);

        // ---- terminator at sector 17 ----
        let term = 17 * BS;
        img[term] = 255;
        img[term + 1..term + 6].copy_from_slice(b"CD001");
        img[term + 6] = 1;

        // ---- root directory extent at sector 18 ----
        let root = root_lba as usize * BS;
        let mut p = root;
        p += write_dir_record(&mut img, p, root_lba, BS as u32, true, &[0]); // "."
        p += write_dir_record(&mut img, p, root_lba, BS as u32, true, &[1]); // ".."
        p += write_dir_record(&mut img, p, docs_lba, 1 * BS as u32, true, b"DOCS"); // dir
        write_dir_record(&mut img, p, 20, 11, false, b"README.TXT;1"); // file, 11 bytes

        // ---- DOCS directory extent at sector 19 ----
        let docs = docs_lba as usize * BS;
        let mut q = docs;
        q += write_dir_record(&mut img, q, docs_lba, 1 * BS as u32, true, &[0]); // "."
        q += write_dir_record(&mut img, q, root_lba, 2 * BS as u32, true, &[1]); // ".."
        write_dir_record(&mut img, q, 21, 4, false, b"A.TXT;1"); // file, 4 bytes

        img
    }

    fn write_u16_both(img: &mut [u8], off: usize, v: u16) {
        img[off..off + 2].copy_from_slice(&v.to_le_bytes());
        img[off + 2..off + 4].copy_from_slice(&v.to_be_bytes());
    }
    fn write_u32_both(img: &mut [u8], off: usize, v: u32) {
        img[off..off + 4].copy_from_slice(&v.to_le_bytes());
        img[off + 4..off + 8].copy_from_slice(&v.to_be_bytes());
    }

    /// Write a directory record, return its byte length.
    fn write_dir_record(
        img: &mut [u8],
        off: usize,
        lba: u32,
        data_len: u32,
        is_dir: bool,
        name: &[u8],
    ) -> usize {
        let fi_len = name.len();
        let mut rec_len = 33 + fi_len;
        if rec_len % 2 == 1 {
            rec_len += 1; // pad to even
        }
        img[off] = rec_len as u8;
        img[off + 1] = 0; // ext attr len
        img[off + 2..off + 6].copy_from_slice(&lba.to_le_bytes());
        img[off + 6..off + 10].copy_from_slice(&lba.to_be_bytes());
        img[off + 10..off + 14].copy_from_slice(&data_len.to_le_bytes());
        img[off + 14..off + 18].copy_from_slice(&data_len.to_be_bytes());
        img[off + 25] = if is_dir { 0x02 } else { 0x00 };
        img[off + 32] = fi_len as u8;
        img[off + 33..off + 33 + fi_len].copy_from_slice(name);
        rec_len
    }

    fn b64(bytes: &[u8]) -> String {
        const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut s = String::new();
        for chunk in bytes.chunks(3) {
            let b = [
                chunk[0],
                *chunk.get(1).unwrap_or(&0),
                *chunk.get(2).unwrap_or(&0),
            ];
            let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
            s.push(A[(n >> 18 & 63) as usize] as char);
            s.push(A[(n >> 12 & 63) as usize] as char);
            s.push(if chunk.len() > 1 {
                A[(n >> 6 & 63) as usize] as char
            } else {
                '='
            });
            s.push(if chunk.len() > 2 {
                A[(n & 63) as usize] as char
            } else {
                '='
            });
        }
        s
    }

    #[test]
    fn parses_tree_and_names() {
        let img = build_iso();
        let out = run(&b64(&img), "tree").unwrap();
        assert!(out.contains("(volume: TESTDISC)"), "{out}");
        assert!(out.contains("DOCS/"), "{out}");
        assert!(out.contains("README.TXT  (11 B)"), "{out}");
        assert!(out.contains("A.TXT  (4 B)"), "{out}");
        // version suffix stripped
        assert!(!out.contains(";1"), "{out}");
        // directories sort before files
        let d = out.find("DOCS").unwrap();
        let r = out.find("README").unwrap();
        assert!(d < r, "dirs should sort first:\n{out}");
    }

    #[test]
    fn summary_counts() {
        let img = build_iso();
        let out = run(&b64(&img), "summary").unwrap();
        assert!(out.contains("Volume label:    TESTDISC"), "{out}");
        assert!(out.contains("Directories:     1"), "{out}");
        assert!(out.contains("Files:           2"), "{out}");
        assert!(out.contains("Block size:      2048 bytes"), "{out}");
        // 11 + 4 = 15 total file bytes
        assert!(out.contains("15 bytes"), "{out}");
    }

    #[test]
    fn list_paths() {
        let img = build_iso();
        let out = run(&b64(&img), "list").unwrap();
        assert!(out.contains("/DOCS/"), "{out}");
        assert!(out.contains("/DOCS/A.TXT  (4 B)"), "{out}");
        assert!(out.contains("/README.TXT  (11 B)"), "{out}");
    }

    #[test]
    fn rejects_non_iso() {
        let junk = vec![0u8; 40 * 2048];
        let err = run(&b64(&junk), "tree").unwrap_err();
        assert!(err.contains("CD001"), "{err}");
    }

    #[test]
    fn rejects_too_small() {
        let err = run(&b64(b"hello"), "tree").unwrap_err();
        assert!(err.contains("not an ISO 9660"), "{err}");
    }

    #[test]
    fn empty_input_errors() {
        assert!(run("   ", "tree").unwrap_err().contains("no input"));
    }

    #[test]
    fn bad_format_errors() {
        let img = build_iso();
        assert!(run(&b64(&img), "bogus")
            .unwrap_err()
            .contains("unknown format"));
    }

    #[test]
    fn human_sizes() {
        assert_eq!(human(0), "0 B");
        assert_eq!(human(512), "512 B");
        assert_eq!(human(1024), "1.0 KiB");
        assert_eq!(human(1536), "1.5 KiB");
        assert_eq!(human(1024 * 1024), "1.0 MiB");
    }
}

//! tar-archive-lister core — pure compute, shared by the chat skill block and the web page.
//!
//! Enumerates the members of a tar archive (optionally gzip-compressed) straight
//! from its header blocks: path, byte size, permission mode, owner/group, entry
//! type, modification time and link target — WITHOUT unpacking any file payload.
//!
//! The parser walks the 512-byte header blocks itself (no `tar` crate) so it can
//! surface every ustar/GNU/PAX field the listing advertises, and so it compiles
//! unchanged to wasm32-wasip1 (chat/CLI) and wasm32-unknown-unknown (page).
//!
//! Supported header dialects: v7, ustar (POSIX.1-1988, incl. the `prefix` field),
//! GNU (`L`/`K` long name / long link, base-256 numeric fields) and PAX
//! (POSIX.1-2001 `x` per-entry and `g` global extended headers).

/// Hard cap on the decoded (and, for gzip, decompressed) archive size.
pub const MAX_BYTES: usize = 64 * 1024 * 1024;
/// Hard cap on the number of members parsed out of one archive.
pub const MAX_ENTRIES: usize = 200_000;

const BLOCK: usize = 512;

/// One archive member, exactly as recorded in its tar header.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// Full member path (PAX `path=` / GNU `L` / ustar `prefix` + `name`).
    pub path: String,
    /// Entry kind: `file`, `directory`, `symlink`, `hardlink`, `chardev`,
    /// `blockdev`, `fifo`, `contiguous` or `unknown`.
    pub kind: &'static str,
    /// Payload size in bytes (0 for directories and link entries).
    pub size: u64,
    /// Permission bits (the low 12 bits of the mode field).
    pub mode: u32,
    pub uid: u64,
    pub gid: u64,
    /// Owner name from the ustar header (empty on v7 archives).
    pub uname: String,
    /// Group name from the ustar header (empty on v7 archives).
    pub gname: String,
    /// Modification time, seconds since the Unix epoch.
    pub mtime: i64,
    /// Target of a symlink/hardlink entry, else empty.
    pub link_target: String,
    /// `major,minor` for character/block devices, else empty.
    pub device: String,
    /// Byte offset of this member's header block inside the (decompressed) tar.
    pub offset: u64,
}

/// Everything the listing knows about one archive.
#[derive(Debug, Clone, PartialEq)]
pub struct Listing {
    /// `tar` or `tar.gz` — how the input bytes were framed.
    pub container: &'static str,
    /// Size of the tar stream in bytes (after gzip decompression).
    pub archive_bytes: u64,
    /// Every member parsed, before `filter`/`include_dirs`/`limit` are applied.
    pub total_entries: usize,
    /// Members left after filtering, before `limit` truncation.
    pub matched_entries: usize,
    /// The members actually returned.
    pub entries: Vec<Entry>,
    /// Sum of `size` over the matched members.
    pub total_size: u64,
}

/// List the members of a tar (or tar.gz) archive.
///
/// - `input`: the archive bytes as a base64 or hex string.
/// - `input_format`: `"base64"` (default) or `"hex"`; blank means base64.
/// - `output`: `"table"` (default), `"paths"`, `"csv"` or `"json"`.
/// - `sort`: `"archive"` (default), `"path"`, `"size"`, `"mtime"` or `"type"`.
/// - `filter`: blank for everything, else a `*`/`?` glob (or a plain substring
///   when the pattern has no wildcard) matched against the member path.
/// - `include_dirs`: when false, directory members are dropped from the listing.
/// - `time_format`: `"iso"` (default), `"epoch"` or `"none"`.
/// - `limit`: maximum members to return (1..=200000).
pub fn run(
    input: &str,
    input_format: &str,
    output: &str,
    sort: &str,
    filter: &str,
    include_dirs: bool,
    time_format: &str,
    limit: usize,
) -> Result<String, String> {
    let listing = list(
        input,
        input_format,
        sort,
        filter,
        include_dirs,
        limit,
    )?;
    render(&listing, output, time_format)
}

/// Decode, decompress and parse the archive, then filter/sort/truncate it.
pub fn list(
    input: &str,
    input_format: &str,
    sort: &str,
    filter: &str,
    include_dirs: bool,
    limit: usize,
) -> Result<Listing, String> {
    if !matches!(sort, "" | "archive" | "path" | "size" | "mtime" | "type") {
        return Err(format!(
            "invalid sort {sort:?}: expected \"archive\", \"path\", \"size\", \"mtime\" or \"type\""
        ));
    }
    if limit == 0 || limit > MAX_ENTRIES {
        return Err(format!(
            "invalid limit {limit}: expected 1..={MAX_ENTRIES}"
        ));
    }

    let raw = decode_bytes(input, input_format)?;
    let (bytes, container) = decompress(raw)?;
    if bytes.len() < BLOCK {
        return Err(format!(
            "archive is only {} byte(s): a tar archive is made of 512-byte blocks, so the smallest valid one is 512 bytes",
            bytes.len()
        ));
    }

    let all = parse_tar(&bytes)?;
    let total_entries = all.len();

    let matcher = Matcher::new(filter);
    let mut kept: Vec<Entry> = all
        .into_iter()
        .filter(|e| include_dirs || e.kind != "directory")
        .filter(|e| matcher.matches(&e.path))
        .collect();

    match sort {
        "path" => kept.sort_by(|a, b| a.path.cmp(&b.path)),
        "size" => kept.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.path.cmp(&b.path))),
        "mtime" => kept.sort_by(|a, b| b.mtime.cmp(&a.mtime).then_with(|| a.path.cmp(&b.path))),
        "type" => kept.sort_by(|a, b| a.kind.cmp(b.kind).then_with(|| a.path.cmp(&b.path))),
        _ => {}
    }

    let matched_entries = kept.len();
    let total_size = kept.iter().map(|e| e.size).sum();
    kept.truncate(limit);

    Ok(Listing {
        container,
        archive_bytes: bytes.len() as u64,
        total_entries,
        matched_entries,
        entries: kept,
        total_size,
    })
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn render(l: &Listing, output: &str, time_format: &str) -> Result<String, String> {
    if !matches!(time_format, "" | "iso" | "epoch" | "none") {
        return Err(format!(
            "invalid time_format {time_format:?}: expected \"iso\", \"epoch\" or \"none\""
        ));
    }
    match output {
        "" | "table" => Ok(render_table(l, time_format)),
        "paths" => Ok(render_paths(l)),
        "csv" => Ok(render_csv(l, time_format)),
        "json" => Ok(render_json(l, time_format)),
        other => Err(format!(
            "invalid output {other:?}: expected \"table\", \"paths\", \"csv\" or \"json\""
        )),
    }
}

fn fmt_time(mtime: i64, time_format: &str) -> String {
    match time_format {
        "epoch" => mtime.to_string(),
        "none" => String::new(),
        _ => iso_utc(mtime),
    }
}

fn render_table(l: &Listing, time_format: &str) -> String {
    let mut lines = Vec::with_capacity(l.entries.len() + 2);
    if l.entries.is_empty() {
        lines.push(summary_line(l));
        return lines.join("\n");
    }

    let owners: Vec<String> = l.entries.iter().map(owner_field).collect();
    let sizes: Vec<String> = l.entries.iter().map(|e| e.size.to_string()).collect();
    let times: Vec<String> = l
        .entries
        .iter()
        .map(|e| fmt_time(e.mtime, time_format))
        .collect();
    let ow = owners.iter().map(|s| s.len()).max().unwrap_or(0);
    let sw = sizes.iter().map(|s| s.len()).max().unwrap_or(0);
    let tw = times.iter().map(|s| s.len()).max().unwrap_or(0);

    for (i, e) in l.entries.iter().enumerate() {
        let mut line = format!(
            "{} {:<ow$} {:>sw$}",
            mode_string(e.kind, e.mode),
            owners[i],
            sizes[i],
            ow = ow,
            sw = sw
        );
        if tw > 0 {
            line.push_str(&format!(" {:<tw$}", times[i], tw = tw));
        }
        line.push(' ');
        line.push_str(&e.path);
        if !e.link_target.is_empty() {
            line.push_str(if e.kind == "hardlink" { " link to " } else { " -> " });
            line.push_str(&e.link_target);
        }
        if !e.device.is_empty() {
            line.push_str(&format!(" (device {})", e.device));
        }
        lines.push(line);
    }
    lines.push(String::new());
    lines.push(summary_line(l));
    lines.join("\n")
}

fn render_paths(l: &Listing) -> String {
    if l.entries.is_empty() {
        return String::new();
    }
    l.entries
        .iter()
        .map(|e| e.path.clone())
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_csv(l: &Listing, time_format: &str) -> String {
    let mut out =
        String::from("path,type,size,mode,uid,gid,uname,gname,mtime,link_target,offset\n");
    for e in &l.entries {
        let row = [
            e.path.clone(),
            e.kind.to_string(),
            e.size.to_string(),
            format!("{:04o}", e.mode),
            e.uid.to_string(),
            e.gid.to_string(),
            e.uname.clone(),
            e.gname.clone(),
            fmt_time(e.mtime, time_format),
            e.link_target.clone(),
            e.offset.to_string(),
        ];
        out.push_str(
            &row.iter()
                .map(|f| csv_field(f))
                .collect::<Vec<_>>()
                .join(","),
        );
        out.push('\n');
    }
    out
}

fn render_json(l: &Listing, time_format: &str) -> String {
    let mut out = String::from("{\n");
    out.push_str(&format!("  \"container\": \"{}\",\n", l.container));
    out.push_str(&format!("  \"archive_bytes\": {},\n", l.archive_bytes));
    out.push_str(&format!("  \"total_entries\": {},\n", l.total_entries));
    out.push_str(&format!("  \"matched_entries\": {},\n", l.matched_entries));
    out.push_str(&format!("  \"listed_entries\": {},\n", l.entries.len()));
    out.push_str(&format!("  \"total_size\": {},\n", l.total_size));
    out.push_str("  \"entries\": [");
    if l.entries.is_empty() {
        out.push_str("]\n}");
        return out;
    }
    out.push('\n');
    for (i, e) in l.entries.iter().enumerate() {
        out.push_str("    {\n");
        out.push_str(&format!("      \"path\": {},\n", json_str(&e.path)));
        out.push_str(&format!("      \"type\": \"{}\",\n", e.kind));
        out.push_str(&format!("      \"size\": {},\n", e.size));
        out.push_str(&format!("      \"mode\": \"{:04o}\",\n", e.mode));
        out.push_str(&format!(
            "      \"mode_string\": \"{}\",\n",
            mode_string(e.kind, e.mode)
        ));
        out.push_str(&format!("      \"uid\": {},\n", e.uid));
        out.push_str(&format!("      \"gid\": {},\n", e.gid));
        out.push_str(&format!("      \"uname\": {},\n", json_str(&e.uname)));
        out.push_str(&format!("      \"gname\": {},\n", json_str(&e.gname)));
        let t = fmt_time(e.mtime, time_format);
        if t.is_empty() {
            out.push_str("      \"mtime\": null,\n");
        } else if time_format == "epoch" {
            out.push_str(&format!("      \"mtime\": {t},\n"));
        } else {
            out.push_str(&format!("      \"mtime\": {},\n", json_str(&t)));
        }
        out.push_str(&format!(
            "      \"link_target\": {},\n",
            json_str(&e.link_target)
        ));
        out.push_str(&format!("      \"device\": {},\n", json_str(&e.device)));
        out.push_str(&format!("      \"offset\": {}\n", e.offset));
        out.push_str(if i + 1 == l.entries.len() {
            "    }\n"
        } else {
            "    },\n"
        });
    }
    out.push_str("  ]\n}");
    out
}

fn summary_line(l: &Listing) -> String {
    let files = l.entries.iter().filter(|e| e.kind == "file").count();
    let dirs = l.entries.iter().filter(|e| e.kind == "directory").count();
    let other = l.entries.len() - files - dirs;
    let mut s = format!(
        "{} of {} member(s) listed ({} file(s), {} director(y/ies)",
        l.entries.len(),
        l.total_entries,
        files,
        dirs
    );
    if other > 0 {
        s.push_str(&format!(", {other} other"));
    }
    s.push_str(&format!(
        ") — {} byte(s) of content in a {} {} stream",
        l.total_size, l.archive_bytes, l.container
    ));
    s
}

fn owner_field(e: &Entry) -> String {
    let u = if e.uname.is_empty() {
        e.uid.to_string()
    } else {
        e.uname.clone()
    };
    let g = if e.gname.is_empty() {
        e.gid.to_string()
    } else {
        e.gname.clone()
    };
    format!("{u}/{g}")
}

/// `drwxr-xr-x`-style mode string (type char + rwx triplets + setuid/setgid/sticky).
fn mode_string(kind: &str, mode: u32) -> String {
    let type_char = match kind {
        "directory" => 'd',
        "symlink" => 'l',
        "hardlink" => 'h',
        "chardev" => 'c',
        "blockdev" => 'b',
        "fifo" => 'p',
        _ => '-',
    };
    let mut s = String::with_capacity(10);
    s.push(type_char);
    for (shift, special) in [(6u32, 0o4000u32), (3, 0o2000), (0, 0o1000)] {
        let bits = (mode >> shift) & 0o7;
        s.push(if bits & 0o4 != 0 { 'r' } else { '-' });
        s.push(if bits & 0o2 != 0 { 'w' } else { '-' });
        let x = bits & 0o1 != 0;
        let sp = mode & special != 0;
        s.push(match (sp, x, shift) {
            (true, true, 0) => 't',
            (true, false, 0) => 'T',
            (true, true, _) => 's',
            (true, false, _) => 'S',
            (false, true, _) => 'x',
            (false, false, _) => '-',
        });
    }
    s
}

fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

// ---------------------------------------------------------------------------
// Path filtering
// ---------------------------------------------------------------------------

enum Matcher {
    All,
    Substring(String),
    Glob(Vec<u8>),
}

impl Matcher {
    fn new(pattern: &str) -> Self {
        let p = pattern.trim();
        if p.is_empty() {
            Matcher::All
        } else if p.contains('*') || p.contains('?') {
            Matcher::Glob(p.as_bytes().to_vec())
        } else {
            Matcher::Substring(p.to_string())
        }
    }

    fn matches(&self, path: &str) -> bool {
        match self {
            Matcher::All => true,
            Matcher::Substring(s) => path.contains(s.as_str()),
            Matcher::Glob(p) => glob_match(p, path.as_bytes()),
        }
    }
}

/// `*` matches any run of characters (including `/`), `?` matches exactly one.
fn glob_match(pat: &[u8], text: &[u8]) -> bool {
    let (mut p, mut t) = (0usize, 0usize);
    let (mut star, mut mark) = (usize::MAX, 0usize);
    while t < text.len() {
        if p < pat.len() && (pat[p] == b'?' || pat[p] == text[t]) {
            p += 1;
            t += 1;
        } else if p < pat.len() && pat[p] == b'*' {
            star = p;
            mark = t;
            p += 1;
        } else if star != usize::MAX {
            p = star + 1;
            mark += 1;
            t = mark;
        } else {
            return false;
        }
    }
    while p < pat.len() && pat[p] == b'*' {
        p += 1;
    }
    p == pat.len()
}

// ---------------------------------------------------------------------------
// Container detection + gzip
// ---------------------------------------------------------------------------

fn decompress(raw: Vec<u8>) -> Result<(Vec<u8>, &'static str), String> {
    if raw.len() >= 2 && raw[0] == 0x1f && raw[1] == 0x8b {
        use std::io::Read;
        let mut out = Vec::new();
        flate2::read::MultiGzDecoder::new(&raw[..])
            .take(MAX_BYTES as u64 + 1)
            .read_to_end(&mut out)
            .map_err(|e| format!("gzip decompression failed: {e}"))?;
        if out.len() > MAX_BYTES {
            return Err(format!(
                "decompressed archive exceeds the {} MiB limit",
                MAX_BYTES / (1024 * 1024)
            ));
        }
        return Ok((out, "tar.gz"));
    }
    if raw.starts_with(b"BZh") {
        return Err("this looks like a bzip2 stream (.tar.bz2), which this tool does not decompress: decompress it first, then list the plain .tar".into());
    }
    if raw.starts_with(&[0xfd, b'7', b'z', b'X', b'Z', 0x00]) {
        return Err("this looks like an xz stream (.tar.xz), which this tool does not decompress: decompress it first, then list the plain .tar".into());
    }
    if raw.starts_with(&[0x28, 0xb5, 0x2f, 0xfd]) {
        return Err("this looks like a zstd stream (.tar.zst), which this tool does not decompress: decompress it first, then list the plain .tar".into());
    }
    if raw.starts_with(b"PK\x03\x04") || raw.starts_with(b"PK\x05\x06") {
        return Err("this looks like a ZIP archive, not a tar archive".into());
    }
    if raw.len() > MAX_BYTES {
        return Err(format!(
            "archive exceeds the {} MiB limit",
            MAX_BYTES / (1024 * 1024)
        ));
    }
    Ok((raw, "tar"))
}

// ---------------------------------------------------------------------------
// tar header parsing
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Pending {
    long_name: Option<String>,
    long_link: Option<String>,
    pax: Vec<(String, String)>,
}

fn parse_tar(bytes: &[u8]) -> Result<Vec<Entry>, String> {
    let mut entries = Vec::new();
    let mut global: Vec<(String, String)> = Vec::new();
    let mut pending = Pending::default();
    let mut pos = 0usize;
    let mut saw_header = false;
    let mut zero_run = 0usize;

    while pos + BLOCK <= bytes.len() {
        let hdr = &bytes[pos..pos + BLOCK];
        if hdr.iter().all(|&b| b == 0) {
            zero_run += 1;
            pos += BLOCK;
            if zero_run >= 2 {
                break;
            }
            continue;
        }
        zero_run = 0;

        if !checksum_ok(hdr) {
            if saw_header {
                return Err(format!(
                    "corrupt tar header at byte offset {pos}: the header checksum does not match (the archive is truncated or damaged)"
                ));
            }
            return Err("not a tar archive: the first 512-byte block is not a valid tar header (checksum mismatch). If the file is .tar.bz2/.tar.xz/.tar.zst, decompress it first; .tar.gz is handled automatically.".into());
        }
        saw_header = true;

        let typeflag = hdr[156];
        let size = numeric(&hdr[124..136])
            .ok_or_else(|| format!("corrupt tar header at byte offset {pos}: unreadable size field"))?;
        let data_start = pos + BLOCK;
        let padded = (size as usize).div_ceil(BLOCK) * BLOCK;
        if data_start + (size as usize) > bytes.len() {
            return Err(format!(
                "truncated tar archive: the member at byte offset {pos} declares {size} byte(s) of content but only {} byte(s) remain",
                bytes.len().saturating_sub(data_start)
            ));
        }
        let payload = &bytes[data_start..data_start + size as usize];

        match typeflag {
            b'L' => {
                pending.long_name = Some(cstr(payload));
                pos = data_start + padded;
                continue;
            }
            b'K' => {
                pending.long_link = Some(cstr(payload));
                pos = data_start + padded;
                continue;
            }
            b'x' | b'X' => {
                pending.pax = parse_pax(payload);
                pos = data_start + padded;
                continue;
            }
            b'g' => {
                global = parse_pax(payload);
                pos = data_start + padded;
                continue;
            }
            _ => {}
        }

        let mut entry = build_entry(hdr, pos, size, typeflag);
        apply_pax(&mut entry, &global);
        if let Some(n) = pending.long_name.take() {
            entry.path = n;
        }
        if let Some(l) = pending.long_link.take() {
            entry.link_target = l;
        }
        let pax = std::mem::take(&mut pending.pax);
        apply_pax(&mut entry, &pax);

        entries.push(entry);
        if entries.len() > MAX_ENTRIES {
            return Err(format!(
                "archive has more than {MAX_ENTRIES} members, which is over this tool's limit"
            ));
        }
        pos = data_start + padded;
    }

    if !saw_header {
        return Err("no tar members found: the archive contains only end-of-archive padding".into());
    }
    Ok(entries)
}

fn build_entry(hdr: &[u8], offset: usize, size: u64, typeflag: u8) -> Entry {
    let name = cstr(&hdr[0..100]);
    let ustar = &hdr[257..263] == b"ustar\0" || &hdr[257..262] == b"ustar";
    let prefix = if ustar { cstr(&hdr[345..500]) } else { String::new() };
    let path = if prefix.is_empty() {
        name
    } else {
        format!("{prefix}/{name}")
    };

    let kind = match typeflag {
        b'0' | b'\0' | b'7' if path.ends_with('/') => "directory",
        b'0' | b'\0' => "file",
        b'1' => "hardlink",
        b'2' => "symlink",
        b'3' => "chardev",
        b'4' => "blockdev",
        b'5' => "directory",
        b'6' => "fifo",
        b'7' => "contiguous",
        _ => "unknown",
    };

    let device = if matches!(kind, "chardev" | "blockdev") {
        let major = numeric(&hdr[329..337]).unwrap_or(0);
        let minor = numeric(&hdr[337..345]).unwrap_or(0);
        format!("{major},{minor}")
    } else {
        String::new()
    };

    Entry {
        path,
        kind,
        size: if matches!(kind, "directory" | "symlink" | "hardlink" | "fifo") {
            0
        } else {
            size
        },
        mode: (numeric(&hdr[100..108]).unwrap_or(0) & 0o7777) as u32,
        uid: numeric(&hdr[108..116]).unwrap_or(0),
        gid: numeric(&hdr[116..124]).unwrap_or(0),
        uname: if ustar { cstr(&hdr[265..297]) } else { String::new() },
        gname: if ustar { cstr(&hdr[297..329]) } else { String::new() },
        mtime: numeric(&hdr[136..148]).unwrap_or(0) as i64,
        link_target: cstr(&hdr[157..257]),
        device,
        offset: offset as u64,
    }
}

fn apply_pax(entry: &mut Entry, kv: &[(String, String)]) {
    for (k, v) in kv {
        match k.as_str() {
            "path" => entry.path = v.clone(),
            "linkpath" => entry.link_target = v.clone(),
            "size" => {
                if let Ok(n) = v.parse::<u64>() {
                    entry.size = n;
                }
            }
            "uid" => {
                if let Ok(n) = v.parse::<u64>() {
                    entry.uid = n;
                }
            }
            "gid" => {
                if let Ok(n) = v.parse::<u64>() {
                    entry.gid = n;
                }
            }
            "uname" => entry.uname = v.clone(),
            "gname" => entry.gname = v.clone(),
            "mtime" => {
                if let Ok(f) = v.parse::<f64>() {
                    entry.mtime = f as i64;
                }
            }
            _ => {}
        }
    }
}

/// PAX extended header records: `"<len> <key>=<value>\n"`, repeated.
fn parse_pax(payload: &[u8]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < payload.len() {
        let sp = match payload[i..].iter().position(|&b| b == b' ') {
            Some(p) => i + p,
            None => break,
        };
        let len: usize = match std::str::from_utf8(&payload[i..sp]).ok().and_then(|s| s.parse().ok()) {
            Some(n) if n > sp - i && i + n <= payload.len() => n,
            _ => break,
        };
        let record = &payload[sp + 1..i + len];
        let record = record.strip_suffix(b"\n").unwrap_or(record);
        if let Some(eq) = record.iter().position(|&b| b == b'=') {
            out.push((
                String::from_utf8_lossy(&record[..eq]).into_owned(),
                String::from_utf8_lossy(&record[eq + 1..]).into_owned(),
            ));
        }
        i += len;
    }
    out
}

/// Header checksum: sum of all bytes with the checksum field read as spaces.
/// Accepts both the unsigned (standard) and signed (historic) interpretations.
fn checksum_ok(hdr: &[u8]) -> bool {
    let stored = match numeric(&hdr[148..156]) {
        Some(n) => n,
        None => return false,
    };
    let mut unsigned: u64 = 0;
    let mut signed: i64 = 0;
    for (i, &b) in hdr.iter().enumerate() {
        let v = if (148..156).contains(&i) { b' ' } else { b };
        unsigned += v as u64;
        signed += v as i8 as i64;
    }
    stored == unsigned || stored as i64 == signed
}

/// Octal (space/NUL padded) or GNU base-256 numeric header field.
fn numeric(field: &[u8]) -> Option<u64> {
    if field.is_empty() {
        return None;
    }
    if field[0] & 0x80 != 0 {
        // GNU base-256: the low 7 bits of the first byte plus the rest, big-endian.
        let mut v: u64 = (field[0] & 0x7f) as u64;
        for &b in &field[1..] {
            v = v.wrapping_shl(8) | b as u64;
        }
        return Some(v);
    }
    let text: String = field
        .iter()
        .take_while(|&&b| b != 0)
        .map(|&b| b as char)
        .collect();
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Some(0);
    }
    u64::from_str_radix(trimmed, 8).ok()
}

fn cstr(field: &[u8]) -> String {
    let end = field.iter().position(|&b| b == 0).unwrap_or(field.len());
    String::from_utf8_lossy(&field[..end]).trim_end().to_string()
}

// ---------------------------------------------------------------------------
// Time formatting (no chrono: the page target has no std clock/tz database)
// ---------------------------------------------------------------------------

/// Unix seconds → `YYYY-MM-DD HH:MM:SS` in UTC.
fn iso_utc(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// Howard Hinnant's `civil_from_days` — days since 1970-01-01 → (y, m, d).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

// ---------------------------------------------------------------------------
// Byte decoding (base64 / hex)
// ---------------------------------------------------------------------------

fn decode_bytes(input: &str, input_format: &str) -> Result<Vec<u8>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("input is empty: paste the tar (or tar.gz) archive bytes as base64 or hex".into());
    }
    match input_format {
        "" | "base64" => decode_base64(trimmed),
        "hex" => decode_hex(trimmed),
        other => Err(format!(
            "invalid input_format {other:?}: expected \"base64\" or \"hex\""
        )),
    }
}

fn decode_hex(s: &str) -> Result<Vec<u8>, String> {
    let compact: String = s
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ':' && *c != '-')
        .collect();
    if compact.len() % 2 != 0 {
        return Err("invalid hex: odd number of digits".into());
    }
    let bytes = compact.as_bytes();
    let mut out = Vec::with_capacity(compact.len() / 2);
    for pair in bytes.chunks(2) {
        let hi = hex_val(pair[0])?;
        let lo = hex_val(pair[1])?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

fn hex_val(c: u8) -> Result<u8, String> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => Err(format!("invalid hex digit {:?}", c as char)),
    }
}

/// Standard + URL-safe base64, padding optional.
fn decode_base64(s: &str) -> Result<Vec<u8>, String> {
    const INVALID: u8 = 255;
    let val = |c: u8| -> u8 {
        match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => INVALID,
        }
    };
    let mut buf = 0u32;
    let mut bits = 0u32;
    let mut out = Vec::new();
    for &c in s.as_bytes() {
        if c == b'=' || c.is_ascii_whitespace() {
            continue;
        }
        let v = val(c);
        if v == INVALID {
            return Err(format!("invalid base64 character {:?}", c as char));
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a ustar header block for one member.
    fn header(
        name: &str,
        mode: u32,
        uid: u64,
        gid: u64,
        size: u64,
        mtime: i64,
        typeflag: u8,
        link: &str,
        uname: &str,
        gname: &str,
    ) -> Vec<u8> {
        let mut h = vec![0u8; BLOCK];
        let put = |h: &mut Vec<u8>, at: usize, s: &str| {
            h[at..at + s.len()].copy_from_slice(s.as_bytes());
        };
        put(&mut h, 0, name);
        put(&mut h, 100, &format!("{mode:07o}\0"));
        put(&mut h, 108, &format!("{uid:07o}\0"));
        put(&mut h, 116, &format!("{gid:07o}\0"));
        put(&mut h, 124, &format!("{size:011o}\0"));
        put(&mut h, 136, &format!("{mtime:011o}\0"));
        h[156] = typeflag;
        put(&mut h, 157, link);
        put(&mut h, 257, "ustar\0");
        put(&mut h, 263, "00");
        put(&mut h, 265, uname);
        put(&mut h, 297, gname);
        // checksum: spaces first, then the octal sum
        for b in h.iter_mut().skip(148).take(8) {
            *b = b' ';
        }
        let sum: u64 = h.iter().map(|&b| b as u64).sum();
        put(&mut h, 148, &format!("{sum:06o}\0 "));
        h
    }

    fn member(h: Vec<u8>, data: &[u8]) -> Vec<u8> {
        let mut v = h;
        v.extend_from_slice(data);
        let pad = (BLOCK - data.len() % BLOCK) % BLOCK;
        v.extend(std::iter::repeat(0u8).take(pad));
        v
    }

    /// A small archive: a dir, a file, and a symlink.
    fn sample_tar() -> Vec<u8> {
        let mut t = Vec::new();
        t.extend(member(
            header("docs/", 0o755, 1000, 1000, 0, 1_714_557_600, b'5', "", "alice", "staff"),
            b"",
        ));
        t.extend(member(
            header(
                "docs/hello.txt",
                0o644,
                1000,
                1000,
                12,
                1_714_557_600,
                b'0',
                "",
                "alice",
                "staff",
            ),
            b"hello world\n",
        ));
        t.extend(member(
            header(
                "docs/link.txt",
                0o777,
                1000,
                1000,
                0,
                1_714_557_600,
                b'2',
                "hello.txt",
                "alice",
                "staff",
            ),
            b"",
        ));
        t.extend(vec![0u8; BLOCK * 2]);
        t
    }

    fn b64(bytes: &[u8]) -> String {
        const TABLE: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for chunk in bytes.chunks(3) {
            let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
            let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
            out.push(TABLE[(n >> 18) as usize & 63] as char);
            out.push(TABLE[(n >> 12) as usize & 63] as char);
            out.push(if chunk.len() > 1 {
                TABLE[(n >> 6) as usize & 63] as char
            } else {
                '='
            });
            out.push(if chunk.len() > 2 {
                TABLE[n as usize & 63] as char
            } else {
                '='
            });
        }
        out
    }

    #[test]
    fn lists_members_as_a_table() {
        let out = run(&b64(&sample_tar()), "base64", "table", "archive", "", true, "iso", 500)
            .unwrap();
        let expected = "\
drwxr-xr-x alice/staff  0 2024-05-01 10:00:00 docs/
-rw-r--r-- alice/staff 12 2024-05-01 10:00:00 docs/hello.txt
lrwxrwxrwx alice/staff  0 2024-05-01 10:00:00 docs/link.txt -> hello.txt

3 of 3 member(s) listed (1 file(s), 1 director(y/ies), 1 other) — 12 byte(s) of content in a 3072 tar stream";
        assert_eq!(out, expected);
    }

    #[test]
    fn paths_output_lists_one_path_per_line() {
        let out =
            run(&b64(&sample_tar()), "base64", "paths", "archive", "", true, "iso", 500).unwrap();
        assert_eq!(out, "docs/\ndocs/hello.txt\ndocs/link.txt");
    }

    #[test]
    fn json_output_carries_every_header_field() {
        let out =
            run(&b64(&sample_tar()), "base64", "json", "path", "hello", true, "epoch", 500)
                .unwrap();
        assert!(out.contains("\"path\": \"docs/hello.txt\""), "{out}");
        assert!(out.contains("\"type\": \"file\""), "{out}");
        assert!(out.contains("\"size\": 12"), "{out}");
        assert!(out.contains("\"mode\": \"0644\""), "{out}");
        assert!(out.contains("\"mode_string\": \"-rw-r--r--\""), "{out}");
        assert!(out.contains("\"uname\": \"alice\""), "{out}");
        assert!(out.contains("\"gname\": \"staff\""), "{out}");
        assert!(out.contains("\"mtime\": 1714557600"), "{out}");
        assert!(out.contains("\"matched_entries\": 1"), "{out}");
        assert!(out.contains("\"offset\": 512"), "{out}");
    }

    #[test]
    fn csv_output_has_a_header_row_and_one_row_per_member() {
        let out =
            run(&b64(&sample_tar()), "base64", "csv", "archive", "", false, "iso", 500).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(
            lines[0],
            "path,type,size,mode,uid,gid,uname,gname,mtime,link_target,offset"
        );
        assert_eq!(lines.len(), 3, "dirs excluded: {out}");
        assert!(lines[1].starts_with("docs/hello.txt,file,12,0644,1000,1000,alice,staff,"));
    }

    #[test]
    fn glob_filter_and_dir_exclusion_narrow_the_listing() {
        let out = run(
            &b64(&sample_tar()),
            "base64",
            "paths",
            "path",
            "*.txt",
            false,
            "iso",
            500,
        )
        .unwrap();
        assert_eq!(out, "docs/hello.txt\ndocs/link.txt");
    }

    #[test]
    fn limit_truncates_but_the_summary_reports_the_true_total() {
        let out =
            run(&b64(&sample_tar()), "base64", "table", "path", "", true, "none", 1).unwrap();
        assert!(out.starts_with("drwxr-xr-x alice/staff 0 docs/\n"), "{out}");
        assert!(out.contains("1 of 3 member(s) listed"), "{out}");
    }

    #[test]
    fn sorts_by_size_descending() {
        let out =
            run(&b64(&sample_tar()), "base64", "paths", "size", "", true, "iso", 500).unwrap();
        assert_eq!(out, "docs/hello.txt\ndocs/\ndocs/link.txt");
    }

    #[test]
    fn reads_gzip_wrapped_archives() {
        use std::io::Write;
        let mut enc =
            flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(&sample_tar()).unwrap();
        let gz = enc.finish().unwrap();
        let l = list(&b64(&gz), "base64", "archive", "", true, 500).unwrap();
        assert_eq!(l.container, "tar.gz");
        assert_eq!(l.total_entries, 3);
    }

    #[test]
    fn reads_hex_input_and_gnu_long_names() {
        let long = "a".repeat(160);
        let mut t = Vec::new();
        t.extend(member(
            header("././@LongLink", 0, 0, 0, long.len() as u64 + 1, 0, b'L', "", "", ""),
            format!("{long}\0").as_bytes(),
        ));
        t.extend(member(
            header("aaaaaaaa", 0o644, 0, 0, 3, 0, b'0', "", "root", "root"),
            b"abc",
        ));
        t.extend(vec![0u8; BLOCK * 2]);
        let hex: String = t.iter().map(|b| format!("{b:02x}")).collect();
        let out = run(&hex, "hex", "paths", "archive", "", true, "iso", 500).unwrap();
        assert_eq!(out, long);
    }

    #[test]
    fn pax_extended_headers_override_the_ustar_fields() {
        // A PAX record is "<len> <key>=<value>\n" where <len> counts its OWN
        // digits too, so build it by fixed-point: the body is 30 bytes ('ü'
        // costs 2), plus a 2-digit length and its space → 33.
        let rec = "33 path=deep/nested/ünicode.txt\n";
        assert_eq!(rec.len(), 33);
        let mut t = Vec::new();
        t.extend(member(
            header("short.txt", 0, 0, 0, rec.len() as u64, 0, b'x', "", "", ""),
            rec.as_bytes(),
        ));
        t.extend(member(
            header("short.txt", 0o600, 7, 8, 5, 0, b'0', "", "root", "root"),
            b"hello",
        ));
        t.extend(vec![0u8; BLOCK * 2]);
        let out = run(&b64(&t), "base64", "paths", "archive", "", true, "iso", 500).unwrap();
        assert_eq!(out, "deep/nested/ünicode.txt");
    }

    #[test]
    fn rejects_input_that_is_not_a_tar_archive() {
        let err = run(&b64(&[b'x'; 1024]), "base64", "table", "archive", "", true, "iso", 500)
            .unwrap_err();
        assert!(err.contains("not a tar archive"), "{err}");
    }

    #[test]
    fn rejects_a_zip_archive_with_a_specific_message() {
        let mut z = b"PK\x03\x04".to_vec();
        z.extend(vec![0u8; 600]);
        let err =
            run(&b64(&z), "base64", "table", "archive", "", true, "iso", 500).unwrap_err();
        assert!(err.contains("ZIP archive"), "{err}");
    }

    #[test]
    fn rejects_bad_base64_and_bad_enum_values() {
        assert!(run("!!!!", "base64", "table", "archive", "", true, "iso", 500)
            .unwrap_err()
            .contains("invalid base64"));
        assert!(run("", "base64", "table", "archive", "", true, "iso", 500)
            .unwrap_err()
            .contains("input is empty"));
        let tar = b64(&sample_tar());
        assert!(run(&tar, "base64", "yaml", "archive", "", true, "iso", 500)
            .unwrap_err()
            .contains("invalid output"));
        assert!(run(&tar, "base64", "table", "colour", "", true, "iso", 500)
            .unwrap_err()
            .contains("invalid sort"));
        assert!(run(&tar, "base64", "table", "archive", "", true, "iso", 0)
            .unwrap_err()
            .contains("invalid limit"));
    }

    #[test]
    fn rejects_a_truncated_archive() {
        let full = sample_tar();
        let cut = &full[..BLOCK + BLOCK]; // header of member 2 dropped mid-stream
        let err =
            run(&b64(cut), "base64", "table", "archive", "", true, "iso", 500).unwrap_err();
        assert!(err.contains("truncated") || err.contains("corrupt"), "{err}");
    }

    #[test]
    fn mode_string_renders_special_bits() {
        assert_eq!(mode_string("file", 0o4755), "-rwsr-xr-x");
        assert_eq!(mode_string("directory", 0o1777), "drwxrwxrwt");
        assert_eq!(mode_string("file", 0o2644), "-rw-r-Sr--");
    }

    #[test]
    fn iso_utc_matches_known_timestamps() {
        assert_eq!(iso_utc(0), "1970-01-01 00:00:00");
        assert_eq!(iso_utc(1_714_557_600), "2024-05-01 10:00:00");
    }

    #[test]
    fn glob_matches_stars_and_question_marks() {
        assert!(glob_match(b"*.txt", b"a/b/c.txt"));
        assert!(glob_match(b"src/*", b"src/main.rs"));
        assert!(glob_match(b"a?c", b"abc"));
        assert!(!glob_match(b"a?c", b"abbc"));
        assert!(!glob_match(b"*.txt", b"a.rs"));
    }
}

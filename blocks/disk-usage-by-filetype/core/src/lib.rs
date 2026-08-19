//! disk-usage-by-filetype core — aggregate a pasted file listing's bytes by
//! file extension (or by broad file-type category) and render it as a sorted
//! bar chart, table, CSV, JSON or colored SVG.
//!
//! Pure and deterministic: same listing + options → same output. Nothing is
//! read from disk; the sizes come from whatever listing you paste (`du -a`,
//! `find -printf '%s %p\n'`, `ls -lR`, a size,path CSV export …).

use std::collections::{HashMap, HashSet};

/// Hard cap on sized entries per run.
pub const MAX_ENTRIES: usize = 20_000;

/// Label used for files with no usable extension (`README`, `.gitignore`, …).
pub const NO_EXTENSION: &str = "(no extension)";

/// Every knob the tool exposes, in descriptor order.
#[derive(Clone, Debug)]
pub struct Options {
    /// "extension" | "category"
    pub group_by: String,
    /// "size" | "count" | "name"
    pub sort_by: String,
    /// "desc" | "asc"
    pub order: String,
    /// How many groups to list before folding the rest into `(other)`.
    pub top_n: u32,
    /// "binary" (KiB) | "si" (kB) | "bytes"
    pub units: String,
    /// Bar width in characters for the `chart` format.
    pub chart_width: u32,
    /// Drop entries that are folders rather than files.
    pub skip_folders: bool,
    /// Fold `.JPG` into `.jpg`.
    pub ignore_case: bool,
    /// "chart" | "table" | "csv" | "json" | "svg"
    pub format: String,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            group_by: "extension".into(),
            sort_by: "size".into(),
            order: "desc".into(),
            top_n: 15,
            units: "binary".into(),
            chart_width: 32,
            skip_folders: true,
            ignore_case: true,
            format: "chart".into(),
        }
    }
}

/// One aggregated row.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub name: String,
    pub bytes: u64,
    pub files: u64,
}

struct Parsed {
    /// (normalized path, bytes, explicit-folder flag)
    entries: Vec<(String, u64, bool)>,
    ignored: usize,
}

// ---------------------------------------------------------------- size parsing

/// Parse one size token (`1234`, `4.0K`, `1.2MiB`, `512B`) into bytes.
/// Unit suffixes are 1024-based, matching `du -h` / `ls -lh`.
fn parse_size(token: &str) -> Option<u64> {
    let t = token.trim();
    if t.is_empty() {
        return None;
    }
    let digits_end = t
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == ','))
        .unwrap_or(t.len());
    let (num_part, unit) = t.split_at(digits_end);
    // Thousands separators are common in spreadsheet exports ("1,234,567").
    let num_clean: String = num_part.chars().filter(|c| *c != ',').collect();
    if num_clean.is_empty() || !num_clean.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    if num_clean.matches('.').count() > 1 {
        return None;
    }
    let value: f64 = num_clean.parse().ok()?;
    if !value.is_finite() || value < 0.0 {
        return None;
    }
    let mult = match unit.trim().to_ascii_lowercase().as_str() {
        "" | "b" | "byte" | "bytes" => 1.0_f64,
        "k" | "kb" | "kib" => 1024.0,
        "m" | "mb" | "mib" => 1024.0_f64.powi(2),
        "g" | "gb" | "gib" => 1024.0_f64.powi(3),
        "t" | "tb" | "tib" => 1024.0_f64.powi(4),
        "p" | "pb" | "pib" => 1024.0_f64.powi(5),
        _ => return None,
    };
    let bytes = value * mult;
    if bytes > u64::MAX as f64 {
        return None;
    }
    Some(bytes.round() as u64)
}

/// `ls -l` style permission column: `-rw-r--r--`, `drwxr-xr-x`, `-rw-r--r--@`.
fn ls_long_kind(token: &str) -> Option<char> {
    let chars: Vec<char> = token.chars().collect();
    if chars.len() < 10 || chars.len() > 11 {
        return None;
    }
    let kind = chars[0];
    if !matches!(kind, '-' | 'd' | 'l' | 'b' | 'c' | 'p' | 's') {
        return None;
    }
    if !chars[1..10]
        .iter()
        .all(|c| matches!(c, 'r' | 'w' | 'x' | 's' | 'S' | 't' | 'T' | '-'))
    {
        return None;
    }
    if chars.len() == 11 && !matches!(chars[10], '+' | '.' | '@') {
        return None;
    }
    Some(kind)
}

/// Normalize a path for folder detection and extension lookup.
fn normalize(path: &str) -> (String, bool) {
    let mut p = path.trim().replace('\\', "/");
    let mut is_dir = false;
    while p.ends_with('/') {
        p.pop();
        is_dir = true;
    }
    while let Some(rest) = p.strip_prefix("./") {
        p = rest.to_string();
    }
    (p, is_dir)
}

/// Pull `(bytes, path)` out of one listing line. Returns `None` when the line
/// carries no readable size (blank lines, `total 48`, tree art, prose).
fn parse_line(line: &str) -> Option<(u64, String, bool)> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }

    let fields: Vec<&str> = trimmed.split_whitespace().collect();
    if fields.is_empty() {
        return None;
    }
    // `ls -l` header line.
    if fields[0].eq_ignore_ascii_case("total") {
        return None;
    }

    // `ls -l` / `ls -lh`: perms links owner group SIZE date... name
    if let Some(kind) = ls_long_kind(fields[0]) {
        if fields.len() >= 8 {
            for idx in [4usize, 3, 2] {
                if let Some(bytes) = parse_size(fields[idx]) {
                    // Name starts after the three date fields.
                    let name_start = idx + 4;
                    if name_start < fields.len() {
                        let name = fields[name_start..].join(" ");
                        // `ls -l` renders symlinks as "link -> target".
                        let name = name.split(" -> ").next().unwrap_or(&name).to_string();
                        return Some((bytes, name, kind == 'd'));
                    }
                }
            }
        }
        return None;
    }

    // Tab- or comma-separated pairs: `du` output, spreadsheet exports.
    for sep in ['\t', ','] {
        if trimmed.contains(sep) {
            let parts: Vec<&str> = trimmed.split(sep).map(|s| s.trim()).collect();
            if parts.len() == 2 {
                if let Some(bytes) = parse_size(parts[0]) {
                    if !parts[1].is_empty() {
                        return Some((bytes, parts[1].to_string(), false));
                    }
                }
                if let Some(bytes) = parse_size(parts[1]) {
                    if !parts[0].is_empty() {
                        return Some((bytes, parts[0].to_string(), false));
                    }
                }
            }
            break;
        }
    }

    if fields.len() >= 2 {
        if let Some(bytes) = parse_size(fields[0]) {
            return Some((bytes, fields[1..].join(" "), false));
        }
        if let Some(bytes) = parse_size(fields[fields.len() - 1]) {
            return Some((bytes, fields[..fields.len() - 1].join(" "), false));
        }
    }
    None
}

fn parse_listing(listing: &str) -> Result<Parsed, String> {
    let mut entries = Vec::new();
    let mut ignored = 0usize;
    for line in listing.lines() {
        if line.trim().is_empty() {
            continue;
        }
        match parse_line(line) {
            Some((bytes, path, explicit_dir)) => {
                let (norm, slash_dir) = normalize(&path);
                if norm.is_empty() || norm == "." {
                    ignored += 1;
                    continue;
                }
                entries.push((norm, bytes, explicit_dir || slash_dir));
                if entries.len() > MAX_ENTRIES {
                    return Err(format!(
                        "too many entries: this tool reads up to {MAX_ENTRIES} sized files per run"
                    ));
                }
            }
            None => ignored += 1,
        }
    }
    Ok(Parsed { entries, ignored })
}

// ------------------------------------------------------------ classification

/// The extension of a path, including the leading dot (`.tar.gz`, `.png`),
/// or [`NO_EXTENSION`].
pub fn extension_of(path: &str, ignore_case: bool) -> String {
    let base = path.rsplit('/').next().unwrap_or(path);
    if base.is_empty() {
        return NO_EXTENSION.to_string();
    }
    let dot = match base.rfind('.') {
        Some(i) if i > 0 => i,
        // No dot at all, or a leading-dot dotfile like `.gitignore`.
        _ => return NO_EXTENSION.to_string(),
    };
    let mut ext = base[dot..].to_string();
    let stem = &base[..dot];
    // Keep the familiar double-barrelled archive extensions intact.
    if matches!(
        ext.to_ascii_lowercase().as_str(),
        ".gz" | ".bz2" | ".xz" | ".zst" | ".lz4" | ".br" | ".z" | ".lzma"
    ) {
        if let Some(inner) = stem.rfind('.') {
            let inner_ext = &stem[inner..];
            if inner_ext.eq_ignore_ascii_case(".tar") {
                ext = format!("{inner_ext}{ext}");
            }
        }
    }
    // Guard against sentences and version numbers ("report v1.2 final").
    let body = ext.trim_start_matches('.');
    if body.is_empty()
        || body.len() > 16
        || ext.contains(char::is_whitespace)
        || !body.chars().all(|c| c.is_ascii_alphanumeric() || c == '.')
    {
        return NO_EXTENSION.to_string();
    }
    if ignore_case {
        ext.to_ascii_lowercase()
    } else {
        ext
    }
}

const IMAGES: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "webp", "bmp", "tif", "tiff", "svg", "ico", "heic", "heif", "avif",
    "psd", "ai", "eps", "raw", "cr2", "nef", "arw", "dng", "xcf",
];
const VIDEO: &[&str] = &[
    "mp4", "mov", "avi", "mkv", "webm", "flv", "wmv", "m4v", "mpg", "mpeg", "3gp", "ts", "m2ts",
    "mts", "ogv", "vob",
];
const AUDIO: &[&str] = &[
    "mp3", "wav", "flac", "aac", "ogg", "oga", "m4a", "wma", "aiff", "aif", "opus", "mid", "midi",
    "amr",
];
const DOCUMENTS: &[&str] = &[
    "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "odt", "ods", "odp", "rtf", "txt", "md",
    "epub", "mobi", "azw3", "pages", "numbers", "key", "tex", "djvu",
];
const ARCHIVES: &[&str] = &[
    "zip", "tar", "gz", "tgz", "bz2", "xz", "7z", "rar", "zst", "lz4", "iso", "dmg", "cab", "lzma",
    "br", "tar.gz", "tar.bz2", "tar.xz", "tar.zst", "tar.lz4",
];
const CODE: &[&str] = &[
    "rs", "js", "mjs", "cjs", "ts", "jsx", "tsx", "py", "java", "c", "h", "cc", "cpp", "hpp", "cs",
    "go", "rb", "php", "swift", "kt", "kts", "scala", "sh", "bash", "zsh", "ps1", "bat", "pl",
    "lua", "r", "m", "sql", "html", "htm", "css", "scss", "sass", "less", "vue", "svelte", "dart",
    "ex", "exs", "erl", "hs", "clj", "vim", "asm",
];
const DATA: &[&str] = &[
    "json", "ndjson", "csv", "tsv", "yaml", "yml", "xml", "toml", "ini", "cfg", "conf", "env",
    "parquet", "avro", "db", "sqlite", "sqlite3", "log", "plist", "proto",
];
const EXECUTABLES: &[&str] = &[
    "exe", "dll", "so", "dylib", "app", "msi", "deb", "rpm", "apk", "bin", "jar", "wasm", "o", "a",
    "pyc", "class", "pkg", "appimage",
];
const FONTS: &[&str] = &["ttf", "otf", "woff", "woff2", "eot", "fon", "pfb"];

/// Broad file-type bucket for an extension (as returned by [`extension_of`]).
pub fn category_of(ext: &str) -> &'static str {
    if ext == NO_EXTENSION {
        return NO_EXTENSION;
    }
    let body = ext.trim_start_matches('.').to_ascii_lowercase();
    let b = body.as_str();
    if IMAGES.contains(&b) {
        "images"
    } else if VIDEO.contains(&b) {
        "video"
    } else if AUDIO.contains(&b) {
        "audio"
    } else if DOCUMENTS.contains(&b) {
        "documents"
    } else if ARCHIVES.contains(&b) {
        "archives"
    } else if CODE.contains(&b) {
        "code"
    } else if DATA.contains(&b) {
        "data"
    } else if EXECUTABLES.contains(&b) {
        "executables"
    } else if FONTS.contains(&b) {
        "fonts"
    } else {
        "other"
    }
}

// ------------------------------------------------------------- size rendering

fn format_bytes(bytes: u64, units: &str) -> String {
    match units {
        "bytes" => format!("{bytes}"),
        "si" => scale(bytes, 1000.0, &["B", "kB", "MB", "GB", "TB", "PB", "EB"]),
        _ => scale(bytes, 1024.0, &["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"]),
    }
}

fn scale(bytes: u64, base: f64, suffixes: &[&str]) -> String {
    let mut value = bytes as f64;
    let mut idx = 0usize;
    while value >= base && idx + 1 < suffixes.len() {
        value /= base;
        idx += 1;
    }
    if idx == 0 {
        format!("{bytes} {}", suffixes[0])
    } else {
        format!("{value:.1} {}", suffixes[idx])
    }
}

// ------------------------------------------------------------------ aggregate

fn validate(opts: &Options) -> Result<(), String> {
    let checks: [(&str, &str, &[&str]); 5] = [
        ("group_by", opts.group_by.as_str(), &["extension", "category"]),
        ("sort_by", opts.sort_by.as_str(), &["size", "count", "name"]),
        ("order", opts.order.as_str(), &["desc", "asc"]),
        ("units", opts.units.as_str(), &["binary", "si", "bytes"]),
        (
            "format",
            opts.format.as_str(),
            &["chart", "table", "csv", "json", "svg"],
        ),
    ];
    for (name, value, allowed) in checks {
        if !allowed.contains(&value) {
            return Err(format!(
                "invalid {name} {value:?}: expected one of {}",
                allowed
                    .iter()
                    .map(|a| format!("\"{a}\""))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }
    if !(1..=200).contains(&opts.top_n) {
        return Err(format!(
            "invalid top_n {}: expected 1 to 200 groups",
            opts.top_n
        ));
    }
    if !(8..=120).contains(&opts.chart_width) {
        return Err(format!(
            "invalid chart_width {}: expected 8 to 120 characters",
            opts.chart_width
        ));
    }
    Ok(())
}

/// Aggregated report, ready to render.
pub struct Report {
    pub groups: Vec<Group>,
    pub folded: usize,
    pub total_bytes: u64,
    pub total_files: u64,
    pub ignored_lines: usize,
    pub skipped_folders: usize,
}

/// Aggregate a listing into sorted groups. Public so the renderers (and tests)
/// can share one code path.
pub fn analyze(listing: &str, opts: &Options) -> Result<Report, String> {
    validate(opts)?;
    let parsed = parse_listing(listing)?;
    if parsed.entries.is_empty() {
        return Err(format!(
            "no sized files found in the listing ({} line(s) had no readable size). \
             Paste output that carries a size next to every path, e.g. \
             `find . -type f -printf '%s\\t%p\\n'`, `du -ah`, `ls -lR` or a `size,path` CSV",
            parsed.ignored
        ));
    }

    // Folder detection: an entry is a folder when it ends in a slash, `ls -l`
    // marked it `d`, or another listed entry sits underneath it (`du -a`).
    let mut ancestors: HashSet<&str> = HashSet::new();
    if opts.skip_folders {
        for (path, _, _) in &parsed.entries {
            let mut rest = path.as_str();
            while let Some(idx) = rest.rfind('/') {
                rest = &rest[..idx];
                if rest.is_empty() {
                    break;
                }
                ancestors.insert(rest);
            }
        }
    }

    let mut totals: HashMap<String, (u64, u64)> = HashMap::new();
    let mut total_bytes = 0u64;
    let mut total_files = 0u64;
    let mut skipped_folders = 0usize;
    for (path, bytes, is_dir) in &parsed.entries {
        if opts.skip_folders && (*is_dir || ancestors.contains(path.as_str())) {
            skipped_folders += 1;
            continue;
        }
        let ext = extension_of(path, opts.ignore_case);
        let key = if opts.group_by == "category" {
            category_of(&ext).to_string()
        } else {
            ext
        };
        let slot = totals.entry(key).or_insert((0, 0));
        slot.0 = slot.0.saturating_add(*bytes);
        slot.1 += 1;
        total_bytes = total_bytes.saturating_add(*bytes);
        total_files += 1;
    }

    if totals.is_empty() {
        return Err(
            "every entry in the listing looked like a folder — turn off \"Skip folder entries\" \
             or paste a file-level listing such as `find . -type f -printf '%s\\t%p\\n'`"
                .to_string(),
        );
    }

    let mut groups: Vec<Group> = totals
        .into_iter()
        .map(|(name, (bytes, files))| Group { name, bytes, files })
        .collect();
    groups.sort_by(|a, b| match opts.sort_by.as_str() {
        "count" => b.files.cmp(&a.files).then_with(|| a.name.cmp(&b.name)),
        "name" => a.name.cmp(&b.name),
        _ => b.bytes.cmp(&a.bytes).then_with(|| a.name.cmp(&b.name)),
    });
    if opts.order == "asc" {
        groups.reverse();
    }

    let mut folded = 0usize;
    if groups.len() > opts.top_n as usize {
        let rest: Vec<Group> = groups.split_off(opts.top_n as usize);
        folded = rest.len();
        let bytes = rest.iter().map(|g| g.bytes).sum();
        let files = rest.iter().map(|g| g.files).sum();
        groups.push(Group {
            name: format!("(other {folded})"),
            bytes,
            files,
        });
    }

    Ok(Report {
        groups,
        folded,
        total_bytes,
        total_files,
        ignored_lines: parsed.ignored,
        skipped_folders,
    })
}

// ------------------------------------------------------------------ rendering

fn percent(bytes: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        bytes as f64 * 100.0 / total as f64
    }
}

const EIGHTHS: [&str; 8] = ["", "▏", "▎", "▍", "▌", "▋", "▊", "▉"];

fn bar(bytes: u64, max: u64, width: u32) -> String {
    if max == 0 {
        return String::new();
    }
    let eighths = (bytes as f64 / max as f64 * width as f64 * 8.0).round() as u64;
    let full = (eighths / 8) as usize;
    let rem = (eighths % 8) as usize;
    let mut out = "█".repeat(full);
    out.push_str(EIGHTHS[rem]);
    if out.is_empty() && bytes > 0 {
        out.push('▏');
    }
    out
}

fn label(opts: &Options) -> &'static str {
    if opts.group_by == "category" {
        "category"
    } else {
        "extension"
    }
}

fn footer(report: &Report, opts: &Options) -> Vec<String> {
    let mut notes = Vec::new();
    if report.folded > 0 {
        notes.push(format!(
            "{} smaller {}(s) folded into (other {}).",
            report.folded,
            label(opts),
            report.folded
        ));
    }
    if report.skipped_folders > 0 {
        notes.push(format!(
            "{} folder entr(y/ies) skipped so their contents are not counted twice.",
            report.skipped_folders
        ));
    }
    if report.ignored_lines > 0 {
        notes.push(format!(
            "{} line(s) carried no readable size and were ignored.",
            report.ignored_lines
        ));
    }
    notes
}

fn render_chart(report: &Report, opts: &Options) -> String {
    let max = report.groups.iter().map(|g| g.bytes).max().unwrap_or(0);
    let name_w = report
        .groups
        .iter()
        .map(|g| g.name.chars().count())
        .max()
        .unwrap_or(4)
        .max(4);
    let sizes: Vec<String> = report
        .groups
        .iter()
        .map(|g| format_bytes(g.bytes, &opts.units))
        .collect();
    let size_w = sizes.iter().map(|s| s.len()).max().unwrap_or(6);
    let count_w = report
        .groups
        .iter()
        .map(|g| g.files.to_string().len())
        .max()
        .unwrap_or(1);

    let mut out = format!(
        "Disk usage by {} — {} file(s), {} total\n\n",
        label(opts),
        report.total_files,
        format_bytes(report.total_bytes, &opts.units)
    );
    for (g, size) in report.groups.iter().zip(sizes.iter()) {
        let pad = name_w - g.name.chars().count();
        // Pad the bar so the file-count column stays aligned across rows.
        let drawn = bar(g.bytes, max, opts.chart_width);
        let bar_pad = (opts.chart_width as usize).saturating_sub(drawn.chars().count());
        out.push_str(&format!(
            "{}{}  {:>size_w$}  {:>5.1}%  {}{}  {:>count_w$} file(s)\n",
            g.name,
            " ".repeat(pad),
            size,
            percent(g.bytes, report.total_bytes),
            drawn,
            " ".repeat(bar_pad),
            g.files,
        ));
    }
    for note in footer(report, opts) {
        out.push_str(&format!("\n{note}"));
    }
    out
}

fn render_table(report: &Report, opts: &Options) -> String {
    let head = if opts.group_by == "category" {
        "Category"
    } else {
        "Extension"
    };
    let sizes: Vec<String> = report
        .groups
        .iter()
        .map(|g| format_bytes(g.bytes, &opts.units))
        .collect();
    let total_size = format_bytes(report.total_bytes, &opts.units);
    let name_w = report
        .groups
        .iter()
        .map(|g| g.name.chars().count())
        .chain(std::iter::once(head.len()))
        .chain(std::iter::once(5)) // "TOTAL"
        .max()
        .unwrap_or(9);
    let size_w = sizes
        .iter()
        .map(|s| s.len())
        .chain(std::iter::once(total_size.len()))
        .chain(std::iter::once(4))
        .max()
        .unwrap_or(6);
    let count_w = report
        .groups
        .iter()
        .map(|g| g.files.to_string().len())
        .chain(std::iter::once(report.total_files.to_string().len()))
        .chain(std::iter::once(5))
        .max()
        .unwrap_or(5);

    let mut out = format!(
        "{:<name_w$}  {:>size_w$}  {:>6}  {:>count_w$}\n",
        head, "Size", "Share", "Files"
    );
    out.push_str(&format!(
        "{}  {}  {}  {}\n",
        "-".repeat(name_w),
        "-".repeat(size_w),
        "-".repeat(6),
        "-".repeat(count_w)
    ));
    for (g, size) in report.groups.iter().zip(sizes.iter()) {
        let pad = name_w - g.name.chars().count();
        out.push_str(&format!(
            "{}{}  {:>size_w$}  {:>5.1}%  {:>count_w$}\n",
            g.name,
            " ".repeat(pad),
            size,
            percent(g.bytes, report.total_bytes),
            g.files
        ));
    }
    out.push_str(&format!(
        "{:<name_w$}  {:>size_w$}  {:>5.1}%  {:>count_w$}\n",
        "TOTAL", total_size, 100.0, report.total_files
    ));
    for note in footer(report, opts) {
        out.push_str(&format!("\n{note}"));
    }
    out
}

fn csv_field(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn render_csv(report: &Report, opts: &Options) -> String {
    let mut out = format!("{},bytes,size,percent,files\n", label(opts));
    for g in &report.groups {
        out.push_str(&format!(
            "{},{},{},{:.1},{}\n",
            csv_field(&g.name),
            g.bytes,
            csv_field(&format_bytes(g.bytes, &opts.units)),
            percent(g.bytes, report.total_bytes),
            g.files
        ));
    }
    out
}

fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
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

fn render_json(report: &Report, opts: &Options) -> String {
    let mut out = String::from("{\n");
    out.push_str(&format!("  \"group_by\": {},\n", json_string(&opts.group_by)));
    out.push_str(&format!("  \"units\": {},\n", json_string(&opts.units)));
    out.push_str(&format!("  \"total_bytes\": {},\n", report.total_bytes));
    out.push_str(&format!(
        "  \"total_size\": {},\n",
        json_string(&format_bytes(report.total_bytes, &opts.units))
    ));
    out.push_str(&format!("  \"total_files\": {},\n", report.total_files));
    out.push_str(&format!(
        "  \"skipped_folders\": {},\n",
        report.skipped_folders
    ));
    out.push_str(&format!(
        "  \"ignored_lines\": {},\n",
        report.ignored_lines
    ));
    out.push_str("  \"groups\": [\n");
    for (i, g) in report.groups.iter().enumerate() {
        out.push_str(&format!(
            "    {{ \"name\": {}, \"bytes\": {}, \"size\": {}, \"percent\": {:.1}, \"files\": {} }}{}\n",
            json_string(&g.name),
            g.bytes,
            json_string(&format_bytes(g.bytes, &opts.units)),
            percent(g.bytes, report.total_bytes),
            g.files,
            if i + 1 == report.groups.len() { "" } else { "," }
        ));
    }
    out.push_str("  ]\n}\n");
    out
}

/// Distinct, colour-blind-friendly palette for the SVG bars.
const PALETTE: [&str; 12] = [
    "#2563eb", "#dc2626", "#059669", "#d97706", "#7c3aed", "#0891b2", "#db2777", "#65a30d",
    "#ea580c", "#4f46e5", "#0d9488", "#9333ea",
];

fn category_color(name: &str) -> Option<&'static str> {
    Some(match name {
        "images" => "#2563eb",
        "video" => "#dc2626",
        "audio" => "#7c3aed",
        "documents" => "#059669",
        "archives" => "#d97706",
        "code" => "#0891b2",
        "data" => "#db2777",
        "executables" => "#65a30d",
        "fonts" => "#ea580c",
        "other" => "#64748b",
        NO_EXTENSION => "#94a3b8",
        _ => return None,
    })
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn render_svg(report: &Report, opts: &Options) -> String {
    let width = 720.0_f64;
    let row_h = 30.0_f64;
    let top = 62.0_f64;
    let left = 150.0_f64;
    let right_gap = 118.0_f64;
    let track = width - left - right_gap;
    let height = top + row_h * report.groups.len() as f64 + 18.0;
    let max = report.groups.iter().map(|g| g.bytes).max().unwrap_or(0);

    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {width:.0} {height:.0}\" width=\"{width:.0}\" height=\"{height:.0}\" role=\"img\" aria-label=\"Disk usage by {}\">\n",
        label(opts)
    );
    out.push_str(&format!(
        "  <rect width=\"{width:.0}\" height=\"{height:.0}\" fill=\"#ffffff\"/>\n"
    ));
    out.push_str(&format!(
        "  <text x=\"16\" y=\"28\" font-family=\"system-ui, sans-serif\" font-size=\"17\" font-weight=\"600\" fill=\"#0f172a\">Disk usage by {}</text>\n",
        xml_escape(label(opts))
    ));
    out.push_str(&format!(
        "  <text x=\"16\" y=\"48\" font-family=\"system-ui, sans-serif\" font-size=\"13\" fill=\"#475569\">{} file(s), {} total</text>\n",
        report.total_files,
        xml_escape(&format_bytes(report.total_bytes, &opts.units))
    ));
    for (i, g) in report.groups.iter().enumerate() {
        let y = top + row_h * i as f64;
        let w = if max == 0 {
            0.0
        } else {
            (g.bytes as f64 / max as f64 * track).max(if g.bytes > 0 { 2.0 } else { 0.0 })
        };
        let color = category_color(&g.name).unwrap_or(PALETTE[i % PALETTE.len()]);
        out.push_str(&format!(
            "  <text x=\"{:.0}\" y=\"{:.1}\" text-anchor=\"end\" font-family=\"ui-monospace, monospace\" font-size=\"13\" fill=\"#0f172a\">{}</text>\n",
            left - 10.0,
            y + 15.0,
            xml_escape(&g.name)
        ));
        out.push_str(&format!(
            "  <rect x=\"{left:.0}\" y=\"{:.1}\" width=\"{w:.1}\" height=\"18\" rx=\"3\" fill=\"{color}\"><title>{} — {} ({:.1}%, {} file(s))</title></rect>\n",
            y + 2.0,
            xml_escape(&g.name),
            xml_escape(&format_bytes(g.bytes, &opts.units)),
            percent(g.bytes, report.total_bytes),
            g.files
        ));
        out.push_str(&format!(
            "  <text x=\"{:.1}\" y=\"{:.1}\" font-family=\"system-ui, sans-serif\" font-size=\"12\" fill=\"#334155\">{} · {:.1}%</text>\n",
            left + w + 8.0,
            y + 15.0,
            xml_escape(&format_bytes(g.bytes, &opts.units)),
            percent(g.bytes, report.total_bytes)
        ));
    }
    out.push_str("</svg>\n");
    out
}

/// Aggregate `listing` and render it in the requested format.
pub fn run(listing: &str, opts: &Options) -> Result<String, String> {
    let report = analyze(listing, opts)?;
    Ok(match opts.format.as_str() {
        "table" => render_table(&report, opts),
        "csv" => render_csv(&report, opts),
        "json" => render_json(&report, opts),
        "svg" => render_svg(&report, opts),
        _ => render_chart(&report, opts),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const DU: &str = "4.0K\t./src/app.js\n2.0M\t./assets/hero.png\n1.0M\t./assets/logo.png\n8.0K\t./README.md\n";

    #[test]
    fn chart_happy_path() {
        let out = run(DU, &Options::default()).unwrap();
        assert!(
            out.starts_with("Disk usage by extension — 4 file(s), 3.0 MiB total"),
            "{out}"
        );
        assert!(out.contains(".png"), "{out}");
        assert!(out.contains("█"), "{out}");
        // .png = 3 MiB of 3.01 MiB total.
        let png_line = out.lines().find(|l| l.starts_with(".png")).unwrap();
        assert!(png_line.contains("99.6%"), "{png_line}");
        assert!(png_line.trim_end().ends_with("2 file(s)"), "{png_line}");
    }

    #[test]
    fn empty_listing_is_an_error() {
        let err = run("   \n\n", &Options::default()).unwrap_err();
        assert!(err.starts_with("no sized files found"), "{err}");
    }

    #[test]
    fn prose_without_sizes_is_an_error() {
        let err = run("hello world\njust some notes\n", &Options::default()).unwrap_err();
        assert!(err.contains("no readable size"), "{err}");
    }

    #[test]
    fn invalid_enum_is_an_error() {
        let opts = Options {
            format: "pdf".into(),
            ..Options::default()
        };
        let err = run(DU, &opts).unwrap_err();
        assert_eq!(
            err,
            "invalid format \"pdf\": expected one of \"chart\", \"table\", \"csv\", \"json\", \"svg\""
        );
    }

    #[test]
    fn out_of_range_numbers_are_errors() {
        let opts = Options {
            top_n: 0,
            ..Options::default()
        };
        assert!(run(DU, &opts).unwrap_err().starts_with("invalid top_n 0"));
        let opts = Options {
            chart_width: 500,
            ..Options::default()
        };
        assert!(run(DU, &opts)
            .unwrap_err()
            .starts_with("invalid chart_width 500"));
    }

    #[test]
    fn parses_size_tokens() {
        assert_eq!(parse_size("1234"), Some(1234));
        assert_eq!(parse_size("4.0K"), Some(4096));
        assert_eq!(parse_size("1.5MiB"), Some(1_572_864));
        assert_eq!(parse_size("512B"), Some(512));
        assert_eq!(parse_size("1,234"), Some(1234));
        assert_eq!(parse_size("Jan"), None);
        assert_eq!(parse_size("10:00"), None);
        assert_eq!(parse_size("1.2.3"), None);
    }

    #[test]
    fn reads_find_ls_and_csv_shapes() {
        // find . -type f -printf '%s %p\n'
        let find = run("1048576 ./a/photo.jpg\n2048 ./a/notes.md\n", &Options::default()).unwrap();
        assert!(find.contains(".jpg"), "{find}");
        // ls -l, including a directory row and the "total" header
        let ls = "total 48\n-rw-r--r--  1 me  staff   1024 Jan  3 10:11 notes.md\ndrwxr-xr-x  4 me  staff    128 Jan  3 10:11 src\n-rw-r--r--  1 me  staff  10240 Jan  3 10:12 photo.jpg\n";
        let report = analyze(ls, &Options::default()).unwrap();
        assert_eq!(report.total_files, 2);
        assert_eq!(report.skipped_folders, 1);
        // size,path CSV export
        let csv = analyze("2048,docs/a.pdf\n4096,docs/b.pdf\n", &Options::default()).unwrap();
        assert_eq!(report_group(&csv, ".pdf").bytes, 6144);
    }

    fn report_group<'a>(report: &'a Report, name: &str) -> &'a Group {
        report.groups.iter().find(|g| g.name == name).unwrap()
    }

    #[test]
    fn du_folder_rows_are_not_double_counted() {
        let listing = "4.0K\t./src/app.js\n4.0K\t./src\n8.0K\t.\n";
        let report = analyze(listing, &Options::default()).unwrap();
        assert_eq!(report.total_files, 1);
        assert_eq!(report.total_bytes, 4096);
        // `.` is ignored outright, `./src` is detected as a parent folder.
        assert_eq!(report.skipped_folders, 1);
        assert_eq!(report.ignored_lines, 1);
    }

    #[test]
    fn extensions_and_categories() {
        assert_eq!(extension_of("a/b/photo.JPG", true), ".jpg");
        assert_eq!(extension_of("a/b/photo.JPG", false), ".JPG");
        assert_eq!(extension_of("dist/app.tar.gz", true), ".tar.gz");
        assert_eq!(extension_of(".gitignore", true), NO_EXTENSION);
        assert_eq!(extension_of("Makefile", true), NO_EXTENSION);
        assert_eq!(extension_of("report v1.2 final", true), NO_EXTENSION);
        assert_eq!(category_of(".mp4"), "video");
        assert_eq!(category_of(".tar.gz"), "archives");
        assert_eq!(category_of(".rs"), "code");
        assert_eq!(category_of(".qqq"), "other");
        assert_eq!(category_of(NO_EXTENSION), NO_EXTENSION);
    }

    #[test]
    fn category_grouping_and_units() {
        let opts = Options {
            group_by: "category".into(),
            units: "si".into(),
            format: "table".into(),
            ..Options::default()
        };
        let out = run("1000000\tclip.mp4\n500000\tsong.mp3\n", &opts).unwrap();
        assert!(out.contains("Category"), "{out}");
        assert!(out.contains("video"), "{out}");
        assert!(out.contains("1.0 MB"), "{out}");
        assert!(out.contains("TOTAL"), "{out}");
    }

    #[test]
    fn top_n_folds_the_tail() {
        let listing = (0..10)
            .map(|i| format!("{}\tfile{i}.e{i}", (10 - i) * 1000))
            .collect::<Vec<_>>()
            .join("\n");
        let opts = Options {
            top_n: 3,
            ..Options::default()
        };
        let report = analyze(&listing, &opts).unwrap();
        assert_eq!(report.groups.len(), 4);
        assert_eq!(report.folded, 7);
        assert_eq!(report.groups[3].name, "(other 7)");
        assert_eq!(report.groups[3].files, 7);
    }

    #[test]
    fn sorting_by_count_and_name_and_order() {
        let listing = "10\ta.txt\n10\tb.txt\n900\tc.zip\n";
        let by_count = analyze(
            listing,
            &Options {
                sort_by: "count".into(),
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(by_count.groups[0].name, ".txt");
        let by_name = analyze(
            listing,
            &Options {
                sort_by: "name".into(),
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(by_name.groups[0].name, ".txt");
        let asc = analyze(
            listing,
            &Options {
                order: "asc".into(),
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(asc.groups[0].name, ".txt");
    }

    #[test]
    fn json_and_csv_and_svg_shapes() {
        let json = run(
            DU,
            &Options {
                format: "json".into(),
                ..Options::default()
            },
        )
        .unwrap();
        assert!(json.contains("\"total_files\": 4"), "{json}");
        assert!(json.contains("\"name\": \".png\""), "{json}");
        let csv = run(
            DU,
            &Options {
                format: "csv".into(),
                ..Options::default()
            },
        )
        .unwrap();
        assert!(csv.starts_with("extension,bytes,size,percent,files\n"), "{csv}");
        let svg = run(
            DU,
            &Options {
                format: "svg".into(),
                ..Options::default()
            },
        )
        .unwrap();
        assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""), "{svg}");
        assert!(svg.contains("<rect"), "{svg}");
        assert!(svg.trim_end().ends_with("</svg>"), "{svg}");
    }

    #[test]
    fn skip_folders_off_counts_folder_rows() {
        let listing = "4.0K\t./src/app.js\n4.0K\t./src/\n";
        let on = analyze(listing, &Options::default()).unwrap();
        assert_eq!(on.total_files, 1);
        let off = analyze(
            listing,
            &Options {
                skip_folders: false,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(off.total_files, 2);
    }

    #[test]
    fn entry_cap_is_enforced() {
        let listing = (0..=MAX_ENTRIES)
            .map(|i| format!("10\tf{i}.txt"))
            .collect::<Vec<_>>()
            .join("\n");
        let err = run(&listing, &Options::default()).unwrap_err();
        assert!(err.starts_with("too many entries"), "{err}");
    }
}

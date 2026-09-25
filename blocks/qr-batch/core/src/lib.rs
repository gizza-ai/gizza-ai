//! qr-batch core — turn a pasted list (or a two-column CSV/TSV) into many QR
//! codes at once and bundle them as a single ZIP archive.
//!
//! Pure-Rust (`qrcode` + `image` for PNG + `zip`), so it runs on ALL backends:
//! the chat Service Worker, the CLI, and the browser page. No wafer /
//! wasm-bindgen deps here.
//!
//! The SVG and the PNG raster are both built by hand from
//! [`qrcode::QrCode::to_colors`] rather than through the crate's own renderers,
//! because those hard-code a 4-module quiet zone and can't emit a transparent
//! background.
//!
//! Determinism: the same input + options always produce byte-identical archive
//! bytes (zip entries carry the fixed 1980-01-01 DOS timestamp), so a batch can
//! be diffed in a build pipeline.

use std::collections::HashSet;
use std::fmt::Write as _;
use std::io::{Cursor, Write as _};

use qrcode::types::Color;
use qrcode::{EcLevel, QrCode};
use zip::write::{SimpleFileOptions, ZipWriter};
use zip::CompressionMethod;

/// Most rows accepted in one batch. A memory guard for the 64 MiB wasm sandbox,
/// not a paywall — split a bigger list and run it twice.
pub const MAX_ROWS: usize = 500;
/// Longest single payload accepted, in bytes. QR byte-mode tops out at 2953
/// bytes (version 40, error correction L); lower levels cap lower still and are
/// checked per error-correction level in [`Ecc::capacity_bytes`].
pub const MAX_PAYLOAD_BYTES: usize = 2953;
/// Cap on the total uncompressed bytes generated before zipping.
pub const MAX_TOTAL_BYTES: usize = 32 * 1024 * 1024;

/// How the pasted `data` is split into rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputFormat {
    /// Sniff: tab-separated if any line has a tab, else comma-separated if any
    /// line has a comma, else one plain value per line.
    Auto,
    /// One value per line; commas and tabs are part of the value.
    List,
    /// Comma-separated, quote-aware.
    Csv,
    /// Tab-separated, quote-aware.
    Tsv,
}

impl InputFormat {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" | "" => Ok(Self::Auto),
            "list" | "lines" | "text" | "txt" => Ok(Self::List),
            "csv" | "comma" => Ok(Self::Csv),
            "tsv" | "tab" => Ok(Self::Tsv),
            other => Err(format!(
                "unknown input_format '{other}' (use auto, list, csv, or tsv)"
            )),
        }
    }
}

/// Which column holds the filename and which holds the QR payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Columns {
    /// Two or more columns → first is the filename, second is the payload;
    /// a single column is the payload with an auto-numbered filename.
    Auto,
    /// First column is the filename, second is the payload.
    NameValue,
    /// First column is the payload, second is the filename.
    ValueName,
    /// The whole line is the payload — no splitting at all, so payloads may
    /// contain commas and tabs.
    ValueOnly,
}

impl Columns {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" | "" => Ok(Self::Auto),
            "name-value" | "name_value" | "name,value" => Ok(Self::NameValue),
            "value-name" | "value_name" | "value,name" => Ok(Self::ValueName),
            "value-only" | "value_only" | "value" => Ok(Self::ValueOnly),
            other => Err(format!(
                "unknown columns '{other}' (use auto, name-value, value-name, or value-only)"
            )),
        }
    }
}

/// Error-correction level. Higher levels survive more print damage/occlusion at
/// the cost of denser codes and a smaller payload budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ecc {
    Low,
    Medium,
    Quartile,
    High,
}

impl Ecc {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "l" | "low" => Ok(Self::Low),
            "m" | "medium" | "med" | "" => Ok(Self::Medium),
            "q" | "quartile" | "quart" => Ok(Self::Quartile),
            "h" | "high" => Ok(Self::High),
            other => Err(format!(
                "unknown error_correction '{other}' (use L, M, Q, or H)"
            )),
        }
    }

    fn level(self) -> EcLevel {
        match self {
            Self::Low => EcLevel::L,
            Self::Medium => EcLevel::M,
            Self::Quartile => EcLevel::Q,
            Self::High => EcLevel::H,
        }
    }

    /// Byte-mode payload capacity at QR version 40 for this level.
    pub fn capacity_bytes(self) -> usize {
        match self {
            Self::Low => 2953,
            Self::Medium => 2331,
            Self::Quartile => 1663,
            Self::High => 1273,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Low => "L",
            Self::Medium => "M",
            Self::Quartile => "Q",
            Self::High => "H",
        }
    }
}

/// Which image file(s) each row produces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutFormat {
    Png,
    Svg,
    /// Both a `.png` and a `.svg` per row, sharing the same base filename.
    Both,
}

impl OutFormat {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "png" | "" => Ok(Self::Png),
            "svg" => Ok(Self::Svg),
            "both" | "png+svg" | "all" => Ok(Self::Both),
            other => Err(format!("unknown format '{other}' (use png, svg, or both)")),
        }
    }

    fn exts(self) -> &'static [&'static str] {
        match self {
            Self::Png => &["png"],
            Self::Svg => &["svg"],
            Self::Both => &["png", "svg"],
        }
    }
}

/// Every batch option. Build with [`Options::default`] and override.
#[derive(Debug, Clone)]
pub struct Options {
    pub input_format: InputFormat,
    pub columns: Columns,
    pub has_header: bool,
    pub format: OutFormat,
    /// Target PNG edge in pixels; the raster is scaled to whole modules, so the
    /// real edge is the smallest whole-module multiple at or above this.
    pub size: u32,
    /// Quiet zone in modules on every side (the QR spec asks for 4).
    pub margin: u32,
    pub ecc: Ecc,
    /// Module (dark) colour: `#rgb`, `#rrggbb`, or a common colour name.
    pub fg_color: String,
    /// Background colour, or `transparent`.
    pub bg_color: String,
    /// Prefix for auto-numbered filenames (`qr` → `qr-001.png`).
    pub name_prefix: String,
    /// Add an `index.csv` mapping every produced file back to its payload.
    pub include_index: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            input_format: InputFormat::Auto,
            columns: Columns::Auto,
            has_header: false,
            format: OutFormat::Png,
            size: 512,
            margin: 4,
            ecc: Ecc::Medium,
            fg_color: "#000000".into(),
            bg_color: "#ffffff".into(),
            name_prefix: "qr".into(),
            include_index: true,
        }
    }
}

/// A row that could not be turned into a QR code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowError {
    /// 1-based position of the row in the (post-header) list.
    pub row: usize,
    pub value: String,
    pub reason: String,
}

/// The finished batch.
#[derive(Debug, Clone)]
pub struct Batch {
    /// The ZIP archive bytes.
    pub zip: Vec<u8>,
    /// Names of every file inside the archive, in archive order.
    pub files: Vec<String>,
    /// Rows read from the input (after the header row, if any).
    pub rows: usize,
    /// Rows that produced at least one image.
    pub generated: usize,
    /// Rows that failed, with the reason.
    pub errors: Vec<RowError>,
}

impl Batch {
    /// One-line human/LLM summary of what the batch produced.
    pub fn summary(&self) -> String {
        let mut s = format!(
            "generated {} QR code{} from {} row{} into {} file{} ({} bytes zipped)",
            self.generated,
            if self.generated == 1 { "" } else { "s" },
            self.rows,
            if self.rows == 1 { "" } else { "s" },
            self.files.len(),
            if self.files.len() == 1 { "" } else { "s" },
            self.zip.len(),
        );
        if !self.errors.is_empty() {
            let _ = write!(
                s,
                "; {} row{} failed (first: row {} — {})",
                self.errors.len(),
                if self.errors.len() == 1 { "" } else { "s" },
                self.errors[0].row,
                self.errors[0].reason,
            );
        }
        s
    }
}

// ---------------------------------------------------------------------------
// colours
// ---------------------------------------------------------------------------

/// A parsed colour: `None` means fully transparent.
type Rgb = Option<[u8; 3]>;

fn named_color(name: &str) -> Option<[u8; 3]> {
    Some(match name {
        "black" => [0, 0, 0],
        "white" => [255, 255, 255],
        "red" => [255, 0, 0],
        "green" => [0, 128, 0],
        "blue" => [0, 0, 255],
        "yellow" => [255, 255, 0],
        "cyan" | "aqua" => [0, 255, 255],
        "magenta" | "fuchsia" => [255, 0, 255],
        "orange" => [255, 165, 0],
        "purple" => [128, 0, 128],
        "navy" => [0, 0, 128],
        "gray" | "grey" => [128, 128, 128],
        _ => return None,
    })
}

/// Parse `#rgb`, `#rrggbb`, `transparent`, or a common colour name.
fn parse_color(s: &str, field: &str) -> Result<Rgb, String> {
    let t = s.trim().to_ascii_lowercase();
    if t.is_empty() {
        return Err(format!(
            "{field} is empty (use #rrggbb, a colour name, or transparent)"
        ));
    }
    if t == "transparent" || t == "none" {
        return Ok(None);
    }
    if let Some(c) = named_color(&t) {
        return Ok(Some(c));
    }
    let body = t.strip_prefix('#').unwrap_or(&t);
    if !body.chars().all(|c| c.is_ascii_hexdigit()) || !matches!(body.len(), 3 | 6) {
        return Err(format!(
            "invalid {field} '{s}': expected #rgb, #rrggbb, a colour name (black, white, red, …), or transparent"
        ));
    }
    let full: String = if body.len() == 3 {
        body.chars().flat_map(|c| [c, c]).collect()
    } else {
        body.to_string()
    };
    let px = |i: usize| u8::from_str_radix(&full[i..i + 2], 16).unwrap_or(0);
    Ok(Some([px(0), px(2), px(4)]))
}

fn hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

// ---------------------------------------------------------------------------
// input parsing
// ---------------------------------------------------------------------------

/// Split one line on `delim`, honouring `"…"` quoting with `""` as an escaped
/// quote (the spreadsheet-export convention).
fn split_line(line: &str, delim: char) -> Vec<String> {
    let mut fields = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    cur.push('"');
                } else {
                    in_quotes = false;
                }
            } else {
                cur.push(c);
            }
        } else if c == '"' && cur.trim().is_empty() {
            cur.clear();
            in_quotes = true;
        } else if c == delim {
            fields.push(std::mem::take(&mut cur));
        } else {
            cur.push(c);
        }
    }
    fields.push(cur);
    fields
}

/// One input row: an optional caller-supplied filename plus the QR payload.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Row {
    name: Option<String>,
    value: String,
    /// Set when the row itself is malformed (reported, never silently dropped).
    error: Option<String>,
}

fn effective_delim(lines: &[&str], fmt: InputFormat) -> Option<char> {
    match fmt {
        InputFormat::List => None,
        InputFormat::Csv => Some(','),
        InputFormat::Tsv => Some('\t'),
        InputFormat::Auto => {
            if lines.iter().any(|l| l.contains('\t')) {
                Some('\t')
            } else if lines.iter().any(|l| l.contains(',')) {
                Some(',')
            } else {
                None
            }
        }
    }
}

fn parse_rows(data: &str, o: &Options) -> Result<Vec<Row>, String> {
    let mut lines: Vec<&str> = data
        .lines()
        .map(|l| l.trim_end_matches('\r'))
        .filter(|l| !l.trim().is_empty())
        .collect();
    if lines.is_empty() {
        return Err("no rows found — paste one value per line (optionally `name,value`)".into());
    }
    if o.has_header {
        lines.remove(0);
        if lines.is_empty() {
            return Err(
                "only a header row was found — add at least one data row below it, or turn the header option off"
                    .into(),
            );
        }
    }
    if lines.len() > MAX_ROWS {
        return Err(format!(
            "{} rows is over the {MAX_ROWS}-row batch cap — split the list and run it again",
            lines.len()
        ));
    }

    let delim = effective_delim(&lines, o.input_format);
    let mut rows = Vec::with_capacity(lines.len());
    for line in lines {
        let row = match (delim, o.columns) {
            // No delimiter, or the caller pinned "the whole line is the value":
            // keep commas/tabs inside the payload.
            (None, _) | (_, Columns::ValueOnly) => Row {
                name: None,
                value: line.trim().to_string(),
                error: None,
            },
            (Some(d), mode) => {
                let f = split_line(line, d);
                let get = |i: usize| {
                    f.get(i)
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                };
                match mode {
                    Columns::NameValue if f.len() < 2 => Row {
                        name: None,
                        value: line.trim().to_string(),
                        error: Some(format!(
                            "expected `name{}value` but the row has only 1 column",
                            if d == '\t' { "<tab>" } else { "," }
                        )),
                    },
                    Columns::NameValue => Row {
                        name: get(0),
                        value: get(1).unwrap_or_default(),
                        error: None,
                    },
                    Columns::ValueName => Row {
                        name: get(1),
                        value: get(0).unwrap_or_default(),
                        error: None,
                    },
                    // Auto: a second, non-empty column means the first is a name.
                    Columns::Auto => match get(1) {
                        Some(v) => Row {
                            name: get(0),
                            value: v,
                            error: None,
                        },
                        None => Row {
                            name: None,
                            value: get(0).unwrap_or_default(),
                            error: None,
                        },
                    },
                    Columns::ValueOnly => unreachable!("handled above"),
                }
            }
        };
        rows.push(row);
    }
    Ok(rows)
}

// ---------------------------------------------------------------------------
// filenames
// ---------------------------------------------------------------------------

/// Keep a caller-supplied name safe as a ZIP entry: ASCII word characters,
/// dots, dashes and underscores only; no path separators, no leading dot, and
/// a trailing `.png`/`.svg` stripped so we never write `label.png.png`.
fn sanitize_name(raw: &str) -> String {
    let mut base = raw.trim();
    for ext in [".png", ".svg"] {
        if base.len() > ext.len() && base.to_ascii_lowercase().ends_with(ext) {
            base = &base[..base.len() - ext.len()];
            break;
        }
    }
    let mut s: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    while s.starts_with('.') || s.starts_with('_') {
        s.remove(0);
    }
    while s.ends_with('.') || s.ends_with('_') {
        s.pop();
    }
    s.truncate(80);
    s
}

fn unique(base: &str, seen: &mut HashSet<String>) -> String {
    if seen.insert(base.to_string()) {
        return base.to_string();
    }
    for n in 2.. {
        let cand = format!("{base}-{n}");
        if seen.insert(cand.clone()) {
            return cand;
        }
    }
    unreachable!()
}

// ---------------------------------------------------------------------------
// rendering
// ---------------------------------------------------------------------------

/// The dark/light module grid of a code, row-major, `width` modules per side.
fn modules(code: &QrCode) -> (usize, Vec<bool>) {
    let w = code.width();
    let dark = code.to_colors().iter().map(|c| *c == Color::Dark).collect();
    (w, dark)
}

/// Hand-built SVG: one `<path>` of 1×1 module squares over an optional
/// background rect, in a `viewBox` of module units so it scales losslessly.
fn render_svg(w: usize, dark: &[bool], margin: u32, fg: [u8; 3], bg: Rgb, px: u32) -> String {
    let m = margin as usize;
    let side = w + 2 * m;
    let mut d = String::new();
    for y in 0..w {
        for x in 0..w {
            if dark[y * w + x] {
                let _ = write!(d, "M{} {}h1v1h-1z", x + m, y + m);
            }
        }
    }
    let mut svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{px}" height="{px}" viewBox="0 0 {side} {side}" shape-rendering="crispEdges">"#
    );
    if let Some(c) = bg {
        let _ = write!(
            svg,
            r#"<rect width="{side}" height="{side}" fill="{}"/>"#,
            hex(c)
        );
    }
    let _ = write!(svg, r#"<path fill="{}" d="{d}"/></svg>"#, hex(fg));
    svg
}

/// Hand-built PNG raster. `px` is the requested edge; the real edge is the
/// smallest whole-module multiple at or above it, so modules never blur.
fn render_png(
    w: usize,
    dark: &[bool],
    margin: u32,
    fg: [u8; 3],
    bg: Rgb,
    px: u32,
) -> Result<Vec<u8>, String> {
    use image::{ImageEncoder, Rgba, RgbaImage};

    let m = margin as usize;
    let side = w + 2 * m;
    let scale = (px as usize).div_ceil(side).max(1);
    let edge = (side * scale) as u32;

    let fg_px = Rgba([fg[0], fg[1], fg[2], 255]);
    let bg_px = match bg {
        Some(c) => Rgba([c[0], c[1], c[2], 255]),
        None => Rgba([0, 0, 0, 0]),
    };
    let mut img = RgbaImage::from_pixel(edge, edge, bg_px);
    for y in 0..w {
        for x in 0..w {
            if !dark[y * w + x] {
                continue;
            }
            let (x0, y0) = (((x + m) * scale) as u32, ((y + m) * scale) as u32);
            for dy in 0..scale as u32 {
                for dx in 0..scale as u32 {
                    img.put_pixel(x0 + dx, y0 + dy, fg_px);
                }
            }
        }
    }
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(&mut out)
        .write_image(&img, edge, edge, image::ExtendedColorType::Rgba8)
        .map_err(|e| format!("failed to encode PNG: {e}"))?;
    Ok(out)
}

// ---------------------------------------------------------------------------
// index.csv
// ---------------------------------------------------------------------------

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

// ---------------------------------------------------------------------------
// the batch
// ---------------------------------------------------------------------------

/// Generate one QR code per input row and bundle them into a ZIP archive.
///
/// Rows that can't be encoded (empty, too long, malformed) are reported in
/// [`Batch::errors`] and in `index.csv` — never silently dropped. Errors are
/// only returned for whole-batch problems (bad options, no usable rows).
pub fn generate_batch(data: &str, o: &Options) -> Result<Batch, String> {
    let fg = parse_color(&o.fg_color, "fg_color")?.ok_or_else(|| {
        "fg_color cannot be transparent — the modules would be invisible".to_string()
    })?;
    let bg = parse_color(&o.bg_color, "bg_color")?;
    if Some(fg) == bg {
        return Err(format!(
            "fg_color and bg_color are both {} — the code would be unscannable",
            hex(fg)
        ));
    }
    if !(64..=2048).contains(&o.size) {
        return Err(format!("size {} is outside 64-2048 pixels", o.size));
    }
    if o.margin > 16 {
        return Err(format!("margin {} is outside 0-16 modules", o.margin));
    }
    let prefix = {
        let p = sanitize_name(&o.name_prefix);
        if p.is_empty() {
            "qr".to_string()
        } else {
            p
        }
    };

    let rows = parse_rows(data, o)?;
    let width = rows.len().to_string().len().max(3);

    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    let mut index = String::from("filename,value,status\n");
    let mut errors: Vec<RowError> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut generated = 0usize;
    let mut total = 0usize;

    for (i, row) in rows.iter().enumerate() {
        let n = i + 1;
        let fail = |reason: String| RowError {
            row: n,
            value: row.value.clone(),
            reason,
        };

        if let Some(e) = &row.error {
            errors.push(fail(e.clone()));
            continue;
        }
        if row.value.is_empty() {
            errors.push(fail("the value column is empty".into()));
            continue;
        }
        let bytes = row.value.len();
        if bytes > o.ecc.capacity_bytes() {
            errors.push(fail(format!(
                "{bytes} bytes is over the {}-byte QR capacity at error correction {} — shorten it or use a lower level",
                o.ecc.capacity_bytes(),
                o.ecc.label()
            )));
            continue;
        }
        let code = match QrCode::with_error_correction_level(row.value.as_bytes(), o.ecc.level()) {
            Ok(c) => c,
            Err(e) => {
                errors.push(fail(format!(
                    "could not encode this value as a QR code: {e}"
                )));
                continue;
            }
        };

        let base = {
            let named = row.name.as_deref().map(sanitize_name).unwrap_or_default();
            let candidate = if named.is_empty() {
                format!("{prefix}-{n:0width$}")
            } else {
                named
            };
            unique(&candidate, &mut seen)
        };

        let (w, dark) = modules(&code);
        for ext in o.format.exts() {
            let bytes = match *ext {
                "svg" => render_svg(w, &dark, o.margin, fg, bg, o.size).into_bytes(),
                _ => render_png(w, &dark, o.margin, fg, bg, o.size)?,
            };
            total += bytes.len();
            if total > MAX_TOTAL_BYTES {
                return Err(format!(
                    "the batch exceeded the {} MiB output cap at row {n} — use a smaller size, fewer rows, or SVG only",
                    MAX_TOTAL_BYTES / (1024 * 1024)
                ));
            }
            let filename = format!("{base}.{ext}");
            let _ = writeln!(
                index,
                "{},{},ok",
                csv_field(&filename),
                csv_field(&row.value)
            );
            entries.push((filename, bytes));
        }
        generated += 1;
    }

    for e in &errors {
        let _ = writeln!(
            index,
            ",{},{}",
            csv_field(&e.value),
            csv_field(&format!("error (row {}): {}", e.row, e.reason))
        );
    }

    if entries.is_empty() {
        let first = errors
            .first()
            .map(|e| format!(" (row {}: {})", e.row, e.reason))
            .unwrap_or_default();
        return Err(format!(
            "no QR codes could be generated from {} row{}{first}",
            rows.len(),
            if rows.len() == 1 { "" } else { "s" }
        ));
    }
    if o.include_index {
        entries.push(("index.csv".to_string(), index.into_bytes()));
    }

    let files: Vec<String> = entries.iter().map(|(n, _)| n.clone()).collect();
    let mut buf = Vec::new();
    {
        let mut zw = ZipWriter::new(Cursor::new(&mut buf));
        // SimpleFileOptions::default() carries the fixed 1980-01-01 DOS
        // timestamp (no system clock), which is what makes the archive
        // reproducible byte-for-byte.
        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for (name, bytes) in &entries {
            zw.start_file(name, opts)
                .map_err(|e| format!("zip start_file: {e}"))?;
            zw.write_all(bytes).map_err(|e| format!("zip write: {e}"))?;
        }
        zw.finish().map_err(|e| format!("zip finish: {e}"))?;
    }

    Ok(Batch {
        zip: buf,
        files,
        rows: rows.len(),
        generated,
        errors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn read_zip(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
        let mut a = zip::ZipArchive::new(Cursor::new(bytes)).expect("valid zip");
        (0..a.len())
            .map(|i| {
                let mut f = a.by_index(i).unwrap();
                let name = f.name().to_string();
                let mut data = Vec::new();
                f.read_to_end(&mut data).unwrap();
                (name, data)
            })
            .collect()
    }

    fn text_of(entries: &[(String, Vec<u8>)], name: &str) -> String {
        let (_, b) = entries.iter().find(|(n, _)| n == name).expect(name);
        String::from_utf8(b.clone()).unwrap()
    }

    // --- happy path -------------------------------------------------------

    #[test]
    fn plain_list_makes_one_png_per_line_plus_index() {
        let b = generate_batch(
            "https://example.com\nhello world\n12345",
            &Options::default(),
        )
        .expect("batch");
        assert_eq!(b.rows, 3);
        assert_eq!(b.generated, 3);
        assert!(b.errors.is_empty());
        assert_eq!(
            b.files,
            vec!["qr-001.png", "qr-002.png", "qr-003.png", "index.csv"]
        );

        let entries = read_zip(&b.zip);
        assert_eq!(entries.len(), 4);
        // Every image is a real PNG (magic bytes).
        for (name, data) in &entries {
            if name.ends_with(".png") {
                assert_eq!(&data[..8], b"\x89PNG\r\n\x1a\n", "{name} is not a PNG");
            }
        }
        let index = text_of(&entries, "index.csv");
        assert_eq!(
            index,
            "filename,value,status\n\
             qr-001.png,https://example.com,ok\n\
             qr-002.png,hello world,ok\n\
             qr-003.png,12345,ok\n"
        );
    }

    #[test]
    fn named_csv_rows_drive_the_filenames() {
        let o = Options {
            format: OutFormat::Svg,
            ..Options::default()
        };
        let b = generate_batch(
            "Front Door,https://example.com/a\nBack Door,https://example.com/b",
            &o,
        )
        .expect("batch");
        assert_eq!(
            b.files,
            vec!["Front_Door.svg", "Back_Door.svg", "index.csv"]
        );
        let entries = read_zip(&b.zip);
        let svg = text_of(&entries, "Front_Door.svg");
        assert!(
            svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""),
            "{svg:.80}"
        );
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains(r##"fill="#000000""##));
    }

    #[test]
    fn header_row_is_skipped_and_both_formats_share_a_base_name() {
        let o = Options {
            has_header: true,
            format: OutFormat::Both,
            include_index: false,
            ..Options::default()
        };
        let b = generate_batch("name,url\nalpha,https://example.com/1", &o).expect("batch");
        assert_eq!(b.rows, 1);
        assert_eq!(b.files, vec!["alpha.png", "alpha.svg"]);
    }

    #[test]
    fn tsv_is_auto_detected_and_duplicate_names_are_made_unique() {
        let b = generate_batch(
            "dup\thttps://example.com/1\ndup\thttps://example.com/2",
            &Options::default(),
        )
        .expect("batch");
        assert_eq!(b.files, vec!["dup.png", "dup-2.png", "index.csv"]);
    }

    #[test]
    fn value_only_keeps_commas_inside_the_payload() {
        let o = Options {
            columns: Columns::ValueOnly,
            ..Options::default()
        };
        let b = generate_batch("WIFI:T:WPA;S:Cafe,Bar;P:pw;;", &o).expect("batch");
        let index = text_of(&read_zip(&b.zip), "index.csv");
        assert!(
            index.contains("\"WIFI:T:WPA;S:Cafe,Bar;P:pw;;\""),
            "{index}"
        );
    }

    #[test]
    fn quoted_csv_fields_are_unwrapped() {
        let b = generate_batch("\"a,b\",\"https://example.com/x\"", &Options::default())
            .expect("batch");
        assert_eq!(b.files, vec!["a_b.png", "index.csv"]);
        let index = text_of(&read_zip(&b.zip), "index.csv");
        assert!(
            index.contains("a_b.png,https://example.com/x,ok"),
            "{index}"
        );
    }

    #[test]
    fn margin_and_size_change_the_svg_viewbox_and_png_edge() {
        let tight = Options {
            margin: 0,
            format: OutFormat::Svg,
            ..Options::default()
        };
        let padded = Options {
            margin: 4,
            format: OutFormat::Svg,
            ..Options::default()
        };
        let a = generate_batch("hello", &tight).unwrap();
        let c = generate_batch("hello", &padded).unwrap();
        let sa = text_of(&read_zip(&a.zip), "qr-001.svg");
        let sc = text_of(&read_zip(&c.zip), "qr-001.svg");
        // "hello" is a version-1 code: 21 modules, +8 with a 4-module quiet zone.
        assert!(sa.contains(r#"viewBox="0 0 21 21""#), "{sa:.200}");
        assert!(sc.contains(r#"viewBox="0 0 29 29""#), "{sc:.200}");
        assert!(sc.contains(r#"width="512""#));
    }

    #[test]
    fn transparent_background_omits_the_svg_rect() {
        let o = Options {
            format: OutFormat::Svg,
            bg_color: "transparent".into(),
            fg_color: "#f00".into(),
            ..Options::default()
        };
        let b = generate_batch("hello", &o).unwrap();
        let svg = text_of(&read_zip(&b.zip), "qr-001.svg");
        assert!(!svg.contains("<rect"), "{svg:.200}");
        assert!(svg.contains(r##"fill="#ff0000""##));
    }

    #[test]
    fn output_is_byte_for_byte_reproducible() {
        let a = generate_batch("one\ntwo", &Options::default()).unwrap();
        let b = generate_batch("one\ntwo", &Options::default()).unwrap();
        assert_eq!(a.zip, b.zip);
    }

    // --- errors -----------------------------------------------------------

    #[test]
    fn empty_input_is_an_error() {
        let e = generate_batch("   \n\n", &Options::default()).unwrap_err();
        assert!(e.contains("no rows found"), "{e}");
    }

    #[test]
    fn over_the_row_cap_is_an_error() {
        let data = (0..MAX_ROWS + 1)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let e = generate_batch(&data, &Options::default()).unwrap_err();
        assert!(e.contains("over the 500-row batch cap"), "{e}");
    }

    #[test]
    fn a_too_long_row_is_reported_not_dropped() {
        let long = "x".repeat(2400);
        let b = generate_batch(&format!("ok-row\n{long}"), &Options::default()).expect("batch");
        assert_eq!(b.generated, 1);
        assert_eq!(b.errors.len(), 1);
        assert_eq!(b.errors[0].row, 2);
        assert!(
            b.errors[0].reason.contains("2331-byte QR capacity"),
            "{}",
            b.errors[0].reason
        );
        let index = text_of(&read_zip(&b.zip), "index.csv");
        assert!(index.contains("error (row 2)"), "{index}");
    }

    #[test]
    fn identical_colors_are_rejected() {
        let o = Options {
            fg_color: "#fff".into(),
            bg_color: "white".into(),
            ..Options::default()
        };
        let e = generate_batch("hello", &o).unwrap_err();
        assert!(e.contains("unscannable"), "{e}");
    }

    #[test]
    fn bad_color_and_size_say_what_was_expected() {
        let o = Options {
            fg_color: "chartreuse".into(),
            ..Options::default()
        };
        let e = generate_batch("hello", &o).unwrap_err();
        assert!(e.contains("invalid fg_color 'chartreuse'"), "{e}");

        let o = Options {
            size: 10,
            ..Options::default()
        };
        let e = generate_batch("hello", &o).unwrap_err();
        assert!(e.contains("outside 64-2048 pixels"), "{e}");

        let o = Options {
            fg_color: "transparent".into(),
            ..Options::default()
        };
        let e = generate_batch("hello", &o).unwrap_err();
        assert!(e.contains("cannot be transparent"), "{e}");
    }

    #[test]
    fn name_value_mode_flags_a_one_column_row() {
        let o = Options {
            columns: Columns::NameValue,
            ..Options::default()
        };
        let b = generate_batch("good,https://example.com\nlonely", &o).expect("batch");
        assert_eq!(b.generated, 1);
        assert_eq!(b.errors.len(), 1);
        assert!(
            b.errors[0].reason.contains("only 1 column"),
            "{}",
            b.errors[0].reason
        );
    }

    #[test]
    fn every_row_failing_is_a_batch_error() {
        let o = Options {
            input_format: InputFormat::Csv,
            columns: Columns::NameValue,
            ..Options::default()
        };
        let e = generate_batch("lonely\nalso-lonely", &o).unwrap_err();
        assert!(
            e.contains("no QR codes could be generated from 2 rows"),
            "{e}"
        );
    }

    #[test]
    fn parsers_reject_unknown_values() {
        assert!(InputFormat::parse("xml")
            .unwrap_err()
            .contains("input_format"));
        assert!(Columns::parse("nope").unwrap_err().contains("columns"));
        assert!(Ecc::parse("z").unwrap_err().contains("error_correction"));
        assert!(OutFormat::parse("eps").unwrap_err().contains("format"));
        assert_eq!(InputFormat::parse("AUTO").unwrap(), InputFormat::Auto);
        assert_eq!(Ecc::parse("high").unwrap(), Ecc::High);
    }

    #[test]
    fn summary_mentions_failures() {
        let long = "x".repeat(2400);
        let b = generate_batch(&format!("ok\n{long}"), &Options::default()).unwrap();
        let s = b.summary();
        assert!(
            s.starts_with("generated 1 QR code from 2 rows into 2 files"),
            "{s}"
        );
        assert!(s.contains("1 row failed (first: row 2"), "{s}");
    }
}

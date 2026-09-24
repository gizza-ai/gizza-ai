//! barcode-batch core — turn a pasted list (or a CSV/TSV) into many 1D barcodes
//! at once, bundled either as a ZIP of PNG/SVG files or as one printable PDF
//! label sheet.
//!
//! Pure-Rust (`barcoders` + `image` for PNG + `zip` + `lopdf` + a public-domain
//! 8x8 bitmap font), so it runs on ALL backends: the chat Service Worker, the
//! CLI, and the browser page. No wafer / wasm-bindgen deps here.
//!
//! The renderers take the raw module vector from `barcoders` rather than that
//! crate's own SVG/image generators, because those hard-code the quiet zone and
//! cannot draw the human-readable text, colours or a transparent background.
//!
//! Determinism: the same input + options always produce byte-identical ZIP bytes
//! (entries carry the fixed 1980-01-01 DOS timestamp), so a batch can be diffed
//! in a build pipeline.

use std::collections::HashSet;
use std::io::{Cursor, Write as _};

use zip::write::{SimpleFileOptions, ZipWriter};
use zip::CompressionMethod;

pub mod render;
pub mod sheet;
pub mod symbology;

pub use render::{parse_color, Geometry, Rgba};
pub use sheet::{SheetOptions, SheetPreset};
pub use symbology::{encode, Encoded, Symbology};

/// Most rows accepted in one batch. A memory guard for the 64 MiB wasm sandbox,
/// not a paywall — split a bigger list and run it twice.
pub const MAX_ROWS: usize = 500;
/// Longest single value accepted. Code 128 tops out well below this in practice;
/// the cap exists so one pathological row cannot blow the raster budget.
pub const MAX_VALUE_LEN: usize = 120;
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
            "" | "auto" => Ok(Self::Auto),
            "list" => Ok(Self::List),
            "csv" => Ok(Self::Csv),
            "tsv" => Ok(Self::Tsv),
            other => Err(format!(
                "unknown input format `{other}` — use auto, list, csv or tsv"
            )),
        }
    }

    fn resolve(self, data: &str) -> Self {
        match self {
            Self::Auto => {
                if data.lines().any(|l| l.contains('\t')) {
                    Self::Tsv
                } else if data.lines().any(|l| l.contains(',')) {
                    Self::Csv
                } else {
                    Self::List
                }
            }
            other => other,
        }
    }

    fn delimiter(self) -> Option<char> {
        match self {
            Self::Csv => Some(','),
            Self::Tsv => Some('\t'),
            _ => None,
        }
    }
}

/// Which column carries the barcode value and which carries the filename.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Columns {
    /// Two-or-more-column rows are read as `value,name`; one-column rows are values.
    Auto,
    /// First field is the barcode value, second is the filename.
    ValueName,
    /// First field is the filename, second is the barcode value.
    NameValue,
    /// The whole line is the value, delimiters included.
    ValueOnly,
}

impl Columns {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(Self::Auto),
            "value-name" => Ok(Self::ValueName),
            "name-value" => Ok(Self::NameValue),
            "value-only" => Ok(Self::ValueOnly),
            other => Err(format!(
                "unknown column mapping `{other}` — use auto, value-name, name-value or value-only"
            )),
        }
    }
}

/// File type placed in the ZIP for each row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutFormat {
    Png,
    Svg,
    Both,
}

impl OutFormat {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "png" => Ok(Self::Png),
            "svg" => Ok(Self::Svg),
            "both" => Ok(Self::Both),
            other => Err(format!(
                "unknown file format `{other}` — use png, svg or both"
            )),
        }
    }

    fn wants_png(self) -> bool {
        matches!(self, Self::Png | Self::Both)
    }

    fn wants_svg(self) -> bool {
        matches!(self, Self::Svg | Self::Both)
    }
}

/// What the batch is bundled as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    /// A ZIP of per-row image files plus an optional index.csv.
    Zip,
    /// One printable PDF laid out on a label-sheet grid.
    Sheet,
}

impl Output {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "zip" => Ok(Self::Zip),
            "sheet" | "pdf" => Ok(Self::Sheet),
            other => Err(format!("unknown output `{other}` — use zip or sheet")),
        }
    }
}

/// Everything the batch generator needs beyond the pasted rows.
#[derive(Debug, Clone)]
pub struct Options {
    pub input_format: InputFormat,
    pub columns: Columns,
    pub has_header: bool,
    pub symbology: Symbology,
    pub auto_check_digit: bool,
    pub output: Output,
    pub format: OutFormat,
    pub sheet_preset: SheetPreset,
    pub module_width: u32,
    pub bar_height: u32,
    pub quiet_zone: u32,
    pub show_text: bool,
    pub text_size: u32,
    pub fg_color: String,
    pub bg_color: String,
    pub name_prefix: String,
    pub include_index: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            input_format: InputFormat::Auto,
            columns: Columns::Auto,
            has_header: false,
            symbology: Symbology::Code128,
            auto_check_digit: true,
            output: Output::Zip,
            format: OutFormat::Png,
            sheet_preset: SheetPreset::Avery5160,
            module_width: 2,
            bar_height: 100,
            quiet_zone: 10,
            show_text: true,
            text_size: 20,
            fg_color: "#000000".to_string(),
            bg_color: "#ffffff".to_string(),
            name_prefix: "barcode".to_string(),
            include_index: true,
        }
    }
}

/// The generated bundle plus the per-row audit trail.
#[derive(Debug, Clone)]
pub struct Batch {
    /// ZIP or PDF bytes, depending on [`Options::output`].
    pub bytes: Vec<u8>,
    pub mime: &'static str,
    pub filename: &'static str,
    /// Rows that produced a barcode.
    pub ok: usize,
    /// `(1-based row number, message)` for every row that did not.
    pub errors: Vec<(usize, String)>,
    /// Filenames placed in the archive, in order (empty for sheet output).
    pub names: Vec<String>,
    /// Layout note for sheet output, empty otherwise.
    pub layout: String,
}

impl Batch {
    /// One-line summary for the chat/CLI envelope.
    pub fn summary(&self) -> String {
        let what = match self.mime {
            "application/pdf" => "PDF label sheet",
            _ => "ZIP",
        };
        let mut s = format!(
            "{what}: {} barcode(s) generated, {} bytes",
            self.ok,
            self.bytes.len()
        );
        if !self.layout.is_empty() {
            s.push_str(&format!(". {}", self.layout));
        }
        if !self.errors.is_empty() {
            s.push_str(&format!(". {} row(s) failed: ", self.errors.len()));
            let detail: Vec<String> = self
                .errors
                .iter()
                .take(3)
                .map(|(n, m)| format!("row {n} — {m}"))
                .collect();
            s.push_str(&detail.join("; "));
            if self.errors.len() > 3 {
                s.push_str(&format!("; and {} more", self.errors.len() - 3));
            }
        }
        s
    }
}

/// Split one delimited line into fields, honouring `"…"` quoting with `""` escapes.
fn split_fields(line: &str, delim: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if in_quotes => {
                if chars.peek() == Some(&'"') {
                    cur.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            }
            '"' => in_quotes = true,
            c if c == delim && !in_quotes => {
                out.push(cur.trim().to_string());
                cur = String::new();
            }
            c => cur.push(c),
        }
    }
    out.push(cur.trim().to_string());
    out
}

/// One parsed input row before encoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// 1-based line number in the pasted input, for error messages.
    pub line: usize,
    pub value: String,
    /// Explicit filename base from a name column, if any.
    pub name: Option<String>,
}

/// Split the pasted blob into rows. Blank lines are skipped; the reported line
/// number is the real line in the user's paste so errors are locatable.
pub fn parse_rows(data: &str, opts: &Options) -> Result<Vec<Row>, String> {
    let fmt = opts.input_format.resolve(data);
    let delim = fmt.delimiter();
    let mut rows = Vec::new();
    let mut seen_header = false;
    for (i, raw) in data.lines().enumerate() {
        let line = raw.trim_end_matches(['\r']).trim();
        if line.is_empty() {
            continue;
        }
        if opts.has_header && !seen_header {
            seen_header = true;
            continue;
        }
        let (value, name) = match (delim, opts.columns) {
            (_, Columns::ValueOnly) | (None, _) => (line.to_string(), None),
            (Some(d), mapping) => {
                let fields = split_fields(line, d);
                match mapping {
                    Columns::NameValue if fields.len() >= 2 => {
                        (fields[1].clone(), Some(fields[0].clone()))
                    }
                    Columns::NameValue => (fields[0].clone(), None),
                    Columns::ValueName | Columns::Auto if fields.len() >= 2 => {
                        (fields[0].clone(), Some(fields[1].clone()))
                    }
                    _ => (fields[0].clone(), None),
                }
            }
        };
        if value.is_empty() {
            continue;
        }
        if value.len() > MAX_VALUE_LEN {
            return Err(format!(
                "line {}: value is {} characters — the limit is {MAX_VALUE_LEN}",
                i + 1,
                value.len()
            ));
        }
        rows.push(Row {
            line: i + 1,
            value,
            name: name.filter(|n| !n.is_empty()),
        });
        if rows.len() > MAX_ROWS {
            return Err(format!(
                "too many rows: the limit is {MAX_ROWS} per batch — split the list and run it twice"
            ));
        }
    }
    if rows.is_empty() {
        return Err("no rows found — paste one value per line, or CSV/TSV rows".to_string());
    }
    Ok(rows)
}

/// Reduce an arbitrary name to a safe archive filename base.
fn sanitize(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let s = s.trim_matches(['-', '.']).to_string();
    if s.is_empty() {
        "row".to_string()
    } else {
        s.chars().take(64).collect()
    }
}

fn csv_cell(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn geometry(opts: &Options) -> Geometry {
    Geometry {
        module_width: opts.module_width,
        bar_height: opts.bar_height,
        quiet_zone: opts.quiet_zone,
        show_text: opts.show_text,
        text_size: opts.text_size,
    }
}

fn validate(opts: &Options) -> Result<(), String> {
    if !(1..=10).contains(&opts.module_width) {
        return Err("module width must be between 1 and 10 pixels".to_string());
    }
    if !(20..=400).contains(&opts.bar_height) {
        return Err("bar height must be between 20 and 400 pixels".to_string());
    }
    if opts.quiet_zone > 30 {
        return Err("quiet zone must be 30 modules or fewer".to_string());
    }
    if !(8..=48).contains(&opts.text_size) {
        return Err("text size must be between 8 and 48 pixels".to_string());
    }
    Ok(())
}

/// Generate the whole batch. Rows that cannot be encoded are reported in
/// `Batch::errors` (and in index.csv) rather than aborting the run; only a batch
/// where NO row encodes is an error.
pub fn generate_batch(data: &str, opts: &Options) -> Result<Batch, String> {
    validate(opts)?;
    let fg = parse_color(&opts.fg_color, "Foreground colour")?;
    let bg = parse_color(&opts.bg_color, "Background colour")?;
    if fg.is_transparent() {
        return Err("the bar colour cannot be transparent — nothing would scan".to_string());
    }
    let rows = parse_rows(data, opts)?;
    let geo = geometry(opts);

    let mut encoded: Vec<(Row, Encoded)> = Vec::new();
    let mut errors: Vec<(usize, String)> = Vec::new();
    for row in rows {
        match encode(opts.symbology, &row.value, opts.auto_check_digit) {
            Ok(e) => encoded.push((row, e)),
            Err(msg) => errors.push((row.line, msg)),
        }
    }
    if encoded.is_empty() {
        let first = errors
            .first()
            .map(|(n, m)| format!(" (line {n}: {m})"))
            .unwrap_or_default();
        return Err(format!(
            "no row could be encoded as {}{first}",
            opts.symbology.label()
        ));
    }

    match opts.output {
        Output::Sheet => {
            let symbols: Vec<Encoded> = encoded.iter().map(|(_, e)| e.clone()).collect();
            let bytes = sheet::render_sheet(
                &symbols,
                &SheetOptions {
                    preset: opts.sheet_preset,
                    quiet_zone: opts.quiet_zone,
                    show_text: opts.show_text,
                    text_size: opts.text_size,
                    fg,
                    bg,
                },
            )?;
            Ok(Batch {
                ok: symbols.len(),
                layout: sheet::describe(opts.sheet_preset, symbols.len()),
                bytes,
                mime: "application/pdf",
                filename: "barcode-sheet.pdf",
                errors,
                names: Vec::new(),
            })
        }
        Output::Zip => {
            let mut buf = Cursor::new(Vec::new());
            let mut zipw = ZipWriter::new(&mut buf);
            // Fixed timestamp => byte-identical archives for identical inputs.
            let zopts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
            let mut used: HashSet<String> = HashSet::new();
            let mut names = Vec::new();
            let mut index = String::from("filename,value,symbology,status\n");
            let mut total = 0usize;
            let mut auto_n = 0usize;

            for (row, enc) in &encoded {
                let base = match &row.name {
                    Some(n) => sanitize(n),
                    None => {
                        auto_n += 1;
                        format!("{}-{auto_n:03}", sanitize(&opts.name_prefix))
                    }
                };
                let mut base = base;
                let mut dedupe = 1;
                while used.contains(&base) {
                    dedupe += 1;
                    base = format!("{base}-{dedupe}");
                }
                used.insert(base.clone());

                let mut write_entry = |zipw: &mut ZipWriter<&mut Cursor<Vec<u8>>>,
                                       name: String,
                                       bytes: Vec<u8>|
                 -> Result<(), String> {
                    total += bytes.len();
                    if total > MAX_TOTAL_BYTES {
                        return Err(format!(
                            "generated output exceeds {} MB — lower the image size or split the batch",
                            MAX_TOTAL_BYTES / (1024 * 1024)
                        ));
                    }
                    zipw.start_file(&name, zopts)
                        .map_err(|e| format!("could not add {name} to the archive: {e}"))?;
                    zipw.write_all(&bytes)
                        .map_err(|e| format!("could not write {name}: {e}"))?;
                    names.push(name.clone());
                    index.push_str(&format!(
                        "{},{},{},ok\n",
                        csv_cell(&name),
                        csv_cell(&enc.hri),
                        csv_cell(enc.symbology.label())
                    ));
                    Ok(())
                };

                if opts.format.wants_png() {
                    let png = render::render_png(enc, &geo, fg, bg)?;
                    write_entry(&mut zipw, format!("{base}.png"), png)?;
                }
                if opts.format.wants_svg() {
                    let svg = render::render_svg(enc, &geo, fg, bg);
                    write_entry(&mut zipw, format!("{base}.svg"), svg.into_bytes())?;
                }
            }

            for (line, msg) in &errors {
                index.push_str(&format!(
                    ",,,{}\n",
                    csv_cell(&format!("error (line {line}): {msg}"))
                ));
            }

            if opts.include_index {
                zipw.start_file("index.csv", zopts)
                    .map_err(|e| format!("could not add index.csv: {e}"))?;
                zipw.write_all(index.as_bytes())
                    .map_err(|e| format!("could not write index.csv: {e}"))?;
                names.push("index.csv".to_string());
            }
            zipw.finish()
                .map_err(|e| format!("could not finish the archive: {e}"))?;

            Ok(Batch {
                ok: encoded.len(),
                bytes: buf.into_inner(),
                mime: "application/zip",
                filename: "barcode-batch.zip",
                errors,
                names,
                layout: String::new(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options::default()
    }

    #[test]
    fn zip_of_svgs_carries_named_entries_and_an_index() {
        let mut o = opts();
        o.format = OutFormat::Svg;
        o.input_format = InputFormat::Csv;
        o.columns = Columns::ValueName;
        let b = generate_batch("SKU-1001,widget\nSKU-1002,gadget", &o).unwrap();
        assert_eq!(b.mime, "application/zip");
        assert_eq!(b.filename, "barcode-batch.zip");
        assert_eq!(b.ok, 2);
        assert_eq!(b.names, vec!["widget.svg", "gadget.svg", "index.csv"]);
        assert_eq!(&b.bytes[..4], b"PK\x03\x04");
        assert!(b.summary().starts_with("ZIP: 2 barcode(s) generated"));
    }

    #[test]
    fn auto_filenames_are_numbered_from_the_prefix() {
        let mut o = opts();
        o.format = OutFormat::Svg;
        o.name_prefix = "asset".to_string();
        o.include_index = false;
        let b = generate_batch("AAA\nBBB\nCCC", &o).unwrap();
        assert_eq!(b.names, vec!["asset-001.svg", "asset-002.svg", "asset-003.svg"]);
    }

    #[test]
    fn upc_check_digits_are_computed_across_the_batch() {
        let mut o = opts();
        o.symbology = Symbology::UpcA;
        o.format = OutFormat::Svg;
        let b = generate_batch("03600029145\n01234565000", &o).unwrap();
        assert_eq!(b.ok, 2);
        assert!(b.errors.is_empty());
    }

    #[test]
    fn a_bad_row_is_reported_without_killing_the_batch() {
        let mut o = opts();
        o.symbology = Symbology::Ean13;
        o.format = OutFormat::Svg;
        let b = generate_batch("5901234123457\nnot-a-number", &o).unwrap();
        assert_eq!(b.ok, 1);
        assert_eq!(b.errors.len(), 1);
        assert_eq!(b.errors[0].0, 2);
        assert!(b.summary().contains("1 row(s) failed"), "{}", b.summary());
    }

    #[test]
    fn a_batch_where_nothing_encodes_is_an_error() {
        let mut o = opts();
        o.symbology = Symbology::Ean13;
        let err = generate_batch("abc\ndef", &o).unwrap_err();
        assert!(err.contains("no row could be encoded as EAN-13"), "{err}");
    }

    #[test]
    fn empty_input_is_rejected_with_guidance() {
        let err = generate_batch("   \n\n", &opts()).unwrap_err();
        assert!(err.contains("no rows found"), "{err}");
    }

    #[test]
    fn header_row_is_skipped() {
        let mut o = opts();
        o.has_header = true;
        o.format = OutFormat::Svg;
        o.input_format = InputFormat::Csv;
        o.columns = Columns::ValueName;
        let b = generate_batch("sku,label\nSKU-1,first", &o).unwrap();
        assert_eq!(b.names, vec!["first.svg", "index.csv"]);
    }

    #[test]
    fn value_only_keeps_commas_inside_the_payload() {
        let mut o = opts();
        o.columns = Columns::ValueOnly;
        o.format = OutFormat::Svg;
        o.include_index = false;
        let b = generate_batch("ACME, INC", &o).unwrap();
        assert_eq!(b.ok, 1);
        assert_eq!(b.names, vec!["barcode-001.svg"]);
    }

    #[test]
    fn both_formats_emit_two_files_per_row() {
        let mut o = opts();
        o.format = OutFormat::Both;
        o.include_index = false;
        let b = generate_batch("AB\nCD", &o).unwrap();
        assert_eq!(
            b.names,
            vec!["barcode-001.png", "barcode-001.svg", "barcode-002.png", "barcode-002.svg"]
        );
    }

    #[test]
    fn duplicate_names_are_deduplicated() {
        let mut o = opts();
        o.format = OutFormat::Svg;
        o.input_format = InputFormat::Csv;
        o.columns = Columns::ValueName;
        o.include_index = false;
        let b = generate_batch("AA,same\nBB,same", &o).unwrap();
        assert_eq!(b.names, vec!["same.svg", "same-2.svg"]);
    }

    #[test]
    fn sheet_output_is_a_pdf() {
        let mut o = opts();
        o.output = Output::Sheet;
        o.sheet_preset = SheetPreset::Avery5163;
        let b = generate_batch("SKU-1\nSKU-2\nSKU-3", &o).unwrap();
        assert_eq!(b.mime, "application/pdf");
        assert_eq!(b.filename, "barcode-sheet.pdf");
        assert_eq!(&b.bytes[..5], b"%PDF-");
        assert!(b.layout.contains("10 up"), "{}", b.layout);
        assert!(b.summary().contains("PDF label sheet"), "{}", b.summary());
    }

    #[test]
    fn zip_output_is_deterministic() {
        let mut o = opts();
        o.format = OutFormat::Svg;
        let a = generate_batch("AA\nBB", &o).unwrap();
        let b = generate_batch("AA\nBB", &o).unwrap();
        assert_eq!(a.bytes, b.bytes);
    }

    #[test]
    fn the_row_cap_is_enforced() {
        let data = (0..MAX_ROWS + 1)
            .map(|i| format!("SKU{i:04}"))
            .collect::<Vec<_>>()
            .join("\n");
        let err = generate_batch(&data, &opts()).unwrap_err();
        assert!(err.contains("too many rows"), "{err}");
    }

    #[test]
    fn transparent_bars_are_refused() {
        let mut o = opts();
        o.fg_color = "transparent".to_string();
        let err = generate_batch("AB", &o).unwrap_err();
        assert!(err.contains("cannot be transparent"), "{err}");
    }

    #[test]
    fn out_of_range_geometry_is_refused() {
        let mut o = opts();
        o.module_width = 99;
        assert!(generate_batch("AB", &o).unwrap_err().contains("module width"));
    }

    #[test]
    fn tsv_is_auto_detected() {
        let mut o = opts();
        o.format = OutFormat::Svg;
        o.include_index = false;
        let b = generate_batch("AA\tfirst\nBB\tsecond", &o).unwrap();
        assert_eq!(b.names, vec!["first.svg", "second.svg"]);
    }

    #[test]
    fn quoted_csv_fields_are_honoured() {
        let mut o = opts();
        o.format = OutFormat::Svg;
        o.include_index = false;
        o.input_format = InputFormat::Csv;
        let rows = parse_rows("\"AA,BB\",name one", &o).unwrap();
        assert_eq!(rows[0].value, "AA,BB");
        assert_eq!(rows[0].name.as_deref(), Some("name one"));
    }
}

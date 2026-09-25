//! Printable label-sheet output: every encoded row laid out on a vector PDF grid.
//!
//! Bars are drawn as filled rectangles in the page content stream (not embedded
//! rasters), so the sheet prints at the printer's own resolution and stays a few
//! KB no matter how many labels it carries. The human-readable text uses the
//! base-14 Helvetica font, so no font file is embedded either.
//!
//! Each label auto-fits: the module width shrinks until the symbol plus its quiet
//! zones fits the label's usable width, so the same batch prints correctly on a
//! 65-up address sheet and a 10-up shipping sheet without re-tuning pixel options.
//!
//! The PDF is written by hand rather than through a PDF crate. `lopdf` — the
//! crate the other PDF-emitting blocks use — has a non-optional `rand` ->
//! `getrandom` dependency (for PDF encryption, which this tool never touches),
//! and `getrandom` has no backend on wasm32-unknown-unknown, so it cannot build
//! for the browser page target. What this file needs from PDF is small and fully
//! specified: filled rectangles, one base-14 font, and a correct xref table.
//! Writing it directly also makes the output byte-deterministic.

use std::fmt::Write as _;

use flate2::write::ZlibEncoder;
use flate2::Compression;

use crate::render::{bar_runs, Rgba};
use crate::symbology::Encoded;

const PT_PER_MM: f64 = 72.0 / 25.4;
const PT_PER_IN: f64 = 72.0;

/// A label-sheet geometry. All fields are PostScript points (72 per inch).
#[derive(Debug, Clone, Copy)]
pub struct SheetGeometry {
    pub page_w: f64,
    pub page_h: f64,
    /// Distance from the page's left edge to the first column's left edge.
    pub margin_left: f64,
    /// Distance from the page's TOP edge to the first row's top edge.
    pub margin_top: f64,
    pub label_w: f64,
    pub label_h: f64,
    /// Column pitch (label width + horizontal gutter).
    pub pitch_x: f64,
    /// Row pitch (label height + vertical gutter).
    pub pitch_y: f64,
    pub cols: usize,
    pub rows: usize,
}

impl SheetGeometry {
    pub fn per_page(&self) -> usize {
        self.cols * self.rows
    }
}

/// The label stocks offered on the page. Dimensions are the published label
/// geometries for these widely-cloned stock sizes; the generic grids are ours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SheetPreset {
    Avery5160,
    Avery5161,
    Avery5163,
    AveryL7651,
    AveryL7160,
    A4Grid,
}

impl SheetPreset {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().replace(['_', ' '], "-").as_str() {
            "" | "avery-5160" => Ok(Self::Avery5160),
            "avery-5161" => Ok(Self::Avery5161),
            "avery-5163" => Ok(Self::Avery5163),
            "avery-l7651" => Ok(Self::AveryL7651),
            "avery-l7160" => Ok(Self::AveryL7160),
            "a4-grid" => Ok(Self::A4Grid),
            other => Err(format!(
                "unknown sheet layout `{other}` — use avery-5160, avery-5161, avery-5163, avery-l7651, avery-l7160 or a4-grid"
            )),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Avery5160 => "Avery 5160 — US Letter, 30 up (2.625 x 1 in)",
            Self::Avery5161 => "Avery 5161 — US Letter, 20 up (4 x 1 in)",
            Self::Avery5163 => "Avery 5163 — US Letter, 10 up (4 x 2 in)",
            Self::AveryL7651 => "Avery L7651 — A4, 65 up (38.1 x 21.2 mm)",
            Self::AveryL7160 => "Avery L7160 — A4, 21 up (63.5 x 38.1 mm)",
            Self::A4Grid => "Generic A4 grid — 40 up (45 x 25 mm)",
        }
    }

    pub fn geometry(self) -> SheetGeometry {
        let letter = (8.5 * PT_PER_IN, 11.0 * PT_PER_IN);
        let a4 = (210.0 * PT_PER_MM, 297.0 * PT_PER_MM);
        match self {
            Self::Avery5160 => SheetGeometry {
                page_w: letter.0,
                page_h: letter.1,
                margin_left: 0.1875 * PT_PER_IN,
                margin_top: 0.5 * PT_PER_IN,
                label_w: 2.625 * PT_PER_IN,
                label_h: 1.0 * PT_PER_IN,
                pitch_x: 2.75 * PT_PER_IN,
                pitch_y: 1.0 * PT_PER_IN,
                cols: 3,
                rows: 10,
            },
            Self::Avery5161 => SheetGeometry {
                page_w: letter.0,
                page_h: letter.1,
                margin_left: 0.15625 * PT_PER_IN,
                margin_top: 0.5 * PT_PER_IN,
                label_w: 4.0 * PT_PER_IN,
                label_h: 1.0 * PT_PER_IN,
                pitch_x: 4.1875 * PT_PER_IN,
                pitch_y: 1.0 * PT_PER_IN,
                cols: 2,
                rows: 10,
            },
            Self::Avery5163 => SheetGeometry {
                page_w: letter.0,
                page_h: letter.1,
                margin_left: 0.15625 * PT_PER_IN,
                margin_top: 0.5 * PT_PER_IN,
                label_w: 4.0 * PT_PER_IN,
                label_h: 2.0 * PT_PER_IN,
                pitch_x: 4.1875 * PT_PER_IN,
                pitch_y: 2.0 * PT_PER_IN,
                cols: 2,
                rows: 5,
            },
            Self::AveryL7651 => SheetGeometry {
                page_w: a4.0,
                page_h: a4.1,
                margin_left: 4.75 * PT_PER_MM,
                margin_top: 10.7 * PT_PER_MM,
                label_w: 38.1 * PT_PER_MM,
                label_h: 21.2 * PT_PER_MM,
                pitch_x: 40.6 * PT_PER_MM,
                pitch_y: 21.2 * PT_PER_MM,
                cols: 5,
                rows: 13,
            },
            Self::AveryL7160 => SheetGeometry {
                page_w: a4.0,
                page_h: a4.1,
                margin_left: 7.2 * PT_PER_MM,
                margin_top: 15.15 * PT_PER_MM,
                label_w: 63.5 * PT_PER_MM,
                label_h: 38.1 * PT_PER_MM,
                pitch_x: 66.0 * PT_PER_MM,
                pitch_y: 38.1 * PT_PER_MM,
                cols: 3,
                rows: 7,
            },
            Self::A4Grid => SheetGeometry {
                page_w: a4.0,
                page_h: a4.1,
                margin_left: 10.0 * PT_PER_MM,
                margin_top: 13.5 * PT_PER_MM,
                label_w: 45.0 * PT_PER_MM,
                label_h: 25.0 * PT_PER_MM,
                pitch_x: 47.5 * PT_PER_MM,
                pitch_y: 27.0 * PT_PER_MM,
                cols: 4,
                rows: 10,
            },
        }
    }
}

/// Helvetica's average advance is close enough to 0.55 em for centring a short
/// numeric caption; the exact AFM widths would buy nothing at this size.
fn caption_width(text: &str, size: f64) -> f64 {
    text.chars().count() as f64 * size * 0.55
}

fn escape_pdf_text(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '(' | ')' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 128 => out.push(c),
            // Non-Latin-1 characters have no glyph in the base-14 encoding.
            _ => out.push('?'),
        }
    }
    out
}

/// Format a PDF real. Two decimals is well below a printer dot at any label
/// size and keeps the content stream small; `-0` is normalised away so the
/// output stays byte-identical for identical input.
fn n(v: f64) -> String {
    let s = format!("{:.2}", v);
    if s == "-0.00" {
        "0".to_string()
    } else {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn push_color(s: &mut String, c: Rgba) {
    let _ = writeln!(
        s,
        "{} {} {} rg",
        n(c.0 as f64 / 255.0),
        n(c.1 as f64 / 255.0),
        n(c.2 as f64 / 255.0)
    );
}

fn push_rect(s: &mut String, x: f64, y: f64, w: f64, h: f64) {
    let _ = writeln!(s, "{} {} {} {} re", n(x), n(y), n(w), n(h));
}

/// Options the sheet renderer needs from the caller's [`crate::Options`].
#[derive(Debug, Clone, Copy)]
pub struct SheetOptions {
    pub preset: SheetPreset,
    pub quiet_zone: u32,
    pub show_text: bool,
    pub text_size: u32,
    pub fg: Rgba,
    pub bg: Rgba,
}

/// Lay `rows` out on as many pages as the preset needs and return the PDF bytes.
///
/// Object layout: 1 = Catalog, 2 = Pages (which carries the inheritable
/// `/Resources` and `/MediaBox`), 3 = the Helvetica font, then two objects per
/// page — `4 + 2k` the page, `5 + 2k` its content stream.
pub fn render_sheet(rows: &[Encoded], opts: &SheetOptions) -> Result<Vec<u8>, String> {
    if rows.is_empty() {
        return Err("no rows could be encoded, so there is nothing to lay out".to_string());
    }
    let geo = opts.preset.geometry();
    let per_page = geo.per_page();

    let streams: Vec<Vec<u8>> = rows
        .chunks(per_page)
        .map(|chunk| {
            let mut s = String::new();
            if !opts.bg.is_transparent() {
                push_color(&mut s, opts.bg);
                push_rect(&mut s, 0.0, 0.0, geo.page_w, geo.page_h);
                s.push_str("f\n");
            }
            push_color(&mut s, opts.fg);
            for (i, enc) in chunk.iter().enumerate() {
                let col = i % geo.cols;
                let row = i / geo.cols;
                let left = geo.margin_left + col as f64 * geo.pitch_x;
                // PDF's origin is bottom-left; the preset's margin is from the top.
                let top = geo.page_h - geo.margin_top - row as f64 * geo.pitch_y;
                draw_label(&mut s, enc, left, top, &geo, opts);
            }
            deflate(s.as_bytes())
        })
        .collect::<Result<Vec<_>, String>>()?;

    let page_count = streams.len();
    let obj_count = 4 + 2 * page_count; // + the free entry at index 0
    let mut out: Vec<u8> = Vec::new();
    // Offsets are 1-indexed by object number; slot 0 is the free head entry.
    let mut offsets: Vec<usize> = vec![0; obj_count];

    out.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");

    let begin = |out: &mut Vec<u8>, offsets: &mut Vec<usize>, id: usize| {
        offsets[id] = out.len();
        out.extend_from_slice(format!("{id} 0 obj\n").as_bytes());
    };

    begin(&mut out, &mut offsets, 1);
    out.extend_from_slice(b"<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");

    begin(&mut out, &mut offsets, 2);
    let kids: Vec<String> = (0..page_count).map(|k| format!("{} 0 R", 4 + 2 * k)).collect();
    out.extend_from_slice(
        format!(
            "<< /Type /Pages /Count {page_count} /Kids [{}] \
             /Resources << /Font << /F1 3 0 R >> /ProcSet [/PDF /Text] >> \
             /MediaBox [0 0 {} {}] >>\nendobj\n",
            kids.join(" "),
            n(geo.page_w),
            n(geo.page_h)
        )
        .as_bytes(),
    );

    begin(&mut out, &mut offsets, 3);
    out.extend_from_slice(
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>\nendobj\n",
    );

    for (k, stream) in streams.iter().enumerate() {
        let page_id = 4 + 2 * k;
        let content_id = page_id + 1;
        begin(&mut out, &mut offsets, page_id);
        out.extend_from_slice(
            format!("<< /Type /Page /Parent 2 0 R /Contents {content_id} 0 R >>\nendobj\n")
                .as_bytes(),
        );
        begin(&mut out, &mut offsets, content_id);
        out.extend_from_slice(
            format!(
                "<< /Length {} /Filter /FlateDecode >>\nstream\n",
                stream.len()
            )
            .as_bytes(),
        );
        out.extend_from_slice(stream);
        out.extend_from_slice(b"\nendstream\nendobj\n");
    }

    let xref_at = out.len();
    out.extend_from_slice(format!("xref\n0 {obj_count}\n").as_bytes());
    // Each xref entry is exactly 20 bytes, per the spec.
    out.extend_from_slice(b"0000000000 65535 f \n");
    for off in offsets.iter().skip(1) {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {obj_count} /Root 1 0 R >>\nstartxref\n{xref_at}\n%%EOF\n"
        )
        .as_bytes(),
    );
    Ok(out)
}

fn deflate(data: &[u8]) -> Result<Vec<u8>, String> {
    use std::io::Write as _;
    let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
    e.write_all(data)
        .map_err(|err| format!("could not compress the PDF page: {err}"))?;
    e.finish()
        .map_err(|err| format!("could not compress the PDF page: {err}"))
}

/// Draw one label into the page content stream, auto-fitting the symbol to the cell.
fn draw_label(
    s: &mut String,
    enc: &Encoded,
    left: f64,
    top: f64,
    geo: &SheetGeometry,
    opts: &SheetOptions,
) {
    let pad = 3.0_f64.min(geo.label_w * 0.05);
    let avail_w = (geo.label_w - 2.0 * pad).max(1.0);
    let avail_h = (geo.label_h - 2.0 * pad).max(1.0);

    let total_modules = enc.modules.len() as f64 + 2.0 * opts.quiet_zone as f64;
    let module_pt = avail_w / total_modules;

    // Page pixels are 96 dpi; PDF points are 72 dpi.
    let mut caption_pt = if opts.show_text {
        (opts.text_size as f64 * 0.75).clamp(4.0, avail_h * 0.35)
    } else {
        0.0
    };
    // A caption wider than the label would spill into the neighbouring one.
    if caption_pt > 0.0 {
        let max_for_width = avail_w / (enc.hri.chars().count().max(1) as f64 * 0.55);
        caption_pt = caption_pt.min(max_for_width);
    }
    let gap = if caption_pt > 0.0 { caption_pt * 0.25 } else { 0.0 };
    let bar_h = (avail_h - caption_pt - gap).max(1.0);

    let symbol_w = total_modules * module_pt;
    let x0 = left + pad + (avail_w - symbol_w) / 2.0 + opts.quiet_zone as f64 * module_pt;
    let bars_bottom = top - pad - avail_h + caption_pt + gap;

    for (start, len) in bar_runs(&enc.modules) {
        let x = x0 + start as f64 * module_pt;
        push_rect(s, x, bars_bottom, len as f64 * module_pt, bar_h);
    }
    s.push_str("f\n");

    if caption_pt > 0.0 {
        let tw = caption_width(&enc.hri, caption_pt);
        let tx = left + pad + (avail_w - tw) / 2.0;
        let ty = top - pad - avail_h + caption_pt * 0.2;
        let _ = write!(
            s,
            "BT\n/F1 {} Tf\n{} {} Td\n({}) Tj\nET\n",
            n(caption_pt),
            n(tx),
            n(ty),
            escape_pdf_text(&enc.hri)
        );
    }
}

/// One-line human description of a preset, for the page and index.csv.
pub fn describe(preset: SheetPreset, rows: usize) -> String {
    let g = preset.geometry();
    let pages = rows.div_ceil(g.per_page()).max(1);
    let mut s = String::new();
    let _ = write!(
        s,
        "{} — {} label(s) across {} page(s), {} per page",
        preset.label(),
        rows,
        pages,
        g.per_page()
    );
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbology::{encode, Symbology};

    #[test]
    fn sheet_is_a_real_pdf_with_the_expected_page_count() {
        let rows: Vec<_> = (0..35)
            .map(|i| encode(Symbology::Code128, &format!("SKU-{i:04}"), true).unwrap())
            .collect();
        let opts = SheetOptions {
            preset: SheetPreset::Avery5160,
            quiet_zone: 10,
            show_text: true,
            text_size: 20,
            fg: Rgba(0, 0, 0, 255),
            bg: Rgba(255, 255, 255, 255),
        };
        let pdf = render_sheet(&rows, &opts).unwrap();
        assert_eq!(&pdf[..5], b"%PDF-");
        // 30 labels per Avery 5160 sheet -> 35 rows needs 2 pages.
        assert_eq!(SheetPreset::Avery5160.geometry().per_page(), 30);
        assert!(describe(SheetPreset::Avery5160, 35).contains("2 page(s)"));
        // Validate the document structure, not just a plausible byte prefix: a
        // wrong xref offset is the failure mode a hand-written PDF actually has,
        // and every reader resolves objects through that table.
        let text = String::from_utf8_lossy(&pdf).to_string();
        assert!(text.ends_with("%%EOF\n"), "missing trailer");
        assert!(text.contains("/Type /Pages /Count 2 "), "wrong page count");
        assert_eq!(text.matches("/Type /Page /Parent").count(), 2);
        assert_xref_resolves(&pdf);
    }

    /// Walk the xref table and confirm every offset lands on that object's own
    /// `<id> 0 obj` header. Byte offsets are validated against the RAW bytes —
    /// the deflated content streams are not valid UTF-8, so a lossy string copy
    /// would shift every offset past the first stream.
    fn assert_xref_resolves(pdf: &[u8]) {
        let tail_at = find(pdf, b"startxref\n").expect("startxref must be present");
        let tail = &pdf[tail_at + b"startxref\n".len()..];
        let nl = find(tail, b"\n").expect("startxref offset must end in a newline");
        let start: usize = std::str::from_utf8(&tail[..nl])
            .unwrap()
            .trim()
            .parse()
            .expect("startxref must carry an offset");
        let table = &pdf[start..];
        assert!(
            table.starts_with(b"xref\n"),
            "startxref must point at the table"
        );
        // "xref\n" then the "0 <count>\n" subsection header, then 20-byte entries
        // starting with the free head entry for object 0.
        let hdr_end = find(&table[5..], b"\n").unwrap() + 5;
        let count: usize = std::str::from_utf8(&table[5..hdr_end])
            .unwrap()
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .expect("xref subsection header");
        let entries = &table[hdr_end + 1..];
        for id in 1..count {
            let entry = &entries[id * 20..id * 20 + 20];
            let off: usize = std::str::from_utf8(&entry[..10]).unwrap().parse().unwrap();
            let want = format!("{id} 0 obj");
            assert!(
                pdf[off..].starts_with(want.as_bytes()),
                "xref entry {id} points at {:?}",
                String::from_utf8_lossy(&pdf[off..(off + 12).min(pdf.len())])
            );
        }
    }

    fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
        hay.windows(needle.len()).position(|w| w == needle)
    }

    #[test]
    fn empty_input_is_an_error_not_a_blank_sheet() {
        let opts = SheetOptions {
            preset: SheetPreset::A4Grid,
            quiet_zone: 10,
            show_text: false,
            text_size: 20,
            fg: Rgba(0, 0, 0, 255),
            bg: Rgba(255, 255, 255, 255),
        };
        assert!(render_sheet(&[], &opts).is_err());
    }

    #[test]
    fn every_preset_has_a_sane_grid() {
        for p in [
            SheetPreset::Avery5160,
            SheetPreset::Avery5161,
            SheetPreset::Avery5163,
            SheetPreset::AveryL7651,
            SheetPreset::AveryL7160,
            SheetPreset::A4Grid,
        ] {
            let g = p.geometry();
            assert!(g.per_page() > 0, "{p:?}");
            // The last column/row must still fit inside the sheet.
            let right = g.margin_left + (g.cols - 1) as f64 * g.pitch_x + g.label_w;
            let bottom = g.margin_top + (g.rows - 1) as f64 * g.pitch_y + g.label_h;
            assert!(right <= g.page_w + 0.5, "{p:?} overflows width: {right} > {}", g.page_w);
            assert!(bottom <= g.page_h + 0.5, "{p:?} overflows height: {bottom} > {}", g.page_h);
        }
    }

    #[test]
    fn preset_parse_rejects_nonsense() {
        assert_eq!(SheetPreset::parse("avery-5163").unwrap(), SheetPreset::Avery5163);
        assert_eq!(SheetPreset::parse("A4_GRID").unwrap(), SheetPreset::A4Grid);
        assert!(SheetPreset::parse("avery-9999").is_err());
    }
}

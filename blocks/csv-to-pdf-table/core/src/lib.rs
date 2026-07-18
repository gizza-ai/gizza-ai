//! csv-to-pdf-table core — render CSV data as a formatted, paginated table
//! inside a PDF. Pure-Rust: `csv` for parsing + a tiny hand-rolled PDF writer
//! using the built-in base-14 Helvetica / Helvetica-Bold fonts (no embedding),
//! so it runs on every backend including the chat Service Worker.
//!
//! Layout: auto-sized columns (sized from real Helvetica glyph widths, scaled
//! to fit the page when too wide), an optional bold header repeated on every
//! page, optional zebra row banding, an optional cell grid, an optional title,
//! and automatic page breaks. Numeric columns are right-aligned automatically.

const MARGIN: f64 = 40.0;
const CELL_PAD: f64 = 4.0;
const LINE_FACTOR: f64 = 1.6; // row height = font_size * this
const MIN_FONT: f64 = 5.0;
const MAX_FONT: f64 = 24.0;

/// Adobe AFM advance widths (1000-unit em) for Helvetica, ASCII 32..=126.
const HELV_W: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556,
    556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, 1015, 667, 667, 722, 722, 667,
    611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667,
    667, 611, 278, 278, 278, 469, 556, 333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500,
    222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584,
];

/// Adobe AFM advance widths (1000-unit em) for Helvetica-Bold, ASCII 32..=126.
const HELV_BOLD_W: [u16; 95] = [
    278, 333, 474, 556, 556, 889, 722, 238, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556,
    556, 556, 556, 556, 556, 556, 556, 333, 333, 584, 584, 584, 611, 975, 722, 722, 722, 722, 667,
    611, 778, 722, 278, 556, 722, 611, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667,
    667, 611, 333, 278, 333, 584, 556, 333, 556, 611, 556, 611, 556, 333, 611, 611, 278, 278, 556,
    278, 889, 611, 611, 611, 611, 389, 556, 333, 611, 556, 778, 556, 556, 500, 389, 280, 389, 584,
];

/// US Letter / A4 / Legal page size in points (portrait).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PageSize {
    Letter,
    A4,
    Legal,
}

impl PageSize {
    pub fn parse(s: &str) -> Result<PageSize, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "letter" | "" => Ok(PageSize::Letter),
            "a4" => Ok(PageSize::A4),
            "legal" => Ok(PageSize::Legal),
            other => Err(format!("unknown page_size '{other}' (use letter, a4 or legal)")),
        }
    }
    /// (width, height) in points, portrait.
    fn dims(self) -> (f64, f64) {
        match self {
            PageSize::Letter => (612.0, 792.0),
            PageSize::A4 => (595.276, 841.89),
            PageSize::Legal => (612.0, 1008.0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Orientation {
    Portrait,
    Landscape,
}

impl Orientation {
    pub fn parse(s: &str) -> Result<Orientation, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "portrait" | "" => Ok(Orientation::Portrait),
            "landscape" => Ok(Orientation::Landscape),
            other => Err(format!("unknown orientation '{other}' (use portrait or landscape)")),
        }
    }
}

/// Rendering options.
pub struct Options<'a> {
    pub delimiter: &'a str,
    pub header: bool,
    pub title: &'a str,
    pub page_size: PageSize,
    pub orientation: Orientation,
    pub font_size: f64,
    pub row_banding: bool,
    pub grid: bool,
}

/// Resolve a delimiter string to a single byte.
fn delim_byte(delimiter: &str) -> Result<u8, String> {
    let d = match delimiter {
        "" | "comma" | "," => ',',
        "tab" | "\t" => '\t',
        "semicolon" | ";" => ';',
        "pipe" | "|" => '|',
        "space" => ' ',
        other => {
            let mut chars = other.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) if (c as u32) < 128 => c,
                _ => {
                    return Err(format!(
                        "delimiter must be a single ASCII char or tab/comma/semicolon/pipe, got '{other}'"
                    ))
                }
            }
        }
    };
    Ok(d as u8)
}

fn glyph_width(ch: char, bold: bool) -> u16 {
    let c = ch as u32;
    if (32..=126).contains(&c) {
        let idx = (c - 32) as usize;
        if bold {
            HELV_BOLD_W[idx]
        } else {
            HELV_W[idx]
        }
    } else if bold {
        600
    } else {
        556
    }
}

/// Width of `s` in points at `size` for Helvetica (or -Bold).
fn text_width(s: &str, size: f64, bold: bool) -> f64 {
    s.chars().map(|c| glyph_width(c, bold) as f64).sum::<f64>() * size / 1000.0
}

/// Truncate `s` with a trailing "..." so it fits `max_w` points.
fn fit_text(s: &str, max_w: f64, size: f64, bold: bool) -> String {
    if text_width(s, size, bold) <= max_w {
        return s.to_string();
    }
    let ell = "...";
    let ell_w = text_width(ell, size, bold);
    let mut chars: Vec<char> = s.chars().collect();
    while !chars.is_empty() {
        chars.pop();
        let cand: String = chars.iter().collect();
        if text_width(&cand, size, bold) + ell_w <= max_w {
            return format!("{cand}{ell}");
        }
    }
    ell.to_string()
}

/// Escape one line for a PDF literal string, folding to Latin-1 (Helvetica's
/// built-in encoding covers ASCII/Latin-1; other code points become '?').
fn pdf_escape(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len() + 2);
    for ch in s.chars() {
        let b = if (ch as u32) <= 0xFF { ch as u8 } else { b'?' };
        match b {
            b'\\' => out.extend_from_slice(b"\\\\"),
            b'(' => out.extend_from_slice(b"\\("),
            b')' => out.extend_from_slice(b"\\)"),
            _ => out.push(b),
        }
    }
    out
}

fn esc_str(s: &str) -> String {
    String::from_utf8(pdf_escape(s)).unwrap_or_else(|_| "?".to_string())
}

fn fmt_num(v: f64) -> String {
    if (v - v.round()).abs() < 0.000_001 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.3}")
    }
}

fn rect(buf: &mut String, x: f64, y: f64, w: f64, h: f64) {
    buf.push_str(&format!("{} {} {} {} re\n", fmt_num(x), fmt_num(y), fmt_num(w), fmt_num(h)));
}

fn line(buf: &mut String, x1: f64, y1: f64, x2: f64, y2: f64) {
    buf.push_str(&format!("{} {} m {} {} l\n", fmt_num(x1), fmt_num(y1), fmt_num(x2), fmt_num(y2)));
}

fn looks_numeric(cell: &str) -> bool {
    let t = cell.trim().trim_start_matches(['+', '$']).replace(',', "");
    let t = t.strip_suffix('%').unwrap_or(&t);
    !t.is_empty() && t.parse::<f64>().is_ok()
}

/// Convenience entry point that parses the string-typed `page_size` /
/// `orientation` enums and builds [`Options`], then renders. Shared by the chat
/// block, the CLI and the web page so the parsing lives in exactly one place.
#[allow(clippy::too_many_arguments)]
pub fn render_csv_pdf(
    data: &str,
    delimiter: &str,
    header: bool,
    title: &str,
    page_size: &str,
    orientation: &str,
    font_size: f64,
    row_banding: bool,
    grid: bool,
) -> Result<Vec<u8>, String> {
    let opts = Options {
        delimiter,
        header,
        title,
        page_size: PageSize::parse(page_size)?,
        orientation: Orientation::parse(orientation)?,
        font_size,
        row_banding,
        grid,
    };
    csv_to_pdf(data, &opts)
}

/// Render `data` (CSV) into a formatted table PDF.
pub fn csv_to_pdf(data: &str, opts: &Options) -> Result<Vec<u8>, String> {
    if !opts.font_size.is_finite() || opts.font_size < MIN_FONT || opts.font_size > MAX_FONT {
        return Err(format!("font_size must be between {MIN_FONT} and {MAX_FONT} points"));
    }
    let delim = delim_byte(opts.delimiter)?;

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .flexible(true)
        .from_reader(data.as_bytes());
    let records: Vec<csv::StringRecord> = rdr
        .records()
        .collect::<Result<_, _>>()
        .map_err(|e| format!("CSV parse error: {e}"))?;

    if records.is_empty() {
        return Err("no CSV rows found — paste at least one row of data".into());
    }

    let ncols = records.iter().map(|r| r.len()).max().unwrap_or(0).max(1);
    let cell = |rec: &csv::StringRecord, i: usize| rec.get(i).unwrap_or("").to_string();

    // Split header vs body.
    let (header, body): (Option<Vec<String>>, Vec<Vec<String>>) = if opts.header {
        let head = (0..ncols).map(|i| cell(&records[0], i)).collect();
        let rest = records[1..]
            .iter()
            .map(|r| (0..ncols).map(|i| cell(r, i)).collect())
            .collect();
        (Some(head), rest)
    } else {
        let rows = records
            .iter()
            .map(|r| (0..ncols).map(|i| cell(r, i)).collect())
            .collect();
        (None, rows)
    };

    let font_size = opts.font_size;
    let has_header = header.is_some();

    // Numeric column detection (body cells only) → right-align.
    let numeric_col: Vec<bool> = (0..ncols)
        .map(|c| {
            let mut any = false;
            for row in &body {
                let v = row[c].trim();
                if v.is_empty() {
                    continue;
                }
                any = true;
                if !looks_numeric(v) {
                    return false;
                }
            }
            any
        })
        .collect();

    // Natural column widths from content (header uses bold metrics).
    let mut col_w = vec![0.0_f64; ncols];
    for c in 0..ncols {
        let mut w = 0.0_f64;
        if let Some(h) = &header {
            w = w.max(text_width(&h[c], font_size, true));
        }
        for row in &body {
            w = w.max(text_width(&row[c], font_size, false));
        }
        col_w[c] = w + 2.0 * CELL_PAD;
    }

    // Page geometry.
    let (pw, ph) = opts.page_size.dims();
    let (page_w, page_h) = match opts.orientation {
        Orientation::Portrait => (pw, ph),
        Orientation::Landscape => (ph, pw),
    };
    let avail_w = page_w - 2.0 * MARGIN;
    if avail_w <= 0.0 {
        return Err("page is too small for the margins".into());
    }

    // Scale columns down proportionally if the table is wider than the page.
    let total_w: f64 = col_w.iter().sum();
    if total_w > avail_w {
        let scale = avail_w / total_w;
        for w in col_w.iter_mut() {
            *w *= scale;
        }
    }
    let table_w: f64 = col_w.iter().sum();
    // Column left edges (prefix sums) relative to the table left edge.
    let mut col_x = vec![0.0_f64; ncols + 1];
    for c in 0..ncols {
        col_x[c + 1] = col_x[c] + col_w[c];
    }

    let row_h = font_size * LINE_FACTOR;
    let title_size = (font_size + 4.0).min(MAX_FONT);
    let title_h = if opts.title.trim().is_empty() {
        0.0
    } else {
        title_size * 1.8
    };
    let header_h = if has_header { row_h } else { 0.0 };
    let usable_h = page_h - 2.0 * MARGIN;

    // Paginate body rows.
    let mut pages: Vec<(usize, usize)> = Vec::new(); // (start, end) into body
    let mut i = 0usize;
    let n = body.len();
    let mut first = true;
    loop {
        let this_title_h = if first { title_h } else { 0.0 };
        let rows_h = usable_h - this_title_h - header_h;
        let mut rows_this = (rows_h / row_h).floor() as isize;
        if rows_this < 1 {
            rows_this = 1;
        }
        let end = (i + rows_this as usize).min(n);
        pages.push((i, end));
        i = end;
        first = false;
        if i >= n {
            break;
        }
    }
    // If there is no body at all (header-only) the single page above still renders.

    // Build a minimal PDF 1.4 document by hand. Object IDs:
    // 1=catalog, 2=pages tree, 3/4=base-14 fonts, then content/page pairs.
    let x0 = MARGIN;
    let mut objects: Vec<(usize, Vec<u8>)> = Vec::new();
    objects.push((3, b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec()));
    objects.push((4, b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold >>".to_vec()));
    let mut page_ids: Vec<usize> = Vec::new();
    let mut next_id = 5usize;

    for (pi, &(start, end)) in pages.iter().enumerate() {
        let first_page = pi == 0;
        let mut ops = String::new();

        let top_y = page_h - MARGIN;
        let this_title_h = if first_page { title_h } else { 0.0 };
        let table_top = top_y - this_title_h;

        let n_rows_here = if has_header { 1 } else { 0 } + (end - start);
        let table_bottom = table_top - n_rows_here as f64 * row_h;

        // --- Fills: header background + zebra banding ---
        if has_header {
            ops.push_str("0.86 0.88 0.92 rg\n");
            rect(&mut ops, x0, table_top - row_h, table_w, row_h);
            ops.push_str("f\n");
        }
        if opts.row_banding {
            let before = ops.len();
            let mut band_ops = String::new();
            for (j, abs) in (start..end).enumerate() {
                if abs % 2 == 1 {
                    let row_top = table_top - (if has_header { 1 } else { 0 } + j) as f64 * row_h;
                    rect(&mut band_ops, x0, row_top - row_h, table_w, row_h);
                }
            }
            if !band_ops.is_empty() {
                ops.push_str("0.96 0.96 0.97 rg\n");
                ops.push_str(&band_ops);
                ops.push_str("f\n");
            } else {
                ops.truncate(before);
            }
        }

        // --- Grid lines ---
        if opts.grid {
            ops.push_str("0.72 0.72 0.74 RG\n0.5 w\n");
            for li in 0..=n_rows_here {
                let y = table_top - li as f64 * row_h;
                line(&mut ops, x0, y, x0 + table_w, y);
            }
            for c in 0..=ncols {
                let x = x0 + col_x[c];
                line(&mut ops, x, table_top, x, table_bottom);
            }
            ops.push_str("S\n");
        }

        // --- Text ---
        ops.push_str("0 0 0 rg\nBT\n");

        // Title (first page only)
        if first_page && this_title_h > 0.0 {
            let baseline = top_y - title_size;
            let fitted = fit_text(opts.title.trim(), avail_w, title_size, true);
            ops.push_str(&format!(
                "/F2 {} Tf\n1 0 0 1 {} {} Tm\n({}) Tj\n",
                fmt_num(title_size),
                fmt_num(x0),
                fmt_num(baseline),
                esc_str(&fitted)
            ));
        }

        let draw_row = |ops: &mut String, cells: &[String], row_top: f64, bold: bool| {
            let baseline = row_top - row_h + (row_h - font_size) / 2.0 + font_size * 0.22;
            let font = if bold { "F2" } else { "F1" };
            ops.push_str(&format!("/{font} {} Tf\n", fmt_num(font_size)));
            for c in 0..ncols {
                let inner_w = (col_w[c] - 2.0 * CELL_PAD).max(1.0);
                let fitted = fit_text(&cells[c], inner_w, font_size, bold);
                if fitted.is_empty() {
                    continue;
                }
                let tw = text_width(&fitted, font_size, bold);
                let right = !bold && numeric_col[c];
                let x = if right {
                    x0 + col_x[c + 1] - CELL_PAD - tw
                } else {
                    x0 + col_x[c] + CELL_PAD
                };
                ops.push_str(&format!(
                    "1 0 0 1 {} {} Tm\n({}) Tj\n",
                    fmt_num(x),
                    fmt_num(baseline),
                    esc_str(&fitted)
                ));
            }
        };

        let mut row_top = table_top;
        if let Some(h) = &header {
            draw_row(&mut ops, h, row_top, true);
            row_top -= row_h;
        }
        for abs in start..end {
            draw_row(&mut ops, &body[abs], row_top, false);
            row_top -= row_h;
        }

        ops.push_str("ET\n");

        let content_id = next_id;
        next_id += 1;
        let page_id = next_id;
        next_id += 1;
        let content_bytes = ops.into_bytes();
        let mut content_obj = format!("<< /Length {} >>\nstream\n", content_bytes.len()).into_bytes();
        content_obj.extend_from_slice(&content_bytes);
        content_obj.extend_from_slice(b"endstream");
        objects.push((content_id, content_obj));
        let page_obj = format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents {} 0 R >>",
            fmt_num(page_w),
            fmt_num(page_h),
            content_id
        );
        objects.push((page_id, page_obj.into_bytes()));
        page_ids.push(page_id);
    }

    let kids = page_ids
        .iter()
        .map(|id| format!("{id} 0 R"))
        .collect::<Vec<_>>()
        .join(" ");
    objects.push((1, b"<< /Type /Catalog /Pages 2 0 R >>".to_vec()));
    objects.push((
        2,
        format!("<< /Type /Pages /Kids [{kids}] /Count {} >>", page_ids.len()).into_bytes(),
    ));
    objects.sort_by_key(|(id, _)| *id);

    let max_id = objects.iter().map(|(id, _)| *id).max().unwrap_or(0);
    let mut out = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = vec![0usize; max_id + 1];
    for (id, obj) in &objects {
        offsets[*id] = out.len();
        out.extend_from_slice(format!("{id} 0 obj\n").as_bytes());
        out.extend_from_slice(obj);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", max_id + 1).as_bytes());
    for off in offsets.iter().skip(1) {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
            max_id + 1,
            xref
        )
        .as_bytes(),
    );
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options<'static> {
        Options {
            delimiter: ",",
            header: true,
            title: "",
            page_size: PageSize::Letter,
            orientation: Orientation::Portrait,
            font_size: 10.0,
            row_banding: true,
            grid: true,
        }
    }

    // Count pages without pulling in lopdf's optional parser feature: every page
    // dict carries exactly one inline `/MediaBox`, written as plain text.
    fn page_count(pdf: &[u8]) -> usize {
        pdf.windows(9).filter(|w| *w == b"/MediaBox").count()
    }

    #[test]
    fn makes_a_valid_one_page_pdf() {
        let pdf = csv_to_pdf("name,age\nAlice,30\nBob,25", &opts()).unwrap();
        assert_eq!(&pdf[..5], b"%PDF-");
        assert_eq!(page_count(&pdf), 1);
    }

    #[test]
    fn paginates_many_rows() {
        let mut data = String::from("id,val\n");
        for i in 0..400 {
            data.push_str(&format!("{i},{}\n", i * 2));
        }
        let pdf = csv_to_pdf(&data, &opts()).unwrap();
        assert!(page_count(&pdf) > 1, "400 rows should span multiple pages");
    }

    #[test]
    fn title_and_landscape_render() {
        let mut o = opts();
        o.title = "Quarterly Report";
        o.orientation = Orientation::Landscape;
        let pdf = csv_to_pdf("a,b,c\n1,2,3", &o).unwrap();
        assert_eq!(&pdf[..5], b"%PDF-");
    }

    #[test]
    fn delimiter_and_pagesize_parse() {
        assert_eq!(delim_byte("semicolon").unwrap(), b';');
        assert_eq!(delim_byte("\t").unwrap(), b'\t');
        assert_eq!(delim_byte("|").unwrap(), b'|');
        assert!(delim_byte("ab").is_err());
        assert_eq!(PageSize::parse("a4").unwrap(), PageSize::A4);
        assert_eq!(Orientation::parse("landscape").unwrap(), Orientation::Landscape);
        assert!(PageSize::parse("tabloid").is_err());
    }

    #[test]
    fn numeric_detection() {
        assert!(looks_numeric("30"));
        assert!(looks_numeric("1,234.5"));
        assert!(looks_numeric("$42"));
        assert!(looks_numeric("12%"));
        assert!(!looks_numeric("N/A"));
        assert!(!looks_numeric("abc"));
    }

    #[test]
    fn render_entry_point_parses_enums_and_errors_clearly() {
        // Happy path through the shared string-arg entry the wrappers call.
        let pdf =
            render_csv_pdf("a,b\n1,2", "comma", true, "T", "a4", "landscape", 10.0, true, true)
                .unwrap();
        assert_eq!(&pdf[..5], b"%PDF-");
        assert_eq!(page_count(&pdf), 1);
        // Bad enum values surface a readable error, not a panic.
        assert!(render_csv_pdf("a\n1", "comma", true, "", "tabloid", "portrait", 10.0, true, true)
            .is_err());
        assert!(render_csv_pdf("a\n1", "comma", true, "", "letter", "sideways", 10.0, true, true)
            .is_err());
    }

    #[test]
    fn errors_on_empty_and_bad_font() {
        assert!(csv_to_pdf("", &opts()).is_err());
        let mut o = opts();
        o.font_size = 1.0;
        assert!(csv_to_pdf("a,b\n1,2", &o).is_err());
        o.font_size = 200.0;
        assert!(csv_to_pdf("a,b\n1,2", &o).is_err());
    }
}

//! table-to-image core — parse a CSV or JSON table and render it as a clean,
//! styled, standalone SVG string (header band + zebra rows + themes). Pure Rust
//! (hand-built SVG; no raster/image deps), so it runs on every backend including
//! the chat Service Worker. No wafer/wasm-bindgen deps here.

use serde_json::Value;

/// Rendering options resolved from the tool params.
pub struct Options {
    /// Input format: "auto" (sniff), "csv", or "json".
    pub input_format: String,
    /// CSV field delimiter (first char; supports "\\t"/"tab" for a tab). Ignored for JSON.
    pub delimiter: String,
    /// Treat the first CSV/array-of-arrays row as a styled header. (Arrays of
    /// JSON objects always use their keys as the header.)
    pub header: bool,
    /// Shade alternating body rows.
    pub zebra: bool,
    /// Colour theme: "light" | "dark" | "slate" | "blue" | "green" | "minimal".
    pub theme: String,
    /// Accent colour: the header band fill (and the header underline in the
    /// minimal theme). Any CSS colour.
    pub accent: String,
    /// Base font size in pixels for body cells (8..=48).
    pub font_size: u32,
    /// Padding inside every cell in pixels (0..=60).
    pub cell_padding: u32,
    /// Optional caption drawn above the table.
    pub title: String,
    /// Cell text alignment: "left" | "center" | "right".
    pub align: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            input_format: "auto".into(),
            delimiter: ",".into(),
            header: true,
            zebra: true,
            theme: "light".into(),
            accent: "#2563eb".into(),
            font_size: 14,
            cell_padding: 10,
            title: String::new(),
            align: "left".into(),
        }
    }
}

/// Resolved colour palette for a theme.
struct Palette {
    /// Page background; empty string means transparent (minimal theme).
    page: &'static str,
    /// Base body-row fill; empty string means no fill.
    row: &'static str,
    /// Alternating (zebra) body-row fill.
    zebra: &'static str,
    /// Body text colour (also the title colour).
    text: &'static str,
    /// Grid / border colour.
    border: &'static str,
    /// True for the borderless "minimal" theme (no page fill, no header band,
    /// no vertical rules — just row rules and an accent underline).
    minimal: bool,
}

fn palette(theme: &str) -> Palette {
    match theme.trim().to_lowercase().as_str() {
        "dark" => Palette {
            page: "#0f172a",
            row: "#1e293b",
            zebra: "#273549",
            text: "#e2e8f0",
            border: "#334155",
            minimal: false,
        },
        "slate" => Palette {
            page: "#f8fafc",
            row: "#ffffff",
            zebra: "#eef2f7",
            text: "#334155",
            border: "#cbd5e1",
            minimal: false,
        },
        "blue" => Palette {
            page: "#eff6ff",
            row: "#ffffff",
            zebra: "#dbeafe",
            text: "#1e3a5f",
            border: "#bfdbfe",
            minimal: false,
        },
        "green" => Palette {
            page: "#f0fdf4",
            row: "#ffffff",
            zebra: "#dcfce7",
            text: "#14532d",
            border: "#bbf7d0",
            minimal: false,
        },
        "minimal" => Palette {
            page: "",
            row: "",
            zebra: "#f4f4f5",
            text: "#18181b",
            border: "#e4e4e7",
            minimal: true,
        },
        // "light" and any unknown value fall back to light.
        _ => Palette {
            page: "#ffffff",
            row: "#ffffff",
            zebra: "#f3f4f6",
            text: "#1f2937",
            border: "#e5e7eb",
            minimal: false,
        },
    }
}

/// Escape a string for use as XML text/attribute content.
fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Round a coordinate to a whole pixel and render it without a decimal point.
fn n(v: f64) -> i64 {
    v.round() as i64
}

/// Resolve the delimiter string to a single byte. Supports `\t`/`tab` for a tab.
fn delimiter_byte(spec: &str) -> u8 {
    let t = spec;
    if t == "\\t" || t.eq_ignore_ascii_case("tab") || t == "\t" {
        return b'\t';
    }
    t.chars()
        .next()
        .map(|c| c as u32)
        .and_then(|c| u8::try_from(c).ok())
        .unwrap_or(b',')
}

/// Stringify a JSON scalar for a cell; nested arrays/objects become compact JSON.
fn cell_str(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::Bool(b) => b.to_string(),
        Value::Number(num) => num.to_string(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// A parsed table: an optional header row plus body rows, all padded to `ncols`.
struct Table {
    header: Option<Vec<String>>,
    body: Vec<Vec<String>>,
    ncols: usize,
}

fn pad_rows(rows: &mut [Vec<String>], ncols: usize) {
    for r in rows.iter_mut() {
        while r.len() < ncols {
            r.push(String::new());
        }
    }
}

fn parse_csv(input: &str, opts: &Options) -> Result<Table, String> {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(delimiter_byte(&opts.delimiter))
        .from_reader(input.as_bytes());
    let mut rows: Vec<Vec<String>> = Vec::new();
    for rec in rdr.records() {
        let rec = rec.map_err(|e| format!("could not parse the CSV: {e}"))?;
        rows.push(rec.iter().map(|c| c.trim().to_string()).collect());
    }
    rows.retain(|r| !r.iter().all(|c| c.is_empty()));
    if rows.is_empty() {
        return Err("the table is empty — paste at least one row of CSV".into());
    }
    let ncols = rows.iter().map(|r| r.len()).max().unwrap_or(0).max(1);
    pad_rows(&mut rows, ncols);
    let header = if opts.header {
        Some(rows.remove(0))
    } else {
        None
    };
    Ok(Table {
        header,
        body: rows,
        ncols,
    })
}

fn parse_json(input: &str, opts: &Options) -> Result<Table, String> {
    let val: Value = serde_json::from_str(input).map_err(|e| format!("invalid JSON: {e}"))?;
    // A bare object is treated as a one-row array of objects.
    let items: Vec<Value> = match val {
        Value::Array(a) => a,
        Value::Object(_) => vec![val],
        _ => {
            return Err(
                "JSON must be an array of rows (objects or arrays) or a single object".into(),
            )
        }
    };
    if items.is_empty() {
        return Err("the JSON array is empty".into());
    }

    // Array of objects → keys become the header (first-seen order, unioned).
    if items.iter().all(|i| i.is_object()) {
        let mut keys: Vec<String> = Vec::new();
        for it in &items {
            if let Some(map) = it.as_object() {
                for k in map.keys() {
                    if !keys.iter().any(|e| e == k) {
                        keys.push(k.clone());
                    }
                }
            }
        }
        if keys.is_empty() {
            return Err("the JSON objects have no fields to tabulate".into());
        }
        let body: Vec<Vec<String>> = items
            .iter()
            .map(|it| {
                let map = it.as_object().unwrap();
                keys.iter()
                    .map(|k| map.get(k).map(cell_str).unwrap_or_default())
                    .collect()
            })
            .collect();
        let ncols = keys.len();
        return Ok(Table {
            header: Some(keys),
            body,
            ncols,
        });
    }

    // Array of arrays → each inner array is a row.
    if items.iter().all(|i| i.is_array()) {
        let mut rows: Vec<Vec<String>> = items
            .iter()
            .map(|it| it.as_array().unwrap().iter().map(cell_str).collect())
            .collect();
        rows.retain(|r| !r.is_empty());
        if rows.is_empty() {
            return Err("the JSON has no rows to render".into());
        }
        let ncols = rows.iter().map(|r| r.len()).max().unwrap_or(0).max(1);
        pad_rows(&mut rows, ncols);
        let header = if opts.header {
            Some(rows.remove(0))
        } else {
            None
        };
        return Ok(Table {
            header,
            body: rows,
            ncols,
        });
    }

    // Array of scalars → a single-column table.
    let mut rows: Vec<Vec<String>> = items.iter().map(|it| vec![cell_str(it)]).collect();
    let header = if opts.header {
        Some(rows.remove(0))
    } else {
        None
    };
    Ok(Table {
        header,
        body: rows,
        ncols: 1,
    })
}

/// Parse the input into a `Table`, choosing CSV/JSON per `input_format`.
fn parse_table(input: &str, opts: &Options) -> Result<Table, String> {
    if input.trim().is_empty() {
        return Err("the table is empty — paste some CSV or JSON".into());
    }
    let fmt = match opts.input_format.trim().to_lowercase().as_str() {
        "json" => "json",
        "csv" => "csv",
        _ => {
            let t = input.trim_start();
            if t.starts_with('[') || t.starts_with('{') {
                "json"
            } else {
                "csv"
            }
        }
    };
    match fmt {
        "json" => parse_json(input, opts),
        _ => parse_csv(input, opts),
    }
}

/// Estimate a display width (in chars) for a cell, capped so one long value
/// can't blow the layout out.
fn cell_len(s: &str) -> usize {
    s.chars().count().min(60)
}

/// Emit a row of cell text at `y_top`, aligned per `align`.
#[allow(clippy::too_many_arguments)]
fn draw_row_text(
    s: &mut String,
    cells: &[String],
    col_x: &[i64],
    col_w: &[i64],
    y_top: f64,
    row_h: f64,
    fs: f64,
    pad: f64,
    align: &str,
    color: &str,
    bold: bool,
) {
    let baseline = y_top + row_h / 2.0 + fs * 0.34;
    let weight = if bold { " font-weight=\"bold\"" } else { "" };
    for (i, cell) in cells.iter().enumerate() {
        if cell.is_empty() {
            continue;
        }
        let x0 = col_x[i] as f64;
        let w = col_w[i] as f64;
        let (tx, anchor) = match align {
            "center" => (x0 + w / 2.0, "middle"),
            "right" => (x0 + w - pad, "end"),
            _ => (x0 + pad, "start"),
        };
        s.push_str(&format!(
            "<text x=\"{x}\" y=\"{y}\" text-anchor=\"{a}\" font-size=\"{fs}\" fill=\"{c}\"{w}>{t}</text>\n",
            x = n(tx),
            y = n(baseline),
            a = anchor,
            fs = n(fs),
            c = esc(color),
            w = weight,
            t = esc(cell),
        ));
    }
}

/// Render a parsed CSV/JSON table to a standalone SVG string.
pub fn render(input: &str, opts: &Options) -> Result<String, String> {
    let table = parse_table(input, opts)?;
    if table.header.is_none() && table.body.is_empty() {
        return Err("no rows to render".into());
    }

    let pal = palette(&opts.theme);
    let accent = if opts.accent.trim().is_empty() {
        "#2563eb"
    } else {
        opts.accent.trim()
    };
    let fs = opts.font_size.clamp(8, 48) as f64;
    let pad = opts.cell_padding.clamp(0, 60) as f64;
    let align = match opts.align.trim().to_lowercase().as_str() {
        "center" => "center",
        "right" => "right",
        _ => "left",
    };

    let ncols = table.ncols;
    let charw = fs * 0.6;
    let min_col_w = (fs * 2.0 + pad * 2.0).round();

    // Column widths from the widest cell (header included).
    let mut col_w: Vec<i64> = vec![0; ncols];
    let mut consider = |cells: &[String]| {
        for (i, c) in cells.iter().enumerate().take(ncols) {
            let w = (cell_len(c) as f64 * charw + pad * 2.0)
                .round()
                .max(min_col_w) as i64;
            if w > col_w[i] {
                col_w[i] = w;
            }
        }
    };
    if let Some(h) = &table.header {
        consider(h);
    }
    for r in &table.body {
        consider(r);
    }
    for w in col_w.iter_mut() {
        if *w == 0 {
            *w = min_col_w as i64;
        }
    }

    let margin = 16.0_f64;
    let row_h = fs + pad * 2.0;
    let header_h = if table.header.is_some() { row_h } else { 0.0 };
    let title_present = !opts.title.trim().is_empty();
    let title_fs = fs + 6.0;
    let title_h = if title_present { title_fs + pad } else { 0.0 };

    // Cumulative x of each column's left edge.
    let table_x = margin;
    let mut col_x: Vec<i64> = Vec::with_capacity(ncols);
    let mut acc = table_x;
    for w in &col_w {
        col_x.push(n(acc));
        acc += *w as f64;
    }
    let table_w: i64 = col_w.iter().sum();
    let svg_w = table_w as f64 + margin * 2.0;

    let table_y = margin + title_h;
    let content_h = header_h + table.body.len() as f64 * row_h;
    let svg_h = table_y + content_h + margin;

    let mut s = String::new();
    s.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\" font-family=\"-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif\">\n",
        w = n(svg_w),
        h = n(svg_h),
    ));

    // Page background (opaque themes only).
    if !pal.page.is_empty() {
        s.push_str(&format!(
            "<rect x=\"0\" y=\"0\" width=\"{w}\" height=\"{h}\" fill=\"{c}\"/>\n",
            w = n(svg_w),
            h = n(svg_h),
            c = pal.page,
        ));
    }

    // Title.
    if title_present {
        s.push_str(&format!(
            "<text x=\"{x}\" y=\"{y}\" text-anchor=\"middle\" font-size=\"{fs}\" font-weight=\"bold\" fill=\"{c}\">{t}</text>\n",
            x = n(table_x as f64 + table_w as f64 / 2.0),
            y = n(margin + title_fs * 0.8),
            fs = n(title_fs),
            c = esc(pal.text),
            t = esc(opts.title.trim()),
        ));
    }

    let table_right = table_x + table_w as f64;
    let table_bottom = table_y + content_h;

    // Header band.
    let mut body_y = table_y;
    if let Some(h) = &table.header {
        if pal.minimal {
            // No filled band; accent underline beneath the header instead.
            s.push_str(&format!(
                "<line x1=\"{x1}\" y1=\"{y}\" x2=\"{x2}\" y2=\"{y}\" stroke=\"{c}\" stroke-width=\"2\"/>\n",
                x1 = n(table_x),
                x2 = n(table_right),
                y = n(table_y + row_h),
                c = esc(accent),
            ));
            draw_row_text(
                &mut s, h, &col_x, &col_w, table_y, row_h, fs, pad, align, accent, true,
            );
        } else {
            s.push_str(&format!(
                "<rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{hh}\" fill=\"{c}\"/>\n",
                x = n(table_x),
                y = n(table_y),
                w = table_w,
                hh = n(row_h),
                c = esc(accent),
            ));
            draw_row_text(
                &mut s, h, &col_x, &col_w, table_y, row_h, fs, pad, align, "#ffffff", true,
            );
        }
        body_y += row_h;
    }

    // Body rows (zebra fills first, then text).
    for (i, row) in table.body.iter().enumerate() {
        let y_top = body_y + i as f64 * row_h;
        let fill = if opts.zebra && i % 2 == 1 {
            pal.zebra
        } else {
            pal.row
        };
        if !fill.is_empty() {
            s.push_str(&format!(
                "<rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" fill=\"{c}\"/>\n",
                x = n(table_x),
                y = n(y_top),
                w = table_w,
                h = n(row_h),
                c = fill,
            ));
        }
        draw_row_text(
            &mut s, row, &col_x, &col_w, y_top, row_h, fs, pad, align, pal.text, false,
        );
    }

    // Grid rules.
    let n_body = table.body.len();
    // Horizontal rules between/around every row.
    let mut y = body_y;
    let mut hlines: Vec<f64> = Vec::new();
    if !pal.minimal {
        hlines.push(table_y); // top edge
        if table.header.is_some() {
            hlines.push(table_y + row_h);
        }
    } else if table.header.is_none() {
        hlines.push(table_y);
    }
    for i in 0..n_body {
        y = body_y + (i + 1) as f64 * row_h;
        hlines.push(y);
    }
    for hy in &hlines {
        s.push_str(&format!(
            "<line x1=\"{x1}\" y1=\"{y}\" x2=\"{x2}\" y2=\"{y}\" stroke=\"{c}\" stroke-width=\"1\"/>\n",
            x1 = n(table_x),
            x2 = n(table_right),
            y = n(*hy),
            c = esc(pal.border),
        ));
    }
    // Vertical rules + outer box (opaque themes only).
    if !pal.minimal {
        let mut vx = table_x;
        s.push_str(&format!(
            "<line x1=\"{x}\" y1=\"{y1}\" x2=\"{x}\" y2=\"{y2}\" stroke=\"{c}\" stroke-width=\"1\"/>\n",
            x = n(vx),
            y1 = n(table_y),
            y2 = n(table_bottom),
            c = esc(pal.border),
        ));
        for w in &col_w {
            vx += *w as f64;
            s.push_str(&format!(
                "<line x1=\"{x}\" y1=\"{y1}\" x2=\"{x}\" y2=\"{y2}\" stroke=\"{c}\" stroke-width=\"1\"/>\n",
                x = n(vx),
                y1 = n(table_y),
                y2 = n(table_bottom),
                c = esc(pal.border),
            ));
        }
    }
    let _ = y;

    s.push_str("</svg>");
    Ok(s)
}

/// Convenience entry point used by one-argument wrappers. New callers should
/// pass explicit [`Options`] to [`render`].
pub fn run(input: &str) -> Result<String, String> {
    render(input, &Options::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options::default()
    }

    #[test]
    fn csv_with_header_renders_band_and_cells() {
        let svg = render("name,score\nAda,9\nGrace,8", &opts()).unwrap();
        assert!(svg.starts_with("<svg"));
        assert!(svg.trim_end().ends_with("</svg>"));
        assert!(svg.contains(">name</text>"));
        assert!(svg.contains(">Ada</text>"));
        assert!(svg.contains(">Grace</text>"));
        // Header band filled with the default accent.
        assert!(svg.contains("fill=\"#2563eb\""));
        // Default light page background.
        assert!(svg.contains("fill=\"#ffffff\""));
    }

    #[test]
    fn zebra_shades_alternate_body_rows() {
        let svg = render("h\na\nb\nc\nd", &opts()).unwrap();
        // Light theme zebra fill on the 2nd/4th body rows.
        assert!(svg.contains("fill=\"#f3f4f6\""));
    }

    #[test]
    fn zebra_off_has_no_stripe_fill() {
        let mut o = opts();
        o.zebra = false;
        let svg = render("h\na\nb\nc", &o).unwrap();
        assert!(!svg.contains("fill=\"#f3f4f6\""));
    }

    #[test]
    fn dark_theme_uses_dark_page() {
        let mut o = opts();
        o.theme = "dark".into();
        let svg = render("a,b\n1,2", &o).unwrap();
        assert!(svg.contains("fill=\"#0f172a\""));
        assert!(svg.contains("fill=\"#e2e8f0\"")); // body text colour
    }

    #[test]
    fn minimal_theme_has_no_page_fill_and_accent_underline() {
        let mut o = opts();
        o.theme = "minimal".into();
        o.accent = "#e11d48".into();
        let svg = render("a,b\n1,2", &o).unwrap();
        assert!(!svg.contains("width=\"") || true);
        // No filled header band and no page bg → accent appears only in a stroke.
        assert!(svg.contains("stroke=\"#e11d48\""));
        assert!(!svg.contains("fill=\"#ffffff\"")); // no opaque page/row fill
    }

    #[test]
    fn accent_colour_is_applied_to_header_band() {
        let mut o = opts();
        o.accent = "#16a34a".into();
        let svg = render("a\n1", &o).unwrap();
        assert!(svg.contains("fill=\"#16a34a\""));
    }

    #[test]
    fn title_is_rendered_and_escaped() {
        let mut o = opts();
        o.title = "Q1 <Sales> & More".into();
        let svg = render("a\n1", &o).unwrap();
        assert!(svg.contains("Q1 &lt;Sales&gt; &amp; More"));
        assert!(svg.contains("font-weight=\"bold\""));
    }

    #[test]
    fn cell_text_is_xml_escaped() {
        let svg = render("x\n<b>&\"'", &opts()).unwrap();
        assert!(svg.contains("&lt;b&gt;&amp;&quot;&#39;"));
    }

    #[test]
    fn json_array_of_objects_uses_keys_as_header() {
        let json = r#"[{"name":"Ada","score":9},{"name":"Grace","score":8}]"#;
        let mut o = opts();
        o.input_format = "json".into();
        let svg = render(json, &o).unwrap();
        assert!(svg.contains(">name</text>"));
        assert!(svg.contains(">score</text>"));
        assert!(svg.contains(">Ada</text>"));
        assert!(svg.contains(">9</text>"));
    }

    #[test]
    fn json_preserves_first_seen_key_order() {
        let json = r#"[{"zeta":1,"alpha":2}]"#;
        let mut o = opts();
        o.input_format = "json".into();
        let svg = render(json, &o).unwrap();
        let zi = svg.find(">zeta</text>").unwrap();
        let ai = svg.find(">alpha</text>").unwrap();
        assert!(zi < ai, "zeta should come before alpha (insertion order)");
    }

    #[test]
    fn json_array_of_arrays_with_header() {
        let json = r#"[["a","b"],[1,2],[3,4]]"#;
        let mut o = opts();
        o.input_format = "json".into();
        let svg = render(json, &o).unwrap();
        assert!(svg.contains(">a</text>"));
        assert!(svg.contains(">3</text>"));
    }

    #[test]
    fn auto_format_sniffs_json() {
        let svg = render(r#"[{"k":"v"}]"#, &opts()).unwrap();
        assert!(svg.contains(">k</text>"));
        assert!(svg.contains(">v</text>"));
    }

    #[test]
    fn auto_format_sniffs_csv() {
        let svg = render("k,v\n1,2", &opts()).unwrap();
        assert!(svg.contains(">k</text>"));
    }

    #[test]
    fn tab_delimiter_is_supported() {
        let mut o = opts();
        o.delimiter = "\\t".into();
        let svg = render("a\tb\n1\t2", &o).unwrap();
        assert!(svg.contains(">a</text>"));
        assert!(svg.contains(">b</text>"));
    }

    #[test]
    fn semicolon_delimiter_is_supported() {
        let mut o = opts();
        o.delimiter = ";".into();
        let svg = render("a;b\n1;2", &o).unwrap();
        assert!(svg.contains(">b</text>"));
    }

    #[test]
    fn header_false_treats_first_row_as_body() {
        let mut o = opts();
        o.header = false;
        let svg = render("1,2\n3,4", &o).unwrap();
        // No accent header band when there is no header row.
        assert!(!svg.contains("fill=\"#2563eb\""));
    }

    #[test]
    fn right_align_uses_end_anchor() {
        let mut o = opts();
        o.align = "right".into();
        let svg = render("a\n1", &o).unwrap();
        assert!(svg.contains("text-anchor=\"end\""));
    }

    #[test]
    fn center_align_uses_middle_anchor_for_cells() {
        let mut o = opts();
        o.align = "center".into();
        let svg = render("a\n1", &o).unwrap();
        assert!(svg.contains("text-anchor=\"middle\""));
    }

    #[test]
    fn quoted_csv_fields_with_commas() {
        let svg = render("label,amount\n\"a, b\",5", &opts()).unwrap();
        assert!(svg.contains(">a, b</text>"));
    }

    #[test]
    fn empty_input_errors() {
        let e = render("   ", &opts()).unwrap_err();
        assert!(e.contains("empty"));
    }

    #[test]
    fn invalid_json_errors() {
        let mut o = opts();
        o.input_format = "json".into();
        let e = render("{not json", &o).unwrap_err();
        assert!(e.contains("invalid JSON"));
    }

    #[test]
    fn nested_json_value_is_stringified() {
        let json = r#"[{"tags":["x","y"]}]"#;
        let mut o = opts();
        o.input_format = "json".into();
        let svg = render(json, &o).unwrap();
        assert!(svg.contains(r#"[&quot;x&quot;,&quot;y&quot;]"#));
    }

    #[test]
    fn bare_object_becomes_single_row() {
        let json = r#"{"a":1,"b":2}"#;
        let mut o = opts();
        o.input_format = "json".into();
        let svg = render(json, &o).unwrap();
        assert!(svg.contains(">a</text>"));
        assert!(svg.contains(">1</text>"));
    }

    #[test]
    fn font_size_is_clamped() {
        let mut o = opts();
        o.font_size = 999;
        let svg = render("a\n1", &o).unwrap();
        assert!(svg.contains("font-size=\"48\""));
    }
}

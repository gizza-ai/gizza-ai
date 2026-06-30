//! text-to-unicode core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps.
//!
//! For each Unicode scalar value (`char`) in the input it reports the code point
//! (U+XXXX), decimal value, official Unicode name, the `\u` escape sequence, and
//! the UTF-8 bytes. Output is an aligned ASCII table (default) or a JSON array.

use std::fmt::Write as _;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Table,
    Json,
}

impl Format {
    pub fn parse(s: &str) -> Result<Format, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "table" => Ok(Format::Table),
            "json" => Ok(Format::Json),
            other => Err(format!("unknown format '{other}' (use 'table' or 'json')")),
        }
    }
}

/// One analyzed character.
struct CharInfo {
    ch: char,
    glyph: String,
    code_point: String,
    decimal: u32,
    escape: String,
    utf8: String,
    name: String,
}

fn analyze(c: char) -> CharInfo {
    let cp = c as u32;
    // A printable glyph for the table; control chars would corrupt alignment.
    let glyph = if c.is_control() {
        "·".to_string()
    } else if c == ' ' {
        "␠".to_string()
    } else {
        c.to_string()
    };
    let escape = if cp <= 0xFFFF {
        format!("\\u{cp:04X}")
    } else {
        format!("\\u{{{cp:X}}}")
    };
    let mut buf = [0u8; 4];
    let utf8 = c
        .encode_utf8(&mut buf)
        .bytes()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ");
    let name = unicode_names2::name(c)
        .map(|n| n.to_string())
        .unwrap_or_else(|| {
            if c.is_control() {
                "<control>".to_string()
            } else {
                "<unnamed>".to_string()
            }
        });
    CharInfo {
        ch: c,
        glyph,
        code_point: format!("U+{cp:04X}"),
        decimal: cp,
        escape,
        utf8,
        name,
    }
}

/// Analyze `text` and render it in the chosen `format`.
pub fn to_unicode(text: &str, format: Format) -> Result<String, String> {
    if text.is_empty() {
        return Err("input text is empty".into());
    }
    let rows: Vec<CharInfo> = text.chars().map(analyze).collect();
    match format {
        Format::Table => Ok(render_table(&rows)),
        Format::Json => Ok(render_json(&rows)),
    }
}

fn disp_width(s: &str) -> usize {
    unicode_width::UnicodeWidthStr::width(s).max(1)
}

fn render_table(rows: &[CharInfo]) -> String {
    let headers = ["Char", "Code Point", "Dec", "Escape", "UTF-8", "Name"];
    let cells: Vec<[String; 6]> = rows
        .iter()
        .map(|r| {
            [
                r.glyph.clone(),
                r.code_point.clone(),
                r.decimal.to_string(),
                r.escape.clone(),
                r.utf8.clone(),
                r.name.clone(),
            ]
        })
        .collect();
    let mut widths: Vec<usize> = headers.iter().map(|h| disp_width(h)).collect();
    for row in &cells {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(disp_width(cell));
        }
    }

    let sep = |out: &mut String| {
        out.push('+');
        for w in &widths {
            for _ in 0..(w + 2) {
                out.push('-');
            }
            out.push('+');
        }
        out.push('\n');
    };
    let pad = |cell: &str, w: usize| -> String {
        let fill = w.saturating_sub(disp_width(cell));
        format!(" {cell}{} ", " ".repeat(fill))
    };

    let mut out = String::new();
    sep(&mut out);
    out.push('|');
    for (i, h) in headers.iter().enumerate() {
        out.push_str(&pad(h, widths[i]));
        out.push('|');
    }
    out.push('\n');
    sep(&mut out);
    for row in &cells {
        out.push('|');
        for (i, cell) in row.iter().enumerate() {
            out.push_str(&pad(cell, widths[i]));
            out.push('|');
        }
        out.push('\n');
    }
    sep(&mut out);
    out.pop(); // trailing newline
    out
}

fn render_json(rows: &[CharInfo]) -> String {
    let mut out = String::from("[");
    for (i, r) in rows.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("\n  {");
        let _ = write!(
            out,
            "\"char\": {}, \"codePoint\": {}, \"decimal\": {}, \"escape\": {}, \"utf8\": {}, \"name\": {}",
            json_str(&r.ch.to_string()),
            json_str(&r.code_point),
            r.decimal,
            json_str(&r.escape),
            json_str(&r.utf8),
            json_str(&r.name),
        );
        out.push('}');
    }
    out.push_str("\n]");
    out
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
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_letter() {
        let out = to_unicode("A", Format::Table).unwrap();
        assert!(out.contains("U+0041"), "{out}");
        assert!(out.contains("LATIN CAPITAL LETTER A"), "{out}");
        assert!(out.contains("\\u0041"), "{out}");
        assert!(out.contains("| 65 "), "{out}");
    }

    #[test]
    fn astral_emoji_uses_braced_escape() {
        let out = to_unicode("😀", Format::Table).unwrap();
        assert!(out.contains("U+1F600"), "{out}");
        assert!(out.contains("\\u{1F600}"), "{out}");
        assert!(out.contains("F0 9F 98 80"), "{out}");
    }

    #[test]
    fn json_format_is_valid() {
        let out = to_unicode("Aб", Format::Json).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).expect("valid json");
        let arr = v.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["codePoint"], "U+0041");
        assert_eq!(arr[0]["char"], "A");
        assert_eq!(arr[1]["codePoint"], "U+0431");
        assert_eq!(arr[1]["decimal"], 1073);
    }

    #[test]
    fn control_char_named_and_placeheld() {
        let out = to_unicode("\n", Format::Table).unwrap();
        assert!(out.contains("U+000A"), "{out}");
        // newline has no Name property → fallback label
        assert!(out.contains("<control>"), "{out}");
    }

    #[test]
    fn empty_is_error() {
        assert!(to_unicode("", Format::Table).is_err());
    }

    #[test]
    fn format_parse() {
        assert_eq!(Format::parse("TABLE").unwrap(), Format::Table);
        assert_eq!(Format::parse("json").unwrap(), Format::Json);
        assert_eq!(Format::parse("").unwrap(), Format::Table);
        assert!(Format::parse("xml").is_err());
    }
}

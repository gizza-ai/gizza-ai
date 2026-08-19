//! gizza-ai/chunk-list core — split a list of items into fixed-size chunks
//! (batches) for paginated APIs, `IN (…)` clauses, bulk imports and rate limits.
//!
//! Pure Rust (only `serde_json` for the JSON output). No wafer/wasm-bindgen deps —
//! shared by the chat skill block and the web page.
//!
//! The input is a blob of text holding the items. `input_separator` says how the
//! items are separated: `auto` (default) splits on ANY of newline, comma,
//! semicolon, tab or pipe, which handles pasted columns and comma lists alike;
//! the explicit choices split on exactly one character; `custom` splits on the
//! literal `custom_separator` string (with `\n`, `\t`, `\r`, `\\` escapes).
//! Items are trimmed and blank items are dropped, so trailing separators and
//! ragged pastes are harmless.
//!
//! The items are then sliced into consecutive groups of `chunk_size` (the last
//! chunk holds the remainder) and rendered as `plain`, `json`, `csv` or
//! `markdown`, optionally labelled with a chunk number and item count.

use serde_json::json;

/// A chunk must hold at least this many items.
pub const MIN_CHUNK_SIZE: usize = 1;
/// Guard against absurd inputs.
pub const MAX_CHUNK_SIZE: usize = 1_000_000;
/// Cap on how many items one run may split.
pub const MAX_ITEMS: usize = 200_000;

/// How the items in the input text are separated.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Separator {
    /// Split on any of newline, comma, semicolon, tab or pipe.
    Auto,
    Comma,
    Newline,
    Semicolon,
    Tab,
    Pipe,
    /// Split on the literal `custom_separator` string.
    Custom,
}

impl Separator {
    pub fn parse(s: &str) -> Result<Separator, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(Separator::Auto),
            "comma" | "," => Ok(Separator::Comma),
            "newline" | "line" | "lines" | "\n" => Ok(Separator::Newline),
            "semicolon" | ";" => Ok(Separator::Semicolon),
            "tab" | "\t" => Ok(Separator::Tab),
            "pipe" | "|" => Ok(Separator::Pipe),
            "custom" => Ok(Separator::Custom),
            other => Err(format!(
                "input_separator must be \"auto\", \"comma\", \"newline\", \"semicolon\", \"tab\", \"pipe\", or \"custom\" (got {other:?})"
            )),
        }
    }
}

/// How the chunks are rendered.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OutputFormat {
    Plain,
    Json,
    Csv,
    Markdown,
}

impl OutputFormat {
    pub fn parse(s: &str) -> Result<OutputFormat, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "plain" | "text" => Ok(OutputFormat::Plain),
            "json" => Ok(OutputFormat::Json),
            "csv" => Ok(OutputFormat::Csv),
            "markdown" | "md" => Ok(OutputFormat::Markdown),
            other => Err(format!(
                "output must be \"plain\", \"json\", \"csv\", or \"markdown\" (got {other:?})"
            )),
        }
    }
}

/// Turn the escapes a user can type into a text field into real characters.
fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// Split the raw input into trimmed, non-empty items.
fn parse_items(text: &str, sep: Separator, custom: &str) -> Result<Vec<String>, String> {
    let raw: Vec<&str> = match sep {
        Separator::Auto => text
            .split(|c| matches!(c, '\n' | '\r' | ',' | ';' | '\t' | '|'))
            .collect(),
        Separator::Comma => text.split(',').collect(),
        // A pasted column is usually CRLF; treat \r as part of the line break.
        Separator::Newline => text.split(|c| matches!(c, '\n' | '\r')).collect(),
        Separator::Semicolon => text.split(';').collect(),
        Separator::Tab => text.split('\t').collect(),
        Separator::Pipe => text.split('|').collect(),
        Separator::Custom => {
            if custom.is_empty() {
                return Err(
                    "custom_separator must not be empty when input_separator is \"custom\""
                        .to_string(),
                );
            }
            let needle = unescape(custom);
            if needle.is_empty() {
                return Err(
                    "custom_separator must not be empty when input_separator is \"custom\""
                        .to_string(),
                );
            }
            return Ok(collect_items(text.split(needle.as_str())));
        }
    };
    Ok(collect_items(raw.into_iter()))
}

fn collect_items<'a>(parts: impl Iterator<Item = &'a str>) -> Vec<String> {
    parts
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// `"3 items"` / `"1 item"`.
fn count_label(n: usize) -> String {
    if n == 1 {
        "1 item".to_string()
    } else {
        format!("{n} items")
    }
}

/// RFC 4180 field quoting.
fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Split `items` into consecutive chunks of `chunk_size` and render them.
///
/// See the module docs for the separator and output semantics. Returns an error
/// for an unknown separator/output, a `chunk_size` outside
/// [`MIN_CHUNK_SIZE`]..=[`MAX_CHUNK_SIZE`], an empty `custom_separator` when
/// `input_separator = "custom"`, an input with no items, or more than
/// [`MAX_ITEMS`] items.
pub fn chunk_list(
    items: &str,
    input_separator: &str,
    custom_separator: &str,
    chunk_size: usize,
    output: &str,
    label_chunks: bool,
) -> Result<String, String> {
    let sep = Separator::parse(input_separator)?;
    let format = OutputFormat::parse(output)?;

    if chunk_size < MIN_CHUNK_SIZE {
        return Err(format!("chunk_size must be at least {MIN_CHUNK_SIZE}"));
    }
    if chunk_size > MAX_CHUNK_SIZE {
        return Err(format!("chunk_size must be at most {MAX_CHUNK_SIZE}"));
    }

    let parsed = parse_items(items, sep, custom_separator)?;
    if parsed.is_empty() {
        return Err("items is empty — paste a list of items to split into chunks".to_string());
    }
    if parsed.len() > MAX_ITEMS {
        return Err(format!(
            "too many items ({}) — the maximum is {MAX_ITEMS}",
            parsed.len()
        ));
    }

    let chunks: Vec<&[String]> = parsed.chunks(chunk_size).collect();

    Ok(match format {
        OutputFormat::Plain => render_plain(&chunks, label_chunks),
        OutputFormat::Json => render_json(&chunks, label_chunks),
        OutputFormat::Csv => render_csv(&chunks, label_chunks),
        OutputFormat::Markdown => render_markdown(&chunks, label_chunks),
    })
}

fn render_plain(chunks: &[&[String]], label: bool) -> String {
    chunks
        .iter()
        .enumerate()
        .map(|(i, chunk)| {
            let line = chunk.join(", ");
            if label {
                format!("Chunk {} ({})\n{line}", i + 1, count_label(chunk.len()))
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn render_json(chunks: &[&[String]], label: bool) -> String {
    let value = if label {
        json!(chunks
            .iter()
            .enumerate()
            .map(|(i, chunk)| json!({
                "chunk": i + 1,
                "count": chunk.len(),
                "items": chunk,
            }))
            .collect::<Vec<_>>())
    } else {
        json!(chunks)
    };
    serde_json::to_string_pretty(&value).unwrap_or_else(|e| e.to_string())
}

fn render_csv(chunks: &[&[String]], label: bool) -> String {
    chunks
        .iter()
        .enumerate()
        .map(|(i, chunk)| {
            let fields = chunk.iter().map(|s| csv_field(s)).collect::<Vec<_>>();
            if label {
                format!("{},{}", i + 1, fields.join(","))
            } else {
                fields.join(",")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_markdown(chunks: &[&[String]], label: bool) -> String {
    let blocks: Vec<String> = chunks
        .iter()
        .enumerate()
        .map(|(i, chunk)| {
            let list = chunk
                .iter()
                .map(|s| format!("- {s}"))
                .collect::<Vec<_>>()
                .join("\n");
            if label {
                format!(
                    "### Chunk {} ({})\n\n{list}",
                    i + 1,
                    count_label(chunk.len())
                )
            } else {
                list
            }
        })
        .collect();
    // Unlabelled bullet lists would run together, so divide them explicitly.
    if label {
        blocks.join("\n\n")
    } else {
        blocks.join("\n\n---\n\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_labelled_chunks_exact_output() {
        let out = chunk_list("a\nb\nc\nd\ne", "newline", "", 2, "plain", true).unwrap();
        assert_eq!(
            out,
            "Chunk 1 (2 items)\na, b\n\nChunk 2 (2 items)\nc, d\n\nChunk 3 (1 item)\ne"
        );
    }

    #[test]
    fn plain_unlabelled_is_just_the_batches() {
        let out = chunk_list("1, 2, 3, 4, 5", "comma", "", 3, "plain", false).unwrap();
        assert_eq!(out, "1, 2, 3\n\n4, 5");
    }

    #[test]
    fn auto_splits_on_any_common_separator_and_drops_blanks() {
        let out = chunk_list("a, b\nc;d\te|f,,\n", "auto", "", 3, "plain", false).unwrap();
        assert_eq!(out, "a, b, c\n\nd, e, f");
    }

    #[test]
    fn csv_labelled_prefixes_the_chunk_number_and_quotes_fields() {
        let out = chunk_list("a|b, c|d", "pipe", "", 2, "csv", true).unwrap();
        assert_eq!(out, "1,a,\"b, c\"\n2,d");
    }

    #[test]
    fn csv_unlabelled_is_one_row_per_chunk() {
        let out = chunk_list("a\tb\tc", "tab", "", 2, "csv", false).unwrap();
        assert_eq!(out, "a,b\nc");
    }

    #[test]
    fn json_labelled_records() {
        let out = chunk_list("x;y;z", "semicolon", "", 2, "json", true).unwrap();
        assert_eq!(
            out,
            "[\n  {\n    \"chunk\": 1,\n    \"count\": 2,\n    \"items\": [\n      \"x\",\n      \"y\"\n    ]\n  },\n  {\n    \"chunk\": 2,\n    \"count\": 1,\n    \"items\": [\n      \"z\"\n    ]\n  }\n]"
        );
    }

    #[test]
    fn json_unlabelled_is_an_array_of_arrays() {
        let out = chunk_list("x,y,z", "comma", "", 2, "json", false).unwrap();
        assert_eq!(
            out,
            "[\n  [\n    \"x\",\n    \"y\"\n  ],\n  [\n    \"z\"\n  ]\n]"
        );
    }

    #[test]
    fn markdown_headings_and_bullets() {
        let out = chunk_list("a\nb\nc", "newline", "", 2, "markdown", true).unwrap();
        assert_eq!(
            out,
            "### Chunk 1 (2 items)\n\n- a\n- b\n\n### Chunk 2 (1 item)\n\n- c"
        );
    }

    #[test]
    fn markdown_unlabelled_divides_the_lists() {
        let out = chunk_list("a\nb\nc", "newline", "", 2, "markdown", false).unwrap();
        assert_eq!(out, "- a\n- b\n\n---\n\n- c");
    }

    #[test]
    fn custom_separator_with_escapes() {
        let out = chunk_list("a :: b :: c", "custom", "::", 2, "plain", false).unwrap();
        assert_eq!(out, "a, b\n\nc");
        // "\n" in the field means a real newline.
        let out = chunk_list("a\nb\nc", "custom", "\\n", 2, "plain", false).unwrap();
        assert_eq!(out, "a, b\n\nc");
    }

    #[test]
    fn chunk_larger_than_the_list_returns_one_chunk() {
        let out = chunk_list("a,b", "comma", "", 10, "plain", false).unwrap();
        assert_eq!(out, "a, b");
    }

    #[test]
    fn chunk_size_zero_is_an_error() {
        let err = chunk_list("a,b", "comma", "", 0, "plain", true).unwrap_err();
        assert!(err.contains("chunk_size must be at least 1"), "{err}");
    }

    #[test]
    fn empty_custom_separator_is_an_error() {
        let err = chunk_list("a,b", "custom", "", 2, "plain", true).unwrap_err();
        assert!(err.contains("custom_separator must not be empty"), "{err}");
    }

    #[test]
    fn empty_items_is_an_error() {
        let err = chunk_list("  \n , ", "auto", "", 2, "plain", true).unwrap_err();
        assert!(err.contains("items is empty"), "{err}");
    }

    #[test]
    fn unknown_separator_and_output_are_errors() {
        assert!(chunk_list("a", "colon", "", 2, "plain", true)
            .unwrap_err()
            .contains("input_separator must be"));
        assert!(chunk_list("a", "comma", "", 2, "yaml", true)
            .unwrap_err()
            .contains("output must be"));
    }
}

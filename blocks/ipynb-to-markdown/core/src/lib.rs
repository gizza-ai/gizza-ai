//! ipynb-to-markdown core — render a Jupyter `.ipynb` notebook into a clean
//! Markdown document: markdown cells verbatim, code cells as fenced blocks
//! tagged with the kernel language, and cell OUTPUTS rendered as output
//! sections (stream text, execute_result/display_data rich reps, and error
//! tracebacks). Unlike a code extractor, this is a document exporter: outputs
//! are included by default, images are embedded inline as base64 `data:` URIs
//! (a single Markdown string has no sidecar `_files/` directory), and the
//! richest available representation of each output is chosen.
//!
//! Only `serde_json` — no wafer/wasm-bindgen deps, so the same logic drives the
//! chat block, the CLI, and the browser page.

use serde_json::Value;

/// How image outputs (and markdown-cell image attachments) are handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageMode {
    /// Embed the image inline as a base64 `data:` URI (default).
    Embed,
    /// Replace the image with a short `*[image output]*` note.
    Placeholder,
    /// Drop image outputs entirely.
    Omit,
}

impl ImageMode {
    pub fn parse(s: &str) -> Result<ImageMode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "embed" | "inline" | "base64" => Ok(ImageMode::Embed),
            "placeholder" | "note" => Ok(ImageMode::Placeholder),
            "omit" | "drop" | "none" => Ok(ImageMode::Omit),
            other => Err(format!(
                "unknown image_mode '{other}' (expected 'embed', 'placeholder', or 'omit')"
            )),
        }
    }
}

/// Options controlling the Markdown render.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Include code cells (as fenced blocks). When false, code is dropped but
    /// each cell's outputs are still rendered (nbconvert `--no-input`).
    pub include_code: bool,
    /// Include each code cell's stored outputs as output sections.
    pub include_outputs: bool,
    /// Include markdown / raw cells. When false only code + outputs remain.
    pub include_markdown: bool,
    /// Prefix code cells with `In [n]:` and their outputs with `Out[n]:`
    /// execution-count prompts.
    pub show_prompts: bool,
    /// How image outputs / attachments are handled.
    pub image_mode: ImageMode,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            include_code: true,
            include_outputs: true,
            include_markdown: true,
            show_prompts: false,
            image_mode: ImageMode::Embed,
        }
    }
}

/// Convert a Jupyter notebook (the raw `.ipynb` JSON) into a Markdown document.
pub fn convert(notebook: &str, opts: Options) -> Result<String, String> {
    let nb: Value = serde_json::from_str(notebook.trim()).map_err(|e| {
        format!("input is not valid JSON: {e}. Paste the full contents of a .ipynb file.")
    })?;

    let cells = extract_cells(&nb)?;
    let lang = notebook_language(&nb);

    let mut blocks: Vec<String> = Vec::new();
    for cell in cells {
        let cell_type = cell.get("cell_type").and_then(|v| v.as_str()).unwrap_or("");
        match cell_type {
            "code" => render_code_cell(&cell, &lang, opts, &mut blocks),
            "markdown" | "raw" => {
                if !opts.include_markdown {
                    continue;
                }
                let src = resolve_attachments(join_source(cell.get("source")), &cell, opts);
                let src = src.trim_matches('\n');
                if !src.trim().is_empty() {
                    blocks.push(src.to_string());
                }
            }
            _ => {}
        }
    }

    if blocks.is_empty() {
        return Err("notebook has no cells to convert (all cells were empty or dropped).".into());
    }

    // One trailing newline — Markdown files conventionally end with one.
    Ok(format!("{}\n", blocks.join("\n\n")))
}

/// Render a single code cell (source fence + optional output sections).
fn render_code_cell(cell: &Value, lang: &str, opts: Options, blocks: &mut Vec<String>) {
    let count = cell.get("execution_count").and_then(|v| v.as_u64());

    if opts.include_code {
        let src = join_source(cell.get("source"));
        let src = src.trim_end_matches('\n');
        if !src.trim().is_empty() {
            let mut block = String::new();
            if opts.show_prompts {
                block.push_str(&prompt_label("In", count));
                block.push('\n');
            }
            block.push_str("```");
            block.push_str(lang);
            block.push('\n');
            block.push_str(src);
            block.push_str("\n```");
            blocks.push(block);
        }
    }

    if opts.include_outputs {
        if let Some(section) = render_outputs(cell.get("outputs"), count, opts) {
            blocks.push(section);
        }
    }
}

/// A bold execution-count prompt, e.g. `**In [3]:**` (or `**In [ ]:**` when null).
fn prompt_label(side: &str, count: Option<u64>) -> String {
    match count {
        Some(n) => format!("**{side} [{n}]:**"),
        None => format!("**{side} [ ]:**"),
    }
}

/// nbformat v4 stores cells at the top level; v3 nests them under `worksheets`.
fn extract_cells(nb: &Value) -> Result<Vec<Value>, String> {
    if let Some(cells) = nb.get("cells").and_then(|v| v.as_array()) {
        return Ok(cells.clone());
    }
    if let Some(sheets) = nb.get("worksheets").and_then(|v| v.as_array()) {
        let mut all = Vec::new();
        for sheet in sheets {
            if let Some(cells) = sheet.get("cells").and_then(|v| v.as_array()) {
                all.extend(cells.iter().cloned());
            }
        }
        if !all.is_empty() {
            return Ok(all);
        }
    }
    Err("this doesn't look like a Jupyter notebook: no 'cells' array found.".into())
}

/// Notebook source language for code fences (default python).
fn notebook_language(nb: &Value) -> String {
    let meta = nb.get("metadata");
    let from = |path: &[&str]| -> Option<String> {
        let mut cur = meta?;
        for key in path {
            cur = cur.get(key)?;
        }
        cur.as_str().map(|s| s.to_string())
    };
    from(&["language_info", "name"])
        .or_else(|| from(&["kernelspec", "language"]))
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "python".to_string())
}

/// nbformat `source`/`text` is a string or an array of line strings (each line
/// usually already carrying its trailing `\n`). Join to a single string.
fn join_source(source: Option<&Value>) -> String {
    match source {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(lines)) => lines
            .iter()
            .filter_map(|l| l.as_str())
            .collect::<Vec<_>>()
            .concat(),
        _ => String::new(),
    }
}

/// Render the output sections of a code cell. Returns `None` when there is no
/// renderable output. Multiple outputs are separated by blank lines.
fn render_outputs(outputs: Option<&Value>, count: Option<u64>, opts: Options) -> Option<String> {
    let arr = outputs.and_then(|v| v.as_array())?;
    let mut pieces: Vec<String> = Vec::new();
    for out in arr {
        let kind = out.get("output_type").and_then(|v| v.as_str()).unwrap_or("");
        let piece = match kind {
            "stream" => {
                let text = join_source(out.get("text"));
                fenced_text(&text)
            }
            "execute_result" | "display_data" => render_mime_bundle(out.get("data"), opts),
            "error" => {
                let tb = join_source(out.get("traceback"));
                let text = if tb.trim().is_empty() {
                    let ename = out.get("ename").and_then(|v| v.as_str()).unwrap_or("");
                    let evalue = out.get("evalue").and_then(|v| v.as_str()).unwrap_or("");
                    format!("{ename}: {evalue}")
                } else {
                    strip_ansi(&tb)
                };
                fenced_text(text.trim_end())
            }
            _ => None,
        };
        if let Some(p) = piece {
            if !p.trim().is_empty() {
                pieces.push(p);
            }
        }
    }

    if pieces.is_empty() {
        return None;
    }

    let mut body = pieces.join("\n\n");
    if opts.show_prompts {
        body = format!("{}\n{body}", prompt_label("Out", count));
    }
    Some(body)
}

/// Wrap plain text in a bare fenced block. `None` if the text is empty.
fn fenced_text(text: &str) -> Option<String> {
    let text = text.trim_end_matches('\n');
    if text.trim().is_empty() {
        return None;
    }
    Some(format!("```\n{text}\n```"))
}

/// Pick and render the richest representation from a MIME bundle. Priority
/// favours a clean visual Markdown document: rendered Markdown, then images,
/// then HTML (e.g. DataFrame tables), then LaTeX, then a plain-text fallback.
fn render_mime_bundle(data: Option<&Value>, opts: Options) -> Option<String> {
    let data = data?;

    // text/markdown — emit verbatim (it IS Markdown).
    if let Some(md) = str_rep(data, "text/markdown") {
        let md = md.trim_matches('\n');
        if !md.trim().is_empty() {
            return Some(md.to_string());
        }
    }

    // Image representations → inline image / placeholder / omit.
    for (mime, base64) in [
        ("image/svg+xml", false),
        ("image/png", true),
        ("image/jpeg", true),
        ("image/gif", true),
    ] {
        if let Some(raw) = str_rep(data, mime) {
            if raw.trim().is_empty() {
                continue;
            }
            return match opts.image_mode {
                ImageMode::Omit => None,
                ImageMode::Placeholder => Some("*[image output]*".to_string()),
                ImageMode::Embed => Some(format!("![output]({})", data_uri(mime, &raw, base64))),
            };
        }
    }

    // text/html — kept as raw inline HTML (valid inside Markdown; renders tables).
    if let Some(html) = str_rep(data, "text/html") {
        let html = html.trim_matches('\n');
        if !html.trim().is_empty() {
            return Some(html.to_string());
        }
    }

    // text/latex — emit verbatim (already delimited, e.g. `$...$`).
    if let Some(tex) = str_rep(data, "text/latex") {
        let tex = tex.trim_matches('\n');
        if !tex.trim().is_empty() {
            return Some(tex.to_string());
        }
    }

    // text/plain — fenced fallback.
    if let Some(text) = str_rep(data, "text/plain") {
        return fenced_text(&text);
    }

    None
}

/// A MIME representation as a joined string (handles the string-or-array form).
fn str_rep(data: &Value, mime: &str) -> Option<String> {
    data.get(mime).map(|v| join_source(Some(v)))
}

/// Build a `data:` URI. `is_base64` reps (png/jpeg/gif) are already base64 in
/// the notebook JSON — strip embedded whitespace/newlines and pass through.
/// Text reps (svg) are base64-encoded here so they render as an `<img>` source.
fn data_uri(mime: &str, raw: &str, is_base64: bool) -> String {
    if is_base64 {
        let b64: String = raw.chars().filter(|c| !c.is_whitespace()).collect();
        format!("data:{mime};base64,{b64}")
    } else {
        format!("data:{mime};base64,{}", base64_encode(raw.as_bytes()))
    }
}

/// Resolve `attachment:<name>` references in a markdown cell to inline `data:`
/// URIs using the cell's `attachments` bundle. Only in Embed mode; otherwise
/// the reference is left untouched.
fn resolve_attachments(src: String, cell: &Value, opts: Options) -> String {
    if opts.image_mode != ImageMode::Embed {
        return src;
    }
    let attachments = match cell.get("attachments").and_then(|v| v.as_object()) {
        Some(a) if !a.is_empty() => a,
        _ => return src,
    };
    let mut out = src;
    for (name, bundle) in attachments {
        let needle = format!("attachment:{name}");
        if !out.contains(&needle) {
            continue;
        }
        // Each attachment is a MIME bundle; take its first image rep.
        if let Some(obj) = bundle.as_object() {
            if let Some((mime, val)) = obj.iter().find(|(m, _)| m.starts_with("image/")) {
                let raw = join_source(Some(val));
                let is_b64 = mime != "image/svg+xml";
                let uri = data_uri(mime, &raw, is_b64);
                out = out.replace(&needle, &uri);
            }
        }
    }
    out
}

/// Drop ANSI escape sequences (error tracebacks are colorized).
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                while let Some(&n) = chars.peek() {
                    chars.next();
                    if n.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Standard base64 encoder (no padding omitted) — used only for the SVG rep,
/// which is stored as text in the notebook JSON.
fn base64_encode(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((bytes.len() + 2) / 3 * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            T[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const NB: &str = r##"{
        "cells": [
            {"cell_type": "markdown", "metadata": {}, "source": ["# Title\n", "\n", "Some text"]},
            {"cell_type": "code", "execution_count": 3, "metadata": {}, "outputs": [
                {"output_type": "stream", "name": "stdout", "text": ["hello\n", "world\n"]},
                {"output_type": "execute_result", "execution_count": 3, "data": {"text/plain": ["42"]}, "metadata": {}}
            ], "source": ["print('hello')\n", "x = 6 * 7"]},
            {"cell_type": "code", "execution_count": null, "metadata": {}, "outputs": [], "source": ""}
        ],
        "metadata": {"language_info": {"name": "python"}},
        "nbformat": 4, "nbformat_minor": 5
    }"##;

    #[test]
    fn full_render_includes_code_and_outputs() {
        let out = convert(NB, Options::default()).unwrap();
        assert_eq!(
            out,
            "# Title\n\nSome text\n\n```python\nprint('hello')\nx = 6 * 7\n```\n\n```\nhello\nworld\n```\n\n```\n42\n```\n"
        );
    }

    #[test]
    fn no_input_keeps_outputs_drops_code() {
        let opts = Options {
            include_code: false,
            ..Options::default()
        };
        let out = convert(NB, opts).unwrap();
        assert_eq!(out, "# Title\n\nSome text\n\n```\nhello\nworld\n```\n\n```\n42\n```\n");
    }

    #[test]
    fn outputs_can_be_dropped() {
        let opts = Options {
            include_outputs: false,
            ..Options::default()
        };
        let out = convert(NB, opts).unwrap();
        assert_eq!(out, "# Title\n\nSome text\n\n```python\nprint('hello')\nx = 6 * 7\n```\n");
    }

    #[test]
    fn markdown_can_be_dropped() {
        let opts = Options {
            include_markdown: false,
            ..Options::default()
        };
        let out = convert(NB, opts).unwrap();
        assert_eq!(out, "```python\nprint('hello')\nx = 6 * 7\n```\n\n```\nhello\nworld\n```\n\n```\n42\n```\n");
    }

    #[test]
    fn prompts_label_in_and_out() {
        let opts = Options {
            show_prompts: true,
            ..Options::default()
        };
        let out = convert(NB, opts).unwrap();
        assert_eq!(
            out,
            "# Title\n\nSome text\n\n**In [3]:**\n```python\nprint('hello')\nx = 6 * 7\n```\n\n**Out [3]:**\n```\nhello\nworld\n```\n\n```\n42\n```\n"
        );
    }

    #[test]
    fn png_output_embeds_as_data_uri() {
        let nb = r##"{
            "cells": [{"cell_type": "code", "execution_count": 1, "outputs": [
                {"output_type": "display_data", "data": {"image/png": "iVBORw0KGgo=\n", "text/plain": ["<Figure>"]}, "metadata": {}}
            ], "source": ["plot()"]}],
            "metadata": {}, "nbformat": 4, "nbformat_minor": 5
        }"##;
        let out = convert(nb, Options::default()).unwrap();
        assert!(
            out.contains("![output](data:image/png;base64,iVBORw0KGgo=)"),
            "got: {out}"
        );
        assert!(!out.contains("Figure"), "text/plain should not win over the image: {out}");
    }

    #[test]
    fn image_placeholder_and_omit() {
        let nb = r##"{
            "cells": [{"cell_type": "code", "outputs": [
                {"output_type": "display_data", "data": {"image/png": "iVBORw0KGgo="}, "metadata": {}}
            ], "source": ["plot()"]}],
            "metadata": {}, "nbformat": 4, "nbformat_minor": 5
        }"##;
        let placeholder = convert(nb, Options { image_mode: ImageMode::Placeholder, ..Options::default() }).unwrap();
        assert!(placeholder.contains("*[image output]*"), "got: {placeholder}");
        let omit = convert(nb, Options { image_mode: ImageMode::Omit, ..Options::default() }).unwrap();
        assert!(!omit.contains("data:image"), "got: {omit}");
        assert!(!omit.contains("image output"), "got: {omit}");
    }

    #[test]
    fn html_rep_kept_over_plain() {
        let nb = r##"{
            "cells": [{"cell_type": "code", "outputs": [
                {"output_type": "execute_result", "data": {"text/html": ["<table><tr><td>1</td></tr></table>"], "text/plain": ["   a\n0  1"]}, "metadata": {}}
            ], "source": ["df"]}],
            "metadata": {}, "nbformat": 4, "nbformat_minor": 5
        }"##;
        let out = convert(nb, Options::default()).unwrap();
        assert!(out.contains("<table><tr><td>1</td></tr></table>"), "got: {out}");
    }

    #[test]
    fn markdown_rep_wins_verbatim() {
        let nb = r##"{
            "cells": [{"cell_type": "code", "outputs": [
                {"output_type": "execute_result", "data": {"text/markdown": ["**Result**\n", "\n", "done"], "text/plain": ["<obj>"]}, "metadata": {}}
            ], "source": ["show()"]}],
            "metadata": {}, "nbformat": 4, "nbformat_minor": 5
        }"##;
        let out = convert(nb, Options::default()).unwrap();
        assert!(out.contains("**Result**\n\ndone"), "got: {out}");
        // The Markdown rep is emitted verbatim, not wrapped in a text fence.
        assert!(!out.contains("```\n**Result**"), "markdown rep should not be fenced: {out}");
    }

    #[test]
    fn svg_output_base64_encoded() {
        let nb = r##"{
            "cells": [{"cell_type": "code", "outputs": [
                {"output_type": "display_data", "data": {"image/svg+xml": "<svg/>"}, "metadata": {}}
            ], "source": ["plot()"]}],
            "metadata": {}, "nbformat": 4, "nbformat_minor": 5
        }"##;
        let out = convert(nb, Options::default()).unwrap();
        // base64("<svg/>") == "PHN2Zy8+"
        assert!(out.contains("![output](data:image/svg+xml;base64,PHN2Zy8+)"), "got: {out}");
    }

    #[test]
    fn error_output_ansi_stripped() {
        let nb = r#"{"cells": [{"cell_type": "code", "outputs": [
            {"output_type": "error", "ename": "ValueError", "evalue": "bad", "traceback": ["\u001b[31mValueError\u001b[0m: bad"]}
        ], "source": ["boom()"]}], "metadata": {}, "nbformat": 4, "nbformat_minor": 5}"#;
        let out = convert(nb, Options::default()).unwrap();
        assert!(out.contains("```\nValueError: bad\n```"), "got: {out}");
        assert!(!out.contains('\u{1b}'), "ansi should be stripped: {out}");
    }

    #[test]
    fn markdown_attachment_resolved() {
        let nb = r##"{
            "cells": [{"cell_type": "markdown", "attachments": {"a.png": {"image/png": "iVBORw0KGgo="}}, "source": ["![pic](attachment:a.png)"]}],
            "metadata": {}, "nbformat": 4, "nbformat_minor": 5
        }"##;
        let out = convert(nb, Options::default()).unwrap();
        assert!(out.contains("![pic](data:image/png;base64,iVBORw0KGgo=)"), "got: {out}");
    }

    #[test]
    fn nbformat_v3_worksheets() {
        let nb = r##"{
            "worksheets": [{"cells": [
                {"cell_type": "markdown", "source": ["hi"]},
                {"cell_type": "code", "input": ["x=1"], "outputs": []}
            ]}],
            "metadata": {}, "nbformat": 3, "nbformat_minor": 0
        }"##;
        // v3 code cells use "input" not "source"; we only guarantee the markdown
        // renders + no crash (v3 is legacy). Assert the markdown cell survives.
        let out = convert(nb, Options::default()).unwrap();
        assert!(out.starts_with("hi"), "got: {out}");
    }

    #[test]
    fn custom_language_fence() {
        let nb = r##"{
            "cells": [{"cell_type": "code", "source": ["SELECT 1"], "outputs": []}],
            "metadata": {"kernelspec": {"language": "sql"}}, "nbformat": 4, "nbformat_minor": 5
        }"##;
        let out = convert(nb, Options::default()).unwrap();
        assert!(out.contains("```sql\nSELECT 1\n```"), "got: {out}");
    }

    #[test]
    fn invalid_json_errors() {
        let err = convert("not json", Options::default()).unwrap_err();
        assert!(err.contains("not valid JSON"), "got: {err}");
    }

    #[test]
    fn missing_cells_errors() {
        let err = convert(r#"{"foo": 1}"#, Options::default()).unwrap_err();
        assert!(err.contains("no 'cells' array"), "got: {err}");
    }

    #[test]
    fn all_empty_errors() {
        let nb = r#"{"cells": [{"cell_type": "code", "source": "", "outputs": []}], "metadata": {}, "nbformat": 4, "nbformat_minor": 5}"#;
        let err = convert(nb, Options::default()).unwrap_err();
        assert!(err.contains("no cells to convert"), "got: {err}");
    }

    #[test]
    fn image_mode_parse() {
        assert_eq!(ImageMode::parse("embed").unwrap(), ImageMode::Embed);
        assert_eq!(ImageMode::parse("Placeholder").unwrap(), ImageMode::Placeholder);
        assert_eq!(ImageMode::parse("OMIT").unwrap(), ImageMode::Omit);
        assert!(ImageMode::parse("zzz").is_err());
    }

    #[test]
    fn base64_known_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"<svg/>"), "PHN2Zy8+");
    }
}

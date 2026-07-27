//! ipynb-to-script core — extract the code cells of a Jupyter `.ipynb` notebook
//! into a clean Python script (or Markdown), dropping outputs and execution
//! counts. No wafer/wasm-bindgen deps (serde_json only). Preserves cell order.

use serde_json::Value;

/// Output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    /// A `.py` script: code cells verbatim, markdown cells as `#` comments.
    Script,
    /// A `.md` document: markdown cells verbatim, code cells as fenced blocks.
    Markdown,
}

impl Output {
    pub fn parse(s: &str) -> Result<Output, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "script" | "python" | "py" => Ok(Output::Script),
            "markdown" | "md" => Ok(Output::Markdown),
            other => Err(format!(
                "unknown output '{other}' (expected 'script' or 'markdown')"
            )),
        }
    }
}

/// Convert a Jupyter notebook (the raw `.ipynb` JSON) into a script or Markdown.
///
/// - `include_markdown` — keep markdown/raw cells (as `#` comments in script mode,
///   verbatim in markdown mode); when false they are dropped entirely.
/// - `include_outputs` — append each code cell's stored text outputs (as `# `
///   comments in script mode, as fenced blocks in markdown mode). Off by default —
///   the tool's whole purpose is dropping outputs.
/// - `cell_markers` — emit `# %%` (VS Code / Jupytext "percent") markers before
///   each cell in script mode. Ignored in markdown mode.
pub fn convert(
    notebook: &str,
    output: Output,
    include_markdown: bool,
    include_outputs: bool,
    cell_markers: bool,
) -> Result<String, String> {
    let nb: Value = serde_json::from_str(notebook.trim())
        .map_err(|e| format!("input is not valid JSON: {e}. Paste the full contents of a .ipynb file."))?;

    let cells = extract_cells(&nb)?;
    let lang = notebook_language(&nb);

    let mut blocks: Vec<String> = Vec::new();
    for cell in cells {
        let cell_type = cell.get("cell_type").and_then(|v| v.as_str()).unwrap_or("");
        let src = join_source(cell.get("source"));

        match cell_type {
            "code" => {
                // Blank code cells are skipped so the script stays clean.
                let mut block = String::new();
                if !src.trim().is_empty() {
                    match output {
                        Output::Script => {
                            if cell_markers {
                                block.push_str("# %%\n");
                            }
                            block.push_str(src.trim_end());
                        }
                        Output::Markdown => {
                            block.push_str("```");
                            block.push_str(&lang);
                            block.push('\n');
                            block.push_str(src.trim_end());
                            block.push_str("\n```");
                        }
                    }
                }
                if include_outputs {
                    if let Some(out) = render_outputs(cell.get("outputs"), output) {
                        if !block.is_empty() {
                            block.push('\n');
                        }
                        block.push_str(&out);
                    }
                }
                if !block.is_empty() {
                    blocks.push(block);
                }
            }
            "markdown" | "raw" => {
                if !include_markdown || src.trim().is_empty() {
                    continue;
                }
                let mut block = String::new();
                match output {
                    Output::Script => {
                        if cell_markers {
                            block.push_str("# %% [markdown]\n");
                        }
                        block.push_str(&comment_lines(src.trim_end()));
                    }
                    Output::Markdown => {
                        block.push_str(src.trim_end());
                    }
                }
                blocks.push(block);
            }
            _ => {}
        }
    }

    if blocks.is_empty() {
        return Err("notebook has no cells to convert (all cells were empty or dropped).".into());
    }

    Ok(blocks.join("\n\n"))
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

/// Notebook source language for markdown fences (default python).
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

/// nbformat `source` is a string or an array of line strings (each usually
/// already carrying its trailing `\n`). Join to a single string.
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

/// Prefix every line with `# ` (blank lines become a bare `#`).
fn comment_lines(text: &str) -> String {
    text.lines()
        .map(|l| {
            if l.is_empty() {
                "#".to_string()
            } else {
                format!("# {l}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Collect the stored text outputs of a code cell (stream text, execute_result /
/// display_data `text/plain`, and error tracebacks), rendered for `output`.
/// Returns `None` when there is no textual output.
fn render_outputs(outputs: Option<&Value>, output: Output) -> Option<String> {
    let arr = outputs.and_then(|v| v.as_array())?;
    let mut text = String::new();
    for out in arr {
        let kind = out.get("output_type").and_then(|v| v.as_str()).unwrap_or("");
        let piece = match kind {
            "stream" => join_source(out.get("text")),
            "execute_result" | "display_data" => out
                .get("data")
                .and_then(|d| d.get("text/plain"))
                .map(|t| join_source(Some(t)))
                .unwrap_or_default(),
            "error" => {
                let tb = join_source(out.get("traceback"));
                if tb.trim().is_empty() {
                    let ename = out.get("ename").and_then(|v| v.as_str()).unwrap_or("");
                    let evalue = out.get("evalue").and_then(|v| v.as_str()).unwrap_or("");
                    format!("{ename}: {evalue}")
                } else {
                    strip_ansi(&tb)
                }
            }
            _ => String::new(),
        };
        if !piece.trim_end().is_empty() {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(piece.trim_end());
        }
    }
    let text = text.trim_end();
    if text.is_empty() {
        return None;
    }
    match output {
        Output::Script => {
            let mut s = String::from("# Output:\n");
            s.push_str(&comment_lines(text));
            Some(s)
        }
        Output::Markdown => Some(format!("```\n{text}\n```")),
    }
}

/// Drop ANSI escape sequences (error tracebacks are colorized).
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            // Skip until a letter terminates the CSI sequence.
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

#[cfg(test)]
mod tests {
    use super::*;

    const NB: &str = r##"{
        "cells": [
            {"cell_type": "markdown", "metadata": {}, "source": ["# Title\n", "\n", "Some text"]},
            {"cell_type": "code", "execution_count": 3, "metadata": {}, "outputs": [
                {"output_type": "stream", "name": "stdout", "text": ["hello\n"]}
            ], "source": ["print('hello')\n", "x = 1"]},
            {"cell_type": "code", "execution_count": null, "metadata": {}, "outputs": [], "source": ""}
        ],
        "metadata": {"language_info": {"name": "python"}},
        "nbformat": 4, "nbformat_minor": 5
    }"##;

    #[test]
    fn script_drops_outputs_and_comments_markdown() {
        let out = convert(NB, Output::Script, true, false, false).unwrap();
        assert_eq!(out, "# # Title\n#\n# Some text\n\nprint('hello')\nx = 1");
    }

    #[test]
    fn script_can_drop_markdown() {
        let out = convert(NB, Output::Script, false, false, false).unwrap();
        assert_eq!(out, "print('hello')\nx = 1");
    }

    #[test]
    fn cell_markers_added() {
        let out = convert(NB, Output::Script, true, false, true).unwrap();
        assert_eq!(
            out,
            "# %% [markdown]\n# # Title\n#\n# Some text\n\n# %%\nprint('hello')\nx = 1"
        );
    }

    #[test]
    fn include_outputs_as_comments() {
        let out = convert(NB, Output::Script, false, true, false).unwrap();
        assert_eq!(out, "print('hello')\nx = 1\n# Output:\n# hello");
    }

    #[test]
    fn markdown_mode_fences_code() {
        let out = convert(NB, Output::Markdown, true, false, false).unwrap();
        assert_eq!(
            out,
            "# Title\n\nSome text\n\n```python\nprint('hello')\nx = 1\n```"
        );
    }

    #[test]
    fn invalid_json_errors() {
        let err = convert("not json", Output::Script, true, false, false).unwrap_err();
        assert!(err.contains("not valid JSON"), "got: {err}");
    }

    #[test]
    fn missing_cells_errors() {
        let err = convert(r#"{"foo": 1}"#, Output::Script, true, false, false).unwrap_err();
        assert!(err.contains("no 'cells' array"), "got: {err}");
    }

    #[test]
    fn output_parse() {
        assert_eq!(Output::parse("script").unwrap(), Output::Script);
        assert_eq!(Output::parse("MD").unwrap(), Output::Markdown);
        assert!(Output::parse("xml").is_err());
    }
}

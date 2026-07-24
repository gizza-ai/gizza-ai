//! gizza-ai/json-from-logs core — scan mixed log/console text for EMBEDDED JSON
//! objects/arrays and extract each one as a separately pretty-printed, validated
//! block. Pure-Rust (no I/O). Key order is preserved (serde_json `preserve_order`).
//!
//! Algorithm: walk the bytes; at every `{` or `[` that isn't inside a matched
//! block, brace-match a balanced run (string- and escape-aware), then VALIDATE
//! that run with serde_json. Only runs that parse are kept, so a stray `{` in
//! prose (or a `{`…`]` mismatch) is discarded and scanning resumes one byte on.
//! Matched blocks are skipped over, so nested JSON isn't double-extracted — only
//! top-level embedded values are returned.

use serde::Serialize;
use serde_json::Value;

/// Reject inputs larger than this (bytes) so the O(n) scan can't be turned
/// pathological. 2 MB comfortably covers pasted console dumps.
pub const MAX_INPUT: usize = 2_000_000;

/// One extracted JSON block: the 1-based line it started on + the parsed value.
struct Block {
    line: usize,
    value: Value,
}

/// Starting at `open` (a `{` or `[`), return the index just past the matching
/// close bracket, or `None` if the run is unterminated. String- and escape-aware,
/// so brackets inside `"…"` don't affect depth. Bracket TYPE isn't checked here
/// (a `{`…`]` still balances by depth) — serde_json validation is the real gate.
fn scan_balanced(bytes: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escape = false;
    for (j, &b) in bytes.iter().enumerate().skip(open) {
        if in_string {
            if escape {
                escape = false;
            } else if b == b'\\' {
                escape = true;
            } else if b == b'"' {
                in_string = false;
            }
            continue;
        }
        match b {
            b'"' => in_string = true,
            b'{' | b'[' => depth += 1,
            b'}' | b']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(j + 1);
                }
            }
            _ => {}
        }
    }
    None
}

/// Pretty-print (or minify) a JSON value. `indent` spaces per level, clamped to
/// 0..=8; `indent == 0` produces a single compact line.
fn render(value: &Value, indent: usize) -> String {
    let n = indent.min(8);
    if n == 0 {
        return serde_json::to_string(value).unwrap_or_default();
    }
    let pad = vec![b' '; n];
    let mut buf = Vec::new();
    let fmt = serde_json::ser::PrettyFormatter::with_indent(&pad);
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
    value.serialize(&mut ser).ok();
    String::from_utf8(buf).unwrap_or_default()
}

/// Scan `text` and return every embedded, valid top-level JSON object/array.
fn extract(text: &str) -> Vec<Block> {
    let bytes = text.as_bytes();
    let mut blocks = Vec::new();
    let mut i = 0usize;
    let mut line = 1usize;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'{' || b == b'[' {
            if let Some(end) = scan_balanced(bytes, i) {
                // `i` and `end` sit on ASCII brackets → always char boundaries.
                if let Ok(value) = serde_json::from_str::<Value>(&text[i..end]) {
                    blocks.push(Block { line, value });
                    for &k in &bytes[i..end] {
                        if k == b'\n' {
                            line += 1;
                        }
                    }
                    i = end;
                    continue;
                }
            }
        }
        if b == b'\n' {
            line += 1;
        }
        i += 1;
    }
    blocks
}

/// Public entry: extract embedded JSON from `text` and format the result.
///
/// - `indent`: spaces per level (0..=8; 0 minifies each block).
/// - `output`: `"blocks"` (default) prints each block under a `// block N (line L)`
///   header, blocks separated by a blank line; `"array"` wraps every extracted
///   block into one pretty JSON array.
///
/// Errors if the input is empty, too large, or contains no valid JSON block.
pub fn run(text: &str, indent: usize, output: &str) -> Result<String, String> {
    if text.trim().is_empty() {
        return Err("no input: paste log or console text containing JSON".into());
    }
    if text.len() > MAX_INPUT {
        return Err(format!(
            "input too large: {} bytes (max {} bytes)",
            text.len(),
            MAX_INPUT
        ));
    }
    let mode = if output.trim().is_empty() { "blocks" } else { output.trim() };
    let blocks = extract(text);
    if blocks.is_empty() {
        return Err("no JSON objects or arrays found in the input".into());
    }
    match mode {
        "array" => {
            let arr = Value::Array(blocks.into_iter().map(|b| b.value).collect());
            Ok(render(&arr, indent))
        }
        "blocks" => {
            let parts: Vec<String> = blocks
                .iter()
                .enumerate()
                .map(|(idx, b)| {
                    format!("// block {} (line {})\n{}", idx + 1, b.line, render(&b.value, indent))
                })
                .collect();
            Ok(parts.join("\n\n"))
        }
        other => Err(format!(
            "unknown output '{other}': expected 'blocks' or 'array'"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_two_embedded_blocks() {
        let logs = "2026-07-24 INFO starting up\n\
                    2026-07-24 DEBUG state={\"user\":\"gizza\",\"ok\":true}\n\
                    2026-07-24 INFO ids [1, 2, 3] processed\n";
        let out = run(logs, 2, "blocks").unwrap();
        let expected = "// block 1 (line 2)\n\
                        {\n  \"user\": \"gizza\",\n  \"ok\": true\n}\n\n\
                        // block 2 (line 3)\n\
                        [\n  1,\n  2,\n  3\n]";
        assert_eq!(out, expected);
    }

    #[test]
    fn preserves_key_order() {
        let out = run("x {\"b\":1,\"a\":2} y", 2, "blocks").unwrap();
        assert!(out.find("\"b\"").unwrap() < out.find("\"a\"").unwrap());
    }

    #[test]
    fn array_mode_wraps_all_blocks() {
        let logs = "a {\"n\":1} b {\"n\":2} c";
        let out = run(logs, 2, "array").unwrap();
        assert_eq!(out, "[\n  {\n    \"n\": 1\n  },\n  {\n    \"n\": 2\n  }\n]");
    }

    #[test]
    fn indent_zero_minifies() {
        let out = run("log {\"a\":[1, 2]}", 0, "blocks").unwrap();
        assert_eq!(out, "// block 1 (line 1)\n{\"a\":[1,2]}");
    }

    #[test]
    fn brace_in_prose_is_skipped() {
        // "{done}" is not valid JSON; the real object after it still extracts.
        let out = run("all {done} here {\"ok\":1}", 2, "blocks").unwrap();
        assert_eq!(out, "// block 1 (line 1)\n{\n  \"ok\": 1\n}");
    }

    #[test]
    fn nested_json_is_not_double_extracted() {
        // One top-level object with a nested object → exactly one block.
        let out = run("x {\"a\":{\"b\":1}} y", 2, "array").unwrap();
        assert_eq!(out, "[\n  {\n    \"a\": {\n      \"b\": 1\n    }\n  }\n]");
    }

    #[test]
    fn braces_inside_strings_do_not_break_matching() {
        let out = run("msg={\"text\":\"a } b ] c\"}", 0, "blocks").unwrap();
        assert_eq!(out, "// block 1 (line 1)\n{\"text\":\"a } b ] c\"}");
    }

    #[test]
    fn empty_input_errors() {
        assert!(run("", 2, "blocks").is_err());
        assert!(run("   \n\t ", 2, "blocks").is_err());
    }

    #[test]
    fn no_json_errors() {
        let err = run("just a plain log line, nothing structured", 2, "blocks").unwrap_err();
        assert!(err.contains("no JSON"));
    }

    #[test]
    fn unknown_output_errors() {
        assert!(run("{\"a\":1}", 2, "yaml").is_err());
    }

    #[test]
    fn oversized_input_errors() {
        let big = "x".repeat(MAX_INPUT + 1);
        assert!(run(&big, 2, "blocks").is_err());
    }
}

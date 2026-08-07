## About this tool

Paste a source snippet and extract the comments as a plain list, JSON records, a Markdown table, or a statistics summary. You can also choose **Source with comments removed** to strip the matched comments while preserving newlines so code line numbers stay recognizable.

The scanner covers common comment syntaxes used by JavaScript, TypeScript, Python, Java, C#, C, C++, Go, Rust, PHP, Ruby, shell, SQL, HTML/XML, CSS, Lua and YAML-style files. It is string-aware, so `//`, `/* */` and `#` inside ordinary strings, Go backtick strings, Rust raw strings and Python triple-quoted strings are not treated as comments.

### Worked example

Input:

```js
// File header
const url = "https://example.com//path"; // Real trailing comment
/* Multi-line
   block note */
console.log(url);
```

With `language = javascript`, `output = comments`, `strip_markers = true`, and `line_numbers = true`, the result is:

```text
[L1] File header
[L2] Real trailing comment
[L3] Multi-line
block note
```

Use `kind = doc` to keep only documentation comments such as `/** ... */`, `///`, `//!`, `##` and Python docstrings. Use `min_length` to drop short noise comments before listing or stripping.

## Limits and edge cases

This is a deterministic tokenizer, not a full language parser. Unterminated block comments run to the end of the input, shell here-doc bodies are not parsed as a separate grammar, and custom comment syntaxes are not configurable. Output is capped at 50,000 comments to keep browser and CLI runs bounded.

## FAQ

<details>
<summary>Can it remove comments as well as list them?</summary>

Yes. Set `output` to `stripped` to return the original source with the selected comment kind removed. Newlines inside removed comments are kept so remaining code stays close to its original line numbers.

</details>

<details>
<summary>Will URLs or comment markers inside strings be extracted by mistake?</summary>

No for the supported string forms. The tokenizer skips quoted strings, character literals, JavaScript template strings, Go backtick strings and Rust raw strings before looking for comment openers, so `"https://example.com//path"` is left alone.

</details>

<details>
<summary>How does Python docstring handling work?</summary>

When `docstrings` is enabled, a triple-quoted Python string that starts its own line is classified as a documentation comment. Turn `docstrings` off if you only want real `#` comments from Python code.

</details>

<details>
<summary>Why not scan a whole repository?</summary>

This block runs in the browser and CLI on one pasted input at a time. Repository traversal, git revisions and per-file rollups belong in a filesystem-aware tool; this one focuses on a portable single-snippet workflow.

</details>

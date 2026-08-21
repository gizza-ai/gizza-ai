## About this tool

Text files fail in quiet ways: a CSV looks fine until a parser sees mixed CRLF and LF, a script has a UTF-8 BOM before its shebang, or a generated file is one giant line with no final newline. This inspector gives you the status-bar and linter facts in one report: detected encoding, BOM presence, line-ending style, line count, longest lines, trailing whitespace, tab/space indentation and control-character counts.

On this page, paste text directly. The CLI and chat tool can inspect exact file bytes with `input_format=base64` or `input_format=hex`, which is the right path when you need to preserve a BOM, CRLF endings or legacy encodings exactly.

Example CLI use with exact file bytes:

```bash
gizza tool text-file-inspector input="$(base64 -w0 README.md)" input_format=base64 max_line_length=120 preview_lines=3
```

The tool diagnoses only; it never rewrites the file. Use an editor, formatter or `text-encoding-converter` when you want to change encodings or line endings.

## FAQ

<details>
<summary>Why does pasted text not show my original CRLF endings or BOM?</summary>

Browsers and terminals often normalize pasted text before JavaScript sees it, and a normal text field cannot carry a file's byte-order mark. Use the CLI with `input_format=base64` for exact byte-level inspection of a real file.

</details>

<details>
<summary>What does MIXED line endings mean?</summary>

The input contains more than one terminator style, such as both CRLF and LF. That can create noisy Git diffs and can break scripts or old parsers that expect one style throughout the file.

</details>

<details>
<summary>Can this convert the file to UTF-8 or normalize line endings?</summary>

No. This block is read-only and reports what is present. Use the report to decide what to fix, then convert with your editor, formatter, or a dedicated encoding/line-ending conversion tool.

</details>

<details>
<summary>Why does the report mention a missing final newline?</summary>

Many POSIX tools and linters expect text files to end with a newline. A missing final newline is not always fatal, but it can produce warnings and awkward diffs when another tool appends to the file.

</details>

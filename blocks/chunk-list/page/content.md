## About this tool

Chunk a list into fixed-size batches when a service, import job, prompt, or review
workflow has a practical limit. Paste one item per line, a comma-separated list,
a tab-separated spreadsheet column, or pipe/semicolon/custom-separated text. The
tool trims whitespace, drops blank entries, keeps the original order, and puts
any remainder in the final chunk.

Plain text output is useful for copying batches into tickets or prompts. CSV puts
one chunk on each row for spreadsheets. JSON is convenient for scripts, and
Markdown turns each chunk into a small checklist.

Worked example:

1. Paste `alpha`, `beta`, `gamma`, `delta`, and `epsilon` on separate lines.
2. Set chunk size to `2`.
3. Choose plain text output with labels on.
4. Copy the three batches: two pairs plus the final single item.

Limits and edge cases: chunk size must be at least 1 and at most 1,000,000. A run
can split up to 200,000 items. Auto-detect splits on newlines, commas,
semicolons, tabs, and pipes, so choose an explicit separator when an item itself
contains one of those characters.

## FAQ

<details>
<summary>Does auto-detect preserve commas inside an item?</summary>

No. Auto-detect treats commas as separators. If your items can contain commas,
choose newline, tab, pipe, or custom separator and paste the list in that form.

</details>

<details>
<summary>What happens when the list length is not divisible by the chunk size?</summary>

The final chunk contains the remainder. For example, five items with chunk size
`2` become chunks of `2`, `2`, and `1` item.

</details>

<details>
<summary>Can I output chunks for a spreadsheet?</summary>

Yes. Choose CSV output. Each chunk becomes one CSV row; fields containing commas,
quotes, or newlines are quoted using standard CSV escaping.

</details>

<details>
<summary>How do custom separators handle typed escapes?</summary>

The custom separator field understands `\n`, `\t`, `\r`, and `\\`, so you can type
`\n---\n` for a multi-line divider or `\t` for a tab.

</details>

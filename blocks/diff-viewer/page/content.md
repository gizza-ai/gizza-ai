## About this tool

The **diff viewer** turns a pasted unified diff into a readable review artifact. It accepts output from `git diff`, `diff -u`, or a `.patch` file and parses the patch into files, hunks, line numbers, additions, deletions, renames, binary-file markers, and file-level stats.

Choose the view that fits your workflow:

- **Inline** keeps the familiar unified diff shape, but adds a concise summary and per-file banners.
- **Side-by-side** aligns old and new lines in two text columns, so replacements are easy to scan.
- **Stats** produces a compact `git diff --stat` style table with plus/minus bars and totals.
- **JSON** returns the parsed structure for scripts, review bots, dashboards, or downstream analysis.

Enable **ignore whitespace-only changes** when a patch is mostly indentation or formatting noise. Matching delete/add pairs that are identical after whitespace normalization become unchanged context and stop counting toward the totals. Everything runs locally in the browser; your patch is not uploaded.

## FAQ

<details>
<summary>Does this compare two separate text files?</summary>

No. This tool views an already-computed unified diff — for example the output of `git diff`, `diff -u old new`, or a `.patch` file. Use a separate text-diff/comparison tool when you need to generate the diff from two inputs.

</details>

<details>
<summary>What diff formats are supported?</summary>

It supports common unified diff syntax: `diff --git` headers, `---`/`+++` file headers, `@@` hunk headers, context/add/delete lines, new and deleted files, renames, binary-file markers, and hunk section headings. It is intentionally lenient about metadata lines such as `index` and file modes.

</details>

<details>
<summary>How does the side-by-side view align replacements?</summary>

Within each hunk, consecutive deleted and added lines are paired row by row. Changed pairs use a `~` marker on both sides, pure deletions use `<` on the old side, pure additions use `>` on the new side, and context lines show the same content on both sides.

</details>

<details>
<summary>What does ignore whitespace-only changes do?</summary>

When enabled, a deleted line followed by an added line with the same words after whitespace normalization is folded into a single context line. That hides indentation-only churn from the rendered output and removes it from the add/delete totals. Real content changes are still shown.

</details>

<details>
<summary>Is the patch uploaded or stored?</summary>

No. The parser is compiled to WebAssembly and runs in your browser tab. The pasted diff is processed locally and disappears when you close the page.

</details>

## About this tool

The **HTML diff** compares two snippets by the text a reader would see, not by markup. It strips tags, attributes, classes, ids, styles, and other HTML noise, then diffs the visible text at line or word granularity.

Use it when a CMS, email editor, markdown renderer, sanitizer, or template system rewrites markup but you only care whether the rendered copy changed. A class rename, wrapper `<div>`, or inline style update disappears; changed words and visible lines remain.

Choose **line** granularity for patch-style output with context lines, or **word** granularity for prose-style inline markers. Use **JSON** when another script needs structured operations and counts. The ignore-case and ignore-whitespace options affect matching only; output keeps the original visible tokens.

## Worked example

Original HTML:

`<p class="lead">The quick brown fox.</p>`

Updated HTML:

`<div><span style="color:red">The slow brown fox.</span></div>`

With **word** granularity and **unified** output, the visible-text comparison ignores the wrapper, class, and style changes and reports the prose edit as `The [-quick-] {+slow+} brown fox.`.

## FAQ

<details>
<summary>Does this compare raw HTML source?</summary>

No. It intentionally ignores tag and attribute noise by converting each HTML snippet to visible text first. If you need to compare the actual markup tree, use a source/AST-oriented diff tool instead.

</details>

<details>
<summary>What output formats are available?</summary>

`unified` returns a patch-style diff for line granularity and a `wdiff`-style inline word diff for word granularity. `json` returns counts plus token operations for scripts or review bots.

</details>

<details>
<summary>When should I use word granularity?</summary>

Use word granularity for paragraphs, email copy, SEO snippets, and other prose where a full line replacement would hide the exact changed word. It marks removed words as `[-old-]` and added words as `{+new+}`.

</details>

<details>
<summary>What do ignore case and ignore whitespace change?</summary>

They affect matching only. If enabled, tokens that differ only by case or spacing are treated as equal, which helps suppress formatting churn. The output still echoes the original visible text where it appears.

</details>

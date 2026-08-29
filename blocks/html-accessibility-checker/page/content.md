## About this tool

Accessibility failures often leave a visible trace in the markup: an `<img>` without `alt`, an `<input>` without a `<label>`, a page that jumps from `h1` to `h3`, duplicate `id` values, an iframe with no title, a positive `tabindex`, or `aria-hidden` wrapped around a focusable link. Paste an HTML document or fragment and this checker reports those automatable issues with severity, a stable rule code, WCAG reference, line/column, affected element, and a concrete fix hint.

Use **WCAG level** to include A, AA, or AAA checks. AA is the default because it matches the most common audit target; AAA adds advisory checks such as generic link text. **Minimum severity** filters the report to suggestions, warnings, or errors. **Show passed checks** adds evidence for rules that ran and found no issue. **Maximum findings** caps the output at 1–5000 rows so very large pages remain readable. Results can be rendered as readable text, Markdown for issue trackers, JSON for scripts, or CSV for spreadsheets.

Worked example: paste `<html><body><h2>Welcome</h2><img src="hero.png"><form><input name="email"></form></body></html>`. The report flags the missing document language and title, the skipped heading level/no h1, the image without alt text, the unlabeled input, and the missing main landmark. Switch **Minimum severity** to `error` and the report keeps only blocking failures; switch **Output format** to `json` or `csv` to feed the same findings into automation.

Limits and edge cases: this is a fast lexical checker, not a browser, screen reader, CSS engine, colour-contrast calculator, keyboard-trap detector, or proof of WCAG conformance. It scans the markup as written and does not fetch external CSS/JS, execute scripts, compute accessible names from the rendered DOM, inspect images for meaningful alt text, or test focus order after layout. Script/style/textarea contents are skipped so `<` inside JavaScript is not treated as markup. Input is capped at 5,000,000 bytes and over-large pastes are rejected rather than truncated. Use this before, not instead of, axe-core, Lighthouse, manual keyboard testing, colour contrast checks, and screen-reader review.

## FAQ

<details>
<summary>Does this replace axe, Lighthouse or a manual accessibility audit?</summary>

No. It catches markup patterns that can be found without rendering a page: missing attributes, weak names, duplicate ids, heading structure, table headers, and similar static signals. It cannot judge colour contrast, visual focus order, keyboard traps, live-region behaviour, whether alt text is truly meaningful, or issues introduced by JavaScript after load. Treat it as a quick pre-flight check.

</details>

<details>
<summary>What does the score mean?</summary>

The score is a deterministic 0–100 roll-up over rules that actually ran. Errors count more than warnings, and warnings count more than suggestions. It is useful for comparing two snippets or tracking whether a template improved, but it is not a WCAG certification score.

</details>

<details>
<summary>Can I scan a fragment instead of a full document?</summary>

Yes. Fragments are supported and document-only checks such as missing `<html lang>` and `<title>` are skipped when the paste does not contain document structure. Element-level checks still run, so a fragment with `<img src="x.png">` or `<input name="email">` will still be flagged.

</details>

<details>
<summary>Why did it flag my table or image?</summary>

The checker sees only source markup. A data table without `<th>` cells is reported unless it is explicitly marked as presentation. An image with missing, empty, filename-like, or generic alt text is reported because the code cannot know whether the surrounding layout makes it decorative. Review the finding and either fix the markup or document why the pattern is intentional.

</details>

<details>
<summary>Which output format should I use?</summary>

Use text while editing, Markdown for a pull-request comment or issue, JSON for automation that wants counts and structured issue objects, and CSV when you want rows in a spreadsheet. JSON includes the score, mode, selected level, per-severity counts, issues, and optionally passed checks.

</details>

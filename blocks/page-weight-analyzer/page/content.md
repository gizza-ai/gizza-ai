## What this tool does

Paste a page's HTML source and get an instant front-end performance snapshot:
how many scripts and stylesheets it loads, which of them block rendering, how
big the inline JavaScript and CSS are, how many network requests the page is
likely to make, and a rough page-weight budget. Everything runs locally in your
browser — nothing is uploaded, it works offline, and there's no sign-up.

It's a quick first-pass audit for spotting render-blocking resources, bloated
inline code, and pages that load far too many files — without opening DevTools.

## What it reports

| Section | What you learn |
| --- | --- |
| **Scripts** | External vs inline count; how many external scripts are **parser-blocking** (classic scripts with no `async`/`defer`); how many use `async`, `defer`, or `type="module"`; how many inline scripts run synchronously; and the total inline JS size. |
| **Stylesheets** | External `<link>` vs inline `<style>` count; how many stylesheets are **render-blocking** (print-only and `disabled` sheets are excluded); and the total inline CSS size. |
| **Other resources** | Images (and how many are `loading="lazy"`), iframes, audio/video, and resource hints (`preload`, `prefetch`, `preconnect`, `dns-prefetch`). |
| **Estimated requests** | A lower-bound count of network requests: the HTML document plus its external scripts, stylesheets, images, iframes, media, and font preloads. |
| **Estimated weight** | The HTML's exact byte size plus a rough estimate of the external resources, compared against a common performance budget. |

## How the weight estimate works

The size of the pasted HTML is **measured exactly**. The tool can't download the
external files (it never makes a network request), so their sizes are
**estimated** from typical median transfer sizes — about 30 KB per script, 16 KB
per stylesheet, 30 KB per image, 25 KB per font, and 60 KB per iframe document.
The real numbers depend on your actual files, so treat the external estimate as a
ballpark for catching obviously heavy pages, not an exact measurement. The
request count is also a lower bound: assets that CSS or JavaScript fetch at
runtime can't be seen in static HTML.

## Render-blocking, briefly

A **parser-blocking script** is a classic `<script src="...">` with no `async` or
`defer` — the browser stops building the page until it downloads and runs. Adding
`defer` (or `type="module"`, which defers by default) lets parsing continue. A
**render-blocking stylesheet** is a normal `<link rel="stylesheet">` — the
browser won't paint until it loads. Inlining critical CSS and deferring the rest
removes the block. The report flags both and suggests the usual fixes.

## Options

| Option | What it does |
| --- | --- |
| **Output format — report** (default) | A readable text summary with sections and recommendations. |
| **Output format — json** | A machine-readable object with every count and the estimate, handy for scripting or feeding another tool. |
| **List every external resource URL** | Adds a grouped list of every script, stylesheet, image, font, iframe, and media URL found in the markup. |

## Examples

Paste a `<head>` like this:

```
<link rel="stylesheet" href="/main.css">
<link rel="stylesheet" href="/print.css" media="print">
<script src="/analytics.js"></script>
<script src="/app.js" defer></script>
```

…and the report shows **2 stylesheets (1 render-blocking** — `print.css` is
print-only, so it doesn't block) and **2 external scripts (1 parser-blocking** —
`analytics.js` has no `defer`; `app.js` does), along with a note to add `defer`
or `async` to the blocking one.

## FAQ

<details>
<summary>Is it free and private?</summary>

Yes. The HTML you paste never leaves your device, and the tool keeps working
offline once the page has loaded.

</details>

<details>
<summary>Why are the external file sizes only estimates?</summary>

The tool runs entirely in your browser and never makes network requests, so it
can't download the linked scripts, styles, images, or fonts to measure them. It
estimates their sizes from typical median transfer sizes. The HTML you paste is
measured exactly.

</details>

<details>
<summary>What counts as a "render-blocking" stylesheet?</summary>

A normal <code>&lt;link rel="stylesheet"&gt;</code> with no media restriction.
Sheets marked <code>media="print"</code> or <code>disabled</code> are excluded
because they don't block the first paint.

</details>

<details>
<summary>Why is the request count a "lower bound"?</summary>

Static HTML only shows the resources written directly in the markup. Files
that CSS (background images, <code>@import</code>, web fonts) or JavaScript fetch
at runtime aren't visible, so the real request count is usually higher.

</details>

<details>
<summary>Can I get the results as JSON?</summary>

Yes. Switch the output format to "json" for a structured object containing
every count and the estimate, ready to script against.

</details>

<details>
<summary>Do data: URIs count as requests?</summary>

No. Inline <code>data:</code> URIs are embedded in the HTML itself, so they add
to the document's measured size but aren't counted as separate network
requests.

</details>

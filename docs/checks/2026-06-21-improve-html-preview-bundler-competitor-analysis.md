# html-preview-bundler — competitor analysis & differentiation

**Tool:** `gizza-ai/html-preview-bundler` — combine separate HTML, CSS, and JS
into one self-contained runnable HTML file.
**Date:** 2026-06-21

## What's out there

| Competitor | Form | Notes / gaps |
|---|---|---|
| CodePen / JSFiddle "Export" | Web | Great editors, but require an account/site and export a zip; you're working in their UI. |
| `html-inline` / `inliner` (npm) | Library/CLI | Inline external assets, but need Node + the files on disk; aimed at inlining `<link>`/`<script src>`, not three pasted blobs. |
| Manual copy-paste into a template | DIY | Tedious and error-prone (head vs body placement, wrapping a fragment). |
| Online "merge html css js" tools | Web | Exist, but typically upload your code to a server. |

## How gizza's tool is better / different

1. **Local — your code never uploaded.** Runs in WASM (chat SW + CLI + page).
2. **Smart placement.** A **full document** gets CSS injected before `</head>`
   and JS before `</body>`; a **fragment** is wrapped in a clean HTML5 page
   (charset, viewport, your `<title>`). You don't think about where things go.
3. **Three pasted blobs → one file.** Exactly the CodePen-trio → portable-file
   workflow, without an account or export step.
4. **Self-contained output.** Everything inlined, so the result opens offline and
   shares as a single `.html`.
5. **Three surfaces, one Rust core**, dependency-free.

## Verification

Six core unit tests cover fragment wrapping, full-document injection (CSS before
`</head>`, JS before `</body>`), css-only/js-only, `<title>` escaping, the
all-empty error, and the missing-`</head>` fallback. **End-to-end CLI** bundled
`<h1>Hi</h1>` + `h1{color:red}` + `console.log(1)` into a valid HTML5 document
with the style in `<head>` and the script before `</body>`. Page Playwright
covers fragment-wrap and full-doc-inject.

## Scope / honest limitations

- Inlines the three blobs you provide; it does not fetch/inline external
  `<link>`/`<img>`/`<script src>` assets (that's `html-inline`'s job).
- CSS/JS are inlined verbatim — a literal `</script>` inside the JS would close
  the tag early (documented on the page).

## Possible future enhancements

- Optional `data:` URL output (one click to open).
- Fetch-and-inline external assets referenced by the HTML.
- Optional minification of the bundled result (pairs with html-minifier).

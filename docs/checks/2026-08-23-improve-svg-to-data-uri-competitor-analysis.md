# svg-to-data-uri — competitor analysis (2026-08-23)

Scan run BEFORE implementing. All findings below are **paraphrased observations** of what the
competitor tools expose to a user — no copy, markup, branding, or assets were taken from them.

## Competitors skimmed

| # | Tool | Angle |
|---|------|-------|
| 1 | SVG Encoder (svgencoder.com) | The most feature-complete: URL-encode ⊕ base64, six snippet shapes, minify toggle, live size comparison, decode mode |
| 2 | MagicPattern "SVG to Base64" | Design-tool audience: paste or upload, base64/URL toggle, optimize checkbox, live preview, copy button |
| 3 | base64.dev "SVG to Base64" | Base64-first, drag-and-drop file only, raw-payload toggle, CSS `url()` snippet, copy button |

Also noted from the search-result surface (not deep-read): base64.guru, iotools.cloud,
allsvgicons.com, fffuel eeencode, svgencode.com — all converge on the same core shape.

## Table stakes (what every one of them ships)

- **Paste SVG markup** as the primary input; file upload as a convenience.
- **Two encodings**: percent/URL-encoding and base64. The market consensus is that **URL-encoding
  wins for SVG** (base64 costs ~33% overhead on text-shaped markup), and the better tools default
  to it and/or label which is smaller.
- **Ready-to-paste snippets**, not just a bare URI: CSS `background-image: url(...)`, an HTML
  `<img src>` tag. The strongest competitor adds `mask-image`, JSX, favicon `<link>` and `<object>`.
- **Minify-before-encode toggle** — strip the XML declaration, DOCTYPE, comments and redundant
  whitespace so the URI is shorter. On by default or prominently offered.
- **Size comparison** — show URL-encoded vs base64 length and flag the smaller one.
- **Copy button**, live/as-you-type result, and an explicit "runs in your browser, nothing is
  uploaded" claim.
- **Educational copy**: base64-vs-URL-encoding trade-off, why the URI sometimes doesn't render,
  decoding back to SVG, and "inline small icons, not big illustrations".

## Defaults observed

| Option | Competitor default | Our choice | Why |
|---|---|---|---|
| encoding | URL-encode (svgencoder) / base64 (base64.dev) | **`url`** | Smaller for SVG in nearly every real case; matches the strongest competitor |
| minify | on | **`true`** | Free size win, no visual change |
| output snippet | raw data URI | **`uri`** | Least surprising; the snippet shapes are one select away |
| xmlns injection | implicit/automatic | **`true`** | A data-URI SVG without `xmlns` silently fails to render in `url()` — the single most common "why is it blank" report |

## Examples competitors lead with

A small single-path icon (a circle, a checkmark, an arrow) — never a large illustration. Their
worked examples always show the input markup and the resulting `background-image` line together.
Our page mirrors that shape with an original example.

## UX patterns worth adopting (in-model)

- **Which-is-smaller feedback.** Adopted as an explicit `output = "compare"` choice that prints
  both encoded lengths, the delta, and the winner — declarative, no bespoke JS.
- **Snippet selector.** Adopted as `Param::enumv` → a native `<select>` (`uri`, `css`, `mask`,
  `img`, `jsx`, `compare`).
- **Minify + xmlns as checkboxes.** Adopted (`minify`, `add_xmlns`, both default on).
- **Quote handling.** Adopted as `quotes` (`single` | `encode`). URL-encoded SVG has to live
  inside `url("…")`, so the embedded attribute `"` has to go somewhere — competitors silently
  rewrite them to `'`; we make it a visible, documented choice.
- **Preset chips.** Adopted as `[[example]]` blocks, the platform's declarative preset answer.
- **Copy button / reset / live run.** Already provided by the shared page runtime.

## In-model decisions (built)

- SVG-aware **minimal** percent-encoding (`% # < > ? [ \ ] ^ \` { | }` + CR/LF), not RFC 3986
  unreserved-only — this is what makes the URL form actually smaller than base64. This is the
  core reason this tool is distinct from `data-uri-encode`, which does the conservative
  unreserved-only escaping and has no SVG knowledge.
- Whitespace collapsing between tags and runs of spaces (minify).
- `xmlns` injection when the root `<svg>` lacks it.
- Both encoded lengths always computed, so `compare` and the structured chat/CLI result can
  report the winner.
- Real validation errors that name what was expected (no root `<svg>`, empty input, over the
  size cap).

## Out-of-model (considered, NOT built — reason)

- **Drag-and-drop `.svg` file upload on the page.** The pure-tool page renders field inputs only;
  the file-upload control belongs to the ffmpeg runtime path. Pasting markup covers the same job,
  and `file-to-data-uri` already handles arbitrary file bytes.
- **Live rendered preview of the SVG.** Would mean injecting untrusted markup into the page DOM;
  declined on safety grounds, and it is not expressible declaratively today.
- **Decode mode (data URI → SVG).** Genuinely a second IO shape; `data-uri-decode` already ships
  it. Cross-linked from our page copy instead.
- **Full SVG optimization** (path re-writing, precision reduction, attribute merging). That is
  `svg-optimize`'s job; duplicating it here would be a redundant tool. Cross-linked instead.
- **Favicon `<link>` / `<object>` snippet shapes.** Rejected on judgment: marginal over `img`,
  and each extra enum value is schema surface an LLM has to disambiguate.

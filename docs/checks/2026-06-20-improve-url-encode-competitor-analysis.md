# improve-tool — url-encode competitor analysis (2026-06-20)

Research record for the `/improve-tool url-encode` run. All competitor notes are
**paraphrased** — no competitor copy, branding, or trademarks were copied into our
tool. Captured via web search + per-competitor fetch.

## Competitors profiled (top 5)

| # | Tool | Distinctive capabilities (paraphrased) | UX patterns |
|---|------|----------------------------------------|-------------|
| 1 | urlencoder.org | charset selection (UTF-8 default, ~40 encodings); **encode each line separately**; 76-char MIME chunking; live client-side mode; binary file upload (to ~100 MB) | copy button, live toggle, status messages |
| 2 | urldecoder.org | **decode each line separately** (strips stray whitespace first); **decode recursively** until stable or 16 rounds; charset pulldown | copy, paste-and-go |
| 3 | freeformatter.com | encode/decode toggle; RFC 1738; charset dropdown | copy, sample, clear |
| 4 | jam.dev | one-step encode/decode; query-string & key=value handling; client-side; ad-free | copy, single-field |
| 5 | 10xtools.io | RFC 3986; UTF-8/emoji; **real-time** conversion; **`+` vs `%20` for spaces** (form vs component); bulk/multiple URLs; **visual highlighting**; **swap** button | live, swap, highlight, copy |
| (browserling.com) | simple encode/decode, live, copy — minimal feature set | press-a-button |

## Gap analysis vs our tool (before this change)

Our tool was: `text`, `mode` (encode/decode), `target` (component/uri); pure, UTF-8,
single value, live page, RFC 3986. Gaps, tagged for fit to gizza's browser-local/wasm/
no-account/no-server model:

### Capabilities — IN-MODEL (built this run)
- **`form` / `+`-for-space encoding** (`application/x-www-form-urlencoded`) — 10xtools,
  jam, freeformatter all handle query/form style; we couldn't encode a space as `+`. Built
  as `target=form` (and `+`→space on decode).
- **Per-line / batch conversion** — urlencoder.org & urldecoder.org "encode/decode each line
  separately"; 10xtools "bulk". Built as `per_line` + a multiline page textarea.
- **Recursive / repeated decode** — urldecoder.org "decode recursively up to 16 times". Built
  as `repeat` (1–16), for un-nesting double-encoded input (and double-encoding).

### Copy/SEO — IN-MODEL (built this run)
- Competitors rank with explainers (reserved vs unreserved, `%20` vs `+`, double-encoding,
  query-string examples). Rewrote `content.md` to cover the new modes + examples + FAQ, and
  broadened `meta.toml` title/description/tags. **Original copy only.**

### UX/layout — IN-MODEL (partial; root-cause infra fix)
- Per-line batch needs a multi-line input. The page generator only rendered single-line
  `<input>`. Added an additive, backward-compatible `multiline` textarea option to the shared
  generator (`tools/generator`) and marked url-encode's text field multiline. Other per-tool
  UX (swap, highlight, copy button) is governed by the shared `gizza-chrome`/`tool.js` and is
  out of scope for a single-tool change.

### Visual design
- Styling is the shared `gizza-chrome`; no tool-specific visual gap.

## Out-of-model — considered, NOT built
- **Non-UTF-8 charset selection** (ISO-8859, Windows-1252, CJK): modern URLs are UTF-8; adds a
  large transcoding dependency for a legacy edge case. Low value vs cost.
- **Binary file upload / encode bytes (to ~100 MB):** a different use case from this text tool;
  large in-browser binary handling is heavy.
- **76-char MIME chunking:** niche, low value.

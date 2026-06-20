# file-to-data-uri — competitor analysis & improvements (2026-06-20)

**Tool:** `gizza-ai/file-to-data-uri` — encode a (small) file as a self-contained
base64 `data:<mime>;base64,…` URI for inline embedding. Chat + CLI (no page: a
File input with a text output has no page surface in this framework — the
file-input page path only exists for the ffmpeg runtime; the F3 no-page
file-input pattern, like file-hash / detect-file-type).

## What competitors do

- **Online "image/file to data URI / base64" sites** (base64-image.de,
  dopiaza data URI generator, css-tricks converter, b64.io) — upload a file, get
  a data: URI or a ready CSS snippet. Strengths: some emit CSS/`<img>` snippets.
  Weaknesses: the file is **uploaded** (privacy), several only do images, and a
  few emit raw base64 without the `data:<mime>;base64,` prefix (not directly
  usable inline).
- **CLI one-liner** — `base64 file | ...` then hand-prepend `data:mime;base64,`;
  works but is fiddly and you must know/set the MIME yourself.
- **Build tools** (webpack/vite asset inlining) — automatic but only inside a
  bundler pipeline, not ad hoc.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust compiled to wasm, runs in the
   chat Service Worker and headless via the CLI. The file never leaves the device.
2. **Correct MIME, automatically.** The content type comes from the fetched file
   (or attachment), so the URI is immediately valid — no manual `data:image/png`
   guessing. An optional `mime` override is there when you need it.
3. **Emits a real `data:` URI**, not bare base64 — paste straight into CSS
   `url(...)`, an `<img src>`, an email, or a JSON payload.
4. **Chainable & informative.** Takes a `url` or a prior tool's `ref`, and reports
   the MIME, source byte size, and final URI length so you can judge whether the
   asset is small enough to inline.
5. **Sane size cap.** Limits input to ~4 MiB — data URIs grow ~33% over the raw
   bytes and are meant for small assets (icons, fonts, tiny images); the cap keeps
   the output usable instead of producing a megabytes-long unusable string.

## Honest scope

- Intended for **small** assets. There's no chunking/streaming; very large files
  are rejected by the cap (inlining them would bloat the host document anyway).
- Does not generate a CSS/HTML snippet wrapper — it returns the raw `data:` URI
  (which drops directly into either).

## Tests

4 core unit tests: known base64 vector (`"hi"`→`data:text/plain;base64,aGk=`),
empty/whitespace MIME → `application/octet-stream` default, MIME trimming, and
empty bytes → empty payload. Plus the block drift-guard schema test. CLI verified
over the wire on `tux.png`: MIME auto-detected as `image/png`, and the URI's
base64 payload **decodes back to the exact original 7666 bytes** (PNG magic
intact); the `mime` override (`image/webp`) is honored in the emitted prefix.

# pdf-compress — competitor analysis (2026-06-20)

Second `/create-next-tool` backlog pick (after the picker skipped the
image-to-webp / jpg-compressor dups). Document-input tool (PDF in → PDF out),
chat + CLI only — no standalone page (a page can't fetch an arbitrary PDF).
Research via `WebSearch` (no firecrawl in this env). All findings **paraphrased**.

## Competitors surveyed

| tool | approach (paraphrased) | dimension |
| ---- | ---------------------- | --------- |
| SaferPDF | Ghostscript-compiled-to-WASM, fully in-browser, offline after first load | capabilities |
| PDF24 | several compression modes incl. a lossless mode; no install | capabilities |
| ConverterDev | lossless text + lossy image recompression; selectable quality level | capabilities |
| SmallPDF / Adobe | quick browser compression (mostly server-backed for the heavy tiers) | — |

## Gap diff vs our tool

Our tool: parse → prune unreachable objects → Flate-compress every stream that
isn't already compressed (content + image XObjects) → renumber → re-serialize.
**Lossless** (text, vectors, and already-compressed image data preserved exactly),
and it never returns a file larger than the input (falls back to the original).

**In-model gaps considered, deferred (fit gizza's pure-Rust/wasm model but are
large; good standalone `/improve-tool` follow-ups, not built this pass to keep
loop throughput up):**
- **Lossy image downsampling / re-encoding** — decode embedded JPEG/Flate images
  with the `image` crate and re-encode at a chosen quality + max DPI. This is the
  single biggest real-world size win (it's what the Ghostscript tools do) and is
  doable in pure Rust, but it's a substantial feature with quality/coverage risk
  (many image filters, color spaces) — deserves its own focused pass with its own
  test corpus.
- **Compression level param** — only meaningful once image downsampling exists
  (lossless stream compression has no "level" knob worth exposing).
- **Metadata stripping** (Info dict + XMP) — small, safe size win; a quick future add.

**Out-of-model (not applicable to browser-local pure-wasm):**
- Bundling Ghostscript-WASM — multi-MB engine, far outside the per-tool wasm
  budget; gizza tools are small single-purpose blocks.
- Server-side compression tiers, accounts, file retention.

## Tested
unit (3: shrinks-a-bulky-pdf, idempotent-on-compressed, rejects-non-pdf) +
drift-guard (Input::Document url⊕ref schema) · `wafer build` validates the block ·
CLI end-to-end on two real public PDFs (dummy.pdf 13264→13122, sample.pdf
18810→18407, output re-parses as valid PDF). No page surface (Document tool).
Note: the network client blocks localhost/private IPs (SSRF guard), so CLI
verification used public PDFs rather than a local fixture server.

> Original work only — no competitor copy, branding, or trademarks copied.

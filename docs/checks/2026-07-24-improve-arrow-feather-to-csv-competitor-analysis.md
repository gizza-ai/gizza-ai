# arrow-feather-to-csv — competitor analysis (2026-07-24)

Snapshot taken while improving the `arrow-feather-to-csv` block (convert an Apache
Arrow IPC / Feather V2 file to CSV). All competitor notes are **paraphrased** —
no copy, branding, or trademarks are reproduced. Features are analysed for ideas
only; every gap we closed was reimplemented originally.

Our tool at the time of review: reads Arrow IPC **file and stream** formats plus
Feather V2 (via `url` or `ref`), decodes LZ4 buffers, rejects legacy Feather V1
and ZSTD-compressed buffers with actionable messages, caps input at 8 MiB.
Output params: `delimiter` (single char or `"tab"`, default comma), `header`
(bool, default true), `null` text (default empty). RFC-4180 quoting, `\n`
terminators, RFC-3339 timestamps. Surfaces: chat + CLI (binary input → **no
web page**, like `xlsx-to-csv` / `web-fetch`).

## Competitors reviewed

1. **CSV Tools — Feather to CSV** (csvtools.com) — nearest twin. Browser-local,
   accepts `.feather`/`.arrow`/`.ipc` (V2 only). Exposes **no** output options.
   Drag-drop upload only. Documents LZ4/ZSTD in an FAQ but punts on ZSTD
   ("re-save uncompressed") — same posture as us. Strong "data never leaves your
   device" privacy pitch and an FAQ-driven SEO page.
2. **Table.Studio — Arrow to CSV** (table.studio) — breadth play: CSV, Excel,
   JSON/NDJSON, Parquet, Arrow, Avro, PDF, images. No output controls surfaced.
   Upload + URL + paste. "No sign-up"; ~20 cross-linked "Arrow to X" pages for
   programmatic SEO; educational copy about nested-type loss on CSV export.
3. **ParquetReader — Feather to CSV** (parquetreader.com) — explorer angle.
   Export to CSV/JSON/Parquet with an **in-browser SQL** layer to filter columns
   and rows before export, plus a schema preview. Upload only.
4. **Reparatio — Feather to CSV** (reparatio.app) — **server-side**, freemium.
   Tiered size caps (10 MB free / 500 MB / 2 GB) and a row gate (100 rows free).
   REST API, CLI, SDKs, MCP server on paid tiers. Schema/column-type preview.
5. **pyarrow / pandas recipe** (arrow.apache.org) — the power-user baseline.
   Full fidelity: Feather V1 & V2, IPC file + stream, Parquet, LZ4 **and** ZSTD,
   and complete `write_csv` control (delimiter, quoting, null string, header,
   batch size, encoding, column selection). Requires writing Python. Honourable
   mention: `clickhouse-local` as the true single-command CLI rival.

## Where we already win

- Among the browser-local tools, we are the only one (besides CSV Tools) that
  exposes **real output controls** (delimiter, header, null) — Table.Studio,
  ParquetReader and Reparatio surface essentially none. Lead with this.
- Documented, standards-clean output (RFC-4180 quoting, RFC-3339 timestamps).
- No accounts, no server, no row gate (vs. Reparatio's 100-row wall).

## Gaps closed in this pass (in-model)

- **Column selection / reorder (`columns`).** Comma-separated column names or
  0-based indices, kept in the order given. Captures ~80% of ParquetReader's SQL
  value and pyarrow's column-select, in-model via `RecordBatch::project` (no new
  dep). Unknown names/indices error with the available column list.
- **Row limit (`limit`).** Cap the number of data rows for a quick preview of a
  large table (0 = all). In-model via `RecordBatch::slice` across batches.

## Considered, not built (with reason)

- **ZSTD buffer decode** — highest-value on paper, but **out-of-model here**:
  `arrow-ipc`'s `zstd` feature pulls the C `zstd` crate, which does not build for
  `wasm32-wasip1`, and there is no pure-Rust ZSTD path *through the Arrow IPC
  reader* (a standalone `ruzstd` can't be injected into the reader's buffer
  decompression). We continue to reject ZSTD with a clear "re-save uncompressed
  or with LZ4" message — the same honest posture CSV Tools takes.
- **CRLF line terminator / always-quote mode** — `arrow-csv`'s `WriterBuilder`
  exposes neither a line terminator nor a quoting-style switch; post-processing
  `\n`→`\r\n` would corrupt quoted fields that legitimately contain `\n`. Left
  out rather than shipped unsafely.
- **UTF-8 BOM toggle** — minor Excel nicety; declined to avoid param bloat on a
  tool that already has five parameters.
- **Feather V1 read** — a bespoke pure-Rust V1 reader is disproportionate for a
  legacy format; we keep the explicit "re-save as V2" message.
- **Raising the 8 MiB cap** — the wasm sandbox OOM-traps on multi-MiB buffer
  growth (Arrow decode + CSV text is several × the input); the conservative cap
  stays.

## Considered, out-of-model (require a backend / accounts)

- Server-side large-file processing and 500 MB–2 GB tiers (Reparatio).
- Accounts, freemium row gates, email capture.
- Hosted REST API / SDK / MCP server (Reparatio).
- In-browser SQL query engine (ParquetReader) — the `columns` + `limit` params
  capture most of the practical value in-model.
- Multi-format input matrix (Table.Studio: Excel/PDF/Avro/images) — scope creep;
  this is a focused Arrow/Feather tool.

## Verification

`cargo test --workspace` (22 tests: core happy/error/columns/limit + drift-guard
+ Args), `scripts/build-block-wasm.sh`, `wasm-pack` (n/a — no web page),
`cargo install --path cli`, `sync-tool-manifest.py`, generator render,
`check-tool-hygiene.py`, and a CLI exact-output case. Chat surface is verified
only via the descriptor/drift-guard here (the live chat UI lives in the private
site repo). No web page — binary Arrow input is not a page-renderable surface.

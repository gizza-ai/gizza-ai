# qr-batch — competitor analysis (2026-08-29)

Scope: bulk / batch QR-code generators that take a pasted list or a CSV and hand back many
QR files at once. Findings below are **paraphrased observations** of publicly visible tool
behaviour — no competitor copy, branding, wording or assets were reused. Original copy only.

## Tools surveyed

| # | Tool (function) | Why it was picked |
|---|---|---|
| 1 | QuickChart — bulk QR generator | Widely-linked, CSV upload + paste, ZIP download, API-backed |
| 2 | QRExplore — bulk QR generator | Deepest option set (sequence generation, rotation, captions) |
| 3 | Omnvert — bulk QR (CSV/list → ZIP) | Closest shape to ours: browser-local, list *or* CSV, ZIP + index |
| 4 | EZQR — bulk QR from spreadsheet | Column mapping (link / filename / caption), on-device processing |
| 5 | GenQRCode / QR Planet (batch) | Account-gated volume tiers, multi-format (PNG/SVG/EPS) exports |

## Table stakes observed (params · defaults · UX)

**Input**
- Paste a list, one entry per line — universal.
- Upload a CSV/TSV/TXT file — universal.
- A two-column form where one column is the *filename* and the other is the *payload*
  (comma or tab separated); rows with a single column fall back to auto-numbered names.
- A header-row toggle so a spreadsheet export's first line isn't encoded as a QR.
- One tool also generates a numeric *sequence* (start / end / step, zero-padding, prefix +
  suffix) instead of a pasted list.

**Per-code options**
- Error correction: L / M / Q / H, with **M** the common default (≈15 % recovery).
- Output size in pixels for raster output — 512 px is a typical shown value; 1024 px for print.
- Quiet-zone / border in modules — the spec minimum is 4; one tool exposes it as "blocks"
  and warns against going below 2.
- Foreground / background colour, including a transparent background option.
- Output format: PNG, SVG, or **both** in the same archive. EPS/JPG appear on two tools.

**Output / packaging**
- A single ZIP containing every code — universal.
- An `index.csv` (or equivalent) mapping each produced filename back to its encoded value,
  so a batch can be audited/re-joined against the source spreadsheet.
- Failed rows surfaced rather than silently dropped (one tool puts them in an `_errors/`
  folder inside the ZIP with the reason).
- Deterministic zero-padded auto names (`qr-001`, `qr-002`, …) so archive order matches
  source order.

**Limits / UX**
- Free-tier row caps cluster at 100–500 per batch (higher tiers/accounts/API keys go to
  1 000–5 000). Progress counters shown for long batches.
- Live preview of the first row's code while options are being tuned.
- Presets / example loaders ("load a sample file").
- Collapsible "advanced options" so the default path stays two fields.

## Decisions for this build

### In-model — built
- **Pasted list *and* delimited two-column input** (`data` + `input_format = auto|list|csv|tsv`).
  Auto-sniffs comma vs tab vs plain list. This is the whole point of the tool.
- **Column-order control** (`columns = auto|name-value|value-name|value-only`) — competitors
  disagree on which column is the filename, so make it explicit instead of guessing wrong.
- **Header-row skip** (`has_header`, default off).
- **Error correction** L/M/Q/H, default **M** — matches the common default and the sibling
  `qr-code-generator` block, keeping the family consistent.
- **PNG / SVG / both** output (`format`), size in px (`size`, default 512) and quiet-zone
  modules (`margin`, default 4).
- **Foreground / background colour**, `transparent` accepted for the background.
- **Auto-numbered zero-padded filenames** with a configurable `name_prefix` (default `qr`)
  → `qr-001.png`; custom names sanitised, de-duplicated (`-2`, `-3`) and order-preserving.
- **`index.csv` manifest** inside the ZIP (`include_index`, default on) mapping
  filename → encoded value, plus a `status`/error column so failed rows are visible.
- **Failed rows are reported, never silently dropped** — they appear in `index.csv` with the
  reason and are summarised in the tool's text output.
- **Stated caps on the page**: 500 rows per batch, 4 096 characters per payload, 32 MiB ZIP.

### In-model — considered, rejected (with reasons)
- **Caption text rendered under each code.** In-model for SVG (a `<text>` element) but PNG
  would need an embedded bitmap/TTF font in the wasm; shipping a feature that silently works
  in one of the two output formats is worse than not shipping it. `index.csv` already gives
  the label↔code mapping for print workflows.
- **Numeric sequence generation** (start/end/step + zero-pad). It is a *list producer*, not a
  QR feature — the same result is one line of shell or a spreadsheet fill, and it would add
  five params to a schema whose primary input is already "paste your list".
- **Centre logo overlay.** Already shipped by the sibling `qr-styled` block; duplicating it
  here would fork the styling surface across two tools.
- **Live first-row preview.** The page recomputes the whole batch on every input change
  already; a separate preview widget would need per-tool JS beyond the declarative controls.

### Out-of-model (cannot run browser-local, no-account) — not built
- Dynamic/editable QR codes (need a redirect service + hosted short links + an account).
- Scan analytics / tracking dashboards.
- Server-side batches of 1 000–5 000 codes behind an API key or paid tier.
- EPS output (a PostScript writer for a format PNG+SVG already covers for print).
- Cloud storage of past batches / re-download history.

## Differentiators we keep
- Runs entirely in the browser (and in the CLI) with no account, no upload, no row quota tied
  to a paid tier — the 500-row cap is a memory guard, not a paywall.
- The same engine is reachable from the CLI and from chat, so a batch is scriptable.
- Deterministic output: the same input and options always produce a byte-identical archive,
  which makes it diff-able in a build pipeline.

# csv-to-xlsx — competitor analysis (2026-07-07)

Tool: convert CSV / TSV / JSON table text into a real binary Excel `.xlsx` workbook.
Type: **pure** (text in → binary `.xlsx` out). Surfaces: chat (download envelope), CLI, standalone page
(custom.js renders a Download button + decodes/streams the base64 workbook).

## Scan

Searched "convert CSV to XLSX Excel online tool free" and reviewed the top real converters
(paraphrased — no copy/branding reproduced):

- **csvtoexcel.net** — zero-config, in-browser (local) conversion. Advertises auto-detection of the
  delimiter (comma, semicolon, tab, pipe, custom), encoding auto-detection (UTF-8/GBK/Big5/Shift_JIS/
  ISO-8859/Windows-1252 → UTF-8 output), and "auto-adjusted column widths". No exposed toggles.
- **freeconvert.com/csv-to-xlsx** — upload/convert with a generic "Advanced settings"; max file size
  ~1 GB (paid tiers larger); batch conversion of multiple files. Specific params not documented on the
  landing page.
- **coolutils / zamzar / tinywow / testmu** — upload-and-convert converters; emphasize "automatic"
  delimiter + structure handling and no-install/no-registration. Server-side conversion.

## Table stakes → decision

| Capability (competitor) | Fit | Where it lands |
| --- | --- | --- |
| Delimiter selection / auto-detect (comma, tab, semicolon, pipe) | in-model | `input_format` enum (`auto` sniffs most-frequent of `,` `\t` `;` `\|`; explicit `csv/tsv/semicolon/pipe`) |
| JSON input (tool brief says "CSV/JSON") | in-model | `input_format` `json` + `ndjson`; array-of-objects unions keys |
| Custom sheet/tab name | in-model | `sheet_name` param (sanitized to Excel's rules; also the download filename) |
| Header row handling | in-model | `header` bool → bold **and frozen** top row; JSON keys become the header |
| Type detection (numbers stay numeric; leading zeros preserved) | in-model | `detect_types` bool → native number/boolean cells; `007`/`+1` stay text; JSON keeps its own types |
| Auto-adjusted column widths | in-model | `autofit` bool → `Worksheet::autofit()` |
| Encoding auto-detection (GBK/Big5/Shift_JIS/…) | **out-of-model / N.A.** | Input on every surface is already-decoded UTF-8 **text** (the browser/CLI decodes the file), so there is no raw byte stream to sniff. Listed, not built. |
| Batch / multiple files, ~1 GB uploads | **out-of-model** | Single text input; workbook capped at 24 MB (in-browser/inline transport). Listed, not built. |
| Multi-sheet workbooks (one sheet per input tab) | **out-of-model (future)** | One worksheet per run. |
| Formulas / styling / charts / cell colors | **out-of-model** | Plain data workbook only (bold+frozen header is the one formatting touch). |

Every table-stake either ships in the descriptor or is on the out-of-model list above — none dropped
silently.

## UX control patterns adopted

- `input_format` renders as a `<select>` with friendly `[input.labels]` (Auto-detect / CSV — comma / …).
- Three `[[example]]` preset chips (People CSV, Sales JSON, Cities semicolon) — competitors ship
  one-click samples; these double as the page's worked examples.
- `data` is a multiline `<textarea>` with a multi-line placeholder showing the expected shape.
- Because the output is a binary `.xlsx`, `page/custom.js` intercepts the result and renders a real
  **Download .xlsx** button (reusing the shared `#tool-output-download` anchor) plus a size summary,
  instead of dumping the base64 `data:` URL as text. Empty input shows a neutral idle prompt.

## Correctness verification (not just "something rendered")

- Core round-trips through `calamine` in unit tests (native number/boolean cells, leading-zero text,
  JSON key-union, semicolon auto-detect, sheet-name sanitize).
- CLI matrix (auto/csv/tsv/pipe/ndjson, header off, detect-types off) — each produced workbook was
  unzipped and asserted: `detect_types` on → `<v>30</v>` numeric in the sheet and 30 absent from the
  shared-string table; off → 30 present as a shared string (text).
- Playwright decodes the page's download in-browser (ZIP magic + `DecompressionStream` inflate of
  `xl/sharedStrings.xml` / `xl/worksheets/sheet1.xml`) and asserts the actual cell text and numeric
  cells, for a CSV run, a JSON `?data=…` deep-link, and a non-default `detect_types` toggle.

## Note (build-level finding — worth recording)

`rust_xlsxwriter` (0.79) stamps a file-creation timestamp via `SystemTime::now()`. That works under
`wafer build` (wasm32-wasip1 — wafer supplies the clock) but **panics on wasm32-unknown-unknown** (no
std clock). The crate's `wasm` feature switches that call to `js_sys::Date::now()`; enabling it **only**
for the wasm32 web build (`[target.'cfg(target_arch = "wasm32")'.dependencies]` in `web/Cargo.toml`)
keeps the chat/wafer build on the SystemTime path and fixes the browser build. (Added to
`.claude/skills/create-next-tool/references/wasm-crates.md`.)

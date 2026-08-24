# toml-to-csv — competitor analysis (2026-08-20)

Scan run **before** implementation, so the shipped descriptor already covers the table stakes.
All competitor notes are **paraphrased observations**; no copy, branding, or assets were reused.

## Competitors reviewed

| # | Competitor | Shape | What it does for TOML → CSV |
|---|---|---|---|
| 1 | HCODX "TOML to CSV Converter" | Free single-purpose browser page | Accepts an array-of-tables (`[[users]]`) or a table holding an array of objects; each entry becomes a row, keys become columns. Client-side only, no upload, works offline after first load, no rate limits. Nested values are JSON-stringified into the cell so nothing is lost; the page tells users to pre-flatten if they want cleaner columns. |
| 2 | Alpha DevTools "TOML to CSV" | Free single-purpose browser page | Same core conversion (array of tables → CSV) with a rendered table preview alongside the CSV text. Advertises 100% client-side processing. |
| 3 | TableConvert (CSV ⇄ TOML, 30+ formats) | Freemium multi-format table workbench | Strongest UX: paste/upload/scrape input, auto-detects comma/tab/semicolon/pipe delimiters, a spreadsheet-like editor (dedupe, delete rows/cols, transpose, case conversion, regex find-replace, undo/redo, fullscreen), live preview, copy/download. Free tier caps files at 10 MB; larger inputs need their extension or paid API. |
| 4 | ConvertSimple data converters | Free multi-format converter site | Bidirectional TOML ⇄ INI/JSON/XML/YAML. Notably **no TOML → CSV path at all** — tabular output is a genuine gap in that family. Options are not documented per converter. |
| 5 | `dasel` (and `yq`) CLI | Open-source command-line | `dasel -f input.toml -o csv` converts formats transparently; dot-notation selectors (`a.b.0.c`) let a user pick exactly which array-of-tables to emit. No flattening mode for nested tables — nested structures either error or collapse. Requires an install and a query language. |
| — | CrossConvert (macOS app) | Paid native app | JSON/CSV/YAML/TOML conversion with "smart flattening" of nested data into dot-notation columns — confirms dot-notation flattening is a shipped, valued feature, not an invention. |

## Table stakes (all shipped in v0.1.0)

- Array-of-tables → one CSV row per entry, keys → columns (all five).
- Header row from the union of keys across entries, so ragged entries don't lose data (1, 2).
- Delimiter choice: comma / semicolon / tab / pipe (3).
- Fully local, no upload, no account (1, 2, 3).
- Nested values preserved rather than dropped — JSON-stringified cells (1).
- Selecting *which* table to convert by dotted path when a document has several (5).

## Gaps closed relative to the field (our differentiators)

- **`nested = flatten` (default): dot-notation columns.** Competitors either JSON-stringify every
  nested table (1) or need a paid native app for flattening. We flatten `[[users]] address.city`
  into an `address.city` column by default, with `json` and `skip` still available.
- **`array_format` for scalar arrays.** `json` (`["a","b"]`), `join` (`a; b`), or `columns`
  (`tags.1`, `tags.2`). No web competitor exposes this at all.
- **`columns` ordering control** — `union` (first-seen document order), `sorted` (alphabetical),
  `first` (lock the schema to the first entry). Ordering is undocumented/implicit elsewhere.
- **Auto-detection with a deterministic rule + a single-table fallback**, plus an error message
  that *lists the array-of-tables it did find* when a requested path is missing. Competitor errors
  are generic parse failures.
- **Stated, enforced caps** (20,000 rows, 2,000 columns) instead of a silent tier limit (3).
- **CLI + chat-tool surface** from the same descriptor, so it is scriptable without learning a
  query language the way `dasel` requires (5).
- **`include_header` toggle** for appending to an existing sheet — not offered by 1, 2, or 4.

## Considered, not built (out of model or rejected)

- **Spreadsheet-style post-edit workbench** (dedupe, transpose, regex replace, undo/redo) — that
  is TableConvert's whole product (3). Out of scope for a single-purpose converter; the toolkit
  covers those as separate `csv-*` tools (`csv-dedupe`, `csv-transpose`, `csv-regex-replace`).
- **Rendered HTML table preview** (2) — the shared page renderer outputs text with a download
  link; a per-tool preview widget would be a slug-specific hack in the shared runtime.
- **File upload / drag-and-drop for `.toml`** — the pure-text page takes pasted input; upload is a
  platform-level capability, not a per-tool one.
- **Round-trip CSV → TOML** — the reverse direction is a different tool shape (and TableConvert's
  CSV → TOML already exists); keeping this block one-directional keeps the schema honest.
  `json-yaml-convert` already covers TOML ⇄ JSON ⇄ YAML.
- **Paid tiers / API / >10 MB server-side conversion** (3) — out of model; everything here runs in
  the browser sandbox with published caps.

## Verification snapshot

Built and verified on 2026-08-20: `cargo test --workspace` (core unit + drift-guard),
`scripts/build-block-wasm.sh toml-to-csv`, `wasm-pack` web build, manifest sync, page generation,
`gizza tool toml-to-csv` exact-output CLI run, the page's generated CLI example run verbatim,
Playwright page spec (incl. a `?param=` deep link and every enum value), and
`scripts/check-tool-hygiene.py toml-to-csv`.

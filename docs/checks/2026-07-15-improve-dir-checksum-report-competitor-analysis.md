# dir-checksum-report — competitor analysis (2026-07-15)

Scope: tools that batch-hash a set of files and produce a checksum manifest/report
for verification (integrity checking, duplicate detection). All observations are
paraphrased from public product behaviour; no competitor copy, branding, or
trademarks are reproduced.

## Competitors reviewed

1. **A desktop "folder manifest" integrity/duplicate-detection tool** — computes
   CRC-32 (fast) and SHA-256 (cryptographic) simultaneously for every file in a
   scanned folder in one pass, and produces a searchable HTML report showing both
   hash values side by side, with a verification view flagging mismatches and a
   filter by algorithm. Positions itself explicitly as an audit/compliance +
   duplicate-detection tool. Runs fully offline.
2. **A file-manager's built-in multi-file checksum tool** — computes CRC-32, MD5,
   SHA-1, SHA-256, and SHA-512 for a batch of files, can create industry-standard
   checksum-collection files (`.sfv` for CRC-32, `.md5`, `.sha`, `.sha256`,
   `.sha512`), and offers a verify mode with visual (color) feedback for matches
   vs. mismatches.
3. **A long-standing free multi-file hashing GUI utility** — computes six digests
   (MD5, SHA-1, CRC-32, SHA-256, SHA-512, SHA-384) for one or more files at once,
   displays a sortable list with filename/size/modified-time/hash columns plus an
   "Identical" column flagging files that hash the same, and exports the whole
   list as text, tab-delimited, HTML, XML, or CSV. Individual hash columns can be
   toggled on/off.

## Table-stakes → where each lands in our model

| Table-stake capability | Decision |
| --- | --- |
| Batch: hash a SET of files (not one) in a single pass | **In model** — the whole point of the tool; `files` is a `source_list` (≥2 sources), matching the `blocks/csv-merge`/`blocks/merge-pdf` batch pattern. |
| CRC-32 + SHA-256 computed together (the backlog's explicit ask) | **In model** — default `algorithms = "crc32,sha256"`. |
| Toggle which digest algorithms are included (MD5/SHA-1/SHA-256/SHA-512/CRC-32) | **In model** — `algorithms` is a comma-separated list (any of `crc32,md5,sha1,sha256,sha512`), reusing the same pure-Rust RustCrypto hashers already proven in `blocks/file-hash`/`blocks/hash-all`. |
| Filename + size columns | **In model** — every row always carries `File`/`Size (bytes)` (Markdown) or `file`/`size_bytes` (CSV). |
| Markdown table report (the backlog's explicit output ask) | **In model** — default `format = "markdown"`. |
| CSV export (HashMyFiles' primary interchange format; also offered by the folder-manifest tool's filtering view) | **In model** — `format = "csv"`, RFC4180-ish quoting for filenames containing commas/quotes. |
| Duplicate-file detection ("Identical" column / the folder-manifest tool's core positioning) | **In model** — a "Duplicate files" section (Markdown only) groups any files whose digests match across *every* requested algorithm, cheap to compute from the already-hashed rows. |
| Sortable rows (by name or size) | **In model** — `sort_by` (`name` default, or `size`); a text-report analogue of a GUI's clickable column sort. |
| Verify against a caller-supplied expected checksum | **Out of model (this tool)** — that is exactly `blocks/verify-checksum`'s job (single value vs. expected digest, with algorithm auto-detection by digest width); duplicating it here would be scope creep onto an existing block, not a distinct capability. |
| Industry-standard per-algorithm checksum-collection files (`.sfv`, `.md5`, `.sha256`, `.sha512`) | **Out of model** — those formats are single-algorithm-per-file conventions (`hash  filename` / SFV's `filename crc32`); this tool's whole value is reporting *multiple* algorithms per file side by side, so a per-algorithm collection file is a different output shape. The two formats we do ship (Markdown table, CSV) already cover the "one shareable file with everything" need; users who want a raw `sha256sum -c`-compatible list can already get one from `blocks/hash-all`/`blocks/file-hash` per file. |
| Searchable/filterable HTML report with a live filter-by-algorithm view | **Out of model** — this repo's tools return text/Markdown (chat + CLI surfaces only, no page for a multi-file report — see below); an interactive HTML/JS report is a browser-app feature outside a text-manifest tool's scope. |
| File modified/created timestamps as report columns | **Out of model** — the gizza dispatch model resolves each source to `(bytes, mime, filename)` only; uploaded/fetched bytes carry no reliable filesystem mtime/ctime to report. |
| Third-party reputation lookup (e.g. submitting a hash to an online file-reputation service) | **Out of model** — gizza tools are offline/local-compute only; shipping an outbound reputation-lookup call would break every other tool's "runs locally, nothing leaves your machine except an explicit fetch you asked for" model, and is unrelated to producing a checksum manifest. |

## Notes / honesty

- **No page.** A batch report needs *more than one* file upload slot at once; the
  gizza page driver's file input is a single upload (the same constraint already
  documented for `blocks/csv-merge` and `blocks/merge-pdf`). This tool follows
  that exact precedent: `Input::None` + a required `files` source_list, chat + CLI
  only, no `page/` directory, no Playwright spec — mirroring how the `new-tool`
  skill treats `network`/`gpu` types as chat/CLI-only when a page surface isn't
  architecturally possible.
- Reused the exact CRC-32 implementation and RustCrypto digest set already proven
  wasm-safe in `blocks/file-hash-core` (`md-5`, `sha1`, `sha2` — all pure-Rust).
  No new dependency risk.
- No competitor copy, UI text, or branding was copied anywhere in this tool's
  descriptor, code comments, or this document — every capability above is a
  paraphrase of observed product behaviour.

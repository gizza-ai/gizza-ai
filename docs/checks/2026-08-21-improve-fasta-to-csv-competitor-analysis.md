# fasta-to-csv — competitor analysis (2026-08-21)

Scan run **before** implementing, per `/create-next-tool` step 4. Everything below is a
**paraphrase** of publicly documented behaviour — no competitor copy, branding or trademark
text is reproduced anywhere in this repo.

Search used: "FASTA to CSV converter online tool sequence id description length".

## Competitors reviewed

| # | Tool | Kind | Reachable |
| - | ---- | ---- | --------- |
| 1 | OligoPool format converter (`oligopool.com/format-converter`) | Browser converter, FASTA ↔ CSV/TSV/plain | yes |
| 2 | Convert.guru FASTA converter (`convert.guru/fasta-converter`) | Browser file converter | yes |
| 3 | seqkit `fx2tab` (`bioinf.shenwei.me/seqkit/usage/`) | CLI, the de-facto reference implementation | yes |

(Biostars threads and one-off Python gists were read as background but are recipes, not tools,
so they are not counted as competitors.)

## Table-stakes matrix

| Capability | Seen in | In-model? | Decision |
| ---------- | ------- | --------- | -------- |
| Emit `id`, `description`, `sequence`, `length` columns | 1, 3 (`--length`), gists | in-model | **built** — the default column set |
| Join multi-line / wrapped sequence lines into one field | 1, 3 | in-model | **built** — always; blank lines ignored |
| Choose the output delimiter (comma vs tab; also semicolon/pipe for EU spreadsheets) | 1 (CSV/TSV presets), 3 (TSV only) | in-model | **built** — `delimiter` enum (`comma`/`tab`/`semicolon`/`pipe`) |
| Header row on/off | 3 (`--header-line`, off by default) | in-model | **built** — `header_row`, default **on** (spreadsheet users expect it) |
| Full header vs id-only vs id+description split | 1 (description toggle), 3 (`--only-id` / `--name`) | in-model | **built** — `header_mode` enum: `split` (default) / `id_only` / `full_header` |
| Drop the sequence column (names/metrics only) | 3 (`-n, --name`) | in-model | **built** — `include_sequence` |
| Sequence length column | 3 (`-l, --length`) | in-model | **built** — `include_length`, default on |
| GC content percentage | 3 (`-g, --gc`) | in-model | **built** — `include_gc`, `(G+C)/(A+C+G+T)×100`, 2 dp — same formula seqkit documents |
| Per-base counts (A/C/G/T + everything else) | 3 (`-C, --base-count`) | in-model | **built** — `include_base_counts` adds 5 columns |
| Uppercase the sequence | 1, 3 (`-I` case handling) | in-model | **built** — `uppercase` |
| Deduplicate identical sequences | 1 | in-model | **built** — `dedupe`, case-insensitive, keeps the first record |
| Load-an-example / preset buttons | 1 | in-model | **built** — four `[[example]]` chips on the page |
| Copy + download the result | 1, 2 | in-model | already generic: `format = "text"` pages ship copy + download |
| Runs client-side, nothing uploaded | 1 ("in your browser by default") | in-model | already true — wasm, no network at all |
| Drag-and-drop file upload | 2 | out-of-model here | not built — this block's page is a paste-a-text-field tool; file upload is the generator's ffmpeg/media path, not a pure-text one |
| Reverse-complement all sequences | 1 | out-of-model for a *converter* | not built — that is a separate transform tool, not a FASTA→CSV concern |
| MD5/seq-hash column | 3 (`-s, --seq-hash`) | in-model but rejected | **considered, rejected** — pulls a hashing dependency into a parser for a column almost nobody exports to a spreadsheet |
| Alphabet / sequence-type column | 3 (`-a, --alphabet`) | in-model but rejected | **considered, rejected** — schema bloat; `include_base_counts` already exposes the raw evidence |
| Average quality per read | 3 (`-q`, FASTQ only) | n/a | not applicable — FASTA carries no quality; `fastq-to-fasta` handles FASTQ |
| Saved-sequence library / dashboard accounts | 1 | out-of-model | not built — needs a backend + login |
| Batch multi-file conversion | 2 | out-of-model | not built — single-input page/CLI by design |

## Defaults chosen (and why)

- `delimiter = comma`, because the tool is named for CSV; TSV is one dropdown away.
- `header_row = true` — seqkit defaults it *off* (pipeline ergonomics), but the audience for a
  browser converter is pasting into a spreadsheet, where a header row is what you want.
- `header_mode = split` — matches the backlog brief (`id, description, sequence, length`) and the
  columns the Python recipes in the wild emit.
- `include_sequence` / `include_length` on; `include_gc`, `include_base_counts`, `uppercase`,
  `dedupe` off, so the untouched default is exactly the four documented columns.

## UX patterns adopted

- Worked example on the page showing a real input **and** its exact CSV output.
- Preset chips (basic, TSV, metrics, dedupe+uppercase) instead of a "load example" button.
- Every fixed-choice param is a `<select>` with friendly labels; every text/number field has a
  real placeholder.
- Stated limits (50,000 records, RFC-4180 quoting, what counts toward `length`) live on the page,
  not only in error strings.
- Errors name the failing line and what was expected.

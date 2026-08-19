# logfmt-converter — competitor analysis (2026-08-13)

Scan run BEFORE implementation. All notes are paraphrased observations of publicly
visible behaviour; no competitor copy, branding or trademarked wording is reused.

## Scope of the picked backlog row

`logfmt-converter` — "Converts log/event data between logfmt, JSON, NDJSON and CSV in
either direction." Type hint: pure.

## Duplicate check (done first)

`ls blocks/ | grep -i log` returns 8 existing log blocks. The two that touch logfmt were
read directly:

- `blocks/log-parser` — parses logfmt (plus JSON/NDJSON, syslog, Apache common/combined)
  and renders a Markdown table, a JSON array or CSV. Read-only: logfmt is an INPUT only.
- `blocks/text-to-json` — parses logfmt (plus INI, key=value, CSV, passwd) to JSON/NDJSON.
  Again logfmt is INPUT only.
- `blocks/data-format-converter` — CSV/TSV/JSON/NDJSON in any direction, with type
  inference and dot-notation flattening. Does not know logfmt at all.

So the JSON↔NDJSON↔CSV half of the matrix is already covered by `data-format-converter`,
and logfmt→JSON is covered twice. The genuinely missing capability, and the reason this
block is not a semantic duplicate, is a **logfmt WRITER**: nothing in the repo can emit
logfmt. That is a real engine (quoting/escaping rules, key sanitising, null vs empty
string, flattening nested records into a flat key space), not a rename of an existing one.
The block is therefore built with logfmt as the hub format, and the overlap on the
JSON/NDJSON/CSV legs is accepted and stated on the page.

## Competitors reviewed (top 3 real tools + the reference implementations)

1. **LogLens "log to JSON" web tool** (getloglens.com/tools/log-to-json) — paste raw logs,
   pick one of three log shapes (nginx error, Apache slow request, logfmt app log), get a
   parsed JSON structure. UX: single big textarea, live "waiting…" status, copy-result
   button, clear button, three one-click sample presets, and a generated CLI command for
   the same job. Output is JSON only — no way back out to logfmt, no CSV, no field
   selection.
2. **prettylog.net** — paste-and-format log viewer. UX: example button, clear button, a
   share link, live line/word/char/size counters, a "custom parser" escape hatch, and an
   explicit in-browser (no upload) privacy claim. It is a *viewer*, not a converter: the
   output is a prettified structured view, not a re-serialised format.
3. **go-logfmt/logfmt** (the de-facto reference encoder/decoder, Go) — defines the writing
   rules everyone else follows: pairs written `key=value`, a single space before every
   pair after the first, values that contain spaces or quotes get double-quoted, records
   terminated by a newline, invalid/empty keys rejected, unsupported value types rejected.
   It deliberately does not standardise logfmt but removes ambiguity so encode/decode
   round-trips.
4. Secondary reads: `mircodz/logfmt2json` (one-way logfmt→JSON, zero options, built for
   piping into `jq`) and `TheEdgeOfRage/logfmt` (CLI viewer whose headline options are
   output FIELD SELECTION plus level/key-value filtering).

## Table stakes extracted

| Capability | Seen in | In model here? |
| --- | --- | --- |
| Paste-a-blob textarea as the primary input | all 3 web tools | yes — `data`, multiline |
| Auto-detect the input format, with a manual override | LogLens (manual only), log-parser (auto) | yes — `from = auto` + 4 explicit values |
| logfmt → JSON | all | yes |
| JSON/NDJSON/CSV → logfmt (round-trip) | none of the web tools; only the libraries | yes — the differentiator |
| NDJSON (JSON-Lines) as a first-class format | jq-piping CLIs | yes |
| CSV export for spreadsheets | JSON formatter sites | yes, incl. delimiter choice |
| Type inference (numbers/bools/null) on unquoted values | logfmt libraries, csvjson | yes — `detect_types` |
| Pretty vs compact JSON | every JSON tool | yes — `pretty` |
| Nested objects → flat keys (dot notation) | data-format-converter, csvjson | yes — `flatten` |
| Field selection / column ordering | TheEdgeOfRage/logfmt | yes — `keys` |
| One-click sample presets | LogLens, prettylog | yes — `[[example]]` chips |
| Copy result / reset / share-by-URL | prettylog, LogLens | yes — generator gives copy+reset; `?param=` deep links are the share form |
| Stated in-browser privacy | prettylog | yes — page copy + WASM |
| Live line/char counters | prettylog | no — out of model (the generator has no counter widget); the record count is instead reported by errors and the CSV/JSON shape |

## Out of model (listed, not built)

- Level/severity and key=value **filtering** of records — that is `log-parser`'s job
  (`level`, `filter`, `regex` params); duplicating it here would make this the near-dup it
  currently is not.
- Syslog / Apache common / combined access-log inputs — already `log-parser`.
- Colourised/highlighted rendering and "custom parser" regex rules (prettylog) — needs a
  bespoke renderer, and gizza pages render a plain text result.
- Streaming / follow-mode over a live log tail — no I/O in a pure block.
- A generated CLI command for a third-party CLI (LogLens) — the gizza page already
  generates its own runnable `gizza tool` example.

## Design decisions taken from the scan

- `from` gets an `auto` value (LogLens's fixed dropdown is the weak spot; log-parser's
  auto-detect is the good pattern) but every format stays explicitly selectable.
- Encoding follows the go-logfmt rules: single-space separation, double quotes with
  `\" \\ \n \r \t` escapes, quoting whenever the value holds a space, `=`, a quote or a
  control character. Keys with those characters are sanitised to `_` rather than dropping
  the pair silently.
- `null` writes as a bare `key=` and an empty string writes as `key=""`, so a
  JSON → logfmt → JSON round-trip with `detect_types` on preserves the difference. The
  libraries leave this ambiguous; making it explicit is a small, honest improvement.
- `keys` (comma-separated) both selects and orders output fields — the one option the CLI
  viewers agree matters and the web tools all lack.

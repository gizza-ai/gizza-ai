# docker-cli-output-parser — competitor analysis (2026-08-15)

Scan run before implementation. Search query: `docker ps output parser convert docker stats to json csv`. Competitors and adjacent references were used for capability/UX comparison only; wording below is paraphrased.

## Competitors reviewed

1. Docker's own `--format` / Go-template documentation — the official way to get machine-readable fields when you can rerun the command.
2. Community scripts that pipe `docker ps` or `docker stats` through awk/jq — usually whitespace-split tables, often breaking `COMMAND`, `CREATED`, `STATUS`, `PORTS`, and composite stats columns.
3. Generic fixed-width/table-to-CSV converters — good at column slicing, but unaware of Docker-specific headers, typed percentages, byte units, or `MEM USAGE / LIMIT` / `NET I/O` pairs.

## Table stakes and decisions

| Capability | Decision |
| --- | --- |
| Parse `docker ps`, `docker images`, and `docker stats --no-stream` pasted output | In model: `kind=auto|ps|images|stats`, detected from headers. |
| Preserve headers with spaces (`CONTAINER ID`, `IMAGE ID`, `MEM USAGE / LIMIT`, `NET I/O`) | In model: fixed-width slicing from the header ruler, plus tab-table support. |
| Values with spaces (`COMMAND`, `CREATED`, `STATUS`, `PORTS`) | In model: kept inside their column boundaries instead of whitespace splitting. |
| JSON for scripts and CSV/TSV/Markdown/table for docs | In model: `output=json|csv|tsv|markdown|table`. |
| Typed values | In model: optional `parse_values`; percentages/numbers/byte counts/ports/names/composite I/O are split and typed. |
| Column naming modes | In model: `keys=snake|header|docker`. |
| Pick/reorder columns and cap rows | In model: `columns` and `limit`. |
| Strict failure for broken pasted logs | In model: `strict` catches kind mismatch and truncated rows. |
| Live Docker daemon inspection | Out of model: this repo runs browser/wasm tools locally and should not talk to a Docker socket. |
| Perfect support for arbitrary user `--format` templates without a header | Out of model: without a header row there is no schema/ruler to infer columns. |
| Replacing Docker's native `--format '{{json .}}'` | Out of scope: this tool is for saved/pasted output when the command cannot be rerun. |

## UX adopted

- Main textarea includes the header line in the placeholder.
- Select controls for command kind, output format, and key naming.
- Checkbox controls for typed parsing, header rows, and strict mode.
- Example chips for `ps → JSON`, `stats → CSV`, `images → Markdown`, and stats column selection.
- Page copy warns about requiring a header row and Docker CLI version drift.

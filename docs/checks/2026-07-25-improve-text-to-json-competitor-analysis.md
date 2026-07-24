# text-to-json — competitor analysis (2026-07-25)

Paraphrased scan only — no competitor copy, branding, or trademarks reproduced.

## Function

Paste a chunk of loosely-structured plain text — an INI/.conf file, a `key=value`
config / `.env` / `.properties` block, a logfmt log line, a CSV/TSV table, or an
`/etc/passwd`-style colon-delimited file — and get clean JSON back. The value over
a single-format converter is one paste box that auto-detects the shape and one
output that covers config objects, log arrays, and tabular data.

## Competitors skimmed

1. **INI-to-JSON browser converters (codeshack-style, jsontotable-style,
   wtools-style, site24x7-style).** All follow the same paste-INI / get-JSON
   pattern. Table-stakes: parse `[section]` headers into nested objects, keep flat
   top-level keys, drop `#`/`;` comment lines, and coerce obvious numbers/booleans.
   Output is pretty-printed JSON with a copy button; errors are shown inline.
2. **CSV/TSV-to-JSON converters (tableconvert-style).** Turn a header row + data
   rows into an array of objects, first row as keys. Users expect delimiter choice
   (comma/tab/semicolon/pipe) or auto-detection, RFC-4180 quoting (`"a, b"`),
   type inference toggle, and a "records vs columns" output shape. Many also offer
   minified vs pretty and download.
3. **`.env` / KEY=VALUE converters (terrific-style).** Convert between `.env`-style
   `KEY=VALUE` lines and a flat JSON object. Table-stakes: one pair per line,
   ignore blank/comment lines, tolerate `export ` prefixes and quoted values.
4. **logfmt parsers (node-logfmt, Grafana Loki `| logfmt`, CloudWatch
   `parse @message logfmt`).** Tokenize `k=v k2="a b"` space-separated pairs into an
   object per line, honoring double-quoted values with spaces and bare-key boolean
   flags (`tls` → `true`). Not usually packaged as a paste-box web tool — mostly
   libraries / query pipelines — so a browser logfmt→JSON box is a genuine gap we
   fill.

## Table stakes → decisions

| Capability / UX pattern | Decision |
|---|---|
| Paste multiline text into a large textarea | **in-model** — `text` is a multiline field with a realistic placeholder. |
| INI `[section]` → nested object, flat globals kept | **in-model** — `parse_ini`. |
| Drop `#` / `;` comment lines | **in-model** — shared `is_comment`, all formats. |
| `KEY=VALUE` / `.env` / `.properties` → flat object | **in-model** — `keyvalue`, tolerates `export ` and `key: value`. |
| logfmt `k=v k2="a b"` → array of objects, bare-key flags | **in-model** — `logfmt`, quoted values + escapes. |
| CSV/TSV header row → array of objects | **in-model** — `csv`, first row as keys. |
| Delimiter auto-detection (`,` `;` tab `\|`) | **in-model** — sniffed; pin via `format`. |
| RFC-4180 double-quote handling (`"a, b"`, `""` escape) | **in-model** — `split_delimited`. |
| Type inference toggle (numbers/booleans) | **in-model** — `detect_types` checkbox, default on. |
| Keep-everything-as-string mode (leading zeros) | **in-model** — uncheck `detect_types`. |
| Pretty vs minified JSON | **in-model** — `pretty` checkbox, default on. |
| Format auto-detection with a "what did it pick" readout | **in-model** — `format=auto` + `output=report` surfaces `detected_format`. |
| NDJSON / JSON-Lines output | **in-model** — `output=ndjson`. |
| `/etc/passwd` / group colon files → named fields | **in-model** — `passwd`, 7-field passwd or 4-field group. |
| Copy / download the JSON output | **in-model** — generic text-page copy + download surface. |
| Example / preset buttons | **in-model** — six `[[example]]` preset chips. |
| Convert JSON *back* to INI/env (reverse direction) | **out-of-model** — this tool is one-way text→JSON; the reverse is a separate tool. |
| Live spreadsheet grid preview / cell editing | **out-of-model** — the generic page is form + text output, not a grid editor. |
| Upload files / server-side batch / API | **out-of-model** — gizza runs browser-local + CLI, not a hosted batch API. |
| YAML / TOML / XML input | **considered, not built** — covered by sibling converter tools; this tool targets the flat/loose text shapes those don't. |

## Worked-example decisions

- Default placeholder + first preset is a logfmt pair (the format with the weakest
  existing web-tool coverage), so the least-served use case is front and centre.
- A `keyvalue` preset uses `export HOST=…` + a comment line to show `.env` tolerance.
- An INI preset shows nested sections plus a flat global key.
- A CSV preset shows type inference (`true`/`false`/ages) into an array of objects.
- A passwd preset shows named-field mapping.
- An auto-detect **report** preset shows the `detected_format` / `record_count`
  wrapper so users learn the auto path.

## Limits surfaced on page

- Auto-detect is a heuristic; ambiguous text (e.g. a single colon-delimited line)
  can be mis-tagged — pin `format` to override.
- CSV needs a header row plus at least one data row, or it errors.
- Duplicate keys within one object/section keep the last value (JSON objects can't
  repeat a key).
- Type inference is opt-out per whole document, not per field; quote a value (or
  turn `detect_types` off) to keep it a string.
- The whole input is parsed in memory in the browser tab, so very large pastes can
  be slow.

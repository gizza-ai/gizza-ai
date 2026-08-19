# arff-converter — competitor analysis (2026-08-08)

Scan run before implementing `blocks/arff-converter`. All findings are paraphrased from public
documentation; no competitor copy, branding or trademarks are reproduced.

## Tools reviewed

| # | Tool | What it is | Direction |
|---|------|-----------|-----------|
| 1 | Weka's own converters (`weka.core.converters.CSVLoader` / `CSVSaver`, plus the ARFF Viewer "save as CSV" path) | The reference implementation and de-facto spec | both |
| 2 | `arff-csv-converter` (PyPI) | Bidirectional CSV↔ARFF library + CLI with type inference | both |
| 3 | Browser ARFF→CSV converter hosted on GitHub Pages (pulipulichen jieba-js/weka/arff2csv) | Paste-or-upload web converter | ARFF→CSV only |
| — | `arff-format-converter` (PyPI), `jsatria/arff-to-csv`, `anaavila/convert-csv-to-arff`, 101convert listing | One-direction / batch helpers, skimmed for table stakes only | mixed |

Reference used for format correctness: the Weka wiki ARFF specification page (relation/attribute/data
sections, nominal `{}` lists, `date` with a SimpleDateFormat pattern, `?` for missing, `%` comments,
sparse `{index value}` rows, trailing `{weight}` instance weights).

## Table-stakes matrix

| Capability | Seen in | Fit | Where it landed |
|---|---|---|---|
| ARFF → CSV | 1, 2, 3 | in-model | `direction=arff-to-csv` |
| CSV → ARFF | 1, 2 | in-model | `direction=csv-to-arff` |
| Auto-detect which way to convert | 2 (partly, by extension) | in-model | `direction=auto` (default) — sniffs `@relation`/`@data` |
| Configurable field separator (`-F`, `--delimiter`) | 1, 2 | in-model | `delimiter` — single char or `comma`/`tab`/`semicolon`/`pipe` |
| Header-row handling (`-H`) | 1, 2 | in-model | `header` boolean |
| `@relation` name (`--relation`) | 1, 2 | in-model | `relation` (empty → `data`) |
| Automatic numeric / nominal / string typing | 1, 2 | in-model | inference + `nominal_threshold` |
| Nominal cut-off (`--nominal-threshold`, default 10) | 2 | in-model | `nominal_threshold`, default 10, `0` = never nominal |
| Force a column's type (`-N`/`-S`/`-R`/`-D`, `--nominal`/`--string`) | 1, 2 | in-model | one `column_types` param taking `name:type` or `index:type` pairs — same power, one field instead of four |
| Date attributes + date pattern (`-D` + `-format`, default `yyyy-MM-dd'T'HH:mm:ss`) | 1, 2 | in-model | `date_format`, applied to columns typed `date` |
| Missing-value token (`-M`, `--missing`, default `?`) | 1, 2 | in-model | `missing_value` — the **CSV-side** token; ARFF's `?` is fixed by the spec |
| Sparse ARFF (`{index value, …}`) read and write | 1, 2 | in-model | parsed automatically on input; `arff_format=sparse` on output |
| Quoted/escaped values, `%` comments, `\n`/`\t`/`\\` escapes | 1, 2 | in-model | full quote + escape handling both ways |
| Instance weights `,{2.0}` | 1 | in-model | parsed and stripped (documented) |
| Preserving attribute types across the round trip | backlog goal; nobody does it in CSV | in-model | `type_row` — optional second CSV header row carrying each ARFF type, and it is read back on the CSV→ARFF side |
| Preset examples / one-click sample data | 3 (sample datasets), 2 (`--analyze`) | in-model | four `[[example]]` chips on the page |
| Download / copy output | 3 | in-model | generator gives `format = "text"` pages a Download link + copy |
| File upload of a `.arff`/`.csv` | 1, 3 | out-of-model here | this block is a pure text tool; paste is the input path |
| Convert ARFF to XLSX / XML / JSON | `arff-format-converter` | out-of-model | out of scope — the toolkit already ships separate CSV→XLSX / CSV→XML / CSV→JSON blocks |
| Relational (multi-instance) attributes | 1 | out-of-model | rejected with an explicit, actionable error rather than mis-parsed |
| Running classifiers / producing predictions | 3 | out-of-model | not a converter feature |
| `--encoding` for legacy input encodings | 2 | out-of-model | the toolkit's `text-encoding-converter` block already covers transcoding |
| `--exclude COL` column dropping | 2 | out-of-model | the toolkit's `csv-filter` / `csv-reorder-columns` blocks already cover it |

## Gaps we close that the field leaves open

- **Bidirectional in one place.** The only free browser converter found goes ARFF→CSV only; the
  bidirectional options are Python/Java command-line tools. This block does both, in the browser,
  with auto-direction detection.
- **Type preservation.** The backlog entry's premise is "preserving attribute types", and no
  reviewed tool preserves ARFF typing through CSV. `type_row` writes the attribute types as a
  second CSV header line and reads that line back on the way in, so ARFF → CSV → ARFF round-trips
  keep nominal label sets, dates and strings instead of collapsing to re-inferred types.
- **One override field instead of four index ranges.** Weka's `-N/-S/-R/-D` take attribute *ranges*
  (`1,4,5-27`), which is error-prone against a header row. `column_types` accepts column names as
  well as 1-based indices.

## Decisions

- Nominal detection matches the field's default (≤10 distinct non-missing values → nominal),
  configurable, with `0` meaning "never nominal".
- Dates are only produced for columns explicitly typed `date`; silent date auto-detection is what
  makes other converters mangle ID columns, so it is deliberately not inferred.
- Input is capped at 2,000,000 characters with an explicit error, to stay inside the wasm sandbox.

# extract-numbers-from-text — competitor analysis (2026-07-17)

Scan of the top "extract numbers from text" web tools to set table-stakes for
the gizza tool. All copy below is paraphrased; no competitor copy, branding, or
trademarks are reproduced.

## Competitors reviewed

1. **MiniWebtool — Number Extractor** (miniwebtool.com/number-extractor/) — the
   richest: extracts integers, decimals, and scientific notation, and reports
   instant statistics (sum, average, min/max), distribution analysis, and offers
   multiple export formats.
2. **Browserling — Extract Numbers** (browserling.com/tools/extract-numbers) —
   minimal, ad-free; pastes text and lists every number found.
3. **Text-Utils — Number Extractor** (text-utils.com/number-extractor/) —
   client-side only; data never leaves the browser (privacy is the headline).
4. **OneCompiler — Extract Numbers** (tools.onecompiler.com/extract-numbers) —
   simple paste → extract flow, free.
5. **IncludeHelp — Extract Numbers from Text and Strings**
   (includehelp.com/tools/...) — identifies numeric values with a basic UI; also
   markets sorting and export of the extracted numbers.

## Table-stakes → decision

| Capability | In competitors | Our tool |
| --- | --- | --- |
| Extract integers & decimals | all | ✅ `mode=all` regex covers both |
| Scientific notation (`6.022e23`) | MiniWebtool | ✅ matched |
| Signed numbers (`-7`, `+5`) | some | ✅ unary-sign logic (date-safe) |
| Thousands separators (`1,000,000`) | some | ✅ matched & value-normalised |
| Summary stats (sum/avg/min/max) | MiniWebtool | ✅ `stats=true` |
| Count of numbers | most | ✅ in `count` + stats block |
| Sort output | IncludeHelp | ✅ `sort` original/asc/desc |
| De-duplicate values | implicit | ✅ `unique` (value-based) |
| Choice of output delimiter / export format | MiniWebtool, IncludeHelp | ✅ `delimiter` newline/comma/space/tab/semicolon; page also has a Download link for text output |
| Filter to only integers / only decimals | — (differentiator) | ✅ `mode=integers`/`decimals` |
| Client-side / privacy | Text-Utils | ✅ runs in-browser via wasm; nothing uploaded |

## Out-of-model (listed, not built)

- **Distribution analysis / histogram chart** (MiniWebtool) — a visual chart is
  outside this pure-text tool's output shape (page renders `format = "text"`);
  the numeric summary (count/sum/min/max/average) covers the analytical need.
- **File export in multiple binary formats** (xlsx, etc.) — the generator gives
  every `format = "text"` page a plain Download link, which covers copy-out;
  spreadsheet-binary export is out of scope.
- **Spelled-out numbers / Roman numerals / fractions (`3/4`)** — extraction is by
  textual numeric form; word-to-number NLP is out of model. Documented as a
  limitation on the page.

## Copy/UX gaps closed

- Added preset **example chips** (invoice totals with stats; sorted unique
  integers) — competitors surface one-click samples.
- Friendly `<select>` labels for `mode`, `sort`, and `delimiter`.
- Page documents the date-vs-negative-sign rule and the unit-stripping behaviour
  explicitly (edge cases competitors leave unstated).

## Sources

- https://miniwebtool.com/number-extractor/
- https://www.browserling.com/tools/extract-numbers
- https://www.text-utils.com/number-extractor/
- https://tools.onecompiler.com/extract-numbers
- https://www.includehelp.com/tools/extract-numbers-from-text-and-strings-online.aspx

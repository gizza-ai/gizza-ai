# zero-pad-ids — competitor analysis (2026-08-17)

Scan run while building `blocks/zero-pad-ids`. Everything below is **paraphrased** from public
tool pages and how-to articles — no competitor copy, branding, or trademarks are reproduced here
or in the shipped tool.

Search used: "online tool pad leading zeros to fixed width CSV column ID codes" (WebSearch), then
per-page reads. Three of the results are real, reachable tools/workflows for this exact job; the
rest of the result set was spreadsheet/R/SQL how-to articles rather than tools, so the profiles
below cover the three real ones plus the two recurring "generic" patterns those articles describe
(spreadsheet `TEXT(A2,"00000")` and `str.zfill`/`LPAD`), which are what users compare against.

---

## 1. dCode — "Leading Zeros" calculator

- **URL:** https://www.dcode.fr/leading-zeros
- **Features:** two directions in one page — add leading zeros to reach a target length, and
  remove existing leading zeros. Batch-oriented: many values at once.
- **Params/options:** target total digit length **or** "number of zeros to add"; a separate
  removal mode; an option to show only the rows that actually changed.
- **Input formats:** several numbers pasted, separated by commas/whitespace.
- **Output formats:** a two-column table (original → formatted), exportable as `.csv`/`.txt`,
  plus copy-to-clipboard.
- **UX patterns:** textarea in / table out, distinct labelled sections per direction, export
  buttons.
- **Stated limits/caveats:** padded values are text, so some systems then sort them
  alphabetically; JavaScript can read some zero-prefixed literals as octal; excessive padding
  grows file size.
- **Copy angles:** what a leading zero is; why `007` and `7` are the same *number* but different
  *identifiers*; postal codes, database keys, ID numbers.
- **Free/paid:** free, no account.

## 2. Boost Tool — zero padding / alignment

- **URL:** https://boost-tool.com/en/tools/zero_padding
- **Features:** bulk multi-line padding; an **auto mode** that pads every value up to the longest
  value present; a fixed-length mode; optional prefix/suffix; runs client-side.
- **Params/options:** padding method (auto-to-longest vs explicit digit count) chosen from a
  dropdown; digit count; prefix/suffix strings.
- **Input formats:** multi-line text pasted from a spreadsheet or CSV column.
- **Output formats:** padded values as text, copyable.
- **UX patterns:** three-step paste → choose method → copy flow, with screenshots; explicit
  "nothing is uploaded" claim.
- **Stated limits:** none published.
- **Copy angles:** employee IDs, product codes, standardizing before a join; privacy of local
  processing; what happens when a value is already long enough or contains non-digits.
- **Free/paid:** free.

## 3. SplitForge data cleaner (+ its leading-zeros guide)

- **URL:** https://splitforge.app/blog/csv-leading-zeros-disappearing-fix
- **Features:** column-aware CSV cleaning — pick the affected column, choose "pad with leading
  zeros", set a target length, download the fixed file. Detects columns whose numeric-looking
  values have inconsistent lengths; previews the change before applying; browser-local.
- **Params/options:** column selector, target length.
- **Input formats:** uploaded CSV.
- **Output formats:** downloaded CSV.
- **UX patterns:** upload → select column → preview → download; guidance framed around specific
  identifier families (5-digit postal codes, 6-digit employee IDs, account numbers).
- **Stated limits:** aimed at standard identifier shapes; the surrounding guide is mostly about
  *preventing* the loss at import/export time.
- **Copy angles:** why the zeros vanish in the first place (a spreadsheet or loader typing the
  column as a number), Power Query "import as text", re-exporting from the source system, and the
  programmatic equivalents (`TEXT(A2,"00000")`, `str.zfill`, `LPAD`).
- **Free/paid:** free tier for the cleaner.

## 4. Generic pattern — spreadsheet formula / custom format

- `TEXT(A2,"00000")` in a helper column, or a `00000` custom number format.
- **Pattern:** per-cell, needs a helper column and a copy-paste-values step; the custom-format
  route only changes the *display*, so the CSV re-export loses the zeros again.
- **Why users leave it:** it does not survive round-tripping, and it is per-column manual work.

## 5. Generic pattern — code (`str.zfill`, `LPAD`, `printf "%05d"`)

- **Pattern:** exact and scriptable, applied to one series/column at a time.
- **Why users leave it:** requires an environment and a script for a one-off paste; nothing
  handles "the column is mostly digits but has `N/A` in three rows" without extra branching.

---

## Gap list vs what we shipped

| # | Gap seen at a competitor | Dimension | Verdict |
| - | ------------------------ | --------- | ------- |
| 1 | Both directions (pad **and** strip) in one tool (dCode) | capabilities | **built** — `mode = pad \| strip` |
| 2 | Auto-width: pad everything up to the longest value (Boost Tool) | capabilities | **built** — `width = 0` means auto, computed **per column** |
| 3 | Column-scoped edit on a real table, not a bare list (SplitForge) | capabilities | **built** — `columns` by name or 1-based position, `header`, `delimiter` incl. `auto` |
| 4 | Defined behavior when a value is already at/over the width (Boost Tool copy) | capabilities | **built** — `overflow = keep \| strip \| error` |
| 5 | Defined behavior for non-digit cells (Boost Tool copy) | capabilities | **built** — `non_numeric = keep \| pad \| error`; blank cells are never invented into `00000` |
| 6 | Table/CSV output that round-trips into a loader | capabilities | **built** — same delimiter out, `quote_style = minimal \| always \| never` |
| 7 | "Show only changed rows" report view (dCode) | UX | **considered, rejected** — the output here is a table you paste back into a pipeline; a diff view would break round-tripping. The worked example on the page shows before/after instead. |
| 8 | Prefix/suffix strings around the padded value (Boost Tool) | capabilities | **considered, rejected** — that is a general column-rewrite job; `csv-regex-replace` and `csv-insert-column` already cover it, and it would make this tool's contract ("only the zeros change") untrue. |
| 9 | Caveat copy: padded IDs are text and sort/round-trip differently (dCode) | copy/SEO | **built** — page limits section + FAQ on Excel re-mangling and on `quote_style = always` |
| 10 | Preset chips for common identifier widths (SplitForge's 5/6-digit framing) | UX | **built** — `[[example]]` chips for 5-digit postal codes, 8-digit SKUs, auto-width, strip, and a TSV/auto-delimiter case |
| 11 | File upload + download of a whole CSV (SplitForge) | UX | **out of model here** — this repo's pure-text page takes a paste and offers a Download link on the result; a true upload widget belongs to the file-input tool family. |
| 12 | Detect which columns "look like" broken IDs automatically (SplitForge) | capabilities | **out of model** for this tool — that is column profiling; `csv-column-type-validator` / `csv-type-inferrer` are the right homes. Listed, not built. |

No competitor copy, wording, or assets were reused. All page copy, examples, and FAQ answers are
original.

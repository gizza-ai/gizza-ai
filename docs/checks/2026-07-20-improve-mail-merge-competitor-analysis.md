# mail-merge — competitor analysis (2026-07-20)

Tool function: fill a text/markdown template once per CSV row (classic "form
letter" / mail merge), producing one rendered document per row joined by a
chosen separator. Pure compute, runs in-browser and in the CLI.

## Competitors scanned (paraphrased — no copy/branding reproduced)

1. **Easy Mail Merge (easymailmerge.com)** — spreadsheet-driven merge. Uses
   `{{variable}}` double-brace placeholders; variable names are matched to
   header columns **case-insensitively** (`{{First Name}}` == `{{first name}}`).
   Source is a Google Sheet (headers become the field vocabulary). Output is
   per-row documents (PDF default).
2. **Mail Merge Letter Generator (mail-merge-letter.com)** — upload a document
   template plus a CSV whose **first row is the column names**; it personalizes
   the letter per recipient row and emits per-row output (PDF download). CSV
   header row is load-bearing.
3. **Aspose Words Mail Merge (products.aspose.app)** — free mass-mailing merge:
   a template document plus a tabular data source (CSV/Excel), one generated
   document per data row, for letters/labels/directories.
4. **Microsoft Word Mail Merge** (reference behaviour) — a main document with
   merge fields combined with a delimited data source (`.csv`/`.txt`) to produce
   a batch of personalized letters/labels/emails. Delimited text is a first-class
   data source; missing merge fields render blank.

## Table-stakes → in-model / out-of-model

| Capability | Competitors | Decision |
|---|---|---|
| Template with named placeholders bound to CSV columns | all | in-model — `template` param |
| CSV data source, first row = headers | all | in-model — `csv` param |
| `{{col}}` double-brace placeholder syntax | Easy Mail Merge, most | in-model — `syntax=double_curly` (default) |
| Alternate placeholder styles (`{col}`, `<<col>>`) | Word/others vary | in-model — `syntax` enum adds `single_curly`, `double_angle` |
| Case-insensitive column matching | Easy Mail Merge | in-model — `case_insensitive` boolean (default true) |
| Non-comma delimiters (`;`, tab) for European/TSV data | Word delimited source | in-model — `delimiter` enum (comma/semicolon/tab) |
| Missing merge field → blank | Word | in-model — `on_missing=empty` (default); also `keep` (leave `{{x}}` for debugging) and `error` |
| Separator between generated documents (blank line, rule, page break) | implied by per-doc output | in-model — `separator` enum (divider/blank_line/newline/form_feed/none) |
| Preset example templates (chips) | landing-page samples | in-model — `[[example]]` chips |
| Per-row **PDF/DOCX file** output, one file per row, zipped | mail-merge-letter, Aspose | out-of-model — this is a text/markdown generator; binary doc packaging + zip-per-row is a separate media pipeline, not built |
| Send as **email** (SMTP/Gmail) | Easy Mail Merge, Word | out-of-model — no network/mail transport in a pure in-browser tool |
| Read data straight from a **Google Sheet / Excel** | Easy Mail Merge, Aspose | out-of-model — paste CSV instead; xlsx→csv is a separate existing tool |
| Handlebars-style loops/conditionals (`{{#each}}`, `{{#if}}`) | some | out-of-model here — that is the sibling `render-template` tool (single JSON render); mail-merge is intentionally simple column substitution × rows |

Every table-stake lands in the descriptor or is listed above as out-of-model;
none dropped silently.

## Distinct from existing blocks

- `render-template` — renders ONE Handlebars/Mustache template against a single
  JSON object/array (loops, conditionals). mail-merge is CSV-row batch
  substitution (N outputs, one per row) with plain named placeholders. Different
  input model and different output cardinality — not a duplicate.
- `csv-*` tools convert/transform CSV; none fills a text template per row.

## UX controls shipped

- `syntax`, `delimiter`, `on_missing`, `separator` → `<select>` (enum params);
  `case_insensitive` → checkbox; `template`/`csv` multiline textareas.
- `[input.labels]` friendly labels for every enum.
- `[[example]]` preset chips: an appointment reminder, a European `;`-delimited
  invoice list, and a `keep`-missing debug run.

## Limits (stated on the page)

- Up to 1000 data rows per merge (in-browser memory bound).
- Placeholders are plain `{{Column}}` substitution — no expressions/loops (use
  `render-template` for those).
</content>
</invoke>

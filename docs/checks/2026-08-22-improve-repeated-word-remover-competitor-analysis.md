# repeated-word-remover — competitor analysis (2026-08-22)

Scan run **before** implementation, per `/create-next-tool` step 4. One web search
("remove duplicate repeated words tool online / double word remover"), then the top
three reachable competitors were skimmed. Everything below is a **paraphrase** of
observed behaviour — no competitor copy, branding or trademarks are reproduced, and
out-of-scope items are listed, not built.

## Competitors skimmed

| # | Tool | URL | Shape |
|---|------|-----|-------|
| 1 | Duplicate word remover (OnlineToolix) | `https://onlinetoolix.com/duplicate-word-remover/` | Multi-mode: consecutive / all-duplicates / line / keyword-list |
| 2 | Remove duplicate words (Online Text Tools) | `https://onlinetexttools.com/remove-duplicate-text-words` | Minimal: case-sensitivity, output delimiter, "remove all copies" |
| 3 | Remove duplicate words (MeFancy) | `https://www.mefancy.com/addremove/remove-duplicate-word` | Sorting + delimiter + case toggle, file upload, preset demos |

## Table stakes observed → decision

| # | Table-stake capability | Seen on | Decision | Where it lands |
|---|------------------------|---------|----------|----------------|
| 1 | **Consecutive-duplicate mode** — collapse `the the` → `the` | 1 | **IN** | The entire tool: the core scans adjacent token pairs only |
| 2 | **Case-insensitive matching** (`The the` counts as a repeat) | 1, 2, 3 | **IN** | `case_sensitive` boolean, default `false` (i.e. insensitive by default) |
| 3 | **Ignore punctuation between words** (`word, word`) | 1 | **IN** | `ignore_punctuation` boolean, default `false`; sentence-enders `.!?` never bridge a repeat |
| 4 | **Trim / whitespace tolerance** so a repeat split by odd spacing still matches | 1 | **IN** | Any whitespace run bridges a repeat; `across_line_breaks` (default `true`) additionally bridges a hard line break — the single most common real typo shape (`… the` / `the …` at a wrap) |
| 5 | **Before/after word counts + duplicates-removed count + % reduction** | 1, 3 | **IN** | Always returned structurally (`total_words`, `kept_words`, `removed_words`, `reduction_percent`) and rendered in the `report` view |
| 6 | **"Change view" showing removed words struck through** | 1 | **IN** | `output = "marked"` wraps each removed occurrence in markdown `~~…~~`, original text otherwise untouched |
| 7 | **Removed-word list with per-word frequency** | 1 | **IN** | `findings[]` (line, column, word, occurrences, removed) + the `report` view |
| 8 | **Preset / demo buttons** to load a worked example in one click | 3 | **IN** | Four `[[example]]` chips in `page/meta.toml` (typo cleanup, OCR line-wrap repeat, punctuation bridging, audit report) |
| 9 | **Copy-to-clipboard + .txt download of the result** | 3 | **IN (generic)** | The generator gives every `format = "text"` page a copy control and a Download link — no per-tool work |
| 10 | **File upload of a `.txt` document** | 3 | **OUT of scope** | This is a pure text-in/text-out block; the page field is a paste-able textarea. Users drop a file into an editor and paste. Not a model limitation, a deliberate surface choice |
| 11 | **"All duplicates" global dedupe** (keep only first occurrence anywhere) | 1, 2 | **OUT of scope — already shipped elsewhere** | That is a different tool and gizza already has it: `blocks/list-dedupe-merge`, `blocks/word-frequency`, `blocks/remove-duplicate-lines`. Folding it in here would make this a near-dup of three existing blocks |
| 12 | **Sort output A–Z / Z–A** | 1, 3 | **OUT of scope** | Only meaningful for the word-*list* mode (#11), which we deliberately don't have. `blocks/sort-lines` covers it |
| 13 | **Custom output delimiter** (space / newline / comma / pipe) | 2, 3 | **OUT of scope** | Same reason: a delimiter only makes sense when the output is a re-joined word list. We return the user's prose with the doubles deleted, so the original spacing IS the delimiter |
| 14 | **Line/keyword-list modes** | 1 | **OUT of scope — already shipped** | `blocks/remove-duplicate-lines` (line mode) and `blocks/list-dedupe-merge` (keyword lists) |

## The differentiator none of the three had

Every competitor treats `had had` and `that that` as errors and silently destroys them.
The backlog description asks for the opposite, so the tool ships:

- **`keep_words`** — a tag-list of words that are legitimately doubled in English, protected
  by default: `had, that, is, do, no, very, long, many, far, ha, blah, bye, night, so, chop,
  tut, yum`. Editable/clearable per run.
- **`include_numbers`** (default `false`) — repeated pure-number tokens (`2024 2024`,
  table columns) are ignored unless asked for. Competitor #1 and #2 both mangle numeric
  tables.
- **`min_length`** — ignore repeats shorter than N characters, for people who don't want
  `a a` / `I I` touched.

## Feasibility spike

No spike needed: the whole tool is a tokenizer plus an adjacent-pair scan over `&str`.
No crates beyond `serde` — nothing that could fail to instantiate on
`wasm32-unknown-unknown` or `wasm32-wasip1`.

## UX controls adopted

- `output` renders as a `<select>` with friendly `[input.labels]` (Cleaned text / Marked-up
  changes / Audit report).
- `min_length` uses the generator's `kind = "slider"` (bounded 1–20, step 1).
- `keep_words` uses `kind = "tag-list"` — the protected words are pills, not a raw CSV box.
- `[[example]]` chips replace competitor #3's preset demo buttons.

## Stated limits (on the page, not just in code)

- 200,000-byte input cap per run.
- Only *adjacent* repeats — a word that reappears later in the sentence is never touched.
- The first occurrence always wins, so its capitalisation and indentation survive.
- Hyphenated compounds count as one word (`well-known well-known` is caught); a repeat split
  by an em dash or `.`/`!`/`?` is not.

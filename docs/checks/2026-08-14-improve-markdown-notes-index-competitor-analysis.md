# markdown-notes-index — competitor analysis (2026-08-14)

Scan run **before** implementation, per `/create-next-tool` step 4. One web search
("markdown notes index generator tool titles tags headings table of contents vault"),
then the top three reachable real tools were skimmed. Everything below is **paraphrased**
— no competitor copy, branding, or trademarks are reproduced anywhere in this repo.

## Competitors skimmed

### 1. markdown-notes-tree (CLI, npm/GitHub — mistermicheels)
Walks a folder of Markdown notes and writes a linked tree into `README.md` files.

- **Title resolution is a documented hierarchy:** YAML front-matter attribute → the first
  level-1 heading in the file → the filename (only when a flag opts in).
- Sorting: filename order by default, title order behind a flag; a flag can list notes
  before subdirectories.
- Indentation is configurable (spaces count, or tabs).
- Descriptions: a subdirectory's single-paragraph description is pulled up into the parent
  tree entry.
- Ignores dot/underscore folders and non-Markdown files by default.
- Documented limit: Markdown links inside note titles are not supported (nested links).

### 2. Markdown Index Generator (browser tool, netlify — xmp-er)
Upload a Markdown file, get the document back with a generated TOC of all headings plus a
back-link to the TOC after each heading. Controls are minimal: upload, reset, copy result.
No level range, no ordered/unordered switch, no per-note metadata. Single document only.

### 3. Index Notes (Obsidian plugin)
Builds index blocks inside a note from **hierarchical tags** rather than folders.

- A note tagged `#projects/university` is collected under a `#projects` index.
- Nested list output mirrors the tag hierarchy.
- A priority tag (default `#top`) floats marked notes to the top in bold.
- Title comes from a note metadata property when present.
- Tag names are humanised for section headings (underscores → spaces, an underscore prefix
  marks an acronym); excluded folders are configurable.

## Table stakes → our decision

| Table stake (seen in ≥1 competitor) | Decision | Where |
|---|---|---|
| Title from front matter, else first heading, else fallback | **in-model, built** | `title:` front-matter key → first ATX heading → file marker name → `Untitled note N` |
| Multiple notes in one run, one index out | **in-model, built** | `split` = `heading` / `hr` / `file-marker` |
| Linked table of contents over the notes | **in-model, built** | `include_toc` (default on), GitHub-style anchor slugs with de-duplication |
| Per-note heading outline, depth-limited | **in-model, built** | `heading_depth` 0–6, default 2 |
| Tag collection + tag-grouped index | **in-model, built** | front-matter `tags`/`tag` (inline, flow, and block-list YAML) + inline `#tags`; `group_by = tag` |
| Sort by title instead of input order | **in-model, built** | `sort` = `input` / `title` / `words` |
| Wiki-link output for vault users | **in-model, built** | `link_style` = `anchor` / `wiki` / `none` |
| Per-note stats (word/heading counts) | **in-model, built** | `include_stats` (default on) |
| Machine-readable index for tooling | **in-model, built** (beyond all three) | `format` = `markdown` / `json` / `csv` |
| Preset one-click starting points | **in-model, built** | four `[[example]]` chips on the page |
| Walk a folder tree on disk / write files back | **out-of-model** | gizza blocks are browser-local and never touch the filesystem; the equivalent here is pasting the notes with a `=== path/note.md ===` marker per note |
| Live re-index while typing inside an editor | **out-of-model** | requires an editor plugin host (Obsidian); our surface is a page/CLI/chat tool |
| Priority/pin tag that bolds a note to the top | **considered, rejected** | a second tag-semantics knob for one competitor's convention; `sort` + `group_by` already cover ordering without extra schema |
| Configurable indentation (tabs / N spaces) | **considered, rejected** | the output is Markdown lists, where 2-space nesting is the portable convention; a knob here only creates broken-nesting footguns |
| Rewriting the input document with back-links to the TOC | **considered, rejected** | that is document rewriting, not indexing — and `blocks/toc-generator` already covers single-document TOCs |

## Neighbouring gizza blocks (dup check)

- `blocks/toc-generator` — TOC of the headings of **one** document. No notes, no tags, no
  per-note metadata.
- `blocks/notes-to-html-export` — bundles notes into one self-contained **HTML export with the
  rendered content**. This tool emits a metadata **index** (titles, tags, outlines, counts) and
  never reproduces note bodies, in Markdown/JSON/CSV.
- `blocks/markdown-query` — extracts one element class (headings/links/…) from one document.
- `blocks/markdown-section-extractor` — pulls a single section out of one document.

Distinct capability confirmed: multi-note metadata cataloguing with tag grouping. Not a duplicate.

## Stated limits (also on the page)

- Headings are recognised in ATX form (`#` … `######`); setext underlines are not treated as
  headings. Fenced code blocks are skipped so `#` comments inside code are never indexed.
- Up to 500 notes per run; the cap is reported as an error, not silently truncated.
- Front matter must be a `---` fenced YAML block at the very start of a note; only `title`,
  `tags` and `tag` are read.

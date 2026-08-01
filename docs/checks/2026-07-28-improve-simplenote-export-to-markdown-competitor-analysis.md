# Competitor analysis — simplenote-export-to-markdown (2026-07-28)

Tool function: convert a Simplenote (or Evernote) **JSON export** into a bundle of clean
Markdown files with titles, tags, and dates. Built as a pure-Rust chat/CLI/page tool. All copy
below is **paraphrased** from public docs/source — no competitor copy, branding, or trademarks
reproduced. Out-of-model items are listed, not built.

## Competitors reviewed (top 3 real, reachable tools)

### 1. philgyford/simplenote-to-obsidian (Python CLI)
- **Filename:** first line of each note becomes the filename; numeric suffix appended on
  collisions. No date prefix, no slugging.
- **Tags:** converted to Obsidian-style `#hashtags`; non-word chars (space, `:`, `.`) replaced
  with `-`; placement configurable start-of-note or end-of-note (default: end).
- **Dates:** carries the original note's creation/last-modified timestamps onto the *filesystem*
  file times (creation time is macOS-only). Not written into note body.
- **Skips:** trashed notes, pinned flag, inter-note links.
- **Options:** `OUTPUT_DIRECTORY`, keep-original-created/modified-time toggles (constants);
  interactive tag-placement prompt.

### 2. mayo/sn2md (JS gist — Simplenote JSON → Markdown files)
- **Filename:** first line of content, truncated to ~60 chars at a word boundary, sanitized to a
  safe character set.
- **Title:** first line emitted as a `#` Markdown heading at the top of the file.
- **Tags:** appended at the bottom as a `TAGS:` comma-separated line; note id emitted as a `KEY:`
  line.
- **Fields used:** `content`, `tags`, `key`, `createdate`/`modifydate` (legacy field names).
- **Options:** input JSON path + output dir only.

### 3. Notesnook Simplenote importer (web upload)
- Consumes the exported `.zip`, claims to preserve formatting/indentation.
- Notes the **markdown flag** caveat: if Markdown was enabled in Simplenote, the importer applies
  Markdown→HTML rules which can produce odd whitespace — recommends disabling Markdown before
  export for plaintext fidelity.
- Server-side app import (not a local converter).

## Format facts established
- **Modern Simplenote export** (`simplenote.json`): a top-level object with `activeNotes` and
  `trashedNotes` arrays. Each note: `id`, `content` (title = first non-empty line, no separate
  title field), `tags` (string array), `creationDate`/`lastModified` (ISO-8601 strings),
  `pinned`, `markdown`, `collaboratorEmails`.
- **Legacy Simplenote export**: flat note objects with `key`, `createdate`, `modifydate`
  (numeric epoch seconds), `content`, `tags`.
- **Evernote JSON** (third-party exporters): typically an array of note objects that DO carry an
  explicit `title`, plus `content`/`text`/`body`, `tags`, `created`/`updated`.

## Gap list + decisions (every table-stake lands in the descriptor or is listed out-of-model)

| Table-stake | Decision | Where |
| --- | --- | --- |
| Filename from title/first line, slugged, length-capped, collision-safe | in-model | core slug + `filename_style` |
| `YYYY-MM-DD` date-prefixed filenames | in-model | `filename_style = date-title` (default) |
| Title-only / id-based filenames | in-model | `filename_style = title` / `id` |
| Title as `# heading` | in-model | all metadata modes emit the title |
| Tags as YAML frontmatter list | in-model | `metadata = frontmatter` |
| Tags as `#hashtags` (non-word → `-`) | in-model | `metadata = inline` |
| Preserve created/modified dates (ISO) | in-model | frontmatter `created`/`updated` |
| Legacy numeric epoch dates | in-model | pure civil-date conversion (no dep) |
| Include/exclude trashed notes | in-model | `include_trashed` (default false) |
| Pinned + markdown flags surfaced | in-model | frontmatter `pinned: true` / `markdown: true` |
| Explicit Evernote `title` field honored | in-model | title fallback chain |
| One file per note (real folder tree / ZIP) | out-of-model | text surface — emit a labeled bundle with a `==== filename ====` header per file |
| Set filesystem file create/modified times | out-of-model | browser sandbox has no filesystem time API |
| Inter-note `[[wikilink]]` resolution | out-of-model | requires whole-vault graph + target-app link syntax |
| Direct import into Obsidian/Joplin/Notesnook | out-of-model | needs the destination app/account |

## UX control patterns matched
- Fixed-choice params (`metadata`, `filename_style`) are `Param::enumv` → `<select>` with friendly
  `[input.labels]`.
- `include_trashed` is a boolean checkbox.
- `[[example]]` preset chips for the two headline shapes (modern Simplenote `activeNotes`, and a
  tag/pinned note) double as worked examples.
- Multi-line `<textarea>` for the pasted JSON export.

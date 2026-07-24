# enex-to-markdown — competitor analysis (2026-07-23)

Function: convert an Evernote **ENEX** export (`.enex`, an XML file whose `<note>`
entries carry ENML/HTML bodies, `<tag>`s, and base64 `<resource>` attachments)
into clean Markdown.

Competitors scanned (paraphrased — no copy/branding reproduced):

| Tool | Shape | Notes |
| --- | --- | --- |
| Yarle (akosbalasko/yarle) | CLI + Electron GUI | The most complete converter: per-note Markdown files, YAML frontmatter, hashtag OR nested tags, note metadata (created/updated, source URL, notebook, GPS), resources written to a `_resources/` subfolder, Obsidian/LogSeq/Tana output presets, color→highlight. |
| evernote2md (wormi4ok) | CLI | Directory in → directory of `.md` out; attachments split into `image/` and `file/` dirs with rewritten links; optional YAML frontmatter (dates, tags); `--tagTemplate` controls tag formatting. |
| evernote-to-obsidian / assorted scripts | CLI/scripts | Thin wrappers: ENML→Markdown body, tags as hashtags, dates in frontmatter, attachments dumped alongside. |

## Table-stakes → decisions

- **ENML/HTML body → Markdown** (headings, links, lists, code, tables, emphasis) — **in-model**, via the already-proven `htmd` crate (html5ever), same engine `html-to-markdown`/`epub-to-markdown` use. The ENML `<en-note>` wrapper + DOCTYPE parse fine as HTML.
- **Note title as `#` heading** — **in-model**.
- **Tags** (hashtag style default, or frontmatter) — **in-model**: `metadata` param controls placement.
- **Created / updated dates** — **in-model**: Evernote's `YYYYMMDDThhmmssZ` stamps are re-printed as ISO-8601.
- **Source URL** metadata — **in-model** (from `<note-attributes><source-url>`).
- **Attachments / resources** — competitors WRITE binary files to a folder. This tool has a single text output surface (chat/CLI/page), so it **reports** each attachment (filename, MIME, decoded size) as a per-note list rather than emitting the binary. Extracting the base64 payloads to files is **out-of-model** for a single-text-output tool (documented as a limitation).
- **Plain-text vs Markdown output** — **in-model**: `format` param (`markdown`/`text`), mirroring `epub-to-markdown`.
- **Multiple notes in one ENEX** — competitors emit one file per note; this tool concatenates all notes into one Markdown document with `---` rules between them (**in-model**; noted in copy).

## Out-of-model (listed, not built)

- Writing attachments/resources out as real image/PDF files into a folder tree (no filesystem/zip output surface for this text tool).
- Obsidian/LogSeq/Tana-specific link syntaxes and platform presets (proprietary target formats).
- GPS/location and notebook-name frontmatter (minor; source URL + dates + tags cover the common case).
- `<en-todo>` checkbox → `- [ ]` task conversion (ENML checkbox state is dropped by the HTML→Markdown pass; documented).

## UX / controls

- `enex` — multiline textarea (paste the `.enex` XML).
- `format` — select (Markdown / Plain text).
- `metadata` — select (Frontmatter / Inline / None) for how title-dates-tags-source render.
- `attachments` — checkbox (list attachments per note), default on.
- `[[example]]` preset chip prefilling a small sample ENEX so the page has a one-click demo.

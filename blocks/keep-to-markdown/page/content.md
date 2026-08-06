## What this tool does

Google Takeout hands your Keep notes back as a folder of one `.json` (and one
`.html`) file per note — machine-readable, but not something you can read or drop
into a notes app. This tool turns that export into **Markdown**: one clean `.md`
note per Keep note, with the labels, the checkboxes, the dates and the
pinned/archived flags all preserved.

Everything runs locally in WebAssembly. Your notes are never uploaded, and the
page keeps working offline once it has loaded.

## What you paste in

The input format is auto-detected:

- **One note's JSON** — the contents of a `Takeout/Keep/<note>.json` file
  (`title`, `textContent` or `listContent`, `labels`, `createdTimestampUsec`, …).
- **A JSON array of notes** — paste several notes as `[ {...}, {...} ]` to convert
  a whole batch in one go.
- **The Keep HTML export** — the `Takeout/Keep/<note>.html` file, including its
  checklist glyphs and label chips.

## What you get

Each note becomes one Markdown file in the output, preceded by an
`==== filename.md ====` header so you can see — and split out — every file:

- A `# Title` heading (or, for untitled notes, a filename taken from the first line).
- Checklist items as `- [ ]` / `- [x]` **task list** items.
- **Labels**, as a YAML `labels:` list or as `#hashtags`.
- **Created / last-edited timestamps**, converted from Keep's microsecond values
  to ISO‑8601 UTC.
- The `pinned`, `archived` and note `color` flags, when set.
- A Markdown link per attachment, and the note's link chips as a link list.

## Options

| Option | Choices | What it does |
| --- | --- | --- |
| **Metadata** | `frontmatter` (default), `inline`, `none` | `frontmatter` writes a YAML block with title, dates, labels and flags. `inline` writes no YAML and appends the labels as `#hashtags`. `none` emits just the heading and body. |
| **Filename style** | `date-title` (default), `title`, `label-title` | `date-title` names files `2026-01-15-grocery-list.md`; `title` drops the date; `label-title` puts each note in a folder named after its first label (`shopping/grocery-list.md`, or `unlabeled/`). Duplicate names get a `-1`, `-2`, … suffix. |
| **Checklist items** | `task-list` (default), `bullet`, `plain` | `task-list` writes `- [ ] Milk` / `- [x] Eggs`; `bullet` writes `- Milk` and drops the checked state; `plain` writes one bare line per item. |
| **Include archived notes** | on (default) | Archived notes are exported and marked `archived: true`. Turn off to skip them. |
| **Include trashed notes** | off (default) | Trashed notes are skipped unless you turn this on. |
| **Link attachments** | on (default) | Writes `![photo.jpg](photo.jpg)` for image attachments and `[voice.3gp](voice.3gp)` for the rest. |

## Example

Paste this Keep Takeout note:

```json
{ "title": "Grocery List", "color": "BLUE", "isPinned": true,
  "labels": [{ "name": "Shopping" }, { "name": "Home" }],
  "createdTimestampUsec": 1768469400000000,
  "userEditedTimestampUsec": 1768557600000000,
  "listContent": [
    { "text": "Milk", "isChecked": false },
    { "text": "Eggs", "isChecked": true } ] }
```

…with **frontmatter** metadata, **date-title** filenames and **task-list**
checkboxes, and you get:

```
==== 2026-01-15-grocery-list.md ====
---
title: "Grocery List"
created: 2026-01-15T09:30:00Z
updated: 2026-01-16T10:00:00Z
labels: ["Shopping", "Home"]
pinned: true
color: BLUE
---

# Grocery List

- [ ] Milk
- [x] Eggs
```

## Limits and edge cases

- **Input size:** up to **4 MB** of pasted text per run. Convert a large Takeout
  folder in batches.
- **Attachments are links, not files.** Takeout stores photos and voice clips as
  separate files next to the notes; this page only ever sees the text you paste,
  so it writes the Markdown link and leaves the file where it is. Copy the Keep
  attachment files next to your `.md` notes and the links resolve.
- **No folder tree, no file times.** The browser has no filesystem, so the output
  is one labeled text bundle; `label-title` encodes the folder in the header name
  rather than creating directories, and original file timestamps cannot be set.
- **`.zip` / `.tgz` Takeout archives are not accepted** — unzip the download and
  paste the note files (or the whole array of them).
- **Nested checklist items flatten.** Keep's export does not record checklist
  indentation, so sub-items come out at the top level.
- **HTML export dates are local.** Keep writes the heading date without a time
  zone; it is carried through as-is with a `Z` suffix. The JSON export's
  microsecond timestamps are true UTC.

## FAQ

<details>
<summary>Where do I get the Google Keep Takeout export?</summary>

Go to Google Takeout, deselect everything, tick **Keep**, and export. The
download contains a `Takeout/Keep/` folder with one `.json` and one `.html` file
per note (plus any attachment files). Open a note's `.json` and paste its
contents here — or paste several notes as a JSON array.

</details>

<details>
<summary>Do checkboxes survive the conversion?</summary>

Yes. Keep stores a checklist as `listContent` entries with an `isChecked` flag,
and the HTML export uses ☐/☑ glyphs. Both become Markdown task list items —
`- [ ]` for open items and `- [x]` for ticked ones — which Obsidian, Logseq,
Joplin, GitHub and VS Code all render as real checkboxes.

</details>

<details>
<summary>What happens to my labels?</summary>

With **frontmatter** metadata they become a YAML `labels: ["Shopping", "Home"]`
list. With **inline** they are appended to the note as `#shopping #home`. And
with the **label-title** filename style the first label also becomes the note's
folder, so `Shopping` notes land under `shopping/`.

</details>

<details>
<summary>Can I convert my whole Keep export at once?</summary>

Paste the notes as a JSON array — `[ {note1}, {note2}, … ]` — and every note is
converted in one run, each with its own `==== filename.md ====` header. The 4 MB
input cap keeps very large exports to a few batches. Notes with the same title
get a `-1`, `-2` suffix so nothing is overwritten.

</details>

<details>
<summary>Is my data private?</summary>

Yes. The conversion runs entirely in your browser with WebAssembly — the notes
never leave your device, there is no sign-up, and the page keeps working with the
network switched off once it has loaded.

</details>

<details>
<summary>How do I turn the output into separate files?</summary>

Each note in the output starts with an `==== filename.md ====` header. Copy the
text under a header into a new file with that name. The `date-title` style keeps
your notes in chronological order when sorted by filename; `label-title` gives
you the folder path to create.

</details>

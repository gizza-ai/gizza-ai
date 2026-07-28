## What this tool does

Simplenote lets you export all of your notes as a single JSON file
(`simplenote.json`). That file is great for backup but useless for reading, and
most note apps — Obsidian, Joplin, VS Code, a plain folder of files — expect one
**Markdown** file per note. This tool does that conversion, entirely in your
browser: paste the JSON and get a bundle of clean `.md` files, one per note, each
with a title heading, its tags, and its dates.

Nothing is uploaded. The conversion runs locally in WebAssembly, works offline
once the page has loaded, and needs no sign-up.

## What you get

Each note becomes one Markdown file in the output, preceded by an
`==== filename.md ====` header so you can see — and split out — every file:

- A `# Title` heading taken from the note's title (or its first line).
- Its **tags**, either as a YAML `tags:` list or as `#hashtags`.
- Its **created / updated dates**, in the frontmatter.
- The note's `pinned` / `markdown` flags, when set.

## Options

| Option | Choices | What it does |
| --- | --- | --- |
| **Filename style** | `date-title` (default), `title`, `id` | `date-title` names files `2026-01-15-grocery-list.md`; `title` drops the date; `id` uses the note's own id/key. Duplicate names get a `-1`, `-2`, … suffix. |
| **Metadata** | `frontmatter` (default), `inline` | `frontmatter` writes a YAML block with the title, created/updated dates, tags, and pinned flag. `inline` writes no frontmatter and appends the tags as `#hashtags` at the bottom. |
| **Include trashed notes** | off (default) | When on, Simplenote's `trashedNotes` (and notes flagged deleted) are exported too. |

## Supported export formats

- **Modern Simplenote export** — a JSON object with `activeNotes` and
  `trashedNotes` arrays. Each note carries `content`, `tags`,
  `creationDate` / `lastModified`, and `pinned`.
- **Legacy Simplenote export** — a JSON array of note objects with `key`,
  `content`, `tags`, and numeric `createdate` / `modifydate` epoch times (converted
  to real dates for you).
- **Evernote-style JSON** — a JSON array where each note has an explicit `title`
  plus `content` / `text` / `body`, `tags`, and `created` / `updated`.

## Example

Paste this Simplenote export:

```json
{ "activeNotes": [
  { "id": "abc-123", "content": "Grocery List\nMilk\nEggs",
    "tags": ["home", "shopping list"],
    "creationDate": "2026-01-15T09:30:00.000Z",
    "lastModified": "2026-01-16T10:00:00.000Z", "pinned": true } ] }
```

…with **date-title** filenames and **frontmatter** metadata, and you get:

```
==== 2026-01-15-grocery-list.md ====
---
title: "Grocery List"
created: 2026-01-15T09:30:00.000Z
updated: 2026-01-16T10:00:00.000Z
tags: ["home", "shopping list"]
pinned: true
---

# Grocery List

Milk
Eggs
```

## FAQ

<details>
<summary>Where do I get the Simplenote export JSON?</summary>

In the Simplenote web or desktop app, open **Settings → Tools → Export Notes**.
That downloads a `.zip` containing `notes/simplenote.json`. Open that file and
paste its contents here.

</details>

<details>
<summary>Is my data private?</summary>

Yes. The conversion runs entirely in your browser with WebAssembly — your notes
never leave your device, and the page keeps working offline once loaded.

</details>

<details>
<summary>How do I turn the output into separate files?</summary>

Each note in the output starts with an `==== filename.md ====` header. Copy the
text under a header into a new file with that name. The `date-title` filename
style keeps your notes in chronological order when sorted by name.

</details>

<details>
<summary>Does it work for importing into Obsidian?</summary>

Yes — use `frontmatter` metadata so tags land in a YAML `tags:` list Obsidian
understands, or `inline` to get `#hashtags` in the note body. This tool produces
the files; you still copy them into your vault folder yourself.

</details>

<details>
<summary>What are the limits?</summary>

The browser has no filesystem, so this tool cannot set each file's
creation/modified **file time** or write a real folder tree — it emits one labeled
text bundle instead. Inter-note `[[wikilinks]]` are left as-is, since resolving
them needs your whole target vault.

</details>

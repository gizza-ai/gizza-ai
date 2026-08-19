## About this tool

A folder of Markdown notes is only as useful as the note that points at the rest of them.
This tool builds that note for you: paste your notes into the box one after another and it
splits them apart, works out each note's title, collects its tags, reads its heading outline
and counts its words, then renders the whole set as one linked index — the "map of content"
a vault, a docs folder or a pile of meeting notes is usually missing.

Everything runs as WebAssembly inside this page. Your notes are never uploaded, and the
output never reproduces a note's body — only its title, tags, outline and counts.

### A worked example

Paste this bundle, leave every option at its default, and press nothing else:

```markdown
# Getting started

Some intro text.

## Install

Run it.

# Weekly review

Notes about the week.
```

You get back:

```markdown
# Notes index

2 notes · 0 tags · 14 words

## Contents

1. [Getting started](#getting-started)
2. [Weekly review](#weekly-review)

## Getting started

8 words · 1 heading

- Install

## Weekly review

6 words · 0 headings
```

The first `# ` heading of each note became its title and its anchor link; the deeper `##
Install` heading stayed in that note's outline; the counts came from the note bodies.

### Where notes are split

- **Heading** (default) — a new note starts at every top-level `# ` heading. This is what a
  concatenated pile of notes usually looks like.
- **Thematic break** — a new note starts at every `---`, `***` or `___` line. A `---` that
  opens a note's YAML front matter is *not* treated as a break, so front matter stays with
  its note.
- **File markers** — a new note starts at every `=== notes/todo.md ===` or
  `==> notes/todo.md <==` banner, the shape `head`/`tail` print when they dump several files
  at once. Each note then remembers its path, so links point at the real files
  (`[Today](notes/todo.md#today)`) and a note with no heading falls back to its file name.

### Titles, tags and links

A note's title is the first of these that exists: a `title:` key in its YAML front matter,
its first ATX heading, its file name from a file marker, then `Untitled note 3` as a last
resort. Tags come from front-matter `tags`, `tag` or `keywords` — inline (`tags: a, b`),
flow (`tags: [a, b]`) or block-list form — plus any `#tag` written in the body, which you
can switch off. Duplicate titles get distinct anchors (`#log`, `#log-1`), so every link in
the Contents list lands somewhere different.

Choose **anchor** links for a self-contained index note, **wiki** links (`[[Note title]]`,
`[[Note title#Heading]]`) for an Obsidian-style vault, or **plain text** for somewhere
without link targets. Switch the output to **JSON** to feed the index into another script,
or **CSV** to open it in a spreadsheet.

### Limits and edge cases

- Headings must be ATX style (`#` through `######`). Setext underlines (`Title` followed by
  `=====`) are not recognised as headings.
- Headings inside ``` or `~~~` code fences are skipped, so a `# comment` in a shell snippet
  never lands in your outline.
- Up to **500 notes** per run. Over that you get a clear error rather than a silently
  truncated index — split the bundle and index it in batches.
- Front matter must be a `---` fenced block at the very start of a note, and only `title`,
  `tags`/`tag` and `keywords` are read from it. Everything else is ignored, not an error.
- Inline tags need at least one letter, so issue references like `#1234` stay out of your
  tag list, while `#build/ci` is kept whole.
- Words are whitespace-separated tokens containing at least one letter or digit, so bare
  Markdown punctuation (`-`, `>`, `|`) is not counted as a word.

## FAQ

<details>
<summary>How do I get my notes into the box in the first place?</summary>

Concatenate them. On macOS or Linux, `cat notes/*.md` gives you a bundle you can split on
headings or thematic breaks, and `head -n -0 notes/*.md` (or `tail -n +1 notes/*.md`) adds
`==> notes/todo.md <==` banners, which is exactly what the file-marker split expects — that
way the index links back to the real file paths. This tool never reads your disk itself.

</details>

<details>
<summary>Why is my whole bundle showing up as one note?</summary>

The split boundary did not match your input. With the default heading split, every note has
to start with a top-level `# ` heading — if your notes start at `##`, switch to the thematic
break or file-marker split, or promote the headings first. If you picked file markers and
your input has none, you get an explicit error naming the marker shapes rather than one
giant note.

</details>

<details>
<summary>Can I keep the outline but drop the word counts?</summary>

Yes. "Include word and heading counts" controls the totals on the summary line and the
per-note counts; the heading outline is controlled separately by the outline depth slider.
Set the depth to 0 for titles and tags only, or turn the counts off for a clean index that
still lists every heading. In CSV output, turning counts off drops the headings and words
columns.

</details>

<details>
<summary>Does a note with several tags appear more than once?</summary>

In the tag-grouped Contents list, yes — a note tagged `#planning` and `#roadmap` is listed
under both, which is the point of a tag index. The per-note sections below it appear exactly
once each. Notes with no tags collect under an "Untagged" subsection at the end, and tags are
matched case-insensitively, so `#Planning` and `#planning` are the same tag.

</details>

<details>
<summary>Is anything sent to a server?</summary>

No. The index is built by a WebAssembly module running in this page, so your notes stay in
the browser tab. You can load the page, disconnect from the network, and it still works. The
same logic is available offline in the command-line tool if you would rather script it.

</details>

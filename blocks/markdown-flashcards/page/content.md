## What this tool does

Paste **Markdown notes** and get back a **flashcard deck file you can import into
Anki** — or into any app that reads tab/comma-separated cards (Quizlet, RemNote,
Mochi, Memrise). It runs entirely in your browser: nothing is uploaded, it works
offline once loaded, and there's no account.

The parser understands the shapes real notes are already written in:

| Card format | What it reads |
| --- | --- |
| **Headings** | `## Question` on one line, the text underneath is the answer. Nested headings become the answer's context. |
| **Q: / A: blocks** | `Q: …` starts a card, `A: …` starts its answer (multi-line answers welcome). `Question:` / `Answer:`, `**Q:**` and bulleted variants work too. |
| **One card per line** | `term :: definition`, `term - definition`, `term => definition`, `term; definition`, `term: definition` or a tab. Bullets and numbering are stripped. |
| **Table rows** | `\| question \| answer \|` rows; the `\| --- \|` row and the header above it are dropped, and an optional **third column becomes tags**. |
| **Cloze** | Every `**bold**` or `==highlighted==` span becomes `{{c1::…}}`, `{{c2::…}}`, … on one cloze note. |

**Auto-detect** picks the format for you (and tells you which one it chose in the
*Readable preview* output), so most notes convert with a single paste.

## Worked example

Input — Markdown notes:

```
## What is mitosis?
Cell division that makes **two identical** cells.

## What is meiosis?
Cell division that makes gametes.
```

Output — the Anki import file (tab-separated, HTML fields):

```
#separator:Tab
#html:true
#notetype:Basic
#columns:Front	Back
What is mitosis?	Cell division that makes <b>two identical</b> cells.
What is meiosis?	Cell division that makes gametes.
```

Save that as a `.txt` file and use **File → Import** in Anki. The `#` lines are
Anki's own header directives, so the separator, note type, deck and tags are set
for you — you don't have to configure the import dialog by hand.

A vocabulary list works the same way. `gato :: cat` on each line, deck name
`Spanish::Week 1`, and every line becomes one card in that subdeck.

## Options

| Option | What it does |
| --- | --- |
| **Card format** | Auto-detect (default), or force headings / one-card-per-line / `Q:`-`A:` / table / cloze. |
| **Line delimiter** | One-card-per-line mode only. `auto` picks whichever of `::`, `=>`, tab, `\|`, `;`, ` - `, `:` splits the most lines; or pass a name (`tab`, `colon`, `dash`, `arrow`, …) or any literal text. |
| **Heading level** | `0` (default) auto-picks the level with the most answers under it; `1`–`6` pins `#`…`######` as the question. |
| **Field separator** | `Tab` (default — what Anki recommends), `Comma`, `Semicolon` or `Pipe`. Fields containing the separator, a quote or a newline are quoted RFC-4180 style. |
| **Field formatting** | `HTML` (default) converts bold, italics, `code`, links, lists and fenced code blocks to HTML and line breaks to `<br>`; `Keep raw Markdown` leaves the source untouched; `Plain text` strips all markup. |
| **Note type / Deck / Tags** | Written as `#notetype:`, `#deck:` and `#tags:` header lines. Use `::` in a deck name for a subdeck (`Biology::Cells`). |
| **Tag from heading path** | Heading mode: each card is tagged with its parent headings as one hierarchical tag, e.g. `Biology::Cell_Parts`. |
| **Include Anki #header lines** | On by default. Turn it off for a bare CSV/TSV for another app — deck-wide tags then move into each row's Tags column instead of being dropped. |
| **Drop duplicate questions** | On by default; keeps the first card when a question repeats (case-insensitive). |
| **Output** | The import file (default), a readable numbered preview (shows the detected format and card count), or JSON. |

## Limits & edge cases

- **Up to 1,000,000 characters and 5,000 cards** per run — past either limit you
  get a clear error asking you to split the notes, not a truncated deck.
- **Answers must have text.** A heading with nothing under it, a `Q:` with no
  `A:`, or a table row with an empty cell is skipped rather than exported blank.
- **Cloze needs emphasis.** In cloze mode a line with no `**bold**` or
  `==highlighted==` span produces no card; if nothing in the notes has emphasis
  you get an error explaining that.
- **HTML formatting is deliberately small** — bold, italic, `code`, links,
  images, bullet/numbered lists and fenced code blocks. Tables, blockquotes and
  nested lists inside an answer are kept as text, not rebuilt as HTML.
- **Import HTML on.** With HTML field formatting, tick *Allow HTML in fields* in
  Anki's import dialog (the `#html:true` header sets this automatically in recent
  Anki versions).
- **Images are referenced, not bundled.** `![alt](cell.png)` becomes
  `<img src='cell.png'>`; copy the file into Anki's `collection.media` folder
  yourself. This tool exports text, never a `.apkg` archive.

## FAQ

<details>
<summary>How do I import the result into Anki?</summary>

Copy the output (or use the download link), save it as a `.txt` file, then in
Anki choose **File → Import** and pick that file. The `#separator:`, `#notetype:`,
`#deck:` and `#tags:` header lines configure the import dialog for you, so you
normally just press Import.

</details>

<details>
<summary>Can it export a .apkg deck file?</summary>

No — a `.apkg` is a zipped SQLite collection, which a browser-local tool can't
build safely. The tab-separated text file this tool produces is Anki's own
documented import format and covers the same job: it carries the deck name, note
type, tags and HTML flag with it.

</details>

<details>
<summary>How do cloze deletions work?</summary>

Choose **Cloze from bold text**. Each `**bold**` or `==highlighted==` span in a
line becomes a numbered deletion, so `The **mitochondrion** is the **powerhouse**
of the cell.` exports as `The {{c1::mitochondrion}} is the {{c2::powerhouse}} of
the cell.` — one note with two cards. The note type is switched to `Cloze`
automatically.

</details>

<details>
<summary>My notes use `-` or `=>` between term and definition. Does that work?</summary>

Yes. In one-card-per-line mode the delimiter defaults to `auto`, which tries
`::`, `=>`, tab, `|`, `;`, ` - ` and `:` and uses whichever splits the most lines.
If your notes mix several, set **Line delimiter** explicitly — you can type a
name (`dash`, `arrow`, `tab`, `colon`) or any literal string.

</details>

<details>
<summary>Why did it pick the wrong card format?</summary>

Auto-detect prefers `Q:`/`A:` blocks, then tables, then whichever of headings or
line-splitting yields more cards — so a vocabulary list under a `# Title` still
converts line by line. When a document is ambiguous, set **Card format**
explicitly and, for headings, pin the **Heading level**.

</details>

<details>
<summary>Can I put every card in a specific deck with tags?</summary>

Yes. Fill in **Deck name** (use `::` for subdecks, e.g. `Biology::Cells`) and
**Tags** (space- or comma-separated). Both are written as header lines, so they
apply to every card in the file. In heading mode you can also tag each card with
its own heading path.

</details>

<details>
<summary>Is it free, and do my notes leave my device?</summary>

It's free and your notes stay local. The conversion runs in your browser through
WebAssembly — nothing is uploaded, there's no account, and the page keeps working
offline once it has loaded.

</details>

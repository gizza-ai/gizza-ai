## Turn Markdown into a slide deck

Write your talk as plain Markdown, separate each slide with a line containing
just `---` (a Markdown thematic break), and this tool builds a single
**self-contained HTML file** you can open in any browser or hand to a colleague.
No reveal.js download, no Node build step, no account — the deck embeds its own
styling and navigation.

## How it works

1. Paste your Markdown. Put `---` (or `***` / `___`) on its own line wherever you
   want a new slide to begin. A document with no separators becomes a single
   slide.
2. Pick a **light** or **dark** theme and, optionally, a document title (the
   browser tab name).
3. Copy the generated HTML or save it as `deck.html` and open it.

## What the generated deck gives you

- **Keyboard navigation** — arrow keys, Page Up/Down, Space, Home/End.
- **Click and swipe** — tap the screen edges or swipe on touch devices.
- A **slide counter** and a **progress bar**, plus deep-links to a slide via the
  URL hash (`#3`).
- **Print to PDF** — your browser's print dialog lays one slide per page.

## Markdown support

Slides render with CommonMark plus GitHub-flavored extensions: headings, lists,
**bold**/*italic*, `inline code`, fenced code blocks, blockquotes, tables, task
lists, strikethrough, links and images. Slide content is sanitized, so a deck is
safe to share.

## Private by design

Everything runs locally in your browser via WebAssembly. Your Markdown is never
uploaded to a server.

## FAQ

<details>
<summary>What starts a new slide?</summary>

A thematic-break line — `---`, `***`, or `___` on its own line. Consecutive separators don't create empty slides (blank chunks are dropped), and a document with no separators becomes a one-slide deck. Note that a `---` directly under a line of text is Markdown for a heading, so leave a blank line before it.

</details>

<details>
<summary>Can I embed raw HTML, scripts, or iframes in a slide?</summary>

No — slide content is sanitized (via an HTML sanitizer) before it's embedded, so `<script>`, event handlers, and other active content are stripped. Standard Markdown — including tables, task lists, fenced code blocks, and images — comes through fine.

</details>

<details>
<summary>How do I get a PDF of the deck?</summary>

Save the generated HTML (e.g. as `deck.html`), open it, and use the browser's print dialog — the deck's print stylesheet lays out one slide per page, so "Save as PDF" gives you a shareable handout.

</details>

<details>
<summary>Does the deck need internet access or reveal.js to run?</summary>

Neither. The output is one self-contained HTML file with its CSS and a small vanilla-JS navigator embedded — no CDN fonts, no framework download, works offline and survives being emailed as an attachment.

</details>

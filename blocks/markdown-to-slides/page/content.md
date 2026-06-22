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

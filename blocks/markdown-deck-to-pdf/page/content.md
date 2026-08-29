## Turn a Markdown deck into a slide-per-page PDF

Write your talk as plain Markdown and download a PDF where **every slide is exactly one page** — the
shape people actually attach to an email, print as a handout, or drop into a shared drive. A thematic
break (`---`) always starts a new slide, and headings can split slides too. Pick **16:9, 4:3, A4 or
Letter landscape**, a **light or dark** theme, repeated **header/footer** text, **page numbers**, and
a **PDF bookmark per slide**. Everything runs in your browser with WebAssembly — your deck is never
uploaded to a server.

### Worked example

Input, with **Also split slides on** set to `# headings (H1)` and **Cover slide title** set to
`Q3 Business Review`:

```
# Quarterly Review

- Revenue up 24%
- Two new markets

# Next Steps

- Hire 3 engineers
- Ship v2
```

Output: a **3-page** `deck.pdf` at 960 × 540 pt. Page 1 is a centered cover reading *Q3 Business
Review*; page 2 is titled **Quarterly Review** with two bullets; page 3 is **Next Steps** with two
bullets. Each page carries `1 / 3`, `2 / 3`, `3 / 3` in the bottom-right corner, and the PDF opens
with a bookmark sidebar listing *Q3 Business Review*, *Quarterly Review*, *Next Steps*.

Nested lists indent, `**bold**` / `*italic*` / `` `code` `` change font, block quotes and tables
render, and a busy slide shrinks its body text (down to half the size you chose) so it still lands on
one page.

### Good to know

- Text uses the standard PDF base-14 fonts, so the file stays small and needs no font embedding.
  Characters outside Latin-1 (WinAnsi) render as `?`.
- A line of three or more `-`, `*` or `_` on its own is *always* a slide break — so it cannot double
  as a setext heading underline.
- If a slide still overflows after the automatic shrink-to-fit, it continues onto extra pages rather
  than silently dropping content.
- A deck is capped at **300 slides**, and the PDF at **8 MB**.
- Images are rendered as their alt text; there is no image embedding, custom CSS or math typesetting.

## FAQ

<details>
<summary>How do I control where one slide ends and the next begins?</summary>

Two ways, and they combine. A thematic break — a line of three or more `-`, `*` or `_` — **always**
starts a new slide, even in `none` mode. On top of that, **Also split slides on** chooses which
heading levels break: `# headings (H1)` starts a slide at every `#`, `## headings (H2)` at every `##`,
`# and ## headings` at both, and `Nothing — only --- breaks` turns heading splitting off entirely.
The heading that starts a slide becomes that slide's title; other headings render as sub-headings in
the body.

</details>

<details>
<summary>What happens if a slide has too much content to fit?</summary>

The body text is laid out at the size you chose, and if it does not fit the tool shrinks it in small
steps down to **half** that size. Only if it still overflows at the smallest size does the slide
continue onto extra pages, keeping the same title — content is never dropped. If you see continuation
pages, split that slide with a `---` or move some bullets.

</details>

<details>
<summary>Is this different from converting a Markdown document to PDF?</summary>

Yes. A document converter flows your text continuously onto portrait pages, breaking wherever the page
runs out. This tool treats the input as a **deck**: each slide gets its own fixed-size landscape page,
with the slide title at the top, so page 4 of the PDF is slide 4 of the talk. That one-to-one mapping
is what makes the PDF usable as a handout or an attachment.

</details>

<details>
<summary>What are the PDF bookmarks for?</summary>

With **PDF bookmarks** on (the default), the file gets an outline with one entry per slide, labelled
with that slide's heading (or `Slide 4` when a slide has no heading). Most viewers show it as a
clickable sidebar, so a long deck is navigable without scrolling. Turn it off for a plain PDF with no
outline.

</details>

<details>
<summary>Can I add a logo, background image, or custom fonts?</summary>

No. The tool renders with the standard base-14 PDF fonts and solid theme colours only — that is what
keeps it fast, offline and dependency-free. Images in your Markdown are rendered as their alt text
rather than embedded, and there is no custom CSS, theme file or math typesetting. If you need those,
build the deck in a full presentation tool.

</details>

<details>
<summary>Is my deck uploaded anywhere?</summary>

No. The PDF is assembled entirely in your browser with WebAssembly — the Markdown never leaves your
device and nothing is sent to a server. The same conversion is available from the command line for
scripts and CI.

</details>

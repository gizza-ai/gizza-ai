## Turn a Markdown outline into a real PowerPoint deck

Write your talk as a plain Markdown outline, and download a genuine binary `.pptx` file — the same
format PowerPoint, Keynote, Google Slides and LibreOffice Impress open natively. Headings become
**slide titles**, list items and paragraphs become **bullets**, and a thematic break (`---`) starts a
new slide. Pick where slides split, a **light or dark** theme, and a **16:9 or 4:3** aspect ratio.
Everything runs in your browser with WebAssembly — your outline is never uploaded to a server.

### Worked example

Input (with **Split slides on** set to `# headings`):

```
# Quarterly Review

- Revenue up 24%
- Two new markets

# Next Steps

- Hire 3 engineers
- Ship v2
```

Output: a two-slide `presentation.pptx`. The first slide is titled **Quarterly Review** with two
bullets; the second is **Next Steps** with two bullets. Add a **Deck title** to prepend a centered
title slide, switch to the **dark** theme for light-on-black slides, or choose **4:3** for the classic
standard shape.

Nested list items indent into sub-bullets, `**bold**`, `` `code` `` and `[links](url)` are flattened
to clean text, and fenced code blocks are kept as verbatim lines — so what you paste is what lands on
the slide.

## FAQ

<details>
<summary>Is this a real .pptx file, or just text renamed to .pptx?</summary>

It is a real, standards-compliant Office Open XML (`.pptx`) presentation — a ZIP of XML parts
(`[Content_Types].xml`, a presentation part, a slide master, layout and theme, and one slide XML per
slide), exactly what PowerPoint writes. It opens without any "the file format doesn't match the
extension" warning and is fully editable in PowerPoint, Keynote, Google Slides and LibreOffice Impress.

</details>

<details>
<summary>How does my Markdown become slides?</summary>

Each split heading starts a new slide and its text becomes the slide **title**; list items (`-`, `*`,
`+`, or `1.`) and plain paragraphs become **bullets** on the current slide. A thematic break — a line
of three or more `-`, `*`, or `_` — always starts a new slide, even without a heading. Indented list
items become nested sub-bullets (up to four levels deep).

</details>

<details>
<summary>What does "Split slides on" control?</summary>

It chooses which heading levels cut the outline into slides. **# headings (H1)** starts a new slide at
each `#` and keeps `##` headings as bullets; **## headings (H2)** splits on each `##`; **# and ##**
splits on both. Whichever you pick, a `---` thematic break always forces a new slide — handy for decks
with no headings at all.

</details>

<details>
<summary>Can I change the theme, aspect ratio, or add a title slide?</summary>

Yes. The **Light** theme is dark text on a white background; **Dark** is light text on a near-black
background. **16:9** is modern widescreen and **4:3** is the classic standard shape. Filling in the
optional **Deck title** adds a centered title slide at the front and records the title in the
document's properties.

</details>

<details>
<summary>Is my content uploaded anywhere?</summary>

No. The presentation is assembled entirely in your browser with WebAssembly — the outline never leaves
your device, and nothing is sent to a server. Very large outlines are bounded only by your device's
memory.

</details>

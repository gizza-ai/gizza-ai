## About this tool

This converter packages an article you already have — the text you drafted, or
the cleaned HTML you saved from a reader view — into a valid **EPUB 3** ebook
you can side-load onto a Kindle, Kobo, Apple Books, Calibre or any other reader.

Everything happens in your browser. The article is never uploaded, there is no
account, and the file you download is built locally by a small WebAssembly
module.

### What you get

Every book contains a complete OCF container: the `mimetype` entry stored first
and uncompressed as the spec requires, `META-INF/container.xml`, a `content.opf`
package document carrying your metadata, an **EPUB 3 navigation document** *and*
an **EPUB 2 `toc.ncx`** so older readers still show the table of contents, a
readable stylesheet, and one XHTML file per chapter.

### Worked example

Paste this as plain text, set the title to `Field Notes` and leave the chapter
split on “Heading 1”:

```
# Tide pools

At low tide the rocks turn into a museum.

- Ochre sea stars
- Green anemones

# Getting there

Park early; the lot fills by nine.
```

You get `field-notes.epub`: two chapters (“Tide pools” and “Getting there”), a
two-entry table of contents, the bullets as a real `<ul>` list, and 24 words
counted in the summary line above the download button.

### Reading messy HTML

Pasted article HTML is rarely tidy. Unclosed `<p>` tags, a stray `</div>`, a
leftover `<script>` block or a raw `&nbsp;` will all make a strict reader refuse
to open a book. This converter re-emits your markup as **well-formed XHTML**:
tags are balanced with a proper stack, scripts, styles, forms and embeds are
dropped along with their contents, unsafe links are unwrapped (their text is
kept), and named entities such as `&nbsp;`, `&mdash;` and `&eacute;` become real
characters.

The tags that survive are the ones books are made of: headings, paragraphs,
lists, blockquotes, preformatted blocks, tables, and inline emphasis, code,
sub/superscript and `http(s)`/`mailto` links.

### Plain text and Markdown-ish input

In text mode, blank lines separate paragraphs, wrapped lines are joined back
into flowing prose, and a few Markdown conventions are recognised: `#` to `######`
headings, `-`, `*`, `+` or `•` bullets, `1.` numbered items, and `---` for a
horizontal rule. Inline Markdown (`**bold**`, `[links](…)`) is *not* converted —
it stays as literal text.

### Limits and behaviour worth knowing

- Up to **2,000,000 characters** of input and **2,000 chapters** per book.
- **Images are not packaged.** A pasted `<img>` points at a remote file this
  offline converter can't embed, so images are dropped and their `alt` text is
  kept inline as `[Image: …]`. The count is reported with the download.
- Output is **deterministic**: the same article and metadata always produce a
  byte-identical file.
- Cover images, embedded fonts and MOBI/AZW3 output are out of scope here.

## FAQ

<details>
<summary>Is the EPUB actually valid, or just a renamed ZIP?</summary>

It is a real OCF container. The `mimetype` entry is written first and stored
uncompressed, `META-INF/container.xml` points at an EPUB 3 `content.opf`, and the
book ships both an EPUB 3 `nav.xhtml` and an EPUB 2 `toc.ncx`. Every chapter is
well-formed XHTML with a declared language, which is what strict readers check
before they will open a file.

</details>

<details>
<summary>How do chapters and the table of contents get decided?</summary>

By the **Start a new chapter at** setting. With `Heading 1` (the default) every
`<h1>` — or `# ` line in text mode — begins a new chapter, and the heading text
becomes that chapter's entry in the table of contents. Choose `Heading 2`,
`Heading 1 and 2` for a finer split, or `No split` to keep the whole article in a
single chapter. Content that appears before the first heading becomes the opening
chapter, titled after the book.

</details>

<details>
<summary>What happens to images in my article?</summary>

They are removed, and any `alt` text is kept in the flow as `[Image: description]`.
An `<img>` in a pasted article points at a URL on someone else's server; a
browser-local converter can't download and embed those bytes, and an EPUB that
links to remote images shows blank frames on most e-readers. The summary line
tells you how many images were dropped.

</details>

<details>
<summary>Do I have to fill in the title, author and language?</summary>

No. Leave the title empty and it is taken from the HTML `<title>` element, then
the first heading, then the first line of text — and it is also used for the
download filename. Author and publisher are simply omitted when blank. Language
defaults to `en`; set any BCP-47 tag such as `de`, `fr` or `pt-BR` so readers
hyphenate and read the book aloud correctly.

</details>

<details>
<summary>Should I set a base font size?</summary>

Usually not. Leaving it at `0` lets the reading app apply the reader's own font
preference, which is what e-reader users expect. Set a value in points (say
`12`) only when you are producing a book for a fixed setup — it is written into
the book's stylesheet, and most apps still let the reader override it.

</details>

<details>
<summary>Can it fetch an article straight from a URL?</summary>

Not here — this tool is deliberately offline and never makes network requests.
Copy the article text, or use a reader-view/extraction tool first and paste the
cleaned HTML. In chat you can chain a fetch or HTML-extraction skill into this
one.

</details>

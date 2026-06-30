## What this tool does

Paste a **Markdown** or **HTML** document and get back a ready-to-use **table of
contents** — a nested list of links built from the document's headings. It runs
entirely in your browser: nothing is uploaded, it works offline, and there's no
sign-up. Drop the result at the top of a README, a wiki page, a blog post, or any
long document so readers can jump straight to a section.

## How it works

Every heading becomes a linked entry. In Markdown that means ATX headings
(`#` through `######`) and setext headings (a line underlined with `===` or
`---`); in HTML it means `<h1>` through `<h6>` tags. Headings inside fenced code
blocks are ignored, and inline formatting (bold, italics, `code`, links) is
stripped from the link text.

Each entry links to a **GitHub-style anchor** of the heading text: lowercased,
punctuation removed, and spaces turned into hyphens — the same slug GitHub,
GitLab, and most static-site generators produce, so the links work out of the
box. Repeated headings get a unique `-1`, `-2`, … suffix. If an HTML heading
already has an `id`, that id is kept as the anchor so the link matches the real
element.

## Options

| Option | What it does |
| --- | --- |
| **Input format** | `auto` detects HTML when an `<h1>`–`<h6>` tag is present, otherwise reads Markdown. Force `markdown` or `html` when auto-detection guesses wrong. |
| **Output format** | `markdown` returns a nested `[text](#anchor)` list; `html` returns a nested `<ul>`/`<ol>` of `<a href="#anchor">` links. |
| **Min / Max heading level** | Limit which levels appear (1–6). Set the minimum to **2** to skip a single top-level title, or lower the maximum to **3** to keep the contents short. |
| **Numbered list** | Switch from bullets (`-` / `<ul>`) to a numbered list (`1.` / `<ol>`). |

## Example

Markdown input:

```markdown
# Getting Started
## Installation
## Usage
### Examples
```

Markdown table of contents:

```markdown
- [Getting Started](#getting-started)
  - [Installation](#installation)
  - [Usage](#usage)
    - [Examples](#examples)
```

## FAQ

**Is it free and private?** Yes — your document never leaves your device, and the
page keeps working offline once it has loaded.

**Does it work with both Markdown and HTML?** Yes. Leave the input format on
**auto** and it picks the right parser, or set it explicitly if your document
mixes both.

**Do the anchor links actually work?** They use the same GitHub-style slug that
GitHub, GitLab, and common Markdown renderers generate from heading text, so a
table of contents pasted into a README or rendered page links correctly. HTML
headings with an existing `id` keep that id.

**Can I leave out the top-level title?** Yes — set the minimum heading level to
**2** so the document's single `#` / `<h1>` title is skipped and the contents
start at the sections.

**Why did I get an error?** The document must contain at least one heading within
the chosen level range. Check the input format and the min/max levels if nothing
comes back.

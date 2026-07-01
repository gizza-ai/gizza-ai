## What this tool does

Paste a Markdown document, type the heading you want, and get back just that
section — the heading and everything beneath it, up to the next heading at the
same or a shallower level. Everything runs locally in your browser. Nothing is
uploaded, it works offline, and there's no sign-up.

Great for pulling one section out of a long README, splitting documentation,
grabbing a single changelog entry, or feeding just the relevant part of a doc to
another tool.

## How it works

A "section" starts at the matched heading and runs until the next heading of the
**same or a higher level** (a smaller or equal number of `#`s). So extracting
`## Installation` from a doc gives you the Installation heading plus every
`###` subsection under it, stopping right before the next `##`.

| Option | What it does |
| --- | --- |
| **Match mode — exact** (default) | Case-insensitive exact match of the trimmed heading text. `installation` matches `## Installation`. |
| **Match mode — exact_case** | Case-sensitive exact match. |
| **Match mode — contains** | Matches any heading whose text contains your query (case-insensitive). The first match wins. |
| **Include nested subsections** (default on) | Keep every deeper `###`/`####` subsection. Turn it off to get only the body directly under the heading, stopping at the first deeper heading. |
| **Include the heading line** (default on) | Turn it off to return only the body, without the heading itself. |

## Heading styles it understands

- **ATX headings** — `#`, `##`, … through `######`, including optional closing
  hashes (`## Notes ##`).
- **Setext headings** — a line of text underlined with `===` (level 1) or `---`
  (level 2).
- `#` lines **inside fenced code blocks** are treated as code, not headings, so a
  comment like `# install` in a shell snippet won't be mistaken for a section.

## Examples

Given this document:

```
# My Project

Intro.

## Installation

Run the installer.

### Linux

apt install foo

## Usage

Use it like so.
```

| Heading | Options | You get |
| --- | --- | --- |
| `Installation` | defaults | The `## Installation` heading, "Run the installer.", and the whole `### Linux` subsection — stopping before `## Usage`. |
| `Installation` | subsections off | Just `## Installation` and "Run the installer." |
| `Usage` | heading off | "Use it like so." |
| `linux` | contains | The `### Linux` subsection. |

## FAQ

<details>
<summary>Is it free and private?</summary>

Yes. Your document never leaves your device, and the tool keeps working
offline once the page has loaded.

</details>

<details>
<summary>What if two headings have the same text?</summary>

The first matching heading in the document is used.

</details>

<details>
<summary>How do I get only the prose under a heading, without its subsections?</summary>

Turn off "Include nested subsections." The section then ends at the first
deeper heading, returning only the body directly under your target.

</details>

<details>
<summary>Does it work on a section that runs to the end of the document?</summary>

Yes. If there is no following heading at the same or a shallower level, the
section continues to the end of the document.

</details>

<details>
<summary>What happens if the heading isn't found?</summary>

You get an error that lists the headings the tool did find, so you can check
the exact spelling or switch to "contains" matching.

</details>

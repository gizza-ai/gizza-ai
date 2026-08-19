## Bundle Markdown notes into one portable page

Use this exporter when several Markdown notes need to travel as a single file:
meeting notes, a project handbook, a research digest, or a release checklist. It
renders CommonMark plus common GitHub-flavored Markdown features, sanitizes the
HTML, adds stable heading anchors, generates an optional linked table of
contents, and embeds all CSS directly in the output.

### Worked example

Paste notes like this:

```markdown
# Project handbook

## Setup

Run the installer.

## Release checklist

- [ ] Tests pass
- [ ] Changelog updated

# Operations

## Rollback

Restore the previous build.
```

Choose **Split notes by: Level-1 headings**, **Table of contents: Sticky
sidebar**, **TOC depth: 3**, turn on **Number sections**, set the title to
`Project Handbook`, and export. The result is one complete HTML document you can
save as `.html`, send to someone else, or open offline.

### Limits and edge cases

Remote images are not fetched and inlined; images that are already `data:` URIs
stay embedded. Code blocks are styled as readable monospace blocks, but the tool
does not ship a language highlighter. Raw HTML from the notes is sanitized, so
scripts, event handlers and unsafe links are removed before the output is
wrapped.

## FAQ

<details>
<summary>Can I export multiple real files or a folder?</summary>

Paste their Markdown into the notes field. Use level-1 headings when each note
already starts with `# Title`, or thematic breaks (`---`) when you want explicit
boundaries between pasted notes.

</details>

<details>
<summary>Is the output really self-contained?</summary>

Yes. The exporter returns one `<!doctype html>` document with embedded CSS and no
external JavaScript, fonts or stylesheets. Remote images are the exception: the
tool does not fetch and embed them for you.

</details>

<details>
<summary>Why sanitize the Markdown output?</summary>

The exported file is meant to be shared. Sanitizing strips scripts, inline event
handlers and `javascript:` links so a pasted note cannot turn into an active
HTML payload in the final document.

</details>

<details>
<summary>How does the table of contents choose headings?</summary>

Every rendered heading receives a unique slug anchor. The TOC depth controls the
deepest heading level listed, from `1` through `6`; headings deeper than that
still appear in the document but are omitted from the TOC.

</details>

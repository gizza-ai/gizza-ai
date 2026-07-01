## Extract links from HTML or Markdown in your browser

Paste **HTML** or **Markdown** and get back every hyperlink and every in-page
anchor (jump target). Each link comes with its destination URL, its visible link
text, and a flag marking relative links. Everything runs locally in your browser
— your input is never uploaded to a server.

### What it finds

- **Hyperlinks** — HTML `<a href>` and `<area href>`; Markdown inline links,
  reference links, and autolinks (`<https://…>`). For each one you get the URL
  and the link text.
- **Anchors (jump targets)** — HTML `id` and legacy `<a name>` attributes;
  Markdown heading slugs (GitHub-style, e.g. `## Big Section` → `big-section`)
  and any explicit `<a name="…">` / `id="…"` anchors embedded in the Markdown.
  These are the targets a same-document `#name` link jumps to.

### Options

- **Parse as** — `auto` (default) sniffs HTML vs. Markdown from the content;
  force it with `html` or `markdown` if the auto-detection guesses wrong.
- **Base URL** — optional. Give an absolute URL like
  `https://example.com/docs/` and every relative link is resolved to its full
  absolute form (absolute, fragment, and `mailto:` links are left untouched).
- **Remove duplicate links** — collapse links that point at the same final URL,
  keeping the first occurrence.
- **Output** — `text` (default) for a readable list, or `json` for a structured
  `{ source, link_count, anchor_count, links, anchors }` object you can pipe
  into other tools. Each JSON link carries its `url`, `text`, a `relative` flag,
  and any HTML `rel` attribute (e.g. `nofollow`, `sponsored`).

### Notes

- Link text is whitespace-collapsed (newlines and runs of spaces become a single
  space).
- Relative links (`/path`, `./a`, `docs/x.md`) are flagged; fragment-only
  (`#section`), protocol-relative (`//cdn…`), and scheme-bearing (`https:`,
  `mailto:`) destinations are reported as written and counted as absolute.
- URLs are reported verbatim — they are not resolved against a base URL.

## FAQ

<details>
<summary>The auto-detection parsed my input as the wrong format — what do I do?</summary>

Set **Parse as** to `html` or `markdown` explicitly instead of leaving it on
`auto`. Mixed content (Markdown with embedded HTML tags) is the usual trigger for
a wrong guess. If you're not sure which parser ran, the JSON output's `source`
field tells you which one auto-detection actually picked.

</details>

<details>
<summary>How do I turn relative links like <code>/docs/page</code> into full URLs?</summary>

Fill in the **Base URL** field with an absolute URL such as
`https://example.com/docs/`. Every relative link is then resolved against it and
its `relative` flag is cleared. Links that are already absolute — including
fragment-only (`#section`), protocol-relative (`//cdn…`), and `mailto:`
destinations — are left exactly as written.

</details>

<details>
<summary>What counts as an "anchor" in the results?</summary>

Anything a same-document `#name` link can jump to: HTML `id="…"` attributes,
legacy `<a name="…">` anchors, and — in Markdown — GitHub-style heading slugs, so
`## Big Section` is reported as the anchor `big-section`. Anchors are listed
separately from hyperlinks with their own count.

</details>

<details>
<summary>Does "remove duplicate links" merge links with different text?</summary>

It collapses by **destination URL**: when several links point at the same final
URL, only the first occurrence is kept, whatever its text. It's off by default,
so an unfiltered run shows every `<a>` tag — handy when you're auditing anchor
text rather than collecting URLs.

</details>

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

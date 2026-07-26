# Competitor analysis — markdown-to-jira (2026-07-26)

Scan performed BEFORE implementing, to fix the table-stakes feature set. All findings are
paraphrased; no competitor copy, branding, or assets were reused.

## Competitors reviewed

1. **J2M / jira2md** (npm `jira2md`, the classic Fokke Zandbergen library that most online tools
   wrap) — bidirectional: `to_jira(markdown)` and `to_markdown(jira)`. This is the de-facto engine
   behind many of the sites below.
2. **MarkdownTools — Markdown to Jira Wiki Markup** (markdowntools.com) — browser-local converter +
   a detailed conversion guide/blog. One-directional page (MD→Jira) plus an ADF-JSON export angle.
3. **FreeMarkdownTools — Free Markdown to Jira** — runs entirely in the browser, "never uploaded,
   logged, or cached"; no registration.
4. **MarkdownMe / 1000freetools / jadapps** — free single-textarea MD→Jira converters, live output,
   copy button, worked examples in copy.
5. **paoloantinori/markdown_to_jira** — Firefox extension that right-click-replaces Jira issue text
   with converted markup (MD→Jira only).

## Table-stakes mappings (Markdown ↔ Jira wiki markup)

| Element        | Markdown                | Jira wiki markup            |
|----------------|-------------------------|-----------------------------|
| Heading 1–6    | `#`…`######`            | `h1.`…`h6.`                 |
| Bold           | `**text**`              | `*text*`                    |
| Italic         | `*text*` / `_text_`     | `_text_`                    |
| Strikethrough  | `~~text~~`              | `-text-`                    |
| Inline code    | `` `code` ``            | `{{code}}`                  |
| Code block     | ```` ```lang ````       | `{code:lang}…{code}` (no lang → `{code}`) |
| Link           | `[text](url)`           | `[text\|url]` (bare `[url]`) |
| Image          | `![alt](url)`           | `!url!`                     |
| Unordered list | `- item`                | `* item`                    |
| Ordered list   | `1. item`               | `# item`                    |
| Nested list    | indentation             | repeated markers (`**`, `##`, mixed `#*`) |
| Table          | pipe table + delimiter  | `\|\|H\|\|H\|\|` header rows, `\|c\|c\|` body |
| Blockquote     | `> text`                | `bq. text` (1 line) / `{quote}…{quote}` |
| Horizontal rule| `---`                   | `----`                      |
| Note/Info/Warn/Tip panel | `> Note:` blockquote convention | `{note}`/`{info}`/`{warning}`/`{tip}` macro |

## Decisions — in-model vs out-of-model

**In-model (built):**
- **Bidirectional** conversion (`direction`: `md-to-jira` default, `jira-to-md`). J2M is the only
  engine that does both; most sites are one-way. We match J2M's headline capability.
- All table-stakes block + inline constructs above.
- `panel_blockquotes` toggle: `> Note:`/`Warning:`/`Info:`/`Tip:` blockquotes ↔ Jira `{note}` etc.
  panel macros (mirrors our sibling `markdown-to-confluence` UX; a well-known convention).
- `heading_offset` (0–5) to demote headings when pasting under an existing page title
  (MD→Jira only) — a genuinely useful knob competitors lack.
- Worked-example preset chips, live output, copy button, browser-local (privacy).
- Single-line blockquote → `bq.` (more idiomatic Jira than always `{quote}`).

**Considered, not built (out-of-model or rejected):**
- **ADF JSON export** (Atlassian Document Format, for POSTing tickets via the Cloud REST API):
  a distinct output format + a heavier schema; MarkdownTools offers it. Out of scope for a
  wiki-markup converter — listed, not built. Could be a separate tool.
- `{color:red}…{color}`, `+underline+`, `^super^`, `~sub~`, `{panel:title=…}`: Jira-only inline
  constructs with **no Markdown equivalent**. MD→Jira can't produce them from standard Markdown,
  and on the reverse path we preserve `+ ^ ~` and `{panel}` text rather than dropping content —
  documented as a known limit. Not forced into the schema.
- Footnotes, definition lists, task/checkbox lists: no wiki-markup equivalent (per the MarkdownTools
  guide) — degrade to literal text, documented.
- Browser-extension / right-click-replace UX (paoloantinori): out of model (this is a page + CLI).

## Notable competitor limitations we share/handle
- No table column alignment, colspan/rowspan, or cell colors in Jira wiki markup — a hard limit of
  the target format, not our tool. Documented on the page.
- Nesting in Jira is by repeated markers, not indentation — handled in both directions.

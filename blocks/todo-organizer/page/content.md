## Organize your brain-dump

Paste a messy list of tasks — one per line — and get back a tidy, prioritized
Markdown checklist. Each task's priority is inferred from urgency keywords, and
any due-date hints are surfaced inline. It all runs in your browser; nothing is
uploaded.

### How priority is inferred

- **High** — words like *urgent*, *asap*, *critical*, *today*, *deadline*,
  *overdue*, *must*, *p0/p1*.
- **Medium** (the default) — *soon*, *tomorrow*, *this week*, *next*, *p2*.
- **Low** — *later*, *someday*, *eventually*, *maybe*, *optional*, *backlog*,
  *p3/p4*.

A task with no urgency keyword lands in **Medium**. If a line carries more than
one signal, the highest priority wins.

### Options

- **Grouping** — *priority* (default) groups tasks under High/Medium/Low
  headings; *none* keeps your original order in one flat list.
- **Numbered list** — emit a numbered list (`1. 2. 3.`) instead of checkbox
  bullets (`- [ ]`).
- **Show due-date hints** — surface detected hints (*today*, *tomorrow*,
  *Friday*, *this week*, …) inline as `_(due: …)_`. On by default.

### Good for

- Turning a stream-of-consciousness dump into an actionable, ranked list.
- Generating a Markdown checklist to paste into GitHub, Notion, or Obsidian.

### FAQ

<details>
<summary>Is my text uploaded?</summary>

No — it's processed locally in your browser with
WebAssembly.

</details>

<details>
<summary>Does it invent due dates?</summary>

No. It only surfaces date words you actually
wrote; it never guesses a calendar date.

</details>

<details>
<summary>Why did a task I consider urgent end up in Medium?</summary>

Priority is inferred purely from keywords in the line — *urgent*, *asap*,
*today*, *p0* and friends push a task to High; without any signal it defaults to
Medium. Add an urgency word to the line (or a `p0`–`p4` tag) and re-run, and it
will be ranked accordingly. When a line carries several signals, the highest one
wins.

</details>

<details>
<summary>Can I paste a list that already has bullets or numbers?</summary>

Yes — existing `-`, `*`, checkbox, and `1.`-style prefixes are stripped before
processing, so re-organizing an old list won't produce doubled-up bullets. Each
non-empty line becomes exactly one task.

</details>

# todo-organizer — competitor analysis (2026-06-21)

## What this tool does
Parses a freeform brain-dump (one task per line) into a prioritized Markdown
checklist. Priority is inferred from urgency keywords (urgent/asap/today → High,
soon/this week → Medium, later/someday → Low; default Medium). Lightweight
due-date hints (today, tomorrow, Friday, this week, …) are surfaced inline as
`_(due: …)_`. Options: `group_by` (priority | none), `numbered`, `show_due`.

## Surfaces verified
- **Chat / LLM API** — `wafer build` validates `target/block.wasm`; drift-guard
  schema test passes (chat schema == authored == manifest.json).
- **CLI** — `gizza tool todo-organizer text=… [group_by=…] [numbered=…] [show_due=…]`
  returns the grouped Markdown; due hints and all options confirmed.
- **Page** — `/tools/todo-organizer/` renders a textarea + a `priority|none`
  select + `numbered`/`show_due` checkboxes; 3 Playwright specs pass (grouping,
  query-param deep-link with due hint, numbered+none).

## Competitor landscape
The "brain dump → prioritized list" space is dominated by **app templates** and
**printable worksheets**, not instant text utilities:

| Competitor | Form | Gap vs. ours |
| --- | --- | --- |
| Asana brain-dump template | Full SaaS project board; tag by priority/urgent/personal, List/Board/Calendar views | Requires an account + workspace; manual tagging, not auto-inferred |
| ClickUp brain-dump worksheets | 12 SaaS doc templates | Account-gated; no automatic prioritization from free text |
| Day Designer / Savvy Sparrow / 101planners / saturdaygift | Free **printable** PDF/worksheets | Paper only — no parsing, no Markdown output |
| ADHD productivity advice (ADDitude, Anchored Women) | How-to articles + manual method | No tool at all; the manual method is exactly what we automate |

Sources:
- [Asana — Brain Dump Template](https://asana.com/templates/brain-dump)
- [ClickUp — 12 Free Brain Dump Worksheet Templates](https://clickup.com/blog/brain-dump-worksheets/)
- [Day Designer — Brain Dump Worksheet](https://daydesigner.com/products/brain-dump)
- [The Savvy Sparrow — Free Brain Dump Template](https://thesavvysparrow.com/free-brain-dump-template/)
- [ADDitude — Prioritize My Brain Dumps Into To-Do Lists](https://www.additudemag.com/to-do-list-advice-brain-dump/)

## Gap analysis & decisions (fit-to-model)

**Closed / in-model (shipped):**
- Auto-priority inference from urgency keywords (the manual "tag by urgent" step
  competitors require) — done, with stem matching so `urgently`/`blocking` match
  their stems.
- Due-date hint surfacing without inventing calendar dates (honesty: only echoes
  date words the user wrote).
- Markdown checkbox output that pastes directly into GitHub/Notion/Obsidian — a
  format none of the printable competitors offer.
- Numbered-list and flat (`group_by=none`) modes for non-grouped exports.
- Tolerant input: strips existing `- [ ]` / `*` / `1.` markers so a half-formatted
  dump doesn't double-bullet.
- 100% client-side (WASM), no upload, no sign-up — the key differentiator vs. all
  account-gated SaaS competitors.

**Out-of-model (deliberately not built):**
- Real calendar date parsing / scheduling (would need a date NLP model + a clock;
  the tool intentionally never guesses a date the user didn't write).
- Multi-view boards / drag-and-drop / persistence — those are stateful SaaS
  features, out of scope for a stateless pure-compute tool.
- Category/tag clustering by topic (would need an embedding model) — deferred.

## Conclusion
The tool occupies an empty niche: an instant, private, no-account utility that
automates the exact manual "dump then prioritize then format" workflow the
competitor articles describe, emitting clean Markdown. No competitor copy,
branding, or trademark was used.

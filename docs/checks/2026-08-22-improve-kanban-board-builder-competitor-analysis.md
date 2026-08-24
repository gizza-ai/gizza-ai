# kanban-board-builder — competitor analysis (2026-08-22)

Scan run **before** implementation, per `/create-next-tool` step 4. All notes are paraphrased
observations of publicly documented behaviour — no competitor copy, branding or trademarks are
reproduced, and nothing below is quoted into our page.

## Scope

Backlog row: *"Turns a task list or freeform notes into a structured kanban board with columns in
markdown or JSON."* Type hint `pure`.

## Duplicate check (done first)

`ls blocks/ | grep -iE 'kanban|task|board|todo|markdown|json'` plus a read of the three closest
existing blocks:

| Existing block | What it does | Why it is not this tool |
| --- | --- | --- |
| `todo-organizer` | Freeform brain-dump → one flat Markdown checklist grouped under High/Medium/Low priority headings | Groups by inferred *urgency*, emits a single linear checklist. No workflow columns, no per-column state, no board object, no WIP limits. |
| `task-list-summarizer` | Parses GFM checklists → done/pending counts and filtered views | Counting/filtering an existing checklist; output is a summary line or two flat lists, never a board. |
| `task-format-converter` | Converts a task list between todo.txt / Markdown checklist / JSON / CSV | Format transcoding of a flat list; the JSON is a flat array of task objects with no lane structure and no status→column routing. |
| `action-item-extractor` | Pulls TODO/action markers out of meeting notes | Extraction only; output is a flat checklist grouped by owner. |
| `natural-language-task`, `recurring-task-expander`, `tasks-to-ical` | Single-task parsing / recurrence / calendar export | Different output surfaces entirely. |

The distinct, unclaimed capability here is **lane routing**: reading a status signal off each task
(explicit tag, section heading, checkbox state, or workflow keyword) and emitting a *column-shaped*
artifact — Markdown sections, a side-by-side Markdown table, or a JSON board object with per-column
counts and WIP-limit state. None of the blocks above produce columns. Not a duplicate; built.

## Competitors reviewed

Four real tools were reached. Each is a board tool whose *storage/interchange* format is Markdown,
which is exactly the artifact we produce.

### 1. Markdown Task Manager (`ioniks/MarkdownTaskManager`)
Local-first browser board that reads/writes Markdown files through the File System Access API.

- Columns are `##` headings; the board carries a configuration block listing the column set,
  categories, users and tags.
- Cards are `###` headings with an id prefix, followed by bolded metadata lines: priority
  (Critical/High/Medium/Low), category, assignee(s) as `@name`, created/due dates, `#tags`, a free
  description, and a nested subtask checklist.
- Completed work is moved to a separate archive file; no WIP limits are enforced.

### 2. Markdown Kanban (`quclo/markdown-kanban`)
Drag-and-drop board with Markdown import **and** export.

- Heading hierarchy is the board model: `#` board title → `##` column → `###` card.
- Metadata rides in blockquote `> key: value` lines — card `type`, `priority`, `tags`, `assigned`,
  `due`; **columns** carry their own metadata including `hidden` and, notably, `limit: 5` — an
  explicit per-column WIP limit.
- Boards, columns and cards all support an optional description under the title.

### 3. Kanban md (`kanbanmd.lecaro.me`)
Visual editor over plain Markdown files, plus a Trello-export → Markdown converter.

- Same H1/H2/H3 board→column→card shape; card titles can carry bracketed `[tag]` labels.
- Card bodies hold descriptions, `- [ ]`/`- [x]` checklists and quoted comments.
- Reinforces bracketed inline labels as a common card-tag convention.

### 4. Taskade — Kanban board → Markdown converter
Hosted converter aimed at exporting an existing board to Markdown notes.

- Sells "format-aware, lossless" conversion that preserves card order and column membership.
- Direction is board → Markdown (an export of a board the user already built in the product);
  requires an account and a board that already exists in the tool.

Also noted from general Kanban references: the standard column vocabulary is Backlog / To Do /
In Progress / Review / Done, and WIP limits are conventionally advisory — a visual warning, not a
hard block on adding cards.

## Table-stakes extracted, and where each landed

| # | Table stake (seen in ≥1 competitor) | Verdict | Where it landed |
| --- | --- | --- | --- |
| 1 | Board title above the columns | in-model | `title` param (default `Kanban Board`), rendered as `#` in Markdown, `title` in JSON |
| 2 | Configurable column set + order | in-model | `columns` param, comma-separated, default `To Do, In Progress, Done` |
| 3 | Columns as `##` headings, cards beneath | in-model | `format=markdown` — Obsidian/`quclo`-compatible section shape |
| 4 | Per-column card counts | in-model | `show_counts` (default on) → `## In Progress (2)` |
| 5 | Per-column WIP limit with an over-limit warning | in-model | `wip_limit` param; heading annotated `(4 / limit 3 — over WIP limit)`, JSON `wip_limit` + `over_limit` |
| 6 | Card metadata: assignee | in-model | `@name` parsed off the task line → `assignee` |
| 7 | Card metadata: tags | in-model | `#tag` **and** bracketed `[tag]` (kanbanmd's convention) → `tags[]` |
| 8 | Card metadata: priority (Critical/High/Medium/Low) | in-model | `!high`/`!critical`/…, `P0`–`P3`, and `priority:high` → `priority` |
| 9 | Card metadata: due date | in-model | `due:2026-09-01` / `due 2026-09-01` / `(due 2026-09-01)` → `due` |
| 10 | Checkbox state routes completed work | in-model | `- [x]` routes to the done-like column; re-emitted as `- [x]` |
| 11 | Structured export (JSON board object) | in-model | `format=json` — columns, counts, limits, parsed card fields, totals |
| 12 | Card ordering / sorting within a lane | in-model | `sort_by` = `none` (input order) / `priority` / `due` / `text` |
| 13 | Re-import an existing board (round trip) | in-model | Input `##` headings are read as lane assignments, so a board this tool emitted can be fed back in |
| 14 | Preset boards to start from | in-model | four `[[example]]` preset chips on the page |
| 15 | Side-by-side column view | in-model | `format=table` — one Markdown table column per lane |
| 16 | Drag-and-drop card movement | **out-of-model** | Needs a stateful interactive canvas; gizza pages are a form + a rendered result. Listed, not built. |
| 17 | Persistent boards / file sync / accounts | **out-of-model** | No server, no storage, no login in this model. |
| 18 | Multi-user collaboration, comments, activity feeds | **out-of-model** | Requires a backend. |
| 19 | Card descriptions + nested subtask checklists per card | **considered, rejected** | A one-line-per-task input can't express a multi-line card body without inventing an indentation grammar the source notes won't have. Nested/indented lines are instead treated as their own cards so nothing is silently dropped. |
| 20 | Separate archive file for finished work | **considered, rejected** | Two output artifacts don't fit a single-output surface; the done column already holds finished cards. |
| 21 | Hidden columns (`hidden: true`) | **considered, rejected** | Meaningful only in an interactive board; a hidden lane in a static export is just an omitted lane — drop the column from `columns` instead. |

Every table stake ends in the descriptor or in the out-of-model/rejected list; none was dropped
silently.

## What we do that none of the four do

- **Freeform notes in, board out.** All four assume you already have a board (or hand-write the
  board Markdown). Ours infers lane membership from workflow keywords (`blocked`, `in progress`,
  `waiting on review`, `shipped`, …), from `status:` tags, from `##` section headings in the pasted
  notes, and from checkbox state — so a raw standup dump becomes a board with no manual sorting.
- **Three output shapes from one input** (sections, side-by-side table, JSON) rather than one fixed
  file format.
- **No account, no upload, no install** — it runs as WebAssembly in the page, and the same engine is
  reachable from the CLI and the chat surface.

## Sources

- https://github.com/ioniks/MarkdownTaskManager
- https://github.com/quclo/markdown-kanban
- https://kanbanmd.lecaro.me/
- https://www.taskade.com/convert/kanban-board/kanban-board-to-markdown

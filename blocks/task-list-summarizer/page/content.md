## What this tool does

Paste a Markdown checklist and instantly see how many tasks are **done**, how
many are **pending**, and your **completion percentage** — right in your browser.
Nothing is uploaded to a server: it runs locally, works offline, and needs no
sign-up. Switch the **Output** to list only the completed or only the remaining
tasks, or to export a JSON object for scripts and dashboards.

## What counts as a task

A task is a [GitHub-flavored Markdown](https://github.github.com/gfm/) checklist
item — a list marker followed by a checkbox:

| Written as | Meaning |
| --- | --- |
| `- [ ] write tests` | pending |
| `- [x] write code` | done |
| `* [X] ship it` | done (any marker, upper- or lowercase `x`) |
| `1. [ ] follow up` | pending (numbered lists work too) |

Unordered markers (`-`, `*`, `+`) and ordered markers (`1.`, `2)`) are all
recognized. Nested/indented items are counted. Plain list items without a
checkbox, headings, and prose are ignored — so you can paste a whole document and
only the real tasks are summarized.

## Output modes

| Output | What you get |
| --- | --- |
| **summary** (default) | One line: total, done, pending, and percent complete. |
| **done** | Just the completed task texts, one per line. |
| **pending** | Just the not-yet-done task texts, one per line. |
| **json** | `{"total":…,"done":…,"pending":…,"percent":…,"done_items":[…],"pending_items":[…]}` for scripts. |

## Example

Input:

```
# Launch checklist
- [x] write code
- [x] write tests
- [ ] update docs
- [ ] ship it
```

**summary** → `4 tasks: 2 done, 2 pending (50% complete).`

**pending** →

```
update docs
ship it
```

## FAQ

**Is it free and private?** Yes — your checklist never leaves your device, and the
tool keeps working offline once the page has loaded.

**Does it support numbered lists?** Yes. `1. [ ] task` and `2) [x] task` are both
recognized alongside the `-`, `*`, and `+` bullet markers.

**Why isn't one of my lines counted?** A task needs a checkbox right after the
list marker — `- [ ] text` or `- [x] text`. A bullet without a `[ ]`/`[x]`
checkbox is treated as a plain list item and ignored.

**How is the percentage calculated?** It's `done ÷ total`, rounded to the nearest
whole percent. An empty list reports 0%.

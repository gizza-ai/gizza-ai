## Write the plan, not the syntax

Mermaid's `gantt` syntax is compact but fussy: every task needs an id before you can point another
task at it, tags have to come first in the spec, and one stray colon in a task name breaks the
whole diagram. This tool takes the plan in the shape you already think about it — a task, when it
starts, how long it takes — and writes the Mermaid source for you.

Everything runs locally in your browser. The task list never leaves your machine.

## The task table

One task per line, up to five columns, separated by a pipe (`|`), a tab, or a comma:

```
name | start | duration | tags | id
```

- **name** — the label shown on the bar. Required. A colon in the name is replaced with a hyphen,
  because Mermaid uses the colon to separate the label from the task spec.
- **start** — a date in your chosen date format, `after <task>` to begin when another task ends, or
  **left empty** to follow the task on the line above. Chain several with `after Design, Research`.
- **duration** — a length such as `5d`, `2w`, `36h`, `1.5d`, or a bare number meaning days. You can
  also give an end date instead, or `until <task>` to run right up to another task's start.
- **tags** — any of `done`, `active`, `crit`, `milestone`, space separated. Milestones with no
  duration become zero-length markers automatically.
- **id** — optional. An id is derived from the task name, so `after Visual design` just works;
  supply your own only when you want a short handle.

A line starting with `section ` or `## ` opens a section. Blank lines, and lines starting with
`#`, `//` or `%%`, are ignored — so you can keep notes in the plan.

## Worked example

Input:

```
section Design
Wireframes | 2026-03-02 | 5d | done
Visual design | after Wireframes | 1w | active
Design sign-off | after Visual design | | milestone
```

with the title `Q2 launch plan`, axis format `%b %d`, tick interval `1week` and **skip weekends**
turned on, produces:

```
gantt
    title Q2 launch plan
    dateFormat YYYY-MM-DD
    axisFormat %b %d
    tickInterval 1week
    excludes weekends
    section Design
        Wireframes      :done, wireframes, 2026-03-02, 5d
        Visual design   :active, visual_design, after wireframes, 1w
        Design sign-off :milestone, design_sign_off, after visual_design, 0d
```

Paste that into a GitHub or GitLab comment, a Markdown file, a Notion code block, an Obsidian note,
or any Mermaid-compatible viewer and it renders as a timeline. Turn on **Wrap in a Markdown mermaid
fence** to get the surrounding ```` ```mermaid ```` block too.

## Chart options

| Option | What it does |
| ------ | ------------ |
| Chart title | Adds a `title` line above the timeline. |
| Date format | How you write dates, from `YYYY-MM-DD` to `DD/MM/YYYY` or Unix seconds. Also validates every date you type. |
| Axis label format | Mermaid's `axisFormat`, a strftime pattern such as `%b %d` or `%d/%m`. |
| Axis tick interval | Mermaid's `tickInterval` — `1week`, `2day`, `6hour`, `1month`. |
| Skip weekends | Adds `weekends` to the `excludes` line so bars step over Saturday and Sunday. |
| Weekend starts on | Moves the weekend to Friday+Saturday (needs Mermaid 11 or newer). |
| Other non-working days | Extra excluded dates or weekday names, e.g. `2026-04-03, monday`. |
| Show the today marker | Off emits `todayMarker off` — useful for a historical or illustrative chart. |
| Compact display mode | Packs non-overlapping tasks onto shared rows (needs Mermaid 10 or newer). |

## Limits and edge cases

- Up to **500 tasks**, **100 sections** and **2 MB** of input.
- Dependencies must point at a task defined **earlier** in the list. Mermaid resolves `after`
  chains top-down, so a forward reference would silently render at the wrong date — you get an
  error naming both lines instead.
- Dates are checked against the format you picked. `03/02/2026` under `YYYY-MM-DD` is rejected,
  with the line number and the expected format.
- Repeated task names get distinct ids (`review`, `review_2`), so duplicates never collide.
- Nothing here computes calendar dates: durations, weekend skipping and `after` chains are resolved
  by Mermaid when it renders. That is also why excluded days lengthen a bar rather than move it.
- With the comma delimiter a task name containing a comma splits into extra columns — switch the
  delimiter to pipe for those.

## FAQ

<details>
<summary>Do I have to invent an id for every task?</summary>

No. An id is derived from the task name — `Visual design` becomes `visual_design` — so you can
write `after Visual design` and it resolves. The optional fifth column lets you set a short id by
hand when you would rather type `after des` than the full name.

</details>

<details>
<summary>How do I add a milestone?</summary>

Tag the task `milestone` and leave the duration column empty. Milestones are single points in time,
so a blank duration becomes `0d` automatically. Give a start date or an `after <task>` reference to
place it — for example `Public launch | after Beta | | milestone`.

</details>

<details>
<summary>Why does my task start on the wrong day when weekends are excluded?</summary>

Mermaid applies `excludes` by extending a bar over the skipped days, not by moving its start. A
five-day task starting on a Thursday with weekends excluded finishes the following Wednesday. If a
task must not begin on a weekend, give it an explicit start date on a working day.

</details>

<details>
<summary>Can I mark work as done, in progress or critical?</summary>

Yes — put `done`, `active` or `crit` in the tags column, space separated if you want more than one.
They map to Mermaid's built-in styling: completed bars, the in-progress highlight, and the critical
emphasis. The generator reorders them into the sequence Mermaid expects, so `crit done` is fine.

</details>

<details>
<summary>What date formats can I use?</summary>

Pick one from the date format list and write every date that way: ISO `2026-03-02`, day-first
`02/03/2026`, month-first `03/02/2026`, slash or hyphen separated, with an optional time, or Unix
epoch seconds. The choice is emitted as Mermaid's `dateFormat` and used to validate your input, so a
typo is caught here rather than rendering as an empty chart.

</details>

<details>
<summary>Where does the generated code actually render?</summary>

Anywhere Mermaid runs: GitHub and GitLab Markdown, Notion and Obsidian code blocks, MkDocs and
Docusaurus sites, the Mermaid live editor, and most modern wikis. Compact display mode needs
Mermaid 10 or newer, and moving the weekend to Friday needs Mermaid 11 or newer; everything else
works on older versions too.

</details>

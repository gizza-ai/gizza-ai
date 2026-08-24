# Competitor analysis: gantt-mermaid-generator

Date: 2026-08-21
Tool: `gantt-mermaid-generator`
Backlog row: builds Mermaid `gantt` source from tasks with start dates, durations and dependencies.

## Competitors scanned

| Competitor | What it offers | Table-stakes seen | In model? | Decision |
| --- | --- | --- | --- | --- |
| mermaidonline.live Gantt editor | Mermaid code editor with live preview, template reset, theme selector, zoom/pan preview, PNG/SVG export and sharing. The default example uses `title`, `dateFormat`, `section`, explicit task ids, `after` dependencies and durations. | Direct Mermaid source output; sections; dependencies; durations; project-title option; preview/export UX. | Partly. Source generation is in-model; live render/export is page/UI out-of-model for a text-only block. | Emit clean Mermaid source, support title/sections/dependencies/durations, and document that rendering/export happens in Mermaid-compatible viewers. |
| Mermaid Editor Gantt maker (`mermaideditor.io/diagrams/gantt`) | Full editor with AI generate, style presets, share/export, syntax quick reference, and rendered preview. It documents title, sections, tasks, dependencies, durations and milestones, with PNG/SVG/PDF export. | Task table concepts; dependency chains via `after`; milestone syntax; export-ready Mermaid; examples/templates; styling presets. | Partly. Syntax generation, milestones and examples are in-model; AI natural-language planning, styling UI and PDF/PNG export are out-of-model. | Build a deterministic table-to-Mermaid generator with milestone support, examples/preset chips, and no AI/render/export claims. |
| mermaid-online-editor.com Gantt page | Simple code+preview page with paste/copy/clear/download/share controls, zoom, quick syntax reference and use-case categories. Its default example uses `dateFormat`, sections, explicit dates, durations and implicit/explicit dependencies. | Copyable source; quick reference; task status tags (`done`, `active`); simple examples; dependency and timeline use cases. | Partly. Copyable generated source and status tags are in-model; graphical preview/download/share controls are out-of-model. | Return text source that can be copied; support `done`, `active`, `crit`, `milestone`; include docs and FAQ for common use cases. |

## Parameters and UX patterns to match

| Capability | Default / pattern observed | In model? | Implemented as |
| --- | --- | --- | --- |
| Task list input | Competitors edit raw Mermaid; a generator should accept a higher-level task list. | Yes | Required multiline `tasks` table: `name | start | duration | tags | id`. |
| Sections | `section Name` groups task rows. | Yes | `section Name` and `## Name` input lines. |
| Dependencies | `after taskId` is the common Mermaid pattern. | Yes | `after <task name or id>` resolves to ids derived from task names. |
| Durations and dates | Examples use dates plus `5d`, `2w`, etc. | Yes | Validates dates by selected `date_format`; accepts duration units and bare day counts. |
| Milestones/status | Quick references show `done`, `active`, and milestones. | Yes | Tags column supports `done`, `active`, `crit`, `milestone`; blank milestone duration becomes `0d`. |
| Date format | Mermaid examples usually show `YYYY-MM-DD`; docs allow alternatives. | Yes | Enum default `YYYY-MM-DD` plus slash/day-first/month-first/date-time/Unix options. |
| Axis and tick controls | Editor users often tune axis readability. | Yes | `axis_format` and `tick_interval` optional text fields. |
| Non-working days/weekends | Mermaid supports `excludes`/`weekend`; project tools often need workday calendars. | Yes | `exclude_weekends`, `weekend`, and extra `excludes`. |
| Today marker | Mermaid has `todayMarker`; historical examples often need it off. | Yes | `today_marker` boolean, default true. |
| Compact display | Mermaid supports compact display for dense charts. | Yes | `compact` boolean emits Mermaid front matter. |
| Markdown fence | Competitors focus on paste/share; Markdown users need fenced blocks. | Yes | `fence` boolean wraps in ```mermaid. |
| Live preview, themes, PNG/SVG/PDF export, share links | Competitor editor UI features. | Out of model for a pure text block. | Documented as rendering/export to perform in Mermaid-compatible viewers/editors. |
| AI natural-language schedule generation | Some competitors advertise AI generation. | Out of model for deterministic local Rust block. | Not built; this tool requires explicit task rows. |

## Design notes

The in-model gap is not another Mermaid renderer; it is a safer source generator that lets users write a task table and get valid Mermaid gantt code. The descriptor therefore prioritizes validation, line-numbered errors, task-name dependency resolution, enum controls for fixed choices, checkbox controls for display toggles, and examples that map common plans to generated source. Out-of-model editor features are called out in the page copy as things users can do after copying the Mermaid source into GitHub, GitLab, Mermaid Live Editor, Notion, Obsidian or another renderer.

# org-chart — competitor analysis (2026-07-29)

Tool function: render an organizational chart image from an indented list or JSON of people/managers.
Scan: one WebSearch ("org chart generator online from text indented list free tool") + fetched the
top reachable competitor tools. All observations paraphrased — no competitor copy/branding reproduced.

## Competitors skimmed

1. **GenerateOrgChart** (generateorgchart.com) — plain-text hierarchy with indentation (spaces =
   reporting level, one role per line), plus CSV/XLSX import with a manager field, plus templates.
   ~6 built-in visual themes (light / doc / deck / dark). Nodes show name + title + department.
   Fully editable on a canvas after generation. Exports PNG and SVG. No stated size limits.
2. **Syncfusion Free Org Chart Maker** — drag-and-drop editor; auto-adjusting layouts with multiple
   views; collapsible/expandable branches; search with highlight; zoom/pan/overview. Node fields
   include photo, job title, and other details. Exports PNG, SVG, JPG. No account.
3. **Musely Org Chart Maker** — AI/text-to-chart; enter positions with indentation; six layouts
   (hierarchical, matrix, circular, …). Professional styling.

(Also seen but not deep-read: Visme, SmartDraw, Organimi, OrgChartCreator, Eraser AI — same shape:
editor UIs, template galleries, exports.)

## Table-stakes → decisions

| capability | competitors | our decision |
| --- | --- | --- |
| Indented-text input (spaces/tabs = depth) | GenerateOrgChart, Musely | **in-model** — parse leading whitespace via an indent stack; `data` param |
| JSON input (nested and/or flat manager list) | (JSON/CSV mapping) | **in-model** — accept nested `{name,title,children}` and flat `[{name,manager,title}]` |
| Node fields: name / title / department | all | **in-model** — `Name \| Title \| Department` in text, `title`/`department` keys in JSON |
| Chart title heading | all | **in-model** — `title` param |
| Orientation: top-down vs left-to-right | Syncfusion, Musely (layouts) | **in-model** — `direction` enum (`vertical`\|`horizontal`) |
| Theme / node colour | GenerateOrgChart (themes) | **in-model** — `color` param (accent bar + border), sanitized CSS colour |
| SVG output (vector, scalable) | all | **in-model** — native output is `image/svg+xml` |
| PNG / JPG raster export | all | **out-of-model** — SVG is vector and converts client-side; no rasteriser in the pure block |
| CSV / XLSX employee-list import | GenerateOrgChart, OrgChartCreator | **out-of-model here** — spreadsheet parsing belongs to a separate importer tool; JSON covers the structured case |
| Drag-drop canvas editing | all | **out-of-model** — interactive editor, needs a live app |
| Collapse / expand, search, zoom / pan | Syncfusion | **out-of-model** — interactive viewer features |
| Photos in nodes | Syncfusion | **out-of-model** — needs image upload/embedding |
| AI plain-English → chart | Musely, Eraser | **out-of-model** — needs a model |
| Matrix / circular layouts | Musely | **considered, not built** — hierarchy tree is the core org-chart shape; extra layouts add schema surface for a niche case |

## Limits stated on our page/descriptor
- Max 400 people; excess is rejected with an actionable error.
- Indentation: 1 tab or N spaces per level (an indent stack infers the step; be consistent).
- Cycles / dangling managers in flat JSON are rejected with a clear message.

No page: like scatter-chart / line-series-chart, an image-bytes (SVG) output has no standalone page
render mode, so the verified surfaces are the schema/descriptor tests and the CLI (chat also renders
the returned SVG envelope). Original work only — no competitor copy, branding, or trademarks.

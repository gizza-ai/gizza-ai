# resume-scaffolder — competitor analysis (2026-07-23)

Function: turn structured résumé inputs (contact, summary, experience, education,
skills, extra sections) into a **clean, print-ready HTML résumé** the user can open
and print to PDF from the browser. Distinct from the existing `resume-builder`
(which emits ATS-friendly **Markdown**) and `resume-to-json` (parse/validate JSON
Resume): this tool's deliverable is a **styled, self-contained HTML document** with
a print stylesheet.

All analysis is paraphrased from public marketing pages — **no competitor copy,
branding, or templates were reproduced.**

## Competitors skimmed (top real tools)

1. **Enhancv** (enhancv.com) — 15+ named template layouts (single/double column,
   compact, minimal, classic…), 20+ section types (experience, education, skills,
   summary, certifications, awards, languages, projects, strengths, interests),
   customizable fonts/colors/backgrounds, A4 + US-Letter page sizes, PDF + TXT
   export with a real text layer.
2. **Resume.io** (resume.io) — 40+ named templates grouped as ATS-optimized /
   professional / creative, step-by-step section entry (experience, education,
   skills, summary, links, certifications), PDF + DOCX + Google-Docs export. Font/
   color controls exist per template but aren't exposed as granular knobs on the
   landing copy.
3. **Canva / Adobe Express** — template-first visual editors: hundreds of
   designer templates, free-form color/font/layout editing, one-click PDF download
   and print ordering. Heavy WYSIWYG editors, account-oriented.

## Table-stakes → in-model / out-of-model

| Capability | In model? | Decision |
|---|---|---|
| Sections: summary, experience, education, skills | **in** | Core renders all four. |
| Extra sections (projects, certifications, awards, languages) | **in** | `sections[{heading,items[]}]` escape hatch (mirrors resume-builder shape). |
| Contact block (email, phone, location, links) | **in** | Rendered in the header. |
| Template / layout choice | **in** | `theme` enum: `classic` (serif, centered header), `modern` (accent sidebar rule), `compact` (tighter spacing for one-page fit). |
| Accent color customization | **in** | `accent` color param (hex/named), used for rules + headings. |
| Font family choice | **in** | `font` enum: `sans` / `serif`. |
| Page size A4 / US-Letter | **in** | `page_size` enum: `letter` / `a4` → `@page` size + max-width. |
| Print-ready / print-to-PDF | **in** | Output is a full HTML doc with `@media print` + `@page` so the browser's Print → Save-as-PDF gives a clean 1-page-margin document. |
| Direct PDF/DOCX file download | **out** | Needs a server-side or binary renderer; we emit HTML and rely on the browser's native Print-to-PDF. Stated on the page. |
| AI content writing / bullet suggestions | **out** | Needs an LLM/backend; out of the pure-wasm model. |
| Job-description tailoring / keyword match | **out** | Needs a backend + job data. |
| Accounts, cloud storage, versioning | **out** | No server, no accounts by design. |
| Live WYSIWYG drag editor | **out** | This is a deterministic input→document generator, not an editor. |

## Design landed

`data` (JSON, required) + four style params: `theme` (classic|modern|compact),
`accent` (color, default `#2563eb`), `font` (sans|serif), `page_size` (letter|a4).
Output: one self-contained, escaped HTML document with embedded CSS and a print
stylesheet. HTML is escaped to prevent markup injection from résumé text.

Sources:
- https://enhancv.com/
- https://resume.io/
- https://www.canva.com/resumes/
- https://www.adobe.com/express/create/resume

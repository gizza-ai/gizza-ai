# resume-builder — competitor analysis (2026-06-20)

Sixteenth `/create-next-tool` backlog pick (readable-article was skiplisted before
it as a dup of readability-extractor). Pure-Rust (serde_json) text tool, all 3
surfaces. Research via `WebSearch`, paraphrased.

## Competitors surveyed
| tool | does well (paraphrased) | dimension |
| ---- | ----------------------- | --------- |
| Devtoollab / markdownresume.app | Markdown input/output, live preview, ATS-friendly, in-browser, no signup | capabilities |
| Resumey.Pro / markdowntailor | auto-format to structured Markdown, multiple templates, PDF export | capabilities |
| Teal / Jobscan / ResumeBuilder | unlimited resumes, ATS optimization, PDF/Word export, 25+ templates | capabilities |

## Gap diff vs our tool
Our tool: a JSON object of fields → clean ATS-friendly Markdown (plain headings +
bullets; no tables/columns/graphics that ATS parsers choke on). Covers name/title/
contact/links/summary/experience/education/skills + arbitrary custom `sections`
(Projects, Certifications, …). Every field optional except name.

**In-model gaps considered, deferred (fit the model; good follow-ups):**
- **Multiple templates / styles** — a `template` param toggling layout/heading
  style (still plain Markdown). Easy add.
- **Reverse-chronological auto-sort** of experience/education by date — a nicety.
- **Plain-text (.txt) output** alongside Markdown — trivial via html-to-text-style
  stripping, or just the Markdown (already plain-ish).

**Out-of-model:** PDF/Word export (needs a renderer — pair with a future
markdown-to-pdf tool), live WYSIWYG editor, AI content suggestions (needs a model).

## Tested
unit (5: full resume render w/ contact line + experience + skills, minimal
name-only, custom sections, missing-name error, bad-JSON/non-object errors) +
drift-guard · wafer fixtures (1) · `wafer build` validates the block · wasm-pack
web · generator · CLI (clean Markdown resume) · Playwright page + query deep-link
(2 tests).

> Original work only — no competitor copy, branding, or trademarks copied.

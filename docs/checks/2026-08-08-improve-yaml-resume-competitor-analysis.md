# yaml-resume — competitor analysis (2026-08-08)

Scan run BEFORE implementation, per `create-next-tool` step 4. All notes are paraphrased
observations of publicly documented behaviour — no competitor copy, branding, or trademarked
wording is reproduced or reused anywhere in the block.

**Tool row:** `yaml-resume` — "Renders a YAML or JSON Resume document into a styled, print-ready
HTML/PDF resume." Type: pure.

## Duplicate / viability check (done first)

Existing neighbours were checked before anything was written:

| Block | What it actually does | Overlap verdict |
| --- | --- | --- |
| `blocks/resume-builder` | Bespoke flat JSON (`name`/`title`/`experience[]`/`skills[]`) → **ATS Markdown** | Different output format. Not a dup. |
| `blocks/resume-scaffolder` | Bespoke flat JSON → styled print-ready HTML, `theme`/`accent`/`font`/`page_size` | Closest neighbour. Its core (`core/src/lib.rs`) reads only `name`, `title`, `email`, `phone`, `location`, `links`, `summary`, `experience[]`, `education[]`, `skills[]`, `sections[]`, and hard-errors with `a 'name' field is required` on anything else. It parses **JSON only** (`serde_json`, no YAML dep) and knows **nothing** about the JSON Resume schema — a standard `resume.json` (whose name lives at `basics.name`) fails outright. Not a dup. |
| `blocks/resume-to-json` | Plain-text resume → **JSON Resume v1.0.0** document (`basics`/`work`/`education`/…), or validates one | Exact complement: it *emits* the format this tool *renders*. Nothing in the repo could render its output before this block. |

Decision: **build**. The deciding fact is the broken chain — the repo already produces JSON Resume
v1.0.0 documents (`resume-to-json`) and had no renderer for them, and the standards schema plus a
YAML front-end is a materially different input surface from `resume-scaffolder`'s six-field bespoke
shape, not a defaulted-param subset of it. To keep the two blocks unambiguous, this tool is scoped
strictly to the **JSON Resume standard schema** and says so in every surface's copy.

Viability: pure Rust. `serde_yml 0.0.12` is already proven wasm-safe in this repo (9 blocks use it,
including `json-yaml-convert` and `yaml-formatter`), and the renderer is string formatting — no new
dependency risk.

## Competitors reviewed

1. **JSON Resume (jsonresume.org) — registry + `resumed` CLI.** The reference implementation of the
   schema. Renders a `resume.json` through a pluggable theme package to HTML, then to PDF; the
   hosted registry does the same server-side. Its value is the schema itself plus a very large
   third-party theme ecosystem (hundreds of npm theme packages). Section coverage is the full
   v1.0.0 set.
2. **YAMLResume.** Resume-as-code in YAML. Builds PDF (via a LaTeX engine), plus HTML, Markdown and
   LaTeX source. Exposes template selection, font size, page margins, and i18n/locale for section
   headings; ships a watch mode, a schema `validate` command, an AI generation command, and a CI
   action.
3. **yaml2resume.** YAML → a single-column, explicitly ATS-oriented printable HTML page with plain
   HTML+CSS. Sections are personal info / summary / skills-by-category / experience / education /
   projects / interests / achievements. No theme, font, colour or page-size options today (its own
   README lists templates and a dark mode as wanted contributions).
4. **jsoncv.** Browser editor + renderer built on the JSON Resume schema (upgraded to JSON Schema
   draft-07, plus an extra `sideProjects` section and a `meta.name` field). Ships one built-in
   theme, renders to a single self-contained HTML file, and leaves PDF to the browser's print
   dialog. Custom themes require adding template + stylesheet source files.

## Table stakes → where each one landed

| Capability | Seen in | In model? | Where it landed |
| --- | --- | --- | --- |
| Accept a standard JSON Resume document | 1, 4 | yes | `data`, parsed as JSON |
| Accept the same document as YAML | 2, 3 | yes | `data` + `format = auto` sniffing; `serde_yml` |
| Auto-detect which of the two was pasted | — (ours) | yes | `format = auto\|yaml\|json` |
| Full v1.0.0 section coverage (basics, work, volunteer, education, awards, certificates, publications, skills, languages, interests, references, projects) | 1, 4 | yes | all twelve rendered |
| `basics.location` + `basics.profiles[]` in the header | 1, 4 | yes | contact line + profile links |
| Theme / template choice | 1, 2, 4 | yes | `theme = classic\|modern\|compact\|ats` |
| Explicitly ATS-safe single-column layout | 3 | yes | `theme = ats` (no colour, no rules, plain headings) |
| Accent colour | — (gap in 1–4's defaults) | yes | `accent`, `kind = "color"` on the page |
| Body font family | 2 | yes | `font = sans\|serif` |
| Base font size | 2 | yes | `font_size` pt, `kind = "slider"` 8.5–13 |
| Print page size | 2 | yes | `page_size = letter\|a4`, drives `@page size` |
| Print margins | 2 | yes | `margin` in (0.3–1.2), `kind = "slider"` |
| ISO-8601 date rendering (`2020-01` → `Jan 2020`), open-ended → "Present" | 1, 2 | yes | `date_format = month-year\|year\|iso` |
| Choose / reorder which sections appear | 2 | yes | `sections`, `kind = "tag-list"` |
| Self-contained single HTML file (no external CSS/JS) | 3, 4 | yes | styles inlined in `<head>` |
| Print → Save-as-PDF path | 3, 4 | yes | embedded print stylesheet |
| Schema validation with an actionable error | 1, 2 | yes | typed errors naming the offending path |
| Preset styles as one-click chips | 2 (templates) | yes | four `[[example]]` chips |

## Out of model (listed, deliberately not built)

- **Direct PDF bytes.** Competitor 2 reaches PDF through a LaTeX engine and 1/3/4 through a
  headless browser. Laying out styled HTML to PDF needs a full HTML layout engine, which is outside
  a pure-Rust wasm block; the shipped answer is the same one competitors 3 and 4 give users — a
  print-ready HTML document plus the browser's Print → Save-as-PDF. Stated plainly on the page.
- **Third-party theme packages.** Competitor 1's value is a registry of hundreds of npm theme
  packages; a block cannot install or execute one. Four built-in themes ship instead.
- **DOCX / LaTeX / Markdown output** (2). Different output pipelines; `resume-builder` already
  covers the Markdown resume case in this repo.
- **Hosted publishing / a resume URL** (1). Needs a server; this repo ships no hosting.
- **AI-assisted content generation** (2). Needs a model; out of the pure-Rust + ffmpeg model.
- **Watch mode / CI action / project scaffolder** (2). Dev-loop tooling, not a single-input tool
  surface.
- **Localised section headings (i18n)** (2). Headings render in English; a locale vocabulary is a
  larger surface than this row asks for.
- **Editor GUI** (4). This repo ships a form-driven page, not a WYSIWYG editor.
- **`sideProjects`, draft-07 metadata and other jsoncv schema extensions** (4). Non-standard
  additions; unknown top-level keys are ignored rather than erroring, so such documents still
  render their standard sections.

## Verification notes

Recorded after the build ran; see the commit for the block itself.

- `cargo test --workspace` in `blocks/yaml-resume/` — core + descriptor drift-guard tests.
- `scripts/build-block-wasm.sh yaml-resume` — canonical locked `Cargo.lock` + `target/block.wasm`.
- `wasm-pack build blocks/yaml-resume/web --target web --release --out-dir pkg`.
- `cargo install --path cli` → `python3 scripts/sync-tool-manifest.py yaml-resume` → generator.
- CLI exact-output run plus one run per `theme` / `format` / `date_format` value.
- `tests/tool-page-yaml-resume.spec.ts` — YAML input, a `?param=` deep link, a non-default enum,
  and the ATS theme.
- `python3 scripts/check-tool-hygiene.py yaml-resume` exits 0.

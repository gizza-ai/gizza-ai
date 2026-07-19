# resume-to-json — competitor analysis (2026-07-19)

New-tool build scan (done BEFORE implementing). Tool: extract a pasted plain-text resume into
the standard JSON Resume schema (jsonresume.org, v1.0.0) and validate resume.json documents
against that schema. Pure compute, runs on all surfaces (chat, CLI, page).

## Competitors skimmed

1. **jsonresume.org (schema + resume-cli)** — the standard itself. Defines the canonical
   v1.0.0 section set (basics/work/volunteer/education/awards/certificates/publications/
   skills/languages/interests/references/projects/meta), ISO-8601 partial dates
   (`YYYY`, `YYYY-MM`, `YYYY-MM-DD` via an explicit regex pattern), `format: email`/`uri`
   annotations, and `additionalProperties: true` at every level (unknown keys are legal).
   `resume-cli validate` checks a resume.json against the published schema. Every section is
   optional; no required fields at the top level.
2. **AllJSONTools — "JSON Resume format" guide + JSON tools** — walks users through mapping
   resume sections onto the v1.0.0 schema by hand, then pretty-printing (JSON formatter) and
   validating (JSON Schema validator). Emphasizes ISO-8601 dates and "every section is
   optional". Ships three worked JSON examples ranging up to a full realistic resume.
3. **Affinda resume parser** — commercial ML parser. Accepts file uploads (PDF/DOCX, 50+
   languages), returns normalized JSON with 50+ fields (contact, work, education, skills,
   certifications, languages, locations), interactive JSON response viewer, accuracy
   benchmarks. API/SDK oriented.

   (schema-resume.org was in the initial result set but returned HTTP 403 — replaced with
   AllJSONTools + Affinda per the "replace unreachable competitors" rule.)

## Table stakes → in-model / out-of-model

| Capability | Tag | Where it landed |
|---|---|---|
| Exact JSON Resume v1.0.0 field names + section set | in-model | Extraction emits only canonical fields; validator knows the full schema shape (fetched the live schema.json to source field names/date pattern — not from memory) |
| ISO-8601 partial dates (`YYYY`, `YYYY-MM`, `YYYY-MM-DD`) | in-model | Date-range parser normalizes "Jan 2020 – Present", "03/2020", "2020–2023" to schema dates; validator enforces the exact schema regex |
| Validate a pasted resume.json (resume-cli validate) | in-model | `mode=validate` (and `mode=auto` on JSON input) → `{valid, errors, warnings, summary}` report; type + date-pattern violations are errors; email/URL format issues and unknown keys are warnings (schema says `additionalProperties: true`) |
| Contact extraction: name, label, email, phone, url, location, profiles (LinkedIn/GitHub/Twitter) | in-model | Header tokenizer splits `\|`/`•`/`·` contact lines; profile-network detection from URL host |
| Work entries: position/company/date range/highlights/summary | in-model | Section splitter + per-block heuristics (role-keyword side detection, bullet → highlights) |
| Education: institution/studyType/area/dates/score (GPA) | in-model | Degree-keyword parsing ("Bachelor of Science in X" → studyType + area), `GPA: 3.8` → score |
| Skills grouping ("Languages: Python, Rust" → name + keywords) | in-model | Colon-grouped lines → `{name, keywords}`; bare lists → per-item `{name}` |
| Languages with fluency ("English (Native)") | in-model | `(…)`, `–`, `:` fluency forms |
| Projects / certificates / awards / volunteer / publications / interests / references sections | in-model | Per-section heuristics emitting canonical fields |
| Pretty-printed output (formatter table stake) | in-model | `pretty` boolean, default true |
| `$schema` reference + `meta` block (official samples ship it) | in-model | `schema_ref` boolean adds `$schema` + `meta.version` to extracted output |
| Sample resume one click away (competitors ship worked examples) | in-model | `[[example]]` preset chips: sample resume → extract; resume.json → validate report |
| PDF/DOCX/image upload + OCR | out-of-model | Needs file parsing/OCR pipeline; this tool takes pasted plain text (limit stated on page). `pdf-extract-text` exists in the toolkit as the upstream step |
| ML/semantic parsing, 50+ languages, 95%+ accuracy claims | out-of-model | Heuristic English-heading parser; limits stated honestly on the page |
| Themes / HTML / PDF rendering of resume.json | out-of-model here | Inverse direction already covered by `blocks/resume-builder` (JSON → ATS Markdown) — page cross-references the concept generically |
| Hosted registry / gist publishing | out-of-model | Server-side product feature, not a compute tool |

## UX control patterns matched

- Big multiline paste box for the resume (competitors are paste/upload-first).
- `mode` as a `<select>` with friendly labels (`[input.labels]`): Auto-detect / Extract / Validate.
- Checkboxes for `pretty` (default ON) and `schema_ref` (default OFF).
- `[[example]]` chips: "Sample resume → JSON" and "Validate a resume.json" (one intentionally
  containing a bad date + unknown key so the report shows errors AND warnings).

## Design decisions

- `mode=auto` (default): input that parses as a JSON object → validate; anything else → extract.
  Mirrors how users paste either surface into one box.
- Extraction output is schema-valid by construction — a unit test runs the extractor's output
  back through the validator and asserts zero errors.
- Severity policy (documented in FAQ): wrong types + bad date patterns = errors (schema
  pattern/type violations); malformed email/URL + unknown keys = warnings (schema declares
  `additionalProperties: true`, and JSON-Schema `format` is an annotation).
- Input cap: 1 MiB of text (tested at and one over the boundary; stated on the page).

No competitor copy, branding, or trademarks were reused; all page copy is original and generic.

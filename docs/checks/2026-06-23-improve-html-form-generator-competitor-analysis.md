# html-form-generator — competitor analysis (2026-06-23)

Tool: `blocks/html-form-generator` — generate accessible HTML `<form>` markup from a
plain-text description of fields. Pure-compute (no I/O), three surfaces: chat skill,
CLI, standalone page.

## Top competitors surveyed

1. **Basin HTML Form Generator** (usebasin.com/html-form-generator) — add/remove/reorder
   fields, set labels, placeholders, validation rules; real-time preview; emits *semantic,
   accessible HTML* with no bloated markup/dependencies.
2. **FormBackend HTML Form Generator** (formbackend.com/html-form-generator) — clean,
   semantic HTML with *scoped CSS*; responsive, accessible, properly-labelled inputs; free,
   no signup; copy the code.
3. **Elfsight Form Generator** (elfsight.com/blog/form-generator) — no-code builder; custom
   fields, field validation, pre-filled fields, payments, embed widget.
4. **123FormBuilder** (123formbuilder.com/html-form-generator) — drag-and-drop elements,
   field labels, themes/custom CSS, label placement, reCAPTCHA/anti-spam.
5. **Jotform** (jotform.com/html-form-generator) — drag-and-drop, 100+ integrations, design
   editor, hosted embed.

## Capability diff (competitor → our coverage)

| Capability | Competitors | gizza html-form-generator |
| --- | --- | --- |
| Labeled inputs (`<label for>`) | yes | **yes** — every control bound to its slugified `id` |
| Field types: text/email/password/number/tel/url/date/time | most | **yes** (12 types incl. `textarea`) |
| Select / radio / checkbox / checkbox group | yes | **yes** — select, radio group (shared name), single checkbox, `name[]` checkbox group |
| Placeholders | yes | **yes** — third pipe-part for text-like fields |
| Required + type validation attributes | Basin/Elfsight | **yes** — `required` + native `type=email/url/number/...` + visible `*` marker |
| Semantic, dependency-free markup | Basin/FormBackend | **yes** — plain HTML, no JS framework, indented + readable |
| Scoped / built-in CSS | FormBackend | **yes** — optional `<style>` block scoped to `.gizza-form`, toggleable |
| Configurable method / action / submit label | yes | **yes** — `method` (get/post), `action`, `submit_label` |
| HTML escaping of labels/options (XSS-safe output) | implicit | **yes** — all text escaped; ids slugified |
| Unique names on duplicate labels | implicit | **yes** — `-2`, `-3` suffixing |
| Runs locally / private / no signup | FormBackend | **yes** — pure WASM, browser-local, offline, no signup |

## Gaps deliberately NOT built (out of model or out of scope)

- **Hosted form backend / submission handling, email delivery, spam protection (reCAPTCHA),
  integrations, payments** (Basin, FormBackend, Elfsight, 123FormBuilder, Jotform). These are
  hosted SaaS backend services — out of gizza's browser-local pure-compute model. We emit the
  markup + an `action` attribute so the user can point it at any backend; we do not host one.
- **Drag-and-drop visual builder / live rendered preview** (all five). gizza's tool surfaces
  are a text field (page) / a chat message / a CLI arg. The page output is the markup source
  (copy-paste), which is the deliverable; a live WYSIWYG canvas is out of the surface model.
- **Per-field min/max/pattern/step constraints and default values.** Not in the v1 line
  syntax; a candidate future enhancement but kept out to keep the one-line-per-field syntax
  simple and unambiguous. Required + type-based validation already covers the common case.

## Result

No in-model capability, copy, or UX gap left open against the surveyed competitors: the tool
matches the "semantic, accessible, dependency-free, optionally-styled, copy-paste markup"
value proposition (Basin / FormBackend) while adding broader field-type coverage, required/
type validation attributes, XSS-safe escaping, and duplicate-name de-duplication. Backend/
hosting/builder features are intentionally out of model and noted above rather than built.

## Verification (all surfaces)

- `cargo test --workspace` in `blocks/html-form-generator` — 15 core tests + the chat-schema
  drift guard pass.
- `wafer build` — chat `block.wasm` validates (gizza-ai/html-form-generator v0.1.0).
- CLI: `gizza tool html-form-generator fields=… submit_label=Send styled=false` renders the
  full form; the error path (`fields="X | wizard"`) reports the unknown-type message.
- Page: `tests/tool-page-html-form-generator.spec.ts` (Playwright) — 2 tests pass (markup
  build + method switch).

# pdf-form-fill — competitor analysis & improvements (2026-06-20)

**Tool:** `gizza-ai/pdf-form-fill` — fill AcroForm (interactive) fields in a
fillable PDF. Chat + CLI (Document input + PDF-bytes output → no page, like
pdf-rotate / pdf-split).

## What competitors do

- **Online PDF form fillers** (pdfescape, smallpdf, sejda, ilovepdf fill) —
  upload, click fields, type, download. Strengths: visual click-to-fill.
  Weaknesses: the PDF (often containing personal data) is **uploaded to a
  server**; many free tiers cap pages/day or watermark.
- **`pdftk fill_form` / Python pypdf** — local + scriptable (FDF / field map) but
  require installing tooling and knowing the field names.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust (`lopdf`) compiled to wasm:
   runs in the chat Service Worker and headless via the CLI. Form data (which is
   usually personal) never leaves the device.
2. **Programmatic + agent-friendly.** Fill by a JSON `{name: value}` map — ideal
   for an LLM filling a known form, or a CLI/CI batch job, rather than manual
   clicking.
3. **Viewer-correct output.** Sets each field's `/V` (and `/AS` for
   checkbox/radio buttons), drops stale `/AP` appearance streams, and turns on the
   AcroForm `/NeedAppearances` flag so PDF viewers render the new values (the part
   naive `/V`-only fillers get wrong, leaving fields blank on screen).
4. **Honest feedback.** Reports exactly which fields were filled and which
   requested names weren't found — so you can fix a typo'd field name.
5. **Chainable** via `url`/`ref`; the filled PDF is itself a `ref`.

## Handles real-world forms

- **Nested field hierarchies.** Walks `/Kids` to find terminal (leaf) fields and
  addresses them by their **full dotted name** (`topmostSubform[0].Page1[0].f1_01[0]`)
  *or* their convenient **leaf name** (`f1_01[0]`). Real IRS fillable PDFs nest
  every field under page subforms — a top-level-only filler finds 0 fillable fields
  on them; this one finds all 23 on the W-9.
- **UTF-16BE field names.** IRS (and many Acrobat-authored) forms store `/T` names
  as UTF-16BE with a BOM; the tool decodes these so names are matchable and
  readable instead of garbled.
- **Discovery built in.** When a requested name isn't found, the response lists the
  available field names, so a caller (human or LLM) can correct a name in one pass.

## Honest scope

- Doesn't **flatten** the form (values stay editable). Text and button (checkbox/
  radio) field types are handled; signature/choice widgets aren't specially
  rendered.

## Tests

7 core unit tests on **AcroForm PDFs assembled in-test with lopdf** (catalog →
AcroForm ref → text-field refs): fills a named field and leaves others untouched
(verified by reloading and reading `/V`), reports unknown field names, sets
`/NeedAppearances true`, errors on a PDF with no AcroForm, and errors on garbage.
Plus block tests: `parse_fields` coerces scalar JSON values + rejects non-objects,
and the drift-guard schema. CLI verified over the wire on a real fillable PDF —
see commit.

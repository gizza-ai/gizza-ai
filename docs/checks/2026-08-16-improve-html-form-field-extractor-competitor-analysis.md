# html-form-field-extractor — competitor analysis (2026-08-16)

Scan run BEFORE implementing, per `/improve-tool` Phase 2. Two WebSearches
(`HTML form field extractor tool list form inputs names types required attributes`,
`extract form fields from HTML python parse input select textarea attributes
required placeholder pattern`) plus a skim of the top real references. No
competitor copy, wording, or branding was reused — only the *capability set* was
compared.

## Competitors reviewed

| # | Reference | What it does | Notable |
|---|-----------|--------------|---------|
| 1 | The Python Code — "Extract and Submit Web Forms from a URL" (`thepythoncode.com/article/extracting-and-submitting-web-page-forms-in-python`) | BeautifulSoup recipe: fetch a page, enumerate every `<form>`, list its controls, then re-submit | Per form it captures `action` + `method`; per control `type`, `name`, `value`; treats `<select>` as `type: "select"` with a `values` list of options and `value` = the selected one, and `<textarea>` as `type: "textarea"` with its text as `value`. Explicitly does **not** capture `id`, `required`, or `placeholder`, and has no checkbox/radio-specific handling. Output is a dict `{action, method, inputs: [...]}` |
| 2 | `seriyps/python-form-parser` (GitHub) | `FormFiller` class: parse an HTML form, read its defaults, fill fields, re-serialize | Form model is `action` / `method` (defaults to GET) / `id`; field model is `type` / `name` / `value`. Validates that a field you try to fill actually exists on the form (raises on unknown names). Its own TODO admits incomplete type coverage (e.g. `file`) — no documented `checked` / `selected` / `disabled` / `multiple` handling |
| 3 | MDN — `<input>` element reference (`developer.mozilla.org/en-US/docs/Web/HTML/Reference/Elements/input`) | The canonical definition of what a form field *is* | 22 input types (text, email, url, tel, password, search, number, range, date, month, week, time, datetime-local, color, checkbox, radio, file, hidden, submit, reset, button, image) and the full constraint-attribute set: `required`, `pattern`, `min`, `max`, `step`, `minlength`, `maxlength`, `readonly`, `disabled`, `multiple`, `accept`, `autocomplete`, `placeholder`, `value`, `list`. Also documents which attributes are meaningful for which types, and the `ValidityState` flags each attribute maps to |

Also noted across the search results: every practical write-up reaches for a real
HTML parser (BeautifulSoup / html5-class parser), never a regex, because forms in
the wild have unquoted attributes, implied closing tags, and nested markup.

## Table stakes → decisions

| Capability | Refs | Decision |
|---|---|---|
| Enumerate every `<form>` in the document | 1, 2 | **Build** — the core. Forms are indexed 0-based; `form_index` picks one, `-1` (default) lists them all. |
| Form `action` + `method` | 1, 2 | **Build** — plus `id`, `name`, and `enctype` (needed to know whether a `file` input can actually upload). `method` is normalized to lowercase and defaults to `get`, matching the HTML spec (and ref 2's behaviour). |
| Per-field `type` | 1, 2, 3 | **Build** — `type` defaults to `text` for a bare `<input>`, exactly as the spec says; `<select>`/`<textarea>`/`<button>` report their tag as the type, matching ref 1's convention. `tag` is reported separately so the two are never conflated. |
| Per-field `name` and default `value` | 1, 2, 3 | **Build** — `name` and `default` (the initial value: the `value` attribute, a `<textarea>`'s text, or the selected `<option>`s). |
| Per-field `id` | 2 (form-level only) | **Build** — ref 1 skips it; it's the join key for `<label for=…>`, so it's table stakes for an *analyzer* rather than a submitter. |
| `required` flag | 3 | **Build** — a first-class boolean column, not buried in an attribute bag. |
| `placeholder` | 3 | **Build** — its own column. |
| Validation attributes: `pattern`, `min`, `max`, `step`, `minlength`, `maxlength` | 3 | **Build** — grouped under `validation` in JSON, and as columns in CSV/Markdown. |
| `<select>` options with their labels + which is selected | 1 | **Build** — `options: [{value, label, selected}]`, including `<optgroup>` children. Ref 1 only returns bare option values; carrying the visible label too is what makes the output readable. |
| State flags `disabled`, `readonly`, `checked`, `multiple` | 3 (ref 1/2 lack them) | **Build** — reported per field. `disabled` matters because such fields are *not* submitted; `checked` is the default state of a checkbox/radio. |
| `accept` + `autocomplete` | 3 | **Build** — part of `validation`; `accept` is the only way to know what a `file` input takes. |
| Machine-readable output | 1, 2 | **Build** — `format = "json"` (default, nested forms→fields) plus `csv` (one flat row per field, spreadsheet-ready) and `markdown` (a table per form, for docs/PR review). |
| Real HTML parser, not regex | 1, 2, 3 | **Build** — `scraper` (html5ever), the same wasm-safe engine `html-extract` and `html-table-extractor` already use. Handles unquoted attributes, implied tags, and malformed nesting. |
| Label text for each control | — (gap in all three) | **Build** — resolved via `<label for=id>`, then a wrapping `<label>`, then `aria-label`, then `title`. None of the three references do this; it's the single biggest readability win for a form *audit*. |
| Controls that sit outside any `<form>` | — (gap in all three) | **Build** — modern JS-driven forms are often `<div>`-based with no `<form>` tag at all. Orphan controls are grouped into a trailing pseudo-form flagged `unattached: true` so they aren't silently dropped. |
| Buttons (`submit`/`reset`/`button`/`image`) | 1 (includes them indistinguishably) | **Build, opt-in** — `include_buttons`, default off, because they're not data fields; ref 1 mixes them into the same list with no way to filter. |
| Hidden inputs | 1, 2 | **Build** — `include_hidden`, default **on**: hidden fields (CSRF tokens, tracking IDs) are exactly what a form audit wants to see. |

## Deliberately not built (out of model / already covered)

- **Fetching a form from a live URL** (refs 1 and 2 both start from `requests.get`) — this is a
  pure block: paste the HTML, or use the existing `blocks/web-fetch` / `blocks/css-select-extract`
  network tools to retrieve it first. Keeps the tool deterministic and offline.
- **Submitting / filling the form** (ref 2's whole `FormFiller` purpose) — that's an I/O action, not
  an extraction, and it needs network. `blocks/form-field-validator` already covers validating a set
  of submitted values offline.
- **Live constraint evaluation** (MDN's `ValidityState`) — reporting whether a *given value* passes
  `pattern`/`min`/`max` needs values, which this tool doesn't take. The attributes are reported so a
  reader can apply them; `blocks/form-field-validator` is the tool that checks values.
- **Rendering the form** — that's `blocks/html-form-generator` (the inverse direction).
- **Following `<input form="other-form-id">` cross-references** — the `form` attribute lets a control
  claim membership in a form it isn't nested inside. Reported as a `form_attr` value on the field
  rather than re-parenting the control, so the output always mirrors the actual document tree.

## Not a duplicate

- `blocks/html-extract` runs an arbitrary CSS selector and returns text / inner-HTML / outer-HTML /
  one attribute per match. It cannot enumerate forms, cannot report more than one attribute at a
  time, and has no notion of a form control — you'd need several runs and manual joining.
- `blocks/html-form-generator` goes the other way: field descriptions → `<form>` markup.
- `blocks/form-field-validator` takes `name: value` submission data and checks formats
  (email/phone/postcode/card). It never parses HTML.
- `blocks/html-table-extractor` handles `<table>`, not `<form>`.
- `blocks/pdf-form-data-extract` reads AcroForm fields out of a PDF, a different container entirely.

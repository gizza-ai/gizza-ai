## About this tool

Paste the HTML of a page or a single `<form>` and this tool walks the markup with a real HTML
parser and reports **every form control it finds** — `<input>`, `<select>`, `<textarea>`, and
optionally `<button>` — together with everything a developer, tester, or auditor needs to know
about it:

- **Identity** — the element's tag, its effective `type` (a bare `<input>` is `text`, per the HTML
  spec), its `name`, and its `id`.
- **Label** — the visible text a user actually sees, resolved from `<label for="…">`, a wrapping
  `<label>`, `aria-label`, or `title`.
- **Required flag** — whether the field carries the `required` attribute.
- **Default value** — the `value` attribute, a `<textarea>`'s text, or the selected `<option>`(s).
- **Placeholder** — the hint text shown in an empty field.
- **Validation rules** — `pattern`, `min`, `max`, `step`, `minlength`, `maxlength`, `accept`, and
  `autocomplete`.
- **State flags** — `disabled`, `readonly`, `checked`, and `multiple`.
- **Dropdown options** — every `<option>` with its submitted value, its visible label, its
  `<optgroup>`, and whether it is selected.

Each form is reported with its `action`, `method` (lowercased, defaulting to `get`), `id`, `name`,
and `enctype`, so you can see exactly where the data goes.

### Worked example

Input HTML:

```html
<form id="signup" action="/register" method="post">
  <label for="email">Email address</label>
  <input type="email" id="email" name="email" required placeholder="you@example.com" maxlength="64">
  <label>Age <input type="number" name="age" min="18" max="120"></label>
  <input type="hidden" name="csrf" value="tok123">
  <button type="submit">Sign up</button>
</form>
```

Output with **Markdown** selected:

```markdown
## Form 0 — POST /register

id: `signup`

| # | Name | Type | Label | Required | Default | Validation |
|---|------|------|-------|----------|---------|------------|
| 1 | `email` | `email` | Email address | yes | — | maxlength=`64` |
| 2 | `age` | `number` | Age | no | — | min=`18`, max=`120` |
| 3 | `csrf` | `hidden` | — | no | `tok123` | — |
```

The submit button is absent because **Include buttons** is off by default — buttons are actions,
not data fields. Switch it on and a fourth row appears. Choose **JSON** for the same data as a
nested structure, or **CSV** for one flat row per field that pastes straight into a spreadsheet.

### Good uses

- **Write an API client or a scraper** — get every field name and its default in one pass, instead
  of reading the markup by hand.
- **Audit a form** — spot missing `required` flags, missing labels (an accessibility problem),
  fields with no `name` (they are never submitted), and `disabled` fields.
- **Review hidden state** — CSRF tokens, cart ids, and tracking fields are shown by default.
- **Document a form** — paste the Markdown table into a README, a ticket, or a PR.
- **Build test fixtures** — the CSV lists every field, type, and constraint to generate cases from.

### Limits and edge cases

- Reports at most **2000 fields** per run. Past that you get an error asking you to pick a single
  form with the form index or split the HTML.
- Everything runs locally in your browser via WebAssembly. Nothing is uploaded, and the tool has
  no network access — it cannot fetch a URL for you, so paste the markup (View Source, or your
  browser's DevTools → Elements → Copy → Copy outerHTML).
- Fields are read from the **static markup**. Controls that JavaScript adds after page load are not
  in the source you copied from View Source; copy from DevTools instead to capture them.
- A `<select>` with no `selected` option reports its **first** option as the default, which is what
  browsers submit. A `multiple` select with nothing selected reports an empty default.
- Radio buttons and checkboxes that share a `name` are listed as separate fields, one per element,
  because each has its own `value` and `checked` state.

## FAQ

<details>
<summary>Why is the submit button missing from my results?</summary>

Buttons are excluded by default because they are actions rather than data fields. Turn on
**Include buttons** to add every `<button>` plus `<input>` controls of type `submit`, `reset`,
`button`, and `image`.

</details>

<details>
<summary>What happens to inputs that are not inside a &lt;form&gt; tag?</summary>

They are not dropped. Any control that sits outside every `<form>` is collected into one extra
group at the end, flagged `unattached` in JSON output and titled "controls outside any &lt;form&gt;"
in Markdown. Modern JavaScript-driven forms are frequently `<div>`-based with no `<form>` element
at all, and those fields still matter.

If a control carries a `form="some-id"` attribute — claiming membership in a form it is not nested
inside — that attribute is reported as `form_attr` on the field, but the control stays in the group
matching its real position in the document, so the output always mirrors the actual markup.

</details>

<details>
<summary>How is the label for each field worked out?</summary>

Four sources are tried in order: a `<label for="…">` pointing at the field's `id`, a `<label>` that
wraps the field, the field's `aria-label` attribute, and finally its `title`. If none of those
exist the label is empty — which is usually worth fixing, since a field with no label is hard to
use with a screen reader.

</details>

<details>
<summary>Can it read a form from a URL instead of pasted HTML?</summary>

No. This tool is fully offline — it has no network access at all, which is why your markup never
leaves your machine. Fetch the page yourself (View Source, `curl`, or DevTools → Elements → Copy
outerHTML) and paste the result here.

</details>

<details>
<summary>Does it handle messy or invalid HTML?</summary>

Yes. Parsing uses the same HTML5 parsing algorithm browsers use, so unquoted attribute values,
unclosed `<p>` and `<li>` tags, mixed-case tag names, and sloppy nesting all parse the way a
browser would read them. That is the main reason to use this rather than a regular expression,
which breaks on all of the above.

</details>

<details>
<summary>What is the difference between the "tag" and "type" columns?</summary>

`tag` is the literal element — `input`, `select`, `textarea`, or `button`. `type` is the effective
control type: for an `<input>` it is the `type` attribute, defaulting to `text` when absent; for a
`<button>` it is the `type` attribute, defaulting to `submit`; and for `<select>` and `<textarea>`
it repeats the tag name, since those elements have no type attribute. Keeping both means a
`<select>` is never confused with `<input type="select">`, which is not a real thing.

</details>

<details>
<summary>Can I extract just one form from a whole page?</summary>

Yes — set **Form index** to the form's 0-based position in the document (`0` for the first form,
`1` for the second, and so on). Leave it at `-1` to report every form. The index of the unattached
group is one past the last real form.

</details>

## Résumé scaffolder

Give your details as a JSON object and get back a clean, **print-ready HTML
résumé** — a single self-contained document with styling and a print stylesheet
baked in. Open it, then use your browser's **Print → Save as PDF** to get a tidy,
one-margin PDF. Pick a **theme**, **accent color**, **font**, and **page size**;
everything runs locally in your browser and nothing is uploaded.

Prefer plain-text/ATS Markdown instead of a styled document? Use the sibling
**Resume Builder** tool — this one is for a formatted, printable page.

### Worked example

Input (`data`), with theme `modern`, accent `#2563eb`, font `sans`, page size
`letter`:

```json
{
  "name": "Ada Lovelace",
  "title": "Software Engineer",
  "email": "ada@example.com",
  "location": "London",
  "summary": "Pioneering engineer who wrote the first published algorithm.",
  "experience": [
    { "role": "Engineer", "company": "Analytical Co", "dates": "1843–1852",
      "bullets": ["Wrote the first algorithm", "Designed loop constructs"] }
  ],
  "education": [
    { "degree": "Mathematics", "school": "Private tutoring", "dates": "1830s" }
  ],
  "skills": ["Algorithms", "Mathematics", "Technical writing"]
}
```

Output is a complete HTML document that begins:

```html
<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Ada Lovelace — Résumé</title>
<style> … </style>
</head>
<body>
<main class="resume theme-modern">
<header class="r-head">
<h1>Ada Lovelace</h1>
<p class="r-title">Software Engineer</p>
…
```

with the name as an `<h1>`, a contact line, and `Summary`, `Experience`,
`Education`, and `Skills` sections styled with your blue accent — ready to print.

### The JSON shape

All fields are optional except `name`:

- `name`, `title`, `email`, `phone`, `location`, `links` (array)
- `summary` — a short paragraph
- `experience` — array of `{ role, company, location, dates, bullets[] }`
- `education` — array of `{ degree, school, location, dates, details }`
- `skills` — array of strings (rendered as chips)
- `sections` — array of `{ heading, items[] }` for extras (Projects,
  Certifications, Awards, Languages, …)

### Style controls

- **theme** — `classic` (serif, centered header, ink-ruled titles), `modern`
  (accent bar beside each section), or `compact` (tighter spacing for one-page fit)
- **accent** — a hex value (`#2563eb`) or a CSS color name (`navy`); used for
  section rules and titles
- **font** — `sans` or `serif`
- **page_size** — `letter` (US Letter) or `a4`; controls the `@page` size used
  when printing

### Limits

- Input must be a single **JSON object** (not an array or plain text).
- `name` is required; every other field is optional and omitted sections don't
  render.
- The output is HTML, not a PDF file — printing to PDF is a one-click browser
  step (there's no server-side PDF renderer).
- All résumé text is **HTML-escaped**, so `<`, `>`, `&`, and quotes in your
  content show literally and can't inject markup.

<details>
<summary>How do I turn the HTML into a PDF?</summary>

Copy or download the HTML, open it in your browser, and use **Print** (Ctrl/Cmd-P)
→ **Save as PDF**. The document ships with an `@media print` stylesheet and an
`@page` size (US Letter or A4), so the printed page drops the on-screen shadow and
background and uses clean margins. There's no separate export step — the browser's
own print dialog is the PDF renderer.

</details>

<details>
<summary>Which JSON fields are required?</summary>

Only `name`. A résumé without it is rejected with `a 'name' field is required`.
Everything else — `title`, `email`, `phone`, `location`, `links`, `summary`,
`experience`, `education`, `skills`, and `sections` — is optional, and any section
you leave out is simply omitted rather than rendered as an empty heading.

</details>

<details>
<summary>How do I add Projects, Certifications, or Languages?</summary>

Use the `sections` array. Each entry is `{ "heading": "Projects", "items": ["First
project…", "Second project…"] }`, and each renders as its own titled section with a
bullet list. That's the escape hatch for anything the built-in `experience` /
`education` / `skills` fields don't cover — Awards, Publications, Volunteering, and
so on.

</details>

<details>
<summary>What do the themes and accent color change?</summary>

`theme` picks the layout: **modern** puts an accent bar beside each section title,
**classic** uses a serif, centered header with ink-black rules (it ignores the
accent color on purpose), and **compact** shrinks spacing so more fits on one page.
The **accent** color drives the section rules and titles for the modern and compact
themes — pass a hex value like `#22c55e` or a CSS color name like `teal`.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The scaffolder is compiled to WebAssembly and runs entirely in your browser
tab — your résumé details never leave your device.

</details>

<details>
<summary>Why won't my input parse?</summary>

The input must be a single **JSON object**. The tool reports `invalid JSON` for
syntax errors (a trailing comma, unquoted keys) and `expected a JSON object of
résumé fields` if you paste an array or plain text. Note that `skills` is an array
of strings and `experience[].bullets` is an array of strings — wrap multi-value
fields in `[ … ]`.

</details>

## Resume builder

Give your details as a JSON object and get back a clean, **ATS-friendly** Markdown
resume — plain headings and bullet points, no multi-column layouts, tables, or
graphics that applicant-tracking systems mis-parse. It runs locally in your
browser; nothing is uploaded.

### The JSON shape

All fields are optional except `name`:

- `name`, `title`, `email`, `phone`, `location`, `links` (array)
- `summary` — a short paragraph
- `experience` — array of `{ role, company, location, dates, bullets[] }`
- `education` — array of `{ degree, school, location, dates, details }`
- `skills` — array of strings
- `sections` — array of `{ heading, items[] }` for extras (Projects,
  Certifications, Awards, …)

### Why Markdown?

Markdown pastes cleanly into most resume sites, converts to HTML or PDF, and —
because it's plain text with simple headings — is exactly what ATS parsers read
most reliably.

### FAQ

<details>
<summary>Which JSON fields are required?</summary>

Only `name` — a resume without it is rejected with an error. Everything else
(`title`, `email`, `phone`, `location`, `links`, `summary`, `experience`,
`education`, `skills`, `sections`) is optional, and sections you leave out are
simply omitted from the Markdown instead of rendering empty headings.

</details>

<details>
<summary>How do I add a Projects or Certifications section?</summary>

Use the `sections` array: each entry is `{ "heading": "Projects", "items":
["First project…", "Second project…"] }`, and each renders as its own `##`
heading with a bullet list. That's the escape hatch for anything the built-in
`experience` / `education` / `skills` fields don't cover — Awards, Publications,
Languages, and so on.

</details>

<details>
<summary>Why won't my input parse?</summary>

The input must be a single **JSON object** — the tool reports "invalid JSON" for
syntax errors (a trailing comma, unquoted keys) and "expected a JSON object of
resume fields" if you paste an array or plain text. Note that `skills` is an array
of strings and renders as one comma-separated line, while `experience[].bullets`
is an array of strings that becomes bullet points.

</details>

<details>
<summary>Is my data uploaded?</summary>

No — the builder is compiled to WebAssembly and runs
entirely in your browser tab.

</details>

<details>
<summary>Can I convert the result to a PDF?</summary>

Render the Markdown to HTML (any Markdown
viewer) and print to PDF, or use a Markdown-to-PDF tool.

</details>

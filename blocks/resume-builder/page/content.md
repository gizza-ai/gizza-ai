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
<summary>Is my data uploaded?</summary>

No — the builder is compiled to WebAssembly and runs
entirely in your browser tab.

</details>

<details>
<summary>Can I convert the result to a PDF?</summary>

Render the Markdown to HTML (any Markdown
viewer) and print to PDF, or use a Markdown-to-PDF tool.

</details>

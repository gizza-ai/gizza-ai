## About this tool

Paste a JSON Resume v1.0 document as YAML or JSON and render it into one self-contained HTML résumé. The output embeds its stylesheet, includes an `@page` print rule, escapes all résumé text, and only turns safe `http`, `https`, and `mailto` URLs into links.

Use the theme control for a polished modern page, a centered classic layout, a compact one-pager, or an ATS-friendly single-column version. The section list lets you hide or reorder standard JSON Resume sections such as `work`, `education`, `projects`, and `skills` without editing the source document.

### Worked example

Input:

```yaml
basics:
  name: Ada Lovelace
  label: Analytical Engineer
  email: ada@example.com
work:
  - name: Analytical Engine Co
    position: Lead Engineer
    startDate: "1843-01"
    endDate: "1852-11"
    highlights:
      - Published the first algorithm intended for a machine
skills:
  - name: Mathematics
    keywords: [Algorithms, Analysis]
```

With the modern theme and month-year dates, the result contains an HTML document headed `Ada Lovelace`, an Experience section dated `Jan 1843 – Nov 1852`, and a Skills section listing `Algorithms, Analysis`.

### Limits and edge cases

- The input must follow the JSON Resume shape with `basics.name`; unknown keys are ignored.
- This tool returns HTML. Use your browser's print dialog to save a PDF.
- It does not fetch remote images, fonts, or themes. Everything stays in the generated document.
- Dates are formatted when they look like ISO `YYYY`, `YYYY-MM`, or `YYYY-MM-DD`; informal dates pass through unchanged.

## FAQ

<details>
<summary>Can this create a PDF directly?</summary>

It returns print-ready HTML with an embedded `@page` rule. Open or preview the HTML in a browser, then use Print → Save as PDF so your local browser handles the final PDF rendering.

</details>

<details>
<summary>What schema does the input use?</summary>

It expects the standard JSON Resume v1.0 shape: `basics`, `work`, `education`, `projects`, `skills`, `awards`, `certificates`, `publications`, `languages`, `interests`, and `references`. YAML and JSON are both accepted.

</details>

<details>
<summary>Is the generated HTML safe to paste into a browser?</summary>

Résumé text is HTML-escaped, style values are validated, and unsafe URL schemes are printed as text instead of links. You should still review the final output before sending it to anyone.

</details>

<details>
<summary>Which theme should I use for applicant tracking systems?</summary>

Use the `ats` theme. It avoids decorative rules and color, keeps a simple single-column structure, and leaves section headings as plain text.

</details>

## Validate schema.org markup before you publish

Structured data helps search engines understand pages, products, recipes, articles, breadcrumbs, and organizations. This tool scans pasted HTML for the three common embedded formats — JSON-LD, microdata, and RDFa — and reports what it found.

## What it checks

- JSON-LD blocks in `<script type="application/ld+json">`.
- Missing or non-schema.org `@context` values.
- Objects that do not declare an `@type`.
- Microdata `itemscope`, `itemtype`, and `itemprop` relationships.
- `itemprop` attributes that are not inside any `itemscope`.
- RDFa `typeof` / `property` blocks and missing vocabularies.

## Output formats

Choose **report** for a readable summary with errors and warnings, or **json** for a machine-readable `{ counts, items, issues, valid }` result that can be copied into tests or SEO QA notes.

## Privacy

Everything runs locally in your browser through WebAssembly. Your HTML is not uploaded to a server.

## FAQ

<details>
<summary>Is this the same as Google's Rich Results Test?</summary>
<div>No. This is a fast local validator for common schema.org markup issues. Search-specific eligibility rules, live crawling, and Google-specific enhancements remain out of scope.</div>
</details>

<details>
<summary>Can it validate malformed HTML?</summary>
<div>The parser is browser-like and forgiving, so it can scan real-world fragments such as a copied `<head>` section or a single product card.</div>
</details>

<details>
<summary>Which structured data formats are supported?</summary>
<div>JSON-LD, microdata, and RDFa. The report groups entities by format and lists detected types and properties.</div>
</details>

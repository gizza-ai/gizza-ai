## SEO Audit

Paste a page's HTML and get an instant on-page SEO report with a 0–100 score.
It runs locally in your browser; nothing is uploaded.

### What it checks

- **Title tag** — present, non-empty, and within the recommended 10–60 characters.
- **Meta description** — present and within the recommended 50–160 characters.
- **H1 heading** — exactly one `<h1>` on the page.
- **Heading hierarchy** — no skipped levels (e.g. an `<h2>` jumping straight to `<h4>`).
- **Image alt text** — every `<img>` has non-empty `alt` text.
- **Canonical link** — a `<link rel="canonical">` is present.
- **Open Graph tags** — `og:title`, `og:description` and `og:image` for rich social previews.

Each check is scored as a pass, warning, or failure, and the results roll up
into an overall score and letter grade.

### Good for

- A quick on-page SEO sanity check before publishing a page.
- Spotting missing meta tags, alt text, or social-preview tags.
- Reviewing a CMS export or a hand-written template.

### FAQ

<details>
<summary>Is my HTML uploaded?</summary>

No — the audit is compiled to WebAssembly and runs
entirely in your browser tab.

</details>

<details>
<summary>Does it crawl my site or fetch the live page?</summary>

No. It only analyses the HTML
you paste — view-source and paste, or paste your template.

</details>

<details>
<summary>What score is good?</summary>

90+ (grade A) means all the basics are covered. Warnings
are worth fixing but won't block indexing; failures usually should be addressed.

</details>

# newsletter-html-builder competitor analysis — 2026-08-18

Backlog item: `newsletter-html-builder` — build responsive, email-client-safe newsletter HTML from simple sections (header, text, button, image).

## Sources skimmed

| Competitor | What it exposes | Table-stakes patterns observed | Fit decision |
| --- | --- | --- | --- |
| Gera Tools — HTML Email Template Builder | A form-driven responsive email template generator with preheader, branded header/body/footer sections, CTA button and table-safe output. | Section-oriented content, subject/preheader text, background/accent styling, single-column email width, bulletproof CTA, inline styles and MSO/Outlook compatibility. | In-model: sections textarea, subject/title, hidden preheader, width, colour controls, inline CSS, table layout, Outlook ghost table, CTA button. Out-of-model: hosted image management, brand asset library, visual drag-and-drop editor. |
| Campaign Monitor — Bulletproof email buttons | A focused email-button generator with text, link, colour, size, border radius and Outlook-aware markup. | Button label + URL, accent/background colour, fixed padding, HTML that survives Outlook/Word rendering, copyable markup examples. | In-model: `button | label | url` section, accent colour, table-cell based button pattern, URL validation. Out-of-model: VML-specific button variants and interactive button-only preview controls. |
| HTML Email — Responsive CSS Inliner | Paste full HTML/CSS and get email-safe inlined output intended for broad email-client compatibility. | Inline CSS as a baseline requirement, responsive/mobile media query retention, copyable HTML output, warnings about client support limits. | In-model: generate already-inline CSS, keep one mobile media query, document limits. Out-of-model: arbitrary CSS parsing/inlining from user-provided full templates. |
| Rubtin — Responsive Newsletter Generator | Newsletter generator/content guidance around responsive templates and inline CSS. | Newsletter sections, mobile responsiveness, inline styling, template defaults and worked examples. | In-model: simple section syntax, preset examples, 600px default width and mobile stacking. Out-of-model: multi-page marketing workflow, image hosting and campaign sending. |

## Descriptor decisions

- `sections` is a required multiline string because competitors center the workflow on assembling newsletter content; one-line-per-section keeps the pure Rust model simple and deterministic.
- `subject` and `preheader` are separate strings because inbox preview/title controls are table stakes for email builders.
- `width` is a bounded integer slider (320–900, default 600) because 600px is the standard desktop email width while mobile clients need a responsive fallback.
- `background`, `content_background`, `text_color` and `accent` are colour inputs to cover common visual customization without a full visual editor.
- `font` is an enum of email-safe font stacks; web fonts are deliberately excluded because support is inconsistent across major clients.
- `dark_mode` is a checkbox because modern builders increasingly expose dark-mode support, but the feature remains optional due to uneven client behavior.

## Verification matrix to cover

- Basic section rendering with a bulletproof button and exact HTML fragments in CLI/page output.
- Query-param/deep-link page run with non-default colours, width, font and `dark_mode=false`.
- Enum coverage for every font stack through the generated descriptor/manifest and at least one real non-default font run.
- Non-default checkbox coverage with dark mode disabled.
- Accepted colour forms: short hex (`#f00`), long hex (`#ff0055`) and named colours.
- Boundary coverage: width 320/900 accepted, width outside that range rejected; 200 sections accepted and 201 rejected.

## Deliberately not built

- No drag-and-drop editor, WYSIWYG preview, hosted image uploads, asset library, send/test-email workflow, ESP integrations or campaign analytics; this pure block returns deterministic HTML only.
- No arbitrary CSS inliner/parser; the tool generates known-safe inline styles from sections rather than transforming user-supplied templates.
- No full email-client rendering simulator. The output includes Outlook/MSO and mobile safeguards, but users should still send real test emails before production campaigns.

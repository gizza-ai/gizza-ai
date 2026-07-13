# contact-info-extractor — competitor analysis (2026-07-13)

Tool: pull **email addresses and phone numbers** out of unstructured text, deduplicated, with
counts. Browser-local wasm, no upload. Built new on branch `feat/tool-loop-20260713-010913`.

## Competitors surveyed (5)

1. **extractemailaddress.com** — extracts emails, phone numbers AND URLs from pasted text.
   Options: sort (alphabetical / company / family name / original order / reverse), output
   presets (Excel-friendly / address-bar friendly / database-friendly / custom separator),
   exclude-containing filter, "group by" counts. Per-type copy/save buttons.
2. **miniwebtool.com/phone-number-extractor** — pasted text → phones. Toggles: "include
   international prefixes (+1/+44…)", "remove duplicates". Detects 8 format categories
   (international, US-parens, dashed, dotted, spaced, continuous, 7-digit, mixed) across 15+
   formats (US/CA/UK/JP/CN/IN/FR/DE). Stats: total / unique / duplicate counts + format
   breakdown + pie chart. Copy-all, download CSV, download TXT, example-text buttons.
3. **phrasefix.com/tools/extract-phone-numbers** — pasted text → list of phones, one per line.
   Copy + upload-file + clean/reset + download. No dedupe/sort/count advertised.
4. **textconverter.com/extract-phone** — phones, TXT output. Sort ascending/descending.
   Upload / clean / download / copy.
5. **Apify / Outscraper "contact info scraper"** — crawl a live website/social profile and pull
   emails + phones in bulk; export Excel/CSV/JSON. Backend crawler + paid tiers.

## Table-stakes → decision (every one lands in the descriptor or the out-of-model list)

| Capability | Seen in | Fit | Decision |
|---|---|---|---|
| Extract emails **and** phones together | 1, 5 | in-model | `types` = both/emails/phones (default both) |
| Deduplicate | 1, 2 | in-model | `dedupe` bool, default true (emails case-insensitive, phones by normalized digits) |
| Total / unique counts | 1, 2 | in-model | output returns `email_count`, `phone_count`, `total`; page shows a summary line |
| Sort (alphabetical / original) | 1, 2, 4 | in-model | `sort` = first-seen / alphabetical (default first-seen) |
| International prefix + multi-format phone detection | 2 | in-model | regex handles `+CC`, `(area)`, dashed/dotted/spaced/continuous US + intl forms |
| Example/preset buttons | 2 | in-model | `[[example]]` preset chips on the page |
| Copy result / download | 1, 2, 3, 4 | platform | shared Copy button + `format="text"` Download link (generator-provided) |
| Custom output separators / Excel-friendly formats | 1 | in-model but rejected | schema/UX bloat for a paste tool; CSV-shaped text is copy-ready. **Considered, rejected** |
| Format-category breakdown + pie chart | 2 | in-model but rejected | viz sugar, not core; counts convey the same. **Considered, rejected** |
| Live website / social-profile scraping | 5 | out-of-model | needs a backend crawler + network; gizza is browser-local on pasted text |
| SMTP email *verification* ("verified emails") | 5 | out-of-model | needs live SMTP/network; `email-validator` covers syntax only |
| Export to Excel / PDF / DOCX | 1, 5 | out-of-model | binary export backend; browser gets copy-paste + TXT/CSV-shaped text |

## Existing-block dedup check

- `extract-email-addresses` — emails only, no phones.
- `phone-format` — parses/formats a **single** phone number; does NOT scan text for phones.
- `ioc-extract` — IPs/URLs/domains/emails/hashes from text, but **no phone numbers**.

No existing block extracts phone numbers from unstructured text. `contact-info-extractor`'s
distinct capability (phone extraction + combined contact-info output) is genuinely new; the email
half overlaps with `extract-email-addresses` but the combined "contact info" tool is a coherent,
competitor-validated category. **Viable — built.**

## Original copy note

All page copy, examples, and FAQ are original. No competitor copy, branding, or trademarks are
reproduced; out-of-model features are listed here, not built.

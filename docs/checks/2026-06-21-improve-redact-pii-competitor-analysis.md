# redact-pii — competitor analysis & improvements (2026-06-21)

**Tool:** `gizza-ai/redact-pii` — detect and mask PII (emails, phone numbers,
IPv4/IPv6 addresses, credit-card numbers, US SSN-like numbers) in text. Pure-Rust
(`regex`). Pure-text input → text output, so chat + CLI + a page.

## What competitors do

- **Online "PII redactor / text anonymizer" sites** — paste text, get a scrubbed
  version. Strength: instant. **Weakness: you paste the sensitive text into a
  third-party page** — exactly the data you're trying to protect.
- **Microsoft Presidio, AWS Comprehend / Macie, Google DLP** — powerful,
  ML/NER-based (catch names, locations), but require a cloud account or a
  Python/ML stack and send data to a service.
- **Hand-rolled regex scripts / `sed`** — local but ad-hoc, error-prone, and rarely
  Luhn-check card numbers (so they over-redact ordinary long digit strings).

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust (`regex`) compiled to wasm:
   runs in the chat Service Worker, the CLI, and in-browser on the page. The text
   never leaves the device — the right default for a privacy tool.
2. **Luhn-validated card detection.** Credit-card matches are confirmed with the
   Luhn checksum, so 16-digit order numbers / IDs aren't falsely redacted — a
   common failure of naive regex redactors.
3. **Non-overlapping, multi-category pass.** Email, phone, IPv4, IPv6, SSN and
   card patterns are matched together and de-overlapped (earliest/longest wins),
   so a value is labelled once with the right type rather than double-masked.
4. **Two output styles.** `label` inserts typed tokens (`[EMAIL]`, `[PHONE]`,
   `[IP]`, `[CREDIT_CARD]`, `[SSN]`) so reviewers can see *what* was removed;
   `mask` blanks each character with `*`.
5. **Counts for auditing.** Chat/CLI return per-category counts of what was
   redacted, so you can verify the scrub. Addressable identically via chat, CLI,
   and a `?text=…&style=…` page.

## Honest scope

- **Pattern-based, not ML.** It catches structured identifiers (emails, phones,
  IPs, cards, SSNs) — **not** names, postal addresses, or unusual formats. The
  page copy says so explicitly; review output for high-stakes use.
- **Phone/SSN patterns are US-leaning** (optional country code, NANP-style); exotic
  international formats may be missed.

## Tests

7 core unit tests: redacts an email (label) and the `mask` style blanks it
char-for-char; SSN + IPv4 in one string; a **Luhn-valid card is redacted while a
same-length Luhn-invalid number is not**; a US phone number; a full IPv6 address;
and clean text passes through unchanged with `total == 0`. Plus the block
drift-guard schema test. **CLI verified** end-to-end on a mixed string (all five
categories). **Page** verified with Playwright (a multiline text field +
style field → `[EMAIL]` in the output). `wafer build` instantiates the chat block.

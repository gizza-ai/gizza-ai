## About this tool

**Redact PII** scans a block of text and replaces personally-identifiable
information with safe placeholders, so you can share logs, transcripts, or support
tickets without leaking personal data.

It detects:

- **Email addresses**
- **Phone numbers** (US-style, with optional country code / area-code parens)
- **IP addresses** — IPv4 and IPv6
- **Credit-card numbers** — 13–19 digit runs, **Luhn-validated** so ordinary long
  numbers aren't falsely redacted
- **US SSN-like numbers** (`123-45-6789`)

Choose a **style**:

- `label` (default) — replace each value with a typed token like `[EMAIL]`,
  `[PHONE]`, `[IP]`, `[CREDIT_CARD]`, `[SSN]` (keeps the text readable and shows
  *what* was removed).
- `mask` — replace every character of the value with `*`.

### Privacy

Everything runs **in your browser** via WebAssembly — the text is never uploaded
to a server. You can also run it from the [gizza CLI](/) or inside a gizza chat,
which return per-category counts of what was redacted.

### Notes

Detection is pattern-based and intended for quick scrubbing; it is **not a
guarantee** that every piece of sensitive data is caught (names, addresses, and
unusual formats aren't matched). Always review the output for high-stakes use.

## FAQ

<details>
<summary>Why wasn't my 16-digit number redacted as a credit card?</summary>

Card detection is Luhn-validated: a run of 13–19 digits (spaces and dashes
between digits are allowed) is only redacted if it passes the Luhn checksum
real card numbers use. A random order ID or tracking number that happens to be
16 digits will usually fail the check and is deliberately left alone.

</details>

<details>
<summary>Does it remove names, street addresses, or dates of birth?</summary>

No. Matching is regex-based and covers emails, US-style phone numbers,
IPv4/IPv6 addresses, Luhn-valid card numbers, and `123-45-6789`-style SSNs.
Free-text PII like names and postal addresses needs context-aware entity
recognition, which this tool doesn't do — proofread the output before sharing
anything sensitive.

</details>

<details>
<summary>When should I pick "mask" instead of the default "label" style?</summary>

`label` swaps each hit for a typed token (`[EMAIL]`, `[PHONE]`, `[IP]`,
`[CREDIT_CARD]`, `[SSN]`), which keeps the text readable and tells reviewers
what kind of data was removed. `mask` overwrites every character of the match
with `*`, preserving the original length — useful when downstream parsing
expects the field to still be there.

</details>

<details>
<summary>Which phone-number formats does it recognise?</summary>

US-style 10-digit numbers with an optional `+` country code (1–3 digits),
parenthesised area codes, and space/dot/dash separators — e.g.
`(555) 123-4567`, `555.123.4567`, `+1 555 123 4567`. Short local numbers and
most non-US formats fall outside the pattern and won't be caught.

</details>

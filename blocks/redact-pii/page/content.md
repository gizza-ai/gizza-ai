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

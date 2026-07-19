# vcard-normalize — competitor analysis (2026-07-17)

Goal: a pure, in-browser tool that takes raw vCard/.vcf text (one or many
`BEGIN:VCARD … END:VCARD` blocks) and returns **normalized vCard text** — tidy
emails, phone numbers reformatted toward E.164, and consistent name casing —
while preserving every unknown property and the card structure. No upload, no
account.

## Landscape

| Competitor | What it does | Gaps we beat |
| --- | --- | --- |
| **vCard / vcf editors (e.g. vcardmaker, various "vcf editor online")** | GUI form editors that rebuild a card field-by-field. | They rewrite the whole card and *drop* properties they don't understand (X-*, PHOTO, custom TYPE params). We preserve unknown lines verbatim. |
| **Contact-manager imports (Google Contacts, iCloud)** | Import .vcf, silently canonicalize on their own servers. | Server-side, account-gated, opaque, and one-way (you can't get the normalized text back out cleanly). Ours is local and returns text. |
| **`vobject` / `vCard` Python & JS libraries** | Programmatic parse/serialize. | Require code + a toolchain. No zero-setup "paste text → get text" surface, and most re-serialize (reordering params, changing escaping) rather than doing a minimal in-place normalization. |
| **Phone-only formatters (libphonenumber demos)** | Format one phone number at a time. | Single-value, not vCard-aware. Ours walks every `TEL` line in a whole file. |
| **Generic "clean up my contacts" SaaS** | Dedupe + normalize behind a login. | Paid, uploads your address book. Ours is offline and does one clear job. |

## What "good" normalization means here (our scope)

1. **EMAIL** — trim surrounding whitespace and lowercase the address (DNS is
   case-insensitive and every mainstream provider treats the mailbox
   case-insensitively). Toggle with `lowercase_email`.
2. **TEL** — reformat to **E.164** (`+15551234567`) using the pure-Rust
   `phonenumber` crate (bundles libphonenumber metadata). A `default_country`
   (ISO-3166 alpha-2, e.g. `US`) interprets numbers written without a `+`
   prefix. **Conservative:** a value is only rewritten when it parses *and* is a
   valid number for its region — anything else is left byte-for-byte untouched,
   so we never mangle short codes, malformed numbers, or already-odd values. Any
   parsed extension is preserved as `;ext=<digits>`.
3. **Name fields** (`FN`, `N`, `NICKNAME`) — collapse repeated internal
   whitespace and trim; optionally recase (`keep` / `title` / `upper` / `lower`)
   per structured component, so `N`'s `Family;Given;…` layout and `NICKNAME`'s
   comma list survive. Default `keep` changes only spacing.
4. **Everything else preserved** — VERSION, ORG, ADR, PHOTO, X-* and any TYPE
   params are emitted verbatim. Line folding is unfolded first (RFC 6350 §3.2)
   and the document's line-ending style (CRLF vs LF) is detected and preserved.

## Documented limits (honesty beats over-promising)

- **Phone parsing** is metadata-driven but conservative: numbers that don't
  validate for `default_country` (or lack a `+` and have no `default_country`)
  are left unchanged rather than guessed at. This is deliberate — a wrong E.164
  is worse than an untouched one.
- **Title-case** is naive (first letter of each whitespace-delimited word); it
  will render `McDonald` as `Mcdonald`, so the default is `keep`.
- Output uses one canonical folded-less line per property; we do not re-fold long
  lines. Values keep their original vCard escaping (`\,` `\;` `\n`).

## Decision

Ship the four-surface tool (chat skill / CLI / web page / tests) with params
`data`, `default_country`, `name_case`, `lowercase_email`. Reuse the sibling
vCard parsing approach (`vcard-to-json`, `vcard-deduplicate`) and the
`phonenumber` crate already used by `phone-format`.

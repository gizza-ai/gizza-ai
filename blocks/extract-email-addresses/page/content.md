## About this tool

**Extract Email Addresses** scans a block of text and pulls out every email
address it finds — **deduplicated** (case-insensitively) and listed in the order
they first appear. Tick **Group by domain** to also see the addresses bucketed by
their domain, which is handy for spotting which organizations are represented.

Everything runs **locally in your browser** via WebAssembly — your text is never
uploaded, so it's safe for pasting contact lists, email threads, or exports.

### Handy for

- Building a recipient list from a pasted email thread or document.
- De-duplicating a messy list of addresses.
- Seeing, at a glance, how many addresses belong to each domain.

### Notes

- Matching is pragmatic (local-part `@` domain with a real TLD); it won't invent
  addresses from stray `@` symbols.
- Addresses are lowercased for de-duplication.

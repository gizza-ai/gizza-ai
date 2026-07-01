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

## FAQ

<details>
<summary>Are User@Example.com and user@example.com counted as two addresses?</summary>

No. Every match is lowercased before de-duplication, so they collapse into a
single entry (`user@example.com`). The output list keeps the order in which each
unique address first appeared in your text.

</details>

<details>
<summary>Does it handle plus-addressing and subdomains?</summary>

Yes. Addresses like `user+tag@mail.example.co.uk` are matched in full — the
local part may contain letters, digits and `. _ % + -`, and the domain can have
any number of subdomain labels as long as it ends in a real TLD of two or more
letters.

</details>

<details>
<summary>What won't be picked up as an email address?</summary>

Fragments without both sides of the `@` — things like `@handle`, `foo@`, or a
bare `@bar.com` — are ignored, as is anything whose domain lacks a valid TLD.
If nothing in the text qualifies, the tool reports "No email addresses found."

</details>

<details>
<summary>How are results ordered when I group by domain?</summary>

With **Group by domain** on, domains are listed alphabetically with a per-domain
count, and each domain's addresses keep their first-seen order. Without
grouping, you get one flat list in first-seen order.

</details>

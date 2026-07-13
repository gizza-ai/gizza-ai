# vcard-deduplicate — competitor analysis (2026-07-13)

Scope: clean up a `.vcf` export by finding duplicate contacts and either merging each duplicate group into one vCard or removing the extra copies.

## Competitors scanned (paraphrased)

1. **vCard Duplicate Remover / online VCF cleaners** — paste or upload a `.vcf`, detect repeated contacts, download a cleaned file. Common controls: pick matching criteria and choose merge vs delete.
2. **ContactsMate / address-book duplicate managers** — desktop tools that group contacts by name, email, or phone and merge details across cards.
3. **Google/Apple contact duplicate flows** — present possible duplicates, then merge names, emails, and phone numbers into a survivor.
4. **VCF parsing libraries / RFC references** — vCard 3.0/4.0 line folding, `FN`, structured `N`, `EMAIL`, and `TEL` fields are the portable keys.
5. **Deduplication scripts** — usually normalize email case and phone punctuation before comparing.

## Table-stakes → decisions

| Capability | Decision |
| --- | --- |
| Multiple vCards in one file | **in-model** — parses repeated `BEGIN:VCARD` / `END:VCARD` blocks. |
| vCard 2.1/3.0/4.0 basics | **in-model** — preserves original property lines and handles folded continuations. |
| Match by name/email/phone | **in-model** — `match_by` enum: `any`, `name`, `email`, `phone`. |
| Phone normalization | **in-model** — compares digits only, so punctuation differences collapse. |
| Email normalization | **in-model** — lowercases email values for matching/deduping. |
| Merge vs remove copies | **in-model** — `merge=true` unions details; `merge=false` keeps the first card only. |
| Preserve contact data | **in-model** — repeatable properties are unioned; singular fields keep the survivor's value and can fill blanks from later cards. |
| Local/private processing | **in-model** — pure Rust + WASM page; no upload. |
| Manual review UI for every suggested pair | **out-of-model** — requires an interactive stateful editor; this tool is deterministic batch cleanup. |
| Country-code-aware phone equivalence | **out-of-model** — comparing national numbers without a region is risky; documented as a limitation. |
| Photo/avatar binary merge | **out-of-model** — large binary embedded fields and conflict resolution need a richer contact editor. |

## UX choices shipped

- Select labels for matching criteria (`any`, `name`, `email`, `phone`).
- Checkbox for merge vs delete behavior, defaulting to safe merge.
- Preset chips for common duplicate shapes: same name, same email, and remove-without-merge.
- Page docs explain first-card survivor semantics, folded lines, and privacy.

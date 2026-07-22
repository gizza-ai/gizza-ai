## Clean pasted email lists before importing or sending

Use this tool when you have addresses copied from a spreadsheet, CRM export, mail header, or sign-up form and need a safer list to paste somewhere else. It splits entries on newlines, commas, and semicolons; trims whitespace; accepts `Name <addr>` and `mailto:` wrappers; lowercases the cleaned address; validates email syntax; removes duplicates; and reports malformed rows with the reason they were rejected.

The default report keeps first-seen order and shows counts for processed entries, unique valid addresses, duplicates removed, and invalid rows. Switch the output format to `clean` for one address per line or `comma` for a comma-separated list. Enable provider alias folding only when you intentionally want Gmail-style variants such as `john.doe+tag@gmail.com` and `johndoe@gmail.com` to collapse to the same canonical address.

### Worked examples

- `Alice@example.com`, `Bob <bob@example.com>`, `alice@example.com`, `not-an-email` returns two valid unique addresses, one duplicate removed, and one invalid row.
- `zeta@example.com, alpha@example.com; zeta@example.com` with `sort=alpha` and `format=clean` returns `alpha@example.com` then `zeta@example.com`.
- `john.doe+news@gmail.com`, `johndoe@gmail.com`, and `JohnDoe@googlemail.com` with provider alias folding enabled collapse to `johndoe@gmail.com`.

### Limits and edge cases

- This is a syntax and list-cleaning tool. It does not perform DNS, MX, SMTP, mailbox, or bounce verification.
- Disposable-domain checks are handled by the separate disposable-email detector tool.
- The parser accepts practical email syntax used by real mail systems; exotic quoted local parts may be accepted by the validator but not provider-canonicalized.
- Whitespace alone is not a separator, so display names like `Ada Lovelace <ada@example.com>` stay together.
- Provider alias folding can intentionally merge distinct aliases; leave it off if tags or dots matter for your workflow.

### FAQ

<details>
<summary>Does this verify that a mailbox really exists?</summary>

No. It validates address syntax and cleans the pasted list locally. Real mailbox verification requires DNS and SMTP/network checks, which are outside this browser-local tool model.

</details>

<details>
<summary>What does provider alias folding do?</summary>

When enabled, known provider rules are applied before de-duplication. For example, Gmail ignores dots in the local part and supports `+tag` aliases, so those variants collapse to one canonical address.

</details>

<details>
<summary>Will duplicates keep the first address I pasted?</summary>

Yes by default. The tool preserves first-seen order after cleaning and removes later duplicates. Choose alphabetical sort only when you want the final valid list sorted.

</details>

<details>
<summary>Can I paste comma-separated addresses from a spreadsheet or To field?</summary>

Yes. Newlines, commas, and semicolons all split entries, while spaces inside display-name wrappers are preserved.

</details>

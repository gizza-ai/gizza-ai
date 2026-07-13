## Extract emails and phone numbers from pasted text

Paste an email thread, signature block, exported notes, CRM snippet, or contact list and this tool returns the contact details it can recognize. It is deterministic regex extraction — no LLM guessing — and it runs locally in your browser.

### What it finds

- Email addresses with common local parts, plus tags, subdomains, and real TLDs.
- Phone numbers written as international numbers (`+44 20 7946 0958`), area-code forms (`(415) 555-2671`), dashed/dotted/spaced groups, or continuous 10–15 digit runs.
- Optional filtering to extract **emails only**, **phones only**, or **both**.
- Optional deduplication: emails by lowercase address, phones by normalized digits.

### Worked example

Input:

`Reach Alice at alice@corp.com or call +1 415 555 2671. Bob: bob@corp.com, (212) 555-0199.`

Output:

- 2 emails: `alice@corp.com`, `bob@corp.com`
- 2 phones: `+1 415 555 2671`, `(212) 555-0199`

### Limits and edge cases

- Phone detection is pragmatic, not a country-aware validator. Use phone-format for country-specific formatting/validation.
- Short numeric codes, years, and long ID numbers are ignored where possible.
- The tool does not infer missing area codes, contact names, companies, or labels.
- Very messy OCR may need cleanup before extraction.

<details>
<summary>Does this validate that an email inbox or phone number is real?</summary>

No. It extracts values that look like email addresses or phone numbers. It does not check MX records, send test messages, call phone-number APIs, or verify deliverability.

</details>

<details>
<summary>How are duplicates handled?</summary>

With dedupe on, emails are compared case-insensitively and returned lowercased. Phone numbers are compared by their digits, so `555-123-4567` and `(555) 123 4567` count as the same number.

</details>

<details>
<summary>Can I keep every occurrence instead?</summary>

Yes. Turn off dedupe to keep duplicate matches in the order they appear. This is useful when you want to count repeated mentions rather than build a clean contact list.

</details>

<details>
<summary>Why did it miss or include a phone-like number?</summary>

Phone numbers vary by country and context. This tool uses safe general patterns with digit-count filters; it may miss unusual local formats or include a phone-like business identifier. For country-specific validation, run the extracted values through a dedicated phone formatter/validator.

</details>

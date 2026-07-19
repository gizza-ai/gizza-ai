## About this tool

vCard Normalize cleans a pasted `.vcf` address book while keeping the original
vCard structure. It is useful before importing contacts into a CRM, phone, or
mailing-list tool: normalize the fields people search and dedupe on, without
uploading the address book to a contact-cleanup service.

The transform is intentionally conservative:

- **EMAIL** values are trimmed and lowercased by default.
- **TEL** values are reformatted to E.164 only when they parse and validate. Use
  **Default country** (`US`, `GB`, `DE`, …) for national numbers without a `+`.
- **FN**, **N**, and **NICKNAME** have repeated whitespace collapsed; optional
  name casing can keep the existing case, title-case it, uppercase it, or
  lowercase it.
- Unknown properties such as `ORG`, `ADR`, `PHOTO`, `X-*`, and custom `TYPE`
  parameters are preserved rather than rebuilt from a lossy form model.

Folded vCard lines are unfolded before processing, and the input's LF/CRLF line
ending style is preserved. Invalid phone numbers are left untouched instead of
being guessed into a wrong international number.

### Worked example

Input (with `default_country=US`, `name_case=title`, `lowercase_email=true`):

```
BEGIN:VCARD
VERSION:3.0
FN: jane   DOE
N:DOE;jane;;;
TEL;TYPE=CELL:(415) 555-2671
EMAIL: Jane.DOE@Example.COM 
END:VCARD
```

Output:

```
BEGIN:VCARD
VERSION:3.0
FN:Jane Doe
N:Doe;Jane;;;
TEL;TYPE=CELL:+14155552671
EMAIL:jane.doe@example.com
END:VCARD
```

## FAQ

<details>
<summary>Why does phone normalization need a default country?</summary>

Numbers such as `(415) 555-2671` are national-format numbers; the same digits
can mean different things in different countries. The country hint tells the
phone metadata parser how to interpret those numbers. Already-international
numbers that start with `+` can be normalized without a hint.

</details>

<details>
<summary>Will invalid or short phone numbers be changed?</summary>

No. A phone value is rewritten only when it parses and validates for the chosen
country metadata. Short codes, extensions the parser cannot understand, typos,
and other ambiguous values are left as-is so the tool does not corrupt a contact.

</details>

<details>
<summary>Does this preserve custom vCard fields?</summary>

Yes. The tool only rewrites values for `EMAIL`, `TEL`, `FN`, `N`, and
`NICKNAME`. It preserves unknown properties and parameters such as `ORG`, `ADR`,
`PHOTO`, `X-CUSTOM`, and `TYPE=work` instead of parsing the whole card into a
limited form and serializing it back.

</details>

<details>
<summary>What are the limits of title casing?</summary>

Title casing is a simple per-word casing pass. It handles ordinary names like
`jane DOE` → `Jane Doe`, but it does not know every cultural or family-name
convention; for example `McDonald` may become `Mcdonald`. The default `keep`
mode only tidies spacing and leaves original casing intact.

</details>
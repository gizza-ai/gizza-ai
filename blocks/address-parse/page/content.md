## Parse a freeform address into structured fields

Paste a postal address exactly as you received it — one line, comma-separated, or
multi-line — and this tool extracts the pieces that are useful for forms,
spreadsheets, CRM cleanup, shipping prep, and geocoding pipelines. It returns a
local, rule-based split into house number, street, unit or suite, city, region,
postcode, country, and ISO country code when the country is recognized.

The parser is intentionally deterministic and browser-local. It does not call a
geocoder, validate deliverability, or send the address to a server. That makes it
useful for quick normalization and data cleanup, especially when you want a
repeatable first pass before manual review.

### What it extracts

- **House number** — leading numbers such as `123`, `221B`, and `12-14`; for
  common European hints it also recognizes trailing numbers like `Hauptstraße 5`.
- **Street** — the street line with the house number and unit designator removed.
- **Unit** — apartment, suite, flat, room, `#12`, and similar secondary designators.
- **City/locality** — the remaining locality after postcode and region are stripped.
- **Region** — US states, Canadian provinces/territories, and Australian states
  resolve to names plus codes such as `IL`, `ON`, or `NSW`.
- **Postcode** — US ZIP and ZIP+4, UK, Canadian, Dutch, and numeric postcodes for
  several other countries.
- **Country** — recognized country names and common aliases become canonical names
  plus ISO 3166-1 alpha-2 codes.

### Worked examples

Input:

```text
123 Main St Apt 4B, Springfield, IL 62704, USA
```

Output includes `House number: 123`, `Street: Main St`, `Unit: Apt 4B`,
`City: Springfield`, `Region: Illinois (IL)`, `Postcode: 62704`, and
`Country: United States (US)`.

If the country is omitted, use **Country hint** to bias postcode and region
parsing. For example, `350 Fifth Avenue, New York, NY 10118` with `US` fills the
country as United States and resolves `NY` as New York.

### Limits and edge cases

This is a heuristic parser, not a postal authority or geocoder. It works best on
common address orders and on addresses that include separators (commas or line
breaks). It may be imperfect for rural route formats, building-campus addresses,
non-Latin scripts, addresses where city and street are in an unusual order, or
country-specific elements such as prefecture/district hierarchy that do not map
cleanly to the simple street/city/region/postcode model. Always review results
before mailing, billing, deduping, or compliance workflows.

## FAQ

<details>
<summary>Does this validate that an address is deliverable?</summary>

No. It only parses the text into likely fields. It does not call USPS, Royal Mail,
Canada Post, a geocoder, or any other delivery database, so it cannot confirm that
an address exists or is deliverable. Use it as a cleanup step before validation.

</details>

<details>
<summary>What does the country hint do?</summary>

The hint biases postcode and region detection and fills the country when the text
omits it. For example, `ON` is treated as Ontario when the hint or detected
country is Canada, while `IL` resolves as Illinois for the United States. Choose
`auto` when the country is written in the address itself.

</details>

<details>
<summary>Can it parse multi-line addresses?</summary>

Yes. Line breaks are treated like commas, so a pasted mailing label such as
`10 Downing Street\nLondon\nSW1A 2AA\nUK` is parsed into street, city, postcode,
and country fields. Blank lines and extra whitespace are ignored.

</details>

<details>
<summary>Why did a field end up blank or in the wrong place?</summary>

Postal formats vary widely. The parser uses deterministic rules and conservative
country tables; it avoids inventing fields when the text is ambiguous. Add commas
or line breaks between major parts, provide a country hint, or review the result
manually for unusual formats.

</details>

<details>
<summary>Is my address uploaded?</summary>

No. The parser runs as WebAssembly in your browser tab. There is no network call,
server-side parsing, account, or logging by this tool.

</details>

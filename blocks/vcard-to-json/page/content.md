## Convert vCard contacts to JSON

Paste one or more vCard (`.vcf`) contact cards and get back a clean **JSON array**
of contacts. The parser understands vCard **2.1, 3.0 (RFC 2426), and 4.0
(RFC 6350)** — including folded continuation lines, multiple cards in one file,
group prefixes like `item1.TEL`, and value escapes (`\n`, `\,`, `\;`, `\\`).
Everything runs locally in your browser; your contacts are never uploaded.

### Worked example

Input:

```
BEGIN:VCARD
VERSION:3.0
FN:John Doe
N:Doe;John;;Mr.;
EMAIL;TYPE=work:john@example.com
TEL;TYPE=CELL:+15551234567
END:VCARD
```

Output (pretty-printed, structured):

```json
[
  {
    "version": "3.0",
    "fn": "John Doe",
    "name": { "family": "Doe", "given": "John", "middle": "", "prefix": "Mr.", "suffix": "" },
    "emails": [ { "value": "john@example.com", "types": ["work"] } ],
    "phones": [ { "value": "+15551234567", "types": ["cell"] } ]
  }
]
```

Repeatable properties (`EMAIL`, `TEL`, `URL`, `ADR`) collect into arrays, and each
one keeps its `TYPE` tags (lower-cased) in a `types` array — so a phone marked
`TYPE=CELL,voice` or the vCard 2.1 bare form `TEL;WORK;VOICE:` both come through as
`"types": ["cell", "voice"]`.

### Options

- **Split Name and Address into fields** (`structured`, default on) — the `N`
  property becomes `{ family, given, middle, prefix, suffix }` and each `ADR`
  becomes `{ poBox, extended, street, locality, region, postalCode, country }`.
  Turn it off to keep `N`/`ADR` as their raw semicolon-joined strings.
- **Output shape** (`wrap`) — `array` returns a bare JSON array of contact
  objects; `object` returns `{ "contacts": [...], "count": N }` so you have the
  total without counting.
- **Pretty-print** (`pretty`, default off) — indent the JSON for reading; leave it
  off for compact single-line output.

### Limits & edge cases

- Known properties (`FN`, `N`, `NICKNAME`, `ORG`, `TITLE`, `ROLE`, `BDAY`, `NOTE`,
  `EMAIL`, `TEL`, `URL`, `ADR`) get dedicated keys; **any other property** is
  preserved under an `other` array as `{ property, value, types }`, so nothing is
  silently dropped.
- Singular properties keep the **first** occurrence if a card repeats them.
- Input must contain at least one `BEGIN:VCARD … END:VCARD` block; text with no
  card returns an error rather than an empty result.
- `BDAY` and other dates are returned as their original string — no reformatting.
- Everything is held in memory in your browser, so very large exports are bounded
  by available RAM rather than a fixed contact cap.

## FAQ

<details>
<summary>Which vCard versions does it support?</summary>

vCard **2.1**, **3.0** (RFC 2426), and **4.0** (RFC 6350). The parser reads the
`VERSION` line into each contact's `version` field but does not require it — it
also handles the 2.1 "bare type" form (`TEL;WORK;VOICE:`) where types appear
without a `TYPE=` prefix.

</details>

<details>
<summary>How are names and addresses broken down?</summary>

With **Split Name and Address into fields** on (the default), the structured `N`
value maps to `family`, `given`, `middle`, `prefix`, and `suffix`, and each `ADR`
maps to `poBox`, `extended`, `street`, `locality`, `region`, `postalCode`, and
`country`. Turn the option off to keep them as the raw `;`-separated strings
exactly as they appear in the file.

</details>

<details>
<summary>Can I convert a file with many contacts at once?</summary>

Yes. Paste as many `BEGIN:VCARD … END:VCARD` blocks as you like — they become one
JSON array in file order. Choose the **object** output shape to also get a
`count` of how many contacts were parsed.

</details>

<details>
<summary>What happens to a property I don't recognise?</summary>

Nothing is lost. Any property outside the known set is collected into an `other`
array as `{ "property": "...", "value": "...", "types": [...] }`, using the
lower-cased property name — so custom `X-` fields and less common properties still
make it into the JSON.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. Parsing runs entirely in your browser with WebAssembly — the `.vcf` text never
leaves your device.

</details>

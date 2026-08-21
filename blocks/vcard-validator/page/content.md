## About the vCard validator

This tool **checks a vCard (`.vcf`) file against the specification** and reports
every problem it finds — it never rewrites your file. Paste the card text into
the **vCard / .vcf text** box and it unfolds continuation lines, splits the
document into individual cards, and reports each issue with a **card number**, a
**line number**, a **severity** (error or warning) and a stable **rule name**.
Everything runs locally in your browser; nothing is uploaded.

It understands **vCard 2.1**, **3.0** (RFC 2426) and **4.0** (RFC 6350). By
default each card is checked against its own `VERSION`; pick a specific version
under **Check against** to validate a whole file against one spec and flag any
card that declares something else.

### What it checks

- **Structure** — `BEGIN:VCARD` / `END:VCARD` pairing, content lines outside any
  card, a continuation (folded) line with nothing to continue, lines longer than
  the 75-octet folding limit, and LF-only line endings where the spec requires
  CRLF.
- **VERSION** — present, a known version, and (in 4.0) the first property after
  `BEGIN:VCARD`. If you pick an explicit version, any card declaring a different
  one is flagged.
- **Required properties** — `FN` in 3.0/4.0, `N` in 2.1/3.0. Add your own with
  **Also require these properties** (for example `UID` for a CardDAV-style
  profile, or `ORG,TITLE` for a company directory).
- **Phone numbers** — every `TEL` value is checked as a real phone number, not
  just as digits. A `tel:` URI prefix is stripped first. Numbers written in
  national format need a **Default country** (`US`, `GB`, `DE`, …); without one
  they are reported as *unverifiable* rather than wrong.
- **Email addresses** — one `@`, a non-empty local part, and a dotted domain
  whose labels are valid.
- **Dates and timestamps** — `BDAY` and `ANNIVERSARY` (including vCard 4.0
  partial dates like `--0415` and `1996`), and `REV` timestamps.
- **Structured values** — `N` must have exactly 5 semicolon-separated
  components, `ADR` exactly 7. Escaped `\;` inside a component is handled
  correctly.
- **Line and parameter syntax** — a missing `:`, an empty or invalid property
  name, a bad `group.` prefix, empty parameters, bare (value-only) parameters
  such as `TEL;WORK:` — legal in 2.1, an error in 3.0/4.0 — unquoted parameter
  values containing `:`, and a `CHARSET` parameter outside 2.1.
- **Enumerations and URIs** — `KIND` and `GENDER` values in 4.0, and absolute
  URIs for `URL`, `SOURCE`, `FBURL`, `CALURI` and `CALADRURI`.
- **Hygiene** — single-instance properties (`VERSION`, `N`, `BDAY`, `UID`, `REV`,
  `KIND`, …) appearing more than once, empty property values, and non-standard
  properties that do not use an `X-` prefix.

Turn off **Check EMAIL address syntax** or **Check TEL phone-number validity**
to silence those rule groups. Choose **Report** for a readable list or **JSON**
for a structured `{ok, cards, error_count, warning_count, versions, issues[]}`
object you can pipe into CI.

### Worked example

Given this card:

```
BEGIN:VCARD
VERSION:3.0
FN:Ada Lovelace
N:Lovelace;Ada
EMAIL:ada@@example.com
TEL;WORK:+44 1632 960 961
BDAY:1815-13-40
URL:example.com
END:VCARD
```

the report opens with `INVALID — 1 card, 4 errors, 2 warnings` and then lists,
in line order: the **bare parameter** `WORK` on the `TEL` line (an error in 3.0 —
write it as `TYPE=WORK`), the **invalid email** `ada@@example.com` (it has more
than one `@`), the **invalid N** on line 4 (only 2 of the required 5 components),
the **invalid date** `1815-13-40` (month 13, day 40), the **non-absolute URI**
`example.com` (a warning — it needs a scheme such as `https://`), plus a
document-level **LF line endings** warning. The phone number `+44 1632 960 961`
passes, because it is a valid UK number in international format.

Switching **Output** to **JSON** returns the same findings as an `issues[]`
array with `"ok": false`, each entry carrying `card`, `line`, `severity`, `rule`,
`property` and `message`.

### Limits & edge cases

- **The file is never rewritten.** This tool only diagnoses. To actually fix
  phone formatting, email casing and name whitespace, use a vCard normalizer.
- **A document with no `BEGIN:VCARD … END:VCARD` block is an error**, not an
  empty report — you get a message saying what was expected.
- **A card with no `VERSION` is checked as 3.0** in Auto mode (and separately
  flagged for the missing `VERSION`).
- **National-format phone numbers cannot be checked without a country.** They
  are reported as `unverifiable-tel` (a warning), never as invalid.
- **Character encoding is not checked.** The text arrives already decoded as
  UTF-8, so byte-level charset problems and quoted-printable bodies are outside
  what this tool can see; it only flags `CHARSET` as a 2.1-era parameter.
- **Base64 blobs in 2.1/3.0 `PHOTO`/`LOGO`/`KEY` are not decoded** — the URI
  rule only applies to 4.0 values without an `ENCODING=` parameter.
- **Severities:** structural breakage, missing required properties, malformed
  values (email, phone, date, `N`/`ADR` arity, `KIND`/`GENDER`) and duplicate
  single-instance properties are **errors**. Interoperability nits — over-long
  lines, LF endings, unknown properties, empty values, non-URI `TEL` in 4.0,
  unquoted parameters, a stray `CHARSET` — are **warnings**. In JSON output,
  `ok` is `true` only when there are zero errors.

## FAQ

<details>
<summary>Does this upload or store my contacts?</summary>

No. The validator is compiled to WebAssembly and runs **entirely in your
browser** — your vCard text never leaves the page and nothing is sent to a
server. You can safely paste a real address book; the tool only reads it to
report issues, and it never rewrites or transmits anything.

</details>

<details>
<summary>Why is my phone number reported as unverifiable instead of invalid?</summary>

A number written in **national format** — `(650) 253-0000`, `020 7946 0958` —
is meaningless without knowing which country it belongs to, so the tool will not
guess. Enter the country in **Default country for phone numbers** as an ISO-3166
alpha-2 code (`US`, `GB`, `DE`, …) and the number is then checked properly. A
number written in international format (`+16502530000`) needs no hint. Once a
country is known, a number that cannot exist for it — a bogus `(555) 000-0000`,
say — is reported as a real `invalid-tel` error.

</details>

<details>
<summary>What's the difference between an error and a warning?</summary>

**Errors** are things that make the card invalid or that a parser will get wrong:
broken `BEGIN`/`END` structure, a missing `VERSION`/`FN`/`N`, a line with no
`:`, a malformed email address or phone number, a bad date, `N` or `ADR` with
the wrong number of components, and a single-instance property used twice.
**Warnings** are things that parse but hurt interoperability: lines longer than
75 octets, LF-only line endings, properties not defined by any vCard version,
empty values, a `TEL` that is not a `tel:` URI in 4.0, and unquoted parameter
values. In **JSON** output, `ok` is `true` only when the error count is zero.

</details>

<details>
<summary>Why is `TEL;WORK:` an error on my card?</summary>

The bare, value-only parameter form (`TEL;WORK;VOICE:…`) belongs to **vCard 2.1**
only. vCard 3.0 and 4.0 require named parameters, so the same line must be
written `TEL;TYPE=WORK:…`. If your file really is a 2.1 export, either leave
**Check against** on Auto with a `VERSION:2.1` line in the card, or select
**vCard 2.1** explicitly — the rule then goes quiet.

</details>

<details>
<summary>Can I use this in CI or a script?</summary>

Yes. Set **Output** to **JSON** and you get
`{ok, cards, error_count, warning_count, versions, issues[]}`, where every issue
carries a stable `rule` slug (`missing-fn`, `invalid-tel`, `duplicate-property`,
…) so you can filter or fail a build on specific rules. The same validator is
available from the command line with the same parameter names — the CLI example
above this section is copy-paste runnable.

</details>

<details>
<summary>Which vCard versions are supported?</summary>

**2.1, 3.0 (RFC 2426) and 4.0 (RFC 6350).** Auto mode reads each card's own
`VERSION` property, so a file mixing versions is handled card by card. Choosing
a specific version instead checks everything against that spec and raises a
`version-mismatch` error for any card declaring a different one — useful when
you are migrating an address book and want to know exactly what breaks.

</details>

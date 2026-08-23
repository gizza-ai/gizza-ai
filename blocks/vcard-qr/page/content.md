## About this tool

A vCard QR code is a QR code whose payload is a whole contact card rather than a link. Point a
phone camera at it and the operating system recognises the `BEGIN:VCARD` payload and offers to save
the person — name, job title, company, phone numbers, email, website, postal address, birthday and
note — straight into the address book. No app, no account, no typing.

This generator builds the vCard itself and encodes it in one step. The card is written as a real
RFC 6350 / RFC 2426 vCard: values are escaped (commas, semicolons and backslashes), long lines are
folded at 75 octets with CRLF continuations, and phone numbers and emails carry the `TYPE` labels
phones use to tell a mobile from a landline. You pick vCard 3.0 (the dialect scanners handle best)
or 4.0.

The result is an SVG, so it stays sharp whether it ends up on a slide, a badge or a printed banner.
The exact vCard source is embedded in the SVG's `<desc>` element and returned by the CLI, so you can
copy it into a `.vcf` file if you want the contact card as a file too.

### Worked example

Fill in:

- First name `Ada`, last name `Lovelace`
- Company `Analytical Engines`, job title `Chief Analyst`
- Mobile `+44 7700 900123`, email `ada@example.com`, website `example.com/ada`

The encoded vCard is:

```
BEGIN:VCARD
VERSION:3.0
N:Lovelace;Ada;;;
FN:Ada Lovelace
ORG:Analytical Engines
TITLE:Chief Analyst
TEL;TYPE=CELL:+44 7700 900123
EMAIL;TYPE=INTERNET:ada@example.com
URL:https://example.com/ada
END:VCARD
```

Scanning the generated code on a phone brings up "Add contact" pre-filled with those fields. With
**Print contact details under the code** left on, the same details are also typeset in monospace
under the QR, which is what makes it usable as a conference badge or the back of a business card.

### Notes on the fields

- At least one of first name, last name or company is required — a vCard with no display name
  cannot be saved.
- `example.com/ada` becomes `https://example.com/ada`; a bare host is what people type, but a
  scanner needs a scheme to open it.
- Email addresses are checked for a single `@` and a dotted domain. Birthdays must be real dates in
  `YYYY-MM-DD` form, so `2023-02-29` is rejected rather than silently encoded.
- The five address fields (street, city, state/region, postal code, country) are combined into one
  `ADR` property. Fill in only the ones you have.

### Limits and edge cases

- **Capacity.** A QR code holds roughly 2,900 bytes at error correction L and about 1,270 at H.
  A long note or address plus a high error-correction level can exceed that; the tool then fails
  with the vCard's byte count instead of producing an unscannable code. Shorten the note or drop to
  L or M.
- **Density vs. print size.** More contact detail means more modules, and more modules need a bigger
  printed code. As a rule of thumb, print at least 20 mm square for a short card and more for a
  detailed one, and always leave the light quiet-zone margin that is already part of the SVG.
- **No photo.** A `PHOTO` property carrying an image makes the payload far too large to scan
  reliably, so contact photos are not supported.
- **Static by design.** The contact data lives inside the code. That means it works offline and
  forever, and it also means the code cannot be edited after printing and cannot count scans — if
  the details change, generate a new code.
- **Output format.** This page renders SVG (vector). Colours accept hex values (`#000`, `#000000`,
  `#000000ff`) or CSS colour names, and `transparent` is allowed for the background.
- **Privacy.** The whole thing runs in your browser as WebAssembly. Contact details are never
  uploaded, and there is no redirect service in the middle.

## FAQ

<details>
<summary>Will scanning this actually save the contact on an iPhone or Android phone?</summary>

Yes. Both stock camera apps recognise a `BEGIN:VCARD` payload and offer to create a contact from
it — the same mechanism that handles a Wi-Fi or URL QR code. vCard 3.0 is the safest choice for
scanning because every address book has supported it for years; 4.0 is the newer RFC and is better
suited to cards consumed by software rather than a phone camera.

</details>

<details>
<summary>How do I get a .vcf file, not just the image?</summary>

The exact vCard source is embedded in the generated SVG's `<desc>` element, and the CLI prints it
alongside the image. Copy those lines — from `BEGIN:VCARD` to `END:VCARD` — into a text file, save
it with a `.vcf` extension, and any address book will import it. The worked example above shows
what those lines look like.

</details>

<details>
<summary>My contact is "too long to encode" — what should I change?</summary>

The error names the vCard's size in bytes. The cheapest wins are shortening the note, dropping the
postal address to the fields you actually need, and lowering the error-correction level from H or Q
to M or L. Level L holds roughly twice as much data as H. Long URLs are also a common culprit — link
to a short profile URL rather than a deep one.

</details>

<details>
<summary>Why do I need to pick between mobile and work phone?</summary>

They are stored as different vCard properties — `TEL;TYPE=CELL` and `TEL;TYPE=WORK,VOICE` — so the
phone that scans the code labels them correctly in the saved contact, and knows which one to text.
Fill in either or both; empty fields are simply left out of the card.

</details>

<details>
<summary>Can I add a logo, custom shapes, or track how many people scan it?</summary>

Not here. Logos and styled modules change the symbol's appearance and belong to a dedicated styling
tool; scan tracking requires the code to point at a redirect service that logs every scan, which
would mean the contact data leaves your device. This tool generates a plain static code that carries
the contact itself, works offline, and reports nothing to anyone.

</details>

<details>
<summary>What happens to names with accents, commas or semicolons?</summary>

They are handled. Text is encoded as UTF-8, and the characters that are structural in a vCard —
comma, semicolon and backslash — are escaped so a company like `Smith, Jones & Co` survives intact
instead of splitting into two values. Long values are folded onto continuation lines exactly as the
vCard spec requires, which every address book unfolds on import.

</details>

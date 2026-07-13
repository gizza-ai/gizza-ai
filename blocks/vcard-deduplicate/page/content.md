## Remove and merge duplicate vCard contacts

Exports from phones, mail apps, and CRMs often stack the same person into a `.vcf`
file two or three times — one card with a work email, another with a mobile number,
a third from an old sync. Paste the file here and get back a **clean vCard** with
each contact appearing once. Matching runs across vCard **2.1, 3.0 (RFC 2426), and
4.0 (RFC 6350)**, and everything happens locally in your browser — your contacts are
never uploaded.

### Worked example

Two cards for the same person, each holding a different email and phone:

```
BEGIN:VCARD
VERSION:3.0
FN:John Doe
EMAIL:john@work.com
TEL:+1-555-111-2222
END:VCARD
BEGIN:VCARD
VERSION:3.0
FN:John Doe
EMAIL:john@home.com
TEL:1 (555) 111-2222
END:VCARD
```

With **merge** on, the two collapse into one card that keeps *both* emails while the
repeated phone (same digits, written two ways) is kept only once:

```
BEGIN:VCARD
VERSION:3.0
FN:John Doe
EMAIL:john@work.com
TEL:+1-555-111-2222
EMAIL:john@home.com
END:VCARD
```

(The survivor keeps its own lines in order, then each new value from the other cards
is appended.)

### Options

- **Treat as duplicates when they share** (`match_by`) — pick how aggressively to
  group cards:
  - **Any of name, email, or phone** (`any`, default) — link cards that share *any*
    of those, so a renamed card and a card with a matching phone still merge.
  - **The same full name** (`name`) — match only on the display name (`FN`, or the
    structured `N` if there's no `FN`), compared case- and spacing-insensitively.
  - **The same email address** (`email`) — match only on a shared email
    (case-insensitive), regardless of name.
  - **The same phone number** (`phone`) — match only on a shared phone, compared by
    its digits so `+1 (555) 123-4567` and `5551234567` count as one.
- **Merge each group into one card** (`merge`, default on) — combine every duplicate
  into a single card, unioning emails, phones, URLs, and addresses and filling empty
  singular fields (like `ORG` or `TITLE`) from later copies. Turn it **off** to
  simply keep the first card of each group and drop the rest without merging.

### Limits & edge cases

- The **first** card of each group is the survivor: it keeps all of its own fields,
  and singular fields it already has are never overwritten by a later copy.
- Repeatable properties are de-duplicated by value — two `EMAIL` lines with the same
  address (any case) collapse to one, and two `TEL` lines with the same digits
  collapse to one, even if written with different punctuation.
- Phone matching compares **all** digits, so a number stored with a country code
  (`+1 555…`) will *not* match the same number stored without one (`555…`). Add or
  strip country codes first if you want those to merge.
- Cards with no name, email, or phone (nothing to match on) are always left as
  separate contacts.
- Input must contain at least one `BEGIN:VCARD … END:VCARD` block; text with no card
  returns an error rather than empty output.

## FAQ

<details>
<summary>How does it decide two contacts are the same?</summary>

By the identifiers you choose in **Treat as duplicates when they share**. With the
default **Any**, two cards are grouped if they share a normalized full name, *or* any
email address (case-insensitive), *or* any phone number (compared by digits). Groups
are transitive: if card A shares a phone with B and B shares an email with C, all
three merge into one contact.

</details>

<details>
<summary>What happens to the different phone numbers and emails when merging?</summary>

They're all kept. When a group merges, the survivor (the first card) keeps its own
fields, and every *unique* email, phone, URL, and address from the other cards is
added to it. Duplicate values — the same email in a different case, or the same phone
written with different punctuation — are collapsed to a single entry.

</details>

<details>
<summary>Can I remove duplicates without combining their details?</summary>

Yes. Turn **Merge each group into one card** off. Then for each set of duplicates the
tool keeps only the first card exactly as it was and discards the rest, without
copying any of their extra emails or phones into the survivor.

</details>

<details>
<summary>Which vCard versions does it handle?</summary>

vCard **2.1**, **3.0** (RFC 2426), and **4.0** (RFC 6350), including folded
continuation lines, group prefixes like `item1.TEL`, and multiple cards in one file.
Each surviving card keeps its original property lines and formatting.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The whole thing runs in your browser with WebAssembly — the `.vcf` text never
leaves your device, so it's safe to clean up sensitive address books.

</details>

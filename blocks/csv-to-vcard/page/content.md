## What this tool does

Turn a spreadsheet or exported contact list into a single **.vcf vCard file** you can import into iPhone Contacts, Android Contacts, Google Contacts, Outlook, Apple Contacts, or many CRM tools.

Paste either:

```csv
First Name,Last Name,Email,Mobile Phone,Company
John,Doe,john@ex.com,555-1234,Acme
```

or a JSON array of contact objects. The converter maps common headers to vCard fields automatically:

- name parts (`First Name`, `Last Name`, `Full Name`) → `N` and `FN`
- email columns → `EMAIL`, with home/work type qualifiers when the header says so
- phone, mobile, cell, fax → `TEL`, with `CELL`, `HOME`, `WORK`, or `FAX`
- company, department, job title → `ORG` and `TITLE`
- street, city, state, zip, country → `ADR`
- website, birthday, note, gender → `URL`, `BDAY`, `NOTE`, `GENDER` (gender is vCard 4.0 only)

Everything runs locally in your browser. No contact data is uploaded.

## Worked example

Input:

```csv
First Name,Last Name,Email,Mobile Phone,Company
John,Doe,john@ex.com,555-1234,Acme
```

Output:

```vcf
BEGIN:VCARD
VERSION:3.0
N:Doe;John;;;
FN:John Doe
ORG:Acme
EMAIL:john@ex.com
TEL;TYPE=CELL:555-1234
END:VCARD
```

Save the result as `contacts.vcf`, then import it in your contacts app.

## Options

- **Input format** — `auto` treats input starting with `{` or `[` as JSON and everything else as CSV. You can force `csv` or `json`.
- **CSV delimiter** — `auto` sniffs comma, semicolon, tab, or pipe from the header row. You can force one when the CSV is ambiguous.
- **vCard version** — `3.0` is the safest default for broad import compatibility. `4.0` emits vCard 4.0 syntax and includes `GENDER` when present.

## Limits and edge cases

- The input must have recognizable headers such as `Name`, `First Name`, `Email`, `Phone`, `Company`, or address fields.
- Up to 5,000 contacts are converted in one run.
- Photos and binary attachments are not embedded. This is for text contact fields.
- Long vCard lines are folded at 75 octets for importer compatibility.
- Unknown columns are ignored rather than copied into custom properties.
- The converter escapes commas, semicolons, backslashes, and newlines in vCard values.

## FAQ

<details>
<summary>Can I import the output into Google Contacts or Outlook?</summary>

Yes. Choose the default vCard 3.0 output for the broadest compatibility, save the result as `contacts.vcf`, then use the import feature in Google Contacts, Outlook, Apple Contacts, iPhone, or Android.

</details>

<details>
<summary>What column names does it recognize?</summary>

It recognizes common exports from spreadsheets, CRMs, and address books: `Name`, `First Name`, `Last Name`, `Email`, `Work Email`, `Phone`, `Mobile Phone`, `Company`, `Department`, `Job Title`, `Street`, `City`, `State`, `Zip`, `Country`, `Website`, `Birthday`, `Notes`, and similar variants.

</details>

<details>
<summary>Should I use vCard 3.0 or 4.0?</summary>

Use `3.0` unless you know the importer supports vCard 4.0. Version 3.0 is widely accepted by older contact apps. Version 4.0 is more modern and includes `GENDER`, but some importers are stricter about it.

</details>

<details>
<summary>Does the tool upload my contacts?</summary>

No. The conversion runs in WebAssembly in your browser, so the pasted contacts stay on your device. If you use the CLI, it runs locally there as well.

</details>

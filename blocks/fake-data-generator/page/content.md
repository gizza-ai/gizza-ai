## About this tool

The fake data generator produces rows of realistic-looking but entirely
synthetic test records — perfect for seeding a database, populating a UI mockup,
load-testing an import pipeline, or building a demo without exposing real
people's information. None of the output describes a real person; the names,
emails, and addresses are randomly assembled from placeholder pools.

### What you can generate

Pick any combination of these columns (or choose **all**):

- **id** — a sequential row number; **uuid** — an RFC-4122 v4-style identifier
- **first_name**, **last_name**, **full_name**
- **username** — a lowercase handle derived from the name
- **email** — a synthetic address on a clearly-fake domain (example.com, test.org, …)
- **phone** — a formatted placeholder number
- **street**, **city**, **state**, **zip**, **country** — a postal address
- **latitude**, **longitude** — geographic coordinates
- **company**, **job_title**
- **birthdate** — an ISO `YYYY-MM-DD` date
- **domain**, **ipv4**, **ipv6**, **mac** — network identifiers
- **color** — a `#rrggbb` hex color; **boolean** — true/false
- **credit_card** — a 16-digit, Luhn-valid placeholder number (never a real account)
- **sentence** — a lorem-ipsum sentence

Leave the columns box blank for a sensible default set
(full_name, email, phone, street, city, state, zip).

### CSV, JSON, SQL, or XML

- **csv** — a header row (optional) plus comma-separated data rows, with
  automatic quoting where a value contains a comma or quote.
- **json** — a pretty-printed array of objects you can paste straight into a
  fixture file.
- **sql** — ready-to-run `INSERT INTO …` statements (numeric and boolean columns
  are emitted unquoted); set the table name to suit your schema.
- **xml** — a `<records>` document with one `<record>` per row.

### Reproducible with a seed

Set the **seed** to any non-zero number and you'll get the exact same dataset
every time — handy for deterministic tests and code review. Leave it at `0` to
get fresh data on each run.

Everything runs locally in your browser via WebAssembly — your inputs never
leave your device.

## FAQ

<details>
<summary>How many rows can I generate at once?</summary>

Between 1 and 1000 rows per run (the default is 10). Asking for more than 1000
returns an error rather than a partial dataset — for bigger fixtures, run the tool
a few times with different seeds, or script it via the gizza CLI.

</details>

<details>
<summary>Could the generated emails or credit-card numbers belong to real people?</summary>

No. Emails are only ever placed on reserved fake domains like `example.com` and
`test.org`, phone numbers are formatted placeholders, and the credit_card column
produces a 16-digit number that passes the Luhn check but is assembled from
random digits — it validates in forms without being a real account.

</details>

<details>
<summary>How do I regenerate the exact same dataset?</summary>

Set the **seed** to any non-zero number: the same seed + columns + row count
always produces byte-identical output, which makes test fixtures reviewable and
diffs stable. Seed `0` (the default) draws fresh random data every run.

</details>

<details>
<summary>What happens if I leave the columns box blank or misspell a column?</summary>

Blank gives you a sensible default set (full_name, email, phone, street, city,
state, zip). A column name that isn't in the supported list produces an error
listing what went wrong, instead of silently skipping the field — use `all` to
get every available column.

</details>

## About this tool

Data Validator checks pasted **CSV** or **JSON** rows against the field rules you
write. It is useful before imports, migrations, mail merges, API uploads, or any
other step where bad rows are expensive to discover later.

Write one rule per line:

```text
email:required
email:type=email
age:type=int
age:min=18
age:max=120
plan:enum=free|pro|team
id:unique
```

The report lists every violation with its record, physical line, field, offending
value, broken rule, and a short explanation. The tool is report-only: it never
modifies your data and it runs locally in your browser.

### Worked example

Input CSV:

```csv
email,age,plan
a@b.com,34,pro
bad-email,200,gold
```

Rules:

```text
email:required
email:type=email
age:type=int
age:min=0
age:max=120
plan:enum=free|pro|team
```

Output highlights three violations on record 2: the invalid email, age above
`120`, and plan outside the allowed enum.

### Options and limits

- **Read data as** — `auto`, `csv`, or `json`. Auto treats values starting with
  `[` or `{` as JSON and everything else as CSV.
- **CSV delimiter** — auto-detects comma, tab, semicolon, and pipe.
- **CSV has a header row** — on by default. If you turn it off, refer to columns
  by 1-based index (`1:required`, `2:type=int`).
- **Max violations to list** — caps how many details are shown while still
  reporting the total count. The accepted range is `1`–`1000`.
- JSON input can be an array of objects, a single object, or JSON Lines / NDJSON.

## FAQ

<details>
<summary>Which validation rules are supported?</summary>

Use `required`, `unique`, `type=int|float|bool|date|email|url`, `min` / `max` for
numeric ranges, `minlen` / `maxlen` for character counts, `regex=...`, and
`enum=a|b|c`. You can also write bare type shorthand such as `age:int`.

</details>

<details>
<summary>Do optional blank values fail type, range, regex, or enum rules?</summary>

No. Every rule except `required` is skipped for a blank or missing value. Combine
rules such as `email:required` and `email:type=email` when a field must be present
and must match a format.

</details>

<details>
<summary>Can I validate JSON as well as CSV?</summary>

Yes. JSON can be an array of objects, one object, or JSON Lines. Field names come
from object keys. CSV can use header names, or 1-based column numbers when the
header option is off.

</details>

<details>
<summary>Are regex rules anchored automatically?</summary>

No. Regex rules use the pattern you provide. Add `^` and `$` yourself when the
whole value must match, for example `email:regex=^[^@\\s]+@[^@\\s]+\\.[^@\\s]+$`.

</details>

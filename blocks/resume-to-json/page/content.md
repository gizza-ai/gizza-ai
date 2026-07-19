## About this tool

This tool converts a pasted resume into the standard **JSON Resume** format — the open
v1.0.0 schema used by resume builders, themes, and registries — and can also **validate**
an existing `resume.json` document against that schema. Everything runs locally in your
browser; the resume text never leaves your machine.

**Extract** reads the resume the way a human skims it: the header block becomes `basics`
(name, label, email, phone, URL, location, and LinkedIn/GitHub/Twitter/GitLab/Stack
Overflow/Medium/Dribbble/Behance links as `profiles`), and recognized section headings —
Experience, Education, Skills, Projects, Languages, Awards, Certifications, Publications,
Volunteer, Interests, References, Summary — become the matching schema sections. Date
ranges like `Jan 2020 – Present`, `03/2021`, or `2016 – 2019` are normalized to the
schema's ISO-8601 partial dates (`YYYY`, `YYYY-MM`, or `YYYY-MM-DD`), with an open
`endDate` for "Present". Bullets become `highlights`, grouped skill lines like
`Languages: Rust, Python` become `{name, keywords}` entries, and `English (Native)`
becomes `{language, fluency}`. Empty sections are omitted, and only canonical schema
field names are emitted, so the output drops straight into any JSON Resume theme or CLI.

**Validate** checks a pasted `resume.json` and returns a report:
`{valid, errors, warnings, summary}`. Type mismatches and malformed dates are errors;
suspicious email/URL formats and unknown keys are warnings (the schema allows extra
fields, but most tools ignore them). The default **Auto-detect** mode validates when the
input is a JSON object and extracts otherwise.

### Worked example

Pasting this resume text:

```text
Jane Doe
Senior Software Engineer
San Francisco, CA | jane.doe@example.com | linkedin.com/in/janedoe

Experience

Senior Software Engineer — Acme Corp
Jan 2020 – Present
- Led migration to a service mesh across 40 services
```

produces this JSON Resume document:

```json
{
  "basics": {
    "name": "Jane Doe",
    "label": "Senior Software Engineer",
    "email": "jane.doe@example.com",
    "location": { "city": "San Francisco", "region": "CA" },
    "profiles": [
      {
        "network": "LinkedIn",
        "username": "janedoe",
        "url": "https://linkedin.com/in/janedoe"
      }
    ]
  },
  "work": [
    {
      "name": "Acme Corp",
      "position": "Senior Software Engineer",
      "startDate": "2020-01",
      "highlights": ["Led migration to a service mesh across 40 services"]
    }
  ]
}
```

### Limits and edge cases

- Input is capped at **1 MiB** of text; larger input is rejected with an error.
- Extraction is a **heuristic, rule-based parser for English section headings** (plus
  common variants like "Employment History" or "Core Competencies"). It is not a machine
  learning model: unusual layouts, tables, multi-column text, or non-English headings may
  land in the wrong section — review the output before publishing it.
- Paste **plain text only**. PDF and Word files must be converted to text first (copy
  and paste from the document works well); this tool does not do OCR.
- Extracted output is schema-valid by construction; a "Present"/"Current" end date is
  represented by omitting `endDate`, exactly as the schema expects.
- In validate mode the input must be a JSON **object**; a JSON array, string, or broken
  JSON returns an error that points at the problem.

## FAQ

<details>
<summary>What is the JSON Resume schema?</summary>

JSON Resume is an open standard that stores a resume as a single JSON document with
well-known sections: `basics`, `work`, `volunteer`, `education`, `awards`,
`certificates`, `publications`, `skills`, `languages`, `interests`, `references`, and
`projects`. Because the shape is standardized, one `resume.json` file can be rendered by
many independent themes, builders, and command-line tools. This tool targets schema
version **v1.0.0** and emits only its canonical field names.

</details>

<details>
<summary>How accurate is the extraction?</summary>

It is a deterministic, rule-based parser — not an AI model. It recognizes the common
resume conventions: a header block with name, title, and contact links; section headings
like "Experience" or "Technical Skills"; `Role — Company` entry lines; date ranges in
`Jan 2020 – Present`, `03/2021`, and `2016 – 2019` styles; and bullet lists. Clean,
conventionally formatted resumes extract very well; creative layouts may need a manual
touch-up afterwards. The output is always structurally valid JSON Resume, so anything
that needs fixing is a copy edit, not a schema repair.

</details>

<details>
<summary>What is the difference between errors and warnings in validation?</summary>

Errors are violations of the schema's hard constraints: a field with the wrong JSON type
(for example `basics.name` as a number, or `skills[0].keywords` as a string instead of an
array) or a date that does not match the schema's ISO-8601 pattern (`YYYY`, `YYYY-MM`,
or `YYYY-MM-DD` — `"Jan 2020"` is an error). Warnings flag things that are technically
allowed but usually mistakes: an email or URL that does not look well-formed, and unknown
keys (the schema sets `additionalProperties: true`, but most renderers silently ignore
fields they do not know). `valid` is `true` when there are no errors.

</details>

<details>
<summary>Why is my start date missing or wrong?</summary>

Dates are read from each entry's header lines, and the first recognized date or date
range wins. Supported forms are `Month YYYY` (including abbreviations like `Sept. 2019`),
`Month DD, YYYY`, `MM/YYYY`, `YYYY-MM`, `YYYY-MM-DD`, and bare `YYYY`, joined by `-`,
`–`, `—`, or the word "to"; `Present`, `Current`, `Ongoing`, and `Today` mark an open
end. If an entry's dates sit inside a paragraph rather than on the title/subtitle lines,
move them next to the role line and re-run.

</details>

<details>
<summary>Can I turn the JSON back into a readable resume?</summary>

Yes — that is the point of the standard. Any JSON Resume-compatible theme, builder, or
CLI can render the extracted document to HTML or PDF, and structured-resume tools
(including a JSON-to-Markdown resume builder) accept the same shape. Tick "Include
$schema reference" to embed the official schema URL and a `meta.version` field so other
tools can identify the format immediately.

</details>

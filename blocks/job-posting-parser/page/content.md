## About this tool

Job posting parser turns a pasted job ad into a structured summary for screening,
spreadsheets, applicant tracking cleanup, or salary comparison. It extracts common
fields recruiters and candidates look for: title, company, location, salary or
compensation range, employment type, remote/hybrid/onsite work mode, experience
level, and known skill keywords.

The parser is deterministic and runs locally in the browser. It uses labelled
fields (`Company:`, `Location:`, `Compensation:`), common job-header patterns, and
a curated skill keyword list. It does not call an LLM, scrape the original job
page, infer missing facts, or verify whether the posting is legitimate.

Worked example:

1. Paste a posting that starts with `Senior Backend Engineer`, has `Company: Acme
   Analytics`, `Location: Remote - US / Toronto`, and a compensation line.
2. Choose JSON output and leave evidence enabled.
3. Copy the parsed fields into a spreadsheet or review checklist, including the
   evidence lines that explain where title, company, location, and salary came
   from.

Limits and edge cases: input is capped at 80,000 characters. The parser works best
when the title, company, location, and pay lines are visible in the pasted text.
Unusual formatting, image-only postings, hidden salary details, or skill names not
in the built-in keyword list may produce warnings or missing fields.

## FAQ

<details>
<summary>Does this use AI to interpret the posting?</summary>

No. It is a deterministic heuristic parser. That makes the output repeatable and
fast, but it also means the tool will not infer unstated facts or understand every
possible synonym.

</details>

<details>
<summary>Can it extract salary when the posting says pay is not disclosed?</summary>

No. If no compensation line or money-like range is present, the salary field is
`null` and the warnings list says salary or compensation was not found.

</details>

<details>
<summary>How are skills detected?</summary>

The parser searches for a curated list of common technical and business skills
such as `Python`, `SQL`, `React`, `Docker`, `AWS`, `Tableau`, and `Excel`. It does
not maintain a full occupation taxonomy, so niche tools may need manual review.

</details>

<details>
<summary>Why include evidence snippets?</summary>

Evidence snippets make the extraction auditable. They show the source text used
for high-value fields, which is useful when cleaning many postings or checking
whether a header line was mistaken for a company or location.

</details>

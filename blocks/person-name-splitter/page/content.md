## About this tool

Person Name Splitter turns a CSV column of free-form full names into structured components: `title`, `first`, `middle`, `last`, and `suffix`. It is meant for contact lists, CRM exports, survey data, and other tables where one full-name field needs to become separate columns.

The parser is deterministic and local. It recognizes common honorifics (`Dr.`, `Mr.`, `Prof.`), suffixes (`Jr.`, `III`, `PhD`, `MD`), surname particles (`van`, `von`, `de`, `del`, `di`, `la`, `mac`, `mc`), hyphenated and apostrophe names, and comma-form names like `Smith, Jane Q`.

### Worked example

Input CSV:

```csv
name,email
Dr. John Michael Smith Jr.,john@example.com
Ludwig van Beethoven,lvb@example.com
"Smith, Jane Q",jane@example.com
```

With **append** output, the original columns are kept and five component columns are added:

```text
name,email,name_title,name_first,name_middle,name_last,name_suffix
Dr. John Michael Smith Jr.,john@example.com,Dr.,John,Michael,Smith,Jr.
Ludwig van Beethoven,lvb@example.com,,Ludwig,,van Beethoven,
"Smith, Jane Q",jane@example.com,,Jane,Q,Smith,
```

Use **replace** to swap the name column for the five components, or **summary** to count how many rows had titles, middle names, suffixes, and ambiguous single-token names.

## Limits & edge cases

- Names are parsed with transparent heuristics, not a demographic database or AI model. Ambiguous names can still be wrong.
- Single-token names such as `Cher` are treated as first names and marked ambiguous in the summary output.
- Comma-form names must be CSV-quoted when the delimiter is comma, for example `"Smith, Jane Q"`.
- Particles are attached to the surname when they appear after the first given-name token, e.g. `Ludwig van Beethoven` → last `van Beethoven`.
- The tool handles one name column per run. Run it again for another name column.
- For non-comma files, choose tab, semicolon, or pipe so output uses the same delimiter.

## FAQ

<details>
<summary>Does this know every culture's naming rules?</summary>

No. It uses a practical set of deterministic rules for common Western-style contact lists: titles, suffixes, particles, hyphenated names, apostrophes, and `Last, First` ordering. It does not query a names database and does not infer gender or ethnicity.

</details>

<details>
<summary>What happens to names with only one word?</summary>

A single token is placed in the `first` column and left without a last name. In **summary** output, that row is counted as ambiguous so you can review it manually.

</details>

<details>
<summary>How should I enter names like `Smith, Jane` in a CSV?</summary>

If your delimiter is comma, quote the cell: `"Smith, Jane"`. Otherwise the comma would be treated as a column separator by normal CSV rules. Tab, semicolon, and pipe files can include commas without quoting if comma is not the delimiter.

</details>

<details>
<summary>Can it preserve my original full-name column?</summary>

Yes. Choose **append** to keep all original columns and add five component columns at the end. Choose **replace** only when you want the full-name column swapped out for the components.

</details>

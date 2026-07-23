## About this tool

Fuzzy Name Matcher helps clean lists of people or organizations when the same entity appears under slightly different spellings. It is useful for CRM dedupe, sanctions or customer-list review, survey cleanup, vendor master data, and spreadsheet joins where exact equality is too strict.

Paste one name per line (or a single-column CSV where the first field is the name). The tool compares every unique name against the others, then either clusters them into canonical groups or lists every matched pair with a score. Duplicate identical rows are counted, so the most frequent spelling can become the canonical name for a group.

### Algorithms

- **Jaro-Winkler** is the default. It is good for short names because it rewards a shared prefix and catches typos like `Jonathan Smith` vs `Jonathon Smith`.
- **Levenshtein** uses a normalized edit-distance ratio. It is stricter and easy to explain: fewer insertions, deletions, or substitutions means a higher score.
- **Soundex** compares phonetic keys. It can catch names that sound alike, such as `Smith` / `Smyth` or `Robert` / `Rupert`, even when the letters differ.

### Modes and outputs

Use **groups** mode when you want a deduplicated mapping table: each input spelling is mapped to a canonical name. Use **pairs** mode when you want review candidates: every pair at or above the threshold is listed with its score, best first.

The default **table** output is readable Markdown. Choose **CSV** when you want to join the mapping back to a spreadsheet, or **JSON** when another script needs structured groups or scored pairs.

### Worked example

Input:

```
Dr. John Adams
John Adams Jr
Jon Adams
Maria Garcia
```

With Jaro-Winkler, threshold `85`, case normalization on, and title/suffix ignoring on, the first three rows are grouped together and `Maria Garcia` stays separate. The output includes a mapping table from each original spelling to the canonical spelling.

### Limits

This is deterministic fuzzy matching, not identity proof. A high score means two strings look or sound similar, not that two people are the same. Review borderline groups before merging production records. Very low thresholds can merge unrelated names; very high thresholds can miss typos. For large lists, pairwise comparison is O(n²), so thousands of unique names can take noticeably longer in the browser.

## FAQ

<details>
<summary>What threshold should I start with?</summary>

Start with `85` for Jaro-Winkler. Lower it toward `80` when you want more recall and are willing to review false positives; raise it toward `90` or `95` when unrelated names are merging. For Soundex, `100` means the phonetic keys are exactly the same.

</details>

<details>
<summary>Should I use groups or pairs?</summary>

Use **groups** when your end goal is deduplication: it creates a canonical name and a mapping for every input spelling. Use **pairs** when you are reviewing candidate matches: it lists every pair above the threshold with its score, which is easier to audit before deciding how to merge.

</details>

<details>
<summary>How are titles and suffixes handled?</summary>

When **Ignore titles and suffixes** is enabled, common honorifics such as `Mr`, `Ms`, `Dr`, and `Prof` are removed from the beginning of a name, and suffixes such as `Jr`, `Sr`, `III`, `PhD`, and `MD` are removed from the end before comparison. The original spelling is still preserved in the output.

</details>

<details>
<summary>Can this deduplicate organizations too?</summary>

Yes, but organization names often need domain-specific cleanup. This tool will catch many aliases like `Acme Corp` and `ACME Corporation`, especially with Jaro-Winkler, but it does not know that `Ltd`, `Limited`, `Inc`, and `LLC` are legal-form variants unless their overall string similarity clears the threshold.

</details>

<details>
<summary>Will a matched group always be correct?</summary>

No. Fuzzy matching is a review aid. Common names, abbreviations, initials, and phonetic matches can create false positives. Treat the output as candidate groups, review the mapping, and keep an audit trail before merging records.

</details>

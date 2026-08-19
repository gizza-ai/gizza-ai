## About this tool

Fuzzy CSV Join merges two CSV tables when the key values are close but not exactly equal. Use it for company names, supplier lists, customer exports, city names, or any reconciliation job where `Acme Ltd` on one side should match `Acme Ltd.` or `Globex Corporation` should match `Globex Corp`.

Paste a left CSV and a right CSV. The first row of each file is treated as its header. Choose a key column for the left table and a key column for the right table by header name or 1-based position. The tool compares every left key with every right key, keeps candidates at or above the similarity threshold, sorts them by score, and emits a joined CSV plus optional match scores. It can also return unmatched rows or a JSON coverage report.

Worked example — join approximate company names:

Left CSV:

```
id,company
1,Acme Ltd
2,Globex Corporation
3,Initech
```

Right CSV:

```
name,city
Acme Ltd.,Berlin
Globex Corp,Cairo
Umbrella,Delhi
```

Settings: left key `company`, right key `name`, algorithm `Jaro-Winkler`, threshold `85`, output `Joined CSV`.

Output:

```
id,company,name,city,match_score
1,Acme Ltd,Acme Ltd.,Berlin,97.8
2,Globex Corporation,Globex Corp,Cairo,94.4
```

`Initech` is not included because this is an inner join by default. Switch to **Left** join to keep it with blank right-side cells, or choose **JSON match report** to see both unmatched lists and coverage stats.

Worked example — keep all left rows:

Settings: same inputs as above, join type `Left`, output `Joined CSV`.

Output:

```
id,company,name,city,match_score
1,Acme Ltd,Acme Ltd.,Berlin,97.8
2,Globex Corporation,Globex Corp,Cairo,94.4
3,Initech,,,
```

Worked example — inspect unmatched rows:

Set output to `JSON match report` to get matched pairs, unmatched left rows, unmatched right rows, and counts:

```
{
  "algorithm": "jaro_winkler",
  "threshold": 85.0,
  "max_matches": 1,
  "stats": {
    "left_rows": 3,
    "right_rows": 3,
    "matched_pairs": 2,
    "matched_left_rows": 2,
    "unmatched_left_rows": 1,
    "matched_right_rows": 2,
    "unmatched_right_rows": 1
  }
}
```

## Choosing an algorithm

- **Jaro-Winkler** is the default and is usually best for names, brands, and abbreviations because it rewards matching prefixes.
- **Levenshtein ratio** is stricter edit-distance matching. It works well for typos of similar length.
- **Token sort** compares sorted words, so `Acme Limited` and `Limited Acme` can match even when word order changes.
- **Soundex** is phonetic and useful for English-ish names that sound alike, but it is coarse; review the scores before trusting it.

## Limits and edge cases

- Each side is capped at **2,000 data rows** (excluding the header). Fuzzy joining compares every left key with every right key, so the worst case is 4 million comparisons in the browser.
- The threshold is **0–100** and inclusive. A row scoring exactly 85 matches when the threshold is 85.
- `max_matches` can keep more than one right row per left row. Candidates are sorted by score descending; ties keep the earlier right row so output stays deterministic.
- Output columns are all left columns followed by all right columns. If a right header collides with a left header, it gets a `_right` suffix. The right key column is kept because, in a fuzzy join, its value is the evidence for the match.
- Delimiter accepts a single character or `comma`, `tab`, `semicolon`, or `pipe`. Quoted CSV fields are handled by the CSV parser.
- Blank `right_key` reuses the left key reference. That only works when both tables share the same header name or compatible column index.
- `ignore punctuation` removes symbols before scoring, but it does not modify the output values.
- `unmatched_left`, `unmatched_right`, and `json` report leftovers independently of join type; join type only controls which rows appear in the joined CSV view.

## FAQ

<details>
<summary>How is this different from a normal CSV join?</summary>

A normal join requires exact key equality: `Acme Ltd` and `Acme Ltd.` are different strings, so they do not join. This tool scores how similar the keys are and joins rows whose score is at or above your threshold. It is for reconciliation and record-linkage cleanup before you commit a final mapping.

</details>

<details>
<summary>What threshold should I start with?</summary>

Start around **85** for company or product names. Lower it if real matches are being missed; raise it if unrelated rows are being paired. Always inspect the `match_score` column or the JSON report on a small sample before running a large merge.

</details>

<details>
<summary>Can it join on multiple columns?</summary>

Not directly. Build a composite key first, such as `name + " " + city`, then use that single column as the left and right key. Composite matching with separate weights per column is a larger record-linkage model and is intentionally out of scope for this deterministic browser tool.

</details>

<details>
<summary>Why are there duplicate left rows in the output?</summary>

If `max_matches` is greater than 1, one left row can match several right rows. Each candidate becomes its own output row, ordered by score. Set `max_matches` back to 1 when you want the single best candidate per left row.

</details>

<details>
<summary>Does the tool upload my CSV files?</summary>

No. The comparison runs locally in WebAssembly on the page or locally in the CLI. The CSV text you paste is not sent to a server by this tool.

</details>

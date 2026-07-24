## About this tool

CSV anti-join is the spreadsheet version of "show me what is missing." Paste two CSV tables,
choose the key column, and the tool returns rows from CSV A whose key does **not** appear in CSV B.
That is useful for reconciling exports, finding customers not present in another system, locating
inventory codes missing from a reference list, or preparing a cleanup list before a migration.

Use **A only** for the standard left anti-join. Switch to **B only** for the reverse check, or
**Both sides** to emit a symmetric difference with a leading `_source` column that marks whether
each unmatched row came from A or B. Keys can be header names (`id`) or 1-based column indexes
(`1`), and comma-separated keys (`first,last`) form a composite match. If the key columns have
different names in the two CSVs, set the B key separately — matching is by value, not by header.

Duplicate keys in the lookup table do not multiply rows. If A contains two unmatched rows with the
same key, both A rows are kept, because they are real rows missing from B. You can choose comma,
tab, semicolon, pipe, or any single-character delimiter, and optionally make key matching
case-insensitive or trim whitespace around key values.

## Worked example

CSV A:

```csv
id,name
1,Alice
2,Bob
3,Carol
```

CSV B:

```csv
id,city
2,Berlin
3,Cairo
4,Delhi
```

Key: `id`, direction: `A only`.

Output:

```csv
id,name
1,Alice
```

## FAQ

<details>
<summary>What is an anti-join?</summary>

An anti-join returns rows from one table when their key does not exist in another table. It is the
opposite of an inner join: instead of keeping matches, it keeps non-matches. In SQL terms, this is
similar to `LEFT JOIN ... WHERE right.key IS NULL` or a `NOT EXISTS` query.

</details>

<details>
<summary>Can the key column have a different name in CSV B?</summary>

Yes. Put the A key in **Key column(s) in A** and the B key in **Key column(s) in B**. For example,
A might use `customer_id` while B uses `id`. The tool compares the key values, so the header names
do not need to match. Leave the B key blank when both files use the same key name or index.

</details>

<details>
<summary>How are duplicate keys handled?</summary>

The comparison side is treated as a key set, so duplicate keys in B do not multiply or otherwise
affect A's output. On the output side, every unmatched row is kept. If CSV A has two rows with key
`x` and `x` is absent from B, both rows appear in the result.

</details>

<details>
<summary>Can I compare on more than one column?</summary>

Yes. Use a comma-separated list such as `first,last` or `1,3`. The same number of key columns must
be supplied for CSV B. A row matches only when all key parts match after the case-sensitivity and
trim settings are applied.

</details>

<details>
<summary>Does the tool upload my CSV data?</summary>

No. The anti-join runs in WebAssembly inside your browser tab. Your pasted CSV text is processed
locally and is not sent to a server.

</details>

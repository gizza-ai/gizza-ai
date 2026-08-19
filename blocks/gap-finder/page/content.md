## About this tool

Sequential identifiers are easy to lose track of once they come from multiple exports or manual
entries. Paste the values here and the gap finder audits the run locally: it sorts the numeric
counter, reports missing values as compact ranges, and optionally calls out duplicates and entries
that were pasted out of order.

It works with plain numbers and with IDs that share the same prefix or suffix. For example,
`INV-0001`, `INV-0002`, and `INV-0005` are treated as a sequence with the counter `0001..0005`, and
missing values are printed with the same prefix and zero padding.

### Worked example

With this input and **Expected start** set to `INV-1000` and **Expected end** set to `INV-1008`:

```
INV-1001
INV-1002
INV-1004
INV-1004
INV-1007
```

the report is:

```
Range: INV-1000 to INV-1008 (step 1)
Present: 4 of 9 expected
Missing: 5 (INV-1000, INV-1003, INV-1005 to INV-1006, INV-1008)
Duplicates: 1 (INV-1004 x2)
```

### Options

**Expected step** defaults to `1`, which means every integer in the range should appear. Set it to
`2` for even-numbered cheque runs or to `10` for identifiers issued in tens; values inside the range
that do not land on the step are reported as off-step.

Leave **Expected start** and **Expected end** empty to use the lowest and highest values found. Fill
them when you need to catch missing numbers at the beginning or end of a batch.

**Output format** can produce a human-readable report, every missing value one per line, a TSV gap
table (`gap_start`, `gap_end`, `count`), or JSON for automation. **List limit** caps how many gaps or
individual missing values are printed; totals remain complete and truncated output says so.

### Limits

Up to **20,000 pasted entries** per run, and up to **5,000,000 expected slots** between start and end.
All entries must share the same prefix and suffix when using ID mode; mixed series such as `INV-1`
and `ORD-2` are rejected so two unrelated runs are not audited as one.

## FAQ

<details>
<summary>Can I use invoice numbers like INV-0007 instead of plain numbers?</summary>

Yes. Leave **ID format** on auto and the tool uses the last run of digits as the counter. Missing
values keep the same prefix, suffix, and zero padding, so `INV-0007` leads to results like
`INV-0008`.

</details>

<details>
<summary>How do I find gaps at the start or end of the sequence?</summary>

Set **Expected start** or **Expected end**. If both are blank, the tool uses the lowest and highest
values already present, which cannot reveal a value missing before the first pasted entry or after
the last pasted entry.

</details>

<details>
<summary>What is the difference between report, missing, table, and JSON output?</summary>

Report is the readable audit summary. Missing lists each missing value on its own line for copying
into another system. Table compresses gaps into `gap_start`, `gap_end`, and `count` columns. JSON
contains the same counts and lists for scripts.

</details>

<details>
<summary>Why does the tool reject mixed prefixes?</summary>

A mixed list usually means two separate sequences were pasted together. Auditing `INV-` and `ORD-`
IDs as one run would produce misleading gaps, so the tool asks you to split them into separate runs.

</details>

<details>
<summary>Can it detect duplicate IDs?</summary>

Yes. **Report duplicates** is on by default and shows each repeated value with its count, such as
`INV-1004 x2`. Turn it off if you only want missing-value information.

</details>

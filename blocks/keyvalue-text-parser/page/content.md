## About this tool

Key-value text shows up everywhere: pasted HTTP headers, form exports, scan metadata, support tickets, email snippets, invoice notes, and configuration fragments. They are easy for humans to read but annoying to feed into scripts because each source chooses its own separator, repeats fields in different ways, and mixes useful lines with headings or comments.

This parser turns that messy text into JSON. It understands `key: value`, `key=value`, tab-separated rows, pipes, or a custom separator like `->`. By default it skips prose lines, trims whitespace, removes one matching pair of quotes around values, and groups repeated keys into arrays so information is not silently overwritten.

Choose **records** when blank lines separate people, products, or tickets. Choose **pairs** when order and source line numbers matter more than a folded object. Turn on type inference only when you want unquoted `true`, `false`, `null`, and safe numbers to become real JSON values — IDs with leading zeros stay strings.

### Worked example

Input:

```text
Title: Quarterly report
Owner: Ada
tag: finance
tag: draft
Reviewed = false
```

With duplicate grouping, `snake_case` keys, and type inference enabled, output is:

```json
{
  "title": "Quarterly report",
  "owner": "Ada",
  "tag": [
    "finance",
    "draft"
  ],
  "reviewed": false
}
```

For multiple records, put a blank line between blocks:

```text
name: Ada
role: engineer

name: Grace
role: admiral
```

and select **Blank-line records** to get:

```json
[
  {
    "name": "Ada",
    "role": "engineer"
  },
  {
    "name": "Grace",
    "role": "admiral"
  }
]
```

## Options and limits

- Up to 10,000 input lines per run.
- `auto` separator uses whichever of `:` or `=` appears first on each line, so URLs such as `https://example.test/a=b` stay in the value when the first separator is the colon after the key.
- Duplicate-key policies are `group`, `last`, `first`, and `error`.
- Comment prefixes are comma-separated and match after leading whitespace. The default skips lines beginning with `#`, `;`, or `//`.
- Type inference is conservative: leading-zero IDs and `+15551234`-style phone numbers remain strings.
- The parser is line-oriented. It does not try to parse full YAML, TOML, INI sections, quoted multiline values, or nested objects.

## FAQ

<details>
<summary>How is this different from a YAML or TOML parser?</summary>

YAML, TOML, and INI parsers expect a formal file format. This tool is for loose pasted text where some lines are headings, some are comments, and the same key may appear several times. It extracts the simple `key separator value` lines and makes a predictable JSON object, record array, or pair list.

</details>

<details>
<summary>What happens when the same key appears more than once?</summary>

The default is to group repeated keys into an array in the order they appeared, so `tag: finance` followed by `tag: draft` becomes `"tag": ["finance", "draft"]`. You can instead keep the first value, keep the last value, or fail with an error on the duplicate line.

</details>

<details>
<summary>When should I use records instead of object output?</summary>

Use records when blank lines separate repeated entities, such as people, tickets, products, or scanned documents. Each block becomes its own JSON object. If your input is one continuous block, object output is simpler.

</details>

<details>
<summary>Does type inference change IDs or phone numbers?</summary>

It only converts values that are clearly safe: booleans, null-like words, and ordinary numbers. Values with leading zeros or a leading plus sign stay strings so ZIP codes, IDs, and phone numbers are not damaged.

</details>

<details>
<summary>Can I preserve every original line?</summary>

Choose **Ordered pairs with line numbers**. That output shape keeps one `{ "key", "value", "line" }` object per parsed pair, preserving order and source line numbers. Lines without separators are still skipped unless you set unmatched lines to error.

</details>

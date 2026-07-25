## About this CSV column tool

Use this tool when a CSV field contains multiple pieces of data in one cell, or
when several columns need to become one display field. It works like a browser
version of spreadsheet "Text to Columns" plus the inverse concatenate operation.

For splitting, choose one column by header name (for example `name`) or by
1-based index (for example `2`), choose a literal separator, and optionally name
the generated columns:

```csv
id,name
1,John Doe
2,Ada Lovelace
```

Splitting `name` on a space with output names `first,last` produces:

```csv
id,first,last
1,John,Doe
2,Ada,Lovelace
```

For concatenating, list the source columns in order. Joining `first,last` with a
space and naming the output `name` turns `Ada,Lovelace,36` into
`Ada Lovelace,36`.

### Options and limits

- `max_columns = 0` splits on every occurrence; `max_columns = 2` splits on the
  first occurrence only and leaves the rest of the text in the second output.
- Missing separators are not errors: the original cell goes into the first output
  column and the remaining generated columns are blank.
- `keep_source` keeps the original column(s); otherwise the generated column(s)
  replace them in place.
- The CSV field delimiter can be comma, tab, semicolon, or pipe.
- Splitting uses a literal string separator, not a regex.

Everything runs locally in your browser. Your CSV data is not uploaded.

### FAQ

<details>
<summary>Can I split a full name into first and last columns?</summary>

Yes. Use `mode=split`, set `columns` to your name column, set `separator` to a
space, and set `names` to something like `first,last`. For names with middle
parts, set `max_columns=2` if you want everything after the first space to stay
in the second column.

</details>

<details>
<summary>How do I split only on the first delimiter?</summary>

Set `max_columns` to `2`. For example, splitting `San Jose, CA, USA` on comma
with `max_columns=2` produces `San Jose` and `CA, USA` instead of three columns.

</details>

<details>
<summary>Can I select columns by number instead of header name?</summary>

Yes. Use 1-based column numbers such as `1` or `2`. This is required when
`has_header` is off, and it also works as a fallback when a header name is not
found.

</details>

<details>
<summary>Does the separator support regular expressions?</summary>

No. The separator is a literal string. This keeps the tool predictable for CSV
cleanup tasks and avoids regex escaping surprises.

</details>

## About this tool

Use this converter when you need to inspect InfluxDB line protocol in a spreadsheet, or turn a CSV export into line protocol for `influx write`. It works both ways: line protocol becomes a wide or long CSV table, and CSV becomes escaped, typed line protocol with measurements, tags, fields, and timestamps in the right places.

Example line protocol input:

```text
cpu,host=host1,region=eu usage=64.23,busy=true 1577836800000000000
cpu,host=host2,region=us usage=12.5,busy=false 1577836800000000000
```

With the default wide CSV layout, that becomes columns for `measurement`, `time`, tag keys (`host`, `region`) and field keys (`busy`, `usage`). Turn on `emit_annotations` when you want a `#datatype` row that can round-trip through InfluxDB's annotated CSV import path. For CSV to line protocol, use the annotation rows, inline `name|datatype|default` headers, or the measurement/tag/field/time parameters to describe column roles.

Limits and edge cases: input is capped at 2,000,000 characters, 200,000 lines and 1,000 output columns. The parser supports common annotated CSV rows (`#datatype`, `#constant`, `#default`), standard line protocol escapes, RFC3339 timestamps and Unix timestamp precisions from seconds to nanoseconds. It does not implement timezone annotation rows, Go-style custom time layouts, locale-specific decimal separators, file/directory reads, or direct writes to an InfluxDB server.

## FAQ

<details>
<summary>What is the difference between wide and long CSV?</summary>

Wide CSV writes one row per line protocol point and creates one column for each distinct tag or field key. It is easiest to read in a spreadsheet. Long CSV writes one row per field value, with `field` and `value` columns; it is better for heterogeneous measurements or data tools that prefer tidy data.

</details>

<details>
<summary>How does CSV to line protocol decide which columns are tags and fields?</summary>

Annotated CSV rows and inline headers have priority. Without those, `measurement`, `tag_columns`, `field_columns`, and `time_column` tell the converter how to map columns. If `field_columns` is blank, every column that is not the measurement, a tag, or the time column becomes a field.

</details>

<details>
<summary>Does it preserve line protocol escaping and field types?</summary>

Yes. Measurements, tag keys, tag values, field keys and string field values are escaped according to their line protocol position. Integer (`1i`), unsigned (`1u`), float, boolean, and quoted string field values are parsed and emitted with their line protocol spelling.

</details>

<details>
<summary>Can it write directly to InfluxDB?</summary>

No. This is an offline text converter. Copy the generated line protocol into your import workflow, or download/copy the annotated CSV and use the InfluxDB CLI or API yourself. Keeping network writes out of the tool makes the result deterministic and safe to preview.

</details>

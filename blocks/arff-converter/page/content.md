## Convert ARFF and CSV datasets locally

Weka's ARFF format stores a relation name, typed attributes and a data section.
This converter turns ARFF into CSV for spreadsheets and notebooks, or turns CSV
back into ARFF for Weka-style machine-learning workflows. Everything runs in the
browser with the same pure-Rust core used by the CLI and chat tool.

### Worked examples

ARFF to CSV:

```arff
@relation weather
@attribute outlook {sunny,overcast,rainy}
@attribute temperature numeric
@data
sunny,85
rainy,70
```

With direction **ARFF → CSV**, the output is:

```csv
outlook,temperature
sunny,85
rainy,70
```

CSV to ARFF:

```csv
outlook,temperature
sunny,85
rainy,70
```

With relation `weather`, the converter infers `temperature` as `numeric` and
`outlook` as a nominal label set because it has only a few distinct values.

### Preserving attribute types

CSV has no native place to store ARFF attribute types. Enable **Include/consume
CSV type row** when converting ARFF to CSV to add a second row containing each
attribute type. Convert that CSV back with the same option enabled and the ARFF
header keeps numeric, string, date and nominal declarations instead of guessing.
You can also force types with `column_types`, for example
`class:nominal,id:string,3:date`.

### Supported ARFF details

The converter handles `%` comments, quoted names and values, escaped newlines and
tabs, nominal `{a,b,c}` label sets, date attributes with a format pattern,
missing values as `?`, dense rows, sparse `{index value, ...}` rows and trailing
instance weights. Relational multi-instance attributes are intentionally rejected
with a clear error because flattening them is schema-specific.

## FAQ

<details>
<summary>How are CSV column types inferred?</summary>

All-numeric columns become `numeric`. Non-numeric columns with at most the
nominal threshold of distinct values become nominal attributes such as
`{yes,no}`. Larger text columns become `string`. You can override any column by
name or 1-based index with `column_types`.

</details>

<details>
<summary>How do I keep ARFF types during a round trip?</summary>

Turn on **Include/consume CSV type row** when converting ARFF to CSV. The CSV
then has a second row with type declarations. When converting back to ARFF with
the same option, those declarations are used instead of guessing from data.

</details>

<details>
<summary>Can it read sparse ARFF rows?</summary>

Yes. Sparse rows are expanded to dense CSV cells using type-aware defaults: zero
for numeric attributes, the first label for nominal attributes, and empty text
for string/date attributes. When writing ARFF you can choose dense or sparse row
output.

</details>

<details>
<summary>What happens to missing values?</summary>

ARFF `?` values become empty CSV cells by default. Set a missing-value token such
as `NA` if your CSV workflow needs an explicit marker; the same token is read
back as `?` when converting CSV to ARFF.

</details>

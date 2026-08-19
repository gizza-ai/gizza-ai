## About this tool

Data Format Sniffer inspects a pasted sample and reports the most likely structure before you hand it to a parser or converter. It recognises CSV, TSV, semicolon/pipe/custom-delimited text, JSON, JSON Lines, XML, HTML, Markdown tables, fixed-width text, marker-led YAML, and common binary containers such as Parquet and Avro when their bytes are supplied as base64 or hex.

The report includes a confidence score, encoding note, line-ending style, delimiter and quote character, a per-candidate delimiter score table, header-row guess, column count, inferred column types, a row preview, and warnings for ragged rows or sampled-only analysis. Use `output=json` when another workflow needs the same facts as machine-readable fields.

### Worked example

Input:

```text
name,age,city,joined
Ada,36,London,1815-12-10
Alan,41,Wilmslow,1912-06-23
Grace,45,New York,1906-12-09
```

With the default settings the tool reports CSV with a comma delimiter, UTF-8 text input, LF line endings, four columns, a likely header row, and column types such as integer and date. If your data uses a less common delimiter, put it in `extra_delimiters`, for example `^` for caret-delimited records.

### Limits and edge cases

- Decoded input is capped at 1 MiB so the browser and WASM sandbox stay responsive.
- `sample_lines` controls delimiter and type inference. The whole input still contributes byte and line counts, and whole-document JSON is checked before sampling.
- Pasted text has already been decoded by the browser, so encoding is reported as UTF-8 by construction. To detect an original file encoding or magic bytes, paste bytes as base64 or hex and set `input_form`.
- YAML is only identified when the sample starts with `---` or `%YAML`; marker-less YAML often overlaps with colon-delimited text.
- This is a sniffer, not a validator or converter. Use a dedicated CSV/JSON validator or converter when you need full error listings or transformed output.

## FAQ

<details>
<summary>Can it detect Parquet or Avro from a real file?</summary>

Yes, if you provide the beginning of the file as bytes using `input_form=base64` or `input_form=hex`. The tool checks magic bytes such as `PAR1` for Parquet and `Obj\x01` for Avro. The page itself does not read uploaded files, so paste a byte sample instead of a filename.

</details>

<details>
<summary>Why does pasted text always say UTF-8?</summary>

A browser text field contains Unicode text, not the original file bytes. By the time the tool receives `input_form=text`, the original encoding has already been decoded. Use base64 or hex input when you need BOM or statistical encoding detection over real bytes.

</details>

<details>
<summary>How reliable is delimiter detection?</summary>

The sniffer tries comma, tab, semicolon, pipe, colon, tilde, space, and any extra delimiters you provide. It prefers candidates that produce at least two columns with consistent row widths, then reports every candidate's column count and consistency so you can spot ambiguous samples.

</details>

<details>
<summary>Does it validate every row in the file?</summary>

No. It samples leading lines for speed and reports early ragged rows when the winning delimiter gives inconsistent column counts. For a complete validation pass with row-by-row errors, use a validation-specific tool after the format has been identified.

</details>

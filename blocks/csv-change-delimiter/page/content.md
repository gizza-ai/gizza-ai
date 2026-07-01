## Change CSV delimiter

Re-save delimited data with a different field separator. Pick what it's separated
by now (**from**) and what you want (**to**) — comma, tab, semicolon, pipe, or any
single character. Quoting is fixed up correctly: fields that contain the new
separator get quoted, and fields that no longer need quotes lose them. Runs in
your browser; nothing is uploaded.

### Notes

- Use the words `comma`, `tab`, `semicolon`, or `pipe`, or type any single
  character.
- Common conversions: CSV → TSV (`,` → `tab`), or European `;`-CSV → standard
  `,`-CSV.
- Embedded quotes, newlines, and the separator inside fields are handled per
  RFC 4180.

### FAQ

<details>
<summary>Can the separator be more than one character?</summary>

No — each separator must be a single character, or one of the named words
`comma`, `tab`, `semicolon`, `pipe`. Something like `||` or `::` is rejected with
an error. The defaults are `,` for **from** and `tab` for **to**, so running with
no options is a straight CSV → TSV conversion.

</details>

<details>
<summary>What happens when a field contains the new separator?</summary>

It gets wrapped in double quotes automatically, per RFC 4180 — so `x;y` stays one
field after switching to semicolons. The reverse also happens: fields that were
quoted only because they contained the *old* separator lose their now-unneeded
quotes. Embedded quotes and newlines inside fields survive the conversion.

</details>

<details>
<summary>Is my data uploaded?</summary>

No — it's processed locally in your browser with
WebAssembly.

</details>

<details>
<summary>Need to convert to/from JSON instead?</summary>

Use the CSV ⇄ JSON converter.

</details>

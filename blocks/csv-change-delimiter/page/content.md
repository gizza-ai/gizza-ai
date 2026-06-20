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

**Is my data uploaded?** No — it's processed locally in your browser with
WebAssembly.

**Need to convert to/from JSON instead?** Use the CSV ⇄ JSON converter.

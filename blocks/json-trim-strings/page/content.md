## About this tool

`json-trim-strings` walks a valid JSON document and rewrites the whitespace of every string **value**, at any depth, inside objects and arrays. Structure is never touched: numbers, booleans and nulls are copied through unchanged, key order is preserved, and arrays keep their length and their positions.

That makes it the opposite of a minifier. A minifier removes the whitespace *between* tokens and deliberately protects the inside of every quoted string. Here the structure is fine and the payload is dirty — `" Berlin"` and `"Berlin"` are different join keys, a padded `" 42 "` breaks a downstream cast, and a duplicate check quietly reports two customers where there is one.

The defaults are the safe ones: trim both ends, leave the inside of the string exactly as it was, treat the full Unicode whitespace set as blank, and copy object keys verbatim. Everything past that is opt-in.

### Worked example

Input, with the defaults (trim both ends, keep the interior, indent 2):

```json
{"name": "  Ada  ", "city": " Berlin ", "tags": [" x ", "y  "], "visits": 7}
```

Output:

```json
{
  "name": "Ada",
  "city": "Berlin",
  "tags": [
    "x",
    "y"
  ],
  "visits": 7
}
```

`visits` stayed the number `7` — a trimmed string is still a string here, and nothing is retyped.

### Choosing the options

- **Trim which end** — `both` (default), `leading`, `trailing`, or `none` to leave the edges alone and rewrite only the interior.
- **Whitespace inside the string** — `keep` (default) makes this a pure trim; `collapse` turns every inner run into a single space, so `"  Ada   Lovelace  "` becomes `"Ada Lovelace"`; `remove` deletes it, so `" AB 12  CD "` becomes `"AB12CD"` — useful for SKUs, IBANs and part numbers.
- **What counts as whitespace** — `unicode` (default) also catches the invisible characters that survive a spreadsheet or web copy-paste: non-breaking space `U+00A0`, narrow no-break space `U+202F`, ideographic space `U+3000`. `ascii` limits it to space, tab, newline, carriage return and form feed, so an NBSP stays part of the content.
- **Also trim object keys** — off by default. When on, a key literally named `" first name "` becomes `"first name"`.
- **Values left empty by the trim** — a value that was nothing but whitespace becomes `""` (default), `null`, or is dropped from its object.
- **Only these keys / Keys to never touch** — limit the pass to named fields, or protect fields whose padding is meaningful. Both match key names at any depth, and a key in both lists is skipped.
- **Indent** — `0` minifies onto one line, `2` (default) through `8` pretty-print.

Everything runs locally in WebAssembly in your browser. Nothing is uploaded, there is no account, and there is no daily quota.

### Limits and edge cases

- The input must already be valid JSON. Comments, trailing commas and unquoted keys are rejected with the line and column of the problem — repair the document first.
- The input cap is 5 MB (5,000,000 bytes); the whole document is parsed into memory.
- Values are never retyped. A trimmed `" 42 "` becomes the string `"42"`, not the number `42` — use `json-coerce-types` for that.
- Zero-width characters (`U+200B` zero-width space, `U+FEFF` byte-order mark) are content, not whitespace, under both whitespace settings — `zero-width-cleaner` removes those.
- With key trimming on, two keys in the same object that trim to the same name are an error, not a silent overwrite.
- **Only these keys** matches key names, so a bare top-level string document has no key and is left alone when the list is non-empty.
- Inside an array, a value emptied by the trim becomes `null` rather than disappearing, so indexes and array length never shift.
- Indent values outside 0–8 are clamped rather than rejected.

## FAQ

<details>
<summary>How is this different from a JSON minifier?</summary>

They are opposites. A minifier strips the whitespace **between** tokens — the indentation and newlines that make a document readable — and protects everything inside the quotes. This tool leaves the structure alone and cleans the text **inside** the quotes. Set the indent to `0` if you also want the result minified.

</details>

<details>
<summary>My values look identical but still do not match. What is going on?</summary>

Almost always a non-breaking space. Spreadsheets and web pages are full of `U+00A0`, `U+202F` and `U+3000`, and they are invisible in every editor while being a different byte sequence from a plain space. The default `unicode` whitespace setting trims them; switch to `ascii` only if those characters are genuinely part of your data.

</details>

<details>
<summary>Will this change my numbers, booleans or nulls?</summary>

No. Only string values are rewritten. `7`, `true` and `null` are copied through byte-for-byte, and a string is always still a string afterwards — `" 42 "` becomes `"42"`, never `42`. If you want real types back, run the result through `json-coerce-types`.

</details>

<details>
<summary>Can I protect one field, like an indented code block or a hash?</summary>

Yes. Put its key in **Keys to never touch** and its value is copied byte-for-byte, at any depth. The reverse also works: put a key in **Only these keys** to clean just that field and leave the rest of the document alone. Strings inside an array inherit the key their array is stored under, so naming `tags` reaches the items of `"tags": [" x "]`.

</details>

<details>
<summary>What happens to a value that was only whitespace?</summary>

You choose. By default `"   "` becomes `""`. Set **Values left empty by the trim** to `null` if a blank really means "missing", or to `drop` to remove the key from its object entirely. A value that was already `""` before the trim is never affected by this setting, and inside an array `drop` writes `null` so positions do not shift.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The whole transform is a WebAssembly module running in your browser tab, so the document never leaves your machine. The same code ships in the `gizza` CLI if you would rather clean files from a terminal or a script.

</details>

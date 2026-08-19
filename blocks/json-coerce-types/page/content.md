## About this tool

`json-coerce-types` is for already-valid JSON where every scalar was imported as text. It walks objects and arrays recursively and changes safe string values into native JSON types: `"42"` becomes `42`, `"true"` becomes `true`, and `"null"` becomes `null`.

The defaults are deliberately conservative for production data cleaning. Numeric strings, booleans and `"null"` are enabled, but leading-zero values such as ZIP codes, account IDs and phone-like fields stay strings unless you choose "Coerce to numbers". Integers outside the 64-bit range also stay strings instead of being rounded through floating point.

Use `skip_keys` when certain field names must never be touched, or `only_keys` when you want to retype one branch such as `counts` or `scores` and leave the rest of the document exactly as strings. Switch the output to "Change report" to audit every path that would change before copying the retyped JSON.

### Limits and edge cases

- Input must be valid JSON first. If the document has comments, trailing commas or unquoted keys, repair it before using this tool.
- The input cap is 5 MB.
- Object keys are never renamed or retyped; only string values can change.
- JSON has no date or BigInt type. Date-looking strings stay strings, and huge integers that would lose precision stay strings.
- Thousands separators are only accepted when the grouping is well-formed, such as `"1,234"` or `"1,234.5"`.

## FAQ

<details>
<summary>Will this change ZIP codes, phone numbers or IDs?</summary>

Not by default when they contain redundant leading zeros. Values such as `"02134"`, `"007"` and `"0005"` stay strings while `leading_zeros` is set to `keep`. For extra safety, add field names such as `zip`, `phone` or `id` to `skip_keys` so their whole subtree is copied through untouched.

</details>

<details>
<summary>Does this fix broken JSON?</summary>

No. This tool starts after parsing succeeds and only retypes string values inside a valid JSON document. If your input has comments, trailing commas, unquoted object keys or missing quotes, run a JSON repair or formatter first and then coerce the repaired document.

</details>

<details>
<summary>How do I see what changed before using the result?</summary>

Set `output` to `report`. The result becomes a plain-text audit such as `$.age: "42" -> 42`, including nested array paths like `$.items[0].active`. Switch back to `json` when the report looks safe.

</details>

<details>
<summary>Why did a number-looking string stay a string?</summary>

The parser only accepts strict JSON number shapes unless you enable the relevant option. `"1,234"` needs `thousands`, zero-padded strings need `leading_zeros = coerce`, and values such as `"12px"`, `"0x1f"`, `".5"`, `"1."`, `"NaN"` and `"Infinity"` are intentionally left alone.

</details>

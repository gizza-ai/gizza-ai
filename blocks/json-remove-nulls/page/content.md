## About this tool

**JSON Null Remover** parses your JSON, walks the whole tree, and drops every object key whose
value is `null` — at any depth, including objects nested inside arrays. What's left is
re-serialized as valid JSON, pretty-printed or minified. Payloads from ORMs, form submissions and
API responses shrink noticeably once the placeholder `null`s are gone.

Emptiness beyond `null` is **opt-in**, one checkbox each: empty strings (`""`), empty arrays
(`[]`) and empty objects (`{}`). Nothing else is touched — `false` and `0` are real values and are
always kept, and the original key order is preserved.

Removal is **bottom-up**, so it cascades. Children are cleaned before their parent decides what to
drop: with **Also remove empty objects** on, `{"a": {"b": {"c": null}}}` collapses all the way to
`{}`, because each level is emptied in turn. With that box off, you get `{"a": {"b": {}}}` — the
`null` is gone but the structure stays.

### Worked example

Input:

```json
{ "name": "Ada", "nickname": null, "address": { "city": "London", "zip": null } }
```

With the defaults (nulls only, **Indent = 2**), the output is:

```json
{
  "name": "Ada",
  "address": {
    "city": "London"
  }
}
```

Now the same input with **Also remove empty objects** on and every value under `address` null —
`{"name": "Ada", "address": {"zip": null}}` — collapses to `{"name": "Ada"}`: the emptied
`address` object goes too.

### Options

- **Also remove empty strings** — drops `""` values. Off by default.
- **Also remove empty arrays** — drops `[]` values, including arrays emptied by the prune itself.
- **Also remove empty objects** — drops `{}` values, including objects emptied by the prune itself
  (this is what makes removal cascade upward).
- **Trim whitespace from string values** — trims each string first, so `"   "` becomes `""` and is
  removable when the empty-strings box is also ticked. On its own it just tidies the values.
- **Values inside arrays** — *Compact* (default) drops removable elements and closes the gap, so
  `[1, null, 2]` becomes `[1, 2]`. *Keep* leaves array elements exactly where they are, so indices
  stay stable for positional/tuple-shaped data; objects inside the array are still pruned.
- **Indent spaces** — 1–8 spaces per level (default 2), or `0` to minify onto one line.

### Limits and behavior

- The input must be **valid JSON**. It's parsed before anything is removed, so a broken document is
  rejected with the parser's exact line and column — this tool prunes JSON, it doesn't repair it.
  Trailing commas, comments and unquoted keys are not accepted.
- The **root value is never dropped**. A document that prunes down to nothing comes back as `{}`
  (or `[]`), never as an empty output, so the result is always valid JSON you can paste onwards.
- `false`, `0`, `"0"` and `" "` (without trimming) are values, not emptiness — they always survive.
- Duplicate keys in the input resolve to the last occurrence, as in any strict JSON parser.
- Everything runs locally in WebAssembly, so document size is bounded only by your browser's
  memory; multi-megabyte documents work but take a moment to render.

## FAQ

<details>
<summary>Does it remove nulls inside nested objects and arrays?</summary>

Yes. The prune is recursive and reaches every level: keys with `null` values are removed from
objects at any depth, including objects that live inside arrays. Whether a bare `null` *element* of
an array is removed depends on the **Values inside arrays** setting.

</details>

<details>
<summary>What happens to a null sitting directly in an array?</summary>

With **Values inside arrays = Compact** (the default) it's dropped and the array closes up, so
`[1, null, 2]` becomes `[1, 2]`. Choose **Keep** when array positions matter — a fixed-length row
or tuple — and the `null` stays put while objects inside the array are still cleaned.

</details>

<details>
<summary>Are `false` and `0` removed too?</summary>

No, never. Only `null` is removed by default; empty strings, empty arrays and empty objects are
removed only when you tick their boxes. Booleans and numbers are always kept, so `false`, `0` and
`0.0` survive untouched — losing them is the classic bug in hand-rolled "remove empty" code.

</details>

<details>
<summary>If removing a null leaves an empty object, is that removed as well?</summary>

Only if **Also remove empty objects** is on — and then yes, it cascades. The tree is cleaned
bottom-up, so an object emptied by the prune is itself dropped, which can empty its parent, and so
on up to the root. With the box off, the emptied `{}` stays where it is.

</details>

<details>
<summary>Is the key order preserved?</summary>

Yes. Surviving keys come out in exactly the order they appeared in the input. If you also want them
alphabetized for clean diffs, run the result through the JSON key sorter tool.

</details>

<details>
<summary>What if my JSON is invalid?</summary>

You get an `invalid JSON` error naming the line and column of the problem, and nothing is removed.
The document is validated before the prune, so you never get half-cleaned or garbled output.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The whole thing runs in your browser via WebAssembly — the JSON never leaves your device, and
there's no account or upload step.

</details>

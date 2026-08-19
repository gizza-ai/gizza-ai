## About this tool

**JSON Key Case Converter** parses your JSON, rewrites every object key into the naming convention
you pick — camelCase, PascalCase, snake_case, kebab-case or SCREAMING_SNAKE_CASE — and re-serializes
the document. **Values are never touched**: strings, numbers, booleans and `null` come out exactly as
they went in, and keys keep their original order.

That is the everyday chore when data crosses a language boundary: a Python or Ruby service emits
`snake_case`, a TypeScript frontend wants `camelCase`, a config schema wants `kebab-case`, and an
environment map wants `SCREAMING_SNAKE_CASE`. Instead of writing a one-off mapper, paste the payload
and switch conventions.

Renaming is **recursive** by default — every level of nesting, including objects inside arrays. Turn
**Rename keys at every nesting level** off to rename only the outermost object's keys (for a root
array, the keys of the objects directly inside it).

### Worked example

Input:

```json
{ "user_id": 1, "profile_data": { "first_name": "ada", "home-address": { "zip_code": "94107" } } }
```

With **Target key case = camelCase** and **Indent = 2**, the output is:

```json
{
  "userId": 1,
  "profileData": {
    "firstName": "ada",
    "homeAddress": {
      "zipCode": "94107"
    }
  }
}
```

Switch the target to **snake_case** and `{ "lineItems": [ { "itemId": 1 } ] }` becomes
`{ "line_items": [ { "item_id": 1 } ] }`. Set **Indent** to `0` to collapse the result onto one
compact line.

### How keys are split into words

Each key is split before it is rejoined in the target case:

- Any run of non-alphanumeric characters is a word boundary — `first_name`, `home-address` and
  `first name` all split the same way.
- A lowercase→uppercase hump starts a new word: `firstName` → `first` + `Name`.
- Acronym runs break before their last capital when a lowercase letter follows, so `HTTPResponse` →
  `HTTP` + `Response` (camelCase `httpResponse`, snake_case `http_response`) and `userID` → `user` +
  `ID` (`user_id`).
- Digits stay attached to the word they follow: `utf8Encoding` → `utf8_encoding`, `address1` stays
  `address1`, `v2Api` → `v2-api`.

### Options

- **Target key case** — camelCase (default), PascalCase, snake_case, kebab-case or
  SCREAMING_SNAKE_CASE.
- **Rename keys at every nesting level** — on by default; off renames only the outermost object.
- **Keys to leave untouched** — a comma-separated list of exact, case-sensitive key names that are
  copied through unchanged (`Content-Type`, `X-Api-Key`, or an object whose keys are data such as
  ids or dates).
- **Keep leading `_` `$` `@` sigils** — on by default, so `_id` stays `_id`, `__typename` stays
  `__typename` and `$schema_url` becomes `$schemaUrl`. Turn it off to strip the sigil (`_id` → `id`).
- **Indent spaces** — 1–8 spaces per level (default 2), or `0` to minify.

### Limits and edge cases

- Maximum input size **5 MB**; maximum nesting depth **100 levels**. Both are reported as plain
  errors rather than truncated output.
- Invalid JSON is rejected with the parser's exact line and column — this tool converts valid JSON,
  it does not repair broken JSON.
- If two different keys in the same object would become the same name (`user_name` and `userName`
  both → `userName`), the conversion **fails with an error naming both keys and the JSON path**
  instead of silently dropping one of them.
- Keys with no letters or digits at all (`"___"`, `"@"`) are passed through unchanged.
- Non-ASCII keys are converted with Unicode-aware casing (`größe_wert` → `größeWert`).

## FAQ

<details>
<summary>Does it rename keys inside nested objects and arrays?</summary>

Yes. With **Rename keys at every nesting level** on (the default) every object key is rewritten at
every depth, including objects that live inside arrays. Array element order and array contents are
never reordered — only the keys of the objects inside them change.

</details>

<details>
<summary>How are acronyms like `userID` or `HTTPResponse` handled?</summary>

Splitting is acronym-aware. A run of capitals is kept together and broken only before its last
capital when a lowercase letter follows, so `userID` → `user_id`, `parseJSONBody` →
`parse_json_body`, and `HTTPResponse` → `httpResponse` in camelCase. A naive splitter would give you
`user_i_d`; this one does not.

</details>

<details>
<summary>What happens to `_id`, `$schema` and `__typename`?</summary>

Leading sigils are preserved by default, so MongoDB's `_id`, JSON Schema's `$schema` and GraphQL's
`__typename` survive the conversion and only the rest of the name is rewritten — `$schema_url`
becomes `$schemaUrl`. Uncheck **Keep leading `_` `$` `@` sigils** if you want them stripped
(`_id` → `id`).

</details>

<details>
<summary>Two of my keys collide after conversion — why did it error?</summary>

Because losing data silently is worse than failing. If an object contains both `user_name` and
`userName`, converting to camelCase would map both onto `userName` and one value would be
overwritten. Instead you get `key collision at $.path: "user_name" and "userName" both become
"userName"` — rename one in the source, or add it to **Keys to leave untouched**.

</details>

<details>
<summary>Are values or key order modified?</summary>

No. Only key *names* change. Values of every type are re-emitted unchanged, and keys keep their
original insertion order — the output diffs cleanly against the input. If you also want keys sorted,
run the result through a JSON key sorter afterwards.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The conversion runs entirely in your browser via WebAssembly — the JSON never leaves your
device, so it is safe to paste API payloads that contain real data.

</details>

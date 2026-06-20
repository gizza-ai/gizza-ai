## About this URL encoder / decoder

This free online tool percent-encodes (URL-encodes) and decodes text and URLs
instantly, right in your browser. Nothing is sent to a server — it runs locally,
works offline, and needs no sign-up.

### Encode vs decode

- **Encode** turns unsafe characters into `%XX` escapes so text can travel
  safely inside a URL. For example, a space becomes `%20` and `ã` becomes
  `%C3%A3`.
- **Decode** reverses that, turning `%XX` escapes back into the original
  characters.

Set the **Mode** field to `encode` (the default when blank) or `decode`.

### Target: component, whole-URL, or form

The **Target** field controls the encoding style:

- **component** (the default) escapes *everything* that isn't a letter, digit,
  or one of `- _ . ~`. Use this for a single query-string value or a single path
  segment, where characters like `&`, `=`, `/`, and `?` must be escaped so they
  aren't mistaken for delimiters. It matches JavaScript's `encodeURIComponent`.
- **uri** preserves the reserved characters that give a URL its structure
  (`: / ? # [ ] @ ! $ & ' ( ) * + , ; =`) and only escapes genuinely unsafe
  bytes such as spaces and non-ASCII characters. Use this to clean up an entire
  URL without breaking it. It matches `encodeURI`.
- **form** is `application/x-www-form-urlencoded` — the format HTML forms and
  many query strings use. It encodes like *component*, except a space becomes
  `+` instead of `%20`. When decoding with **form**, a `+` turns back into a
  space. Use this when a value will sit in a posted form body or a `+`-style
  query string.

### Batch: convert each line separately

Turn on **Per line** (set it to `true`) to encode or decode every line of the
input independently, keeping the line breaks between them. This is handy for a
list of values or URLs — paste one per line and convert them all at once,
without the newlines themselves getting encoded.

### Repeat: un-nesting double-encoded data

Sometimes a value gets URL-encoded more than once (for example `a b` →
`a%20b` → `a%2520b`), which is a common source of bugs when a string passes
through several systems. Set **Repeat** to a number from 1 to 16 to apply the
operation that many times — decoding with `repeat = 2` un-nests a
double-encoded string back to the original. (You can also double-*encode* by
repeating the encode operation.)

### Examples

- `São Paulo` → `S%C3%A3o%20Paulo` (encode, component)
- `name=John Doe&city=x` → `name%3DJohn%20Doe%26city%3Dx` (encode, component)
- `https://ex.com/a b?x=1&y=2` → `https://ex.com/a%20b?x=1&y=2` (encode, uri)
- `a b+c` → `a+b%2Bc` (encode, form — space becomes `+`, a literal `+` becomes `%2B`)
- `a%2520b` → `a b` (decode, repeat 2 — un-nests double encoding)
- `S%C3%A3o%20Paulo` → `São Paulo` (decode)

### FAQ

**Is it really free and private?** Yes — your input never leaves your device.

**Does it work offline?** Yes, once the page has loaded.

**When should I use `form` instead of `component`?** Use **form** when the value
goes into an HTML form submission or a query string that encodes spaces as `+`
(the `application/x-www-form-urlencoded` convention). Use **component** for
modern percent-encoded URLs where a space is `%20`.

**What's the difference between this and `encodeURIComponent`?** The default
"component" target matches `encodeURIComponent`; the "uri" target matches
`encodeURI`; the "form" target additionally turns spaces into `+`.

**My string looks like it's encoded twice — how do I fix it?** Decode with the
**Repeat** field set to 2 (or higher) to peel off each layer of encoding.

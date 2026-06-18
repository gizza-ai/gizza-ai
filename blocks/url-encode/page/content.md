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

### Component vs whole-URL

When encoding, the **Target** field controls how aggressive the escaping is:

- **component** (the default) escapes *everything* that isn't a letter, digit,
  or one of `- _ . ~`. Use this for a single query-string value or a single path
  segment, where characters like `&`, `=`, `/`, and `?` must be escaped so they
  aren't mistaken for delimiters.
- **uri** preserves the reserved characters that give a URL its structure
  (`: / ? # [ ] @ ! $ & ' ( ) * + , ; =`) and only escapes genuinely unsafe
  bytes such as spaces and non-ASCII characters. Use this to clean up an entire
  URL without breaking it.

Target is ignored when decoding.

### Examples

- `São Paulo` → `S%C3%A3o%20Paulo` (encode, component)
- `name=John Doe&city=x` → `name%3DJohn%20Doe%26city%3Dx` (encode, component)
- `https://ex.com/a b?x=1&y=2` → `https://ex.com/a%20b?x=1&y=2` (encode, uri)
- `S%C3%A3o%20Paulo` → `São Paulo` (decode)

### FAQ

**Is it really free and private?** Yes — your input never leaves your device.

**Does it work offline?** Yes, once the page has loaded.

**What's the difference between this and `encodeURIComponent`?** The default
"component" target matches `encodeURIComponent`; the "uri" target matches
`encodeURI`.

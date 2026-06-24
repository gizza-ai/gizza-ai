## What this tool does

Decode a string produced by PHP's `serialize()` back into readable **JSON**, so
you can inspect it outside of PHP. It runs entirely in your browser — nothing is
sent to a server, it works offline, and there is no sign-up.

This is handy when a non-PHP system (Node, Python, a shell script) has to read a
value a PHP application wrote — a session payload, a cached object, or a
WordPress `wp_options` row stored in PHP's serialized format. It is the inverse
of the JSON → PHP `serialize()` converter.

## The format

PHP's serialized format is compact and type-tagged. Each value carries its type:

| PHP serialize() | JSON |
| --- | --- |
| `N;` | `null` |
| `b:1;` / `b:0;` | `true` / `false` |
| `i:42;` | `42` |
| `d:3.5;` | `3.5` |
| `s:2:"hi";` | `"hi"` |
| `a:3:{i:0;i:1;i:1;i:2;i:2;i:3;}` | `[1, 2, 3]` |
| `a:1:{s:4:"name";s:2:"Al";}` | `{"name": "Al"}` |
| `O:4:"User":1:{s:2:"id";i:7;}` | `{"__class": "User", "id": 7}` |

## How arrays and objects are mapped

- **A list array becomes a JSON array.** When a PHP array's keys are exactly the
  sequential integers `0, 1, 2, …` in order, it is rendered as a JSON array —
  matching PHP's own `json_encode`.
- **Any other array becomes a JSON object.** String-keyed arrays, sparse arrays
  (with gaps), and arrays whose integer keys are out of order are rendered as a
  JSON object with the keys as strings. Key order is preserved.
- **Serialized objects keep their class name.** An `O:…` object value becomes a
  JSON object with the original class name under a `"__class"` field, so the
  type information is not lost.

## Notes that trip people up

- **String length is in bytes, not characters.** A `"€"` is one character but
  three UTF-8 bytes, so PHP writes it as `s:3:"€";`. This tool reads the byte
  length exactly as PHP does, which is why a serialized string can safely contain
  quotes and semicolons — the length prefix says where it ends.
- **Non-finite doubles become `null`.** PHP can serialize `INF`, `-INF`, and
  `NAN`, but JSON has no way to represent them, so they decode to `null`.
- **Invalid input is rejected with a clear message.** A truncated string, a wrong
  length prefix, or trailing data after the value produces an error rather than a
  silently wrong result.

## Examples

| PHP input | JSON output |
| --- | --- |
| `a:2:{s:4:"name";s:2:"Al";s:3:"age";i:30;}` | `{"name": "Al", "age": 30}` |
| `a:3:{i:0;b:1;i:1;N;i:2;d:1.5;}` | `[true, null, 1.5]` |
| `s:5:"café";` | `"café"` |
| `a:1:{s:5:"items";a:2:{i:0;i:1;i:1;i:2;}}` | `{"items": [1, 2]}` |

## FAQ

**Is it free and private?** Yes — your input never leaves your device, and the
page keeps working offline once it has loaded.

**Where does this serialized string come from?** Anywhere PHP's `serialize()` is
used: `$_SESSION` files, object caches, queue payloads, and WordPress options and
post meta are common sources.

**Why does my string length look "too big"?** Because PHP counts string length in
bytes, not characters — accented and non-Latin text takes more than one byte per
character. The byte count is what the format requires.

**What is the `__class` field?** When the serialized value is a PHP object
(`O:…`), the original class name is preserved under `"__class"` so you can tell
which class produced the data. Plain arrays do not get this field.

**Does it handle nested arrays and objects?** Yes, to any depth — nested values
are decoded recursively.

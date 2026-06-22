## What this tool does

Convert **JSON** into the string format produced by PHP's `serialize()`, so a PHP
app's `unserialize()` can read it back. It runs entirely in your browser —
nothing is sent to a server, it works offline, and there is no sign-up.

This is handy when a non-PHP system (Node, Python, a shell script) has to write a
value that a PHP application will read — a session payload, a cached object, or a
WordPress `wp_options` row stored in PHP's serialized format.

## The format

PHP's serialized format is compact and type-tagged. Each value carries its type:

| JSON | PHP serialize() |
| --- | --- |
| `null` | `N;` |
| `true` / `false` | `b:1;` / `b:0;` |
| `42` | `i:42;` |
| `3.5` | `d:3.5;` |
| `"hi"` | `s:2:"hi";` |
| `[1, 2, 3]` | `a:3:{i:0;i:1;i:1;i:2;i:2;i:3;}` |
| `{"name": "Al"}` | `a:1:{s:4:"name";s:2:"Al";}` |

## Notes that trip people up

- **String length is in bytes, not characters.** A `"€"` is one character but
  three UTF-8 bytes, so it serializes as `s:3:"€";`. This tool counts bytes
  correctly, which is exactly what PHP expects — getting it wrong makes
  `unserialize()` fail.
- **JSON objects and arrays both become PHP arrays.** PHP has no separate plain
  "map" type — `{"a": 1}` and `["a"]` are both `a:...` arrays, keyed by the
  object's string keys or by sequential integer indices respectively. This
  matches `serialize(json_decode($json, true))`.
- **Object key order is preserved**, in the order it appears in your JSON.
- **Big integers and fractions become doubles.** A whole number that fits a
  64-bit integer is emitted as `i:`; a fractional value, or an integer too large
  for 64 bits, is emitted as `d:` — the same fallback PHP uses when a literal
  overflows the platform integer.

## Examples

| JSON input | PHP output |
| --- | --- |
| `{"name": "Al", "age": 30}` | `a:2:{s:4:"name";s:2:"Al";s:3:"age";i:30;}` |
| `[true, null, 1.5]` | `a:3:{i:0;b:1;i:1;N;i:2;d:1.5;}` |
| `"café"` | `s:5:"café";` |
| `{"items": [1, 2]}` | `a:1:{s:5:"items";a:2:{i:0;i:1;i:1;i:2;}}` |

## FAQ

**Is it free and private?** Yes — your input never leaves your device, and the
page keeps working offline once it has loaded.

**Can I unserialize the output in PHP?** Yes. Paste the result into
`unserialize($string)` in any PHP 7+ install and you get the equivalent array or
scalar back.

**Why does my string length look "too big"?** Because PHP counts string length
in bytes, not characters — accented and non-Latin text takes more than one byte
per character. The byte count here is what `unserialize()` requires.

**Does it handle nested arrays and objects?** Yes, to any depth — nested arrays
and objects are serialized recursively.

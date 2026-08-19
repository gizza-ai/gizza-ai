## About this tool

INI files — `.ini`, `.conf`, `.cfg`, `php.ini`, `.gitconfig`, systemd units, `tox.ini` — are still
everywhere, but almost nothing else speaks them. This converter turns that text into JSON your
scripts, editors and APIs can read, turns JSON back into an INI file, and can read or rewrite one
single setting without touching the rest of the file.

Everything runs locally in WebAssembly inside your own browser tab. Config files hold hostnames,
ports and credentials, so nothing is uploaded, logged or stored.

### Worked example — INI to JSON

Input:

```ini
# app config
app = demo

[server]
host = localhost
port = 8080  ; listen port

[db]
name = main
```

Output (with **Detect booleans & numbers** off, so every value stays a lossless string):

```json
{
  "app": "demo",
  "server": {
    "host": "localhost",
    "port": "8080"
  },
  "db": {
    "name": "main"
  }
}
```

Keys above the first `[section]` header stay at the root of the object; each section becomes a
nested object. Paste that JSON back in with mode **JSON → INI** and you get the INI form again.

### Worked example — set one key in place

With mode **Set**, key `server.port` and value `9090`, the same input comes back as:

```ini
# app config
app = demo

[server]
host = localhost
port = 9090  ; listen port

[db]
name = main
```

Only the one line changed. The comment header, the blank lines, the key order and the trailing
`; listen port` note all survive, because the edit modes rewrite a single line rather than
re-serialising the document. **Get** returns just `8080`, and **Delete** removes the key — or the
whole `[server]` section when you name a section and leave the key blank.

### Modes

- **Auto-detect direction** — input starting with `{` is treated as JSON, anything else as INI.
- **INI → JSON** / **JSON → INI** — force one direction.
- **Get** — return one key's value, or a whole section as JSON when the key is blank.
- **Set** — write a value to one key, creating the key (or the section) if it doesn't exist.
- **Delete** — remove one key, or a whole section when only the section is given.

Address a setting with the **Section** and **Key** fields, or with a dotted key alone —
`database.port` means key `port` inside `[database]`. Get, set and delete all expect INI input.

### Limits and edge cases

- Input is capped at 100,000 lines; JSON nesting is capped at 64 levels.
- A key repeated inside one section becomes a JSON array, so nothing is silently dropped. Going
  the other way, a JSON array of scalars is written back as repeated key lines.
- Nested JSON deeper than one level becomes dotted section headers: `{"a":{"b":{"y":"2"}}}` writes
  `[a]` followed by `[a.b]`.
- A JSON array or bare scalar at the top level has no INI form and is rejected — the top level
  must be an object. So are keys containing `=`, `:`, `[` or a newline, and string values
  containing a newline.
- Values whose edges would not survive a read-back (leading/trailing spaces, a leading quote, or
  text that looks like a trailing comment) are quoted automatically on the way out.
- CRLF input keeps CRLF line endings; LF input keeps LF.
- Comments are stripped when converting to JSON — JSON has no comment syntax. Use the get / set /
  delete modes when you need to keep them.

## FAQ

<details>
<summary>Does converting to JSON and back preserve my comments?</summary>

No — JSON has no comment syntax, so a round trip through JSON drops comment lines and blank-line
grouping. That is exactly why the **Get**, **Set** and **Delete** modes exist: they edit the INI
text directly and rewrite only the targeted line, so comments, blank lines, key order and the
file's existing `key=value` versus `key = value` spacing all survive.

</details>

<details>
<summary>Why are my numbers and true/false coming out as strings?</summary>

That's the default, and it's deliberate: INI has no types, so keeping every value as a string is
lossless and round-trips exactly. Tick **Detect booleans & numbers** to convert unquoted values —
`true`/`false`/`yes`/`no`/`on`/`off` become JSON booleans, and integers and decimals become JSON
numbers. Quoted values such as `name = "8080"` always stay strings, which is how you force a
numeric-looking value to remain text.

</details>

<details>
<summary>What happens to a key that appears twice in the same section?</summary>

Both values are kept: the key becomes a JSON array, in file order. So `server = a` followed by
`server = b` under `[hosts]` converts to `{"hosts": {"server": ["a", "b"]}}`, and converting that
JSON back writes the two lines again. **Get** returns every occurrence, one per line. **Set** is
the exception — it collapses the repeats to a single line holding your new value, since a set is
an instruction to make the key have one value.

</details>

<details>
<summary>How do I keep a trailing comment out of the value — or inside it?</summary>

The **Treat ' ; note' as a trailing comment** option (on by default) ends a value at a `;` or `#`
that is preceded by whitespace, so `port = 8080  ; listen port` converts to `8080`, and a **Set**
on that key keeps the note attached. Turn it off when a value legitimately contains a `#` or `;` —
a URL fragment or a password, for example — and the whole rest of the line is kept as the value. A
value wrapped in quotes is always kept whole either way.

</details>

<details>
<summary>Can it write `key: value` or `key=value` instead of `key = value`?</summary>

Yes. The **Delimiter** option controls lines this tool writes — the JSON → INI output and any key
that **Set** adds — and offers `key = value`, `key=value` and `key: value`. Reading is unaffected:
both `=` and `:` are always accepted as separators on input. A key that already exists keeps
whatever spacing it had, so a set never reformats a line it didn't need to.

</details>

<details>
<summary>Is this the same as a properties or TOML file?</summary>

Not quite. Java `.properties` files and simple `key = value` config share the flat part of INI
syntax and generally convert fine, but they use escapes and line continuations this parser does
not interpret. TOML looks similar — it has `[section]` headers too — but adds typed values, arrays,
inline tables and dotted keys with their own meaning, so a TOML file should go through a TOML
parser instead. This tool targets classic INI / `.conf` / `.cfg` text: `[section]` headers,
`key = value` or `key: value` pairs, `;` and `#` comments, and section-less keys at the top.

</details>

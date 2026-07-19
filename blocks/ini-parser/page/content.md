## What this tool does

Paste an INI or `.conf` file and this tool parses it into clean, structured JSON.
It understands the everyday INI shape — `[sections]`, `key = value` (or
`key: value`), `;`/`#` comments, quoted values, and section-less "global" keys
before the first section — and, crucially, it **reports duplicate keys** instead
of silently dropping them. It runs entirely in your browser: nothing is uploaded,
it works offline once the page has loaded, and there is no sign-up.

## What it parses

| INI piece | Example | Becomes |
| --- | --- | --- |
| **Section** | `[server]` | a nested JSON object named `server` |
| **Key / value** | `host = localhost` or `host: localhost` | `"host": "localhost"` |
| **Global key** | `app = demo` before any `[section]` | a key at the JSON root |
| **Comment** | `; note` or `# note` on its own line | skipped (counted in the report) |
| **Quoted value** | `greeting = "hello world"` | `"hello world"` (quotes stripped, kept a string) |

## Options

- **Output as** — **Nested JSON** (the default: globals at the root, each section a
  nested object), **Flat dotted keys** (`server.host`), or **Report** (adds a
  `duplicates` list and a `stats` block).
- **Duplicate keys** — when a key repeats inside one section, keep the **last**
  value (default), the **first**, **collect all values into an array**, or **error**
  out. Duplicates are always listed in the **Report** output.
- **Detect booleans & numbers** — off by default (every value stays a string, so
  nothing is lost). Turn it on to coerce `true/false/yes/no/on/off` to booleans and
  numeric values to numbers. Quoted values always stay strings.
- **Comment prefixes** — treat `;` and `#` (default), only `;`, or only `#` as
  comments. A disabled prefix is parsed as ordinary data.
- **Strip inline comments** — off by default. Turn it on to drop a trailing
  ` ; note` / ` # note` from a value (the comment mark must follow whitespace).

## Worked example

Paste this and leave every option at its default:

```
app = demo

[server]
host = localhost
port = 8080

[db]
name = main
```

You get:

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

Now switch **Output as** to **Report** on a file with a repeated key:

```
[db]
host = primary
host = replica
```

```json
{
  "globals": {},
  "sections": {
    "db": { "host": "replica" }
  },
  "duplicates": [
    { "scope": "db", "key": "host", "count": 2, "values": ["primary", "replica"] }
  ],
  "stats": { "sections": 1, "keys": 1, "comments": 0, "duplicates": 1 }
}
```

## Limits and edge cases

- **Value continuation / multi-line values** are not supported — each entry is a
  single line. Paste a value that already fits on one line.
- **Type detection is opt-in and lossy by design**: with it on, `007` becomes `7`
  and `1.10` becomes `1.1`. Leave it off (the default) to keep values verbatim, or
  quote a value to force it to stay a string.
- **Duplicate section headers merge** into one object (their keys are combined),
  and duplicate keys are then resolved by the chosen policy.
- **Global key vs section-name clash**: if a global key has the same name as a
  section, the section object wins in the nested output.
- Input is capped at **100,000 lines**; paste a slice of anything larger.

## FAQ

<details>
<summary>Is anything uploaded to a server?</summary>

No. All parsing happens locally in your browser with WebAssembly, so your config
files never leave your device and the tool works offline once the page has loaded.

</details>

<details>
<summary>How are duplicate keys handled?</summary>

By an explicit policy you choose. **Keep last** (the default) uses the last value,
**Keep first** uses the first, **Collect into an array** turns the repeated key
into a JSON array of every value, and **Error on duplicate** stops with a message.
Whatever you pick, the **Report** output lists each duplicate with its section,
key, occurrence count, and all values.

</details>

<details>
<summary>Does it support `key: value` as well as `key = value`?</summary>

Yes. Either `=` or `:` — whichever appears first on the line — separates the key
from the value, so both `host = localhost` and `host: localhost` parse the same
way. Keys and values are trimmed of surrounding whitespace.

</details>

<details>
<summary>What happens to comments and quoted values?</summary>

Lines starting with `;` or `#` are treated as comments and skipped (you can
restrict this to only `;` or only `#`). A value wrapped in matching single or
double quotes has the quotes stripped and is always kept as a string, which lets
you preserve leading/trailing spaces or a value that looks like a number.

</details>

<details>
<summary>Why did my value like `8080` come out as a string?</summary>

By default every value is kept as a string so nothing is lost. Tick **Detect
booleans & numbers** to convert unquoted `true/false/yes/no/on/off` to booleans
and numeric values to numbers. Quoted values always stay strings.

</details>

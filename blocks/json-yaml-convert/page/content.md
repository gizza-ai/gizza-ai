## About this tool

**JSON ⇄ YAML ⇄ TOML** converts configuration data between the three most common
config formats, in any direction. Pick the **From** and **To** formats, paste
your data, and get the converted result.

- All six directions are supported: JSON↔YAML, JSON↔TOML, and YAML↔TOML.
- The data round-trips through a shared model, so structure and values are
  preserved.
- **Pretty-print** indents JSON and TOML output (on by default).

Everything runs **locally in your browser** via WebAssembly — your config is
never uploaded.

### Notes on TOML

TOML is stricter than JSON/YAML: the top level must be a **table** (object), and
TOML has **no `null`**. Converting a top-level array or a document containing
`null` *to* TOML will report a clear error.

### Handy for

- Moving a config between tools that prefer different formats (e.g. `Cargo.toml`
  ↔ a JSON build config).
- Reading a TOML file as JSON, or vice-versa.

## FAQ

<details>
<summary>Why does converting to TOML fail when JSON and YAML accept my data?</summary>

TOML is the strict one of the three: the document root must be a
**table/object** (a top-level array like `[1, 2, 3]` has no TOML
representation) and TOML has **no null**. Either condition produces a clear
error instead of a mangled file — wrap the array under a key, or replace
`null`s, and the conversion goes through.

</details>

<details>
<summary>Do comments and YAML anchors survive the conversion?</summary>

No. The input is parsed into a pure data model (maps, lists, scalars) and
re-emitted, so `#` comments in YAML/TOML are dropped, and YAML anchors/aliases
(`&base` / `*base`) are expanded into their concrete values in the output.
Keys, values, nesting and types are what round-trips.

</details>

<details>
<summary>What exactly does the Pretty-print switch do?</summary>

It controls indentation of **JSON and TOML** output — on (the default) gives
human-readable, indented output; off gives compact single-line JSON, handy for
embedding in an environment variable or HTTP body. YAML is inherently
indented, so the switch doesn't change it.

</details>

<details>
<summary>Is it safe to paste configs with secrets in them?</summary>

Yes — the conversion runs entirely in your browser via WebAssembly. API keys,
connection strings and other config values never leave your device; nothing is
sent to a server or logged.

</details>

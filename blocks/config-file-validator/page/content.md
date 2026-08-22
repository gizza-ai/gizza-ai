## Validate pasted config files locally

Paste a JSON, YAML, TOML, INI or XML config file and get a syntax report with the
format, validity, line and column, source context, and a focused hint when the
parser error is recognizable. The validator runs entirely in your browser with
pure parsers, so secrets in config snippets are not uploaded.

Use **Auto-detect** for quick checks or force a format when a file could be read
more than one way, such as `[server]` headers in TOML and INI. Turn on **Strict
warnings** to catch portability issues after syntax passes: duplicate JSON keys,
tab indentation, byte-order marks and mixed line endings.

### Worked examples

- `gizza tool config-file-validator 'server:\n  host: localhost\n   port: 8080' format=yaml context_lines=1`
  reports the YAML indentation problem with nearby source.
- `gizza tool config-file-validator '{"a":1,}' format=json` reports the JSON
  trailing comma with a line and column.
- `gizza tool config-file-validator '[server]\nhost = "localhost"' format=toml report_format=json`
  returns a machine-readable diagnostic object.
- Use `format=auto strict=true` for a first-pass lint on pasted config files when
  you are not sure whether they are JSON, YAML, TOML, INI or XML.

### Limits and edge cases

- Input is capped at 1 MiB and diagnostics are capped at 100 entries.
- JSON, YAML, TOML and XML use real parsers. INI has no single standard, so this
  tool validates common section/key-value syntax and reports suspicious lines.
- Auto-detect keeps the first parser that succeeds; force a format when a short
  snippet is valid in more than one language.
- This checks syntax and lightweight portability warnings. It does not validate a
  product-specific schema such as Kubernetes OpenAPI, Docker Compose rules or an
  application's custom config schema.

## FAQ

<details>
<summary>Does the config text leave my browser?</summary>

No. The page uses WebAssembly parsers in your tab and the CLI runs the same pure
Rust code locally. There is no upload, network request or server-side validation
step.

</details>

<details>
<summary>Why can auto-detect choose TOML when my file looks like INI?</summary>

Some snippets, especially `[section]` plus `key = value`, are valid TOML and look
like INI. Auto-detect prefers stricter formats when they parse cleanly. Select
`ini` explicitly when you want INI-style rules.

</details>

<details>
<summary>What does strict mode add?</summary>

Strict mode adds warnings after syntax succeeds. It flags portability issues such
as duplicate JSON keys, tabs in indentation, a leading byte-order mark and mixed
line endings. These are warnings, so the report can still say the syntax is
valid.

</details>

<details>
<summary>Can this validate Kubernetes, Docker Compose or app-specific schemas?</summary>

It validates syntax only. A Kubernetes manifest can be valid YAML but still fail
Kubernetes schema rules. Use this first to fix parser errors, then run a
schema-aware validator for the specific product.

</details>

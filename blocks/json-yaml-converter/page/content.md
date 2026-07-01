## Convert JSON ⇄ YAML in your browser

Paste **JSON** or **YAML** and get the other format back instantly. Everything
runs locally in your browser — your data is never uploaded to a server.

### How it works

- **Direction = auto** (default) — input that starts with `{` or `[` is treated
  as JSON and converted to YAML; anything else is treated as YAML and converted
  to JSON. Force it with `json-to-yaml` or `yaml-to-json`.
- **Pretty-print** — indent the JSON output (only applies to YAML → JSON).
- Conversions are **value-preserving**: JSON → YAML → JSON round-trips back to
  the same data.

### Notes

- YAML is a superset of JSON, so valid JSON is also valid YAML.
- Great for turning API responses into readable config, or config files into
  JSON for tooling.

## FAQ

<details>
<summary>How does auto-detection pick the conversion direction?</summary>

It looks at the first non-whitespace character of your input: `{` or `[` means the
text is treated as JSON and converted to YAML; anything else is parsed as YAML and
converted to JSON. If that guess is wrong for your input, set the direction
explicitly to `json-to-yaml` or `yaml-to-json` (the short aliases `j2y`, `y2j`,
`json2yaml`, and `yaml2json` also work).

</details>

<details>
<summary>Will comments and anchors in my YAML survive the conversion?</summary>

No. The converter parses your document into plain data values, so `#` comments are
dropped and `&anchor`/`*alias` references are expanded into their literal values in
the JSON output. The *data* round-trips exactly; the YAML-only syntax does not.

</details>

<details>
<summary>Why doesn't Pretty-print change my YAML output?</summary>

The pretty option only applies in the YAML → JSON direction, where it switches the
JSON output from a single compact line to indented multi-line form. YAML output is
always written in standard block style, which is already readable.

</details>

<details>
<summary>Can I convert a multi-document YAML file (separated by ---)?</summary>

Not in one pass — the parser accepts a single document, so a stream of `---`
separated documents is rejected as invalid. Split the stream and convert each
document on its own.

</details>

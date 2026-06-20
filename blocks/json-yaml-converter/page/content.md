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

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

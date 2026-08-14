## About this tool

Real applications usually read more than one configuration file: a checked-in default, an
environment-specific override, a local file, and finally deployment variables. This tool merges
those layers in the same order. Paste JSON, YAML, TOML or `.env` text into the four layer boxes;
blank layers are skipped, and later layers override earlier layers.

The result is a normalized config document, not a textual patch. Comments, blank lines and YAML
anchors are not preserved because they are not part of the merged value tree. The upside is that
layers can use different formats: a JSON base, YAML staging override and `.env` deployment layer
can all merge into one JSON, YAML, TOML, `.env` or report output.

### Worked example — JSON base, YAML override and env values

Layer 1:

```json
{
  "db": { "host": "localhost", "port": 5432 },
  "debug": false
}
```

Layer 2:

```yaml
db:
  host: staging.internal
```

Layer 3:

```dotenv
DEBUG=true
DB__PORT=6543
```

With the defaults, the merged JSON is:

```json
{
  "db": {
    "host": "staging.internal",
    "port": "6543"
  },
  "debug": "true"
}
```

The YAML layer changed only `db.host`. The `.env` layer used `DB__PORT` to address `db.port` and
`DEBUG` to override `debug` using the default case-matching mode. Env values are strings, so the
port and debug values are strings in the result.

### Merge options

- **Object merge**: deep merge nested objects, or shallow-replace the whole top-level value.
- **Array merge**: replace lists (default), append lists, or append only values that are not already
  present.
- **Null deletes**: with the default on, a later `null` removes an inherited key; turn it off to keep
  `null` as a value.
- **Key case matching**: default `match` lets env-style `DB__HOST` override `db.host`; `preserve`
  treats case differences as separate keys.
- **Variable substitution**: `${VAR}`, `${VAR:-default}` and `${VAR-default}` expand after the merge.
  Values come from the **Vars** box first, then from the merged config itself (`${db.host}` or
  `${DB__HOST}`). `$$` is a literal dollar sign, and unresolvable references are left as written.

### Limits and edge cases

- Total pasted input is capped at 256 KiB across all layers and vars.
- Nesting is capped at 64 levels.
- Each layer must parse to a top-level mapping/object. Bare lists and scalars are rejected.
- TOML output cannot represent `null`, so nulls are dropped before TOML serialization.
- `.env` output flattens nested paths with `__` and uppercases them. It rejects lists of objects and
  key collisions such as `db.host` plus `db__host` becoming the same env name.
- Auto-detection is intentionally simple: valid JSON wins first, `[section]` means TOML,
  `KEY=value` without spaces means `.env`, TOML-style `key = value` is TOML when it parses, and the
  remaining mapping syntax is YAML. Force the input format when your file is ambiguous.

## FAQ

<details>
<summary>Which layer wins when two layers set the same key?</summary>

Layers apply left to right. Layer 1 is the base, layer 2 overrides it, layer 3 overrides both, and
layer 4 has the highest precedence. Use **Output format → Annotated report** when you need to see
which layer set each final value and which keys were overridden.

</details>

<details>
<summary>Can I merge JSON, YAML, TOML and .env in one run?</summary>

Yes. With **Input format → Auto-detect each layer**, every non-blank layer is sniffed independently
and converted into the same internal value tree before merging. That means a JSON base can be
overridden by YAML and then by `.env` variables. If a layer is ambiguous, choose a forced input
format so every layer is parsed the same way.

</details>

<details>
<summary>Why did my env override change a number into a string?</summary>

`.env` files are text, so `PORT=6543` is parsed as the string `"6543"`. JSON, YAML and TOML layers
keep their typed booleans and numbers. If you need a typed numeric override, use a JSON, YAML or
TOML layer instead of `.env`, or post-process the merged output with a schema-aware validator.

</details>

<details>
<summary>How do I remove a default from a later layer?</summary>

Leave **Null deletes inherited keys** on and set the key to `null` in the later JSON or YAML layer.
For example, `cache: { ttl: null }` removes an inherited `cache.ttl`. Turn the option off when you
want `null` to survive as an explicit value in JSON or YAML output.

</details>

<details>
<summary>Does variable substitution read my real environment?</summary>

No. The tool does not read browser, shell or deployment environment variables. It only uses values
you paste into the **Vars** box and values already present in the merged config tree. That makes the
run reproducible and keeps secrets out of the output unless you explicitly reference them.

</details>

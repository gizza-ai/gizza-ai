## Query YAML with jq-style filters in your browser

Paste a YAML document, type a jq/yq-style filter, and get the selected or
reshaped result back as YAML or JSON. The tool uses the same pure-Rust jaq engine
as the JSON jq tool, but parses YAML first, so common config workflows work in
one step: read docker-compose ports, list Kubernetes container images, filter CI
jobs, or convert a YAML projection to compact JSON for another tool.

Everything runs locally in WebAssembly. No YAML is uploaded, and there is no
server-side `jq` or `yq` process.

### Worked examples

- `gizza tool yaml-query --yaml 'services:\n  web:\n    ports:\n      - "80:80"' --query '.services.web.ports'`
  returns the `ports` list as YAML.
- Use `--output-format json --pretty false` with `.services | keys` to get a
  compact JSON array of service names.
- Use `--documents slurp --query 'map(.metadata.name)'` on a `---` separated
  Kubernetes stream to query across all resources at once.
- Use `--raw-output true --query '.image'` when a scalar string should be copied
  into a shell pipeline without JSON/YAML quotes.

### Limits and edge cases

- Input is capped at 4 MiB and output streams are capped at 50,000 values to keep
  browser runs bounded.
- YAML anchors, aliases, merge keys (`<<`) and custom tags are resolved into data
  before the jq filter runs. Comments and exact formatting are not preserved
  because jq-style transforms operate on a data tree.
- YAML mapping keys that are scalars become jq object keys as strings. Complex
  sequence or mapping keys are rejected because jq objects cannot represent them.
- jaq implements jq with the standard library (`map`, `select`, `sort_by`,
  `group_by`, `to_entries`, `with_entries`, `add`, `length`, `unique`, and more),
  but it may differ from a local yq installation for yq-specific assignment or
  in-place editing extensions.

## FAQ

<details>
<summary>Is this the same as yq?</summary>

It covers the browser-safe query and transform path: YAML is parsed to a jq data
tree, a jq-style filter runs, and the result is emitted as YAML or JSON. It does
not edit files in place or preserve comments, so use it for selection,
projection, filtering, aggregation, and conversion rather than source-preserving
rewrites.

</details>

<details>
<summary>How do I query a multi-document Kubernetes YAML stream?</summary>

Leave **Documents** as `each` to run the filter independently on every `---`
document. Choose `slurp` when the filter needs to see all documents at once, for
example `map(.metadata.name)` or `map(select(.kind == "Service"))`.

</details>

<details>
<summary>Why did I get several output values?</summary>

jq filters produce a stream. A filter like `.items[] | .name` emits one value per
item. Wrap the filter in brackets, such as `[.items[] | .name]`, when you want a
single array result instead.

</details>

<details>
<summary>Does this preserve YAML comments and anchors?</summary>

Anchors, aliases, merge keys, and tags are resolved before querying, so their
data is visible to the filter. Comments and original formatting are not
preserved in transformed output; the result is newly serialized YAML or JSON.

</details>

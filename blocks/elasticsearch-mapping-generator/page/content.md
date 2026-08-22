## About this tool

Paste one representative JSON document, or an array of sample documents, and this tool infers a deterministic Elasticsearch mapping. It follows Elasticsearch dynamic mapping as the baseline, then makes the common production choices explicit: text plus `.keyword` fields, date detection, numeric-string detection, `nested` arrays, `ip` fields, `geo_point` objects, dynamic-field policy, and an optional create-index wrapper with shard and replica settings.

Example input:

```json
{"id":1,"title":"Hello world","published_at":"2026-01-02T03:04:05Z","views":42,"rating":4.5,"tags":["search","mapping"]}
```

With the default settings, the output includes `id` as `long`, `published_at` as `date`, `views` as `long`, `rating` as `float`, and `title` as a `text` field with a `keyword` sub-field. Use the output-shape menu when you need only `properties`, a `{ "mappings": ... }` body, or a full create-index body with `settings`.

Limits and edge cases: the input must be valid JSON with an object or an array of objects at the root. Empty arrays and `null` values do not create fields. Multiple samples are merged field-by-field; integer plus fractional observations widen to the configured float type, while incompatible observations fall back to the selected string strategy. The generator emits the modern typeless mapping shape used by current Elasticsearch versions; legacy mapping-type wrappers are intentionally not generated.

## FAQ

<details>
<summary>Does this exactly match Elasticsearch dynamic mapping?</summary>

It uses Elasticsearch dynamic mapping as the default mental model, but it is more conservative when several samples disagree. Elasticsearch decides from the first observed value in an index; this tool merges all supplied samples so `42` plus `4.5` becomes the configured float type, and incompatible shapes become the selected string field strategy instead of silently choosing the first type.

</details>

<details>
<summary>When should I choose `nested` instead of `object` for arrays?</summary>

Keep the default `object` when flattened arrays are acceptable and you want the same behavior Elasticsearch uses by default. Choose `nested` when each object in the array must be queried as its own unit, such as matching a line-item `sku` and `qty` from the same order line. Nested fields are more precise for those queries, but they add indexing and query overhead.

</details>

<details>
<summary>Why are per-field manual overrides not in the form?</summary>

The public page uses a single declarative parameter form, so a full editable field table would be awkward and easy to desynchronize from the generated JSON. Generate the closest mapping here, then edit individual fields in your editor before sending the body to Elasticsearch.

</details>

<details>
<summary>Which Elasticsearch versions is the output for?</summary>

The output is the modern typeless mapping shape used by current Elasticsearch releases. Older clusters that require mapping-type wrappers need a small manual wrapper around the generated `properties`; the inference itself is still useful, but this tool does not emit legacy type names.

</details>

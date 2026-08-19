## About this tool

This tool deep-merges multiple YAML documents into one YAML result. Paste a
`---`-separated stream the same way you would layer several Helm values files:
the first document is the base, each later document overrides it, and the merged
YAML is produced locally in your browser.

The default mode matches the common Helm mental model:

- mappings merge recursively;
- when two scalar values conflict, the later document wins;
- arrays are replaced by the later document;
- a key set to `null` in a later document deletes that key.

Those defaults are useful for Kubernetes and application config, but the controls
let you choose stricter or more expansive behavior. Use **Conflict precedence**
to keep the first value or reject conflicts, **Array merge** to append or merge
lists, **Array item key** to line up object-list entries such as containers or
environment variables by `name`, and **Sort keys A-Z** when you want a stable
alphabetized output for diffs.

### Worked example

Input:

```yaml
image:
  repository: app
  tag: "1.0"
replicas: 1
service:
  ports:
    - 80
---
image:
  tag: "2.0"
service:
  ports:
    - 443
```

Merged result with defaults:

```yaml
image:
  repository: app
  tag: "2.0"
replicas: 1
service:
  ports:
    - 443
```

`image.repository` stays from the base document, `image.tag` is overridden by the
second document, and `service.ports` is replaced because Helm-style array merging
uses replacement by default.

### Array merge modes

- **Replace arrays** keeps the winning document's list. This is the default and
  matches Helm values layering.
- **Append arrays** concatenates the base list and override list.
- **Append unique items** concatenates, then removes duplicate YAML values while
  preserving first-seen order.
- **Merge object lists by key** lines up mapping items whose `array_key` field
  matches, then deep-merges those pairs. The default key is `name`, which fits
  Kubernetes-style `env`, `containers`, and similar lists.

### Limits and edge cases

- Input is capped at **1 MiB** and **20 YAML documents** per merge.
- The output is canonical YAML, not a byte-for-byte patch. Comments, blank-line
  layout, tags, and anchors are not preserved by the YAML value model.
- Empty documents are ignored, just like empty values files.
- `null` deletes keys only when **Null deletes keys** is enabled. Turn it off when
  you need `key: null` to remain in the final YAML.
- **Error on conflict** reports the key path that disagreed. Identical values and
  newly added keys are still allowed.
- **Shallow top-level merge** replaces entire nested subtrees below a top-level
  key instead of recursively merging them.

## FAQ

<details>
<summary>Does this match Helm values file merging?</summary>

The defaults are intentionally Helm-like for day-to-day values files: later
layers win, mappings merge recursively, lists are replaced, and `null` removes a
key. Helm has additional chart-specific behavior outside raw YAML values, but for
plain layered values documents this tool mirrors the rules most people need to
preview.

</details>

<details>
<summary>How do I merge Kubernetes lists such as containers or env vars?</summary>

Choose **Merge object lists by key** and leave **Array item key** as `name` for
common Kubernetes lists. Items with the same `name` are deep-merged, unmatched
items are kept in order, and items without that key are appended because there is
nothing safe to line up.

</details>

<details>
<summary>Why did my comments or anchors disappear?</summary>

The merge operates on parsed YAML values, not source spans. Comments, blank-line
layout, custom tags, and anchor names are not part of that value tree, so the
result is re-emitted as canonical YAML. This makes the merge deterministic but
not comment-preserving.

</details>

<details>
<summary>What happens if two documents disagree on the same value?</summary>

By default the later document wins. Set **Conflict precedence** to **First
document wins** to keep the base value, or **Error on conflict** to reject any
non-identical scalar/list disagreement and report the YAML path that conflicted.

</details>

<details>
<summary>Can I merge more than two YAML files?</summary>

Yes. Paste them into one stream separated by lines containing `---`. The tool
merges left-to-right, so each later document is applied on top of the accumulated
result. The current limit is 20 documents and 1 MiB total input.

</details>

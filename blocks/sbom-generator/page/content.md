## About this tool

SBOM Generator turns a resolved dependency lockfile into a Software Bill of Materials (SBOM). Paste an npm `package-lock.json`, a Rust `Cargo.lock`, or a Python `requirements.txt`, choose CycloneDX JSON, SPDX JSON, or SPDX tag-value, and the tool emits a deterministic dependency inventory with package URLs (`purl`s).

It does not run a package manager or contact registries. A lockfile already records the dependency set, so this tool parses the file locally in WebAssembly and serializes the result. That makes it useful before sharing a project snapshot, attaching an SBOM to a release, or comparing inventories in CI.

### Worked example

Given this small `package-lock.json`:

```json
{"name":"my-app","version":"1.0.0","lockfileVersion":3,"packages":{"":{"name":"my-app","version":"1.0.0"},"node_modules/lodash":{"version":"4.17.21"}}}
```

with **input format** `npm` and **output** `cyclonedx-json`, the result includes a CycloneDX 1.6 document whose metadata component is `my-app@1.0.0` and whose components list contains a package URL like:

```text
pkg:npm/lodash@4.17.21
```

Switch **output** to `spdx-tag` to emit SPDX 2.3 tag-value text instead, including `SPDXVersion`, package entries, purl external references, and root `DEPENDS_ON` relationships.

### Limits & notes

- Supported inputs: npm `package-lock.json` v1/v2/v3, Rust `Cargo.lock`, and pinned or unpinned pip `requirements.txt` lines.
- The tool parses lockfiles as text in memory and caps component inventories at 50,000 packages.
- It emits a flat component inventory plus root package metadata. It does not resolve missing transitive dependencies, query vulnerability databases, enrich license metadata from registries, or convert existing SBOMs.
- JSON output is deterministic by default. Leave **timestamp** blank to avoid embedding the current clock; set it only when your workflow requires a real creation time.

## FAQ

<details>
<summary>What is an SBOM?</summary>

An SBOM, or Software Bill of Materials, is an inventory of the packages that make up a project. Security, compliance, and release workflows use it to answer questions like "what dependencies are in this build?" and "which package URL identifies each component?"

</details>

<details>
<summary>Why does this tool read lockfiles instead of package manifests?</summary>

A manifest such as `package.json` or `Cargo.toml` often describes ranges like `^1.2` or workspace rules that require a package manager to resolve. A lockfile records the resolved packages and versions, so it can be converted locally without network access or registry credentials.

</details>

<details>
<summary>Which output format should I choose?</summary>

Choose `cyclonedx-json` for the common CycloneDX JSON workflow, `spdx-json` when your tooling expects SPDX as JSON, and `spdx-tag` when a scanner or compliance system wants SPDX tag-value text. All three include package URLs for the dependencies the parser found.

</details>

<details>
<summary>Are dev dependencies included?</summary>

For npm package-lock files, **Include npm dev / optional dependencies** controls whether packages marked dev or optional are kept. Cargo.lock and requirements.txt do not carry the same dev/runtime distinction in this parser, so that option only affects npm input.

</details>

<details>
<summary>Is my lockfile uploaded?</summary>

No. The parser and serializer run in your browser through WebAssembly. The lockfile text stays on your device unless you copy the generated SBOM somewhere else.

</details>

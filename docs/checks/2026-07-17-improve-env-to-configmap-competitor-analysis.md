# env-to-configmap — competitor analysis (2026-07-17)

Scan done to set the descriptor's table-stakes. All findings are paraphrased — no competitor
copy, branding, or trademarks are reproduced. gizza runs browser-local wasm with no
server/account, so cloud/upload-only features are out-of-model.

## Competitors surveyed

1. **jsontotable.org — Environment File Converter** — converts between `.env`, JSON, YAML,
   Kubernetes ConfigMap and Kubernetes Secret; conversions run client-side in the browser with
   nothing sent to a server.
2. **ZeroData Tools — Kubernetes ConfigMap Generator** — visual YAML manifest builder, imports
   from `.env` or JSON; 100% in-browser.
3. **ZeroData Tools — Kubernetes Secret Generator** — builds Secret YAML with automatic base64
   encoding, supports multiple Secret types (Opaque, docker-registry, TLS, basic-auth); in-browser.
4. **ryuheechul/kube-env-gen** (GitHub) — CLI that generates env references to ConfigMap and
   Secret from simple key-list files.
5. **`kubectl create configmap/secret generic --from-env-file=… --dry-run=client -o yaml`** — the
   canonical CLI baseline: reads a `.env`-style file and emits a ConfigMap or Opaque Secret
   manifest (Secret values base64-encoded).
6. **Kustomize `configMapGenerator` / `secretGenerator`** — declarative generators driven by
   `envs:`/`literals:` that produce ConfigMap/Secret with an optional content-hash name suffix.

## Table stakes → decision

| Capability | Decision | Where |
|---|---|---|
| Parse `.env` KEY=value into a manifest | **in-model** | core `parse_env` |
| Emit ConfigMap **or** Secret | **in-model** | `kind` enum (`configmap` \| `secret`) |
| Base64-encode Secret values under `data` | **in-model** | `secret_encoding=data` (default), self-contained base64 |
| Plaintext `stringData` alternative for readable diffs | **in-model** | `secret_encoding=stringData` |
| `type: Opaque` Secret | **in-model** | emitted for every Secret |
| Set `metadata.name` | **in-model** | `name` param, RFC 1123 validated |
| Set `metadata.namespace` | **in-model** | `namespace` param, blank omits |
| Set `metadata.labels` | **in-model** | `labels` param, `key=value,…` |
| Keep numeric/boolean-looking values as strings (avoid `kubectl apply` type errors) | **in-model** | `yaml_scalar` quotes ambiguous scalars |
| `.env` niceties: `#` comments, blank lines, leading `export `, quoted values, inline comments, dup keys | **in-model** | `parse_env` / `unquote_value` |
| Runs locally, secrets never uploaded | **in-model** (inherent) | wasm, no network |
| Typed Secrets (docker-registry / TLS / basic-auth) | **out-of-model (scope)** | expect fixed keys + file contents, not a flat `.env`; equivalent to `kubectl create secret generic` only. Documented in FAQ |
| Content-hash name suffix (Kustomize generator behavior) | **considered, not built** | ties output to a Kustomize workflow; a stable `metadata.name` is more predictable for direct `kubectl apply` |
| Import from / export to JSON & YAML env formats | **out-of-model (scope)** | separate `.env`↔JSON/YAML converters; this tool's input is a `.env` document, output is a K8s manifest |
| Visual drag-drop file upload | **out-of-model (input model)** | gizza pure tools take pasted text; no file-picker |
| Rich styled YAML preview with copy button | **out-of-model (visual)** | this repo renders generic monospace text; the branded site repo styles it. Core emits the exact YAML any UI needs |

## Result

Descriptor ships every in-model table stake: `env`, `kind` (configmap/secret), `name`,
`namespace`, `secret_encoding` (data/stringData), `labels`. Values that look like numbers or
booleans are quoted so manifests apply cleanly. Not a duplicate of any existing block — no other
gizza tool emits Kubernetes manifests. Out-of-model: typed Secrets, format interconversion, file
upload, and branded visual styling.

Sources: [jsontotable.org Environment File Converter](https://jsontotable.org/environment-file-converter),
[ZeroData Tools Kubernetes Secret Generator](https://www.zerodatatools.com/kubernetes-secret-generator/),
[ryuheechul/kube-env-gen](https://github.com/ryuheechul/kube-env-gen),
[Kubernetes docs — Managing Secrets using Kustomize](https://kubernetes.io/docs/tasks/configmap-secret/managing-secret-using-kustomize/).

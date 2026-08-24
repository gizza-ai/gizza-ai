## About this tool

`helm template`, `kustomize build` and `kubectl get -o yaml` all hand you one enormous YAML
file with dozens of resources jammed together between `---` lines. That single stream is
awkward to review, impossible to diff usefully, and not the layout you want in Git. This tool
takes that stream apart: one resource at a time, each with a filename derived from what the
resource actually is.

The split is textual on purpose. Documents are separated on column-0 `---` and `...` markers,
which is exactly what the YAML spec says starts a document, and each body is then carried
through **byte for byte**. Your comments, key ordering, blank lines, anchors and long block
scalars come out the way you wrote them — nothing is re-serialised, re-indented or
alphabetised behind your back. YAML is parsed only to *read* `apiVersion`, `kind` and
`metadata`, so the filename can be built from them. It all runs as WebAssembly inside this
page, so a manifest full of secrets never leaves the browser tab.

### A worked example

Paste a two-resource chart render and leave every option alone:

```yaml
---
# Source: chart/templates/service.yaml
apiVersion: v1
kind: Service
metadata:
  name: web
  namespace: prod
spec:
  ports:
    - port: 80
---
# Source: chart/templates/deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: web
  namespace: prod
spec:
  replicas: 2
```

You get back:

```text
# ===== service-web.yaml =====
# Source: chart/templates/service.yaml
apiVersion: v1
kind: Service
metadata:
  name: web
  namespace: prod
spec:
  ports:
    - port: 80

# ===== deployment-web.yaml =====
# Source: chart/templates/deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: web
  namespace: prod
spec:
  replicas: 2
```

Each `# ===== … =====` line is the filename that resource would be written to, and the
Helm `# Source:` comments survived, because the body was never touched. Switch **What to
render** to *Index* and the same paste becomes an inventory instead:

```text
#  KIND        NAME  NAMESPACE  APIVERSION  LINES  FILE
1  Service     web   prod       v1          9      service-web.yaml
2  Deployment  web   prod       apps/v1     8      deployment-web.yaml

2 documents, 2 kinds
```

### Naming the files

The filename template defaults to `{kind}-{name}.yaml`, giving `deployment-web.yaml`. The
placeholders are `{kind}` (lowercased), `{Kind}` (as written), `{name}`, `{namespace}`
(`default` when the resource has none), `{apiVersion}`, `{group}` (`core` for the core group),
`{version}` and `{index}` — a 1-based, zero-padded counter, which is how you keep the apply
order visible in a directory listing: `{index}-{kind}-{name}.yaml`.

Anything else with a dot in it is read straight out of the document as a field path, so
`{metadata.labels.app}-{kind}.yaml` names files after your app label and
`{spec.template.spec.containers.0.image}` reaches into a list by position. A path that the
document does not have, or one that lands on a whole block rather than a single value, is
reported rather than silently blanked.

A `/` in the template makes directories: `{namespace}/{kind}-{name}.yaml` gives you
`prod/service-web.yaml`. Characters that do not belong in a filename are replaced with `-`,
and if two resources still end up with the same name the second becomes `…-2.yaml`, the third
`…-3.yaml`, so nothing is ever overwritten.

### Five ways to take the output

- **Files** — the default. Every document under its `# ===== filename =====` header, ready to
  copy out or save with the Download link.
- **Index** — the table above. The fastest way to answer "what is actually in this bundle?"
  without reading 4000 lines.
- **JSON** — an array with one object per resource: `file`, `apiVersion`, `kind`, `name`,
  `namespace`, `lines` and the full `content`. This is the shape to pipe into a script.
- **kustomization.yaml** — a ready `resources:` list of the filenames, in the chosen order, so
  the split directory is immediately a Kustomize base.
- **Shell script** — a POSIX `sh` script that recreates every file with heredocs, `mkdir -p`
  included for templates containing `/`. Save it, run it, and you have the directory.

### Filtering and ordering

**Keep only these** and **Drop these** take comma-separated selectors. A selector is `Kind` or
`Kind/name`, case-insensitive, with `*` and `?` wildcards: `Deployment,StatefulSet` keeps two
kinds, `Service/web-*` keeps services whose name starts with `web-`, and `*/*-canary` matches
by name across every kind. Excludes are applied after includes, so keeping
`Deployment` while dropping `*/*-canary` is a perfectly normal combination.

Output order is **document** by default — exactly as pasted, which is what you want when the
bundle is already in a deliberate order. **By kind** and **by name** sort alphabetically.
**Apply order** is the useful one: it ranks by a dependency-safe install sequence — namespaces,
quotas and policies, then service accounts, secrets and config maps, then storage, CRDs and
RBAC, then services, then workloads, with `Ingress` and `APIService` last — so applying the
files in the listed order does not fail on a resource that does not exist yet.

### Limits and edge cases

- Up to **2,000,000 bytes** and **1000 documents** per run. Past either, the split stops and
  says so rather than truncating.
- A `---` only starts a document at **column 0**. A `---` indented inside a block scalar (a
  `README` inside a ConfigMap, say) is content, and stays with its document.
- Documents holding nothing but comments and blank lines are dropped — a leading `---` from
  Helm does not produce an empty first file.
- A document with no `apiVersion`/`kind`/`metadata.name` is still emitted, named
  `unknown-unnamed.yaml`. Turn on **Drop documents that are not Kubernetes resources** to
  leave those out instead; this is how you strip the stray notes or values files out of a
  concatenated bundle.
- `kind: List` and `*List` wrappers — what `kubectl get -o yaml` returns for a collection —
  are expanded into their items by default, one resource per item. Those items are the one
  thing that *is* re-serialised, so comments inside a List are lost; untick the option to keep
  the wrapper whole.
- The **Shell script** output refuses to run if a document contains a line equal to its
  heredoc marker, since that would truncate the file it writes. Use the Files output for that
  bundle.
- Invalid YAML is reported with the number of the document it is in, so you can find it in the
  original paste.
- Splitting is textual: a resource that was invalid Kubernetes going in is still invalid
  coming out. This tool is not a linter or a schema validator.

## FAQ

<details>
<summary>Will my comments and formatting survive the split?</summary>

Yes. Document bodies are copied through byte for byte — comments (including the
`# Source: chart/templates/…` lines Helm adds), key order, indentation style, anchors and
block scalars all come out exactly as they went in. The YAML is parsed only to read
`apiVersion`, `kind` and `metadata` for the filename, and that parse never feeds back into
the output. The single exception is a `kind: List` wrapper when list expansion is on: its
items have to be re-serialised to become standalone documents, so comments inside a List are
lost. Untick **Expand kind: List wrappers into their items** if you need that wrapper intact.

</details>

<details>
<summary>How do I actually get the files onto disk?</summary>

Choose the **Shell script** output and save it as `split.sh`, then `sh split.sh` in an empty
directory — it writes each resource with a heredoc and creates any directories your filename
template implies. If you would rather not run a generated script, use the **Files** output and
copy each block under its `# ===== filename =====` header, or use the JSON output and let
your own script write `content` to `file`. The same split is available offline in the
command-line tool, which is the better route when the manifest is already a file on disk.

</details>

<details>
<summary>Can I name the files after a label instead of the kind?</summary>

Yes — any placeholder containing a dot is read as a field path into the document itself, so
`{metadata.labels.app}/{kind}-{name}.yaml` groups the output into one directory per app
label. Numeric steps index into lists, which is how
`{spec.template.spec.containers.0.image}` gets the first container's image. If a document is
missing the path you asked for, the run stops and names the placeholder rather than writing a
file with a hole in the name — usually the sign that one resource in the bundle is missing the
label you assumed everything had.

</details>

<details>
<summary>Why are two of my resources sharing a filename?</summary>

They are not — the second one gets a `-2` before the extension, the third a `-3`, and so on.
It happens when the template does not include something that actually distinguishes the two
resources: the same `ConfigMap` name in two namespaces collides under `{kind}-{name}.yaml`
because the namespace is nowhere in the name. Add it: `{namespace}-{kind}-{name}.yaml`, or
`{namespace}/{kind}-{name}.yaml` to put each namespace in its own directory. Use the **Index**
output to see every assigned filename at a glance.

</details>

<details>
<summary>What does apply order actually sort by?</summary>

A fixed dependency-safe sequence, not alphabet: `Namespace` first, then policies and quotas,
then `ServiceAccount`, `Secret` and `ConfigMap`, then storage classes and volumes, then
`CustomResourceDefinition` and the RBAC kinds, then `Service`, then the workload kinds
(`DaemonSet`, `Deployment`, `StatefulSet`, `Job`, `CronJob`), with `Ingress` and `APIService`
last. Kinds it does not know — your own custom resources — sort after the known ones, which is
normally right, because a custom resource usually needs its CRD and its operator to exist
first. Combine it with an `{index}-` prefix in the filename template and `apply -f` on the
directory replays that order.

</details>

<details>
<summary>Does it validate the manifests?</summary>

No, and deliberately so. Each document must be parseable YAML — a syntax error is reported
with the number of the document it appears in — but nothing checks that `apiVersion` exists,
that the kind is real, or that required fields are present. A bundle you cannot apply will
split perfectly happily into files you cannot apply. Keep `kubectl apply --dry-run=server` or
a schema linter in the loop for that; this tool's job is to preserve exactly what you gave it
while reorganising where it lives.

</details>

<details>
<summary>Is anything uploaded?</summary>

No. The split is a WebAssembly module running inside this page, so the manifest stays in the
browser tab — which matters, since a rendered chart routinely carries `Secret` resources with
real credentials in them. Load the page, disconnect from the network, and it still works.

</details>

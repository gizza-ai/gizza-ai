# k8s-manifest-splitter — competitor analysis (2026-08-22)

## Scope

The tool takes a multi-document Kubernetes YAML stream (`helm template`, `kustomize build`,
`kubectl get -o yaml`, or a hand-concatenated bundle) and splits it into individual resources,
naming each one from a filename template and rendering the result as a file bundle, an index
table, JSON, a `kustomization.yaml`, or a POSIX shell script that writes the files. It does not
merge, lint, or validate manifests.

## Sources reviewed

- `patrickdappollonio/kubectl-slice` — the de-facto standard krew plugin for this job; full flag
  surface reviewed (naming template, include/exclude, sort, skip-non-k8s, triple-dash, prune).
- `kubernetes-split-yaml` (Ubuntu manpage) — Go-template naming with flat and per-namespace
  presets, plus regex filters on name/namespace/kind/filename.
- `nathforge/kubectl-split-yaml` and `Agilicus/split-k8s-yaml` — minimal one-file-per-resource
  splitters used mainly for diffing.
- DevStackTools "YAML Multi-Doc Split/Merge" — the only comparable browser-based tool found:
  paste, document count, per-document panels, individual `.yaml` downloads, validate/merge
  buttons; no naming templates and no filtering.
- `yq --split-exp` recipes — the ad-hoc shell approach people reach for without installing a
  dedicated tool.

## Table-stakes capabilities

| Capability | In model? | Decision |
| --- | --- | --- |
| Split a multi-doc stream on `---` into one resource each | yes | Column-0 `---`/`...` split; bodies carried byte-verbatim so Helm `# Source:` comments and formatting survive. |
| Configurable filename template with kind/name | yes | `filename_template`, default `{kind}-{name}.yaml` — the same default kubectl-slice ships (`{{.kind \| lower}}-{{.metadata.name}}.yaml`). |
| Namespace / apiVersion / index in the filename | yes | `{namespace}`, `{apiVersion}`, `{group}`, `{version}`, `{Kind}`, `{index}` (zero-padded) placeholders. Covers kubernetes-split-yaml's flat and per-namespace presets via one template. |
| Arbitrary field access in the naming template | yes | **Gap closed this pass.** kubectl-slice's headline feature is a full Go template over the document. Added dotted-path placeholders — `{metadata.labels.app}`, `{spec.replicas}`, `{spec.template.spec.containers.0.image}` — resolved from the parsed document, with numeric steps indexing sequences. Missing or non-scalar paths are a named error, not a silent blank. Go template control flow (`if`/`range`/pipelines) stays out: it is a templating language, not a naming feature. |
| Directory structure from the template | yes | `/` in the template is preserved; the shell output emits `mkdir -p` for it. |
| Include/exclude by kind | yes | `include` / `exclude` selectors, comma-separated. |
| Include/exclude by name | yes | Same selectors in `Kind/name` form; `*/web-*` filters by name across kinds, covering kubectl-slice's separate `--include-name`/`--exclude-name` flags with one syntax. |
| Glob matching in selectors | yes | Case-insensitive `*`/`?` matcher. kubernetes-split-yaml uses regexes instead; globs match kubectl-slice and are the friendlier form for a paste-in web field. |
| Sort by kind | yes | `sort = kind` (kubectl-slice's `--sort-by-kind`), plus `name`. |
| Dependency-safe apply order | yes | `sort = apply` — **beyond every competitor reviewed**; none of them order for `kubectl apply`. |
| Skip non-Kubernetes documents | yes | `skip_non_k8s`, matching kubectl-slice's `--skip-non-k8s`; default off, and documents missing identity fields are named `unknown-unnamed.yaml` (the effect of kubectl-slice's `--allow-empty-kinds`/`--allow-empty-names`, which are its non-default escape hatches). |
| Prepend `---` to each document | yes | `include_triple_dash`, matching kubectl-slice's flag of the same name. |
| Expand `kind: List` wrappers | yes | `expand_lists`, default on — needed for `kubectl get -o yaml` output, which returns a `List`. Items are the only re-serialised content; documented. |
| Dry run / preview before writing | yes | The `index` output is the preview: an aligned kind/name/namespace/apiVersion/lines/filename table plus a document and kind count. Covers `--dry-run` and DevStackTools' "3 documents found" panel listing. |
| Get the files onto disk | yes | The `shell` output is a POSIX `sh` script of heredocs with `mkdir -p`, so a browser tool can still land a directory. |
| Machine-readable output | yes | The `json` output carries one object per resource including its YAML — nothing in the reviewed set offers this. |
| Emit a `kustomization.yaml` for the split | yes | The `kustomization` output — **beyond every competitor reviewed**; the split directory is immediately a Kustomize base. |
| Per-document download buttons (DevStackTools) | partial | The page has a single Download link for the whole rendered output. Per-file downloads would need a zip/multi-blob page control that the generic page runtime does not have; the `shell` output covers the same need in one file. Listed, not built. |
| Read an input *folder*, recursively (`-d`, `-r`, `--extensions`) | no | Filesystem traversal; there is no filesystem in the browser or the chat sandbox. Out of model. |
| `--output-dir`, `--prune`, `--quiet`, `--config` file | no | Writer-side and CLI-ergonomics concerns for a tool that writes files itself. This tool returns text; `shell` output is the writer. Out of model. |
| Merge many documents back into one stream | no | The opposite operation and a separate tool; not silently bolted on here. |
| Validate / lint the resources | no | Schema validation needs the cluster's API schemas. YAML syntax errors are reported with the offending document number; correctness of the resource is explicitly not claimed. |
| Regex (rather than glob) filters | no | kubernetes-split-yaml's `--name_re`/`--kind_re` style. Globs already cover the realistic selector cases and are far less error-prone typed into a web field. Listed, not built. |

## UX / parameter decisions

- The page ships five preset chips (helm bundle → files, bundle inventory → index, per-namespace
  folders in apply order → kustomization, write-the-files → shell, workloads-only → filtered),
  because every CLI competitor documents its behaviour through worked examples and a paste-in
  tool has to do the same in one click.
- Enum labels spell out what each output and sort choice produces, so the choice is legible
  without reading the docs.
- The manifest field is `multiline = true` so a pasted bundle keeps its newlines.
- Default output is `files` rather than `index`: the overwhelmingly common intent is the split
  itself, and the header lines make the filenames visible anyway.
- Byte-verbatim bodies were treated as a hard requirement, not a nicety — the alternative
  (parse and re-emit) loses Helm's `# Source:` comments, which are the main way people trace a
  rendered resource back to its template.
- Limits (2,000,000 bytes / 1000 documents) are stated in the descriptor and the page copy, and
  are enforced at the boundary with a test.

## Verification implications

Advertised-values matrix to cover: all five `output` choices, all four `sort` choices, both
non-default boolean states (`skip_non_k8s` on, `expand_lists` off, `include_triple_dash` on),
the dotted-path placeholder and its error, the document-cap boundary, and one exact-output CLI
case.

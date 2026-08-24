# k8s-manifest-scaffold — competitor analysis (2026-08-23)

Scan run **before** implementing the tool, per `/improve-tool` Phase 2–3. One web search
("kubernetes manifest generator online deployment service yaml scaffold tool"), then the top real
generator pages were skimmed. **Everything below is paraphrased** — no competitor copy, branding,
logos or trademarks are reproduced, and no competitor asset is used.

## Competitors skimmed

| # | Tool (paraphrased description) | Reachable? |
| - | ------------------------------ | ---------- |
| 1 | A free multi-resource K8s YAML builder (Deployment, Pod, Service, StatefulSet, Job, CronJob, ConfigMap, Secret) with live preview and no signup | yes — full form inspected |
| 2 | A "production-ready manifest" generator covering Deployment/Service/Ingress/HPA/PVC with very deep option coverage | yes — full form inspected |
| 3 | A visual generator with app/container/service sections and quick image presets | yes — full form inspected |
| 4 | A well-known single-page Deployment builder | partial — client-rendered SPA, form fields not retrievable server-side; only its positioning was observable |
| 5 | Several thinner "generate K8s YAML" pages surfaced by the same search | skimmed via search summaries only; no capability beyond 1–3 |

Fewer than five *distinct* capability profiles exist: 1–3 cover the entire feature surface, and the
rest repeat it. Recorded honestly rather than padding the list.

## Table stakes observed (params · defaults · enums)

Aggregated across tools 1–3.

| Capability | Typical shape | Typical default |
| ---------- | ------------- | --------------- |
| App / resource name | text, required | `my-app` |
| Namespace | text, optional | `default` or omitted |
| Container image | text, required | an `nginx:latest`-style sample |
| Replicas | number | 1–3 |
| Container port | number | 80 |
| Service type | enum: ClusterIP / NodePort / LoadBalancer (one adds headless/None) | ClusterIP |
| Service port + target port | numbers | mirror the container port |
| Node port | number, only meaningful for NodePort | blank / auto-assigned |
| CPU request + limit | text quantity (`100m`, `0.5`, `2`) | `100m` request |
| Memory request + limit | text quantity (`128Mi`, `1Gi`) | `128Mi` request |
| Image pull policy | enum: Always / IfNotPresent / Never | `Always` or `IfNotPresent` |
| Environment variables | repeatable key/value rows | empty |
| Labels (+ annotations) | repeatable key/value rows | `app=<name>` implied |
| HTTP health probes | liveness + readiness, path + port | off, or a `/` path |
| Live preview | YAML re-renders as fields change | — |
| Copy / download / reset | buttons on the result | — |
| Quick presets | one-click common images (web server, node, redis, postgres) | — |
| Stated best practice | "always set requests/limits"; "add probes so failures are detected" | — |

Deeper options seen on tool 2 only: startup probes, volumes/PVC, storage class + access modes,
node selector, tolerations, affinity/anti-affinity, security context, capabilities, DNS policy,
priority class, rolling-update strategy (maxSurge/maxUnavailable), termination grace period,
init containers, HPA, Ingress (host/path/class/TLS), GPU limits, CronJob scheduling.

Also observed: generated YAML is pinned to stable API groups (`apps/v1` for Deployment, `v1` for
Service), targeting modern clusters.

## Classification for this tool

### In-model — shipped in this build

Browser-local, pure-wasm, deterministic, no account/server needed:

- `name`, `image`, `namespace` (optional), `replicas`
- `container_port`, `service_port`, `service_type` enum (ClusterIP / NodePort / LoadBalancer),
  `node_port` for NodePort
- `cpu_request`, `cpu_limit`, `memory_request`, `memory_limit` (each optional; omitted cleanly when blank)
- `image_pull_policy` enum (IfNotPresent / Always / Never)
- `env` — `KEY=value` lines/pairs → container `env`
- `labels` — extra `key=value` pairs merged with the standard `app: <name>` selector label
- `probe_path` — one field that emits both a liveness and a readiness HTTP probe
- Multi-document YAML output (`---` between Deployment and Service), stable key ordering
- Validation with actionable messages: RFC 1123 names, port ranges, NodePort range 30000–32767,
  replica bounds, CPU/memory quantity syntax, env-var key syntax, label syntax, probe path
- Page: enum `<select>`s with friendly labels, sliders for replicas and the ports, placeholders on
  every text/number field, example chips standing in for competitor "quick presets", copy + reset
  (shared page chrome), worked example + limits + FAQ in the page copy

### In-model, considered and rejected for this build

- **Ingress / HPA / PVC / ConfigMap / Secret in the same output.** Buildable, but each is its own
  resource with its own option set; folding them in would triple the schema for a tool whose stated
  job is Deployment + Service. Adjacent existing tools already cover part of this ground
  (`env-to-configmap` for ConfigMap/Secret), and `k8s-manifest-splitter` handles bundles.
- **Repeatable key/value row UI for env and labels.** The page's list control joins on commas, and
  env values legitimately contain commas — a plain `KEY=value` text/textarea field round-trips
  pasted `.env` content more faithfully. Same reasoning already recorded for other list-valued fields.
- **Annotations, startup probes, security context, tolerations/affinity, rolling-update strategy,
  init containers, DNS/priority/grace settings.** Each is expressible in YAML and would fit the
  model technically, but they are long-tail knobs from the single deepest competitor; adding ~20
  more params would make the common case worse. Listed here so the gap is recorded, not hidden.

### Out-of-model — not buildable here

- Cluster-connected features: applying/validating a manifest against a live API server, dry-run
  server-side validation, image-tag existence checks.
- Accounts, saved configurations, shareable server-hosted links, team workspaces.
- Anything requiring a backend, API key, or paid tier.

## Positioning

The differentiator is not option count — it is that the manifest is generated **locally in the
browser via WebAssembly**, with the identical generator reachable from a CLI and from chat, and with
validation that explains what was expected instead of emitting invalid YAML. Competitors 1–3 all run
their generation in page JavaScript too, but none offer the same generator across three surfaces.

> Original work only — no competitor copy, branding, or trademarks were copied.
